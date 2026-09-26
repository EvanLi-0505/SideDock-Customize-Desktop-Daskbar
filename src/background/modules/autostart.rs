//! Start with Windows through the per-user `Run` registry key.
//!
//! The registry is used instead of a Startup-folder shortcut so that nothing is
//! written to the user profile on disk. Since SideDock is portable, the stored path
//! is refreshed on every start in case the folder was moved.

use winreg::{RegKey, enums::HKEY_CURRENT_USER};

use crate::{error::Result, paths};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "SideDock";
pub const AUTOSTART_ARG: &str = "--autostart";

fn command_line() -> String {
    format!("\"{}\" {AUTOSTART_ARG}", paths::get().exe.display())
}

pub fn is_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|key| key.get_value::<String, _>(VALUE_NAME))
        .is_ok()
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;
    if enabled {
        key.set_value(VALUE_NAME, &command_line())?;
        log::info!("autostart enabled");
    } else {
        match key.delete_value(VALUE_NAME) {
            Ok(()) => log::info!("autostart disabled"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Rewrites the registry entry if the executable moved since it was created.
pub fn repair() {
    let current = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|key| key.get_value::<String, _>(VALUE_NAME));
    if let Ok(value) = current
        && value != command_line()
    {
        log::info!("autostart path changed, updating registry");
        let _ = set_enabled(true);
    }
}
