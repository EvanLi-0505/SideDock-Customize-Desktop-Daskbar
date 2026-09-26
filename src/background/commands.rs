//! Every IPC command exposed to the webviews, in one place.
//!
//! All commands are `async` so they run on the async runtime instead of the
//! event-loop thread: a slow shell call can never freeze the UI, and creating
//! windows from a command cannot dead-lock.

use std::{path::PathBuf, time::Duration};

use serde::Serialize;
use tauri::{AppHandle, WebviewWindow};

use crate::{
    app,
    error::{AppError, Result},
    logger,
    modules::{apps, icons, system},
    paths, session,
    state::{
        dock_items::{self, DockItem, DockItems},
        settings::{self, AppSettings, DockSettings},
    },
    utils::throttle,
    widgets::{
        self,
        dock::{self, DockInfo, native_taskbar},
        popup::{self, ActionPayload, AnchorRect, Placement, PopupRequest},
        tooltip,
    },
    windows_api::{keyboard, media, power, radios, shell, window::Window},
};

pub fn handler() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        // settings & state
        get_settings,
        save_settings,
        reset_settings,
        get_dock_items,
        save_dock_items,
        reset_dock_items,
        dock_pin_paths,
        dock_import_taskbar_pins,
        // dock widget
        dock_ready,
        dock_get_info,
        dock_set_content_length,
        dock_set_dragging,
        // windows
        get_windows,
        get_focused,
        window_toggle,
        window_focus,
        window_close,
        window_kill,
        // shell
        launch,
        reveal_path,
        shell_action,
        // system modules
        get_system_state,
        set_keyboard_layout,
        set_bluetooth,
        media_command,
        power_action,
        empty_recycle_bin,
        // popup & tooltip
        popup_open,
        popup_ready,
        popup_resize,
        popup_action,
        popup_close,
        tooltip_show,
        tooltip_ready,
        tooltip_hide,
        // app
        open_settings,
        settings_ready,
        get_app_info,
        get_system_colors,
        get_storage_usage,
        open_data_dir,
        clear_data,
        restart_app,
        quit_app,
    ]
}

// ======================= settings & state =======================

#[tauri::command]
async fn get_settings() -> AppSettings {
    settings::get()
}

#[tauri::command]
async fn save_settings(app: AppHandle, settings: AppSettings) -> Result<AppSettings> {
    app::apply_settings(&app, settings)
}

#[tauri::command]
async fn reset_settings(app: AppHandle, section: String) -> Result<AppSettings> {
    let mut current = settings::get();
    match section.as_str() {
        "dock" => current.dock = DockSettings::default(),
        "all" => {
            let autostart = current.autostart;
            current = AppSettings::default();
            current.autostart = autostart;
        }
        other => return Err(format!("unknown settings section {other}").into()),
    }
    app::apply_settings(&app, current)
}

#[tauri::command]
async fn get_dock_items() -> DockItems {
    dock_items::get()
}

/// `source` identifies the sender so it can ignore its own echo.
#[tauri::command]
async fn save_dock_items(items: DockItems, source: String) -> Result<DockItems> {
    let saved = dock_items::replace(items)?;
    app::broadcast_dock_items(&source);
    Ok(saved)
}

#[tauri::command]
async fn reset_dock_items() -> Result<DockItems> {
    let items = dock_items::reset()?;
    app::broadcast_dock_items("backend");
    Ok(items)
}

