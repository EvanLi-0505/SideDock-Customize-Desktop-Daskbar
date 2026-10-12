//! Polls the small pieces of system state shown by the dock's modules and the toolbar's
//! items (battery, keyboard layout, recycle bin, media, bluetooth, network, volume).
//! Polling is deliberately simple: each value is cheap to query and only what is shown
//! somewhere is refreshed.

use std::{collections::HashSet, sync::LazyLock, time::Duration};

use parking_lot::RwLock;
use serde::Serialize;

use crate::{
    app,
    modules::apps,
    state::{
        dock_items::{self, DockItem},
        settings,
    },
    windows_api::{
        audio::{self, VolumeStatus},
        keyboard::{self, KeyboardLayout},
        media::{self, MediaInfo},
        network::{self, NetworkStatus},
        power::{self, PowerStatus},
        radios,
        shell::{self, RecycleBinInfo},
    },
};

pub const EVENT_SYSTEM_STATE: &str = "system-state-changed";

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemState {
    pub power: Option<PowerStatus>,
    pub keyboard: Vec<KeyboardLayout>,
    pub recycle_bin: Option<RecycleBinInfo>,
    pub media: Option<MediaInfo>,
    /// `None` = no adapter
    pub bluetooth: Option<bool>,
    pub network: Option<NetworkStatus>,
    /// `None` = no playback device
    pub volume: Option<VolumeStatus>,
}

static STATE: LazyLock<RwLock<SystemState>> = LazyLock::new(|| RwLock::new(SystemState::default()));

pub fn get() -> SystemState {
    STATE.read().clone()
}

/// What is shown right now: the dock's modules plus what the toolbar's items need.
fn active_modules() -> HashSet<String> {
    let mut active: HashSet<String> = dock_items::get()
        .all()
        .filter_map(|item| match item {
            DockItem::Module { module, .. } => Some(module.clone()),
            _ => None,
        })
        .collect();
    let toolbar = settings::get().toolbar;
    if toolbar.enabled {
        let needs: [(&str, &[&str]); 5] = [
            ("battery", &["power"]),
            ("keyboard", &["keyboard"]),
            ("bluetooth", &["bluetooth"]),
            ("media", &["media"]),
            ("quick-settings", &["network", "volume"]),
        ];
        for (item, modules) in needs {
            if toolbar.items.contains(item) {
                active.extend(modules.iter().map(|m| m.to_string()));
            }
        }
    }
    active
}

/// Publishes a volume just set from here, without waiting for the next poll.
pub fn set_volume(volume: VolumeStatus) {
    let mut state = STATE.write();
    if state.volume != Some(volume) {
        state.volume = Some(volume);
        app::emit(EVENT_SYSTEM_STATE, &*state);
    }
}

/// Forces a refresh of one value right away (e.g. after toggling bluetooth).
pub fn refresh_now(what: &str) {
    let mut state = STATE.write();
    match what {
        "bluetooth" => state.bluetooth = radios::bluetooth_enabled().ok().flatten(),
        "keyboard" => {
            let active = keyboard::active_layout_for(apps::last_external_foreground());
            state.keyboard = keyboard::list(active);
        }
        "recycle-bin" => state.recycle_bin = Some(shell::recycle_bin_info()),
        _ => return,
    }
    app::emit(EVENT_SYSTEM_STATE, &*state);
}

pub fn start() {
    crate::utils::spawn_supervised("system-poller", || {
        let _com = crate::windows_api::ComGuard::new();
        let mut media_manager = None;
        let mut audio_devices = None;
        let mut tick: u64 = 0;
        loop {
            let modules = active_modules();
            let mut next = STATE.read().clone();

            if modules.contains("power") && tick.is_multiple_of(5) {
                next.power = Some(power::status());
            }
            if modules.contains("keyboard") {
                let active = keyboard::active_layout_for(apps::last_external_foreground());
                next.keyboard = keyboard::list(active);
            }
            if modules.contains("recycle-bin") && tick.is_multiple_of(3) {
                next.recycle_bin = Some(shell::recycle_bin_info());
            }
            if modules.contains("bluetooth") && tick.is_multiple_of(5) {
                next.bluetooth = radios::bluetooth_enabled().ok().flatten();
            }
            if modules.contains("media") {
                if media_manager.is_none() {
                    media_manager = media::manager().ok();
                }
                if let Some(manager) = &media_manager {
                    let previous = next.media.clone();
                    next.media = media::current(manager, previous.as_ref()).ok().flatten();
                }
            } else {
                next.media = None;
            }
            if modules.contains("network") && tick.is_multiple_of(3) {
                next.network = Some(network::status());
            }
            if modules.contains("volume") {
                if audio_devices.is_none() {
                    audio_devices = audio::enumerator().ok();
                }
                next.volume = audio_devices.as_ref().and_then(audio::status);
            }

            let changed = {
                let mut guard = STATE.write();
                let changed = *guard != next;
                *guard = next;
                changed
            };
            if changed {
                app::emit(EVENT_SYSTEM_STATE, &*STATE.read());
            }

            tick = tick.wrapping_add(1);
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}
