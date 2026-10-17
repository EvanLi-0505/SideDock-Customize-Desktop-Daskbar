//! Windows asking for attention (the orange flashing taskbar button: a new message, a
//! finished download...).
//!
//! An application asks with `FlashWindowEx`; Windows tells the shell through the shell
//! hook (`HSHELL_FLASH`), which any window can receive with `RegisterShellHookWindow`.
//! SideDock keeps a hidden window for it on a thread of its own. Like on the taskbar, a
//! window keeps asking until it is brought to the front (`apps` clears the mark then).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG,
            RegisterClassW, RegisterShellHookWindow, RegisterWindowMessageW, TranslateMessage,
            WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
    core::{HSTRING, PCWSTR, w},
};

use super::apps;

const CLASS: &str = "SideDockAttention";
/// `HSHELL_REDRAW | HSHELL_HIGHBIT`: the window flashes
const HSHELL_FLASH: usize = 0x8006;
const HSHELL_WINDOWACTIVATED: usize = 4;
const HSHELL_RUDEAPPACTIVATED: usize = 0x8004;

static STARTED: AtomicBool = AtomicBool::new(false);
/// the registered "SHELLHOOK" message
static SHELL_MESSAGE: AtomicU32 = AtomicU32::new(0);

unsafe extern "system" fn proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let shell = SHELL_MESSAGE.load(Ordering::Acquire);
    if shell != 0 && msg == shell {
        let hwnd = lp.0;
        match wp.0 {
            HSHELL_FLASH => apps::set_attention(hwnd, true),
            HSHELL_WINDOWACTIVATED | HSHELL_RUDEAPPACTIVATED => apps::set_attention(hwnd, false),
            _ => {}
        }
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(h, msg, wp, lp) }
}

/// Starts listening (once).
pub fn start() {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    crate::utils::spawn_named("attention-hook", || unsafe {
        let class = HSTRING::from(CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
        // a hidden top-level window: the shell hook is for top-level windows
        let created = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
        );
        let Ok(hwnd) = created else {
            log::warn!("attention: window not created: {created:?}");
            STARTED.store(false, Ordering::Release);
            return;
        };
        SHELL_MESSAGE.store(RegisterWindowMessageW(w!("SHELLHOOK")), Ordering::Release);
        if !RegisterShellHookWindow(hwnd).as_bool() {
            log::warn!("attention: shell hook not registered");
            return;
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}
