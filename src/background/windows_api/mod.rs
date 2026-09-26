//! Thin, safe-ish wrappers over the Win32/WinRT APIs used by SideDock.
//! Nothing in here knows about Tauri or about settings.

pub mod app_bar;
pub mod event_hook;
pub mod icons;
pub mod keyboard;
pub mod media;
pub mod monitor;
pub mod power;
pub mod radios;
pub mod shell;
pub mod window;

use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use windows::Win32::{
    Globalization::GetUserDefaultUILanguage,
    System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
};

use crate::state::settings::Language;

/// Null terminated UTF-16 string.
pub fn to_wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

pub fn system_language() -> Language {
    // primary language id is the low 10 bits; LANG_CHINESE = 0x04
    let lang = unsafe { GetUserDefaultUILanguage() };
    if lang & 0x3ff == 0x04 {
        Language::ZhCn
    } else {
        Language::En
    }
}

/// Initializes COM (STA) for the current thread for as long as the guard lives.
pub struct ComGuard {
    owned: bool,
}

impl ComGuard {
    pub fn new() -> Self {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Self { owned: hr.is_ok() }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.owned {
            unsafe { CoUninitialize() };
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemColors {
    /// Windows accent color as `#rrggbb`
    pub accent: String,
    /// "Choose your app mode" is set to dark
    pub dark_mode: bool,
}

pub fn system_colors() -> SystemColors {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let accent = hkcu
        .open_subkey(r"Software\Microsoft\Windows\DWM")
        .and_then(|k| k.get_value::<u32, _>("AccentColor"))
        .map(|abgr| {
            let (r, g, b) = (abgr & 0xff, (abgr >> 8) & 0xff, (abgr >> 16) & 0xff);
            format!("#{r:02x}{g:02x}{b:02x}")
        })
        .unwrap_or_else(|_| "#3b82f6".to_string());
    let dark_mode = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|k| k.get_value::<u32, _>("AppsUseLightTheme"))
        .map(|v| v == 0)
        .unwrap_or(false);
    SystemColors { accent, dark_mode }
}
