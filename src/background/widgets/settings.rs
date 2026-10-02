//! Settings / home window. Created on demand and destroyed on close to free memory.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, WebviewWindow};

use super::WidgetWindow;
use crate::{
    error::{Result, ResultLogExt},
    state::settings,
    windows_api::monitor,
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

/// Preferred / minimum logical size, shrunk to fit small or low-resolution screens
/// (e.g. 1920x1080 at 150% leaves only ~1280x670 logical px of work area).
fn window_sizes() -> ((f64, f64), (f64, f64)) {
    const PREFERRED: (f64, f64) = (1160.0, 780.0);
    const MIN: (f64, f64) = (880.0, 600.0);
    let monitors = monitor::list();
    let target = {
        let mut p = windows::Win32::Foundation::POINT::default();
        let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p) };
        monitor::from_point(p.x, p.y)
    }
    .or_else(|| monitors.into_iter().find(|m| m.is_primary));
    let Some(m) = target else {
        return (PREFERRED, MIN);
    };
    let avail_w = m.work_area.width() as f64 / m.scale_factor * 0.94;
    let avail_h = m.work_area.height() as f64 / m.scale_factor * 0.94;
    let size = (PREFERRED.0.min(avail_w), PREFERRED.1.min(avail_h));
    (size, (MIN.0.min(size.0), MIN.1.min(size.1)))
}

fn create(app: &AppHandle) -> Result<()> {
    let ((width, height), (min_width, min_height)) = window_sizes();
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
    .min_inner_size(min_width, min_height)
    .inner_size(width, height)
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
