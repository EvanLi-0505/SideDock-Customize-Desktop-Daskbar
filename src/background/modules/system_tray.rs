//! Notification area ("system tray") icons of every application, for the toolbar.
//!
//! Applications register tray icons with `Shell_NotifyIcon`, which sends `WM_COPYDATA`
//! to Explorer's `Shell_TrayWnd`. SideDock loads `sidedock_tray.dll` (the `tray-hook`
//! crate) into that window's thread with a `WH_CALLWNDPROC` hook; the DLL forwards a copy
//! of each message to [`RECEIVER_CLASS`], a hidden window owned by this module. Icons are
//! kept here in registration order and served to the toolbar; clicks are forwarded to
//! the owning application exactly like Explorer does. Ported from Seelen UI's
//! `system_tray` module.
//!
//! * Existing icons: after hooking, the "TaskbarCreated" message is broadcast, which makes
//!   every application register its icons again (what they do when Explorer restarts).
//! * Explorer restarts: the receiver gets "TaskbarCreated" too and hooks the new taskbar.
//! * Applications that exit without removing their icons are pruned periodically.

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        LazyLock,
        atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering},
    },
    time::Duration,
};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE, HWND, LPARAM, LRESULT, POINT, WPARAM},
        System::{
            Com::CoTaskMemFree,
            DataExchange::COPYDATASTRUCT,
            LibraryLoader::{GetProcAddress, LoadLibraryW},
        },
        UI::Shell::{KNOWN_FOLDER_FLAG, SHGetKnownFolderPath},
        UI::WindowsAndMessaging::{
            AllowSetForegroundWindow, CopyIcon, CreateWindowExW, DefWindowProcW, DestroyIcon,
            DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos, GetMessageW,
            GetWindowThreadProcessId, HHOOK, HICON, HOOKPROC, HWND_BROADCAST, IsWindow, MSG,
            PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, ReplyMessage,
            SendNotifyMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
            WH_CALLWNDPROC, WINDOW_EX_STYLE, WM_CLOSE, WM_CONTEXTMENU, WM_COPYDATA, WM_DESTROY,
            WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
            WM_RBUTTONDOWN, WM_RBUTTONUP, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
    core::{GUID, HSTRING, PCSTR, PCWSTR, w},
};

use crate::{
    app,
    error::Result,
    state::settings,
    windows_api::{icons, shell, window::Window},
};

pub const EVENT_CHANGED: &str = "system-tray-changed";
/// Must match the class the hook DLL looks for.
const RECEIVER_CLASS: &str = "SideDockTrayReceiver";
const DLL_NAME: &str = "sidedock_tray.dll";
/// PNG size of an icon whose own size cannot be read (icons keep their native size)
const ICON_PX: i32 = 32;

// Shell_NotifyIcon constants
const NIM_ADD: u32 = 0;
const NIM_MODIFY: u32 = 1;
const NIM_DELETE: u32 = 2;
const NIM_SETVERSION: u32 = 4;
const NIF_MESSAGE: u32 = 0x1;
const NIF_ICON: u32 = 0x2;
const NIF_TIP: u32 = 0x4;
const NIF_STATE: u32 = 0x8;
const NIF_GUID: u32 = 0x20;
const NIS_HIDDEN: u32 = 0x1;
const NIN_SELECT: u32 = 0x400;

