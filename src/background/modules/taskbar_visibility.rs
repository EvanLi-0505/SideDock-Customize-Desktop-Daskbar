//! Optional hiding of the native Windows taskbar, **without changing the desktop
//! layout**.
//!
//! Seelen UI hides the taskbar by switching it to auto-hide first, which releases the
//! space it reserves: the work area grows, desktop icons and maximized windows move.
//! SideDock only hides the taskbar *windows* (`Shell_TrayWnd`, `Shell_SecondaryTrayWnd`)
//! and leaves their auto-hide state alone, so Explorer keeps reserving exactly the same
//! space: the work area stays identical on every monitor, whatever its resolution,
//! aspect ratio (16:9, 16:10, ...) or scaling. The toolbar can then take that space
//! (`widgets/toolbar.rs`).
//!
//! * Explorer shows the taskbar again now and then (Start, flyouts, display changes, a
//!   restart): a WinEvent hook hides it again immediately, and a slow poll covers the
//!   rest.
//! * Showing and hiding are asynchronous requests to Explorer, and the hook and the poll
//!   run on their own threads: a "hide" decided just before the user turns the option
//!   off can land after the "show". Restoring therefore never trusts the current
//!   visibility: it always asks Explorer to show the taskbar, then keeps checking for a
//!   few seconds (`RESTORE_WATCH`) and shows it again if something hid it meanwhile.
//! * SideDock exiting restores it; after a crash the guardian does (marker file in
//!   `data/runtime`).
//! * The user is never left with neither a taskbar nor SideDock: the taskbar is only
//!   hidden once SideDock's window thread is known to be running (a heartbeat, see
//!   `heartbeat`) and a dock or toolbar has shown up (`ui_ready`). If the window thread
//!   stops responding (`UI_TIMEOUT`), the taskbar comes back until it recovers.
//! * At startup a taskbar left hidden by another SideDock copy (another folder, killed
//!   with its guardian) is shown again when this one does not hide it (`init`).

use std::{
    path::PathBuf,
    sync::{
        LazyLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use parking_lot::Mutex;

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        System::Threading::GetCurrentThreadId,
        UI::{
            Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
            WindowsAndMessaging::{
                DispatchMessageW, EVENT_OBJECT_SHOW, FindWindowExW, GetClassNameW, GetMessageW,
                GetWindowRect, IsWindowVisible, MSG, PostThreadMessageW, SW_HIDE, SW_SHOWNA,
                ShowWindowAsync, TranslateMessage, WINEVENT_OUTOFCONTEXT, WM_QUIT,
            },
        },
    },
    core::{HSTRING, PCWSTR},
};

use crate::{paths, windows_api::window::Rect};

static HIDDEN: AtomicBool = AtomicBool::new(false);
static POLLING: AtomicBool = AtomicBool::new(false);
/// serializes `set_hidden` (settings are applied from several threads)
static SWITCH: Mutex<()> = Mutex::new(());
/// until when a restored taskbar is shown again if something hides it
static RESTORE_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);
/// how long a restored taskbar is watched
const RESTORE_WATCH: Duration = Duration::from_secs(4);
/// thread running the WinEvent hook, 0 when none
static HOOK_THREAD: AtomicU32 = AtomicU32::new(0);

/// when this process started (heartbeat times are counted from it)
static START: LazyLock<Instant> = LazyLock::new(Instant::now);
/// last heartbeat of the window thread, in ms since `START` (0 = none yet)
static HEARTBEAT: AtomicU64 = AtomicU64::new(0);
/// a dock or a toolbar page has loaded
static UI_READY: AtomicBool = AtomicBool::new(false);
/// the taskbar is shown because the window thread stopped responding
static SUSPENDED: AtomicBool = AtomicBool::new(false);
/// the window thread counts as hung after this long without a heartbeat
const UI_TIMEOUT: Duration = Duration::from_secs(8);
/// with nothing to show (no dock, no toolbar), hide once the window thread has run this long
const NOTHING_TO_SHOW_AFTER: Duration = Duration::from_secs(10);

fn since_start() -> u64 {
    START.elapsed().as_millis() as u64
}

/// Called on the window thread (the event loop) about once a second.
pub fn heartbeat() {
    HEARTBEAT.store(since_start().max(1), Ordering::Release);
}

