//! CPU usage, memory usage and network throughput for the toolbar's status items.
//!
//! CPU and network figures are rates: each comes from the difference between two
//! samples, so a `PerfSampler` is kept between calls (once a second by the system
//! poller). Everything here is a cheap system call, no performance counters.

use std::time::Instant;

use serde::Serialize;
use windows::Win32::{
    Foundation::FILETIME,
    NetworkManagement::{
        IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2},
        Ndis::IfOperStatusUp,
    },
    System::{
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::GetSystemTimes,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatus {
    /// 0-100
    pub percent: u32,
    /// bytes
    pub used: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetSpeed {
    /// bytes per second, all hardware network adapters together
    pub down: u64,
    pub up: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfStatus {
    /// 0-100; `None` until two samples were taken
    pub cpu: Option<u32>,
    pub memory: Option<MemoryStatus>,
    /// `None` until two samples were taken
    pub net: Option<NetSpeed>,
}

#[derive(Default)]
pub struct PerfSampler {
    /// (idle, kernel + user) in 100 ns units
    cpu: Option<(u64, u64)>,
    /// (received, sent, when)
    net: Option<(u64, u64, Instant)>,
}

fn ticks(t: FILETIME) -> u64 {
    (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
}

impl PerfSampler {
    pub fn sample(&mut self) -> PerfStatus {
        PerfStatus {
            cpu: self.cpu(),
            memory: memory(),
            net: self.net(),
        }
    }

    fn cpu(&mut self) -> Option<u32> {
        let (mut idle, mut kernel, mut user) = Default::default();
        unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
        // kernel time includes the idle time
        let now = (ticks(idle), ticks(kernel) + ticks(user));
        let previous = self.cpu.replace(now)?;
        let total = now.1.saturating_sub(previous.1);
        let idle = now.0.saturating_sub(previous.0);
        (total > 0).then(|| (100 * total.saturating_sub(idle) / total).min(100) as u32)
    }

    fn net(&mut self) -> Option<NetSpeed> {
        let (down, up) = octets()?;
        let now = Instant::now();
        let previous = self.net.replace((down, up, now))?;
        let seconds = now.duration_since(previous.2).as_secs_f64();
        if seconds <= 0.0 {
            return None;
        }
        // a counter that went backwards (adapter reset): no reading this time
        let rate = |now: u64, before: u64| (now.saturating_sub(before) as f64 / seconds) as u64;
        Some(NetSpeed {
            down: rate(down, previous.0),
            up: rate(up, previous.1),
        })
    }
}

fn memory() -> Option<MemoryStatus> {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
    let total = status.ullTotalPhys;
    Some(MemoryStatus {
        percent: status.dwMemoryLoad,
        used: total.saturating_sub(status.ullAvailPhys),
        total,
    })
}

/// Bytes received / sent so far by the hardware adapters that are up. Virtual and
/// filter interfaces repeat the traffic of the adapter below them, so they are skipped.
fn octets() -> Option<(u64, u64)> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    unsafe { GetIfTable2(&mut table) }.ok().ok()?;
    if table.is_null() {
        return None;
    }
    let (mut down, mut up) = (0u64, 0u64);
    unsafe {
        let count = (*table).NumEntries as usize;
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), count);
        for row in rows {
            // bit 0 of the flags: HardwareInterface
            let hardware = row.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
            if hardware && row.OperStatus == IfOperStatusUp {
                down = down.saturating_add(row.InOctets);
                up = up.saturating_add(row.OutOctets);
            }
        }
        FreeMibTable(table as *const _);
    }
    Some((down, up))
}
