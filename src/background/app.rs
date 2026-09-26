use std::sync::OnceLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget, Manager, RunEvent};

use crate::{
    commands,
    error::{Result, ResultLogExt},
    modules::{apps, autostart, icons, system},
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

    let old = settings::replace(new)?;
    let new = settings::get();
    emit(EVENT_SETTINGS_CHANGED, &new);

    if old.language != new.language {
        tray::refresh(app);
    }
    if old.theme != new.theme {
        if let Some(w) = app.get_webview_window(widgets::settings::LABEL) {
            widgets::settings::apply_theme(&w);
        }
        if let Some(w) = app.get_webview_window(widgets::popup::LABEL) {
            widgets::popup::apply_theme(&w);
        }
    }
    if old.dock != new.dock {
        widgets::dock::request_reconcile();
    }
    Ok(new)
}

pub fn broadcast_dock_items(source: &str) {
    #[derive(Serialize, Clone)]
    struct Payload {
        source: String,
        items: dock_items::DockItems,
    }
    emit(
        EVENT_DOCK_ITEMS_CHANGED,
        &Payload {
            source: source.to_string(),
            items: dock_items::get(),
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
            let uri = request.uri().to_string();
            std::thread::spawn(move || {
                let bytes = icons::handle_request(&uri);
                let response = tauri::http::Response::builder()
                    .header("Content-Type", "image/png")
                    .header("Cache-Control", "max-age=604800")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(bytes)
                    .unwrap_or_default();
                responder.respond(response);
            });
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
            tray::create(&handle)?;
            widgets::popup::create(&handle)?;
            widgets::tooltip::create(&handle)?;
            widgets::dock::start(handle.clone());

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
            widgets::dock::shutdown(app);
            session::end();
        }
        _ => {}
    });
}
