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

/// Which start menu the dock's start button (and optionally the Win key) opens.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartMenuMode {
    /// the Windows start menu
    Native,
    /// SideDock's full-screen launcher
    SideDock,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LauncherSettings {
    pub start_menu: StartMenuMode,
    /// A single press of the Win key opens the SideDock launcher (only with
    /// `StartMenuMode::SideDock`). Needs the elevated helper and its scheduled task,
    /// see `modules/win_key.rs`; only `win_key::enable` / `disable` should turn it on.
    pub take_over_win_key: bool,
    /// icon size in the launcher grid (CSS px)
    pub icon_size: u32,
    /// every app, or only the apps the user added (see `state/launcher_layout.rs`)
    pub content: LauncherContent,
    /// shortcuts in the launcher's bottom bar, in order (see `LAUNCHER_PLACES`); the user
    /// and the power button are always there
    pub places: Vec<String>,
}

/// Shortcuts the launcher's bottom bar can show, like the folders next to the power
/// button of the Windows start menu. Keep in sync with `src/ui/shared/launcherPlaces.ts`.
pub const LAUNCHER_PLACES: &[&str] = &[
    "settings",
    "explorer",
    "documents",
    "downloads",
    "music",
    "pictures",
    "videos",
    "network",
    "user-folder",
];

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LauncherContent {
    /// the user's layout, followed by every other app (except the removed ones)
    All,
    /// only the user's layout; the other apps are found by search or "All apps"
    Pinned,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            start_menu: StartMenuMode::Native,
            take_over_win_key: false,
            icon_size: 64,
            content: LauncherContent::All,
            places: ["settings", "explorer", "downloads"]
                .map(String::from)
                .to_vec(),
        }
    }
}

impl LauncherSettings {
    fn sanitize(&mut self) {
        self.icon_size = self.icon_size.clamp(40, 96);
        let mut seen = std::collections::HashSet::new();
        self.places
            .retain(|p| LAUNCHER_PLACES.contains(&p.as_str()) && seen.insert(p.clone()));
    }

    /// The Win key hook is only wanted when the SideDock start menu is selected.
    pub fn wants_win_key(&self) -> bool {
        self.start_menu == StartMenuMode::SideDock && self.take_over_win_key
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolbarPosition {
    Top,
    /// in place of the native taskbar when it is hidden, above it otherwise
    Bottom,
}

/// Every item the toolbar can show. Keep in sync with `src/ui/apps/toolbar/items.ts`.
pub const TOOLBAR_ITEMS: &[&str] = &[
    "start",
    "app-name",
    "media",
    "tray",
    "keyboard",
    "bluetooth",
    "quick-settings",
    "battery",
    "notifications",
    "clock",
    "task-manager",
    "show-desktop",
];

/// Items of the toolbar's three sections, in order. Items in no section are not shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ToolbarItems {
    pub left: Vec<String>,
    pub center: Vec<String>,
    pub right: Vec<String>,
}

impl Default for ToolbarItems {
    fn default() -> Self {
        let list = |items: &[&str]| items.iter().map(|s| s.to_string()).collect();
        Self {
            left: list(&["start", "app-name"]),
            center: list(&["media"]),
            right: list(&[
                "tray",
                "keyboard",
                "bluetooth",
                "quick-settings",
                "battery",
                "notifications",
                "clock",
                "show-desktop",
            ]),
        }
    }
}

impl ToolbarItems {
    /// Drops unknown and repeated items.
    fn sanitize(&mut self) {
        let mut seen: Vec<String> = Vec::new();
        for section in [&mut self.left, &mut self.center, &mut self.right] {
            section.retain(|id| {
                let ok = TOOLBAR_ITEMS.contains(&id.as_str()) && !seen.contains(id);
                if ok {
                    seen.push(id.clone());
                }
                ok
            });
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        [&self.left, &self.center, &self.right]
            .iter()
            .any(|s| s.iter().any(|i| i == id))
    }
}

/// Which notification area icons are shown on the toolbar and which go to the overflow
/// flyout ("^"). Icons are identified by `system_tray` keys (GUID or executable + id).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TrayPrefs {
    /// shown on the toolbar, in this order
    pub inline: Vec<String>,
    /// moved to the overflow flyout
    pub overflow: Vec<String>,
    /// where icons the user never placed go
    pub new_icons_inline: bool,
}

impl Default for TrayPrefs {
    fn default() -> Self {
        Self {
            inline: Vec::new(),
            overflow: Vec::new(),
            new_icons_inline: true,
        }
    }
}

const TRAY_PREFS_MAX: usize = 512;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ToolbarSettings {
    pub enabled: bool,
    pub monitors: DockMonitors,
    pub position: ToolbarPosition,
    /// The toolbar never reserves screen space (that would rearrange the desktop): it
    /// floats over windows, hides with them, or sits in the hidden taskbar's place.
    pub hide_mode: HideMode,
    pub delay_to_show: u32,
    pub delay_to_hide: u32,
    /// CSS px
    pub height: u32,
    /// in the hidden taskbar's place: exactly the taskbar's height
    pub match_taskbar_height: bool,
    /// content size (percent)
    pub scale: u32,
    /// None = same theme as the dock
    pub theme: Option<ThemeMode>,
    /// bar background opacity (percent) for the solid themes
    pub background_opacity: u32,
    pub items: ToolbarItems,
    pub tray: TrayPrefs,
    pub show_seconds: bool,
}

impl Default for ToolbarSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            monitors: DockMonitors::Primary,
            position: ToolbarPosition::Bottom,
            hide_mode: HideMode::OnOverlap,
            delay_to_show: 120,
            delay_to_hide: 600,
            height: 40,
            match_taskbar_height: true,
            scale: 100,
            theme: None,
            background_opacity: 92,
            items: ToolbarItems::default(),
            tray: TrayPrefs::default(),
            show_seconds: false,
        }
    }
}