/// Layout of the data Explorer receives from `Shell_NotifyIcon` (handles are 32 bit).
#[repr(C)]
#[derive(Clone, Copy)]
struct ShellTrayMessage {
    magic: i32,
    message: u32,
    data: NotifyIconData,
    version: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NotifyIconData {
    size: u32,
    hwnd: u32,
    uid: u32,
    flags: u32,
    callback_message: u32,
    icon: u32,
    tip: [u16; 128],
    state: u32,
    state_mask: u32,
    info: [u16; 256],
    timeout_or_version: u32,
    info_title: [u16; 64],
    info_flags: u32,
    guid: GUID,
    balloon_icon: u32,
}

#[derive(Debug, Clone)]
struct TrayIcon {
    id: String,
    /// stable across restarts of the application (see `key_for`)
    key: String,
    hwnd: Option<isize>,
    pid: u32,
    uid: Option<u32>,
    callback: Option<u32>,
    version: Option<u32>,
    tooltip: String,
    exe: Option<PathBuf>,
    guid: Option<u128>,
    /// one of Explorer's own status icons (see `is_shell_status_icon`): never listed
    shell_status: bool,
    png: Option<Vec<u8>>,
    png_hash: u64,
    hidden: bool,
}

impl TrayIcon {
    /// Shown on the toolbar or in its overflow flyout.
    fn listed(&self) -> bool {
        !self.hidden && !self.shell_status && self.png.is_some()
    }
}

/// What the toolbar gets.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayIconInfo {
    pub id: String,
    /// identity kept in the settings (`TrayPrefs`)
    pub key: String,
    pub tooltip: String,
    /// changes whenever the image changes (cache busting for the icon URL)
    pub icon_version: String,
    /// shown on the toolbar (else in the overflow flyout)
    pub inline: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrayAction {
    Left,
    Double,
    Right,
    Middle,
}

static ICONS: LazyLock<Mutex<Vec<TrayIcon>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static RUNNING: AtomicBool = AtomicBool::new(false);
static RECEIVER: AtomicIsize = AtomicIsize::new(0);
static EMIT_PENDING: AtomicBool = AtomicBool::new(false);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
/// `Shell_TrayWnd` the hook is installed on
static HOOKED_TASKBAR: AtomicIsize = AtomicIsize::new(0);
/// hook + DLL handles, so they can be released
static HOOK: Mutex<Option<(isize, isize)>> = Mutex::new(None);

// ======================= public api =======================

/// Every visible icon: the toolbar ones first (in the user's order), then the overflow
/// ones, each group otherwise in registration order.
pub fn icons() -> Vec<TrayIconInfo> {
    let prefs = settings::get().toolbar.tray;
    let mut list: Vec<TrayIconInfo> = ICONS
        .lock()
        .iter()
        .filter(|i| i.listed())
        .map(|i| TrayIconInfo {
            id: i.id.clone(),
            key: i.key.clone(),
            tooltip: i.tooltip.clone(),
            icon_version: format!("{:x}", i.png_hash),
            inline: if prefs.inline.contains(&i.key) {
                true
            } else if prefs.overflow.contains(&i.key) {
                false
            } else {
                prefs.new_icons_inline
            },
        })
        .collect();
    // stable sort: registration order within equal ranks
    list.sort_by_key(|i| {
        let pinned = prefs.inline.iter().position(|k| k == &i.key);
        (!i.inline, pinned.unwrap_or(usize::MAX))
    });
    list
}

/// Placement preferences changed: send the new order.
pub fn refresh() {
    schedule_emit();
}

/// The toolbar / overflow split the Windows taskbar uses for the current icons
/// ("Other system tray icons" in the taskbar settings): (inline, overflow) keys.
pub fn windows_placement() -> (Vec<String>, Vec<String>) {
    let promoted = windows_promoted();
    let mut inline = Vec::new();
    let mut overflow = Vec::new();
    for icon in ICONS.lock().iter().filter(|i| i.listed()) {
        let target = if promoted.iter().any(|p| p.matches(icon)) {
            &mut inline
        } else {
            &mut overflow
        };
        target.push(icon.key.clone());
    }
    (inline, overflow)
}

/// PNG of an icon (served through the `sdicon` protocol: `?tray=<id>`).
pub fn icon_png(id: &str) -> Option<Vec<u8>> {
    ICONS
        .lock()
        .iter()
        .find(|i| i.id == id)
        .and_then(|i| i.png.clone())
}

/// Starts watching the tray (idempotent).
pub fn start() {
    if RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    crate::utils::spawn_named("system-tray", run_receiver);
    crate::utils::spawn_supervised("system-tray-prune", || {
        while RUNNING.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_secs(3));
            if prune() {
                schedule_emit();
            }
        }
    });
}

/// Stops watching: removes the hook and closes the receiver.
pub fn stop() {
    if !RUNNING.swap(false, Ordering::AcqRel) {
        return;
    }
    unhook();
    let receiver = RECEIVER.swap(0, Ordering::AcqRel);
    if receiver != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(receiver as *mut _)),
                WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
    ICONS.lock().clear();
    schedule_emit();
}

