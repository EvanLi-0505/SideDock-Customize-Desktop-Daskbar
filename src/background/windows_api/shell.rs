use std::path::{Path, PathBuf};

use windows::{
    Win32::{
        Storage::{
            EnhancedStorage::PKEY_AppUserModel_ID,
            FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW},
        },
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ},
        UI::{
            Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
                SendInput, VIRTUAL_KEY, VK_D, VK_LWIN,
            },
            Shell::{
                IShellItem, IShellItem2, IShellLinkW, SHCreateItemFromParsingName,
                SHEmptyRecycleBinW, SHQUERYRBINFO, SHQueryRecycleBinW, SIGDN_NORMALDISPLAY,
                ShellExecuteW, ShellLink,
            },
            WindowsAndMessaging::{SHOW_WINDOW_CMD, SW_SHOWNORMAL},
        },
    },
    core::{HSTRING, PCWSTR},
};

use super::{ComGuard, from_wide, to_wide};
use crate::error::Result;

fn shell_execute(verb: &str, file: &str, params: Option<&str>, dir: Option<&Path>) -> Result<()> {
    let verb = to_wide(verb);
    let file_w = to_wide(file);
    let params_w = params.map(to_wide);
    let dir_w = dir.map(|d| to_wide(d.as_os_str()));
    let res = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file_w.as_ptr()),
            params_w
                .as_ref()
                .map_or(PCWSTR::null(), |p| PCWSTR(p.as_ptr())),
            dir_w
                .as_ref()
                .map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
            SHOW_WINDOW_CMD(SW_SHOWNORMAL.0),
        )
    };
    if res.0 as isize <= 32 {
        return Err(format!("failed to open {file} (code {})", res.0 as isize).into());
    }
    Ok(())
}

/// Launches a program, file, folder, URI or `shell:AppsFolder\<umid>` target.
pub fn launch(
    program: &str,
    args: Option<&str>,
    working_dir: Option<&Path>,
    elevated: bool,
) -> Result<()> {
    let _com = ComGuard::new();
    if program
        .to_ascii_lowercase()
        .starts_with("shell:appsfolder\\")
    {
        // packaged apps can only be activated through explorer
        return shell_execute("open", "explorer.exe", Some(program), None);
    }
    let dir = working_dir.filter(|d| d.is_dir());
    shell_execute(if elevated { "runas" } else { "open" }, program, args, dir)
}

pub fn open_uri(uri: &str) -> Result<()> {
    let _com = ComGuard::new();
    shell_execute("open", uri, None, None)
}

pub fn reveal_in_explorer(path: &Path) -> Result<()> {
    let _com = ComGuard::new();
    if path.is_dir() {
        return shell_execute(
            "open",
            "explorer.exe",
            Some(&format!("\"{}\"", path.display())),
            None,
        );
    }
    shell_execute(
        "open",
        "explorer.exe",
        Some(&format!("/select,\"{}\"", path.display())),
        None,
    )
}

fn key_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                ..Default::default()
            },
        },
    }
}

/// Presses the given keys in order and releases them in reverse order.
fn send_chord(keys: &[VIRTUAL_KEY]) {
    let mut inputs: Vec<INPUT> = keys.iter().map(|k| key_input(*k, false)).collect();
    inputs.extend(keys.iter().rev().map(|k| key_input(*k, true)));
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

pub fn toggle_start_menu() {
    send_chord(&[VK_LWIN]);
}

pub fn toggle_desktop() {
    send_chord(&[VK_LWIN, VK_D]);
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleBinInfo {
    pub items: i64,
    pub size: i64,
}

pub fn recycle_bin_info() -> RecycleBinInfo {
    let mut info = SHQUERYRBINFO {
        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    let ok = unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) }.is_ok();
    if ok {
        RecycleBinInfo {
            items: info.i64NumItems,
            size: info.i64Size,
        }
    } else {
        RecycleBinInfo { items: 0, size: 0 }
    }
}

/// Empties the recycle bin; the shell shows its own confirmation dialog.
pub fn empty_recycle_bin() -> Result<()> {
    let _com = ComGuard::new();
    unsafe { SHEmptyRecycleBinW(None, PCWSTR::null(), 0)? };
    Ok(())
}

pub struct LnkTarget {
    pub target: PathBuf,
    pub args: String,
    pub working_dir: PathBuf,
}

pub fn resolve_lnk(path: &Path) -> Result<LnkTarget> {
    let _com = ComGuard::new();
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let persist: IPersistFile = windows::core::Interface::cast(&link)?;
        persist.Load(&HSTRING::from(path.as_os_str()), STGM_READ)?;

        let mut target = [0u16; 1024];
        let _ = link.GetPath(&mut target, std::ptr::null_mut(), 0);
        let mut args = [0u16; 2048];
        let _ = link.GetArguments(&mut args);
        let mut dir = [0u16; 1024];
        let _ = link.GetWorkingDirectory(&mut dir);

        Ok(LnkTarget {
            target: PathBuf::from(from_wide(&target)),
            args: from_wide(&args),
            working_dir: PathBuf::from(from_wide(&dir)),
        })
    }
}

