//! Portable path layout.
//!
//! SideDock never writes to the user profile (`%APPDATA%`, `%LOCALAPPDATA%`, ...).
//! Everything it produces lives under a single `data` folder next to the executable:
//!
//! ```text
//! <exe dir>/
//!   SideDock.exe
//!   data/
//!     config/     settings.json, dock_items.json (+ .bak copies)
//!     logs/       rolling log files
//!     userdata/   WebView2 profile (cookies, cache, local storage)
//!     cache/      extracted application icons
//!     runtime/    session lock + crash bookkeeping
//! ```
//!
//! During development (`debug` builds) the folder is `<repo>/data` so that
//! `target/` stays disposable. It can always be overridden with `SIDEDOCK_DATA_DIR`.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::error::Result;

static PATHS: OnceLock<AppPaths> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub exe: PathBuf,
    pub root: PathBuf,
    pub config: PathBuf,
    pub logs: PathBuf,
    pub userdata: PathBuf,
    pub cache: PathBuf,
    pub runtime: PathBuf,
}

impl AppPaths {
    fn resolve() -> Result<Self> {
        let exe = std::env::current_exe()?;
        let root = match std::env::var_os("SIDEDOCK_DATA_DIR") {
            Some(custom) if !custom.is_empty() => PathBuf::from(custom),
            _ if cfg!(debug_assertions) => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("data"),
            _ => exe
                .parent()
                .ok_or("executable has no parent directory")?
                .join("data"),
        };
        let root = normalize(&root);

        Ok(Self {
            exe,
            config: root.join("config"),
            logs: root.join("logs"),
            userdata: root.join("userdata"),
            cache: root.join("cache"),
            runtime: root.join("runtime"),
            root,
        })
    }

    fn ensure_writable(&self) -> Result<()> {
        for dir in [
            &self.root,
            &self.config,
            &self.logs,
            &self.userdata,
            &self.cache,
            &self.runtime,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        let probe = self.runtime.join(".write-probe");
        std::fs::write(&probe, b"ok")?;
        std::fs::remove_file(&probe)?;
        Ok(())
    }

    pub fn webview_data_dir(&self) -> PathBuf {
        self.userdata.join("webview")
    }

    pub fn icon_cache_dir(&self) -> PathBuf {
        self.cache.join("icons")
    }
}

fn normalize(path: &Path) -> PathBuf {
    // canonicalize adds the `\\?\` prefix on Windows which some shell APIs dislike.
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolves and creates the data folders. Must be called once, before anything else.
pub fn init() -> Result<&'static AppPaths> {
    let paths = AppPaths::resolve()?;
    paths.ensure_writable().map_err(|e| {
        format!(
            "SideDock cannot write to its data folder:\n{}\n\n{e}\n\nMove SideDock to a folder you can write to.",
            paths.root.display()
        )
    })?;
    // Global fallback: any WebView2 instance that is not given an explicit data folder
    // would otherwise create one under %LOCALAPPDATA%.
    // SAFETY: called on the main thread before any other thread is spawned.
    unsafe { std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", paths.webview_data_dir()) };
    Ok(PATHS.get_or_init(|| paths))
}

pub fn get() -> &'static AppPaths {
    PATHS.get().expect("paths::init must be called first")
}

/// Total size in bytes of a directory tree. Unreadable entries are skipped.
pub fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(ft) if ft.is_dir() => dir_size(&entry.path()),
            // directory entries report stale sizes for files that are open (the log)
            Ok(_) => std::fs::metadata(entry.path())
                .map(|m| m.len())
                .unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

/// Deletes the content of a directory but keeps the directory itself.
/// Entries listed in `keep` and files that are locked (in use) are skipped.
/// Returns how many entries could not be removed.
pub fn clear_dir(path: &Path, keep: &[&Path]) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let mut failed = 0;
    for entry in entries.flatten() {
        let p = entry.path();
        if keep.iter().any(|k| *k == p) {
            continue;
        }
        let res = if p.is_dir() {
            std::fs::remove_dir_all(&p)
        } else {
            std::fs::remove_file(&p)
        };
        if res.is_err() {
            failed += 1;
        }
    }
    failed
}
