//! Every application the Start menu knows about ("All apps"): classic programs and
//! packaged (Store) apps alike, read from the `shell:AppsFolder` virtual folder.
//!
//! The list is scanned in the background at startup and refreshed each time the
//! launcher opens (at most every few seconds); the webviews get it with
//! `get_start_apps` / the `start-apps-changed` event.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use windows::{
    Win32::{
        Storage::EnhancedStorage::PKEY_Link_TargetParsingPath,
        System::Com::CoTaskMemFree,
        UI::Shell::{
            BHID_EnumItems, IEnumShellItems, IShellItem, IShellItem2, KF_FLAG_DEFAULT,
            SHCreateItemFromParsingName, SHGetKnownFolderPath, SIGDN_NORMALDISPLAY,
            SIGDN_PARENTRELATIVEPARSING,
        },
    },
    core::{GUID, HSTRING, Interface, PWSTR},
};

use crate::{app, windows_api::ComGuard};

pub const EVENT_CHANGED: &str = "start-apps-changed";
/// a scan takes a few hundred ms: never more often than this
const MIN_RESCAN: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartApp {
    /// parsing name inside `shell:AppsFolder` (an AppUserModelID or a known-folder path)
    pub id: String,
    pub name: String,
    /// target executable / file when the app is a classic program
    pub path: Option<PathBuf>,
    /// packaged (Store) app: can only be started through its AppUserModelID
    pub packaged: bool,
}

static APPS: LazyLock<RwLock<Vec<StartApp>>> = LazyLock::new(|| RwLock::new(Vec::new()));
static LAST_SCAN: LazyLock<Mutex<Option<Instant>>> = LazyLock::new(|| Mutex::new(None));
static SCANNING: AtomicBool = AtomicBool::new(false);

pub fn list() -> Vec<StartApp> {
    APPS.read().clone()
}

/// Rescans in the background (unless a recent scan is still fresh).
pub fn refresh() {
    {
        let last = LAST_SCAN.lock();
        if last.is_some_and(|t| t.elapsed() < MIN_RESCAN) {
            return;
        }
    }
    if SCANNING.swap(true, Ordering::AcqRel) {
        return;
    }
    crate::utils::spawn_named("start-apps-scan", || {
        let started = Instant::now();
        match scan() {
            Ok(apps) => {
                let changed = *APPS.read() != apps;
                log::debug!(
                    "{} start apps scanned in {:?}",
                    apps.len(),
                    started.elapsed()
                );
                // icons first: the launcher must find them in memory
                crate::modules::icons::warm_up(apps.iter().map(|a| a.id.clone()).collect());
                if changed {
                    *APPS.write() = apps.clone();
                    app::emit(EVENT_CHANGED, &apps);
                }
            }
            Err(err) => log::warn!("start apps scan failed: {err}"),
        }
        *LAST_SCAN.lock() = Some(Instant::now());
        SCANNING.store(false, Ordering::Release);
    });
}

/// AppUserModelID of the packaged app installed in the folder that contains `path`:
/// `...\WindowsApps\Name_1.2.3.0_x64__publisherid\app.exe` -> `Name_publisherid!AppId`.
pub fn umid_for_package_path(path: &Path) -> Option<String> {
    const MARKER: &str = "\\windowsapps\\";
    let text = path.to_string_lossy();
    let start = text.to_lowercase().find(MARKER)? + MARKER.len();
    let folder = text.get(start..)?.split('\\').next()?;
    let name = folder.split('_').next()?;
    let (_, publisher) = folder.rsplit_once("__")?;
    if name.is_empty() || publisher.is_empty() {
        return None;
    }
    let prefix = format!("{name}_{publisher}!").to_lowercase();
    APPS.read()
        .iter()
        .find(|a| a.packaged && a.id.to_lowercase().starts_with(&prefix))
        .map(|a| a.id.clone())
}

/// Takes ownership of a shell-allocated string.
fn take(text: PWSTR) -> Option<String> {
    if text.is_null() {
        return None;
    }
    let value = unsafe { text.to_string().ok() };
    unsafe { CoTaskMemFree(Some(text.0 as *const _)) };
    value.filter(|v| !v.is_empty())
}

/// `{KNOWNFOLDERID}\rest\of\path` -> real path.
fn resolve_known_folder(id: &str) -> Option<PathBuf> {
    let rest = id.strip_prefix('{')?;
    let (guid, tail) = rest.split_once('}')?;
    let guid = GUID::try_from(guid).ok()?;
    let base = take(unsafe { SHGetKnownFolderPath(&guid, KF_FLAG_DEFAULT, None).ok()? })?;
    Some(PathBuf::from(base).join(tail.trim_start_matches('\\')))
}

fn is_uninstaller(name: &str, path: Option<&PathBuf>) -> bool {
    let name = name.to_lowercase();
    if name.contains("uninstall") || name.contains("卸载") {
        return true;
    }
    path.and_then(|p| p.file_name())
        .map(|f| f.to_string_lossy().to_lowercase())
        .is_some_and(|f| f.starts_with("unins") || f.starts_with("uninst"))
}

fn scan() -> crate::error::Result<Vec<StartApp>> {
    let _com = ComGuard::new();
    let folder: IShellItem =
        unsafe { SHCreateItemFromParsingName(&HSTRING::from("shell:AppsFolder"), None)? };
    let items: IEnumShellItems = unsafe { folder.BindToHandler(None, &BHID_EnumItems)? };

    let mut apps = Vec::new();
    let mut seen = HashSet::new();
    loop {
        let mut batch = [None];
        let mut fetched = 0;
        if unsafe { items.Next(&mut batch, Some(&mut fetched)) }.is_err() || fetched == 0 {
            break;
        }
        let Some(item) = batch[0].take() else {
            break;
        };
        let Some(id) = take(
            unsafe { item.GetDisplayName(SIGDN_PARENTRELATIVEPARSING) }.unwrap_or(PWSTR::null()),
        ) else {
            continue;
        };
        let Some(name) =
            take(unsafe { item.GetDisplayName(SIGDN_NORMALDISPLAY) }.unwrap_or(PWSTR::null()))
        else {
            continue;
        };
        if !seen.insert(id.to_lowercase()) {
            continue;
        }
        let packaged = id.contains('!');
        let path = if packaged {
            None
        } else {
            item.cast::<IShellItem2>()
                .ok()
                .and_then(|i2| take(unsafe { i2.GetString(&PKEY_Link_TargetParsingPath) }.ok()?))
                .map(PathBuf::from)
                .or_else(|| resolve_known_folder(&id))
        };
        // web links and other non-programs listed in the folder
        if path
            .as_ref()
            .is_some_and(|p| p.to_string_lossy().contains("://"))
        {
            continue;
        }
        if is_uninstaller(&name, path.as_ref()) {
            continue;
        }
        apps.push(StartApp {
            id,
            name,
            path,
            packaged,
        });
    }
    Ok(apps)
}