/// Forwards a click to the application that owns the icon.
pub fn action(id: &str, action: TrayAction) -> Result<()> {
    let icon = ICONS
        .lock()
        .iter()
        .find(|i| i.id == id)
        .cloned()
        .ok_or("unknown tray icon")?;
    let (Some(hwnd), Some(uid), Some(callback)) = (icon.hwnd, icon.uid, icon.callback) else {
        return Err("this tray icon does not accept clicks".into());
    };
    if !owner_alive(&icon) {
        if prune() {
            schedule_emit();
        }
        return Ok(());
    }

    // the application usually opens a menu or a window: let it take the foreground
    unsafe {
        let _ = AllowSetForegroundWindow(icon.pid);
    }
    let messages: &[u32] = match action {
        TrayAction::Left => &[WM_LBUTTONDOWN, WM_LBUTTONUP],
        TrayAction::Double => &[WM_LBUTTONDBLCLK, WM_LBUTTONUP],
        TrayAction::Right => &[WM_RBUTTONDOWN, WM_RBUTTONUP],
        TrayAction::Middle => &[WM_MBUTTONDOWN, WM_MBUTTONUP],
    };
    for &message in messages {
        notify(hwnd, callback, uid, icon.version, message);
    }
    // version 3+ applications also expect these (Explorer sends them as well)
    if icon.version.is_some_and(|v| v >= 3) {
        match action {
            TrayAction::Left => notify(hwnd, callback, uid, icon.version, NIN_SELECT),
            TrayAction::Right => notify(hwnd, callback, uid, icon.version, WM_CONTEXTMENU),
            _ => {}
        }
    }
    Ok(())
}

// ======================= forwarding a click =======================

fn pack(low: i32, high: i32) -> u32 {
    (low as u16 as u32) | ((high as u16 as u32) << 16)
}

/// `NOTIFYICON_VERSION_4` gets the cursor position in wParam and the uid in the high word
/// of lParam; older versions get the uid in wParam.
fn notify(hwnd: isize, callback: u32, uid: u32, version: Option<u32>, message: u32) {
    let v4 = version.is_some_and(|v| v > 3);
    let (wparam, lparam) = if v4 {
        let mut p = POINT::default();
        let _ = unsafe { GetCursorPos(&mut p) };
        (pack(p.x, p.y), pack(message as i32, uid as i32))
    } else {
        (uid, pack(message as i32, 0))
    };
    unsafe {
        let _ = SendNotifyMessageW(
            HWND(hwnd as *mut _),
            callback,
            WPARAM(wparam as usize),
            LPARAM(lparam as i32 as isize),
        );
    }
}

// ======================= icon bookkeeping =======================

fn window_pid(hwnd: isize) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(HWND(hwnd as *mut _), Some(&mut pid)) };
    pid
}

fn owner_alive(icon: &TrayIcon) -> bool {
    match icon.hwnd {
        Some(hwnd) => {
            unsafe { IsWindow(Some(HWND(hwnd as *mut _))) }.as_bool()
                && (icon.pid == 0 || window_pid(hwnd) == icon.pid)
        }
        None => true,
    }
}

/// Removes icons whose application exited without removing them.
fn prune() -> bool {
    let mut icons = ICONS.lock();
    let before = icons.len();
    icons.retain(owner_alive);
    icons.len() != before
}

fn wide(text: &[u16]) -> String {
    let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    String::from_utf16_lossy(&text[..len]).replace('\r', "")
}

/// Explorer's status icons all use `7820AExx-23E3-4229-82C1-E41CB67D5B9C` (volume 73,
/// network 74, power 75, action center 76, safely remove hardware 78, meet now 83...).
const SHELL_ICON_GUIDS: u128 = 0x7820AE00_23E3_4229_82C1_E41CB67D5B9C;
const SHELL_ICON_GUID_MASK: u128 = !(0xFF << 96);
const SAFELY_REMOVE_HARDWARE: u128 = 0x7820AE78_23E3_4229_82C1_E41CB67D5B9C;

