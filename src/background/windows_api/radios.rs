use windows::Devices::Radios::{Radio, RadioKind, RadioState};

use crate::error::Result;

fn bluetooth_radio() -> Result<Option<Radio>> {
    let radios = Radio::GetRadiosAsync()?.join()?;
    for radio in radios {
        if radio.Kind()? == RadioKind::Bluetooth {
            return Ok(Some(radio));
        }
    }
    Ok(None)
}

/// `None` when the machine has no bluetooth adapter.
pub fn bluetooth_enabled() -> Result<Option<bool>> {
    Ok(match bluetooth_radio()? {
        Some(radio) => Some(radio.State()? == RadioState::On),
        None => None,
    })
}

pub fn set_bluetooth(enabled: bool) -> Result<()> {
    let _ = Radio::RequestAccessAsync()?.join()?;
    let radio = bluetooth_radio()?.ok_or("no bluetooth adapter found")?;
    let state = if enabled {
        RadioState::On
    } else {
        RadioState::Off
    };
    radio.SetStateAsync(state)?.join()?;
    Ok(())
}
