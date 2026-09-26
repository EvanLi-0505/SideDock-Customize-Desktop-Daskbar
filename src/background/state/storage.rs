//! Crash-safe JSON persistence.
//!
//! Writes go to `<file>.tmp` first and are then atomically renamed over the target,
//! keeping the previous version as `<file>.bak`. A forced kill in the middle of a save
//! can therefore never leave a half written config behind. On load, a corrupt file is
//! moved aside and the backup (or the defaults) is used instead.

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Serialize, de::DeserializeOwned};

use crate::error::Result;

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

/// Loads `path`, falling back to `path.bak` and finally to `T::default()`.
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    if !path.exists() {
        return T::default();
    }
    match read::<T>(path) {
        Ok(value) => value,
        Err(err) => {
            log::error!("{} is unreadable ({err}), trying backup", path.display());
            let stamp = time::OffsetDateTime::now_utc().unix_timestamp();
            let _ = std::fs::rename(path, with_suffix(path, &format!(".corrupt-{stamp}")));
            match read::<T>(&with_suffix(path, ".bak")) {
                Ok(value) => {
                    log::warn!("restored {} from backup", path.display());
                    value
                }
                Err(_) => {
                    log::warn!("no usable backup for {}, using defaults", path.display());
                    T::default()
                }
            }
        }
    }
}

/// Atomically replaces `path` with the JSON representation of `value`.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let data = serde_json::to_vec_pretty(value)?;
    let tmp = with_suffix(path, ".tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(&data)?;
        file.sync_all()?;
    }
    if path.exists() {
        let _ = std::fs::copy(path, with_suffix(path, ".bak"));
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}
