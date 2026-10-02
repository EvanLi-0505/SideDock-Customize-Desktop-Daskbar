//! `SideDock.exe --win-key-helper`: the elevated half of the Win key takeover.
//!
//! Started by the "SideDock Win Key" scheduled task (see `win_key.rs`), so it runs with
//! administrator rights without a UAC prompt; it does nothing but the keyboard hook.
//! The non-elevated SideDock process could not keep the Windows start menu from opening:
//! input injected by a medium-integrity process is not honoured for this. The hook below
//! follows Seelen UI's `win-hotkeys` crate (MIT): when a lone Win press is released, an
//! unassigned "silent" key (vkE8) is injected from inside the hook callback - it is
//! processed before the key-up being hooked - and the key-up itself is passed on. Every
//! key event reaches Windows untouched, so all Win+key combinations keep working.
//!
//! Lifetime: the helper attaches to the SideDock process that owns the target window and
//! exits as soon as that process exits or signals the stop event. It never starts on its
//! own (the task has no trigger).

use std::{
    sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering},
    time::{Duration, Instant},
};

use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM, LRESULT, WAIT_OBJECT_0, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{
                OpenEventW, OpenProcess, PROCESS_SYNCHRONIZE, SYNCHRONIZATION_SYNCHRONIZE,
            },
        },
        UI::{
            Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
                SendInput, VIRTUAL_KEY, VK_LWIN, VK_RWIN,
            },
            WindowsAndMessaging::{
                AllowSetForegroundWindow, CallNextHookEx, DispatchMessageW, FindWindowExW,
                GetWindowThreadProcessId, HC_ACTION, HWND_MESSAGE, KBDLLHOOKSTRUCT,
                MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW,
                PostMessageW, QS_ALLINPUT, SetWindowsHookExW, TranslateMessage,
                UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN,
                WM_SYSKEYUP,
            },
        },
    },
    core::{HSTRING, PCWSTR},
};

use super::win_key::{
    HELPER_ARG, MSG_HELPER_READY, MSG_LONE_WIN_PRESS, TARGET_CLASS, stop_event_name,
};

/// Unassigned virtual key used to keep the start menu closed.
const SILENT_KEY: u16 = 0xE8;

static TARGET: AtomicIsize = AtomicIsize::new(0);
static MAIN_PID: AtomicU32 = AtomicU32::new(0);
static HELD: AtomicBool = AtomicBool::new(false);
static COMBO: AtomicBool = AtomicBool::new(false);

fn key(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                ..Default::default()
            },
        },
    }
}

fn find_target() -> Option<HWND> {
    unsafe {
        FindWindowExW(
            Some(HWND_MESSAGE),
            None,
            &HSTRING::from(TARGET_CLASS),
            PCWSTR::null(),
        )
        .ok()
    }
}

fn notify(msg: u32) {
    let mut hwnd = HWND(TARGET.load(Ordering::Acquire) as *mut _);
    unsafe {
        if PostMessageW(Some(hwnd), msg, WPARAM(0), LPARAM(0)).is_err() {
            // the target window was recreated
            if let Some(fresh) = find_target() {
                hwnd = fresh;
                TARGET.store(hwnd.0 as isize, Ordering::Release);
                let _ = PostMessageW(Some(hwnd), msg, WPARAM(0), LPARAM(0));
            }
        }
    }
}

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = info.vkCode as u16;
        if vk != SILENT_KEY {
            let is_win = vk == VK_LWIN.0 || vk == VK_RWIN.0;
            let msg = wparam.0 as u32;
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
            if is_win && down {
                // first down only, not auto-repeat
                if !HELD.swap(true, Ordering::AcqRel) {
                    COMBO.store(false, Ordering::Release);
                }
            } else if !is_win && down && HELD.load(Ordering::Acquire) {
                COMBO.store(true, Ordering::Release);
            } else if is_win
                && up
                && HELD.swap(false, Ordering::AcqRel)
                && !COMBO.load(Ordering::Acquire)
            {
                // a lone press: silent key now (processed before this key-up), the key-up
                // itself goes on (blocking key-ups leaves keys stuck in some apps)
                unsafe {
                    SendInput(
                        &[key(SILENT_KEY, false), key(SILENT_KEY, true)],
                        std::mem::size_of::<INPUT>() as i32,
                    );
                    let _ = AllowSetForegroundWindow(MAIN_PID.load(Ordering::Acquire));
                }
                notify(MSG_LONE_WIN_PRESS);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Entry point of `SideDock.exe --win-key-helper`.
pub fn helper_main() {
    log::info!("Win key helper starting ({HELPER_ARG})");

    // the SideDock process that started us owns the target window
    let deadline = Instant::now() + Duration::from_secs(15);
    let target = loop {
        if let Some(hwnd) = find_target() {
            break hwnd;
        }
        if Instant::now() > deadline {
            log::warn!("Win key helper: SideDock is not running, exiting");
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    TARGET.store(target.0 as isize, Ordering::Release);
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(target, Some(&mut pid)) };
    MAIN_PID.store(pid, Ordering::Release);

    unsafe {
        let Ok(process) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) else {
            log::warn!("Win key helper: cannot watch SideDock (pid {pid})");
            return;
        };
        let stop = OpenEventW(
            SYNCHRONIZATION_SYNCHRONIZE,
            false,
            &HSTRING::from(stop_event_name(pid)),
        )
        .unwrap_or(HANDLE::default());

        let module = GetModuleHandleW(None).unwrap_or_default();
        let hook_handle =
            match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), Some(module.into()), 0) {
                Ok(h) => h,
                Err(err) => {
                    log::warn!("Win key helper: hook not installed: {err}");
                    let _ = CloseHandle(process);
                    return;
                }
            };
        notify(MSG_HELPER_READY);
        log::info!("Win key helper: active for SideDock pid {pid}");

        // pump messages (the hook needs it) until SideDock exits or asks us to stop
        let handles: Vec<HANDLE> = if stop.is_invalid() {
            vec![process]
        } else {
            vec![process, stop]
        };
        'outer: loop {
            let wait = MsgWaitForMultipleObjectsEx(
                Some(&handles),
                u32::MAX,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            );
            if wait.0 < WAIT_OBJECT_0.0 + handles.len() as u32 {
                break; // SideDock exited or stop requested
            }
            let mut msg = Default::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break 'outer;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        let _ = UnhookWindowsHookEx(hook_handle);
        let _ = CloseHandle(process);
        if !stop.is_invalid() {
            let _ = CloseHandle(stop);
        }
    }
    log::info!("Win key helper: stopped");
}
