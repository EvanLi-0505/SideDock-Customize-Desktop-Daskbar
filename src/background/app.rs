use std::sync::OnceLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget, Manager, RunEvent};

use crate::{
    commands,
    error::{Result, ResultLogExt},
    modules::{
        apps, autostart, hotkeys, icons, start_apps, system, system_tray, taskbar_visibility,
        win_key,
    },
    paths, session,
    state::{
        dock_items,
        settings::{self, AppSettings},
    },
    tray, widgets,
};

pub const EVENT_SETTINGS_CHANGED: &str = "settings-changed";
pub const EVENT_DOCK_ITEMS_CHANGED: &str = "dock-items-changed";

static HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Broadcasts an event to every webview. No-op before the app is running.
pub fn emit<S: Serialize + Clone>(event: &str, payload: &S) {
    if let Some(app) = HANDLE.get() {
        app.emit(event, payload.clone()).log_error();
    }
}

pub fn emit_to<S: Serialize + Clone>(app: &AppHandle, label: &str, event: &str, payload: &S) {
    app.emit_to(EventTarget::webview_window(label), event, payload.clone())
        .log_error();
}

/// Persists new settings and applies every side effect of the change.
pub fn apply_settings(app: &AppHandle, mut new: AppSettings) -> Result<AppSettings> {
    if new.autostart != autostart::is_enabled() {
        autostart::set_enabled(new.autostart)?;
    }
    new.autostart = autostart::is_enabled();

    // the main dock moved onto an auxiliary dock's edge: they swap edges
    let current = settings::get();
    if new.dock.position != current.dock.position {
        for aux in &mut new.aux_docks {
            if aux.position == new.dock.position {
                aux.position = current.dock.position;
            }
        }
    }
    let old = settings::replace(new)?;
    let new = settings::get();
    emit(EVENT_SETTINGS_CHANGED, &new);

    if old.language != new.language {
        tray::refresh(app);
    }
    if old.theme != new.theme {
        let apply = |app: &AppHandle| {
            if let Some(w) = app.get_webview_window(widgets::settings::LABEL) {
                widgets::settings::apply_theme(&w);
            }
            if let Some(w) = app.get_webview_window(widgets::popup::LABEL) {
                widgets::popup::apply_theme(&w);
            }
        };
        if new.theme.has_backdrop() {
            // the backdrop must be there before the page turns transparent
            apply(app);
        } else {
            // and must stay until the page has painted its solid background, otherwise
            // the window is see-through for a frame (visible flash)
            let app = app.clone();
            crate::utils::spawn_named("theme-backdrop", move || {
                std::thread::sleep(std::time::Duration::from_millis(150));
                apply(&app);
            });
        }
    }
    if old.dock != new.dock || old.aux_docks != new.aux_docks {
        widgets::dock::request_reconcile();
    }
    if old.shortcuts != new.shortcuts {
        hotkeys::reload();
    }
    if old.launcher != new.launcher {
        win_key::sync(app);
    }
    if old.taskbar != new.taskbar {
        taskbar_visibility::set_hidden(new.taskbar.hide_native);
    }
    if old.toolbar != new.toolbar || old.taskbar != new.taskbar {
        sync_toolbar();
    }
    if old.toolbar.tray != new.toolbar.tray {
        system_tray::refresh();
    }
    Ok(new)
}

/// Starts or stops the system tray watcher and re-places the toolbars.
fn sync_toolbar() {
    let toolbar = settings::get().toolbar;
    if toolbar.shows_tray() {
        system_tray::start();
    } else {
        system_tray::stop();
    }
    widgets::toolbar::request_reconcile();
}

/// Runs the action bound to a global shortcut (called on the hotkey thread).
pub fn run_shortcut(app: &AppHandle, action: &str) {
    match action {
        "toggle-dock" => {
            let mut s = settings::get();
            s.dock.enabled = !s.dock.enabled;
            apply_settings(app, s).log_error();
        }
        "open-settings" => widgets::settings::open(app).log_error(),
        "open-launcher" => widgets::overlay::toggle_launcher(app),
        "window-switcher" => widgets::overlay::switcher_hotkey(app),
        other => log::warn!("no handler for shortcut action {other}"),
    }
}

/// Tells the docks (and the settings page) that the pinned items of a dock changed:
/// `store` is `None` for the main dock, an auxiliary dock's id otherwise.
pub fn broadcast_dock_items(source: &str, store: Option<&str>) {
    #[derive(Serialize, Clone)]
    struct Payload {
        source: String,
        store: Option<String>,
        items: dock_items::DockItems,
    }
    emit(
        EVENT_DOCK_ITEMS_CHANGED,
        &Payload {
            source: source.to_string(),
            store: store.map(str::to_string),
            items: dock_items::get_for(store),
        },
    );
}

