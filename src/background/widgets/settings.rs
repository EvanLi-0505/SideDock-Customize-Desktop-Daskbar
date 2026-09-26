//! Settings / home window. Created on demand and destroyed on close to free memory.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, WebviewWindow};

use super::WidgetWindow;
use crate::{
    error::{Result, ResultLogExt},
    state::settings,
};

pub const LABEL: &str = "settings";

static CREATING: AtomicBool = AtomicBool::new(false);

/// Opens the settings window or focuses it if it already exists.
/// Safe to call many times in a row (double clicks, repeated launches).
pub fn open(app: &AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.unminimize();
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    if CREATING.swap(true, Ordering::AcqRel) {
        return Ok(()); // another call is already creating it
    }
    // building a webview on the event-loop thread can dead-lock on Windows
    let app = app.clone();
    crate::utils::spawn_named("settings-create", move || {
        create(&app).log_error();
        CREATING.store(false, Ordering::Release);
    });
    Ok(())
}

fn create(app: &AppHandle) -> Result<()> {
    let window = WidgetWindow {
        label: LABEL,
        widget: "settings",
        monitor_id: None,
    }
    .builder(app)?
    .title("SideDock")
    .transparent(true)
    .decorations(false)
    .shadow(true)
    .resizable(true)
    .min_inner_size(880.0, 600.0)
    .inner_size(1160.0, 780.0)
    .center()
    .build()?;
    apply_theme(&window);
    log::info!("settings window opened");
    Ok(())
}

/// The page calls this once it has painted, so the window never flashes blank.
pub fn show_when_ready(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        window.show().log_error();
        window.set_focus().log_error();
    }
}

pub fn apply_theme(window: &WebviewWindow) {
    super::apply_backdrop(window, settings::get().theme, true);
}
