//! Optional takeover of the Win key: a single press opens the SideDock launcher instead
//! of the Windows start menu; Win + any other key is left to Windows.
//!
//! A non-elevated process cannot keep the Windows 11 start menu from opening, so the
//! keyboard hook runs in a tiny elevated helper (`win_key_helper.rs`, same executable,
//! `--win-key-helper`). Everything else - dock, launcher, settings - stays non-elevated.
//!
//! The helper is started through a scheduled task ("SideDock Win Key", run with highest
//! privileges, no trigger: it only ever runs on demand), which is how it gets
//! administrator rights without a UAC prompt each time:
//!
//! * turning the option on registers the task once (one UAC prompt, `--win-key-register`);
//! * while SideDock runs with the option on, it starts the task; the helper attaches to
//!   this process and exits with it (or when the stop event is signalled);
//! * turning the option off stops the helper and deletes the task (`--win-key-unregister`).
//!
//! The task definition is the only thing SideDock ever writes outside its own folder
//! (`C:\Windows\System32\Tasks\SideDock Win Key`, a few KB), and only while the option
//! is on. The helper talks to us by posting messages to a message-only window
//! ([`TARGET_CLASS`]): an elevated process may post to a non-elevated one.

use std::{
    os::windows::process::CommandExt,
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::AppHandle;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM, LRESULT, WAIT_OBJECT_0, WPARAM},
        System::Threading::{
            CreateEventW, GetExitCodeProcess, ResetEvent, SetEvent, WaitForSingleObject,
        },
        UI::{
            Shell::{
                SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
                RegisterClassW, SW_HIDE, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP,
                WNDCLASSW,
            },
        },
    },
    core::{HSTRING, PCWSTR},
};

use crate::{app, error::Result, paths, state::settings};

pub const HELPER_ARG: &str = "--win-key-helper";
pub const REGISTER_ARG: &str = "--win-key-register";
pub const UNREGISTER_ARG: &str = "--win-key-unregister";
pub const TASK_NAME: &str = "SideDock Win Key";
/// class of the message-only window the helper posts to
pub const TARGET_CLASS: &str = "SideDockWinKeyTarget";
pub const MSG_LONE_WIN_PRESS: u32 = WM_APP + 0x51;
pub const MSG_HELPER_READY: u32 = WM_APP + 0x52;
pub const EVENT_STATUS: &str = "win-key-status-changed";

/// Named event the helper of SideDock process `pid` waits on to stop.
pub fn stop_event_name(pid: u32) -> String {
    format!("Local\\SideDockWinKeyStop-{pid}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WinKeyStatus {
    Off,
    /// the option is on but the task is missing (or registered for another SideDock path)
    NeedsAuthorization,
    Starting,
    Active,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WinKeyState {
    pub status: WinKeyStatus,
    /// the scheduled task exists (also while the option is off, if removing it failed)
    pub task_registered: bool,
}

static APP: OnceLock<AppHandle> = OnceLock::new();
static STATUS: Mutex<WinKeyStatus> = Mutex::new(WinKeyStatus::Off);
static STOP_EVENT: Mutex<isize> = Mutex::new(0);
static TARGET_STARTED: AtomicBool = AtomicBool::new(false);
static HELPER_LOCK: Mutex<()> = Mutex::new(());

// ======================= task registration (elevated) =======================

/// Remembers which executable the task was registered for (moving the SideDock folder
/// needs a new registration).
fn marker() -> PathBuf {
    paths::get().runtime.join("win-key-task.txt")
}

fn task_matches_exe() -> bool {
    std::fs::read_to_string(marker())
        .is_ok_and(|p| std::path::Path::new(p.trim()) == paths::get().exe)
}

fn schtasks(args: &[&str]) -> bool {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("schtasks.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// No trigger (on demand only), highest privileges, no time limit, also on battery.
fn task_xml(user: &str) -> String {
    let exe = paths::get().exe.to_string_lossy().to_string();
    let dir = paths::get()
        .exe
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>SideDock: lets the Win key open the SideDock launcher. Runs only while SideDock runs. Turn the option off in SideDock to remove this task.</Description>
  </RegistrationInfo>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{arg}</Arguments>
      <WorkingDirectory>{dir}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#,
        user = xml_escape(user),
        exe = xml_escape(&exe),
        dir = xml_escape(&dir),
        arg = HELPER_ARG,
    )
}

/// `SideDock.exe --win-key-register <DOMAIN\user>`, run elevated. Returns the exit code.
pub fn register_main(user: &str) -> i32 {
    // the definition file lives in SideDock's own folder and is deleted right away
    let file = paths::get().runtime.join("win-key-task.xml");
    let mut bytes = vec![0xFF, 0xFE]; // UTF-16 LE BOM, required by schtasks
    for unit in task_xml(user).encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    if let Err(err) = std::fs::write(&file, bytes) {
        log::error!("Win key task: cannot write the definition: {err}");
        return 2;
    }
    let ok = schtasks(&[
        "/Create",
        "/TN",
        TASK_NAME,
        "/XML",
        &file.to_string_lossy(),
        "/F",
    ]);
    let _ = std::fs::remove_file(&file);
    if !ok {
        log::error!("Win key task: registration failed");
        return 1;
    }
    let _ = std::fs::write(marker(), paths::get().exe.to_string_lossy().as_bytes());
    log::info!("Win key task registered for {user}");
    0
}

/// `SideDock.exe --win-key-unregister`, run elevated. Returns the exit code.
pub fn unregister_main() -> i32 {
    let ok =
        schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]) || !schtasks(&["/Query", "/TN", TASK_NAME]);
    if ok {
        let _ = std::fs::remove_file(marker());
        log::info!("Win key task removed");
        0
    } else {
        log::error!("Win key task: removal failed");
        1
    }
}

/// Runs this executable elevated (UAC prompt) with `args` and waits for it.
fn run_elevated(args: &str) -> Result<()> {
    let exe = HSTRING::from(paths::get().exe.as_os_str());
    let verb = HSTRING::from("runas");
    let params = HSTRING::from(args);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(exe.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe {
        if ShellExecuteExW(&mut info).is_err() {
            return Err("administrator permission was not granted".into());
        }
        let process = info.hProcess;
        if process.is_invalid() {
            return Err("the elevated helper did not start".into());
        }
        let waited = WaitForSingleObject(process, 60_000);
        let mut code = 1u32;
        let _ = GetExitCodeProcess(process, &mut code);
        let _ = CloseHandle(process);
        if waited != WAIT_OBJECT_0 || code != 0 {
            return Err(format!("the scheduled task could not be changed (code {code})").into());
        }
    }
    Ok(())
}

fn current_user() -> String {
    let user = std::env::var("USERNAME").unwrap_or_default();
    match std::env::var("USERDOMAIN") {
        Ok(domain) if !domain.is_empty() => format!("{domain}\\{user}"),
        _ => user,
    }
}

// ======================= status =======================

pub fn state() -> WinKeyState {
    WinKeyState {
        status: *STATUS.lock(),
        task_registered: marker().exists(),
    }
}

fn set_status(status: WinKeyStatus) {
    let changed = {
        let mut current = STATUS.lock();
        let changed = *current != status;
        *current = status;
        changed
    };
    if changed {
        log::info!("Win key takeover: {status:?}");
        app::emit(EVENT_STATUS, &state());
    }
}

// ======================= target window (messages from the helper) =======================

unsafe extern "system" fn target_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        MSG_LONE_WIN_PRESS => {
            if let Some(app) = APP.get() {
                let app = app.clone();
                std::thread::spawn(move || crate::widgets::overlay::toggle_launcher(&app));
            }
            LRESULT(0)
        }
        MSG_HELPER_READY => {
            set_status(WinKeyStatus::Active);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(h, msg, wp, lp) },
    }
}

