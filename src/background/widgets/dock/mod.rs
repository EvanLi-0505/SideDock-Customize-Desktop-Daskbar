//! The dock ("SeelenWeg" in Seelen UI). One transparent webview per monitor, plus up
//! to three auxiliary docks on the primary monitor (pinned items only, each on an edge
//! of its own; `settings::AuxDockSettings`). Every dock runs with its own effective
//! settings ([`effective_settings`]).
//!
//! Layout of several docks on one monitor ([`span`]): with no auxiliary dock, as before;
//! with one auxiliary dock and the main dock at the top or bottom, the main dock spans its
//! whole edge; otherwise the left and right docks span the full height and the top and
//! bottom docks sit between them. The toolbar, like the taskbar, keeps its whole edge:
//! docks are placed inside it.
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
pub mod backdrop;
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
    state::settings::{self, DockMonitors, DockSettings, DockSide, HideMode},
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
    /// bar shape for the frosted glass backdrop (only reported with that theme)
    pub backdrop: Option<backdrop::BackdropShape>,
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
    /// id of the auxiliary dock, `None` for the main dock
    pub aux: Option<String>,
}

impl DockState {
    pub(super) fn to_physical(&self, x: f64, y: f64, w: f64, h: f64) -> Rect {
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
        let thickness = (effective_settings(self.aux.as_deref()).total_thickness() as f64
            * self.scale)
            .round() as i32;
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
    /// id of the auxiliary dock, `None` for the main dock
    pub aux: Option<String>,
    /// the settings this dock runs with (an auxiliary dock's differ from the main one's)
    pub settings: DockSettings,
}

/// Settings a dock runs with: the main dock's, or an auxiliary dock's derived from them.
pub fn effective_settings(aux: Option<&str>) -> DockSettings {
    let all = settings::get();
    match aux {
        None => all.dock,
        Some(id) => all
            .aux_docks
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.effective(&all.dock))
            .unwrap_or(all.dock),
    }
}

