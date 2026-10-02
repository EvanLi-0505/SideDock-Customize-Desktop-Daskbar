//! Full-screen overlay shared by the launcher (SideDock start menu) and the window
//! switcher (stage manager). One window, created hidden at startup so that opening is
//! instant, moved to the monitor under the cursor each time it opens.
//!
//! * Launcher: the page renders every Start menu app (`modules/start_apps.rs`).
//! * Switcher: live previews are DWM thumbnails. The page lays them out and reports
//!   their rects ([`set_thumbnails`]); DWM draws them on top of the page.
//!
//! Switcher keyboard model (no keyboard hook): the shortcut is a `RegisterHotKey`
//! accelerator, each press moves the selection. While the shortcut's modifiers are held
//! a poller watches them: releasing after two or more presses switches to the selected
//! window; releasing after a single press keeps the switcher open for mouse picking.

use std::{
    collections::HashMap,
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewWindow};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Dwm::{
        DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY, DWM_TNP_RECTDESTINATION, DWM_TNP_RECTSOURCE,
        DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE, DwmQueryThumbnailSourceSize,
        DwmRegisterThumbnail, DwmUnregisterThumbnail, DwmUpdateThumbnailProperties,
    },
    UI::{
        Input::KeyboardAndMouse::GetAsyncKeyState,
        WindowsAndMessaging::{GetCursorPos, HWND_TOPMOST, SWP_NOACTIVATE, SetWindowPos},
    },
};

use super::{WidgetWindow, hwnd_of, native, set_tool_window};
use crate::{
    app,
    error::{Result, ResultLogExt},
    modules::start_apps,
    state::settings,
    windows_api::{monitor, window::Window},
};

pub const LABEL: &str = "overlay";
pub const EVENT_SHOW: &str = "overlay-show";
pub const EVENT_HIDDEN: &str = "overlay-hidden";
pub const EVENT_SWITCHER_STEP: &str = "switcher-step";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Launcher,
    Switcher,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowPayload {
    token: u64,
    mode: Mode,
}

/// A DWM thumbnail requested by the page, in CSS px relative to the overlay.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailRequest {
    /// stable slot id chosen by the page ("main", "side-3", ...)
    pub key: String,
    pub hwnd: isize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// visible part, from the top-left, as fractions of the full preview (stacked cards
    /// are partly covered by the next one; DWM would otherwise draw over it)
    #[serde(default = "one")]
    pub visible_width: f64,
    #[serde(default = "one")]
    pub visible_height: f64,
}

fn one() -> f64 {
    1.0
}

struct Thumbnail {
    source: isize,
    handle: isize,
}

#[derive(Default)]
struct State {
    /// mode being shown (or about to be shown once the page is ready)
    mode: Option<Mode>,
    visible: bool,
    token: u64,
    scale: f64,
    thumbnails: HashMap<String, Thumbnail>,
    /// hotkey presses since the switcher opened
    presses: u32,
    /// window selected in the switcher
    selected: Option<isize>,
    hidden_at: Option<Instant>,
    shown_at: Option<Instant>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));
/// generation of the modifier poller, a new switcher session invalidates older ones
static POLL_GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn create(app: &AppHandle) -> Result<()> {
    if app.get_webview_window(LABEL).is_some() {
        return Ok(());
    }
    let window = WidgetWindow {
        label: LABEL,
        widget: "overlay",
        monitor_id: None,
    }
    .builder(app)?
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .focused(false)
    .inner_size(800.0, 600.0)
    .build()?;
    set_tool_window(&window, false)?;
    // dark acrylic whatever the Windows mode, like Launchpad (white text on top)
    let _ = window.set_theme(Some(tauri::Theme::Dark));
    apply_backdrop(&window);

    let handle = window.clone();
    window.on_window_event(move |event| match event {
        // clicking another window closes the overlay, like the start menu
        tauri::WindowEvent::Focused(false) => {
            // taking the foreground can briefly bounce focus: only a real click
            // elsewhere (after the opening) closes the overlay
            let settled = STATE
                .lock()
                .shown_at
                .is_some_and(|t| t.elapsed() > Duration::from_millis(300));
            if settled {
                hide(handle.app_handle());
            }
        }
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide(handle.app_handle());
        }
        _ => {}
    });
    Ok(())
}

/// Blurred desktop behind the overlay (system acrylic, active while the overlay is).
fn apply_backdrop(window: &WebviewWindow) {
    use tauri::window::{Color, Effect, EffectsBuilder};
    let tint = Color(16, 18, 22, 110);
    if let Err(err) = window.set_effects(
        EffectsBuilder::new()
            .effect(Effect::Acrylic)
            .color(tint)
            .build(),
    ) {
        log::debug!("overlay backdrop not applied: {err}");
    }
}