/// Volume, network, power and the other status icons Explorer registers for itself.
///
/// On Windows 11 the taskbar draws them from its own state: the registered icons are
/// white on transparent (invisible on a light toolbar) and clicks sent to them do
/// nothing. The toolbar has its own items for these (quick settings, battery,
/// notifications), so they are left out. "Safely remove hardware" still works and has
/// no counterpart: it stays.
fn is_shell_status_icon(guid: Option<u128>, exe: Option<&Path>) -> bool {
    if guid == Some(SAFELY_REMOVE_HARDWARE) {
        return false;
    }
    guid.is_some_and(|g| g & SHELL_ICON_GUID_MASK == SHELL_ICON_GUIDS)
        || exe
            .and_then(Path::file_name)
            .is_some_and(|n| n.eq_ignore_ascii_case("explorer.exe"))
}

/// Identity of an icon that survives restarts: its GUID, or its executable and id.
fn key_for(guid: Option<GUID>, exe: Option<&Path>, uid: u32) -> Option<String> {
    if let Some(guid) = guid {
        return Some(format!("guid:{:032x}", guid.to_u128()));
    }
    let exe = exe?.to_string_lossy().to_lowercase();
    Some(format!("exe:{exe}#{uid}"))
}

// ======================= Windows' own placement =======================

/// A promoted record of `HKCU\Control Panel\NotifyIconSettings` (written by Explorer).
struct PromotedRecord {
    exe: Option<String>,
    uid: Option<u32>,
    guid: Option<u128>,
}

impl PromotedRecord {
    fn matches(&self, icon: &TrayIcon) -> bool {
        if let (Some(a), Some(b)) = (self.guid, icon.guid) {
            return a == b;
        }
        let Some(exe) = &icon.exe else {
            return false;
        };
        self.exe
            .as_deref()
            .is_some_and(|e| e == exe.to_string_lossy().to_lowercase())
            && self.uid.is_none_or(|uid| Some(uid) == icon.uid)
    }
}

/// Icons the Windows taskbar shows outside its overflow flyout.
fn windows_promoted() -> Vec<PromotedRecord> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let Ok(root) =
        RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Control Panel\NotifyIconSettings")
    else {
        return Vec::new();
    };
    root.enum_keys()
        .flatten()
        .filter_map(|name| root.open_subkey(name).ok())
        .filter(|k| k.get_value::<u32, _>("IsPromoted").unwrap_or(0) != 0)
        .map(|k| PromotedRecord {
            exe: k
                .get_value::<String, _>("ExecutablePath")
                .ok()
                .map(|p| resolve_known_folder(&p).to_lowercase()),
            uid: k.get_value::<u32, _>("UID").ok(),
            guid: k
                .get_value::<String, _>("IconGuid")
                .ok()
                .and_then(|g| GUID::try_from(g.trim_matches(['{', '}'])).ok())
                .map(|g| g.to_u128()),
        })
        .collect()
}

/// `{6D809377-...}\App\app.exe` (a known folder id) -> `C:\Program Files\App\app.exe`
fn resolve_known_folder(path: &str) -> String {
    let resolve = || {
        let rest = path.strip_prefix('{')?;
        let (id, tail) = rest.split_once('}')?;
        let id = GUID::try_from(id).ok()?;
        let folder = unsafe { SHGetKnownFolderPath(&id, KNOWN_FOLDER_FLAG(0), None) }.ok()?;
        let base = unsafe { folder.to_string() }.ok();
        unsafe { CoTaskMemFree(Some(folder.0 as *const _)) };
        Some(format!("{}{tail}", base?))
    };
    resolve().unwrap_or_else(|| path.to_string())
}

