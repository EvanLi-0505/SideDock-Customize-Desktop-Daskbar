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
    modules::{
        apps, hotkeys, icons,
        shell_flyouts::{self, Flyout},
        start_apps, system, system_tray, win_key,
    },
    paths, session,
    state::{
        dock_items::{self, DockItem, DockItems},
        launcher_layout::{self, LauncherLayout},
        settings::{self, AppSettings, DockSettings, StartMenuMode},
    },
    utils::throttle,
    widgets::{
        self,
        dock::{self, DockInfo, native_taskbar},
        overlay,
        popup::{self, ActionPayload, AnchorRect, Placement, PopupRequest},
        tooltip,
    },
    windows_api::{ComGuard, audio, keyboard, media, power, radios, shell, window::Window},
};

/// The launcher layout was saved (payload: `LauncherLayout`).
const EVENT_LAUNCHER_LAYOUT: &str = "launcher-layout-changed";

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
        dock_pin_start_app,
        // toolbar & system tray
        toolbar_ready,
        toolbar_get_info,
        get_tray_icons,
        tray_icon_action,
        tray_place,
        toolbar_run_custom,
        tray_use_windows_placement,
        tray_drag,
        // Win key takeover
        win_key_state,
        win_key_set_enabled,
        // overlay
        get_start_apps,
        get_launcher_layout,
        launcher_open_place,
        get_user_name,
        volume_step,
        volume_toggle_mute,
        save_launcher_layout,
        overlay_ready,
        overlay_hide,
        overlay_toggle_launcher,
        overlay_launch,
        overlay_set_thumbnails,
        overlay_switcher_select,
        overlay_activate_window,
        // dock widget
        dock_ready,
        dock_get_info,
        dock_set_hitbox,
        dock_set_backdrop,
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
        tooltip_follow,
        // app
        open_settings,
        settings_ready,
        get_app_info,
        get_system_colors,
        get_shortcut_status,
        shortcuts_suspend,
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

/// Pins an app of the launcher (Start menu app) to the dock.
#[tauri::command]
async fn dock_pin_start_app(id: String) -> Result<usize> {
    let entry = start_apps::list()
        .into_iter()
        .find(|a| a.id == id)
        .ok_or("unknown app")?;
    // classic programs registered under an AppUserModelID keep it for window grouping
    let umid = if entry.packaged || !(id.starts_with('{') || id.contains('\\')) {
        Some(id)
    } else {
        entry.path.as_deref().and_then(shell::file_umid)
    };
    let item = dock_items::AppItem {
        id: uuid::Uuid::new_v4().to_string(),
        display_name: entry.name,
        path: entry.path.unwrap_or_default(),
        umid,
        pinned: true,
        prevent_pinning: false,
        relaunch: None,
    };
    pin_items(vec![item])
}

#[tauri::command]
async fn dock_import_taskbar_pins() -> Result<usize> {
    let items = native_taskbar::pinned_apps()?;
    let count = pin_items(items)?;
    log::info!("imported {count} pinned items from the Windows taskbar");
    Ok(count)
}

// ======================= toolbar & system tray =======================

#[tauri::command]
async fn toolbar_ready(window: WebviewWindow) {
    widgets::toolbar::set_ready(window.label());
}

#[tauri::command]
async fn toolbar_get_info(window: WebviewWindow) -> Option<widgets::toolbar::ToolbarInfo> {
    widgets::toolbar::info(window.label())
}

#[tauri::command]
async fn get_tray_icons() -> Vec<system_tray::TrayIconInfo> {
    system_tray::icons()
}

#[tauri::command]
async fn tray_icon_action(id: String, action: system_tray::TrayAction) -> Result<()> {
    system_tray::action(&id, action)
}

