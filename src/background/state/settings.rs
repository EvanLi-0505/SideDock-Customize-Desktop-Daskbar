use std::{collections::BTreeMap, sync::LazyLock};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::storage;
use crate::{error::Result, paths};

// ============== enums ==============

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    En,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Frosted glass: translucent surfaces over a native Mica/acrylic backdrop
    Glass,
    /// Clear glass: more see-through surfaces over an acrylic backdrop
    Clear,
    Dark,
    Light,
    /// Follows the Windows light/dark preference
    System,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockMode {
    FullWidth,
    MinContent,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockSide {
    Left,
    Right,
    Top,
    Bottom,
}

impl DockSide {
    pub fn is_horizontal(self) -> bool {
        matches!(self, DockSide::Top | DockSide::Bottom)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HideMode {
    /// Never hide; the dock reserves screen space like the native taskbar.
    Never,
    /// Always hidden until the cursor touches the screen edge.
    Always,
    /// Hide only when a window overlaps the dock.
    OnOverlap,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalItemsVisibility {
    All,
    OnMonitor,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinnedItemsVisibility {
    Always,
    WhenPrimary,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MiddleClickAction {
    CloseApp,
    OpenNewInstance,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockMonitors {
    Primary,
    All,
}

/// What the dock does when its items do not fit.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverflowMode {
    /// shrink the icons, then overlap them like a stack of cards
    ShrinkThenStack,
    /// only shrink the icons
    ShrinkOnly,
    /// shrink, then move the apps that still do not fit into a "more" popup
    Collapse,
}

// ============== structs ==============

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DockSettings {
    pub enabled: bool,
    pub monitors: DockMonitors,
    pub mode: DockMode,
    pub position: DockSide,
    pub hide_mode: HideMode,
    pub delay_to_show: u32,
    pub delay_to_hide: u32,
    pub temporal_items_visibility: TemporalItemsVisibility,
    pub pinned_items_visibility: PinnedItemsVisibility,
    /// item size in px
    pub size: u32,
    pub margin: u32,
    pub padding: u32,
    pub space_between_items: u32,
    pub show_window_title: bool,
    pub show_instance_counter: bool,
    pub split_windows: bool,
    pub show_end_task: bool,
    pub middle_click_action: MiddleClickAction,
    /// Hide the dock while a fullscreen window (game, video) is focused on its monitor.
    pub hide_on_fullscreen: bool,
    /// macOS-like wave magnification under the cursor
    pub magnification: bool,
    /// scale of the icon right under the cursor (1.0 = no zoom)
    pub magnification_scale: f32,
    /// how many neighbours on each side are magnified too
    pub magnification_range: u32,
    /// show the app name next to the magnified icon
    pub show_labels: bool,
    pub overflow_mode: OverflowMode,
}

impl Default for DockSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            monitors: DockMonitors::Primary,
            mode: DockMode::FullWidth,
            position: DockSide::Left,
            hide_mode: HideMode::OnOverlap,
            delay_to_show: 100,
            delay_to_hide: 800,
            temporal_items_visibility: TemporalItemsVisibility::All,
            pinned_items_visibility: PinnedItemsVisibility::Always,
            size: 40,
            margin: 8,
            padding: 8,
            space_between_items: 8,
            show_window_title: false,
            show_instance_counter: true,
            split_windows: false,
            show_end_task: false,
            middle_click_action: MiddleClickAction::OpenNewInstance,
            hide_on_fullscreen: true,
            magnification: true,
            magnification_scale: 1.8,
            magnification_range: 3,
            show_labels: true,
            overflow_mode: OverflowMode::ShrinkThenStack,
        }
    }
}

impl DockSettings {
    /// Thickness of the dock in logical px (item + padding + margin on both sides).
    pub fn total_thickness(&self) -> u32 {
        self.size + self.padding * 2 + self.margin * 2
    }

    /// Extra room (logical px) the dock window needs on its inner side so magnified icons
    /// are not clipped. This area is transparent and click-through. It is kept as small as
    /// possible: a transparent WebView costs GPU/CPU per frame in proportion to its area,
    /// which is why the name label lives in the separate tooltip window.
    pub fn magnification_room(&self) -> u32 {
        if !self.magnification {
            return 0;
        }
        (self.size as f32 * (self.magnification_scale - 1.0)).ceil() as u32 + 8
    }

    fn sanitize(&mut self) {
        self.size = self.size.clamp(16, 128);
        if !self.magnification_scale.is_finite() {
            self.magnification_scale = 1.8;
        }
        self.magnification_scale = self.magnification_scale.clamp(1.1, 2.5);
        self.magnification_range = self.magnification_range.clamp(1, 6);
        self.margin = self.margin.min(64);
        self.padding = self.padding.min(64);
        self.space_between_items = self.space_between_items.min(64);
        self.delay_to_show = self.delay_to_show.min(10_000);
        self.delay_to_hide = self.delay_to_hide.min(10_000);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    pub language: Language,
    pub theme: ThemeMode,
    /// CSS color, empty means "use the Windows accent color".
    pub accent_color: String,
    /// Mirrors the registry Run key; the registry is the source of truth.
    pub autostart: bool,
    /// Relaunch SideDock automatically after an unexpected exit.
    pub crash_recovery: bool,
    pub dock: DockSettings,
    /// action id -> accelerator ("Ctrl+Alt+Backquote"); empty string = disabled
    pub shortcuts: BTreeMap<String, String>,
}

/// Global shortcuts and their defaults. Keep in sync with `src/ui/shared/shortcuts.ts`.
pub const SHORTCUT_DEFAULTS: &[(&str, &str)] = &[
    ("toggle-dock", "Ctrl+Alt+Shift+D"),
    ("open-settings", "Ctrl+Alt+Shift+S"),
];

pub fn default_shortcuts() -> BTreeMap<String, String> {
    SHORTCUT_DEFAULTS
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: crate::windows_api::system_language(),
            theme: ThemeMode::Glass,
            accent_color: String::new(),
            autostart: false,
            crash_recovery: true,
            dock: DockSettings::default(),
            shortcuts: default_shortcuts(),
        }
    }
}

impl AppSettings {
    pub fn sanitize(&mut self) {
        self.dock.sanitize();
        if self.accent_color.len() > 64 {
            self.accent_color.clear();
        }
        // unknown actions are dropped, missing ones (added in a newer version) get defaults
        let defaults = default_shortcuts();
        self.shortcuts.retain(|k, _| defaults.contains_key(k));
        for (k, v) in defaults {
            self.shortcuts.entry(k).or_insert(v);
        }
    }
}

// ============== manager ==============

static SETTINGS: LazyLock<RwLock<AppSettings>> = LazyLock::new(|| {
    let mut settings: AppSettings = storage::load_json(&file_path());
    settings.sanitize();
    RwLock::new(settings)
});

fn file_path() -> std::path::PathBuf {
    paths::get().config.join("settings.json")
}

pub fn get() -> AppSettings {
    SETTINGS.read().clone()
}

/// Persists `new` and returns the previous value.
pub fn replace(mut new: AppSettings) -> Result<AppSettings> {
    new.sanitize();
    let mut guard = SETTINGS.write();
    storage::save_json(&file_path(), &new)?;
    Ok(std::mem::replace(&mut *guard, new))
}
