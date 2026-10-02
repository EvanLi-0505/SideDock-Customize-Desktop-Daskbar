//! Minimal file logger that writes into the portable `data/logs` folder.
//!
//! One file per day (`sidedock_YYYY-MM-DD.log`), a file is rotated when it grows past
//! [`MAX_FILE_SIZE`] and only the newest [`KEEP_FILES`] files are kept.

use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};

use log::{Level, LevelFilter, Log, Metadata, Record};
use parking_lot::Mutex;
use time::{OffsetDateTime, macros::format_description};

const MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;
const KEEP_FILES: usize = 10;

struct FileLogger {
    dir: PathBuf,
    inner: Mutex<Option<(String, File)>>,
}

static LOGGER: std::sync::OnceLock<FileLogger> = std::sync::OnceLock::new();

fn now() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

impl FileLogger {
    fn file_name(&self, date: &str) -> PathBuf {
        self.dir.join(format!("sidedock_{date}.log"))
    }

    fn open(&self, date: &str) -> Option<File> {
        let path = self.file_name(date);
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_FILE_SIZE) {
            let rotated = self
                .dir
                .join(format!("sidedock_{date}_{}.log", now().unix_timestamp()));
            let _ = std::fs::rename(&path, rotated);
        }
        OpenOptions::new().create(true).append(true).open(path).ok()
    }

    fn prune(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<_> = entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".log"))
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.0));
        for (_, path) in files.into_iter().skip(KEEP_FILES) {
            let _ = std::fs::remove_file(path);
        }
    }

    fn write_line(&self, line: &str) {
        let date = now()
            .format(format_description!("[year]-[month]-[day]"))
            .unwrap_or_default();
        let mut guard = self.inner.lock();
        let needs_reopen = match guard.as_ref() {
            Some((d, file)) => {
                *d != date
                    || file
                        .metadata()
                        .map(|m| m.len() > MAX_FILE_SIZE)
                        .unwrap_or(true)
            }
            None => true,
        };
        if needs_reopen {
            *guard = None;
            if let Some(file) = self.open(&date) {
                *guard = Some((date, file));
            }
            self.prune();
        }
        if let Some((_, file)) = guard.as_mut() {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let target = metadata.target();
        // Silence very chatty dependencies.
        if target.starts_with("tao") || target.starts_with("wry") || target.starts_with("tracing") {
            return metadata.level() <= Level::Warn;
        }
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let ts = now()
            .format(format_description!(
                "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"
            ))
            .unwrap_or_default();
        let line = format!(
            "{ts} [{:<5}] {}: {}\r\n",
            record.level(),
            record.target(),
            record.args()
        );
        if cfg!(debug_assertions) {
            eprint!("{line}");
        }
        self.write_line(&line);
    }

    fn flush(&self) {
        if let Some((_, file)) = self.inner.lock().as_mut() {
            let _ = file.flush();
        }
    }
}

pub fn init(dir: PathBuf) {
    let logger = LOGGER.get_or_init(|| FileLogger {
        dir,
        inner: Mutex::new(None),
    });
    if log::set_logger(logger).is_ok() {
        log::set_max_level(if cfg!(debug_assertions) {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        });
    }
}

/// Removes every log file except the one currently open, which is truncated.
pub fn clear() -> usize {
    let Some(logger) = LOGGER.get() else {
        return 0;
    };
    let mut guard = logger.inner.lock();
    let current = guard.as_ref().map(|(d, _)| logger.file_name(d));
    if let Some((_, file)) = guard.as_mut() {
        let _ = file.set_len(0);
    }
    let keep: Vec<&std::path::Path> = current.iter().map(PathBuf::as_path).collect();
    crate::paths::clear_dir(&logger.dir, &keep)
}
