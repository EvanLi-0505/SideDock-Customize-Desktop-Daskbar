//! Tracks every "user application window" (the ones Explorer would show on the taskbar).
//! Ported from Seelen UI `modules/apps`, simplified to a single worker thread.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        LazyLock,
        atomic::{AtomicIsize, Ordering},
        mpsc::RecvTimeoutError,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use parking_lot::{Mutex, RwLock};
use serde::Serialize;

use crate::{
    app,
    windows_api::{
        event_hook::{self, *},
        shell,
        window::{Rect, Window},
    },
};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserAppWindow {
    pub hwnd: isize,
    pub title: String,
    pub app_name: String,
    pub umid: Option<String>,
    pub path: Option<PathBuf>,
    pub monitor: isize,
    pub rect: Option<Rect>,
    pub is_iconic: bool,
    pub is_zoomed: bool,
    pub is_fullscreen: bool,
    pub last_foreground_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusedApp {
    pub hwnd: isize,
    pub title: String,
    pub path: Option<PathBuf>,
    /// true when the focused window belongs to SideDock itself
    pub is_own: bool,
}

static WINDOWS: LazyLock<RwLock<Vec<UserAppWindow>>> = LazyLock::new(|| RwLock::new(Vec::new()));
static FOCUSED: LazyLock<RwLock<FocusedApp>> = LazyLock::new(|| RwLock::new(FocusedApp::default()));
/// Last foreground window that does not belong to SideDock.
static LAST_EXTERNAL_FOREGROUND: AtomicIsize = AtomicIsize::new(0);
static NAME_CACHE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub const EVENT_WINDOWS_CHANGED: &str = "windows-changed";
pub const EVENT_FOCUS_CHANGED: &str = "focus-changed";

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn app_name(path: Option<&Path>, umid: Option<&str>) -> String {
    let key = umid
        .map(|u| format!("umid:{u}"))
        .or_else(|| path.map(|p| p.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    if let Some(name) = NAME_CACHE.lock().get(&key) {
        return name.clone();
    }
    let name = match path {
        Some(p) => shell::display_name_for(p, umid),
        None => umid.map(|u| u.to_string()).unwrap_or_default(),
    };
    NAME_CACHE.lock().insert(key, name.clone());
    name
}

fn snapshot(window: Window, last_foreground_at: i64) -> UserAppWindow {
    let umid = window.app_user_model_id();
    let path = window.exe_path();
    UserAppWindow {
        hwnd: window.0,
        title: window.title(),
        app_name: app_name(path.as_deref(), umid.as_deref()),
        umid,
        path,
        monitor: window.monitor(),
        rect: window.rect(),
        is_iconic: window.is_minimized(),
        is_zoomed: window.is_maximized(),
        is_fullscreen: window.is_fullscreen(),
        last_foreground_at,
    }
}

pub fn windows() -> Vec<UserAppWindow> {
    WINDOWS.read().clone()
}

pub fn focused() -> FocusedApp {
    FOCUSED.read().clone()
}

pub fn last_external_foreground() -> Option<Window> {
    let hwnd = LAST_EXTERNAL_FOREGROUND.load(Ordering::Acquire);
    (hwnd != 0)
        .then_some(Window(hwnd))
        .filter(|w| w.is_window())
}

fn update_focus(window: Window) {
    let is_own = window.process_id() == std::process::id();
    if !is_own {
        LAST_EXTERNAL_FOREGROUND.store(window.0, Ordering::Release);
    }
    let focused = FocusedApp {
        hwnd: window.0,
        title: window.title(),
        path: window.exe_path(),
        is_own,
    };
    let changed = {
        let mut guard = FOCUSED.write();
        let changed = *guard != focused;
        *guard = focused.clone();
        changed
    };
    if changed {
        app::emit(EVENT_FOCUS_CHANGED, &focused);
    }
}

/// Full rescan in z-order. Returns true if the list changed.
fn rescan() -> bool {
    let now = now_millis();
    let old: HashMap<isize, i64> = WINDOWS
        .read()
        .iter()
        .map(|w| (w.hwnd, w.last_foreground_at))
        .collect();
    let fresh: Vec<UserAppWindow> = Window::enumerate()
        .into_iter()
        .filter(|w| w.is_app_window())
        .enumerate()
        .map(|(i, w)| {
            let stamp = old.get(&w.0).copied().unwrap_or(now - i as i64);
            snapshot(w, stamp)
        })
        .collect();
    let mut list = WINDOWS.write();
    if *list != fresh {
        *list = fresh;
        return true;
    }
    false
}

fn handle_event(ev: event_hook::WinEvent) -> bool {
    let window = Window(ev.hwnd);
    let tracked = WINDOWS.read().iter().any(|w| w.hwnd == ev.hwnd);

    match ev.event {
        EVENT_SYSTEM_FOREGROUND => {
            update_focus(window);
            if tracked {
                let now = now_millis();
                if let Some(w) = WINDOWS.write().iter_mut().find(|w| w.hwnd == ev.hwnd) {
                    w.last_foreground_at = now;
                    w.is_iconic = false;
                }
                return true;
            }
            // a window can become an app window when it is first activated
            if window.is_app_window() {
                WINDOWS.write().insert(0, snapshot(window, now_millis()));
                return true;
            }
            false
        }
        EVENT_OBJECT_DESTROY => {
            if tracked {
                WINDOWS.write().retain(|w| w.hwnd != ev.hwnd);
                return true;
            }
            false
        }
        EVENT_OBJECT_LOCATIONCHANGE => {
            if !tracked {
                return false;
            }
            let rect = window.rect();
            let (iconic, zoomed, fullscreen) = (
                window.is_minimized(),
                window.is_maximized(),
                window.is_fullscreen(),
            );
            let monitor = window.monitor();
            let mut list = WINDOWS.write();
            if let Some(w) = list.iter_mut().find(|w| w.hwnd == ev.hwnd) {
                let changed = w.rect != rect
                    || w.is_iconic != iconic
                    || w.is_zoomed != zoomed
                    || w.is_fullscreen != fullscreen
                    || w.monitor != monitor;
                w.rect = rect;
                w.is_iconic = iconic;
                w.is_zoomed = zoomed;
                w.is_fullscreen = fullscreen;
                w.monitor = monitor;
                return changed;
            }
            false
        }
        EVENT_SYSTEM_MINIMIZESTART | EVENT_SYSTEM_MINIMIZEEND => {
            if let Some(w) = WINDOWS.write().iter_mut().find(|w| w.hwnd == ev.hwnd) {
                w.is_iconic = ev.event == EVENT_SYSTEM_MINIMIZESTART;
                return true;
            }
            false
        }
        EVENT_OBJECT_NAMECHANGE => {
            if !tracked {
                // some apps set their title after being shown
                if window.is_app_window() {
                    WINDOWS.write().insert(0, snapshot(window, now_millis()));
                    return true;
                }
                return false;
            }
            let title = window.title();
            let umid = window.app_user_model_id();
            let mut list = WINDOWS.write();
            if let Some(w) = list.iter_mut().find(|w| w.hwnd == ev.hwnd)
                && (w.title != title || w.umid != umid)
            {
                w.title = title;
                if w.umid != umid {
                    w.app_name = app_name(w.path.as_deref(), umid.as_deref());
                    w.umid = umid;
                }
                return true;
            }
            false
        }
        EVENT_OBJECT_SHOW | EVENT_OBJECT_UNCLOAKED | EVENT_OBJECT_CREATE => {
            if !tracked && window.is_app_window() {
                WINDOWS.write().insert(0, snapshot(window, now_millis()));
                return true;
            }
            false
        }
        EVENT_OBJECT_HIDE | EVENT_OBJECT_CLOAKED => {
            // UWP frames are hidden on minimize but still alive
            if tracked && !window.is_app_window() && !window.is_minimized() {
                WINDOWS.write().retain(|w| w.hwnd != ev.hwnd);
                return true;
            }
            false
        }
        _ => false,
    }
}

/// Starts the hook + worker thread. Changes are coalesced and emitted at most every 80ms.
pub fn start() {
    rescan();
    if let Some(fg) = Window::foreground() {
        update_focus(fg);
    }
    let rx = event_hook::start();

    crate::utils::spawn_supervised("apps-tracker", move || {
        let mut dirty = false;
        let mut last_emit = Instant::now();
        let mut last_rescan = Instant::now();
        loop {
            match rx.recv_timeout(Duration::from_millis(80)) {
                Ok(ev) => dirty |= handle_event(ev),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            // drain bursts (window drags produce hundreds of events)
            while let Ok(ev) = rx.try_recv() {
                dirty |= handle_event(ev);
            }
            if last_rescan.elapsed() > Duration::from_secs(4) {
                dirty |= rescan();
                last_rescan = Instant::now();
            }
            if dirty && last_emit.elapsed() >= Duration::from_millis(80) {
                app::emit(EVENT_WINDOWS_CHANGED, &*WINDOWS.read());
                dirty = false;
                last_emit = Instant::now();
            }
        }
    });
}
