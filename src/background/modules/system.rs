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
        perf::{PerfSampler, PerfStatus},
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
    /// CPU, memory and network speed (toolbar items)
    pub perf: Option<PerfStatus>,
}

static STATE: LazyLock<RwLock<SystemState>> = LazyLock::new(|| RwLock::new(SystemState::default()));

pub fn get() -> SystemState {
    STATE.read().clone()
}

/// What is shown right now: the modules of the main dock and of the auxiliary docks that
/// are on, plus what the toolbar's items need.
fn active_modules() -> HashSet<String> {
    let all = settings::get();
    let mut docks = vec![dock_items::get()];
    docks.extend(
        all.aux_docks
            .iter()
            .filter(|a| a.enabled)
            .map(|a| dock_items::get_for(Some(&a.id))),
    );
    let mut active: HashSet<String> = docks
        .iter()
        .flat_map(|d| d.all())
        .filter_map(|item| match item {
            DockItem::Module { module, .. } => Some(module.clone()),
            _ => None,
        })
        .collect();
    let toolbar = all.toolbar;
    if toolbar.enabled {
        let needs: [(&str, &[&str]); 8] = [
            ("battery", &["power"]),
            ("keyboard", &["keyboard"]),
            ("bluetooth", &["bluetooth"]),
            ("media", &["media"]),
            ("quick-settings", &["network", "volume"]),
            ("cpu", &["perf"]),
            ("memory", &["perf"]),
            ("net-speed", &["perf"]),
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
            let active = keyboard::active_input(apps::last_external_foreground());
            state.keyboard = keyboard::list(active);
        }
        "recycle-bin" => state.recycle_bin = Some(shell::recycle_bin_info()),
        _ => return,
    }
    app::emit(EVENT_SYSTEM_STATE, &*state);
}

/// Follows the input language (Win+Space, Alt+Shift, a language picked in the dock) and the
/// Chinese / English mode of an IME (Shift) closely: Windows sends no notification for
/// them to other applications, so the focused window's input is compared every 150 ms.
/// That is a few cheap calls; the list is only rebuilt and sent when something changed.
fn watch_keyboard() {
    crate::utils::spawn_supervised("keyboard-watch", || {
        const EVERY: Duration = Duration::from_millis(150);
        let mut last = keyboard::ActiveInput::default();
        let mut wanted = false;
        let mut tick: u32 = 0;
        loop {
            std::thread::sleep(EVERY);
            // whether a dock / the toolbar shows the keyboard, checked about once a second
            if tick.is_multiple_of(7) {
                wanted = active_modules().contains("keyboard");
            }
            tick = tick.wrapping_add(1);
            if !wanted {
                last = keyboard::ActiveInput::default();
                continue;
            }
            let active = keyboard::active_input(apps::last_external_foreground());
            if active != last {
                last = active;
                refresh_now("keyboard");
            }
        }
    });
}

pub fn start() {
    watch_keyboard();
    crate::utils::spawn_supervised("system-poller", || {
        let _com = crate::windows_api::ComGuard::new();
        let mut media_manager = None;
        let mut audio_devices = None;
        let mut perf = PerfSampler::default();
        let mut tick: u64 = 0;
        loop {
            let modules = active_modules();
            let mut next = STATE.read().clone();

            if modules.contains("power") && tick.is_multiple_of(5) {
                next.power = Some(power::status());
            }
            // the active layout is followed by `watch_keyboard`; this only picks up
            // languages added or removed in the Windows settings
            if modules.contains("keyboard") && tick.is_multiple_of(5) {
                let active = keyboard::active_input(apps::last_external_foreground());
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
            if modules.contains("perf") {
                next.perf = Some(perf.sample());
            } else if next.perf.is_some() {
                // rates need two fresh samples when the items come back
                next.perf = None;
                perf = PerfSampler::default();
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