fn ensure_target_window() {
    if TARGET_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    crate::utils::spawn_named("win-key-target", || unsafe {
        let class = HSTRING::from(TARGET_CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(target_proc),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let created = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        );
        if created.is_err() {
            log::warn!("Win key: message window not created: {created:?}");
            TARGET_STARTED.store(false, Ordering::Release);
            return;
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

// ======================= helper lifetime =======================

fn stop_event() -> HANDLE {
    let mut event = STOP_EVENT.lock();
    if *event == 0 {
        let name = HSTRING::from(stop_event_name(std::process::id()));
        if let Ok(h) = unsafe { CreateEventW(None, true, false, &name) } {
            *event = h.0 as isize;
        }
    }
    HANDLE(*event as *mut _)
}

fn start_helper() {
    let _guard = HELPER_LOCK.lock();
    if matches!(
        *STATUS.lock(),
        WinKeyStatus::Active | WinKeyStatus::Starting
    ) {
        return;
    }
    ensure_target_window();
    unsafe {
        let _ = ResetEvent(stop_event());
    }
    set_status(WinKeyStatus::Starting);
    std::thread::sleep(Duration::from_millis(100)); // let the target window come up
    if !schtasks(&["/Run", "/TN", TASK_NAME]) {
        log::warn!("Win key: the scheduled task could not be started");
        set_status(WinKeyStatus::Failed);
        return;
    }
    // the helper reports back once its hook is installed
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(8));
        if *STATUS.lock() == WinKeyStatus::Starting {
            log::warn!("Win key: the helper did not report back");
            set_status(WinKeyStatus::Failed);
        }
    });
}

fn stop_helper() {
    let _guard = HELPER_LOCK.lock();
    unsafe {
        let _ = SetEvent(stop_event());
    }
}

/// Starts or stops the helper to match the settings (no UAC prompt here).
pub fn sync(app: &AppHandle) {
    let _ = APP.set(app.clone());
    let wanted = settings::get().launcher.wants_win_key();
    if !wanted {
        stop_helper();
        set_status(WinKeyStatus::Off);
        return;
    }
    if !task_matches_exe() {
        set_status(WinKeyStatus::NeedsAuthorization);
        return;
    }
    std::thread::spawn(start_helper);
}

/// Turns the option on: registers the task when needed (UAC prompt), then starts.
pub fn enable(app: &AppHandle) -> Result<WinKeyState> {
    if !task_matches_exe() {
        run_elevated(&format!("{REGISTER_ARG} \"{}\"", current_user()))?;
    }
    let mut s = settings::get();
    s.launcher.take_over_win_key = true;
    app::apply_settings(app, s)?;
    sync(app);
    Ok(state())
}

/// Turns the option off, stops the helper and removes the task (UAC prompt).
pub fn disable(app: &AppHandle) -> Result<WinKeyState> {
    let mut s = settings::get();
    s.launcher.take_over_win_key = false;
    app::apply_settings(app, s)?;
    stop_helper();
    set_status(WinKeyStatus::Off);
    if marker().exists() || schtasks(&["/Query", "/TN", TASK_NAME]) {
        // deleting a task that runs with highest privileges needs administrator rights
        if !schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]) {
            run_elevated(UNREGISTER_ARG)?;
        }
        let _ = std::fs::remove_file(marker());
    }
    app::emit(EVENT_STATUS, &state());
    Ok(state())
}

pub fn shutdown() {
    stop_helper();
}
