//! Polls the small pieces of system state shown by dock modules (battery, keyboard
//! layout, recycle bin, media, bluetooth). Polling is deliberately simple: each value
//! is cheap to query and only the modules present on the dock are refreshed.

use std::{collections::HashSet, sync::LazyLock, time::Duration};

use parking_lot::RwLock;
use serde::Serialize;

use crate::{
    app,
    modules::apps,
    state::dock_items::{self, DockItem},
    windows_api::{
        keyboard::{self, KeyboardLayout},
        media::{self, MediaInfo},
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
}

static STATE: LazyLock<RwLock<SystemState>> = LazyLock::new(|| RwLock::new(SystemState::default()));

pub fn get() -> SystemState {
    STATE.read().clone()
}

fn active_modules() -> HashSet<String> {
    dock_items::get()
        .all()
        .filter_map(|item| match item {
            DockItem::Module { module, .. } => Some(module.clone()),
            _ => None,
        })
        .collect()
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