pub fn reload_docks(app: &AppHandle) {
    log::info!("reloading docks");
    widgets::dock::shutdown(app);
    widgets::dock::request_reconcile();
}

pub fn quit(app: &AppHandle) {
    log::info!("quit requested");
    app.exit(0);
}

/// Starts a fresh process that waits for this one to exit, then exits.
pub fn restart(app: &AppHandle) {
    log::info!("restart requested");
    let spawned = std::process::Command::new(&paths::get().exe)
        .args([crate::WAIT_FOR_ARG, &std::process::id().to_string()])
        .spawn();
    match spawned {
        Ok(_) => app.exit(0),
        Err(err) => log::error!("restart failed: {err}"),
    }
}

pub fn run() {
    let first_run = !paths::get().config.join("settings.json").exists();

    let builder = tauri::Builder::default()
        // must be the first plugin: a second launch only focuses the running instance
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            log::info!("second instance launched with {argv:?}, focusing settings");
            widgets::settings::open(app).log_error();
        }))
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("sdicon", |_ctx, request, responder| {
            let respond = |responder: tauri::UriSchemeResponder, bytes: Vec<u8>| {
                let response = tauri::http::Response::builder()
                    .header("Content-Type", "image/png")
                    .header("Cache-Control", "max-age=604800")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(bytes)
                    .unwrap_or_default();
                responder.respond(response);
            };
            let uri = request.uri().to_string();
            // answer known icons right away, on this thread: no cross-thread round-trip
            // with the webview (a burst of those can stall a foreground webview)
            if let Some(bytes) = icons::handle_request_cached(&uri) {
                respond(responder, bytes);
                return;
            }
            crate::utils::run_in_pool(move || respond(responder, icons::handle_request(&uri)));
        })
        .invoke_handler(commands::handler())
        .setup(move |app| {
            let handle = app.handle().clone();
            let _ = HANDLE.set(handle.clone());

            session::begin(!cfg!(debug_assertions))?;

            // only the primary instance gets here (single-instance plugin), and no
            // webview exists yet, so the profile folder is not locked
            let marker = paths::get().runtime.join(crate::CLEAR_USERDATA_MARKER);
            if marker.exists() {
                let skipped = paths::clear_dir(&paths::get().userdata, &[]);
                log::info!("webview profile cleared ({skipped} entries skipped)");
                let _ = std::fs::remove_file(marker);
            }

            autostart::repair();
            let mut current = settings::get();
            if current.autostart != autostart::is_enabled() || first_run {
                current.autostart = autostart::is_enabled();
                settings::replace(current)?;
            }

            apps::start();
            system::start();
            hotkeys::start(handle.clone());
            tray::create(&handle)?;
            widgets::popup::create(&handle)?;
            widgets::tooltip::create(&handle)?;
            widgets::overlay::create(&handle)?;
            // hidden until first used: their memory can be released meanwhile
            for label in [
                widgets::popup::LABEL,
                widgets::tooltip::LABEL,
                widgets::overlay::LABEL,
            ] {
                widgets::memory::idle(label);
            }
            widgets::memory::start(handle.clone());
            start_apps::refresh();
            win_key::sync(&handle);
            widgets::dock::start(handle.clone());
            // the window thread's heartbeat: the native taskbar is only hidden while
            // SideDock's windows respond (taskbar_visibility)
            let beat = handle.clone();
            crate::utils::spawn_supervised("ui-heartbeat", move || {
                loop {
                    let _ = beat.run_on_main_thread(taskbar_visibility::heartbeat);
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            });
            // native taskbar first: the toolbar may take its place
            taskbar_visibility::init(settings::get().taskbar.hide_native);
            widgets::toolbar::start(handle.clone());
            sync_toolbar();

            if first_run {
                widgets::settings::open(&handle)?;
            }
            log::info!("setup finished");
            Ok(())
        });

    let app = match builder.build(tauri::generate_context!()) {
        Ok(app) => app,
        Err(err) => {
            log::error!("failed to build the app: {err}");
            crate::fatal(&format!("SideDock failed to start:\n{err}"));
            return;
        }
    };

    app.run(|app, event| match event {
        // closing the settings window must not quit: the dock and tray stay alive
        RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        RunEvent::Exit => {
            win_key::shutdown();
            system_tray::stop();
            taskbar_visibility::set_hidden(false);
            widgets::toolbar::shutdown(app);
            widgets::dock::shutdown(app);
            session::end();
        }
        _ => {}
    });
}
