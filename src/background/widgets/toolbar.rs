//! The toolbar ("Fancy Toolbar" in Seelen UI): a full-width bar with the start button,
//! the focused app, the clock, the system tray (every icon inline) and the system
//! modules. One webview per monitor (all monitors or only the primary one).
//!
//! The toolbar **never reserves screen space**: any change of the work area makes
//! Explorer rearrange the desktop icons. Placement:
//! * **in place of the native taskbar** (position "bottom" while the taskbar is hidden,
//!   see `modules/taskbar_visibility.rs`): the toolbar takes the rect of the hidden
//!   taskbar, whose space Explorer still reserves.
//! * otherwise at the top or bottom edge of the work area (above a visible taskbar),
//!   `height` CSS px high, floating over windows: always shown, hidden while a window
//!   overlaps it, or hidden until the cursor reaches its edge.
//!
//! Like the dock window it is shown and hidden only through Win32 (`super::native`).

use std::{
    collections::HashMap,
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use windows::Win32::{
    Foundation::{HWND, POINT},
    UI::WindowsAndMessaging::{
        GetCursorPos, HWND_TOPMOST, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SetWindowPos,
    },
};

use super::{
    WidgetWindow, dock::backdrop::BackdropTarget, hwnd_of, native, popup, set_tool_window,
};
use crate::{
    app,
    error::{Result, ResultLogExt},
    modules::{apps, shell_flyouts, taskbar_visibility},
    state::settings::{self, DockMonitors, DockSide, HideMode, ThemeMode, ToolbarPosition},
    windows_api::{
        monitor::{self, MonitorInfo},
        window::Rect,
    },
};

pub const EVENT_INFO: &str = "toolbar-info";
pub const EVENT_TRAY_DRAG: &str = "tray-drag";
/// px around the toolbar's outer edge that bring it back (auto-hide)
const EDGE_THRESHOLD: i32 = 2;

/// What the page needs to know about its placement.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolbarInfo {
    pub label: String,
    pub monitor: MonitorInfo,
    /// "top" | "bottom": popups open on the other side
    pub edge: &'static str,
    /// sits in the hidden taskbar's place
    pub replaces_taskbar: bool,
    /// CSS px at its left / right end covered by a Windows flyout (quick settings,
    /// notification center): the page keeps its items out of there
    pub covered_left: f64,
    pub covered_right: f64,
}

#[derive(Debug, Clone)]
struct ToolbarState {
    monitor: isize,
    hwnd: isize,
    rect: Rect,
    ready: bool,
    shown: bool,
    /// hidden by auto-hide
    hidden: bool,
    fullscreen_hidden: bool,
    replaces_taskbar: bool,
    auto_hide: bool,
    edge_bottom: bool,
    /// placed at least once (the docks were told about its strip)
    laid_out: bool,
    pending: Option<(bool, Instant)>,
    /// physical px covered by a Windows flyout at the left / right end
    covered: (i32, i32),
}

static TOOLBARS: LazyLock<Mutex<HashMap<String, ToolbarState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static STARTED: AtomicBool = AtomicBool::new(false);
static DIRTY: AtomicBool = AtomicBool::new(true);

fn label_for(monitor: &MonitorInfo) -> String {
    format!("toolbar-{}", monitor.id as usize)
}

/// Re-evaluates windows and placement (settings, monitors, taskbar visibility changed).
pub fn request_reconcile() {
    DIRTY.store(true, Ordering::Release);
}

pub fn start(app: AppHandle) {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    crate::utils::spawn_supervised("toolbar", move || {
        let mut tick: u32 = 0;
        let mut last_monitors = monitor::list();
        loop {
            std::thread::sleep(Duration::from_millis(50));
            tick = tick.wrapping_add(1);
            // monitors and the work area change without notice: check every second
            if tick.is_multiple_of(20) {
                let current = monitor::list();
                if current != last_monitors {
                    last_monitors = current;
                    DIRTY.store(true, Ordering::Release);
                }
            }
            if DIRTY.swap(false, Ordering::AcqRel) {
                reconcile(&app).log_error();
            }
            auto_hide(&app);
            update_covered(&app);
        }
    });
}

