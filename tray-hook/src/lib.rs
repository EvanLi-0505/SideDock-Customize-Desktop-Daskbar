//! `sidedock_tray.dll`: loaded into Explorer's taskbar thread by SideDock (a
//! `WH_CALLWNDPROC` hook, see `src/background/modules/system_tray.rs`).
//!
//! Applications register their notification area ("tray") icons with
//! `Shell_NotifyIcon`, which sends a `WM_COPYDATA` message to the `Shell_TrayWnd`
//! window. This hook sees those messages before the taskbar does and forwards a copy to
//! SideDock's receiver window, so SideDock can show every icon in its toolbar.
//!
//! Approach ported from Seelen UI's `sluhk.dll`. This code runs inside explorer.exe, so
//! it is deliberately minimal: it never panics, never blocks for long (the forward has a
//! short timeout) and always lets the message continue to the taskbar unchanged.

use core::sync::atomic::{AtomicIsize, Ordering};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::DataExchange::COPYDATASTRUCT,
        UI::WindowsAndMessaging::{
            CWPSTRUCT, CallNextHookEx, FindWindowW, GetClassNameW, IsWindow, SMTO_ABORTIFHUNG,
            SendMessageTimeoutW, WM_COPYDATA,
        },
    },
    core::{BOOL, w},
};

/// `COPYDATASTRUCT::dwData` of `Shell_NotifyIcon` messages.
const NOTIFY_ICON_DATA: usize = 1;
/// How long Explorer may wait for SideDock (ms).
const FORWARD_TIMEOUT: u32 = 150;

/// Receiver window of SideDock (cached).
static RECEIVER: AtomicIsize = AtomicIsize::new(0);

fn is_taskbar(hwnd: HWND) -> bool {
    let mut name = [0u16; 32];
    let len = unsafe { GetClassNameW(hwnd, &mut name) };
    let len = usize::try_from(len).unwrap_or(0).min(name.len());
    name.get(..len)
        .is_some_and(|n| n.iter().copied().eq("Shell_TrayWnd".encode_utf16()))
}

fn receiver() -> Option<HWND> {
    let cached = HWND(RECEIVER.load(Ordering::Relaxed) as *mut _);
    if !cached.is_invalid() && unsafe { IsWindow(Some(cached)) }.as_bool() {
        return Some(cached);
    }
    // a hidden top-level window (it also receives the "TaskbarCreated" broadcast)
    let found = unsafe { FindWindowW(w!("SideDockTrayReceiver"), None) }.ok()?;
    RECEIVER.store(found.0 as isize, Ordering::Relaxed);
    Some(found)
}

/// Forwards a copy of a `Shell_NotifyIcon` message to SideDock.
unsafe fn forward(message: &CWPSTRUCT) {
    let Some(data) = (unsafe { (message.lParam.0 as *const COPYDATASTRUCT).as_ref() }) else {
        return;
    };
    if data.dwData != NOTIFY_ICON_DATA || data.lpData.is_null() || data.cbData == 0 {
        return;
    }
    if !is_taskbar(message.hwnd) {
        return;
    }
    let Some(target) = receiver() else {
        return;
    };
    let copy = COPYDATASTRUCT {
        dwData: NOTIFY_ICON_DATA,
        cbData: data.cbData,
        lpData: data.lpData,
    };
    unsafe {
        let _ = SendMessageTimeoutW(
            target,
            WM_COPYDATA,
            WPARAM(message.hwnd.0 as usize),
            LPARAM(&copy as *const COPYDATASTRUCT as isize),
            SMTO_ABORTIFHUNG,
            FORWARD_TIMEOUT,
            None,
        );
    }
}

/// `WH_CALLWNDPROC` hook procedure.
///
/// # Safety
/// Called by Windows with the documented `CallWndProc` arguments.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn CallWndProc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0
        && let Some(message) = unsafe { (lparam.0 as *const CWPSTRUCT).as_ref() }
        && message.message == WM_COPYDATA
    {
        unsafe { forward(message) };
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// # Safety
/// DLL entry point, called by the loader.
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllMain(
    _instance: *mut core::ffi::c_void,
    _reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> BOOL {
    BOOL(1)
}