fn ui_alive() -> bool {
    let last = HEARTBEAT.load(Ordering::Acquire);
    last != 0 && since_start().saturating_sub(last) < UI_TIMEOUT.as_millis() as u64
}

/// A dock or toolbar page loaded: the taskbar may go (if the option is on).
pub fn ui_ready() {
    if !UI_READY.swap(true, Ordering::AcqRel) && HIDDEN.load(Ordering::Acquire) && may_hide() {
        apply(true);
    }
}

/// Whether hiding is safe right now: SideDock is visibly running.
fn may_hide() -> bool {
    ui_alive() && (UI_READY.load(Ordering::Acquire) || START.elapsed() >= NOTHING_TO_SHOW_AFTER)
}

const CLASSES: [&str; 2] = ["Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

fn marker() -> PathBuf {
    paths::get().runtime.join("taskbar-hidden")
}

fn class_of(hwnd: HWND) -> String {
    let mut buf = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..usize::try_from(len).unwrap_or(0).min(buf.len())])
}

fn is_taskbar(hwnd: HWND) -> bool {
    CLASSES.contains(&class_of(hwnd).as_str())
}

/// Every taskbar window (one per monitor).
///
/// Not `EnumWindows`: while Start, quick settings or the notification center is open,
/// Explorer moves the taskbar to a higher window band, which `EnumWindows` does not list
/// for ordinary processes. The taskbar then looked gone: it could be neither hidden nor
/// restored, and the toolbar lost its place. `FindWindowEx` sees every band.
pub fn taskbars() -> Vec<isize> {
    let mut list: Vec<isize> = Vec::new();
    for class in CLASSES {
        let class = HSTRING::from(class);
        let mut after: Option<HWND> = None;
        // the bound only guards against a window list changing under us
        for _ in 0..64 {
            let Ok(hwnd) = (unsafe { FindWindowExW(None, after, &class, PCWSTR::null()) }) else {
                break;
            };
            list.push(hwnd.0 as isize);
            after = Some(hwnd);
        }
    }
    list
}

/// Rect of the taskbar on a monitor, if one sits there (also while it is hidden).
pub fn taskbar_on_monitor(monitor_rect: &Rect) -> Option<Rect> {
    taskbars().into_iter().find_map(|hwnd| {
        // the plain window rect: DWM's frame bounds are for windows on screen
        let mut raw = windows::Win32::Foundation::RECT::default();
        unsafe { GetWindowRect(HWND(hwnd as *mut _), &mut raw) }.ok()?;
        let rect: Rect = raw.into();
        let cx = (rect.left + rect.right) / 2;
        let cy = (rect.top + rect.bottom) / 2;
        (cx >= monitor_rect.left
            && cx < monitor_rect.right
            && cy >= monitor_rect.top
            && cy < monitor_rect.bottom)
            .then_some(rect)
    })
}

pub fn is_hidden() -> bool {
    HIDDEN.load(Ordering::Acquire)
}

/// Hides / shows the taskbars whose visibility differs. Returns true if one did.
fn apply(hidden: bool) -> bool {
    let mut changed = false;
    for hwnd in taskbars() {
        let h = HWND(hwnd as *mut _);
        let visible = unsafe { IsWindowVisible(h) }.as_bool();
        if hidden == visible {
            changed = true;
            unsafe {
                let _ = ShowWindowAsync(h, if hidden { SW_HIDE } else { SW_SHOWNA });
            }
        }
    }
    changed
}

/// Asks Explorer to show every taskbar, whatever its current state: a pending "hide"
/// may not have been carried out yet, so the current visibility proves nothing.
fn show_all() {
    for hwnd in taskbars() {
        unsafe {
            let _ = ShowWindowAsync(HWND(hwnd as *mut _), SW_SHOWNA);
        }
    }
}

unsafe extern "system" fn on_show(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    id_object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object == 0 && HIDDEN.load(Ordering::Acquire) && may_hide() && is_taskbar(hwnd) {
        unsafe {
            let _ = ShowWindowAsync(hwnd, SW_HIDE);
        }
    }
}

