//! Session bookkeeping and crash recovery.
//!
//! * `runtime/session.lock` exists while SideDock runs. Finding it at startup means the
//!   previous run did not exit cleanly.
//! * A tiny guardian process (`SideDock.exe --guardian <pid>`, no webview, ~1 MB) waits
//!   for the main process. If the main process dies without removing the lock (crash,
//!   killed from Task Manager) the guardian removes the AppBar space it reserved and,
//!   when enabled, relaunches SideDock. Crash loops are detected and stopped.
//! * A panic hook logs the panic with a backtrace and releases the AppBars right away.

use std::{
    os::windows::process::CommandExt,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use windows::Win32::{
    Foundation::{CloseHandle, WAIT_OBJECT_0},
    System::Threading::{INFINITE, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    UI::WindowsAndMessaging::{GetSystemMetrics, SM_SHUTTINGDOWN},
};

use crate::{error::Result, paths, windows_api::app_bar};

pub const GUARDIAN_ARG: &str = "--guardian";
pub const RECOVERED_ARG: &str = "--recovered";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;
/// More than this many crashes within [`CRASH_WINDOW_SECS`] stops auto relaunch.
const MAX_CRASHES: usize = 3;
const CRASH_WINDOW_SECS: u64 = 120;

static PREVIOUS_SESSION_CRASHED: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize)]
struct LockFile {
    pid: u32,
    started_at: u64,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn lock_path() -> PathBuf {
    paths::get().runtime.join("session.lock")
}

fn crashes_path() -> PathBuf {
    paths::get().runtime.join("crashes.json")
}

pub fn previous_session_crashed() -> bool {
    PREVIOUS_SESSION_CRASHED.load(Ordering::Relaxed)
}

/// Marks the session as running and spawns the guardian.
pub fn begin(spawn_guardian: bool) -> Result<()> {
    let lock = lock_path();
    if lock.exists() {
        PREVIOUS_SESSION_CRASHED.store(true, Ordering::Relaxed);
        log::warn!("previous session did not exit cleanly");
        // the guardian normally cleans these, but it may have been killed too
        if let Ok(text) = std::fs::read_to_string(paths::get().runtime.join("appbars.json"))
            && let Ok(stale) = serde_json::from_str::<Vec<isize>>(&text)
        {
            app_bar::remove_stale(&stale);
        }
    }
    std::fs::write(
        &lock,
        serde_json::to_vec(&LockFile {
            pid: std::process::id(),
            started_at: unix_now(),
        })?,
    )?;

    if spawn_guardian {
        let exe = &paths::get().exe;
        match std::process::Command::new(exe)
            .args([GUARDIAN_ARG, &std::process::id().to_string()])
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn()
        {
            Ok(child) => log::info!("guardian started (pid {})", child.id()),
            Err(err) => log::error!("could not start guardian: {err}"),
        }
    }
    Ok(())
}

/// Clean shutdown: releases the AppBars and removes the lock so the guardian exits quietly.
pub fn end() {
    app_bar::unregister_all();
    let _ = std::fs::remove_file(lock_path());
    let _ = std::fs::remove_file(paths::get().runtime.join("appbars.json"));
    log::info!("session ended cleanly");
    log::logger().flush();
}

pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("<unnamed>");
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("panic in thread '{name}': {info}\n{backtrace}");
        if name == "main" {
            // the event loop is gone; free the reserved screen space immediately
            app_bar::unregister_all();
        }
        log::logger().flush();
        default(info);
    }));
}

fn record_crash_and_should_restart() -> bool {
    let now = unix_now();
    let mut crashes: Vec<u64> = std::fs::read_to_string(crashes_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    crashes.retain(|t| now.saturating_sub(*t) < CRASH_WINDOW_SECS);
    crashes.push(now);
    let _ = std::fs::write(
        crashes_path(),
        serde_json::to_vec(&crashes).unwrap_or_default(),
    );
    crashes.len() <= MAX_CRASHES
}

fn crash_recovery_enabled() -> bool {
    crate::state::settings::get().crash_recovery
}

fn system_is_shutting_down() -> bool {
    unsafe { GetSystemMetrics(SM_SHUTTINGDOWN) != 0 }
}

/// Entry point of the guardian process. Never creates a window.
pub fn guardian_main(parent_pid: u32) {
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_SYNCHRONIZE, false, parent_pid) else {
            return;
        };
        let res = WaitForSingleObject(handle, INFINITE);
        let _ = CloseHandle(handle);
        if res != WAIT_OBJECT_0 {
            return;
        }
    }

    let lock = lock_path();
    let Ok(text) = std::fs::read_to_string(&lock) else {
        return; // clean exit
    };
    match serde_json::from_str::<LockFile>(&text) {
        Ok(l) if l.pid == parent_pid => {}
        _ => return, // lock belongs to another (newer) session
    }

    log::error!("SideDock (pid {parent_pid}) exited unexpectedly, cleaning up");
    if let Ok(text) = std::fs::read_to_string(paths::get().runtime.join("appbars.json"))
        && let Ok(stale) = serde_json::from_str::<Vec<isize>>(&text)
    {
        app_bar::remove_stale(&stale);
    }

    // let a logoff/shutdown finish instead of fighting it
    std::thread::sleep(Duration::from_millis(1500));
    if system_is_shutting_down() || !crash_recovery_enabled() {
        return;
    }
    if !record_crash_and_should_restart() {
        log::error!("too many crashes in a short time, not restarting");
        return;
    }
    log::info!("relaunching SideDock");
    let _ = std::process::Command::new(&paths::get().exe)
        .arg(RECOVERED_ARG)
        .creation_flags(DETACHED_PROCESS)
        .spawn();
}
