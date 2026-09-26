//! Shell AppBar registration (ported from Seelen UI `windows_api/app_bar.rs`).
//!
//! An AppBar reserves a strip of the monitor work area exactly like the native taskbar
//! does, so maximized windows never cover the dock. Because SideDock keeps the native
//! taskbar alive, both bars simply coexist.

use std::sync::LazyLock;

use parking_lot::Mutex;
use windows::Win32::{
    Foundation::HWND,
    UI::Shell::{
        ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS,
        APPBARDATA, SHAppBarMessage,
    },
};

use super::window::Rect;
use crate::{error::Result, state::settings::DockSide};

/// hwnd -> reserved rect (as granted by the shell)
static REGISTERED: LazyLock<Mutex<Vec<(isize, Rect, DockSide)>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

fn data(hwnd: isize) -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: HWND(hwnd as *mut _),
        ..Default::default()
    }
}

fn edge(side: DockSide) -> u32 {
    match side {
        DockSide::Left => ABE_LEFT,
        DockSide::Top => ABE_TOP,
        DockSide::Right => ABE_RIGHT,
        DockSide::Bottom => ABE_BOTTOM,
    }
}

/// Registers (or updates) `hwnd` as an AppBar reserving `rect` on `side`.
/// Returns the rect actually granted by the shell.
pub fn register(hwnd: isize, rect: Rect, side: DockSide) -> Result<Rect> {
    let mut guard = REGISTERED.lock();
    let mut abd = data(hwnd);

    if let Some(pos) = guard.iter().position(|(h, _, _)| *h == hwnd) {
        let (_, old_rect, old_side) = guard[pos];
        if old_rect == rect && old_side == side {
            return Ok(old_rect);
        }
    } else {
        let ok = unsafe { SHAppBarMessage(ABM_NEW, &mut abd) };
        if ok == 0 {
            return Err("failed to register the dock as an AppBar".into());
        }
        guard.push((hwnd, rect, side));
    }

    abd.uEdge = edge(side);
    abd.rc = rect.into();
    unsafe {
        SHAppBarMessage(ABM_QUERYPOS, &mut abd);
    }
    // the shell may move us away from other bars; keep the requested thickness
    let mut granted: Rect = abd.rc.into();
    match side {
        DockSide::Left => granted.right = granted.left + rect.width(),
        DockSide::Right => granted.left = granted.right - rect.width(),
        DockSide::Top => granted.bottom = granted.top + rect.height(),
        DockSide::Bottom => granted.top = granted.bottom - rect.height(),
    }
    abd.rc = granted.into();
    unsafe {
        SHAppBarMessage(ABM_SETPOS, &mut abd);
    }

    if let Some(entry) = guard.iter_mut().find(|(h, _, _)| *h == hwnd) {
        *entry = (hwnd, rect, side);
    }
    persist(&guard);
    Ok(granted)
}

pub fn unregister(hwnd: isize) {
    let mut guard = REGISTERED.lock();
    if let Some(pos) = guard.iter().position(|(h, _, _)| *h == hwnd) {
        let mut abd = data(hwnd);
        unsafe { SHAppBarMessage(ABM_REMOVE, &mut abd) };
        guard.remove(pos);
        persist(&guard);
    }
}

/// Reserved rect for a registered bar, if any.
pub fn reserved(hwnd: isize) -> Option<(Rect, DockSide)> {
    REGISTERED
        .lock()
        .iter()
        .find(|(h, _, _)| *h == hwnd)
        .map(|(_, r, s)| (*r, *s))
}

/// Removes every bar registered by this process. Safe to call from a panic hook.
pub fn unregister_all() {
    let Some(mut guard) = REGISTERED.try_lock_for(std::time::Duration::from_millis(500)) else {
        return;
    };
    for (hwnd, _, _) in guard.drain(..) {
        let mut abd = data(hwnd);
        unsafe { SHAppBarMessage(ABM_REMOVE, &mut abd) };
    }
    persist(&guard);
}

/// Removes bars left behind by a crashed process (called by the guardian).
pub fn remove_stale(hwnds: &[isize]) {
    for hwnd in hwnds {
        let mut abd = data(*hwnd);
        unsafe { SHAppBarMessage(ABM_REMOVE, &mut abd) };
    }
}

/// The guardian process reads this file to clean up if we die unexpectedly.
fn persist(list: &[(isize, Rect, DockSide)]) {
    let hwnds: Vec<isize> = list.iter().map(|(h, _, _)| *h).collect();
    let path = crate::paths::get().runtime.join("appbars.json");
    if let Ok(json) = serde_json::to_string(&hwnds) {
        let _ = std::fs::write(path, json);
    }
}
