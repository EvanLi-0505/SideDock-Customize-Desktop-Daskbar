//! Icon cache served to the webviews through the `sdicon://` protocol
//! (`http://sdicon.localhost/?path=...&umid=...` inside WebView2).
//!
//! Extracted icons are cached in memory and in `data/cache/icons` so that the
//! shell is only asked once per application.

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    sync::LazyLock,
};

use parking_lot::Mutex;

use crate::{paths, windows_api::icons};

static MEMORY: LazyLock<Mutex<HashMap<u64, Vec<u8>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Transparent 1x1 png returned when no icon can be found.
const EMPTY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

fn key_for(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.to_lowercase().hash(&mut hasher);
    // icons cached at another size are ignored (and cleaned with the icon cache)
    icons::ICON_SIZE.hash(&mut hasher);
    hasher.finish()
}

fn disk_path(key: u64) -> PathBuf {
    paths::get()
        .icon_cache_dir()
        .join(format!("{key:016x}.png"))
}

/// Candidate parsing names in priority order: packaged app id first, then the file.
fn sources(path: Option<&str>, umid: Option<&str>) -> Vec<String> {
    let mut list = Vec::new();
    if let Some(umid) = umid.filter(|u| !u.is_empty()) {
        list.push(format!("shell:AppsFolder\\{umid}"));
    }
    if let Some(path) = path.filter(|p| !p.is_empty()) {
        list.push(path.to_string());
    }
    list
}

pub fn get_png(path: Option<&str>, umid: Option<&str>) -> Vec<u8> {
    for source in sources(path, umid) {
        let key = key_for(&source);
        if let Some(bytes) = MEMORY.lock().get(&key) {
            return bytes.clone();
        }
        let file = disk_path(key);
        if let Ok(bytes) = std::fs::read(&file) {
            MEMORY.lock().insert(key, bytes.clone());
            return bytes;
        }
        match icons::extract_png(&source) {
            Ok(bytes) => {
                let _ = std::fs::create_dir_all(paths::get().icon_cache_dir());
                let _ = std::fs::write(&file, &bytes);
                MEMORY.lock().insert(key, bytes.clone());
                return bytes;
            }
            Err(err) => log::debug!("no icon for {source}: {err}"),
        }
    }
    EMPTY_PNG.to_vec()
}

/// Memory-only lookup: lets the protocol handler answer without leaving its thread.
pub fn memory_hit(path: Option<&str>, umid: Option<&str>) -> Option<Vec<u8>> {
    let memory = MEMORY.lock();
    sources(path, umid)
        .iter()
        .find_map(|s| memory.get(&key_for(s)).cloned())
}

/// Loads (or extracts) the icons of these packaged / Start menu app ids in the
/// background, so the launcher never waits for the shell while it is on screen.
pub fn warm_up(umids: Vec<String>) {
    for umid in umids {
        if memory_hit(None, Some(&umid)).is_some() {
            continue;
        }
        crate::utils::run_in_pool(move || {
            get_png(None, Some(&umid));
        });
    }
}

pub fn clear_memory() {
    MEMORY.lock().clear();
}

fn query_param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == name).then(|| percent_decode(v))
    })
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn params(uri: &str) -> (Option<String>, Option<String>) {
    let query = uri.split_once('?').map(|(_, q)| q).unwrap_or("");
    (query_param(query, "path"), query_param(query, "umid"))
}

/// Handler for the `sdicon` URI scheme (may block: call it off the UI thread).
pub fn handle_request(uri: &str) -> Vec<u8> {
    let (path, umid) = params(uri);
    get_png(path.as_deref(), umid.as_deref())
}

/// Non-blocking variant: only icons already in memory.
pub fn handle_request_cached(uri: &str) -> Option<Vec<u8>> {
    let (path, umid) = params(uri);
    memory_hit(path.as_deref(), umid.as_deref())
}
