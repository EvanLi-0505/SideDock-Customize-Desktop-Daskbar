//! Backups of the configuration (settings, dock items, auxiliary dock items, launcher
//! layout) as one readable JSON file, and restoring them.
//!
//! Backups only ever go where the user wants them: `backups` in the program's own folder
//! by default, or a folder the user picked (`AppSettings::backup_dir`). The folder is only
//! created when a backup is written. Nothing else (logs, WebView2 profile, icon cache) is
//! backed up: it is either rebuilt on its own or of no use on another computer.
//!
//! Restoring first backs up the current configuration next to the other backups (so a
//! restore can be undone), writes the files, then restarts SideDock: every part of it
//! reads the configuration again at startup.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, macros::format_description};

use super::{settings, storage};
use crate::{error::Result, paths};

const FORMAT: &str = "sidedock-backup";
const FORMAT_VERSION: u32 = 1;
const PREFIX: &str = "SideDock-backup-";
/// configuration files a backup holds (and the only ones a restore writes)
const FILES: [&str; 4] = [
    "settings.json",
    "dock_items.json",
    "aux_dock_items.json",
    "launcher_layout.json",
];
/// a backup larger than this is not one of ours
const MAX_SIZE: u64 = 8 * 1024 * 1024;
/// error code (translated by the page): the backup file is gone (deleted by hand)
pub const ERR_MISSING: &str = "backup-missing";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupFile {
    format: String,
    format_version: u32,
    app_version: String,
    /// local time, "2026-10-18 15:30:00"
    created: String,
    /// file name in `data/config` -> its content
    files: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub path: PathBuf,
    pub name: String,
    pub created: String,
    pub app_version: String,
    pub size: u64,
    /// made automatically right before a restore
    pub automatic: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    /// where backups go now
    pub dir: PathBuf,
    /// `backups` next to SideDock.exe
    pub default_dir: PathBuf,
    pub custom: bool,
    /// the folder exists (it is only created by the first backup, and may be deleted by hand)
    pub dir_exists: bool,
    /// newest first
    pub entries: Vec<BackupEntry>,
}

/// `backups` in the program's folder.
pub fn default_dir() -> PathBuf {
    let exe = &paths::get().exe;
    exe.parent().unwrap_or(exe).join("backups")
}

/// The folder backups go to.
pub fn dir() -> PathBuf {
    let custom = settings::get().backup_dir;
    if custom.trim().is_empty() {
        default_dir()
    } else {
        PathBuf::from(custom)
    }
}

pub fn info() -> BackupInfo {
    let dir = dir();
    BackupInfo {
        custom: !settings::get().backup_dir.trim().is_empty(),
        default_dir: default_dir(),
        dir_exists: dir.is_dir(),
        entries: list(&dir),
        dir,
    }
}

fn now() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

fn read(path: &Path) -> Result<BackupFile> {
    let size = std::fs::metadata(path)?.len();
    if size > MAX_SIZE {
        return Err("not a SideDock backup (file too large)".into());
    }
    parse(&std::fs::read_to_string(path)?)
}

/// Checks that `text` is a backup this version can restore.
fn parse(text: &str) -> Result<BackupFile> {
    let backup: BackupFile =
        serde_json::from_str(text).map_err(|_| "not a SideDock backup (unreadable)")?;
    if backup.format != FORMAT {
        return Err("not a SideDock backup".into());
    }
    if backup.format_version > FORMAT_VERSION {
        return Err("this backup was made by a newer SideDock".into());
    }
    if !backup.files.contains_key("settings.json") {
        return Err("this backup holds no settings".into());
    }
    Ok(backup)
}

fn list(dir: &Path) -> Vec<BackupEntry> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut list: Vec<BackupEntry> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with(PREFIX) || !name.ends_with(".json") {
                return None;
            }
            let path = e.path();
            let backup = read(&path).ok()?;
            Some(BackupEntry {
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                automatic: name.ends_with("-auto.json"),
                created: backup.created,
                app_version: backup.app_version,
                name,
                path,
            })
        })
        .collect();
    // the file names sort by time
    list.sort_by(|a, b| b.name.cmp(&a.name));
    list
}

