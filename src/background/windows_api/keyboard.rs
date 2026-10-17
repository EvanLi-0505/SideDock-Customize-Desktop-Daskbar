use serde::Serialize;
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    Globalization::{GetLocaleInfoEx, LCIDToLocaleName, LOCALE_SLOCALIZEDDISPLAYNAME},
    UI::{
        Input::{
            Ime::ImmGetDefaultIMEWnd,
            KeyboardAndMouse::{GetKeyboardLayout, GetKeyboardLayoutList, HKL},
        },
        WindowsAndMessaging::{
            FindWindowExW, GUITHREADINFO, GetGUIThreadInfo, PostMessageW, SMTO_ABORTIFHUNG,
            SendMessageTimeoutW, WM_IME_CONTROL, WM_INPUTLANGCHANGEREQUEST,
        },
    },
};
use windows::core::w;
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
    /// Two/three letter label shown on the dock ("英" / "A" while a Chinese / Japanese /
    /// Korean IME types Latin letters)
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

fn short_label(locale: &str, latin: bool) -> String {
    let lang = locale
        .split('-')
        .next()
        .unwrap_or(locale)
        .to_ascii_lowercase();
    // an IME typing Latin letters: what the Windows taskbar shows too
    if latin {
        match lang.as_str() {
            "zh" => return "英".into(),
            "ja" | "ko" => return "A".into(),
            _ => {}
        }
    }
    match lang.as_str() {
        "zh" => "中".into(),
        "ja" => "あ".into(),
        "ko" => "한".into(),
        "ru" => "РУ".into(),
        other => other.to_ascii_uppercase(),
    }
}

/// The window that takes the keyboard input of `window`: its focused child (an edit box,
/// a browser's content view...), or for a Store app the core window inside its frame,
/// which lives in another thread than the frame itself.
fn input_window(window: Window) -> Window {
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetGUIThreadInfo(window.thread_id(), &mut info) }.is_ok()
        && !info.hwndFocus.is_invalid()
    {
        return Window::from(info.hwndFocus);
    }
    if window.class() == "ApplicationFrameWindow"
        && let Ok(core) = unsafe {
            FindWindowExW(Some(window.hwnd()), None, w!("Windows.UI.Core.CoreWindow"), None)
        }
        && !core.is_invalid()
    {
        return Window::from(core);
    }
    window
}

/// What the user types with right now: the input language, and whether its IME (Chinese,
/// Japanese, Korean) is in its Latin letters mode (Shift in Microsoft Pinyin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActiveInput {
    pub layout: isize,
    pub latin: bool,
}

// WM_IME_CONTROL commands and the conversion mode bit (imm.h)
const IMC_GETCONVERSIONMODE: usize = 0x0001;
const IMC_GETOPENSTATUS: usize = 0x0005;
const IME_CMODE_NATIVE: usize = 0x0001;

/// Chinese, Japanese and Korean input languages have an IME with a Latin letters mode.
fn has_ime_mode(layout: isize) -> bool {
    matches!(layout as usize & 0x3ff, 0x04 | 0x11 | 0x12)
}

/// Asks the IME of `window` whether it types Latin letters: closed, or not in its native
/// (Chinese / kana / hangul) conversion mode. `None` when it does not answer. The IME's
/// default window answers for its thread; a hung application costs 50 ms at most.
fn ime_latin_mode(window: Window) -> Option<bool> {
    let ime = unsafe { ImmGetDefaultIMEWnd(window.hwnd()) };
    if ime.is_invalid() {
        return None;
    }
    let ask = |command: usize| {
        let mut result = 0usize;
        let ok = unsafe {
            SendMessageTimeoutW(
                ime,
                WM_IME_CONTROL,
                WPARAM(command),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                50,
                Some(&mut result),
            )
        };
        (ok.0 != 0).then_some(result)
    };
    if ask(IMC_GETOPENSTATUS)? == 0 {
        return Some(true);
    }
    Some(ask(IMC_GETCONVERSIONMODE)? & IME_CMODE_NATIVE == 0)
}

/// The input of the window the user types in (`window` = the last foreground application
/// window). Read from the window that has the keyboard focus: the top-level window's
/// thread may not be the one that switched.
pub fn active_input(window: Option<Window>) -> ActiveInput {
    let input = window.map(input_window);
    let thread = input.map_or(0, Window::thread_id);
    let layout = unsafe { GetKeyboardLayout(thread).0 as isize };
    let latin = has_ime_mode(layout)
        && input.and_then(ime_latin_mode).unwrap_or(false);
    ActiveInput { layout, latin }
}

pub fn list(active: ActiveInput) -> Vec<KeyboardLayout> {
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
            let is_active = raw == active.layout;
            KeyboardLayout {
                id: format!("{:x}", raw as usize),
                display_name: localized_language(&locale),
                layout_name: layout_text(raw),
                short_label: short_label(&locale, is_active && active.latin),
                active: is_active,
                locale,
            }
        })
        .collect()
}

/// Switches the input language of `target`, the application the user was typing in.
///
/// Since Windows 8 the input language follows the foreground window: a request sent to a
/// window in the background is undone as soon as it comes back to the front. So `target`
/// gets the focus back first (SideDock's popup has it while the user picks a language,
/// which lets SideDock hand it over), then the request goes to the window that takes its
/// keyboard input.
pub fn activate(target: Window, id: &str) -> Result<()> {
    let raw = usize::from_str_radix(id, 16).map_err(|e| e.to_string())? as isize;
    if let Err(err) = target.bring_to_foreground(false) {
        log::warn!("keyboard layout: could not focus the target window: {err}");
    }
    let input = input_window(target);
    unsafe {
        PostMessageW(
            Some(input.hwnd()),
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(raw),
        )?;
    }
    Ok(())
}
