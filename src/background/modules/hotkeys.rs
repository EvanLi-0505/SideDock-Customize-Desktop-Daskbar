//! Global shortcuts registered with `RegisterHotKey` (no keyboard hook needed).
//!
//! Registration happens on a dedicated message-loop thread, because `WM_HOTKEY` is
//! delivered to the thread that registered the key. Other threads talk to it with
//! thread messages ([`reload`], [`suspend`]).
//!
//! Accelerators are stored as text, e.g. `Ctrl+Alt+Backquote`. The key names are shared
//! with the settings page (`src/ui/shared/shortcuts.ts`).

use std::{
    collections::BTreeMap,
    sync::{
        LazyLock,
        atomic::{AtomicU32, Ordering},
    },
};

use parking_lot::RwLock;
use serde::Serialize;
use tauri::AppHandle;
use windows::Win32::{
    Foundation::{GetLastError, LPARAM, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{
            HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
            RegisterHotKey, UnregisterHotKey,
        },
        WindowsAndMessaging::{
            GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_APP, WM_HOTKEY,
        },
    },
};

use crate::{app, state::settings};

pub const EVENT_STATUS: &str = "shortcut-status-changed";

const WM_RELOAD: u32 = WM_APP + 1;
const WM_SUSPEND: u32 = WM_APP + 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HotkeyStatus {
    Active,
    Disabled,
    /// the text could not be parsed or has no modifier
    Invalid,
    /// already used by Windows or another program
    Conflict,
    /// temporarily released while the user records a new shortcut
    Suspended,
}

static THREAD_ID: AtomicU32 = AtomicU32::new(0);
static STATUS: LazyLock<RwLock<BTreeMap<String, HotkeyStatus>>> =
    LazyLock::new(|| RwLock::new(BTreeMap::new()));

pub fn status() -> BTreeMap<String, HotkeyStatus> {
    STATUS.read().clone()
}

fn key_code(name: &str) -> Option<u32> {
    let bytes = name.as_bytes();
    if bytes.len() == 1 && (bytes[0].is_ascii_uppercase() || bytes[0].is_ascii_digit()) {
        return Some(bytes[0] as u32);
    }
    if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<u32>().ok())
        && (1..=24).contains(&n)
    {
        return Some(0x70 + n - 1);
    }
    if let Some(n) = name
        .strip_prefix("Numpad")
        .and_then(|n| n.parse::<u32>().ok())
        && n <= 9
    {
        return Some(0x60 + n);
    }
    Some(match name {
        "Space" => 0x20,
        "Tab" => 0x09,
        "Enter" => 0x0D,
        "Backspace" => 0x08,
        "Delete" => 0x2E,
        "Insert" => 0x2D,
        "Home" => 0x24,
        "End" => 0x23,
        "PageUp" => 0x21,
        "PageDown" => 0x22,
        "Left" => 0x25,
        "Up" => 0x26,
        "Right" => 0x27,
        "Down" => 0x28,
        "Backquote" => 0xC0,
        "Minus" => 0xBD,
        "Equal" => 0xBB,
        "BracketLeft" => 0xDB,
        "BracketRight" => 0xDD,
        "Backslash" => 0xDC,
        "Semicolon" => 0xBA,
        "Quote" => 0xDE,
        "Comma" => 0xBC,
        "Period" => 0xBE,
        "Slash" => 0xBF,
        _ => return None,
    })
}

/// Parses `Ctrl+Alt+Backquote` into modifiers + virtual key.
pub fn parse(accelerator: &str) -> Option<(HOT_KEY_MODIFIERS, u32)> {
    let mut mods = HOT_KEY_MODIFIERS(0);
    let mut key = None;
    for part in accelerator.split('+').map(str::trim) {
        match part {
            "Ctrl" => mods |= MOD_CONTROL,
            "Alt" => mods |= MOD_ALT,
            "Shift" => mods |= MOD_SHIFT,
            "Win" => mods |= MOD_WIN,
            other if key.is_none() => key = Some(key_code(other)?),
            _ => return None, // two non-modifier keys
        }
    }
    let key = key?;
    let is_function_key = (0x70..=0x87).contains(&key);
    let has_modifier = (mods.0 & (MOD_CONTROL.0 | MOD_ALT.0 | MOD_WIN.0)) != 0;
    (has_modifier || is_function_key).then_some((mods | MOD_NOREPEAT, key))
}

