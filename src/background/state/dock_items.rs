use std::{collections::HashSet, path::PathBuf, sync::LazyLock};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::storage;
use crate::{error::Result, paths};

/// Built-in dock modules. Keep in sync with `src/ui/shared/modules.ts`.
pub const MODULES: &[&str] = &[
    "start-menu",
    "show-desktop",
    "recycle-bin",
    "clock",
    "keyboard",
    "media",
    "power",
    "bluetooth",
    "network",
    "notifications",
    "task-manager",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relaunch {
    pub command: String,
    pub args: Option<String>,
    pub working_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppItem {
    pub id: String,
    pub display_name: String,
    /// Executable, shortcut (.lnk) or any file/folder.
    pub path: PathBuf,
    /// AppUserModelID, used for UWP/packaged apps and grouping.
    pub umid: Option<String>,
    pub pinned: bool,
    #[serde(default)]
    pub prevent_pinning: bool,
    #[serde(default)]
    pub relaunch: Option<Relaunch>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DockItem {
    App(AppItem),
    Separator { id: String },
    Module { id: String, module: String },
}

impl DockItem {
    pub fn id(&self) -> &str {
        match self {
            DockItem::App(app) => &app.id,
            DockItem::Separator { id } | DockItem::Module { id, .. } => id,
        }
    }

    fn module(id: &str) -> Self {
        DockItem::Module {
            id: format!("module-{id}"),
            module: id.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DockItems {
    pub is_reorder_disabled: bool,
    pub left: Vec<DockItem>,
    pub center: Vec<DockItem>,
    pub right: Vec<DockItem>,
}

impl Default for DockItems {
    fn default() -> Self {
        let explorer =
            PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()))
                .join("explorer.exe");
        Self {
            is_reorder_disabled: false,
            left: vec![DockItem::module("start-menu")],
            center: vec![DockItem::App(AppItem {
                id: uuid::Uuid::new_v4().to_string(),
                display_name: crate::windows_api::shell::shell_display_name(
                    r"shell:AppsFolder\Microsoft.Windows.Explorer",
                )
                .unwrap_or_else(|| "File Explorer".into()),
                path: explorer,
                umid: Some("Microsoft.Windows.Explorer".into()),
                pinned: true,
                prevent_pinning: false,
                relaunch: None,
            })],
            right: vec![
                DockItem::module("recycle-bin"),
                DockItem::module("keyboard"),
                DockItem::module("clock"),
                DockItem::module("show-desktop"),
            ],
        }
    }
}

impl DockItems {
    /// Removes duplicated ids, unknown modules and non pinned apps (running apps are
    /// derived from open windows at runtime and must never be persisted).
    pub fn sanitize(&mut self) {
        let mut seen = HashSet::new();
        let mut modules = HashSet::new();
        let mut keep = |item: &DockItem| {
            if !seen.insert(item.id().to_string()) {
                return false;
            }
            match item {
                DockItem::App(app) => app.pinned,
                DockItem::Module { module, .. } => {
                    MODULES.contains(&module.as_str()) && modules.insert(module.clone())
                }
                DockItem::Separator { .. } => true,
            }
        };
        self.left.retain(&mut keep);
        self.center.retain(&mut keep);
        self.right.retain(&mut keep);
    }

    pub fn all(&self) -> impl Iterator<Item = &DockItem> {
        self.left.iter().chain(&self.center).chain(&self.right)
    }
}

static ITEMS: LazyLock<RwLock<DockItems>> = LazyLock::new(|| {
    let mut items: DockItems = storage::load_json(&file_path());
    items.sanitize();
    RwLock::new(items)
});

fn file_path() -> PathBuf {
    paths::get().config.join("dock_items.json")
}

pub fn get() -> DockItems {
    ITEMS.read().clone()
}

pub fn replace(mut items: DockItems) -> Result<DockItems> {
    items.sanitize();
    let mut guard = ITEMS.write();
    if *guard != items {
        storage::save_json(&file_path(), &items)?;
        *guard = items;
    }
    Ok(guard.clone())
}

pub fn reset() -> Result<DockItems> {
    replace(DockItems::default())
}