/// Runs a custom toolbar button. Only the id comes from the page: what it opens is read
/// from the settings the user saved.
#[tauri::command]
async fn toolbar_run_custom(id: String) -> Result<()> {
    if !throttle(&format!("custom:{id}"), Duration::from_millis(800)) {
        return Ok(());
    }
    let button = settings::get()
        .toolbar
        .custom_buttons
        .into_iter()
        .find(|b| b.id == id)
        .ok_or("unknown custom button")?;
    let target = button.target.trim();
    if button.is_url() {
        return shell::open_uri(target);
    }
    let path = PathBuf::from(target);
    // programs start in their own folder, like from a shortcut
    let dir = path
        .is_file()
        .then(|| path.parent().map(PathBuf::from))
        .flatten();
    let args = Some(button.args.trim()).filter(|a| !a.is_empty());
    shell::launch(target, args, dir.as_deref(), false)
}

/// Moves a tray icon to the toolbar (at `index` among its icons) or to the overflow
/// flyout. The current order becomes explicit, so it no longer depends on the default.
#[tauri::command]
async fn tray_place(
    app: AppHandle,
    key: String,
    inline: bool,
    index: Option<usize>,
) -> Result<AppSettings> {
    let icons = system_tray::icons();
    let mut new = settings::get();
    let tray = &mut new.toolbar.tray;
    let mut order: Vec<String> = icons
        .iter()
        .filter(|i| i.inline && i.key != key)
        .map(|i| i.key.clone())
        .collect();
    // icons placed earlier whose application is not running keep their place
    for k in &tray.inline {
        if k != &key && !order.contains(k) {
            order.push(k.clone());
        }
    }
    tray.overflow.retain(|k| k != &key);
    if inline {
        let at = index.unwrap_or(order.len()).min(order.len());
        order.insert(at, key);
    } else {
        tray.overflow.insert(0, key);
    }
    tray.inline = order;
    app::apply_settings(&app, new)
}

/// Copies the Windows taskbar's choice of icons to show (the rest go to the overflow).
#[tauri::command]
async fn tray_use_windows_placement(app: AppHandle) -> Result<AppSettings> {
    let (inline, overflow) = system_tray::windows_placement();
    let mut new = settings::get();
    new.toolbar.tray.inline = inline;
    new.toolbar.tray.overflow = overflow;
    app::apply_settings(&app, new)
}

/// A tray icon is being dragged (phase "move" / "end" / "cancel"): tells the toolbars
/// and the overflow flyout where the cursor is, in their own coordinates. The window
/// under the cursor at "end" places the icon (`tray_place`).
#[tauri::command]
async fn tray_drag(app: AppHandle, phase: String, key: String) -> Result<()> {
    widgets::toolbar::tray_drag(&app, &phase, &key)
}

// ======================= Win key takeover =======================

#[tauri::command]
async fn win_key_state() -> win_key::WinKeyState {
    win_key::state()
}

/// Turns the takeover on (one UAC prompt to register the scheduled task) or off (stops
/// the helper and removes the task).
#[tauri::command]
async fn win_key_set_enabled(app: AppHandle, enabled: bool) -> Result<win_key::WinKeyState> {
    if enabled {
        win_key::enable(&app)
    } else {
        win_key::disable(&app)
    }
}

// ======================= overlay (launcher / switcher) =======================

#[tauri::command]
async fn get_start_apps() -> Vec<start_apps::StartApp> {
    start_apps::refresh();
    start_apps::list()
}

/// Opens one of the launcher's bottom bar shortcuts (`settings::LAUNCHER_PLACES`).
#[tauri::command]
async fn launcher_open_place(app: AppHandle, id: String) -> Result<()> {
    if !throttle(&format!("place:{id}"), Duration::from_millis(800)) {
        return Ok(());
    }
    // shell: names of the known folders, so redirected folders (another drive, OneDrive)
    // open where they really are
    let target = match id.as_str() {
        "settings" => "ms-settings:",
        "explorer" => "explorer.exe",
        "documents" => "shell:Personal",
        "downloads" => "shell:Downloads",
        "music" => "shell:My Music",
        "pictures" => "shell:My Pictures",
        "videos" => "shell:My Video",
        "network" => "shell:NetworkPlacesFolder",
        "user-folder" => "shell:UsersFilesFolder",
        "account" => "ms-settings:yourinfo",
        other => return Err(format!("unknown place {other}").into()),
    };
    overlay::hide(&app);
    if target.starts_with("ms-settings:") {
        shell::open_uri(target)
    } else {
        shell::launch(target, None, None, false)
    }
}

