//! Memory of the helper windows (popup, tooltip, launcher overlay) and of auto-hidden
//! auxiliary docks.
//!
//! They are created once and kept hidden so they open instantly, but each is a WebView2
//! renderer of its own. A window that has been hidden for a while gets the "low" memory
//! target (`super::set_memory_low`): WebView2 may swap its memory out. It goes back to
//! normal the moment the window is about to be used, before its page renders.
//!
//! The delay matters: tooltips and popups come and go all the time, and trimming a
//! window that is shown again a second later would only cost page faults.

use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use tauri::{AppHandle, Manager};

/// hidden for this long before its memory is released
const IDLE_AFTER: Duration = Duration::from_secs(30);
const CHECK_EVERY: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
struct Entry {
    /// since when the window is hidden, `None` while it is in use
    idle_since: Option<Instant>,
    lowered: bool,
}

static WINDOWS: Mutex<Option<HashMap<String, Entry>>> = Mutex::new(None);
static STARTED: AtomicBool = AtomicBool::new(false);

fn with<R>(f: impl FnOnce(&mut HashMap<String, Entry>) -> R) -> R {
    f(WINDOWS.lock().get_or_insert_with(HashMap::new))
}

/// The window was just hidden (or created hidden).
pub fn idle(label: &str) {
    with(|windows| {
        let entry = windows.entry(label.to_string()).or_insert(Entry {
            idle_since: None,
            lowered: false,
        });
        if entry.idle_since.is_none() {
            entry.idle_since = Some(Instant::now());
        }
    });
}

/// The window is about to render and show: give it its memory back first.
pub fn active(app: &AppHandle, label: &str) {
    let was_lowered = with(|windows| {
        let entry = windows.entry(label.to_string()).or_insert(Entry {
            idle_since: None,
            lowered: false,
        });
        entry.idle_since = None;
        std::mem::replace(&mut entry.lowered, false)
    });
    if was_lowered && let Some(window) = app.get_webview_window(label) {
        super::set_memory_low(&window, false);
    }
}

/// Starts the thread that lowers windows left idle.
pub fn start(app: AppHandle) {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    crate::utils::spawn_supervised("webview-memory", move || {
        loop {
            std::thread::sleep(CHECK_EVERY);
            let due: Vec<String> = with(|windows| {
                windows
                    .iter_mut()
                    .filter(|(_, e)| {
                        !e.lowered && e.idle_since.is_some_and(|t| t.elapsed() >= IDLE_AFTER)
                    })
                    .map(|(label, e)| {
                        e.lowered = true;
                        label.clone()
                    })
                    .collect()
            });
            for label in due {
                if let Some(window) = app.get_webview_window(&label) {
                    super::set_memory_low(&window, true);
                }
            }
        }
    });
}

/// A window is gone: forget it.
pub fn forget(label: &str) {
    with(|windows| windows.remove(label));
}