fn watch() {
    // immediate: hide again as soon as Explorer shows a taskbar
    if HOOK_THREAD.load(Ordering::Acquire) == 0 {
        crate::utils::spawn_named("taskbar-show-hook", || unsafe {
            HOOK_THREAD.store(GetCurrentThreadId(), Ordering::Release);
            let hook = SetWinEventHook(
                EVENT_OBJECT_SHOW,
                EVENT_OBJECT_SHOW,
                None,
                Some(on_show),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWinEvent(hook);
            HOOK_THREAD.store(0, Ordering::Release);
        });
    }
    enforce();
}

/// Starts the thread that keeps the taskbar in the wanted state: hidden while the
/// option is on (Explorer restarts create new windows, some shows are not reported),
/// and visible for `RESTORE_WATCH` after it is turned off (see the module docs).
fn enforce() {
    if POLLING.swap(true, Ordering::AcqRel) {
        return;
    }
    crate::utils::spawn_named("taskbar-visibility", || {
        let mut tick: u32 = 0;
        let mut previous = Instant::now();
        loop {
            tick = tick.wrapping_add(1);
            // the whole process was suspended (sleep, hibernation): not a hung window thread
            let now = Instant::now();
            if now.duration_since(previous) > Duration::from_secs(2) {
                heartbeat();
            }
            previous = now;
            if HIDDEN.load(Ordering::Acquire) {
                if may_hide() {
                    if SUSPENDED.swap(false, Ordering::AcqRel) {
                        log::info!("SideDock responds again: native taskbar hidden again");
                        apply(true);
                    }
                    // the hook is the fast path: once a second is enough here
                    if tick.is_multiple_of(5) {
                        apply(true);
                    }
                } else {
                    // starting, or SideDock's windows stopped responding: keep the
                    // taskbar until SideDock can be seen and used
                    if UI_READY.load(Ordering::Acquire)
                        && !SUSPENDED.swap(true, Ordering::AcqRel)
                    {
                        log::warn!(
                            "SideDock's window thread is not responding: native taskbar shown until it recovers"
                        );
                    }
                    apply(false);
                }
            } else {
                let watching = RESTORE_UNTIL.lock().is_some_and(|t| Instant::now() < t);
                if !watching {
                    POLLING.store(false, Ordering::Release);
                    // switched again meanwhile and nobody else watching: keep going
                    let idle = !HIDDEN.load(Ordering::Acquire)
                        && !RESTORE_UNTIL.lock().is_some_and(|t| Instant::now() < t);
                    if idle || POLLING.swap(true, Ordering::AcqRel) {
                        break;
                    }
                    continue;
                }
                if apply(false) {
                    log::warn!("native taskbar was hidden again after being restored: shown");
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    });
}

fn stop_watching() {
    let thread = HOOK_THREAD.load(Ordering::Acquire);
    if thread != 0 {
        unsafe {
            let _ = PostThreadMessageW(thread, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

/// Hides or restores the native taskbar (keeps its reserved space either way).
pub fn set_hidden(hidden: bool) {
    let _switch = SWITCH.lock();
    let was = HIDDEN.swap(hidden, Ordering::AcqRel);
    if was == hidden {
        return;
    }
    if hidden {
        *RESTORE_UNTIL.lock() = None;
        let _ = std::fs::write(marker(), b"1");
        if may_hide() {
            apply(true);
            log::info!("native taskbar hidden (its space stays reserved)");
        } else {
            log::info!("native taskbar will be hidden once SideDock is on screen");
        }
        watch();
    } else {
        stop_watching();
        *RESTORE_UNTIL.lock() = Some(Instant::now() + RESTORE_WATCH);
        show_all();
        // short synchronous check, so quitting right after this still leaves it shown
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(40));
            apply(false);
        }
        enforce();
        let _ = std::fs::remove_file(marker());
        log::info!("native taskbar restored");
    }
}

/// Applies the option at startup. Unlike `set_hidden`, it does not trust the current
/// state: a taskbar hidden by another SideDock copy (other folder, killed together with
/// its guardian) or left over by a crash is shown again when the option is off.
pub fn init(hidden: bool) {
    LazyLock::force(&START);
    if hidden {
        set_hidden(true);
        return;
    }
    if apply(false) {
        log::warn!("native taskbar was hidden (another SideDock copy or an earlier crash): shown");
    }
    let _ = std::fs::remove_file(marker());
}

/// Used by the guardian after a crash: shows the taskbar again if SideDock hid it.
pub fn restore_after_crash() {
    if marker().exists() {
        show_all();
        let _ = std::fs::remove_file(marker());
        log::info!("native taskbar restored after an unexpected exit");
    }
}