/// Which pinned items a dock window shows: `None` for the main dock, the auxiliary
/// dock's id otherwise (from its window label, `auxdock-<id>-<monitor>`).
pub fn store_of(label: &str) -> Option<String> {
    let rest = label.strip_prefix("auxdock-")?;
    Some(rest.split('-').next()?.to_string())
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

fn label_for(monitor: &MonitorInfo, aux: Option<&str>) -> String {
    match aux {
        None => format!("dock-{}", monitor.id as usize),
        Some(id) => format!("auxdock-{id}-{}", monitor.id as usize),
    }
}

/// Enabled auxiliary docks (each on an edge the main dock does not use).
fn enabled_aux(all: &settings::AppSettings) -> Vec<settings::AuxDockSettings> {
    all.aux_docks
        .iter()
        .filter(|a| a.enabled && !(all.dock.enabled && a.position == all.dock.position))
        .cloned()
        .collect()
}

/// Every dock that should exist: (monitor, auxiliary dock id).
fn targets() -> Vec<(MonitorInfo, Option<String>)> {
    let all = settings::get();
    let monitors = monitor::list();
    let mut out = Vec::new();
    if all.dock.enabled {
        for m in &monitors {
            if all.dock.monitors == DockMonitors::All || m.is_primary {
                out.push((m.clone(), None));
            }
        }
    }
    if let Some(primary) = monitors.iter().find(|m| m.is_primary) {
        for aux in enabled_aux(&all) {
            out.push((primary.clone(), Some(aux.id)));
        }
    }
    out
}

fn reconcile_now(app: &AppHandle) -> Result<()> {
    let targets = targets();
    let wanted: HashSet<String> = targets
        .iter()
        .map(|(m, aux)| label_for(m, aux.as_deref()))
        .collect();

    // remove docks that are no longer wanted
    let mut stale: Vec<DockState> = Vec::new();
    DOCKS.lock().retain(|label, dock| {
        let keep = wanted.contains(label);
        if !keep {
            stale.push(dock.clone());
        }
        keep
    });
    for dock in stale {
        crate::widgets::memory::forget(&dock.label);
        app_bar::unregister(dock.hwnd);
        if let Some(window) = app.get_webview_window(&dock.label) {
            window.destroy().log_error();
        }
        log::info!("dock {} removed", dock.label);
    }

    // create the missing ones
    for (monitor, aux) in targets {
        let label = label_for(&monitor, aux.as_deref());
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
        let settings = effective_settings(aux.as_deref());
        DOCKS.lock().insert(
            label.clone(),
            DockState {
                label: label.clone(),
                monitor: monitor.id,
                hwnd: hwnd_of(&window)?,
                scale: monitor.scale_factor,
                hitbox: None,
                backdrop: None,
                interactive: true,
                ready: false,
                shown: false,
                hidden: false,
                fullscreen_hidden: false,
                dragging: false,
                window: Rect::default(),
                side: settings.position,
                aux,
            },
        );
        log::info!("dock {label} created on monitor {:?}", monitor.rect);
    }
    Ok(())
}

/// Monitor work area without the space SideDock's own docks reserve: what the taskbar
/// (and other applications' bars) leave. The toolbar and the docks are laid out in it.
pub fn base_work_area(monitor: &MonitorInfo) -> Rect {
    let mut wa = monitor.work_area;
    let hwnds: Vec<isize> = DOCKS
        .lock()
        .values()
        .filter(|d| d.monitor == monitor.id)
        .map(|d| d.hwnd)
        .collect();
    for hwnd in hwnds {
        let Some((r, side)) = app_bar::reserved(hwnd) else {
            continue;
        };
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

/// A dock present on a monitor: its edge, its thickness (physical px), auxiliary or not.
struct Bar {
    side: DockSide,
    thickness: i32,
    aux: bool,
}

fn bars_on(monitor: &MonitorInfo) -> Vec<Bar> {
    let all = settings::get();
    let thickness =
        |s: &DockSettings| (s.total_thickness() as f64 * monitor.scale_factor).round() as i32;
    let mut bars = Vec::new();
    if all.dock.enabled && (all.dock.monitors == DockMonitors::All || monitor.is_primary) {
        bars.push(Bar {
            side: all.dock.position,
            thickness: thickness(&all.dock),
            aux: false,
        });
    }
    if monitor.is_primary {
        for aux in enabled_aux(&all) {
            let s = aux.effective(&all.dock);
            bars.push(Bar {
                side: s.position,
                thickness: thickness(&s),
                aux: true,
            });
        }
    }
    bars
}

/// The part of its edge a dock may use, along that edge (x for top / bottom docks, y
/// for left / right ones), inside `area`.
fn span(side: DockSide, bars: &[Bar], area: Rect) -> (i32, i32) {
    let full = if side.is_horizontal() {
        (area.left, area.right)
    } else {
        (area.top, area.bottom)
    };
    let aux_count = bars.iter().filter(|b| b.aux).count();
    if aux_count == 0 {
        return full;
    }
    let main = bars.iter().find(|b| !b.aux);
    if aux_count == 1
        && let Some(main) = main.filter(|m| m.side.is_horizontal())
    {
        // the main dock keeps its whole edge; a side dock makes room for it
        if side == main.side || side.is_horizontal() {
            return full;
        }
        return match main.side {
            DockSide::Bottom => (area.top, area.bottom - main.thickness),
            _ => (area.top + main.thickness, area.bottom),
        };
    }
    if !side.is_horizontal() {
        return full;
    }
    let inset = |s: DockSide| bars.iter().find(|b| b.side == s).map_or(0, |b| b.thickness);
    (
        area.left + inset(DockSide::Left),
        area.right - inset(DockSide::Right),
    )
}

fn layout_now(app: &AppHandle, label: &str) -> Result<()> {
    let Some(mut dock) = DOCKS.lock().get(label).cloned() else {
        return Ok(());
    };
    let settings = effective_settings(dock.aux.as_deref());
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

    let base = base_work_area(&monitor);
    // a floating toolbar keeps its whole edge: docks go inside it
    let strip = crate::widgets::toolbar::floating_strip(monitor.id);
    let mut wa = base;
    match strip {
        Some((DockSide::Top, h)) => wa.top += h,
        Some((DockSide::Bottom, h)) => wa.bottom -= h,
        _ => {}
    }
    let scale = monitor.scale_factor;
    let thickness = (settings.total_thickness() as f64 * scale).round() as i32;
    // the page sizes the bar itself (full width or fit content) inside a window that
    // covers its part of the edge plus the magnification room
    let depth = thickness + (settings.magnification_room() as f64 * scale).round() as i32;
    let (from, to) = span(side, &bars_on(&monitor), wa);
    let rect = match side {
        DockSide::Left => Rect {
            left: wa.left,
            top: from,
            right: wa.left + depth,
            bottom: to,
        },
        DockSide::Right => Rect {
            left: wa.right - depth,
            top: from,
            right: wa.right,
            bottom: to,
        },
        DockSide::Top => Rect {
            left: from,
            top: wa.top,
            right: to,
            bottom: wa.top + depth,
        },
        DockSide::Bottom => Rect {
            left: from,
            top: wa.bottom - depth,
            right: to,
            bottom: wa.bottom,
        },
    };

    // reserve screen space like the native taskbar only when the dock never hides
    let reserve = settings.hide_mode == HideMode::Never && dock.ready && !dock.fullscreen_hidden;
    if reserve {
        // on the toolbar's edge the reservation covers the toolbar too, so maximized
        // windows stay clear of both
        let bar = match side {
            DockSide::Left => Rect {
                left: wa.left,
                top: from,
                right: wa.left + thickness,
                bottom: to,
            },
            DockSide::Right => Rect {
                left: wa.right - thickness,
                top: from,
                right: wa.right,
                bottom: to,
            },
            DockSide::Top => Rect {
                left: from,
                top: base.top,
                right: to,
                bottom: wa.top + thickness,
            },
            DockSide::Bottom => Rect {
                left: from,
                top: wa.bottom - thickness,
                right: to,
                bottom: base.bottom,
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
        aux: dock.aux.clone(),
        settings,
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
        settings: effective_settings(dock.aux.as_deref()),
        label: dock.label,
        monitor: monitor::by_id(dock.monitor)?,
        rect: dock.window,
        hidden: dock.hidden,
        aux: dock.aux,
    })
}

pub fn set_ready(label: &str) {
    crate::modules::taskbar_visibility::ui_ready();
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

pub fn set_backdrop(label: &str, shape: Option<backdrop::BackdropShape>) {
    if let Some(dock) = DOCKS.lock().get_mut(label) {
        dock.backdrop = shape;
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
    backdrop::shutdown();
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
