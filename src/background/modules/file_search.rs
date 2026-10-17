//! File search for the launcher: the user's own folders (Desktop, Documents, Downloads,
//! Pictures, Music, Videos) and the recent items, by name.
//!
//! The index lives in memory only (nothing is written to disk). It is built in the
//! background on the first search and rebuilt when it is older than `MAX_AGE`; a search
//! meanwhile uses what is there. The walk is bounded so a huge folder (a source tree, a
//! photo library) cannot make it slow: limited depth, entries and time, and folders that
//! only hold tool output (`node_modules`, `.git`...) are skipped.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime},
};

use parking_lot::RwLock;
use serde::Serialize;
use windows::{
    Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{
            FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music,
            FOLDERID_Pictures, FOLDERID_Recent, FOLDERID_Videos, KF_FLAG_DEFAULT,
            SHGetKnownFolderPath,
        },
    },
    core::GUID,
};

const MAX_DEPTH: usize = 5;
const MAX_ENTRIES: usize = 40_000;
const MAX_WALK: Duration = Duration::from_secs(3);
const MAX_AGE: Duration = Duration::from_secs(120);
const MAX_RESULTS: usize = 12;
/// folders whose content nobody looks for by name
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    ".svn",
    ".hg",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    ".idea",
    ".vscode",
    ".gradle",
    ".next",
    ".nuxt",
    "$recycle.bin",
];

struct Entry {
    name: String,
    /// lowercase name, for matching
    key: String,
    path: PathBuf,
    is_dir: bool,
    modified: SystemTime,
    /// from the recent items: ranks a little higher
    recent: bool,
}

struct Index {
    entries: Vec<Entry>,
    built: Option<Instant>,
}

static INDEX: RwLock<Index> = RwLock::new(Index {
    entries: Vec::new(),
    built: None,
});
static BUILDING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHit {
    pub name: String,
    pub path: PathBuf,
    /// the folder it is in, for display
    pub folder: String,
    pub is_dir: bool,
    /// a recent item (a shortcut): it has no folder of its own to reveal
    pub recent: bool,
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    let raw = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }.ok()?;
    let path = unsafe { raw.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(raw.0 as *const _)) };
    path.filter(|p| !p.is_empty()).map(PathBuf::from)
}

fn is_hidden(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    // FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM
    meta.file_attributes() & 0x6 != 0
}

fn walk(root: &Path, out: &mut Vec<Entry>, started: Instant) {
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if out.len() >= MAX_ENTRIES || started.elapsed() > MAX_WALK {
            return;
        }
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if is_hidden(&meta) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let key = name.to_lowercase();
            let is_dir = meta.is_dir();
            if is_dir && (SKIP_DIRS.contains(&key.as_str()) || key.starts_with('.')) {
                continue;
            }
            // desktop.ini and friends
            if !is_dir && key.ends_with(".ini") {
                continue;
            }
            let path = entry.path();
            if is_dir && depth + 1 < MAX_DEPTH && !meta.file_type().is_symlink() {
                stack.push((path.clone(), depth + 1));
            }
            out.push(Entry {
                name,
                key,
                path,
                is_dir,
                modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                recent: false,
            });
            if out.len() >= MAX_ENTRIES {
                return;
            }
        }
    }
}

/// The recent items: shortcuts to files the user opened (opening one opens the file).
fn recent(out: &mut Vec<Entry>) {
    let Some(dir) = known_folder(&FOLDERID_Recent) else {
        return;
    };
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let file = entry.file_name().to_string_lossy().to_string();
        let Some(name) = file.strip_suffix(".lnk") else {
            continue;
        };
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        out.push(Entry {
            key: name.to_lowercase(),
            name: name.to_string(),
            path: entry.path(),
            is_dir: false,
            modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            recent: true,
        });
    }
}

fn build() {
    let started = Instant::now();
    let mut entries = Vec::new();
    recent(&mut entries);
    let roots = [
        FOLDERID_Desktop,
        FOLDERID_Documents,
        FOLDERID_Downloads,
        FOLDERID_Pictures,
        FOLDERID_Music,
        FOLDERID_Videos,
    ];
    let mut seen: Vec<PathBuf> = Vec::new();
    for id in roots {
        let Some(root) = known_folder(&id) else {
            continue;
        };
        // a folder redirected inside another (Documents inside OneDrive...) only once
        if seen.iter().any(|s| root.starts_with(s)) {
            continue;
        }
        seen.push(root.clone());
        walk(&root, &mut entries, started);
    }
    log::info!(
        "file search: {} entries indexed in {} ms",
        entries.len(),
        started.elapsed().as_millis()
    );
    *INDEX.write() = Index {
        entries,
        built: Some(Instant::now()),
    };
}

/// Builds or refreshes the index in the background when needed.
pub fn refresh() {
    let stale = INDEX.read().built.is_none_or(|t| t.elapsed() > MAX_AGE);
    if !stale || BUILDING.swap(true, Ordering::AcqRel) {
        return;
    }
    crate::utils::spawn_named("file-index", || {
        build();
        BUILDING.store(false, Ordering::Release);
    });
}

/// Files and folders whose name matches every word of `query`, best first.
pub fn search(query: &str) -> Vec<FileHit> {
    refresh();
    let words: Vec<String> = query
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let index = INDEX.read();
    let mut hits: Vec<(i64, &Entry)> = index
        .entries
        .iter()
        .filter(|e| words.iter().all(|w| e.key.contains(w.as_str())))
        .map(|e| {
            let first = &words[0];
            let mut score: i64 = 0;
            if e.key == *first || e.key.rsplit_once('.').is_some_and(|(stem, _)| stem == first) {
                score += 400;
            } else if e.key.starts_with(first.as_str()) {
                score += 200;
            } else if e
                .key
                .split(|c: char| !c.is_alphanumeric())
                .any(|part| part.starts_with(first.as_str()))
            {
                score += 100;
            }
            if e.recent {
                score += 150;
            }
            // newer first among equals: up to +90 for files changed in the last 90 days
            let age_days = e
                .modified
                .elapsed()
                .map(|d| d.as_secs() / 86_400)
                .unwrap_or(u64::MAX);
            score += 90 - age_days.min(90) as i64;
            // shorter names are closer matches
            score -= e.key.chars().count().min(60) as i64;
            (score, e)
        })
        .collect();
    hits.sort_by_key(|h| std::cmp::Reverse(h.0));
    let mut out: Vec<FileHit> = Vec::new();
    for (_, e) in hits {
        // the same file through a recent item and through its folder: keep the first
        if out
            .iter()
            .any(|h| h.is_dir == e.is_dir && h.name.eq_ignore_ascii_case(&e.name))
        {
            continue;
        }
        out.push(FileHit {
            name: e.name.clone(),
            folder: e
                .path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            path: e.path.clone(),
            is_dir: e.is_dir,
            recent: e.recent,
        });
        if out.len() >= MAX_RESULTS {
            break;
        }
    }
    out
}

/// Opens a search result (`reveal`: its folder, with it selected).
pub fn open(path: &Path, reveal: bool) -> crate::error::Result<()> {
    if !path.exists() {
        return Err("the file no longer exists".into());
    }
    if reveal {
        crate::windows_api::shell::reveal_in_explorer(path)
    } else {
        crate::windows_api::shell::launch(&path.to_string_lossy(), None, None, false)
    }
}