impl ToolbarSettings {
    fn sanitize(&mut self) {
        self.height = self.height.clamp(28, 72);
        self.scale = self.scale.clamp(80, 140);
        self.delay_to_show = self.delay_to_show.min(10_000);
        self.delay_to_hide = self.delay_to_hide.min(10_000);
        self.background_opacity = self.background_opacity.min(100);
        self.items.sanitize();
        let tray = &mut self.tray;
        let mut seen = std::collections::HashSet::new();
        tray.inline.retain(|k| seen.insert(k.clone()));
        tray.overflow.retain(|k| seen.insert(k.clone()));
        tray.inline.truncate(TRAY_PREFS_MAX);
        tray.overflow.truncate(TRAY_PREFS_MAX);
    }

    /// The toolbar's theme (its own or the dock's).
    pub fn theme_or(&self, global: ThemeMode) -> ThemeMode {
        self.theme.unwrap_or(global)
    }

    pub fn shows_tray(&self) -> bool {
        self.enabled && self.items.contains("tray")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TaskbarSettings {
    /// hide the native Windows taskbar (its reserved space is kept)
    pub hide_native: bool,
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
    /// bar background opacity in percent for the solid (dark / light) themes
    pub background_opacity: u32,
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
            background_opacity: 92,
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
        self.background_opacity = self.background_opacity.min(100);
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
    pub launcher: LauncherSettings,
    pub toolbar: ToolbarSettings,
    pub taskbar: TaskbarSettings,
    /// action id -> accelerator ("Ctrl+Alt+Backquote"); empty string = disabled
    pub shortcuts: BTreeMap<String, String>,
}

/// Global shortcuts and their defaults. Keep in sync with `src/ui/shared/shortcuts.ts`.
pub const SHORTCUT_DEFAULTS: &[(&str, &str)] = &[
    ("toggle-dock", "Ctrl+Alt+Shift+D"),
    ("open-settings", "Ctrl+Alt+Shift+S"),
    ("open-launcher", "Alt+Shift+Space"),
    ("window-switcher", "Alt+Backquote"),
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
            launcher: LauncherSettings::default(),
            toolbar: ToolbarSettings::default(),
            taskbar: TaskbarSettings::default(),
            shortcuts: default_shortcuts(),
        }
    }
}

impl AppSettings {
    pub fn sanitize(&mut self) {
        self.dock.sanitize();
        self.launcher.sanitize();
        self.toolbar.sanitize();
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

impl ThemeMode {
    /// Glass themes draw over a native backdrop (Mica / acrylic).
    pub fn has_backdrop(self) -> bool {
        matches!(self, Self::Glass | Self::Clear)
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

/// Cheap accessor for per-frame code.
pub fn theme() -> ThemeMode {
    SETTINGS.read().theme
}

/// The toolbar's theme (its own or the global one), cheap.
pub fn toolbar_theme() -> ThemeMode {
    let s = SETTINGS.read();
    s.toolbar.theme_or(s.theme)
}

/// Persists `new` and returns the previous value.
pub fn replace(mut new: AppSettings) -> Result<AppSettings> {
    new.sanitize();
    let mut guard = SETTINGS.write();
    storage::save_json(&file_path(), &new)?;
    Ok(std::mem::replace(&mut *guard, new))
}
