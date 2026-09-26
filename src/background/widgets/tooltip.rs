//! Tooltip window: never takes focus and lets clicks through.

use std::sync::LazyLock;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use super::{
    WidgetWindow, hwnd_of, native,
    popup::{AnchorRect, Placement},
    set_rounded_corners, set_tool_window,
};
use crate::{
    app,
    error::Result,
    windows_api::{monitor, window::Rect},
};

pub const LABEL: &str = "tooltip";
pub const EVENT_RENDER: &str = "tooltip-render";
const GAP: f64 = 10.0;

#[derive(Default)]
struct TooltipState {
    token: u64,
    anchor: Rect,
    placement: Option<Placement>,
}

static STATE: LazyLock<Mutex<TooltipState>> = LazyLock::new(|| Mutex::new(TooltipState::default()));

#[derive(Serialize, Clone)]
struct RenderPayload {
    token: u64,
    text: String,
}

pub fn create(app: &AppHandle) -> Result<()> {
    if app.get_webview_window(LABEL).is_some() {
        return Ok(());
    }
    let window = WidgetWindow {
        label: LABEL,
        widget: "tooltip",
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
    .inner_size(120.0, 32.0)
    .build()?;
    set_tool_window(&window, true)?;
    set_rounded_corners(&window);
    native::set_click_through(hwnd_of(&window)?, true);
    window.on_window_event(|event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
        }
    });
    Ok(())
}

pub fn show(
    app: &AppHandle,
    caller: &WebviewWindow,
    text: String,
    anchor: AnchorRect,
    placement: Placement,
) -> Result<()> {
    let scale = caller.scale_factor()?;
    let origin = caller.inner_position()?;
    let rect = Rect {
        left: origin.x + (anchor.x * scale).round() as i32,
        top: origin.y + (anchor.y * scale).round() as i32,
        right: origin.x + ((anchor.x + anchor.width) * scale).round() as i32,
        bottom: origin.y + ((anchor.y + anchor.height) * scale).round() as i32,
    };
    let token = {
        let mut state = STATE.lock();
        state.token += 1;
        state.anchor = rect;
        state.placement = Some(placement);
        state.token
    };
    app::emit_to(app, LABEL, EVENT_RENDER, &RenderPayload { token, text });
    Ok(())
}

pub fn ready(app: &AppHandle, token: u64, width: f64, height: f64) -> Result<()> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or("tooltip window missing")?;
    let (anchor, placement) = {
        let state = STATE.lock();
        if state.token != token {
            return Ok(());
        }
        (state.anchor, state.placement.unwrap_or(Placement::Right))
    };
    let mon = monitor::from_point(
        anchor.left + anchor.width() / 2,
        anchor.top + anchor.height() / 2,
    )
    .ok_or("no monitor")?;
    let scale = mon.scale_factor;
    let w = (width * scale).ceil() as i32;
    let h = (height * scale).ceil() as i32;
    let gap = (GAP * scale) as i32;
    let (mut x, mut y) = match placement {
        Placement::Right => (anchor.right + gap, anchor.top + anchor.height() / 2 - h / 2),
        Placement::Left => (
            anchor.left - gap - w,
            anchor.top + anchor.height() / 2 - h / 2,
        ),
        Placement::Above => (
            anchor.left + anchor.width() / 2 - w / 2,
            anchor.top - gap - h,
        ),
        Placement::Below => (
            anchor.left + anchor.width() / 2 - w / 2,
            anchor.bottom + gap,
        ),
    };
    let area = mon.rect;
    x = x.clamp(area.left, (area.right - w).max(area.left));
    y = y.clamp(area.top, (area.bottom - h).max(area.top));

    window.set_size(PhysicalSize::new(w as u32, h as u32))?;
    window.set_position(PhysicalPosition::new(x, y))?;
    native::show_passive(hwnd_of(&window)?);
    Ok(())
}

pub fn hide(app: &AppHandle) {
    STATE.lock().token += 1;
    if let Some(window) = app.get_webview_window(LABEL)
        && let Ok(hwnd) = hwnd_of(&window)
    {
        native::hide(hwnd);
    }
}
