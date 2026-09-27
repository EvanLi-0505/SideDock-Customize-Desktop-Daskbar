//! The dock ("SeelenWeg" in Seelen UI). One transparent webview per monitor.
//!
//! The backend owns the geometry: it computes the window rect from the settings and the
//! monitor work area, and registers the dock as a shell AppBar when it should reserve
//! space. Unlike Seelen UI the native Windows taskbar is never touched, so both bars can
//! live side by side.
//!
//! The window spans the whole edge and is deeper than the visible bar: the extra room on
//! the inner side hosts magnified icons and their labels. Only the bar (reported by the
//! page, see [`Hitbox`]) receives the mouse; everything else is click-through
//! (see `autohide.rs`).
//!
//! Every mutation of window geometry goes through a single worker thread
//! ([`request`]) so concurrent events can never race each other.

pub mod autohide;
pub mod native_taskbar;

use std::{
    collections::{HashMap, HashSet},
    sync::{
        LazyLock, OnceLock,
        mpsc::{Sender, channel},
    },
    time::Duration,
};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{HWND_TOPMOST, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SetWindowPos},
};

use super::{WidgetWindow, hwnd_of, native, set_tool_window};
use crate::{
    app,
    error::{Result, ResultLogExt},
    state::settings::{self, DockMonitors, DockSide, HideMode},
    windows_api::{
        app_bar,
        monitor::{self, MonitorInfo},
        window::Rect,
    },
};

pub const EVENT_DOCK_INFO: &str = "dock-info";

/// Visible bar reported by the page, in CSS px relative to the dock window.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hitbox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// how far magnified icons reach past the bar toward the screen center
    pub inward: f64,
    /// how far the magnification wave can push icons past both ends of the bar
    pub along: f64,
}

#[derive(Debug, Clone)]
pub struct DockState {
    pub label: String,
    pub monitor: isize,
    pub hwnd: isize,
    pub scale: f64,
    pub hitbox: Option<Hitbox>,
    /// false while the window is click-through
    pub interactive: bool,
    pub ready: bool,
    /// the window was shown at least once (auto-hide may only "repair" it after that)
    pub shown: bool,
    /// hidden by the auto-hide logic (content slid out, click-through)
    pub hidden: bool,
    /// window hidden because a fullscreen app is focused on this monitor
    pub fullscreen_hidden: bool,
    pub dragging: bool,
    /// physical rect of the whole (partly transparent) window
    pub window: Rect,
    pub side: DockSide,
}

impl DockState {
    fn to_physical(&self, x: f64, y: f64, w: f64, h: f64) -> Rect {
        let s = self.scale;
        Rect {
            left: self.window.left + (x * s).round() as i32,
            top: self.window.top + (y * s).round() as i32,
            right: self.window.left + ((x + w) * s).round() as i32,
            bottom: self.window.top + ((y + h) * s).round() as i32,
        }
    }

    /// Physical rect of the visible bar.
    pub fn bar(&self) -> Rect {
        if let Some(h) = self.hitbox {
            return self.to_physical(h.x, h.y, h.width, h.height);
        }
        // until the page reports it: the full edge strip
        let thickness = (settings::get().dock.total_thickness() as f64 * self.scale).round() as i32;
        let w = self.window;
        match self.side {
            DockSide::Left => Rect {
                right: w.left + thickness,
                ..w
            },
            DockSide::Right => Rect {
                left: w.right - thickness,
                ..w
            },
            DockSide::Top => Rect {
                bottom: w.top + thickness,
                ..w
            },
            DockSide::Bottom => Rect {
                top: w.bottom - thickness,
                ..w
            },
        }
    }