fn pin_items(new_items: Vec<dock_items::AppItem>) -> Result<usize> {
    let mut items = dock_items::get();
    let existing: Vec<String> = items
        .all()
        .filter_map(|i| match i {
            DockItem::App(a) => Some(a.path.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect();
    let mut added = 0;
    for item in new_items {
        if existing.contains(&item.path.to_string_lossy().to_lowercase()) {
            continue;
        }
        items.center.push(DockItem::App(item));
        added += 1;
    }
    if added > 0 {
        dock_items::replace(items)?;
        app::broadcast_dock_items("backend");
    }
    Ok(added)
}

#[tauri::command]
async fn dock_pin_paths(paths: Vec<PathBuf>) -> Result<usize> {
    let items = paths
        .iter()
        .filter_map(|p| match native_taskbar::item_from_path(p) {
            Ok(item) => Some(item),
            Err(err) => {
                log::warn!("cannot pin {}: {err}", p.display());
                None
            }
        })
        .collect();
    pin_items(items)
}

#[tauri::command]
async fn dock_import_taskbar_pins() -> Result<usize> {
    let items = native_taskbar::pinned_apps()?;
    let count = pin_items(items)?;
    log::info!("imported {count} pinned items from the Windows taskbar");
    Ok(count)
}

// ======================= dock widget =======================

#[tauri::command]
async fn dock_ready(window: WebviewWindow) {
    dock::set_ready(window.label());
}

#[tauri::command]
async fn dock_get_info(window: WebviewWindow) -> Option<DockInfo> {
    dock::info(window.label())
}

#[tauri::command]
async fn dock_set_content_length(window: WebviewWindow, length: f64) {
    dock::set_content_length(window.label(), length);
}

#[tauri::command]
async fn dock_set_dragging(window: WebviewWindow, dragging: bool) {
    dock::set_dragging(window.label(), dragging);
}

// ======================= windows =======================

#[tauri::command]
async fn get_windows() -> Vec<apps::UserAppWindow> {
    apps::windows()
}

#[tauri::command]
async fn get_focused() -> apps::FocusedApp {
    apps::focused()
}

/// Click on a dock item with one window: minimize it if it was the active window,
/// otherwise bring it to front (same behaviour as the Windows taskbar).
#[tauri::command]
async fn window_toggle(hwnd: isize) -> Result<()> {
    let window = Window(hwnd);
    if !window.is_window() {
        return Err("window no longer exists".into());
    }
    let was_active = apps::last_external_foreground() == Some(window) && !window.is_minimized();
    if was_active {
        window.minimize();
    } else {
        window.focus()?;
    }
    Ok(())
}

#[tauri::command]
async fn window_focus(hwnd: isize) -> Result<()> {
    Window(hwnd).focus()
}

#[tauri::command]
async fn window_close(hwnd: isize) -> Result<()> {
    Window(hwnd).close()
}

#[tauri::command]
async fn window_kill(hwnd: isize) -> Result<()> {
    log::info!("killing process of window {hwnd:#x}");
    Window(hwnd).kill_process()
}

// ======================= shell =======================

#[tauri::command]
async fn launch(
    program: String,
    args: Option<String>,
    working_dir: Option<PathBuf>,
    elevated: bool,
) -> Result<()> {
    // swallow double clicks: one launch per program per 800ms
    if !throttle(&format!("launch:{program}"), Duration::from_millis(800)) {
        return Ok(());
    }
    log::info!("launch {program} (elevated: {elevated})");
    shell::launch(&program, args.as_deref(), working_dir.as_deref(), elevated)
}

#[tauri::command]
async fn reveal_path(path: PathBuf) -> Result<()> {
    shell::reveal_in_explorer(&path)
}

#[tauri::command]
async fn shell_action(action: String) -> Result<()> {
    if !throttle(&format!("shell:{action}"), Duration::from_millis(300)) {
        return Ok(());
    }
    match action.as_str() {
        "start-menu" => shell::toggle_start_menu(),
        "show-desktop" => shell::toggle_desktop(),
        "task-manager" => shell::launch("taskmgr.exe", None, None, false)?,
        "recycle-bin" => shell::launch("shell:RecycleBinFolder", None, None, false)?,
        "action-center" => shell::open_uri("ms-actioncenter:")?,
        "network" => shell::open_uri("ms-availablenetworks:")?,
        "network-settings" => shell::open_uri("ms-settings:network")?,
        "bluetooth-settings" => shell::open_uri("ms-settings:bluetooth")?,
        "keyboard-settings" => shell::open_uri("ms-settings:regionlanguage")?,
        "power-settings" => shell::open_uri("ms-settings:powersleep")?,
        "date-settings" => shell::open_uri("ms-settings:dateandtime")?,
        "notification-settings" => shell::open_uri("ms-settings:notifications")?,
        other => return Err(format!("unknown shell action {other}").into()),
    }
    Ok(())
}

// ======================= system modules =======================

#[tauri::command]
async fn get_system_state() -> system::SystemState {
    system::get()
}

#[tauri::command]
async fn set_keyboard_layout(id: String) -> Result<()> {
    let target = apps::last_external_foreground().ok_or("no target window")?;
    keyboard::activate(target, &id)?;
    // the layout switch is asynchronous in the target thread
    std::thread::sleep(Duration::from_millis(120));
    system::refresh_now("keyboard");
    Ok(())
}

#[tauri::command]
async fn set_bluetooth(enabled: bool) -> Result<()> {
    radios::set_bluetooth(enabled)?;
    system::refresh_now("bluetooth");
    Ok(())
}

#[tauri::command]
async fn media_command(command: media::MediaCommand) -> Result<()> {
    media::send(command)
}

#[tauri::command]
async fn power_action(action: power::PowerAction) -> Result<()> {
    if !throttle("power-action", Duration::from_secs(3)) {
        return Ok(());
    }
    power::perform(action)
}

#[tauri::command]
async fn empty_recycle_bin() -> Result<()> {
    shell::empty_recycle_bin()?;
    system::refresh_now("recycle-bin");
    Ok(())
}

// ======================= popup & tooltip =======================

#[tauri::command]
async fn popup_open(app: AppHandle, window: WebviewWindow, request: PopupRequest) -> Result<()> {
    tooltip::hide(&app);
    popup::open(&app, &window, request)
}

#[tauri::command]
async fn popup_ready(app: AppHandle, token: u64, width: f64, height: f64) -> Result<()> {
    popup::ready(&app, token, width, height)
}

#[tauri::command]
async fn popup_resize(app: AppHandle, token: u64, width: f64, height: f64) -> Result<()> {
    popup::resize(&app, token, width, height)
}

#[tauri::command]
async fn popup_action(app: AppHandle, payload: ActionPayload, close: bool) {
    popup::action(&app, payload, close);
}

#[tauri::command]
async fn popup_close(app: AppHandle) {
    popup::close(&app);
}

#[tauri::command]
async fn tooltip_show(
    app: AppHandle,
    window: WebviewWindow,
    text: String,
    anchor: AnchorRect,
    placement: Placement,
) -> Result<()> {
    if popup::current_owner().is_some() {
        return Ok(()); // never cover an open popup
    }
    tooltip::show(&app, &window, text, anchor, placement)
}

#[tauri::command]
async fn tooltip_ready(app: AppHandle, token: u64, width: f64, height: f64) -> Result<()> {
    tooltip::ready(&app, token, width, height)
}

#[tauri::command]
async fn tooltip_hide(app: AppHandle) {
    tooltip::hide(&app);
}

// ======================= app =======================

#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<()> {
    widgets::settings::open(&app)
}

#[tauri::command]
async fn settings_ready(app: AppHandle) {
    widgets::settings::show_when_ready(&app);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    exe: PathBuf,
    data_dir: PathBuf,
    previous_session_crashed: bool,
    debug: bool,
}

#[tauri::command]
async fn get_app_info() -> AppInfo {
    let p = paths::get();
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        exe: p.exe.clone(),
        data_dir: p.root.clone(),
        previous_session_crashed: session::previous_session_crashed(),
        debug: cfg!(debug_assertions),
    }
}

#[tauri::command]
async fn get_system_colors() -> crate::windows_api::SystemColors {
    crate::windows_api::system_colors()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageUsage {
    logs: u64,
    userdata: u64,
    cache: u64,
    config: u64,
}

#[tauri::command]
async fn get_storage_usage() -> StorageUsage {
    let p = paths::get();
    StorageUsage {
        logs: paths::dir_size(&p.logs),
        userdata: paths::dir_size(&p.userdata),
        cache: paths::dir_size(&p.cache),
        config: paths::dir_size(&p.config),
    }
}

fn data_dir(kind: &str) -> Result<PathBuf> {
    let p = paths::get();
    Ok(match kind {
        "root" => p.root.clone(),
        "logs" => p.logs.clone(),
        "userdata" => p.userdata.clone(),
        "cache" => p.cache.clone(),
        "config" => p.config.clone(),
        other => return Err(AppError(format!("unknown data folder {other}"))),
    })
}

#[tauri::command]
async fn open_data_dir(kind: String) -> Result<()> {
    let dir = data_dir(&kind)?;
    std::fs::create_dir_all(&dir)?;
    shell::launch(&dir.to_string_lossy(), None, None, false)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClearResult {
    /// entries that could not be removed because they are in use
    skipped: usize,
    restart_required: bool,
}

#[tauri::command]
async fn clear_data(app: AppHandle, kind: String) -> Result<ClearResult> {
    log::info!("clear data requested: {kind}");
    let p = paths::get();
    match kind.as_str() {
        "logs" => Ok(ClearResult {
            skipped: logger::clear(),
            restart_required: false,
        }),
        "cache" => {
            icons::clear_memory();
            Ok(ClearResult {
                skipped: paths::clear_dir(&p.cache, &[]),
                restart_required: false,
            })
        }
        // the WebView2 profile is locked while webviews run: clear it on next start
        "userdata" => {
            std::fs::write(p.runtime.join(crate::CLEAR_USERDATA_MARKER), b"1")?;
            Ok(ClearResult {
                skipped: 0,
                restart_required: true,
            })
        }
        "config" => {
            let defaults = AppSettings {
                autostart: settings::get().autostart,
                ..AppSettings::default()
            };
            app::apply_settings(&app, defaults)?;
            dock_items::reset()?;
            app::broadcast_dock_items("backend");
            Ok(ClearResult {
                skipped: 0,
                restart_required: false,
            })
        }
        other => Err(format!("unknown data kind {other}").into()),
    }
}

#[tauri::command]
async fn restart_app(app: AppHandle) {
    app::restart(&app);
}

#[tauri::command]
async fn quit_app(app: AppHandle) {
    app::quit(&app);
}