fn register_all(ids: &mut Vec<String>) {
    let shortcuts = settings::get().shortcuts;
    let mut status = BTreeMap::new();
    ids.clear();
    for (action, accelerator) in shortcuts {
        let state = if accelerator.is_empty() {
            HotkeyStatus::Disabled
        } else if let Some((mods, vk)) = parse(&accelerator) {
            let id = ids.len() as i32 + 1;
            if unsafe { RegisterHotKey(None, id, mods, vk) }.is_ok() {
                ids.push(action.clone());
                HotkeyStatus::Active
            } else {
                // taken by another program, or reserved by Windows (Win+L, Win+D...)
                let err = unsafe { GetLastError() };
                log::warn!("shortcut {accelerator} for {action} unavailable ({err:?})");
                HotkeyStatus::Conflict
            }
        } else {
            HotkeyStatus::Invalid
        };
        status.insert(action, state);
    }
    *STATUS.write() = status;
}

fn unregister_all(ids: &mut Vec<String>) {
    for id in 1..=ids.len() as i32 {
        unsafe {
            let _ = UnregisterHotKey(None, id);
        }
    }
    ids.clear();
}

fn post(message: u32, wparam: usize) {
    let thread = THREAD_ID.load(Ordering::Acquire);
    if thread != 0 {
        unsafe {
            let _ = PostThreadMessageW(thread, message, WPARAM(wparam), LPARAM(0));
        }
    }
}

/// Re-registers every shortcut from the current settings.
pub fn reload() {
    post(WM_RELOAD, 0);
}

/// Releases all shortcuts while the settings page records a new one, so the pressed
/// keys reach the page instead of triggering the action.
pub fn suspend(suspended: bool) {
    post(WM_SUSPEND, suspended as usize);
}

pub fn start(app: AppHandle) {
    crate::utils::spawn_supervised("hotkeys", move || {
        let mut msg = MSG::default();
        unsafe {
            // make sure the thread has a message queue before anyone posts to it
            let _ = PeekMessageW(&mut msg, None, WM_APP, WM_APP, PM_NOREMOVE);
            THREAD_ID.store(GetCurrentThreadId(), Ordering::Release);
        }
        let mut ids: Vec<String> = Vec::new();
        register_all(&mut ids);
        app::emit(EVENT_STATUS, &status());

        while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
            match msg.message {
                WM_HOTKEY => {
                    let index = msg.wParam.0.saturating_sub(1);
                    if let Some(action) = ids.get(index) {
                        log::info!("shortcut triggered: {action}");
                        app::run_shortcut(&app, action);
                    }
                }
                WM_RELOAD => {
                    unregister_all(&mut ids);
                    register_all(&mut ids);
                    app::emit(EVENT_STATUS, &status());
                }
                WM_SUSPEND => {
                    unregister_all(&mut ids);
                    if msg.wParam.0 == 1 {
                        let mut status = STATUS.write();
                        for state in status.values_mut() {
                            if *state == HotkeyStatus::Active {
                                *state = HotkeyStatus::Suspended;
                            }
                        }
                    } else {
                        register_all(&mut ids);
                    }
                    app::emit(EVENT_STATUS, &status());
                }
                _ => {}
            }
        }
        THREAD_ID.store(0, Ordering::Release);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_accelerators() {
        assert!(parse("Alt+Backquote").is_some());
        assert!(parse("Ctrl+Alt+Shift+D").is_some());
        assert!(parse("Win+Space").is_some());
        assert!(parse("F13").is_some());
        assert_eq!(parse("Alt+Backquote").unwrap().1, 0xC0);
    }

    #[test]
    fn rejects_invalid_accelerators() {
        assert!(parse("").is_none());
        assert!(parse("Shift+A").is_none()); // would break normal typing
        assert!(parse("A").is_none());
        assert!(parse("Ctrl+A+B").is_none());
        assert!(parse("Ctrl+Unknown").is_none());
        assert!(parse("Ctrl+Alt").is_none());
    }
}
