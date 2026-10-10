//! Toggling the Windows 11 flyouts (quick settings, notification center) from a button.
//!
//! The `ms-actioncenter:` URIs can only open a flyout, and a flyout opened that way stays
//! until it is toggled (it does not close when another window is clicked). The real
//! toggles are the system shortcuts, Win+A and Win+N, which is what this module sends.
//!
//! One case needs care: a flyout opened by its shortcut closes by itself when the user
//! clicks elsewhere, including on the button that should close it. Sending the shortcut
//! then would reopen it at once. The flyouts take the foreground (quick settings is a
//! `ControlCenterWindow` of `ShellHost.exe`, the notification center a core window of
//! `ShellExperienceHost.exe`); this module follows the foreground window (fed by
//! `modules/apps.rs`) to know that a flyout lost it a moment ago, and then checks
//! whether that flyout is really gone (hidden or cloaked) before deciding.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use windows::Win32::{
    Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
    UI::{
        Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_A, VK_LWIN, VK_N},
        WindowsAndMessaging::IsWindowVisible,
    },
};

use crate::windows_api::{
    shell,
    window::{Rect, Window},
};

/// processes hosting the flyouts
const HOSTS: [&str; 2] = ["ShellHost.exe", "ShellExperienceHost.exe"];
/// a flyout that lost the foreground this recently may have been dismissed by the click
const JUST_LOST: Duration = Duration::from_millis(700);
/// longest a dismissed flyout takes to disappear (it stays on screen while it animates)
const CLOSING: Duration = Duration::from_millis(1200);
/// how long after the shortcut the next flyout window is taken for the one asked for
const LEARN_WINDOW: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flyout {
    QuickSettings,
    NotificationCenter,
}

impl Flyout {
    /// The system shortcut that toggles it.
    fn shortcut(self) -> [VIRTUAL_KEY; 2] {
        match self {
            Self::QuickSettings => [VK_LWIN, VK_A],
            Self::NotificationCenter => [VK_LWIN, VK_N],
        }
    }
}

/// A flyout window seen in the foreground.
#[derive(Debug, Clone)]
struct Seen {
    hwnd: isize,
    title: String,
}

struct State {
    /// flyout in the foreground
    current: Option<Seen>,
    /// flyout that last lost the foreground, and when
    lost: Option<(Seen, Instant)>,
    /// flyout whose shortcut was just sent, and when
    toggled: Option<(Flyout, Instant)>,
    last_toggled: Option<Flyout>,
    /// window titles (localized) learned for each flyout: quick settings, notifications
    titles: [Option<String>; 2],
    /// every flyout window seen so far (they are reused): see `open_rects`
    windows: Vec<isize>,
}

impl State {
    fn is(&self, seen: &Seen, flyout: Flyout) -> bool {
        match &self.titles[flyout as usize] {
            Some(known) => *known == seen.title,
            // not seen yet: the last one toggled from here
            None => self.last_toggled == Some(flyout),
        }
    }
}

static STATE: Mutex<State> = Mutex::new(State {
    current: None,
    lost: None,
    toggled: None,
    last_toggled: None,
    titles: [None, None],
    windows: Vec::new(),
});

/// Called for every foreground window change.
pub fn on_foreground(window: Window, exe: Option<&Path>) {
    let is_flyout = exe
        .and_then(|p| p.file_name())
        .is_some_and(|n| HOSTS.iter().any(|host| n.eq_ignore_ascii_case(host)));
    let mut state = STATE.lock();
    if let Some(seen) = state.current.take() {
        state.lost = Some((seen, Instant::now()));
    }
    if !is_flyout {
        return;
    }
    let title = window.title();
    if let Some((flyout, at)) = state.toggled.take()
        && at.elapsed() < LEARN_WINDOW
    {
        state.titles[flyout as usize] = Some(title.clone());
    }
    if !state.windows.contains(&window.0) {
        state.windows.retain(|&h| Window(h).is_window());
        state.windows.push(window.0);
    }
    state.current = Some(Seen {
        hwnd: window.0,
        title,
    });
}

/// Screen rects of the flyouts that are open right now.
///
/// They open against the work area's bottom edge, over anything SideDock shows there
/// (they are above every ordinary window): a toolbar floating above the taskbar moves
/// its content out from under them (`widgets/toolbar.rs`).
pub fn open_rects() -> Vec<Rect> {
    let windows = STATE.lock().windows.clone();
    windows
        .into_iter()
        .filter(|&h| is_open(h))
        .filter_map(|h| Window(h).rect())
        .collect()
}

/// On screen: a closed flyout is hidden (quick settings) or cloaked (notifications).
fn is_open(hwnd: isize) -> bool {
    let window = Window(hwnd);
    if !window.is_window() || !unsafe { IsWindowVisible(window.hwnd()) }.as_bool() {
        return false;
    }
    let mut cloaked: u32 = 0;
    let ok = unsafe {
        DwmGetWindowAttribute(
            window.hwnd(),
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
    };
    ok.is_ok() && cloaked == 0
}

/// Opens the flyout, or closes it when it is open.
pub fn toggle(flyout: Flyout) {
    let just_lost = {
        let state = STATE.lock();
        state
            .lost
            .as_ref()
            .filter(|(seen, at)| at.elapsed() < JUST_LOST && state.is(seen, flyout))
            .map(|(seen, at)| (seen.hwnd, *at))
    };
    if let Some((hwnd, at)) = just_lost {
        // The click on the button took the foreground from this flyout. Usually that
        // closes it, after a short animation: wait for it, and if it goes away the
        // click has done its job.
        let deadline = at + CLOSING;
        while is_open(hwnd) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(30));
        }
        if !is_open(hwnd) {
            log::debug!(
                "{flyout:?}: closed by the click itself ({:?})",
                at.elapsed()
            );
            STATE.lock().lost = None;
            return;
        }
        // still there (opened in a way that does not close by itself): toggle it
        log::debug!("{flyout:?}: still open after the click, toggling");
    }
    {
        let mut state = STATE.lock();
        state.toggled = Some((flyout, Instant::now()));
        state.last_toggled = Some(flyout);
    }
    shell::send_keys(&flyout.shortcut());
}
