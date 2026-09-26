use std::{
    collections::HashMap,
    panic::AssertUnwindSafe,
    sync::LazyLock,
    thread::JoinHandle,
    time::{Duration, Instant},
};

use parking_lot::Mutex;

pub fn spawn_named<F>(name: &str, f: F) -> JoinHandle<()>
where
    F: FnOnce() + Send + 'static,
{
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(f)
        .expect("failed to spawn thread")
}

/// Runs `f` on a background thread and restarts it if it panics, so a bug in one
/// watcher (e.g. a flaky WinRT call) can never take down the whole dock.
/// Gives up after too many crashes in a short time to avoid busy loops.
pub fn spawn_supervised<F>(name: &'static str, f: F) -> JoinHandle<()>
where
    F: Fn() + Send + 'static,
{
    spawn_named(name, move || {
        let mut crashes: Vec<Instant> = Vec::new();
        loop {
            match std::panic::catch_unwind(AssertUnwindSafe(&f)) {
                Ok(()) => return,
                Err(_) => {
                    let now = Instant::now();
                    crashes.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
                    crashes.push(now);
                    if crashes.len() > 5 {
                        log::error!("thread `{name}` keeps crashing, giving up");
                        return;
                    }
                    log::error!("thread `{name}` panicked, restarting");
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    })
}

static THROTTLE: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Returns `false` if `key` was already accepted less than `window` ago.
/// Used to swallow double/triple clicks that would otherwise launch an app several times.
pub fn throttle(key: &str, window: Duration) -> bool {
    let mut map = THROTTLE.lock();
    let now = Instant::now();
    map.retain(|_, t| now.duration_since(*t) < Duration::from_secs(30));
    match map.get(key) {
        Some(last) if now.duration_since(*last) < window => false,
        _ => {
            map.insert(key.to_string(), now);
            true
        }
    }
}