/// Display name of the signed-in user ("Zhe Li"), or the account name.
#[tauri::command]
async fn get_user_name() -> String {
    shell::user_display_name()
}

#[tauri::command]
async fn get_launcher_layout() -> LauncherLayout {
    launcher_layout::get()
}

#[tauri::command]
async fn save_launcher_layout(layout: LauncherLayout) -> Result<LauncherLayout> {
    let (layout, changed) = launcher_layout::replace(layout)?;
    if changed {
        app::emit(EVENT_LAUNCHER_LAYOUT, &layout);
    }
    Ok(layout)
}

#[tauri::command]
async fn overlay_ready(app: AppHandle, token: u64) -> Result<()> {
    overlay::ready(&app, token)
}

#[tauri::command]
async fn overlay_hide(app: AppHandle) {
    overlay::hide(&app);
}

#[tauri::command]
async fn overlay_toggle_launcher(app: AppHandle) {
    overlay::toggle_launcher(&app);
}

#[tauri::command]
async fn overlay_launch(app: AppHandle, id: String, elevated: bool) -> Result<()> {
    if !throttle(&format!("launch:{id}"), Duration::from_millis(800)) {
        return Ok(());
    }
    log::info!("launcher: start {id} (elevated: {elevated})");
    overlay::launch(&app, &id, elevated)
}

#[tauri::command]
async fn overlay_set_thumbnails(
    app: AppHandle,
    thumbnails: Vec<overlay::ThumbnailRequest>,
) -> Result<()> {
    overlay::set_thumbnails(&app, thumbnails)
}

#[tauri::command]
async fn overlay_switcher_select(hwnd: isize) {
    overlay::switcher_select(hwnd);
}

#[tauri::command]
async fn overlay_activate_window(app: AppHandle, hwnd: isize) -> Result<()> {
    overlay::activate_window(&app, hwnd)
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
async fn dock_set_hitbox(window: WebviewWindow, hitbox: dock::Hitbox) {
    dock::set_hitbox(window.label(), hitbox);
}

#[tauri::command]
async fn dock_set_backdrop(window: WebviewWindow, shape: Option<dock::backdrop::BackdropShape>) {
    dock::set_backdrop(window.label(), shape);
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
async fn shell_action(app: AppHandle, action: String) -> Result<()> {
    if !throttle(&format!("shell:{action}"), Duration::from_millis(300)) {
        return Ok(());
    }
    match action.as_str() {
        "start-menu" => {
            if settings::get().launcher.start_menu == StartMenuMode::SideDock {
                overlay::toggle_launcher(&app);
            } else {
                shell::toggle_start_menu();
            }
        }
        "show-desktop" => shell::toggle_desktop(),
        "task-manager" => shell::launch("taskmgr.exe", None, None, false)?,
        "recycle-bin" => shell::launch("shell:RecycleBinFolder", None, None, false)?,
        "action-center" => shell_flyouts::toggle(Flyout::NotificationCenter),
        "network" => shell::open_uri("ms-availablenetworks:")?,
        "quick-settings" => shell_flyouts::toggle(Flyout::QuickSettings),
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

/// Turns the master volume up or down by `delta` percent (wheel over the volume icon).
#[tauri::command]
async fn volume_step(delta: i32) -> Result<()> {
    let _com = ComGuard::new();
    system::set_volume(audio::step(delta.clamp(-20, 20))?);
    Ok(())
}

#[tauri::command]
async fn volume_toggle_mute() -> Result<()> {
    let _com = ComGuard::new();
    system::set_volume(audio::toggle_mute()?);
    Ok(())
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
async fn tooltip_follow(
    app: AppHandle,
    window: WebviewWindow,
    anchor: AnchorRect,
    placement: Placement,
) -> Result<()> {
    tooltip::follow(&app, &window, anchor, placement)
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
async fn get_shortcut_status() -> std::collections::BTreeMap<String, hotkeys::HotkeyStatus> {
    hotkeys::status()
}

#[tauri::command]
async fn shortcuts_suspend(suspended: bool) {
    hotkeys::suspend(suspended);
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
