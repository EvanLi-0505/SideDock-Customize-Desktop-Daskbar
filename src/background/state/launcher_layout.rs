//! The launcher's own arrangement: the apps the user placed (in order) and the
//! collections (folders) grouping several apps, plus the apps removed from the launcher.
//! What is shown depends on `LauncherSettings::content` (every app, or only these).
//!
//! Apps are identified by their `shell:AppsFolder` id (`modules/start_apps.rs`).

use std::{collections::HashSet, path::PathBuf, sync::LazyLock};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::storage;
use crate::{error::Result, paths};

const MAX_ENTRIES: usize = 2048;
const MAX_NAME: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LauncherEntry {
    App {
        id: String,
    },
    /// a collection: opens into its own grid
    Folder {
        id: String,
        name: String,
        apps: Vec<String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LauncherLayout {
    /// placed apps and collections, in order
    pub items: Vec<LauncherEntry>,
    /// apps the user removed from the launcher (still found by search)
    pub removed: Vec<String>,
}

impl LauncherLayout {
    /// Every app appears at most once; empty collections disappear.
    fn sanitize(&mut self) {
        let mut seen: HashSet<String> = HashSet::new();
        let mut folder_ids: HashSet<String> = HashSet::new();
        self.items.retain_mut(|entry| match entry {
            LauncherEntry::App { id } => !id.is_empty() && seen.insert(id.clone()),
            LauncherEntry::Folder { id, name, apps } => {
                apps.retain(|a| !a.is_empty() && seen.insert(a.clone()));
                if name.chars().count() > MAX_NAME {
                    *name = name.chars().take(MAX_NAME).collect();
                }
                !apps.is_empty() && !id.is_empty() && folder_ids.insert(id.clone())
            }
        });
        self.items.truncate(MAX_ENTRIES);
        self.removed.retain(|a| !seen.contains(a));
        let mut removed_seen = HashSet::new();
        self.removed.retain(|a| removed_seen.insert(a.clone()));
        self.removed.truncate(MAX_ENTRIES);
    }
}

static LAYOUT: LazyLock<RwLock<LauncherLayout>> = LazyLock::new(|| {
    let mut layout: LauncherLayout = storage::load_json(&file_path());
    layout.sanitize();
    RwLock::new(layout)
});

fn file_path() -> PathBuf {
    paths::get().config.join("launcher_layout.json")
}

pub fn get() -> LauncherLayout {
    LAYOUT.read().clone()
}

/// Persists the layout; returns the sanitized value and whether it changed.
pub fn replace(mut layout: LauncherLayout) -> Result<(LauncherLayout, bool)> {
    layout.sanitize();
    let mut guard = LAYOUT.write();
    let changed = *guard != layout;
    if changed {
        storage::save_json(&file_path(), &layout)?;
        *guard = layout.clone();
    }
    Ok((layout, changed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_dedupes_and_drops_empty_folders() {
        let mut layout = LauncherLayout {
            items: vec![
                LauncherEntry::App { id: "a".into() },
                LauncherEntry::Folder {
                    id: "f1".into(),
                    name: "Games".into(),
                    apps: vec!["a".into(), "b".into()],
                },
                LauncherEntry::Folder {
                    id: "f2".into(),
                    name: "Empty".into(),
                    apps: vec!["b".into()],
                },
            ],
            removed: vec!["b".into(), "c".into(), "c".into()],
        };
        layout.sanitize();
        assert_eq!(
            layout.items,
            vec![
                LauncherEntry::App { id: "a".into() },
                LauncherEntry::Folder {
                    id: "f1".into(),
                    name: "Games".into(),
                    apps: vec!["b".into()],
                },
            ]
        );
        assert_eq!(layout.removed, vec!["c".to_string()]);
    }
}
