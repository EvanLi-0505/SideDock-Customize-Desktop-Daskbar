use std::path::PathBuf;

use serde::Serialize;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, RECT, WPARAM},
        Graphics::{
            Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
            Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
        },
        Storage::EnhancedStorage::PKEY_AppUserModel_ID,
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
            QueryFullProcessImageNameW, TerminateProcess,
        },
        UI::{
            Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
                SendInput, VK_MENU,
            },
            Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow},
            WindowsAndMessaging::{
                EnumWindows, GW_OWNER, GWL_EXSTYLE, GWL_STYLE, GetClassNameW, GetForegroundWindow,
                GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
                GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, IsZoomed,
                PostMessageW, SW_MINIMIZE, SW_RESTORE, SetForegroundWindow, ShowWindowAsync,
                WM_CLOSE, WS_CHILD, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
            },
        },
    },
    core::BOOL,
};

use super::from_wide;
use crate::error::Result;

/// Desktop/taskbar windows and Edge's per-tab proxies are never user applications.
const SHELL_CLASSES: &[&str] = &[
    "Windows.Internal.Shell.TabProxyWindow",
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window(pub isize);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl From<RECT> for Rect {
    fn from(r: RECT) -> Self {
        Self {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

impl From<Rect> for RECT {
    fn from(r: Rect) -> Self {
        RECT {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
    pub fn intersects(&self, other: &Rect) -> bool {
        !(self.right <= other.left
            || self.left >= other.right
            || self.bottom <= other.top
            || self.top >= other.bottom)
    }
}

impl From<HWND> for Window {
    fn from(hwnd: HWND) -> Self {
        Self(hwnd.0 as isize)
    }
}

impl Window {
    pub fn hwnd(self) -> HWND {
        HWND(self.0 as *mut _)
    }

    pub fn foreground() -> Option<Self> {
        let hwnd = unsafe { GetForegroundWindow() };
        (!hwnd.is_invalid()).then(|| Self::from(hwnd))
    }

    pub fn is_window(self) -> bool {
        unsafe { IsWindow(Some(self.hwnd())).as_bool() }
    }

    pub fn is_visible(self) -> bool {
        unsafe { IsWindowVisible(self.hwnd()).as_bool() }
    }

    pub fn is_minimized(self) -> bool {
        unsafe { IsIconic(self.hwnd()).as_bool() }
    }

    pub fn is_maximized(self) -> bool {
        unsafe { IsZoomed(self.hwnd()).as_bool() }
    }

    pub fn is_cloaked(self) -> bool {
        let mut cloaked: u32 = 0;
        let res = unsafe {
            DwmGetWindowAttribute(
                self.hwnd(),
                DWMWA_CLOAKED,
                &mut cloaked as *mut u32 as *mut _,
                std::mem::size_of::<u32>() as u32,
            )
        };
        res.is_ok() && cloaked != 0
    }

    pub fn title(self) -> String {
        let len = unsafe { GetWindowTextLengthW(self.hwnd()) };
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let copied = unsafe { GetWindowTextW(self.hwnd(), &mut buf) };
        String::from_utf16_lossy(&buf[..copied.max(0) as usize])
    }

    pub fn class(self) -> String {
        let mut buf = [0u16; 256];
        let len = unsafe { GetClassNameW(self.hwnd(), &mut buf) };
        String::from_utf16_lossy(&buf[..len.max(0) as usize])
    }

    pub fn style(self) -> u32 {
        unsafe { GetWindowLongPtrW(self.hwnd(), GWL_STYLE) as u32 }
    }

    pub fn ex_style(self) -> u32 {
        unsafe { GetWindowLongPtrW(self.hwnd(), GWL_EXSTYLE) as u32 }
    }

    pub fn owner(self) -> Option<Window> {
        let owner = unsafe { GetWindow(self.hwnd(), GW_OWNER) }.ok()?;
        (!owner.is_invalid()).then(|| Window::from(owner))
    }

    pub fn process_id(self) -> u32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(self.hwnd(), Some(&mut pid)) };
        pid
    }

    pub fn thread_id(self) -> u32 {
        unsafe { GetWindowThreadProcessId(self.hwnd(), None) }
    }

    pub fn exe_path(self) -> Option<PathBuf> {
        process_image_path(self.process_id())
    }

    /// Visible bounds without the invisible resize borders.
    pub fn rect(self) -> Option<Rect> {
        let mut rect = RECT::default();
        let ok = unsafe {
            DwmGetWindowAttribute(
                self.hwnd(),
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut rect as *mut RECT as *mut _,
                std::mem::size_of::<RECT>() as u32,
            )
        };
        if ok.is_err() {
            unsafe { GetWindowRect(self.hwnd(), &mut rect) }.ok()?;
        }
        Some(rect.into())
    }

    pub fn monitor(self) -> isize {
        unsafe { MonitorFromWindow(self.hwnd(), MONITOR_DEFAULTTONEAREST).0 as isize }
    }

    /// A window covering its whole monitor without being maximized (games, videos, F11).
    pub fn is_fullscreen(self) -> bool {
        let Some(rect) = self.rect() else {
            return false;
        };
        let monitor = unsafe { MonitorFromWindow(self.hwnd(), MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            return false;
        }
        let m: Rect = info.rcMonitor.into();
        rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom
    }

    pub fn app_user_model_id(self) -> Option<String> {
        let store: IPropertyStore = unsafe { SHGetPropertyStoreForWindow(self.hwnd()) }.ok()?;
        let value = unsafe { store.GetValue(&PKEY_AppUserModel_ID) }.ok()?;
        let text = value.to_string();
        (!text.is_empty()).then_some(text)
    }

    pub fn minimize(self) {
        unsafe {
            let _ = ShowWindowAsync(self.hwnd(), SW_MINIMIZE);
        }
    }

    pub fn close(self) -> Result<()> {
        unsafe { PostMessageW(Some(self.hwnd()), WM_CLOSE, WPARAM(0), LPARAM(0))? };
        Ok(())
    }

    pub fn kill_process(self) -> Result<()> {
        let pid = self.process_id();
        if pid == std::process::id() {
            return Err("refusing to kill SideDock itself".into());
        }
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, false, pid)?;
            let res = TerminateProcess(handle, 1);
            let _ = CloseHandle(handle);
            res?;
        }
        Ok(())
    }

    /// Restores (if minimized) and brings the window to the foreground.
    ///
    /// Windows only lets the foreground process change the foreground window; a
    /// synthetic ALT press is the documented-by-practice way to be granted that right.
    pub fn focus(self) -> Result<()> {
        if self.is_minimized() {
            unsafe {
                let _ = ShowWindowAsync(self.hwnd(), SW_RESTORE);
            }
        }
        let alt = |flags: KEYBD_EVENT_FLAGS| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_MENU,
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        };
        unsafe {
            SendInput(
                &[alt(KEYBD_EVENT_FLAGS(0)), alt(KEYEVENTF_KEYUP)],
                std::mem::size_of::<INPUT>() as i32,
            );
            if !SetForegroundWindow(self.hwnd()).as_bool() {
                return Err("SetForegroundWindow was refused".into());
            }
        }
        Ok(())
    }

    /// Mirrors the "shown on the taskbar" rules used by Explorer (and by Seelen UI).
    pub fn is_app_window(self) -> bool {
        if !self.is_window() || !self.is_visible() || self.is_cloaked() {
            return false;
        }
        if self.title().is_empty() {
            return false;
        }
        let class = self.class();
        if SHELL_CLASSES.contains(&class.as_str()) {
            return false;
        }
        let style = self.style();
        let ex_style = self.ex_style();
        if ex_style & WS_EX_APPWINDOW.0 == 0 {
            if style & WS_CHILD.0 != 0 || self.owner().is_some() {
                return false;
            }
            if ex_style & WS_EX_TOOLWINDOW.0 != 0 || ex_style & WS_EX_NOACTIVATE.0 != 0 {
                return false;
            }
        }
        // our own windows (dock, popups, settings) are never listed
        if self.process_id() == std::process::id() {
            return false;
        }
        // Shell hosts living in `C:\Windows\SystemApps` (start menu, search, action center,
        // lock screen...) cover the whole screen while open; treating them as apps would
        // make the dock think it is overlapped. Same rule as Seelen UI's "no-interactive".
        if self.exe_path().is_some_and(|p| {
            p.to_string_lossy()
                .to_ascii_lowercase()
                .contains("\\windows\\systemapps\\")
        }) {
            return false;
        }
        true
    }

    /// All top level windows in z-order (top-most first).
    pub fn enumerate() -> Vec<Window> {
        unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let list = unsafe { &mut *(lparam.0 as *mut Vec<Window>) };
            list.push(Window::from(hwnd));
            true.into()
        }
        let mut list: Vec<Window> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(callback), LPARAM(&mut list as *mut _ as isize));
        }
        list
    }
}

pub fn process_image_path(pid: u32) -> Option<PathBuf> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let res = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        res.ok()?;
        Some(PathBuf::from(from_wide(&buf[..len as usize])))
    }
}
