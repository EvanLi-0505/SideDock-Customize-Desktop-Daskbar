//! Import the apps pinned on the native Windows taskbar.
//! Ported from Seelen UI `widgets/weg/native_taskbar.rs`.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use winreg::{RegKey, enums::HKEY_CURRENT_USER};

use crate::{
    error::Result,
    state::dock_items::{AppItem, Relaunch},
    windows_api::shell,
};

/// Apps pinned on the Windows taskbar, in taskbar order when it can be determined.
pub fn pinned_apps() -> Result<Vec<AppItem>> {
    let mut items: Vec<AppItem> = match order_from_registry() {
        Ok(paths) if !paths.is_empty() => paths
            .into_iter()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .filter_map(|p| item_from_shortcut(&p).ok())
            .collect(),
        _ => from_pinned_folder()?,
    };
    let mut seen = HashSet::new();
    items.retain(|item| seen.insert(item.path.to_string_lossy().to_lowercase()));
    Ok(items)
}

/// `Taskband\FavoritesResolve` embeds the `.lnk` paths as ASCII runs in a binary blob.
fn order_from_registry() -> Result<Vec<String>> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Taskband")?;
    let blob = key.get_raw_value("FavoritesResolve")?.bytes;
    let text: String = blob.iter().map(|&b| b as char).collect();
    let profile = std::env::var("USERPROFILE").map_err(|e| e.to_string())?;
    let pattern = regex::Regex::new(&format!(r"{}.+?\.\w{{2,4}}", regex::escape(&profile)))
        .map_err(|e| e.to_string())?;
    Ok(pattern
        .find_iter(&text)
        .map(|m| m.as_str().to_string())
        .collect())
}

fn from_pinned_folder() -> Result<Vec<AppItem>> {
    let Ok(app_data) = std::env::var("APPDATA") else {
        return Ok(Vec::new());
    };
    let dir = PathBuf::from(app_data)
        .join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut with_time: Vec<(AppItem, std::time::SystemTime)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")))
        .filter_map(|p| {
            let created = std::fs::metadata(&p).and_then(|m| m.created()).ok()?;
            Some((item_from_shortcut(&p).ok()?, created))
        })
        .collect();
    with_time.sort_by_key(|(_, t)| *t);
    Ok(with_time.into_iter().map(|(i, _)| i).collect())
}

pub fn item_from_shortcut(lnk: &Path) -> Result<AppItem> {
    let target = shell::resolve_lnk(lnk)?;
    let display_name = lnk
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();
    let relaunch = (!target.target.as_os_str().is_empty()).then(|| Relaunch {
        command: target.target.to_string_lossy().to_string(),
        args: (!target.args.is_empty()).then(|| target.args.clone()),
        working_dir: if target.working_dir.as_os_str().is_empty() {
            target.target.parent().map(Path::to_path_buf)
        } else {
            Some(target.working_dir.clone())
        },
    });
    Ok(AppItem {
        id: uuid::Uuid::new_v4().to_string(),
        display_name,
        umid: shell::file_umid(lnk),
        path: lnk.to_path_buf(),
        pinned: true,
        prevent_pinning: false,
        relaunch,
    })
}

/// Builds a pinned item for any file dropped on / chosen for the dock.
pub fn item_from_path(path: &Path) -> Result<AppItem> {
    if !path.exists() {
        return Err(format!("{} does not exist", path.display()).into());
    }
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
    {
        return item_from_shortcut(path);
    }
    let umid = shell::file_umid(path);
    Ok(AppItem {
        id: uuid::Uuid::new_v4().to_string(),
        display_name: shell::display_name_for(path, None),
        umid,
        path: path.to_path_buf(),
        pinned: true,
        prevent_pinning: false,
        relaunch: None,
    })
}