pub fn mode() -> Option<Mode> {
    let state = STATE.lock();
    state.visible.then_some(state.mode).flatten()
}

fn cursor_monitor() -> Option<monitor::MonitorInfo> {
    let mut p = POINT::default();
    let _ = unsafe { GetCursorPos(&mut p) };
    monitor::from_point(p.x, p.y).or_else(|| monitor::list().into_iter().find(|m| m.is_primary))
}

/// Moves the (hidden) window over the monitor under the cursor and asks the page to
/// render `mode`; the window is shown when the page calls [`ready`].
fn open(app: &AppHandle, mode: Mode) -> Result<()> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or("overlay window is not available")?;
    let mon = cursor_monitor().ok_or("no monitor")?;
    let hwnd = hwnd_of(&window)?;
    let r = mon.rect;
    unsafe {
        SetWindowPos(
            HWND(hwnd as *mut _),
            Some(HWND_TOPMOST),
            r.left,
            r.top,
            r.width(),
            r.height(),
            SWP_NOACTIVATE,
        )?;
    }
    super::tooltip::hide(app);
    super::popup::close(app);
    let token = {
        let mut state = STATE.lock();
        state.token += 1;
        state.mode = Some(mode);
        state.scale = mon.scale_factor;
        state.token
    };
    if mode == Mode::Launcher {
        start_apps::refresh();
    }
    app::emit_to(app, LABEL, EVENT_SHOW, &ShowPayload { token, mode });
    Ok(())
}

/// Called by the page once the requested view is rendered.
pub fn ready(app: &AppHandle, token: u64) -> Result<()> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or("overlay window is not available")?;
    {
        let mut state = STATE.lock();
        if state.token != token || state.mode.is_none() {
            return Ok(());
        }
        state.visible = true;
        state.shown_at = Some(Instant::now());
    }
    let hwnd = hwnd_of(&window)?;
    native::show_active(hwnd);
    // the switcher is opened while the user holds Alt: no synthetic Alt tap there
    let alt_tap = STATE.lock().mode == Some(Mode::Launcher);
    if let Err(err) = Window(hwnd).bring_to_foreground(alt_tap) {
        // stays usable with the mouse; keyboard focus comes with the first click
        log::warn!("overlay: {err}");
    }
    Ok(())
}

pub fn hide(app: &AppHandle) {
    let was_visible = {
        let mut state = STATE.lock();
        let was = state.visible || state.mode.is_some();
        state.visible = false;
        state.mode = None;
        state.presses = 0;
        state.selected = None;
        state.hidden_at = Some(Instant::now());
        for (_, t) in state.thumbnails.drain() {
            unsafe {
                let _ = DwmUnregisterThumbnail(t.handle);
            }
        }
        was
    };
    POLL_GENERATION.fetch_add(1, Ordering::AcqRel);
    if !was_visible {
        return;
    }
    if let Some(window) = app.get_webview_window(LABEL)
        && let Ok(hwnd) = hwnd_of(&window)
    {
        native::hide(hwnd);
    }
    app::emit_to(app, LABEL, EVENT_HIDDEN, &());
}

/// The click that closed the overlay (focus lost) must not reopen it right away.
fn just_hidden() -> bool {
    STATE
        .lock()
        .hidden_at
        .is_some_and(|t| t.elapsed() < Duration::from_millis(250))
}

// ======================= launcher =======================

pub fn toggle_launcher(app: &AppHandle) {
    if mode() == Some(Mode::Launcher) {
        hide(app);
        return;
    }
    if just_hidden() {
        return;
    }
    open(app, Mode::Launcher).log_error();
}

/// Starts an app of the launcher and closes the overlay.
pub fn launch(app: &AppHandle, id: &str, elevated: bool) -> Result<()> {
    hide(app);
    let entry = start_apps::list().into_iter().find(|a| a.id == id);
    match entry {
        Some(entry) if elevated && !entry.packaged && entry.path.is_some() => {
            let path = entry.path.unwrap_or_default();
            crate::windows_api::shell::launch(&path.to_string_lossy(), None, None, true)
        }
        _ => {
            crate::windows_api::shell::launch(&format!("shell:AppsFolder\\{id}"), None, None, false)
        }
    }
}

// ======================= switcher =======================

/// Virtual keys of the modifiers of the switcher shortcut (e.g. Alt for Alt+`).
fn held_modifiers() -> Vec<Vec<i32>> {
    let accelerator = settings::get()
        .shortcuts
        .get("window-switcher")
        .cloned()
        .unwrap_or_default();
    accelerator
        .split('+')
        .filter_map(|part| match part.trim() {
            "Ctrl" => Some(vec![0x11]),
            "Alt" => Some(vec![0x12]),
            "Shift" => Some(vec![0x10]),
            "Win" => Some(vec![0x5B, 0x5C]),
            _ => None,
        })
        .collect()
}