    /// Bar plus the room magnified icons use while the cursor is over the dock.
    pub fn hover_area(&self) -> Rect {
        let mut r = self.bar();
        let Some(h) = self.hitbox else {
            return r;
        };
        let inward = (h.inward * self.scale).round() as i32;
        let along = (h.along * self.scale).round() as i32;
        match self.side {
            DockSide::Left => r.right += inward,
            DockSide::Right => r.left -= inward,
            DockSide::Top => r.bottom += inward,
            DockSide::Bottom => r.top -= inward,
        }
        if self.side.is_horizontal() {
            r.left -= along;
            r.right += along;
        } else {
            r.top -= along;
            r.bottom += along;
        }
        let w = self.window;
        Rect {
            left: r.left.max(w.left),
            top: r.top.max(w.top),
            right: r.right.min(w.right),
            bottom: r.bottom.min(w.bottom),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockInfo {
    pub label: String,
    pub monitor: MonitorInfo,
    pub rect: Rect,
    pub hidden: bool,
}

pub static DOCKS: LazyLock<Mutex<HashMap<String, DockState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

enum Job {
    Reconcile,
    Layout(String),
    LayoutAll,
}

static WORKER: OnceLock<Sender<Job>> = OnceLock::new();

fn send(job: Job) {
    if let Some(tx) = WORKER.get() {
        let _ = tx.send(job);
    }
}

/// Create/destroy docks so there is exactly one per target monitor, then relayout.
pub fn request_reconcile() {
    send(Job::Reconcile);
}

pub fn request_layout(label: &str) {
    send(Job::Layout(label.to_string()));
}

pub fn request_layout_all() {
    send(Job::LayoutAll);
}

pub fn start(app: AppHandle) {
    let (tx, rx) = channel::<Job>();
    let _ = WORKER.set(tx);
    autohide::start(app.clone());
    crate::utils::spawn_supervised("dock-worker", move || {
        while let Ok(job) = rx.recv() {
            // coalesce bursts: a reconcile implies a full layout
            let mut reconcile = matches!(job, Job::Reconcile);
            let mut all = matches!(job, Job::LayoutAll);
            let mut labels: HashSet<String> = HashSet::new();
            if let Job::Layout(l) = job {
                labels.insert(l);
            }
            while let Ok(next) = rx.recv_timeout(Duration::from_millis(16)) {
                match next {
                    Job::Reconcile => reconcile = true,
                    Job::LayoutAll => all = true,
                    Job::Layout(l) => {
                        labels.insert(l);
                    }
                }
            }
            if reconcile {
                reconcile_now(&app).log_error();
                all = true;
            }
            let targets: Vec<String> = if all {
                DOCKS.lock().keys().cloned().collect()
            } else {
                labels.into_iter().collect()
            };
            for label in targets {
                layout_now(&app, &label).log_error();
            }
        }
    });
    watch_monitors();
    request_reconcile();
}

fn label_for(monitor: &MonitorInfo) -> String {
    format!("dock-{}", monitor.id as usize)
}

fn target_monitors() -> Vec<MonitorInfo> {
    let settings = settings::get().dock;
    if !settings.enabled {
        return Vec::new();
    }
    let all = monitor::list();
    match settings.monitors {
        DockMonitors::All => all,
        DockMonitors::Primary => all.into_iter().filter(|m| m.is_primary).collect(),
    }
}

fn reconcile_now(app: &AppHandle) -> Result<()> {
    let targets = target_monitors();
    let wanted: HashSet<String> = targets.iter().map(label_for).collect();

    // remove docks that are no longer wanted
    let stale: Vec<DockState> = {
        let mut docks = DOCKS.lock();
        let stale_labels: Vec<String> = docks
            .keys()
            .filter(|l| !wanted.contains(*l))
            .cloned()
            .collect();
        stale_labels
            .into_iter()
            .filter_map(|l| docks.remove(&l))
            .collect()
    };
    for dock in stale {
        app_bar::unregister(dock.hwnd);
        if let Some(window) = app.get_webview_window(&dock.label) {
            window.destroy().log_error();
        }
        log::info!("dock {} removed", dock.label);
    }

    // create the missing ones
    for monitor in targets {
        let label = label_for(&monitor);
        if DOCKS.lock().contains_key(&label) {
            continue;
        }
        if let Some(existing) = app.get_webview_window(&label) {
            // leftover from a previous crash of the worker: reuse it
            existing.destroy().log_error();
        }
        let window = WidgetWindow {
            label: &label,
            widget: "dock",
            monitor_id: Some(monitor.id),
        }
        .builder(app)?
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .focused(false)
        .inner_size(1.0, 1.0)
        .build()?;
        set_tool_window(&window, false)?;
        let settings = settings::get().dock;
        DOCKS.lock().insert(
            label.clone(),
            DockState {
                label: label.clone(),
                monitor: monitor.id,
                hwnd: hwnd_of(&window)?,
                scale: monitor.scale_factor,
                hitbox: None,
                interactive: true,
                ready: false,
                shown: false,
                hidden: false,
                fullscreen_hidden: false,
                dragging: false,
                window: Rect::default(),
                side: settings.position,
            },
        );
        log::info!("dock {label} created on monitor {:?}", monitor.rect);
    }
    Ok(())
}

/// Monitor work area without the space reserved by this dock itself.
fn base_work_area(monitor: &MonitorInfo, hwnd: isize) -> Rect {
    let mut wa = monitor.work_area;
    if let Some((r, side)) = app_bar::reserved(hwnd) {
        match side {
            DockSide::Left if wa.left >= r.right => wa.left = r.left,
            DockSide::Right if wa.right <= r.left => wa.right = r.right,
            DockSide::Top if wa.top >= r.bottom => wa.top = r.top,
            DockSide::Bottom if wa.bottom <= r.top => wa.bottom = r.bottom,
            _ => {}
        }
    }
    // never go outside the physical monitor
    wa.left = wa.left.max(monitor.rect.left);
    wa.top = wa.top.max(monitor.rect.top);
    wa.right = wa.right.min(monitor.rect.right);
    wa.bottom = wa.bottom.min(monitor.rect.bottom);
    wa
}

fn layout_now(app: &AppHandle, label: &str) -> Result<()> {
    let settings = settings::get().dock;
    let Some(mut dock) = DOCKS.lock().get(label).cloned() else {
        return Ok(());
    };
    let Some(monitor) = monitor::by_id(dock.monitor) else {
        // the monitor vanished, reconcile will take care of it
        request_reconcile();
        return Ok(());
    };

    let side = settings.position;
    if dock.side != side {
        app_bar::unregister(dock.hwnd);
        dock.side = side;
    }

    let wa = base_work_area(&monitor, dock.hwnd);
    let scale = monitor.scale_factor;
    let thickness = (settings.total_thickness() as f64 * scale).round() as i32;
    // the page sizes the bar itself (full width or fit content) inside a window that
    // covers the whole edge plus the magnification room
    let depth = thickness + (settings.magnification_room() as f64 * scale).round() as i32;
    let rect = match side {
        DockSide::Left => Rect {
            left: wa.left,
            top: wa.top,
            right: wa.left + depth,
            bottom: wa.bottom,
        },
        DockSide::Right => Rect {
            left: wa.right - depth,
            top: wa.top,
            right: wa.right,
            bottom: wa.bottom,
        },
        DockSide::Top => Rect {
            left: wa.left,
            top: wa.top,
            right: wa.right,
            bottom: wa.top + depth,
        },
        DockSide::Bottom => Rect {
            left: wa.left,
            top: wa.bottom - depth,
            right: wa.right,
            bottom: wa.bottom,
        },
    };

    // reserve screen space like the native taskbar only when the dock never hides
    let reserve = settings.hide_mode == HideMode::Never && dock.ready && !dock.fullscreen_hidden;
    if reserve {
        let bar = match side {
            DockSide::Left => Rect {
                left: wa.left,
                top: wa.top,
                right: wa.left + thickness,
                bottom: wa.bottom,
            },
            DockSide::Right => Rect {
                left: wa.right - thickness,
                top: wa.top,
                right: wa.right,
                bottom: wa.bottom,
            },
            DockSide::Top => Rect {
                left: wa.left,
                top: wa.top,
                right: wa.right,
                bottom: wa.top + thickness,
            },
            DockSide::Bottom => Rect {
                left: wa.left,
                top: wa.bottom - thickness,
                right: wa.right,
                bottom: wa.bottom,
            },
        };
        app_bar::register(dock.hwnd, bar, side).log_error();
    } else {
        app_bar::unregister(dock.hwnd);
    }

    if rect != dock.window {
        unsafe {
            SetWindowPos(
                HWND(dock.hwnd as *mut _),
                Some(HWND_TOPMOST),
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            )?;
        }
    }

    let info = DockInfo {
        label: label.to_string(),
        monitor,
        rect,
        hidden: dock.hidden,
    };
    if let Some(stored) = DOCKS.lock().get_mut(label) {
        stored.window = rect;
        stored.side = side;
        stored.scale = scale;
    }
    app::emit_to(app, label, EVENT_DOCK_INFO, &info);
    Ok(())
}

pub fn info(label: &str) -> Option<DockInfo> {
    let dock = DOCKS.lock().get(label).cloned()?;
    Some(DockInfo {
        label: dock.label,
        monitor: monitor::by_id(dock.monitor)?,
        rect: dock.window,
        hidden: dock.hidden,
    })
}

pub fn set_ready(label: &str) {
    let show = {
        let mut docks = DOCKS.lock();
        let Some(dock) = docks.get_mut(label) else {
            return;
        };
        dock.ready = true;
        (!dock.fullscreen_hidden).then_some(dock.hwnd)
    };
    request_layout(label);
    if let Some(hwnd) = show {
        // give the layout a moment so the window never flashes at the wrong spot
        let label = label.to_string();
        crate::utils::spawn_named("dock-show", move || {
            std::thread::sleep(Duration::from_millis(60));
            native::show_passive(hwnd);
            if let Some(dock) = DOCKS.lock().get_mut(&label) {
                dock.shown = true;
            }
        });
    }
}

pub fn set_hitbox(label: &str, hitbox: Hitbox) {
    if let Some(dock) = DOCKS.lock().get_mut(label) {
        dock.hitbox = Some(hitbox);
    }
}

pub fn set_dragging(label: &str, dragging: bool) {
    if let Some(dock) = DOCKS.lock().get_mut(label) {
        dock.dragging = dragging;
    }
}

/// Shows/hides the whole window (used for fullscreen apps).
pub(super) fn set_window_visible(hwnd: isize, visible: bool) {
    if visible {
        native::show_passive(hwnd);
    } else {
        native::hide(hwnd);
    }
}

/// Removes every dock and its AppBar reservation (used on exit).
pub fn shutdown(app: &AppHandle) {
    let docks: Vec<DockState> = DOCKS.lock().drain().map(|(_, d)| d).collect();
    for dock in docks {
        app_bar::unregister(dock.hwnd);
        if let Some(window) = app.get_webview_window(&dock.label) {
            let _ = window.destroy();
        }
    }
    app_bar::unregister_all();
}

/// Reconciles the docks when monitors are plugged/unplugged or the work area changes
/// (e.g. the native taskbar was moved or set to auto-hide).
fn watch_monitors() {
    crate::utils::spawn_supervised("monitor-watcher", || {
        let mut last = monitor::list();
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let current = monitor::list();
            if current != last {
                let ids_changed = current.iter().map(|m| m.id).collect::<Vec<_>>()
                    != last.iter().map(|m| m.id).collect::<Vec<_>>();
                last = current;
                if ids_changed {
                    log::info!("monitor configuration changed");
                    request_reconcile();
                } else {
                    request_layout_all();
                }
            }
        }
    });
}
