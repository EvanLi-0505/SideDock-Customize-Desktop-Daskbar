//! What the toolbar's custom buttons run (Settings > Toolbar > Custom buttons):
//! a program / file / folder / web address, a command (cmd or PowerShell), or a jump to
//! a key in Registry Editor.
//!
//! The page only says which button was clicked; everything below reads the button from
//! the saved settings.

use std::path::{Path, PathBuf};

use base64::Engine;
use windows::{
    Win32::{
        Globalization::GetUserDefaultUILanguage,
        UI::WindowsAndMessaging::{FindWindowW, SW_HIDE, SW_SHOWNORMAL, SetForegroundWindow},
    },
    core::w,
};

use crate::{
    error::Result,
    paths,
    state::settings::{ButtonKind, CommandShell, CustomButton, RunMode},
    windows_api::shell,
};

/// Error codes the page turns into messages.
pub const ERR_REGEDIT_OPEN: &str = "regedit-open";
pub const ERR_NOT_REGISTRY_PATH: &str = "not-a-registry-path";

/// Where Registry Editor keeps the key it shows when it opens (its own setting).
const REGEDIT_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Applets\Regedit";

pub fn run(button: &CustomButton) -> Result<()> {
    match button.kind {
        ButtonKind::Open => open(button),
        ButtonKind::Command => command(button),
        ButtonKind::Registry => registry(&button.target),
    }
}

// ---------------- program / file / folder / address ----------------

fn open(button: &CustomButton) -> Result<()> {
    let target = button.target.trim();
    if button.is_url() {
        return shell::open_uri(target);
    }
    let path = PathBuf::from(target);
    // programs start in their own folder, like from a shortcut
    let dir = path
        .is_file()
        .then(|| path.parent().map(PathBuf::from))
        .flatten();
    let args = Some(button.args.trim()).filter(|a| !a.is_empty());
    shell::launch(target, args, dir.as_deref(), false)
}

// ---------------- commands ----------------

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

/// Several lines of cmd go into a batch file in SideDock's data folder (portable),
/// switched to UTF-8 so paths and text in any language survive.
fn batch_file(button: &CustomButton, text: &str) -> Result<PathBuf> {
    let dir = paths::get().runtime.join("commands");
    std::fs::create_dir_all(&dir)?;
    let name: String = button
        .id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let file = dir.join(format!("{name}.cmd"));
    let body = text.lines().collect::<Vec<_>>().join("\r\n");
    std::fs::write(&file, format!("@echo off\r\nchcp 65001 >nul\r\n{body}\r\n"))?;
    Ok(file)
}

fn command(button: &CustomButton) -> Result<()> {
    let text = button.command.trim();
    if text.is_empty() {
        return Err("empty command".into());
    }
    let show = if button.run_mode == RunMode::Hidden {
        SW_HIDE
    } else {
        SW_SHOWNORMAL
    };
    let (program, params) = match button.shell {
        CommandShell::Cmd => {
            let flag = if button.run_mode == RunMode::Keep {
                "/k"
            } else {
                "/c"
            };
            let body = if text.contains('\n') {
                format!("\"{}\"", batch_file(button, text)?.display())
            } else {
                text.to_string()
            };
            // /s: cmd removes only the outer quotes and keeps the command as written
            ("cmd.exe", format!("/s {flag} \"{body}\""))
        }
        CommandShell::PowerShell => {
            // -EncodedCommand carries any text, several lines and quotes included
            let utf16: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
            let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
            let mut params = String::from("-NoLogo -ExecutionPolicy Bypass");
            match button.run_mode {
                RunMode::Keep => params.push_str(" -NoExit"),
                RunMode::Hidden => params.push_str(" -WindowStyle Hidden"),
                RunMode::Close => {}
            }
            params.push_str(" -EncodedCommand ");
            params.push_str(&encoded);
            ("powershell.exe", params)
        }
    };
    shell::run(
        program,
        Some(&params),
        home().as_deref(),
        button.elevated,
        show,
    )
}

// ---------------- Registry Editor ----------------

