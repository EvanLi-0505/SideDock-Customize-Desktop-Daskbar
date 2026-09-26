// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod error;
mod logger;
mod modules;
mod paths;
mod session;
mod state;
mod tray;
mod utils;
mod widgets;
mod windows_api;

use std::time::Duration;

use windows::{
    Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
        UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
    },
    core::HSTRING,
};

/// `SideDock.exe --wait-for <pid>`: used by restart, waits for the old instance to exit.
pub const WAIT_FOR_ARG: &str = "--wait-for";
/// Marker file asking the next start to wipe the WebView2 profile.
pub const CLEAR_USERDATA_MARKER: &str = "clear-userdata";

pub fn fatal(message: &str) {
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            &HSTRING::from("SideDock"),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn arg_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let idx = args.iter().position(|a| a == name)?;
    args.get(idx + 1).map(String::as_str)
}

fn wait_for_process(pid: u32, timeout: Duration) {
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            let _ = WaitForSingleObject(handle, timeout.as_millis() as u32);
            let _ = CloseHandle(handle);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let paths = match paths::init() {
        Ok(paths) => paths,
        Err(err) => {
            fatal(&err.to_string());
            return;
        }
    };
    logger::init(paths.logs.clone());

    if let Some(pid) = arg_value(&args, session::GUARDIAN_ARG).and_then(|p| p.parse().ok()) {
        session::guardian_main(pid);
        return;
    }

    if let Some(pid) = arg_value(&args, WAIT_FOR_ARG).and_then(|p| p.parse().ok()) {
        wait_for_process(pid, Duration::from_secs(15));
    }

    session::install_panic_hook();
    log::info!(
        "SideDock {} starting (data: {}, args: {:?})",
        env!("CARGO_PKG_VERSION"),
        paths.root.display(),
        &args[1..]
    );
    app::run();
}