fn hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// `icon`: SideDock's own copy of the message's icon.
fn process(message: &ShellTrayMessage, icon: Option<HICON>) -> bool {
    let data = &message.data;
    let has_guid = data.flags & NIF_GUID != 0 && data.guid != GUID::zeroed();
    let hwnd = (data.hwnd != 0).then_some(data.hwnd as isize);
    let id = if has_guid {
        format!("g-{:032x}", data.guid.to_u128())
    } else if let Some(hwnd) = hwnd {
        format!("h-{hwnd:x}-{}", data.uid)
    } else {
        return false;
    };

    let mut icons = ICONS.lock();
    match message.message {
        NIM_DELETE => {
            let before = icons.len();
            icons.retain(|i| i.id != id);
            icons.len() != before
        }
        NIM_ADD | NIM_MODIFY | NIM_SETVERSION => {
            let index = match icons.iter().position(|i| i.id == id) {
                Some(index) => index,
                None => {
                    if message.message == NIM_SETVERSION && hwnd.is_none() {
                        return false;
                    }
                    let exe = hwnd.and_then(|h| Window(h).exe_path());
                    let guid = has_guid.then(|| data.guid.to_u128());
                    let shell_status = is_shell_status_icon(guid, exe.as_deref());
                    log::info!(
                        "system tray: new icon {id} ({}){}",
                        exe.as_deref()
                            .and_then(Path::file_name)
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        if shell_status {
                            ", Explorer status icon: not listed"
                        } else {
                            ""
                        }
                    );
                    icons.push(TrayIcon {
                        key: key_for(has_guid.then_some(data.guid), exe.as_deref(), data.uid)
                            .unwrap_or_else(|| id.clone()),
                        exe,
                        guid,
                        shell_status,
                        id,
                        hwnd,
                        pid: hwnd.map(window_pid).unwrap_or(0),
                        uid: hwnd.map(|_| data.uid),
                        callback: None,
                        version: None,
                        tooltip: String::new(),
                        png: None,
                        png_hash: 0,
                        hidden: false,
                    });
                    icons.len() - 1
                }
            };
            let copy = icon;
            let icon = &mut icons[index];
            let before = (
                icon.tooltip.clone(),
                icon.png_hash,
                icon.hidden,
                icon.callback,
            );
            if let Some(hwnd) = hwnd {
                icon.hwnd = Some(hwnd);
                icon.uid = Some(data.uid);
            }
            if data.flags & NIF_MESSAGE != 0 {
                icon.callback = Some(data.callback_message);
            }
            if data.flags & NIF_TIP != 0 {
                icon.tooltip = wide(&data.tip);
            }
            if data.flags & NIF_STATE != 0 && data.state_mask & NIS_HIDDEN != 0 {
                icon.hidden = data.state & NIS_HIDDEN != 0;
            }
            if message.message == NIM_SETVERSION && (1..=4).contains(&data.timeout_or_version) {
                icon.version = Some(data.timeout_or_version);
            }
            // always redrawn: applications often change the image behind the same handle
            if let Some(handle) = copy {
                match icons::hicon_to_png(handle, ICON_PX) {
                    Ok(png) => {
                        icon.png_hash = hash(&png);
                        icon.png = Some(png);
                    }
                    Err(err) => log::debug!("tray icon {}: {err}", icon.id),
                }
            }
            if icon.tooltip.is_empty()
                && let Some(exe) = &icon.exe
            {
                icon.tooltip = shell::display_name_for(exe, None);
            }
            before
                != (
                    icon.tooltip.clone(),
                    icon.png_hash,
                    icon.hidden,
                    icon.callback,
                )
        }
        _ => false,
    }
}

/// Coalesces bursts (e.g. every application re-registering at once).
fn schedule_emit() {
    if EMIT_PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(60));
        EMIT_PENDING.store(false, Ordering::Release);
        app::emit(EVENT_CHANGED, &icons());
    });
}

// ======================= hook =======================

fn dll_path() -> PathBuf {
    crate::paths::get()
        .exe
        .parent()
        .map(|dir| dir.join(DLL_NAME))
        .unwrap_or_else(|| PathBuf::from(DLL_NAME))
}

fn unhook() {
    if let Some((hook, module)) = HOOK.lock().take() {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(hook as *mut _));
            let _ = FreeLibrary(HMODULE(module as *mut _));
        }
    }
}

