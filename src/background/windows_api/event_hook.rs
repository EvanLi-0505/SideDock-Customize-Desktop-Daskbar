//! Global WinEvent hook running on its own message-loop thread.

use std::sync::{
    OnceLock,
    mpsc::{Receiver, Sender, channel},
};

use windows::Win32::{
    Foundation::HWND,
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook},
        WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, MSG, TranslateMessage, WINEVENT_OUTOFCONTEXT,
        },
    },
};

pub use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_CLOAKED, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW,
    EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND,
    EVENT_SYSTEM_MINIMIZESTART,
};

#[derive(Debug, Clone, Copy)]
pub struct WinEvent {
    pub event: u32,
    pub hwnd: isize,
}

static SENDER: OnceLock<Sender<WinEvent>> = OnceLock::new();

unsafe extern "system" fn callback(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    // only window-level events (OBJID_WINDOW / CHILDID_SELF)
    if id_object != 0 || id_child != 0 || hwnd.is_invalid() {
        return;
    }
    if let Some(tx) = SENDER.get() {
        let _ = tx.send(WinEvent {
            event,
            hwnd: hwnd.0 as isize,
        });
    }
}

/// Starts the hook thread and returns the receiving end. Can only be called once.
pub fn start() -> Receiver<WinEvent> {
    let (tx, rx) = channel();
    if SENDER.set(tx).is_err() {
        panic!("event hook already started");
    }
    crate::utils::spawn_named("win-event-hook", || unsafe {
        let hooks = [
            (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND),
            (EVENT_OBJECT_CREATE, EVENT_OBJECT_UNCLOAKED),
        ];
        for (min, max) in hooks {
            let hook = SetWinEventHook(min, max, None, Some(callback), 0, 0, WINEVENT_OUTOFCONTEXT);
            if hook.is_invalid() {
                log::error!("SetWinEventHook({min:#x}..{max:#x}) failed");
            }
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
    rx
}