fn any_down(vks: &[i32]) -> bool {
    vks.iter()
        .any(|vk| unsafe { GetAsyncKeyState(*vk) } as u16 & 0x8000 != 0)
}

/// Called for every press of the switcher shortcut.
pub fn switcher_hotkey(app: &AppHandle) {
    let (open_now, presses) = {
        let mut state = STATE.lock();
        if state.mode == Some(Mode::Switcher) {
            state.presses += 1;
            (false, state.presses)
        } else {
            state.presses = 1;
            (true, 1)
        }
    };
    if open_now {
        if let Err(err) = open(app, Mode::Switcher) {
            log::warn!("switcher: {err}");
            return;
        }
        watch_modifiers(app.clone());
    } else {
        app::emit_to(app, LABEL, EVENT_SWITCHER_STEP, &presses);
    }
}

/// Polls the shortcut's modifiers until they are released (see the module docs).
fn watch_modifiers(app: AppHandle) {
    let generation = POLL_GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    let modifiers = held_modifiers();
    crate::utils::spawn_named("switcher-modifiers", move || {
        let started = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(15));
            if POLL_GENERATION.load(Ordering::Acquire) != generation {
                return; // closed or reopened meanwhile
            }
            let held = !modifiers.is_empty() && modifiers.iter().all(|vks| any_down(vks));
            if held && started.elapsed() < Duration::from_secs(120) {
                continue;
            }
            let (presses, selected) = {
                let state = STATE.lock();
                (state.presses, state.selected)
            };
            if presses >= 2 {
                if let Some(hwnd) = selected {
                    activate_window(&app, hwnd).log_error();
                } else {
                    hide(&app);
                }
            }
            // a single press: stay open for mouse / keyboard picking
            return;
        }
    });
}

pub fn switcher_select(hwnd: isize) {
    STATE.lock().selected = Some(hwnd);
}

pub fn activate_window(app: &AppHandle, hwnd: isize) -> Result<()> {
    hide(app);
    let window = Window(hwnd);
    if !window.is_window() {
        return Ok(());
    }
    window.focus()
}

/// Registers / moves / removes the DWM thumbnails drawn over the page.
pub fn set_thumbnails(app: &AppHandle, requests: Vec<ThumbnailRequest>) -> Result<()> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or("overlay window is not available")?;
    let dest = HWND(hwnd_of(&window)? as *mut _);
    let mut state = STATE.lock();
    if state.mode != Some(Mode::Switcher) {
        return Ok(());
    }
    let scale = state.scale;

    // drop slots that are gone or now show another window
    let wanted: HashMap<&str, isize> = requests.iter().map(|r| (r.key.as_str(), r.hwnd)).collect();
    state.thumbnails.retain(|key, t| {
        let keep = wanted.get(key.as_str()) == Some(&t.source);
        if !keep {
            unsafe {
                let _ = DwmUnregisterThumbnail(t.handle);
            }
        }
        keep
    });

    for r in requests {
        if !state.thumbnails.contains_key(&r.key) {
            match unsafe { DwmRegisterThumbnail(dest, HWND(r.hwnd as *mut _)) } {
                Ok(handle) => {
                    state.thumbnails.insert(
                        r.key.clone(),
                        Thumbnail {
                            source: r.hwnd,
                            handle,
                        },
                    );
                }
                Err(err) => {
                    log::debug!("thumbnail for {:x} not registered: {err}", r.hwnd);
                    continue;
                }
            }
        }
        let t = &state.thumbnails[&r.key];
        let fx = r.visible_width.clamp(0.0, 1.0);
        let fy = r.visible_height.clamp(0.0, 1.0);
        let source = unsafe { DwmQueryThumbnailSourceSize(t.handle) }.unwrap_or_default();
        let props = DWM_THUMBNAIL_PROPERTIES {
            dwFlags: DWM_TNP_RECTDESTINATION
                | DWM_TNP_RECTSOURCE
                | DWM_TNP_VISIBLE
                | DWM_TNP_OPACITY
                | DWM_TNP_SOURCECLIENTAREAONLY,
            rcDestination: RECT {
                left: (r.x * scale).round() as i32,
                top: (r.y * scale).round() as i32,
                right: ((r.x + r.width * fx) * scale).round() as i32,
                bottom: ((r.y + r.height * fy) * scale).round() as i32,
            },
            rcSource: RECT {
                left: 0,
                top: 0,
                right: (source.cx as f64 * fx).round() as i32,
                bottom: (source.cy as f64 * fy).round() as i32,
            },
            opacity: 255,
            fVisible: (fx > 0.01 && fy > 0.01).into(),
            fSourceClientAreaOnly: false.into(),
        };
        unsafe {
            let _ = DwmUpdateThumbnailProperties(t.handle, &props);
        }
    }
    Ok(())
}
