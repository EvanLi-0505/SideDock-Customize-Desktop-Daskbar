//! A single reusable popup window that hosts context menus, the window list, the
//! calendar, the keyboard selector and every other flyout. Reusing one window keeps
//! popups instant and cheap. Kinds are plain components on the frontend
//! (`src/ui/apps/popup/registry.ts`), so new popups need no backend change.

use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use super::{WidgetWindow, hwnd_of, native, set_rounded_corners, set_tool_window};
use crate::{
    app,
    error::Result,
    state::settings,
    windows_api::{monitor, window::Rect},
};

pub const LABEL: &str = "popup";
pub const EVENT_RENDER: &str = "popup-render";
pub const EVENT_CLOSED: &str = "popup-closed";
pub const EVENT_ACTION: &str = "popup-action";
const GAP: i32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Placement {
    /// to the right of the anchor
    Right,
    Left,
    Above,
    Below,
}

/// How the popup lines up with its anchor along the dock axis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Align {
    /// centered on the anchor (flyouts opened from a dock item)
    #[default]
    Center,
    /// starts at the anchor, like a native context menu opened at the cursor
    Start,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupRequest {
    pub kind: String,
    #[serde(default)]
    pub data: serde_json::Value,
    /// anchor rect in CSS px, relative to the calling window
    pub anchor: AnchorRect,
    pub placement: Placement,
    #[serde(default)]
    pub align: Align,
    /// identity used to toggle the popup when the same element is clicked twice
    #[serde(default)]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct AnchorRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderPayload {
    token: u64,
    kind: String,
    data: serde_json::Value,
    owner: String,
    placement: Placement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionPayload {
    pub kind: String,
    pub action: String,
    pub value: serde_json::Value,
}

#[derive(Default)]
struct PopupState {
    token: u64,
    owner: Option<String>,
    key: Option<String>,
    /// physical screen rect of the anchor
    anchor: Rect,
    placement: Option<Placement>,
    align: Align,
    monitor: isize,
    visible: bool,
    /// token + physical rect of what is currently on screen
    shown: Option<(u64, Rect)>,
    closed_at: Option<(Instant, Option<String>)>,
}

static STATE: LazyLock<Mutex<PopupState>> = LazyLock::new(|| Mutex::new(PopupState::default()));

pub fn create(app: &AppHandle) -> Result<()> {
    if app.get_webview_window(LABEL).is_some() {
        return Ok(());
    }
    let window = WidgetWindow {
        label: LABEL,
        widget: "popup",
        monitor_id: None,
    }
    .builder(app)?
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .inner_size(300.0, 200.0)
    .build()?;
    set_tool_window(&window, false)?;
    set_rounded_corners(&window);
    apply_theme(&window);

    let handle = window.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Focused(false) => close_internal(&handle, true),
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            close_internal(&handle, false);
        }
        _ => {}
    });
    Ok(())
}

pub fn apply_theme(window: &WebviewWindow) {
    super::apply_backdrop(window, settings::get().theme, false);
}

pub fn current_owner() -> Option<String> {
    let state = STATE.lock();
    state.visible.then(|| state.owner.clone()).flatten()
}

fn close_internal(window: &WebviewWindow, by_blur: bool) {
    let owner = {
        let mut state = STATE.lock();
        if !state.visible {
            return;
        }
        state.visible = false;
        state.closed_at = Some((Instant::now(), state.key.clone()));
        state.owner.take()
    };
    if let Ok(hwnd) = hwnd_of(window) {
        native::hide(hwnd);
    }
    if let Some(owner) = owner {
        app::emit_to(window.app_handle(), &owner, EVENT_CLOSED, &by_blur);
    }
}

pub fn close(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        close_internal(&window, false);
    }
}

pub fn open(app: &AppHandle, caller: &WebviewWindow, request: PopupRequest) -> Result<()> {
    // clicking the element that opened the popup closes it instead of reopening
    {
        let state = STATE.lock();
        if let (Some((at, key)), Some(req_key)) = (&state.closed_at, &request.key)
            && at.elapsed() < Duration::from_millis(250)
            && key.as_ref() == Some(req_key)
        {
            return Ok(());
        }
    }

    let scale = caller.scale_factor()?;
    let origin = caller.inner_position()?;
    let a = request.anchor;
    let anchor = Rect {
        left: origin.x + (a.x * scale).round() as i32,
        top: origin.y + (a.y * scale).round() as i32,
        right: origin.x + ((a.x + a.width) * scale).round() as i32,
        bottom: origin.y + ((a.y + a.height) * scale).round() as i32,
    };
    let monitor = monitor::from_point(
        anchor.left + anchor.width() / 2,
        anchor.top + anchor.height() / 2,
    )
    .map(|m| m.id)
    .unwrap_or(0);

    let token = {
        let mut state = STATE.lock();
        state.token += 1;
        state.owner = Some(caller.label().to_string());
        state.key = request.key.clone();
        state.anchor = anchor;
        state.placement = Some(request.placement);
        state.align = request.align;
        state.monitor = monitor;
        state.token
    };

    app::emit_to(
        app,
        LABEL,
        EVENT_RENDER,
        &RenderPayload {
            token,
            kind: request.kind,
            data: request.data,
            owner: caller.label().to_string(),
            placement: request.placement,
        },
    );
    Ok(())
}

