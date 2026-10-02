//! Widgets are webview windows. Every widget is a Svelte app under `src/ui/apps/<name>`.
//!
//! To add a widget (e.g. a future window switcher): create the Svelte app, add a module
//! here with its window options and wire its commands in `commands.rs`.
//!
//! IMPORTANT: the dock, popup and tooltip windows are shown/hidden and made click-through
//! only through [`native`]. Tauri's own `show`/`hide`/`set_ignore_cursor_events` rewrite
//! the whole window style from tao's cached flags, which silently hides a window that was
//! shown natively and strips `WS_EX_TOOLWINDOW`. Never mix the two on those windows.

pub mod dock;
pub mod overlay;
pub mod popup;
pub mod settings;
pub mod tooltip;

use serde::Serialize;
use tauri::{AppHandle, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use windows::Win32::{
    Foundation::HWND,
    Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute},
    UI::WindowsAndMessaging::{WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW},
};

use crate::{error::Result, paths, state::settings::ThemeMode};

/// Data injected into the page before any script runs (`window.__SIDEDOCK__`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetBootstrap {
    pub label: String,
    pub widget: String,
    pub monitor_id: Option<isize>,
}

pub struct WidgetWindow<'a> {
    pub label: &'a str,
    pub widget: &'a str,
    pub monitor_id: Option<isize>,
}

impl WidgetWindow<'_> {
    /// Common builder: portable WebView2 profile, hidden until the page says it is ready.
    pub fn builder<'b>(
        &self,
        app: &'b AppHandle,
    ) -> Result<WebviewWindowBuilder<'b, tauri::Wry, AppHandle>> {
        let bootstrap = WidgetBootstrap {
            label: self.label.to_string(),
            widget: self.widget.to_string(),
            monitor_id: self.monitor_id,
        };
        let script = format!(
            "window.__SIDEDOCK__ = Object.freeze({});",
            serde_json::to_string(&bootstrap)?
        );
        let url = WebviewUrl::App(format!("apps/{}/index.html", self.widget).into());
        Ok(WebviewWindowBuilder::new(app, self.label, url)
            .data_directory(paths::get().webview_data_dir())
            .initialization_script(script)
            .title(format!("SideDock {}", self.widget))
            .visible(false))
    }
}

pub fn hwnd_of(window: &WebviewWindow) -> Result<isize> {
    Ok(window.hwnd()?.0 as isize)
}

/// Hides the window from Alt+Tab and optionally prevents it from taking focus.
pub fn set_tool_window(window: &WebviewWindow, no_activate: bool) -> Result<()> {
    let hwnd = hwnd_of(window)?;
    native::update_ex_style(hwnd, |ex| {
        let mut ex = (ex | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0;
        if no_activate {
            ex |= WS_EX_NOACTIVATE.0;
        }
        ex
    });
    Ok(())
}

/// Win32 window operations for the natively managed widget windows (see module docs).
pub mod native {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, IsWindowVisible, SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE,
            SetForegroundWindow, SetWindowLongPtrW, ShowWindow, WS_EX_LAYERED, WS_EX_TRANSPARENT,
        },
    };

    fn hwnd(h: isize) -> HWND {
        HWND(h as *mut _)
    }

    pub fn update_ex_style(h: isize, f: impl FnOnce(u32) -> u32) {
        unsafe {
            let ex = GetWindowLongPtrW(hwnd(h), GWL_EXSTYLE) as u32;
            let next = f(ex);
            if next != ex {
                SetWindowLongPtrW(hwnd(h), GWL_EXSTYLE, next as isize);
            }
        }
    }

    pub fn is_visible(h: isize) -> bool {
        unsafe { IsWindowVisible(hwnd(h)).as_bool() }
    }

    /// Shows without stealing focus from the active application.
    pub fn show_passive(h: isize) {
        unsafe {
            let _ = ShowWindow(hwnd(h), SW_SHOWNOACTIVATE);
        }
    }

    /// Shows and brings to the foreground (popups that need keyboard focus / blur events).
    pub fn show_active(h: isize) {
        unsafe {
            let _ = ShowWindow(hwnd(h), SW_SHOW);
            let _ = SetForegroundWindow(hwnd(h));
        }
    }

    pub fn hide(h: isize) {
        unsafe {
            let _ = ShowWindow(hwnd(h), SW_HIDE);
        }
    }

    /// Mouse events go to whatever is below the window.
    pub fn set_click_through(h: isize, enabled: bool) {
        update_ex_style(h, |ex| {
            if enabled {
                ex | WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0
            } else {
                // same pair tao toggles for `set_ignore_cursor_events`
                ex & !(WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0)
            }
        });
    }
}

/// Windows 11 rounded corners (no-op on Windows 10).
pub fn set_rounded_corners(window: &WebviewWindow) {
    if let Ok(hwnd) = window.hwnd() {
        let pref = DWMWCP_ROUND;
        unsafe {
            let _ = DwmSetWindowAttribute(
                HWND(hwnd.0),
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &pref as *const _ as *const _,
                std::mem::size_of_val(&pref) as u32,
            );
        }
    }
}

/// Native backdrop for the glass themes.
/// Frosted glass: Mica on large windows, acrylic on popups. Clear glass: acrylic
/// everywhere (see-through blur) with a lighter tint where Windows honours it.
pub fn apply_backdrop(window: &WebviewWindow, theme: ThemeMode, large_window: bool) {
    use tauri::window::{Color, Effect, EffectsBuilder};
    let res = match theme {
        ThemeMode::Glass => {
            let effect = if large_window {
                Effect::Mica
            } else {
                Effect::Acrylic
            };
            window.set_effects(EffectsBuilder::new().effect(effect).build())
        }
        ThemeMode::Clear => {
            let tint = if crate::windows_api::system_colors().dark_mode {
                Color(18, 20, 24, 40)
            } else {
                Color(255, 255, 255, 30)
            };
            window.set_effects(
                EffectsBuilder::new()
                    .effect(Effect::Acrylic)
                    .color(tint)
                    .build(),
            )
        }
        ThemeMode::Dark | ThemeMode::Light | ThemeMode::System => window.set_effects(None),
    };
    if let Err(err) = res {
        log::debug!("backdrop not applied: {err}");
    }
}