/// Writes a backup of the current configuration into the backup folder.
pub fn create(automatic: bool) -> Result<BackupEntry> {
    let config = &paths::get().config;
    let mut files = BTreeMap::new();
    for name in FILES {
        let path = config.join(name);
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let value: serde_json::Value = serde_json::from_str(&text)?;
        files.insert(name.to_string(), value);
    }
    if !files.contains_key("settings.json") {
        // never saved yet: back up what SideDock runs with
        files.insert(
            "settings.json".into(),
            serde_json::to_value(settings::get())?,
        );
    }
    let at = now();
    let backup = BackupFile {
        format: FORMAT.into(),
        format_version: FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        created: at
            .format(format_description!(
                "[year]-[month]-[day] [hour]:[minute]:[second]"
            ))
            .unwrap_or_default(),
        files,
    };
    let dir = dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let stamp = at
        .format(format_description!(
            "[year][month][day]-[hour][minute][second]"
        ))
        .unwrap_or_default();
    let suffix = if automatic { "-auto" } else { "" };
    let mut path = dir.join(format!("{PREFIX}{stamp}{suffix}.json"));
    // two backups within the same second
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{PREFIX}{stamp}-{n}{suffix}.json"));
        n += 1;
    }
    let data = serde_json::to_vec_pretty(&backup)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &data)?;
    std::fs::rename(&tmp, &path)?;
    log::info!("configuration backed up to {}", path.display());
    Ok(BackupEntry {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        size: data.len() as u64,
        created: backup.created,
        app_version: backup.app_version,
        automatic,
        path,
    })
}

/// Restores a backup: the current configuration is backed up first, then the files
/// are written. The caller restarts SideDock.
pub fn restore(path: &Path) -> Result<()> {
    if !path.is_file() {
        return Err(ERR_MISSING.into());
    }
    let mut backup = read(path)?;
    create(true)?;
    // where backups go and whether SideDock starts with Windows belong to this
    // computer, not to the backup
    let current = settings::get();
    if let Some(serde_json::Value::Object(s)) = backup.files.get_mut("settings.json") {
        s.insert("backupDir".into(), current.backup_dir.clone().into());
        s.insert("autostart".into(), current.autostart.into());
    }
    let config = &paths::get().config;
    for name in FILES {
        let target = config.join(name);
        match backup.files.get(name) {
            Some(value) => storage::save_json(&target, value)?,
            // not in the backup: back to the defaults
            None => {
                if target.exists() {
                    std::fs::remove_file(&target)?;
                }
            }
        }
    }
    log::info!("configuration restored from {}", path.display());
    Ok(())
}

/// Deletes one backup (only a backup file in the backup folder). A backup already
/// deleted by hand counts as deleted, so its entry can go.
pub fn delete(path: &Path) -> Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let inside = path.parent().is_some_and(|p| p == dir());
    if !inside || !name.starts_with(PREFIX) || !name.ends_with(".json") {
        return Err("not a backup of the backup folder".into());
    }
    if !path.exists() {
        return Ok(());
    }
    read(path)?;
    std::fs::remove_file(path)?;
    log::info!("backup {} deleted", path.display());
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    /// A backup written by 3.0.0. Every later version must keep restoring it: never
    /// change what `format` / `formatVersion` 1 means, add a migration instead.
    const BACKUP_3_0_0: &str = r#"{
        "format": "sidedock-backup",
        "formatVersion": 1,
        "appVersion": "3.0.0",
        "created": "2026-10-18 01:02:06",
        "files": {
            "settings.json": { "dock": { "enabled": true, "position": "Left" } },
            "dock_items.json": { "isReorderDisabled": false, "left": [], "center": [], "right": [] }
        }
    }"#;

    #[test]
    fn backups_of_3_0_0_are_restorable() {
        let backup = parse(BACKUP_3_0_0).expect("3.0.0 backups stay readable");
        assert_eq!(backup.app_version, "3.0.0");
        let settings: crate::state::settings::AppSettings =
            serde_json::from_value(backup.files["settings.json"].clone()).expect("settings parse");
        assert!(settings.dock.enabled);
    }

    #[test]
    fn foreign_or_newer_files_are_refused() {
        assert!(parse("{}").is_err());
        assert!(parse(r#"{"format":"other","formatVersion":1,"appVersion":"1","created":"","files":{"settings.json":{}}}"#).is_err());
        let newer = BACKUP_3_0_0.replace("\"formatVersion\": 1", "\"formatVersion\": 99");
        assert!(parse(&newer).is_err());
    }
}
