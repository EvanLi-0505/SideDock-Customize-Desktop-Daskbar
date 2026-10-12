//! Master volume of the default playback device, for the volume status icon.
//!
//! Every function needs COM on the calling thread (`ComGuard`).

use serde::Serialize;
use windows::Win32::{
    Media::Audio::{
        Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator, MMDeviceEnumerator, eMultimedia,
        eRender,
    },
    System::Com::{CLSCTX_ALL, CoCreateInstance},
};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeStatus {
    /// 0-100
    pub level: u32,
    pub muted: bool,
}

/// Kept by the caller between reads; the default device is looked up each time (it
/// changes when headphones are plugged in).
pub fn enumerator() -> Result<IMMDeviceEnumerator> {
    Ok(unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?)
}

fn endpoint(enumerator: &IMMDeviceEnumerator) -> Result<IAudioEndpointVolume> {
    unsafe {
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
        Ok(device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)?)
    }
}

fn read(endpoint: &IAudioEndpointVolume) -> Result<VolumeStatus> {
    unsafe {
        let level = endpoint.GetMasterVolumeLevelScalar()?;
        Ok(VolumeStatus {
            level: (level * 100.0).round().clamp(0.0, 100.0) as u32,
            muted: endpoint.GetMute()?.as_bool(),
        })
    }
}

/// `None` when there is no playback device.
pub fn status(enumerator: &IMMDeviceEnumerator) -> Option<VolumeStatus> {
    read(&endpoint(enumerator).ok()?).ok()
}

/// Changes the volume by `delta` percent (like turning the wheel over the taskbar's
/// volume icon); raising it also un-mutes.
pub fn step(delta: i32) -> Result<VolumeStatus> {
    let endpoint = endpoint(&enumerator()?)?;
    let current = read(&endpoint)?;
    let level = (current.level as i32 + delta).clamp(0, 100);
    unsafe {
        endpoint.SetMasterVolumeLevelScalar(level as f32 / 100.0, std::ptr::null())?;
        if delta > 0 && current.muted {
            endpoint.SetMute(false, std::ptr::null())?;
        }
    }
    read(&endpoint)
}

pub fn toggle_mute() -> Result<VolumeStatus> {
    let endpoint = endpoint(&enumerator()?)?;
    let current = read(&endpoint)?;
    unsafe { endpoint.SetMute(!current.muted, std::ptr::null())? };
    read(&endpoint)
}