fn monitor_for(id: isize) -> Result<monitor::MonitorInfo> {
    Ok(monitor::by_id(id)
        .or_else(|| monitor::list().into_iter().find(|m| m.is_primary))
        .ok_or("no monitor")?)
}

fn place(window: &WebviewWindow, token: u64, rect: Rect) -> Result<()> {
    window.set_size(PhysicalSize::new(rect.width() as u32, rect.height() as u32))?;
    window.set_position(PhysicalPosition::new(rect.left, rect.top))?;
    STATE.lock().shown = Some((token, rect));
    Ok(())
}

/// Called by the popup page once the requested content is rendered and measured.
pub fn ready(app: &AppHandle, token: u64, width: f64, height: f64) -> Result<()> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or("popup window is not available")?;
    let (anchor, placement, align, monitor_id) = {
        let state = STATE.lock();
        if state.token != token || state.owner.is_none() {
            return Ok(()); // a newer request superseded this one
        }
        (
            state.anchor,
            state.placement.unwrap_or(Placement::Right),
            state.align,
            state.monitor,
        )
    };

    let mon = monitor_for(monitor_id)?;
    let scale = mon.scale_factor;
    let w = (width * scale).ceil() as i32;
    let h = (height * scale).ceil() as i32;
    let gap = (GAP as f64 * scale) as i32;
    let area = mon.work_area;

    let centered_y = anchor.top + anchor.height() / 2 - h / 2;
    let centered_x = anchor.left + anchor.width() / 2 - w / 2;
    let (mut x, mut y) = match (placement, align) {
        (Placement::Right, Align::Center) => (anchor.right + gap, centered_y),
        (Placement::Right, Align::Start) => (anchor.right + gap, anchor.top),
        (Placement::Left, Align::Center) => (anchor.left - gap - w, centered_y),
        (Placement::Left, Align::Start) => (anchor.left - gap - w, anchor.top),
        (Placement::Above, Align::Center) => (centered_x, anchor.top - gap - h),
        (Placement::Above, Align::Start) => (anchor.left, anchor.top - gap - h),
        (Placement::Below, Align::Center) => (centered_x, anchor.bottom + gap),
        (Placement::Below, Align::Start) => (anchor.left, anchor.bottom + gap),
    };
    x = x.clamp(area.left + gap, (area.right - w - gap).max(area.left));
    y = y.clamp(area.top + gap, (area.bottom - h - gap).max(area.top));

    place(
        &window,
        token,
        Rect {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        },
    )?;
    // focus is required so that clicking elsewhere closes the popup (blur)
    native::show_active(hwnd_of(&window)?);
    STATE.lock().visible = true;
    Ok(())
}

/// Resizes the visible popup (e.g. when a submenu expands).
///
/// Content that is already on screen must never move under the cursor, so the popup
/// only grows away from the dock: the edges facing the dock stay where they are and the
/// size is capped by the work area (the page scrolls whatever does not fit).
pub fn resize(app: &AppHandle, token: u64, width: f64, height: f64) -> Result<()> {
    let (current, placement, monitor_id) = {
        let state = STATE.lock();
        match state.shown {
            Some((shown_token, rect)) if state.visible && shown_token == token => (
                rect,
                state.placement.unwrap_or(Placement::Right),
                state.monitor,
            ),
            _ => return Ok(()),
        }
    };
    let window = app
        .get_webview_window(LABEL)
        .ok_or("popup window is not available")?;
    let mon = monitor_for(monitor_id)?;
    let scale = mon.scale_factor;
    let gap = (GAP as f64 * scale) as i32;
    let area = mon.work_area;
    let w = (width * scale).ceil() as i32;
    let h = (height * scale).ceil() as i32;

    // grow right/down, except toward the left for right docks and upward for bottom docks
    let grow_left = placement == Placement::Left;
    let grow_up = placement == Placement::Above;
    let rect = {
        let (left, right) = if grow_left {
            let w = w.min(current.right - area.left - gap);
            (current.right - w, current.right)
        } else {
            let w = w.min(area.right - gap - current.left);
            (current.left, current.left + w)
        };
        let (top, bottom) = if grow_up {
            let h = h.min(current.bottom - area.top - gap);
            (current.bottom - h, current.bottom)
        } else {
            let h = h.min(area.bottom - gap - current.top);
            (current.top, current.top + h)
        };
        Rect {
            left,
            top,
            right,
            bottom,
        }
    };
    if rect != current {
        place(&window, token, rect)?;
    }
    Ok(())
}

/// Forwards an action chosen in the popup (menu item, toggle, ...) to its owner.
pub fn action(app: &AppHandle, payload: ActionPayload, close_after: bool) {
    let owner = STATE.lock().owner.clone();
    if let Some(owner) = owner {
        app::emit_to(app, &owner, EVENT_ACTION, &payload);
    }
    if close_after {
        close(app);
    }
}
