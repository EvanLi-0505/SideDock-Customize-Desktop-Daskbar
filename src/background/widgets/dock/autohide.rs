//! Auto-hide state machine (port of Seelen UI `weg/state/hidden.svelte.ts`, moved to the
//! backend so it keeps working even while the webview is throttled).
//!
//! Hidden docks keep their window: the page slides the content out and the window
//! becomes click-through. Fullscreen apps hide the window entirely.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::AppHandle;
use windows::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};

use super::{DOCKS, request_layout, set_window_visible};
use crate::{
    app,
    modules::apps,
    state::settings::{self, DockSide, HideMode},
    widgets::{native, popup},
    windows_api::{monitor, window::Window},
};

pub const EVENT_DOCK_HIDDEN: &str = "dock-hidden";
const EDGE_THRESHOLD: i32 = 2;

#[derive(Serialize, Clone)]
struct HiddenPayload {
    hidden: bool,
}

#[derive(Default)]
struct Pending {
    want_hidden: bool,
    since: Option<Instant>,
}

fn cursor() -> (i32, i32) {
    let mut p = POINT::default();
    let _ = unsafe { GetCursorPos(&mut p) };
    (p.x, p.y)
}

fn inside(r: &crate::windows_api::window::Rect, (x, y): (i32, i32)) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

pub fn start(app: AppHandle) {
    crate::utils::spawn_supervised("dock-autohide", move || {
        let mut pending: HashMap<String, Pending> = HashMap::new();
        loop {
            let settings = settings::get().dock;
            let active = settings.hide_mode != HideMode::Never || settings.hide_on_fullscreen;
            std::thread::sleep(Duration::from_millis(if active { 50 } else { 500 }));

            let docks: Vec<_> = DOCKS.lock().values().cloned().collect();
            if docks.is_empty() {
                continue;
            }
            let pos = cursor();
            let windows = apps::windows();
            let foreground = Window::foreground();
            let fg_hwnd = foreground.map(|w| w.0).unwrap_or(0);
            let fg_app = windows.iter().find(|w| w.hwnd == fg_hwnd);
            let popup_owner = popup::current_owner();

            for dock in docks {
                if !dock.ready {
                    continue;
                }
                let Some(mon) = monitor::by_id(dock.monitor) else {
                    continue;
                };

                // ---- fullscreen apps ----
                let fullscreen = settings.hide_on_fullscreen
                    && fg_app.is_some_and(|w| w.is_fullscreen && w.monitor == dock.monitor);
                if fullscreen != dock.fullscreen_hidden {
                    log::info!(
                        "dock {} fullscreen hide = {fullscreen} (foreground: {:?})",
                        dock.label,
                        fg_app.map(|w| (&w.title, &w.path))
                    );
                    if let Some(d) = DOCKS.lock().get_mut(&dock.label) {
                        d.fullscreen_hidden = fullscreen;
                    }
                    set_window_visible(dock.hwnd, !fullscreen);
                    request_layout(&dock.label);
                } else if dock.shown && !fullscreen && !native::is_visible(dock.hwnd) {
                    // something outside of our control hid the dock (explorer restart,
                    // a third-party tool...): bring it back
                    log::warn!(
                        "dock {} was hidden externally, showing it again",
                        dock.label
                    );
                    set_window_visible(dock.hwnd, true);
                }

                // ---- auto hide ----
                let r = dock.rect;
                let m = mon.rect;
                let at_edge = match settings.position {
                    DockSide::Left => {
                        pos.0 <= m.left + EDGE_THRESHOLD && pos.1 >= r.top && pos.1 < r.bottom
                    }
                    DockSide::Right => {
                        pos.0 >= m.right - 1 - EDGE_THRESHOLD && pos.1 >= r.top && pos.1 < r.bottom
                    }
                    DockSide::Top => {
                        pos.1 <= m.top + EDGE_THRESHOLD && pos.0 >= r.left && pos.0 < r.right
                    }
                    DockSide::Bottom => {
                        pos.1 >= m.bottom - 1 - EDGE_THRESHOLD && pos.0 >= r.left && pos.0 < r.right
                    }
                };
                let hovered = !dock.hidden && inside(&r, pos);
                let focused = fg_hwnd == dock.hwnd;
                let engaged = at_edge
                    || hovered
                    || focused
                    || dock.dragging
                    || popup_owner.as_deref() == Some(dock.label.as_str());

                // Seelen rule: only "overlapped" when the foreground is an app window
                let overlapped = fg_app.is_some()
                    && windows.iter().any(|w| {
                        w.monitor == dock.monitor
                            && !w.is_iconic
                            && w.rect.is_some_and(|wr| wr.intersects(&r))
                    });

                let want_hidden = match settings.hide_mode {
                    HideMode::Never => false,
                    HideMode::Always => !engaged,
                    HideMode::OnOverlap => overlapped && !engaged,
                };

                let p = pending.entry(dock.label.clone()).or_default();
                if want_hidden == dock.hidden {
                    p.since = None;
                    continue;
                }
                if p.want_hidden != want_hidden || p.since.is_none() {
                    p.want_hidden = want_hidden;
                    p.since = Some(Instant::now());
                }
                let delay = if want_hidden {
                    settings.delay_to_hide
                } else if at_edge {
                    settings.delay_to_show
                } else {
                    0 // e.g. the overlapping window went away
                };
                if p.since
                    .is_some_and(|s| s.elapsed() >= Duration::from_millis(delay as u64))
                {
                    p.since = None;
                    if let Some(d) = DOCKS.lock().get_mut(&dock.label) {
                        d.hidden = want_hidden;
                    }
                    native::set_click_through(dock.hwnd, want_hidden);
                    app::emit_to(
                        &app,
                        &dock.label,
                        EVENT_DOCK_HIDDEN,
                        &HiddenPayload {
                            hidden: want_hidden,
                        },
                    );
                }
            }
            pending.retain(|label, _| DOCKS.lock().contains_key(label));
        }
    });
}
