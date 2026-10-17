use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::LazyLock,
};

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
    /// A folder stack: several pinned apps behind one icon, opened as a grid.
    Stack {
        id: String,
        name: String,
        items: Vec<AppItem>,
    },
}

/// longest stack name kept
const STACK_NAME_MAX: usize = 40;

impl DockItem {
    pub fn id(&self) -> &str {
        match self {
            DockItem::App(app) => &app.id,
            DockItem::Separator { id } | DockItem::Module { id, .. } | DockItem::Stack { id, .. } => {
                id
            }
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
    /// derived from open windows at runtime and must never be persisted), and empty
    /// stacks.
    pub fn sanitize(&mut self) {
        for list in [&mut self.left, &mut self.center, &mut self.right] {
            for item in list.iter_mut() {
                if let DockItem::Stack { name, items, .. } = item {
                    let mut ids = HashSet::new();
                    items.retain(|a| a.pinned && ids.insert(a.id.clone()));
                    *name = name.trim().chars().take(STACK_NAME_MAX).collect();
                }
            }
        }
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
                DockItem::Stack { items, .. } => !items.is_empty(),
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

// ---------------- auxiliary docks ----------------
//
// Each auxiliary dock (`settings::AUX_DOCK_IDS`) has its own items, kept in
// `aux_dock_items.json` by dock id: pinned apps and files, separators and modules, like
// the main dock, but never running (unpinned) apps.

static AUX_ITEMS: LazyLock<RwLock<HashMap<String, DockItems>>> = LazyLock::new(|| {
    let mut all: HashMap<String, DockItems> = storage::load_json(&aux_file_path());
    all.retain(|id, _| crate::state::settings::AUX_DOCK_IDS.contains(&id.as_str()));
    for items in all.values_mut() {
        items.sanitize();
    }
    RwLock::new(all)
});

fn aux_file_path() -> PathBuf {
    paths::get().config.join("aux_dock_items.json")
}

fn empty() -> DockItems {
    DockItems {
        is_reorder_disabled: false,
        left: Vec::new(),
        center: Vec::new(),
        right: Vec::new(),
    }
}

/// Items of a dock: the main one (`None`) or an auxiliary one.
pub fn get_for(store: Option<&str>) -> DockItems {
    match store {
        None => get(),
        Some(id) => AUX_ITEMS.read().get(id).cloned().unwrap_or_else(empty),
    }
}

pub fn replace_for(store: Option<&str>, mut items: DockItems) -> Result<DockItems> {
    let Some(id) = store else {
        return replace(items);
    };
    if !crate::state::settings::AUX_DOCK_IDS.contains(&id) {
        return Err(format!("unknown dock {id}").into());
    }
    items.sanitize();
    let mut all = AUX_ITEMS.write();
    if all.get(id) != Some(&items) {
        all.insert(id.to_string(), items.clone());
        storage::save_json(&aux_file_path(), &*all)?;
    }
    Ok(items)
}

pub fn reset() -> Result<DockItems> {
    replace(DockItems::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, pinned: bool) -> AppItem {
        AppItem {
            id: id.into(),
            display_name: id.into(),
            path: PathBuf::from(format!("C:\\{id}.exe")),
            umid: None,
            pinned,
            prevent_pinning: false,
            relaunch: None,
        }
    }

    #[test]
    fn stacks_are_sanitized() {
        let mut items = DockItems {
            is_reorder_disabled: false,
            left: vec![],
            center: vec![
                DockItem::Stack {
                    id: "s1".into(),
                    name: format!("  {}  ", "n".repeat(60)),
                    items: vec![app("a", true), app("a", true), app("b", false), app("c", true)],
                },
                DockItem::Stack {
                    id: "empty".into(),
                    name: "x".into(),
                    items: vec![app("d", false)],
                },
            ],
            right: vec![],
        };
        items.sanitize();
        assert_eq!(items.center.len(), 1, "a stack without pinned apps goes away");
        let DockItem::Stack { name, items: apps, .. } = &items.center[0] else {
            panic!("stack expected");
        };
        assert_eq!(name.chars().count(), STACK_NAME_MAX);
        let ids: Vec<&str> = apps.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["a", "c"], "duplicates and unpinned apps are dropped");
    }

    #[test]
    fn stack_json_matches_the_page() {
        let json = r#"{"type":"Stack","id":"s","name":"Games","items":[
            {"type":"App","id":"a","displayName":"A","path":"C:\\a.exe","umid":null,"pinned":true,
             "preventPinning":false,"relaunch":null}]}"#;
        let item: DockItem = serde_json::from_str(json).expect("stack parses");
        assert!(matches!(&item, DockItem::Stack { items, .. } if items.len() == 1));
        let back = serde_json::to_value(&item).unwrap();
        assert_eq!(back["type"], "Stack");
        assert_eq!(back["items"][0]["displayName"], "A");
    }
}
