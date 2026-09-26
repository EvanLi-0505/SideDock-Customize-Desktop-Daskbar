use serde::Serialize;
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    Globalization::{GetLocaleInfoEx, LCIDToLocaleName, LOCALE_SLOCALIZEDDISPLAYNAME},
    UI::{
        Input::KeyboardAndMouse::{GetKeyboardLayout, GetKeyboardLayoutList, HKL},
        WindowsAndMessaging::{PostMessageW, WM_INPUTLANGCHANGEREQUEST},
    },
};
use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};

use super::{from_wide, window::Window};
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardLayout {
    /// HKL value as a hex string (JS numbers can't hold every 64 bit handle)
    pub id: String,
    /// BCP-47 locale, e.g. `zh-CN`
    pub locale: String,
    /// Localized language name, e.g. `中文(简体，中国)`
    pub display_name: String,
    /// Layout / IME description from the registry when available
    pub layout_name: String,
    /// Two/three letter label shown on the dock
    pub short_label: String,
    pub active: bool,
}

fn locale_name(lang_id: u16) -> String {
    let mut buf = [0u16; 85];
    let len = unsafe { LCIDToLocaleName(lang_id as u32, Some(&mut buf), 0) };
    if len <= 0 {
        return format!("{lang_id:04x}");
    }
    from_wide(&buf)
}

fn localized_language(locale: &str) -> String {
    let wide = super::to_wide(locale);
    let mut buf = [0u16; 128];
    let len = unsafe {
        GetLocaleInfoEx(
            windows::core::PCWSTR(wide.as_ptr()),
            LOCALE_SLOCALIZEDDISPLAYNAME,
            Some(&mut buf),
        )
    };
    if len <= 0 {
        return locale.to_string();
    }
    from_wide(&buf)
}

fn layout_text(hkl: isize) -> String {
    // the keyboard layout id (KLID) of plain layouts is the low word zero-padded
    let device = ((hkl as usize >> 16) & 0xffff) as u16;
    let klid = if device & 0xf000 == 0xf000 {
        return String::new();
    } else {
        format!("{:08x}", device as u32)
    };
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(format!(
            "SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts\\{klid}"
        ))
        .and_then(|k| k.get_value::<String, _>("Layout Text"))
        .unwrap_or_default()
}

fn short_label(locale: &str) -> String {
    let lang = locale
        .split('-')
        .next()
        .unwrap_or(locale)
        .to_ascii_lowercase();
    match lang.as_str() {
        "zh" => "中".into(),
        "ja" => "あ".into(),
        "ko" => "한".into(),
        "ru" => "РУ".into(),
        other => other.to_ascii_uppercase(),
    }
}

pub fn active_layout_for(window: Option<Window>) -> isize {
    let thread = window.map(|w| w.thread_id()).unwrap_or(0);
    unsafe { GetKeyboardLayout(thread).0 as isize }
}

pub fn list(active: isize) -> Vec<KeyboardLayout> {
    let count = unsafe { GetKeyboardLayoutList(None) };
    if count <= 0 {
        return Vec::new();
    }
    let mut handles = vec![HKL::default(); count as usize];
    let count = unsafe { GetKeyboardLayoutList(Some(&mut handles)) };
    handles.truncate(count.max(0) as usize);

    handles
        .into_iter()
        .map(|hkl| {
            let raw = hkl.0 as isize;
            let lang_id = (raw as usize & 0xffff) as u16;
            let locale = locale_name(lang_id);
            KeyboardLayout {
                id: format!("{:x}", raw as usize),
                display_name: localized_language(&locale),
                layout_name: layout_text(raw),
                short_label: short_label(&locale),
                active: raw == active,
                locale,
            }
        })
        .collect()
}

/// Asks `target` to switch its input language.
pub fn activate(target: Window, id: &str) -> Result<()> {
    let raw = usize::from_str_radix(id, 16).map_err(|e| e.to_string())? as isize;
    unsafe {
        PostMessageW(
            Some(target.hwnd()),
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(raw),
        )?;
    }
    Ok(())
}