/// Installs the hook on the taskbar's thread, then asks every application to register
/// its icons again.
fn hook() -> Result<()> {
    unhook();
    let taskbar = unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) }
        .map_err(|_| "the Windows taskbar was not found")?;
    let thread = unsafe { GetWindowThreadProcessId(taskbar, None) };
    let path = dll_path();
    if !path.exists() {
        return Err(format!("{} is missing", path.display()).into());
    }
    unsafe {
        let module = LoadLibraryW(&HSTRING::from(path.as_os_str()))?;
        let Some(proc) = GetProcAddress(module, PCSTR(c"CallWndProc".as_ptr().cast())) else {
            let _ = FreeLibrary(module);
            return Err("the tray hook DLL is invalid".into());
        };
        let proc: HOOKPROC = Some(std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT,
        >(proc));
        let hook = match SetWindowsHookExW(WH_CALLWNDPROC, proc, Some(module.into()), thread) {
            Ok(hook) => hook,
            Err(err) => {
                let _ = FreeLibrary(module);
                return Err(err.into());
            }
        };
        *HOOK.lock() = Some((hook.0 as isize, module.0 as isize));
    }
    HOOKED_TASKBAR.store(taskbar.0 as isize, Ordering::Release);
    log::info!("system tray: hooked the taskbar");
    // applications re-register their icons, which the hook now sees
    unsafe {
        let _ = SendNotifyMessageW(
            HWND_BROADCAST,
            TASKBAR_CREATED.load(Ordering::Acquire),
            WPARAM(0),
            LPARAM(0),
        );
    }
    Ok(())
}

// ======================= receiver window =======================

unsafe extern "system" fn receiver_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COPYDATA => {
            // wp = the taskbar window, lp = the forwarded COPYDATASTRUCT
            let data = unsafe { (lp.0 as *const COPYDATASTRUCT).as_ref() };
            if let Some(data) = data
                && data.dwData == 1
                && !data.lpData.is_null()
                && data.cbData as usize >= std::mem::size_of::<ShellTrayMessage>()
            {
                let message =
                    unsafe { std::ptr::read_unaligned(data.lpData as *const ShellTrayMessage) };
                // The application may destroy its icon as soon as Shell_NotifyIcon returns:
                // copy it (fast), then let Explorer go on before the slow work (drawing,
                // PNG encoding, app name lookup).
                let icon = (message.data.flags & NIF_ICON != 0 && message.data.icon != 0)
                    .then(|| unsafe { CopyIcon(HICON(message.data.icon as isize as *mut _)) }.ok())
                    .flatten();
                unsafe {
                    let _ = ReplyMessage(LRESULT(1));
                }
                let changed = process(&message, icon);
                if let Some(icon) = icon {
                    unsafe {
                        let _ = DestroyIcon(icon);
                    }
                }
                if changed {
                    schedule_emit();
                }
            }
            LRESULT(1)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ if msg != 0 && msg == TASKBAR_CREATED.load(Ordering::Acquire) => {
            // our own broadcast (see `hook`) comes back here too: only a new taskbar
            // window (Explorer restarted) needs a new hook
            let current = unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) }
                .map(|h| h.0 as isize)
                .unwrap_or(0);
            if current != 0 && current == HOOKED_TASKBAR.load(Ordering::Acquire) {
                return LRESULT(0);
            }
            log::info!("system tray: the taskbar was recreated");
            ICONS.lock().clear();
            schedule_emit();
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(500));
                if RUNNING.load(Ordering::Acquire)
                    && let Err(err) = hook()
                {
                    log::warn!("system tray: {err}");
                }
            });
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(h, msg, wp, lp) },
    }
}

fn run_receiver() {
    unsafe {
        TASKBAR_CREATED.store(
            RegisterWindowMessageW(w!("TaskbarCreated")),
            Ordering::Release,
        );
        let class = HSTRING::from(RECEIVER_CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(receiver_proc),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
        // hidden top-level window: message-only windows do not get broadcasts
        let created = CreateWindowExW(
            WINDOW_EX_STYLE(WS_EX_TOOLWINDOW.0),
            PCWSTR(class.as_ptr()),
            w!("SideDock tray receiver"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
        );
        let Ok(window) = created else {
            log::warn!("system tray: receiver not created: {created:?}");
            RUNNING.store(false, Ordering::Release);
            return;
        };
        RECEIVER.store(window.0 as isize, Ordering::Release);
        if let Err(err) = hook() {
            log::warn!("system tray: {err}");
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = DestroyWindow(window);
    }
}
