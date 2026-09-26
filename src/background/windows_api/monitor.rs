use serde::Serialize;
use windows::{
    Win32::{
        Foundation::POINT,
        Foundation::{LPARAM, RECT},
        Graphics::Gdi::{
            EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST,
            MONITORINFO, MonitorFromPoint,
        },
        UI::{
            HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
            WindowsAndMessaging::MONITORINFOF_PRIMARY,
        },
    },
    core::BOOL,
};

use super::window::Rect;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    /// HMONITOR value, stable while the display configuration does not change
    pub id: isize,
    pub rect: Rect,
    pub work_area: Rect,
    pub scale_factor: f64,
    pub is_primary: bool,
}

pub fn list() -> Vec<MonitorInfo> {
    unsafe extern "system" fn callback(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let list = unsafe { &mut *(lparam.0 as *mut Vec<HMONITOR>) };
        list.push(monitor);
        true.into()
    }

    let mut handles: Vec<HMONITOR> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(callback),
            LPARAM(&mut handles as *mut _ as isize),
        );
    }
    handles.into_iter().filter_map(info).collect()
}

pub fn info(handle: HMONITOR) -> Option<MonitorInfo> {
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(handle, &mut mi) }.as_bool() {
        return None;
    }
    let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
    let _ = unsafe { GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    Some(MonitorInfo {
        id: handle.0 as isize,
        rect: mi.rcMonitor.into(),
        work_area: mi.rcWork.into(),
        scale_factor: dpi_x as f64 / 96.0,
        is_primary: mi.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}

pub fn by_id(id: isize) -> Option<MonitorInfo> {
    info(HMONITOR(id as *mut _))
}

pub fn from_point(x: i32, y: i32) -> Option<MonitorInfo> {
    info(unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) })
}