fn target_monitors() -> Vec<MonitorInfo> {
    let settings = settings::get().toolbar;
    if !settings.enabled {
        return Vec::new();
    }
    let all = monitor::list();
    match settings.monitors {
        DockMonitors::All => all,
        DockMonitors::Primary => all.into_iter().filter(|m| m.is_primary).collect(),
    }
}

fn reconcile(app: &AppHandle) -> Result<()> {
    let targets = target_monitors();
    let wanted: Vec<String> = targets.iter().map(label_for).collect();

    let mut stale: Vec<(String, ToolbarState)> = Vec::new();
    TOOLBARS.lock().retain(|label, state| {
        let keep = wanted.contains(label);
        if !keep {
            stale.push((label.clone(), state.clone()));
        }
        keep
    });
    let removed = !stale.is_empty();
    for (label, _) in stale {
        if let Some(window) = app.get_webview_window(&label) {
            window.destroy().log_error();
        }
    }
    if removed {
        super::dock::request_layout_all();
    }

    for monitor in targets {
        let label = label_for(&monitor);
        let exists = TOOLBARS.lock().contains_key(&label);
        if !exists {
            if let Some(old) = app.get_webview_window(&label) {
                old.destroy().log_error();
            }
            let window = WidgetWindow {
                label: &label,
                widget: "toolbar",
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
            TOOLBARS.lock().insert(
                label.clone(),
                ToolbarState {
                    monitor: monitor.id,
                    hwnd: hwnd_of(&window)?,
                    rect: Rect::default(),
                    ready: false,
                    shown: false,
                    hidden: false,
                    fullscreen_hidden: false,
                    replaces_taskbar: false,
                    auto_hide: false,
                    edge_bottom: true,
                    laid_out: false,
                    pending: None,
                    covered: (0, 0),
                },
            );
            log::info!("toolbar {label} created");
        }
        layout(app, &label, &monitor)?;
    }
    Ok(())
}

/// Where the hidden taskbar was last found on a monitor, and the monitor's geometry then.
#[derive(Clone, Copy)]
struct KnownSlot {
    monitor: Rect,
    work_area: Rect,
    slot: Rect,
}

static SLOTS: LazyLock<Mutex<HashMap<isize, KnownSlot>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Rect of a bottom taskbar whose space is reserved (not set to auto-hide).
fn detect_slot(monitor: &MonitorInfo) -> Option<Rect> {
    let r = taskbar_visibility::taskbar_on_monitor(&monitor.rect)?;
    let m = monitor.rect;
    let at_bottom = r.bottom >= m.bottom - 2 && r.top > m.top + m.height() / 2;
    let reserved = monitor.work_area.bottom <= r.top + 2;
    (at_bottom && reserved).then(|| Rect {
        left: r.left.max(m.left),
        top: r.top.max(m.top),
        right: r.right.min(m.right),
        bottom: r.bottom.min(m.bottom),
    })
}

/// The hidden taskbar's rect when the toolbar should take its place.
///
/// Explorer may move or resize the taskbar window while it is hidden; as long as the
/// monitor and its work area are the same as when the taskbar was last found, the space
/// is still reserved and the toolbar stays there instead of jumping above it.
fn taskbar_slot(monitor: &MonitorInfo) -> Option<Rect> {
    let mut slots = SLOTS.lock();
    if settings::get().toolbar.position != ToolbarPosition::Bottom
        || !taskbar_visibility::is_hidden()
    {
        slots.remove(&monitor.id);
        return None;
    }
    if let Some(slot) = detect_slot(monitor) {
        slots.insert(
            monitor.id,
            KnownSlot {
                monitor: monitor.rect,
                work_area: monitor.work_area,
                slot,
            },
        );
        return Some(slot);
    }
    let remembered = slots
        .get(&monitor.id)
        .filter(|k| k.monitor == monitor.rect && k.work_area == monitor.work_area)
        .map(|k| k.slot);
    log::warn!(
        "toolbar: hidden taskbar not found where expected (taskbar {:?}, monitor {:?}, work area {:?}), {}",
        taskbar_visibility::taskbar_on_monitor(&monitor.rect),
        monitor.rect,
        monitor.work_area,
        if remembered.is_some() {
            "keeping its last place"
        } else {
            "floating above it"
        }
    );
    remembered
}

fn layout(app: &AppHandle, label: &str, monitor: &MonitorInfo) -> Result<()> {
    let settings = settings::get().toolbar;
    let Some(mut state) = TOOLBARS.lock().get(label).cloned() else {
        return Ok(());
    };

    let slot = taskbar_slot(monitor);
    let replaces_taskbar = slot.is_some();
    let edge_bottom = replaces_taskbar || settings.position == ToolbarPosition::Bottom;
    let h = (settings.height as f64 * monitor.scale_factor).round() as i32;
    let rect = match slot {
        Some(slot) if settings.match_taskbar_height => slot,
        // another height: still anchored to the bottom of the screen
        Some(slot) => Rect {
            top: slot.bottom - h,
            ..slot
        },
        None => {
            // the outer edge, before SideDock's docks: they are placed inside the toolbar
            let wa = super::dock::base_work_area(monitor);
            if edge_bottom {
                Rect {
                    left: wa.left,
                    top: wa.bottom - h,
                    right: wa.right,
                    bottom: wa.bottom,
                }
            } else {
                Rect {
                    left: wa.left,
                    top: wa.top,
                    right: wa.right,
                    bottom: wa.top + h,
                }
            }
        }
    };

    if rect != state.rect {
        unsafe {
            SetWindowPos(
                HWND(state.hwnd as *mut _),
                Some(HWND_TOPMOST),
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            )?;
        }
    }
    // the docks make room for a floating toolbar: tell them when its strip changes
    let strip_changed = rect.height() != state.rect.height()
        || replaces_taskbar != state.replaces_taskbar
        || edge_bottom != state.edge_bottom
        || !state.laid_out;
    state.laid_out = true;
    state.rect = rect;
    state.replaces_taskbar = replaces_taskbar;
    state.edge_bottom = edge_bottom;
    state.auto_hide = !replaces_taskbar && settings.hide_mode != HideMode::Never;
    if !state.auto_hide && state.hidden {
        state.hidden = false;
        if state.ready && !state.fullscreen_hidden {
            native::show_passive(state.hwnd);
        }
    }
    TOOLBARS.lock().insert(label.to_string(), state);
    if strip_changed {
        super::dock::request_layout_all();
    }

    if let Some(info) = info(label) {
        app::emit_to(app, label, EVENT_INFO, &info);
    }
    Ok(())
}

/// The strip a floating toolbar takes at the edge of a monitor (`None` when there is no
/// toolbar there, or it sits in the hidden taskbar's place, outside the work area). Its
/// auto-hide state does not matter: the docks keep out of its way all the time.
pub fn floating_strip(monitor_id: isize) -> Option<(DockSide, i32)> {
    TOOLBARS
        .lock()
        .values()
        .find(|t| t.monitor == monitor_id && t.laid_out && !t.replaces_taskbar)
        .map(|t| {
            let side = if t.edge_bottom {
                DockSide::Bottom
            } else {
                DockSide::Top
            };
            (side, t.rect.height())
        })
}

pub fn info(label: &str) -> Option<ToolbarInfo> {
    let state = TOOLBARS.lock().get(label).cloned()?;
    let monitor = monitor::by_id(state.monitor)?;
    let scale = monitor.scale_factor;
    Some(ToolbarInfo {
        label: label.to_string(),
        edge: if state.edge_bottom { "bottom" } else { "top" },
        replaces_taskbar: state.replaces_taskbar,
        covered_left: f64::from(state.covered.0) / scale,
        covered_right: f64::from(state.covered.1) / scale,
        monitor,
    })
}

/// Tells each toolbar how much of its ends a Windows flyout covers. A toolbar floating
/// above the visible taskbar sits where quick settings and the notification center open;
/// they are above every ordinary window, so the toolbar's items move aside instead.
fn update_covered(app: &AppHandle) {
    let flyouts = shell_flyouts::open_rects();
    let mut changed: Vec<String> = Vec::new();
    for (label, state) in TOOLBARS.lock().iter_mut() {
        let r = state.rect;
        let mut covered = (0, 0);
        for f in flyouts.iter().filter(|f| f.intersects(&r)) {
            // a flyout hangs on one side of the screen: that end is covered
            if f.right >= r.right {
                covered.1 = covered.1.max(r.right - f.left.max(r.left));
            } else if f.left <= r.left {
                covered.0 = covered.0.max(f.right.min(r.right) - r.left);
            }
        }
        if covered != state.covered {
            state.covered = covered;
            changed.push(label.clone());
        }
    }
    for label in changed {
        if let Some(info) = info(&label) {
            app::emit_to(app, &label, EVENT_INFO, &info);
        }
    }
}

/// The page rendered: show the window (and reserve space if needed).
pub fn set_ready(label: &str) {
    let hwnd = {
        let mut toolbars = TOOLBARS.lock();
        let Some(state) = toolbars.get_mut(label) else {
            return;
        };
        state.ready = true;
        state.shown = true;
        state.hwnd
    };
    DIRTY.store(true, Ordering::Release);
    native::show_passive(hwnd);
}

fn cursor() -> (i32, i32) {
    let mut p = POINT::default();
    let _ = unsafe { GetCursorPos(&mut p) };
    (p.x, p.y)
}

fn inside(r: &Rect, (x, y): (i32, i32)) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

/// Fullscreen apps hide the toolbar; auto-hide modes hide it until the cursor reaches
/// its screen edge.
fn auto_hide(_app: &AppHandle) {
    let states: Vec<(String, ToolbarState)> = TOOLBARS
        .lock()
        .iter()
        .map(|(l, s)| (l.clone(), s.clone()))
        .collect();
    if states.is_empty() {
        return;
    }
    let settings = settings::get().toolbar;
    let mode = settings.hide_mode;
    let windows = apps::windows();
    let foreground = crate::windows_api::window::Window::foreground().map(|w| w.0);
    let fg_app = windows.iter().find(|w| Some(w.hwnd) == foreground);
    let pos = cursor();
    let popup_owner = popup::current_owner();

    for (label, state) in states {
        if !state.ready {
            continue;
        }
        let fullscreen = fg_app.is_some_and(|w| w.is_fullscreen && w.monitor == state.monitor);
        if fullscreen != state.fullscreen_hidden {
            if fullscreen {
                native::hide(state.hwnd);
            } else if !state.hidden {
                native::show_passive(state.hwnd);
            }
            if let Some(s) = TOOLBARS.lock().get_mut(&label) {
                s.fullscreen_hidden = fullscreen;
            }
            DIRTY.store(true, Ordering::Release);
            continue;
        }
        if !state.auto_hide || fullscreen {
            continue;
        }

        let Some(mon) = monitor::by_id(state.monitor) else {
            continue;
        };
        let r = state.rect;
        // a thin band along the outer edge (the screen edge, or the top of a visible
        // taskbar); the screen edge itself always counts
        let at_edge = pos.0 >= r.left
            && pos.0 < r.right
            && if state.edge_bottom {
                (pos.1 >= r.bottom - 1 - EDGE_THRESHOLD && pos.1 <= r.bottom + EDGE_THRESHOLD)
                    || (r.bottom >= mon.rect.bottom - 1 && pos.1 >= mon.rect.bottom - 1)
            } else {
                (pos.1 <= r.top + EDGE_THRESHOLD && pos.1 >= r.top - EDGE_THRESHOLD)
                    || (r.top <= mon.rect.top && pos.1 <= mon.rect.top)
            };
        let engaged = at_edge
            || (!state.hidden && inside(&r, pos))
            || popup_owner.as_deref() == Some(label.as_str());
        let overlapped = fg_app.is_some()
            && windows.iter().any(|w| {
                w.monitor == state.monitor
                    && !w.is_iconic
                    && w.rect.is_some_and(|wr| wr.intersects(&r))
            });
        let want_hidden = match mode {
            HideMode::Never => false,
            HideMode::Always => !engaged,
            HideMode::OnOverlap => overlapped && !engaged,
        };
        let mut toolbars = TOOLBARS.lock();
        let Some(s) = toolbars.get_mut(&label) else {
            continue;
        };
        if want_hidden == s.hidden {
            s.pending = None;
            continue;
        }
        let since = match s.pending {
            Some((target, since)) if target == want_hidden => since,
            _ => {
                s.pending = Some((want_hidden, Instant::now()));
                continue;
            }
        };
        let delay = Duration::from_millis(u64::from(if want_hidden {
            settings.delay_to_hide
        } else {
            settings.delay_to_show
        }));
        if since.elapsed() >= delay {
            s.pending = None;
            s.hidden = want_hidden;
            if want_hidden {
                native::hide(s.hwnd);
            } else {
                native::show_passive(s.hwnd);
            }
        }
    }
}

/// Blurred backgrounds under the toolbars (when their theme is the frosted glass).
pub fn backdrop_targets() -> Vec<BackdropTarget> {
    if settings::toolbar_theme() != ThemeMode::Glass {
        return Vec::new();
    }
    TOOLBARS
        .lock()
        .iter()
        .map(|(label, s)| BackdropTarget {
            label: label.clone(),
            owner: s.hwnd,
            rect: s.rect,
            radius: 0,
            visible: s.ready
                && s.shown
                && !s.hidden
                && !s.fullscreen_hidden
                && native::is_visible(s.hwnd),
        })
        .collect()
}

/// Removes the toolbars (exit).
pub fn shutdown(app: &AppHandle) {
    let all: Vec<(String, ToolbarState)> = TOOLBARS.lock().drain().collect();
    for (label, _) in all {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.destroy();
        }
    }
}