fn shell_item(parsing_name: &str) -> Result<IShellItem2> {
    let item: IShellItem2 =
        unsafe { SHCreateItemFromParsingName(&HSTRING::from(parsing_name), None)? };
    Ok(item)
}

pub fn shell_display_name(parsing_name: &str) -> Option<String> {
    let _com = ComGuard::new();
    let item = shell_item(parsing_name).ok()?;
    let item: IShellItem = windows::core::Interface::cast(&item).ok()?;
    unsafe {
        let name = item.GetDisplayName(SIGDN_NORMALDISPLAY).ok()?;
        let text = name.to_string().ok();
        windows::Win32::System::Com::CoTaskMemFree(Some(name.0 as *const _));
        text.filter(|t| !t.is_empty())
    }
}

/// AppUserModelID declared by a file (usually a `.lnk` created by an installer).
pub fn file_umid(path: &Path) -> Option<String> {
    let _com = ComGuard::new();
    let item = shell_item(&path.to_string_lossy()).ok()?;
    unsafe {
        let value = item.GetString(&PKEY_AppUserModel_ID).ok()?;
        let text = value.to_string().ok();
        windows::Win32::System::Com::CoTaskMemFree(Some(value.0 as *const _));
        text.filter(|t| !t.is_empty())
    }
}

/// `FileDescription` from the version resource of an executable, e.g. "Google Chrome".
pub fn exe_description(path: &Path) -> Option<String> {
    let wide = to_wide(path.as_os_str());
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(
            PCWSTR(wide.as_ptr()),
            None,
            size,
            data.as_mut_ptr() as *mut _,
        )
        .ok()?;

        let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let query = to_wide("\\VarFileInfo\\Translation");
        let mut lang_cp = "040904B0".to_string();
        if VerQueryValueW(
            data.as_ptr() as *const _,
            PCWSTR(query.as_ptr()),
            &mut ptr,
            &mut len,
        )
        .as_bool()
            && len >= 4
        {
            let pair = std::slice::from_raw_parts(ptr as *const u16, 2);
            lang_cp = format!("{:04x}{:04x}", pair[0], pair[1]);
        }
        let query = to_wide(format!("\\StringFileInfo\\{lang_cp}\\FileDescription"));
        if !VerQueryValueW(
            data.as_ptr() as *const _,
            PCWSTR(query.as_ptr()),
            &mut ptr,
            &mut len,
        )
        .as_bool()
            || len == 0
        {
            return None;
        }
        let text = from_wide(std::slice::from_raw_parts(ptr as *const u16, len as usize));
        let text = text.trim().to_string();
        (!text.is_empty()).then_some(text)
    }
}

/// Best effort human readable name for a file, shortcut or packaged app.
pub fn display_name_for(path: &Path, umid: Option<&str>) -> String {
    if let Some(umid) = umid
        && let Some(name) = shell_display_name(&format!("shell:AppsFolder\\{umid}"))
    {
        return name;
    }
    let is_exe = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
    if is_exe && let Some(desc) = exe_description(path) {
        return desc;
    }
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}
