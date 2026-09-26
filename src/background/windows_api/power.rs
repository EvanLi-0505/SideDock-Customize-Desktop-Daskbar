use std::os::windows::process::CommandExt;

use serde::{Deserialize, Serialize};
use windows::Win32::System::{
    Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS, SetSuspendState},
    Shutdown::{EWX_LOGOFF, ExitWindowsEx, LockWorkStation, SHTDN_REASON_NONE},
};

use crate::error::Result;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerStatus {
    pub has_battery: bool,
    pub percentage: u8,
    pub charging: bool,
    pub plugged_in: bool,
    pub saver: bool,
}

pub fn status() -> PowerStatus {
    let mut s = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut s) }.is_err() {
        return PowerStatus {
            has_battery: false,
            percentage: 100,
            charging: false,
            plugged_in: true,
            saver: false,
        };
    }
    let has_battery = s.BatteryFlag != 128 && s.BatteryFlag != 255;
    PowerStatus {
        has_battery,
        percentage: if s.BatteryLifePercent > 100 {
            100
        } else {
            s.BatteryLifePercent
        },
        charging: s.BatteryFlag & 8 != 0,
        plugged_in: s.ACLineStatus == 1,
        saver: s.SystemStatusFlag == 1,
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PowerAction {
    Lock,
    SignOut,
    Sleep,
    Hibernate,
    Restart,
    Shutdown,
}

pub fn perform(action: PowerAction) -> Result<()> {
    log::info!("power action requested: {action:?}");
    match action {
        PowerAction::Lock => unsafe { LockWorkStation()? },
        PowerAction::SignOut => unsafe { ExitWindowsEx(EWX_LOGOFF, SHTDN_REASON_NONE)? },
        PowerAction::Sleep => unsafe {
            let _ = SetSuspendState(false, false, false);
        },
        PowerAction::Hibernate => unsafe {
            let _ = SetSuspendState(true, false, false);
        },
        PowerAction::Restart | PowerAction::Shutdown => {
            let flag = if matches!(action, PowerAction::Restart) {
                "/r"
            } else {
                "/s"
            };
            std::process::Command::new("shutdown.exe")
                .args([flag, "/t", "0"])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()?;
        }
    }
    Ok(())
}
