//! Pointer hit-testing and auto-hide (port of Seelen UI `weg/state/hidden.svelte.ts`,
//! moved to the backend so it keeps working even while the webview is throttled).
//!
//! * Hit-testing (every frame): the dock window is larger than the visible bar, so it
//!   is click-through except while the cursor is over the bar, or over the magnified
//!   icons once the cursor has entered the bar.
//! * Auto-hide: hidden docks keep their window; the page slides the content out and the
//!   window stays click-through. Fullscreen apps hide the window entirely.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::AppHandle;
use windows::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};

use super::{DOCKS, DockState, request_layout, set_window_visible};
use crate::{
    app,
    modules::apps,
    state::settings::{self, DockSide, HideMode},
    widgets::{native, popup},
    windows_api::{monitor, window::Rect, window::Window},
};

pub const EVENT_DOCK_HIDDEN: &str = "dock-hidden";
/// Sent when the cursor leaves the dock, so the page can relax the magnification wave
/// (a click-through window receives no `pointerleave`).
pub const EVENT_POINTER_LEAVE: &str = "dock-pointer-leave";
const EDGE_THRESHOLD: i32 = 2;
const FRAME: Duration = Duration::from_millis(16);
/// auto-hide runs every Nth frame (~50ms)
const AUTOHIDE_EVERY: u32 = 3;

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

fn inside(r: &Rect, (x, y): (i32, i32)) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

/// Makes the window receive the mouse only where the dock actually is.
fn update_hit_test(app: &AppHandle, dock: &DockState, pos: (i32, i32)) {
    let interactive = dock.ready
        && !dock.hidden
        && !dock.fullscreen_hidden
        && (dock.dragging
            || inside(&dock.bar(), pos)
            || (dock.interactive && inside(&dock.hover_area(), pos)));
    if interactive == dock.interactive {
        return;
    }
    if let Some(d) = DOCKS.lock().get_mut(&dock.label) {
        d.interactive = interactive;
    }
    native::set_click_through(dock.hwnd, !interactive);
    if !interactive {
        app::emit_to(app, &dock.label, EVENT_POINTER_LEAVE, &());
    }
}

pub fn start(app: AppHandle) {
    crate::utils::spawn_supervised("dock-pointer", move || {
        let mut pending: HashMap<String, Pending> = HashMap::new();
        let mut tick: u32 = 0;
        loop {
            std::thread::sleep(FRAME);
            tick = tick.wrapping_add(1);

            let docks: Vec<DockState> = DOCKS.lock().values().cloned().collect();
            if docks.is_empty() {
                continue;
            }
            let pos = cursor();
            for dock in &docks {
                update_hit_test(&app, dock, pos);
            }
            if !tick.is_multiple_of(AUTOHIDE_EVERY) {
                continue;
            }

            let settings = settings::get().dock;
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
                let r = dock.bar();
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
                let hovered = !dock.hidden && dock.interactive;
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
                    // click-through follows on the next frame (see update_hit_test)
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