const HIVES: &[(&str, &str)] = &[
    ("HKEY_CLASSES_ROOT", "HKEY_CLASSES_ROOT"),
    ("HKCR", "HKEY_CLASSES_ROOT"),
    ("HKEY_CURRENT_USER", "HKEY_CURRENT_USER"),
    ("HKCU", "HKEY_CURRENT_USER"),
    ("HKEY_LOCAL_MACHINE", "HKEY_LOCAL_MACHINE"),
    ("HKLM", "HKEY_LOCAL_MACHINE"),
    ("HKEY_USERS", "HKEY_USERS"),
    ("HKU", "HKEY_USERS"),
    ("HKEY_CURRENT_CONFIG", "HKEY_CURRENT_CONFIG"),
    ("HKCC", "HKEY_CURRENT_CONFIG"),
];

fn hive(name: &str) -> Option<&'static str> {
    HIVES
        .iter()
        .find(|(alias, _)| alias.eq_ignore_ascii_case(name))
        .map(|(_, full)| *full)
}

/// `计算机\HKEY_LOCAL_MACHINE\...`, `HKEY_LOCAL_MACHINE\...`, `HKLM/...` ->
/// `HKEY_LOCAL_MACHINE\...`; `None` when it does not name a key.
pub fn normalize_registry_path(input: &str) -> Option<String> {
    let cleaned = input.trim().trim_matches('"').replace('/', "\\");
    let parts: Vec<&str> = cleaned
        .split('\\')
        .filter(|s| !s.trim().is_empty())
        .collect();
    // the root shown in Registry Editor's address bar ("计算机", "Computer") is optional
    let start = parts.iter().position(|p| hive(p).is_some())?;
    if start > 1 {
        return None;
    }
    let mut path = vec![hive(parts[start])?];
    path.extend(&parts[start + 1..]);
    Some(path.join("\\"))
}

/// Name of the root of Registry Editor's tree, which its saved position starts with: taken
/// from the position it saved last, else from the display language.
fn regedit_root() -> String {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let saved: Option<String> = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(REGEDIT_KEY)
        .and_then(|k| k.get_value("LastKey"))
        .ok();
    if let Some(root) = saved
        .as_deref()
        .and_then(|last| last.split('\\').next())
        .filter(|root| !root.is_empty() && hive(root).is_none())
    {
        return root.to_string();
    }
    let language = unsafe { GetUserDefaultUILanguage() };
    match language {
        // Chinese: traditional (Taiwan, Hong Kong, Macao) or simplified
        0x0404 | 0x0C04 | 0x1404 => "電腦".into(),
        l if l & 0x3FF == 0x04 => "计算机".into(),
        _ => "Computer".into(),
    }
}

fn registry(target: &str) -> Result<()> {
    let path = normalize_registry_path(target).ok_or(ERR_NOT_REGISTRY_PATH)?;
    // An open Registry Editor only comes to the front: it would not move, and would
    // overwrite the saved position when it closes. It runs elevated, so SideDock cannot
    // close it; the page asks the user to.
    if let Ok(open) = unsafe { FindWindowW(w!("RegEdit_RegEdit"), None) } {
        unsafe {
            let _ = SetForegroundWindow(open);
        }
        return Err(ERR_REGEDIT_OPEN.into());
    }
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(REGEDIT_KEY)?;
    key.set_value("LastKey", &format!("{}\\{path}", regedit_root()))?;
    // its manifest asks for administrator rights: the shell shows the UAC prompt
    shell::launch("regedit.exe", None, None::<&Path>, false)
}

#[cfg(test)]
mod tests {
    use super::normalize_registry_path as n;

    #[test]
    fn registry_paths() {
        assert_eq!(
            n(r"计算机\HKEY_CURRENT_USER\Control Panel\Desktop").as_deref(),
            Some(r"HKEY_CURRENT_USER\Control Panel\Desktop")
        );
        assert_eq!(
            n("HKLM/SOFTWARE/Microsoft/").as_deref(),
            Some(r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft")
        );
        assert_eq!(
            n(r#""hkcu\Software""#).as_deref(),
            Some(r"HKEY_CURRENT_USER\Software")
        );
        assert_eq!(n("HKEY_USERS").as_deref(), Some("HKEY_USERS"));
        assert_eq!(n(r"C:\Windows\System32"), None);
        assert_eq!(n(r"a\b\HKLM\x"), None);
    }
}