// ---------------- dragging tray icons ----------------

/// Where the cursor is for a window taking part in a tray icon drag, in its CSS px.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrayDragEvent<'a> {
    phase: &'a str,
    key: &'a str,
    x: f64,
    y: f64,
    inside: bool,
}

/// Sends the cursor position to every toolbar and to the overflow flyout. When a drag
/// ends, the flyout gets the focus back (it was shown without it during the drag) so it
/// closes again when the user clicks elsewhere.
pub fn tray_drag(app: &AppHandle, phase: &str, key: &str) -> Result<()> {
    let (cx, cy) = cursor();
    let mut labels: Vec<String> = TOOLBARS.lock().keys().cloned().collect();
    if popup::current_owner().is_some() {
        labels.push(popup::LABEL.to_string());
    }
    for label in labels {
        let Some(window) = app.get_webview_window(&label) else {
            continue;
        };
        let (Ok(origin), Ok(size), Ok(scale)) = (
            window.inner_position(),
            window.inner_size(),
            window.scale_factor(),
        ) else {
            continue;
        };
        let inside = native::is_visible(hwnd_of(&window)?)
            && cx >= origin.x
            && cy >= origin.y
            && cx < origin.x + size.width as i32
            && cy < origin.y + size.height as i32;
        app::emit_to(
            app,
            &label,
            EVENT_TRAY_DRAG,
            &TrayDragEvent {
                phase,
                key,
                x: f64::from(cx - origin.x) / scale,
                y: f64::from(cy - origin.y) / scale,
                inside,
            },
        );
    }
    if phase != "move" {
        popup::activate(app);
    }
    Ok(())
}
