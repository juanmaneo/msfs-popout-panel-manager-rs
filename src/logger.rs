//! Minimal file logger matching upstream's log4net `ErrorLog` appender: error messages are
//! appended to `LogFiles/error.log` in the data folder as `%date - %message`, rolling over
//! by size (25 MB, 5 backups).

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const LOG_FILE: &str = "error.log";
const MAX_FILE_SIZE: u64 = 25 * 1024 * 1024;
const MAX_BACKUPS: u32 = 5;

struct FileLogger {
    dir: Mutex<PathBuf>,
}

static LOGGER: FileLogger = FileLogger {
    dir: Mutex::new(PathBuf::new()),
};

impl Log for FileLogger {
    fn enabled(&self, m: &Metadata) -> bool {
        m.level() <= Level::Error
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        // Holding the lock while writing keeps lines from concurrent threads whole.
        let dir = self.dir.lock().unwrap_or_else(|e| e.into_inner());
        if dir.as_os_str().is_empty() {
            return;
        }
        // A logger has nowhere to report its own failures.
        let _ = append(
            &dir,
            &format!("{} - {}", timestamp(), r.args()),
            MAX_FILE_SIZE,
        );
    }

    fn flush(&self) {}
}

/// Append one line to the log in `dir`, rolling the file over first when it is full.
fn append(dir: &Path, line: &str, max_size: u64) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(LOG_FILE);
    if fs::metadata(&path).is_ok_and(|m| m.len() >= max_size) {
        roll_over(&path)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{line}")
}

/// log4net size rolling: `error.log.4` -> `.5`, ..., `error.log` -> `error.log.1`; the
/// oldest backup is dropped.
fn roll_over(path: &Path) -> io::Result<()> {
    let backup = |n: u32| {
        let mut name = path.as_os_str().to_owned();
        name.push(format!(".{n}"));
        PathBuf::from(name)
    };
    let oldest = backup(MAX_BACKUPS);
    if oldest.exists() {
        fs::remove_file(&oldest)?;
    }
    for n in (1..MAX_BACKUPS).rev() {
        let from = backup(n);
        if from.exists() {
            fs::rename(&from, backup(n + 1))?;
        }
    }
    fs::rename(path, backup(1))
}

/// Local time in log4net's `%date` (ISO 8601) layout: `2024-01-31 13:45:07,123`.
fn timestamp() -> String {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: GetLocalTime only writes the SYSTEMTIME it is given.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02},{:03}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}

/// Install the logger (idempotent). `dir` is the `LogFiles` directory.
pub fn init(dir: PathBuf) {
    set_dir(dir);
    // Fails only when a logger is already installed, which is fine.
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(LevelFilter::Error);
}

/// Change the log directory (e.g. when the data location setting changes).
pub fn set_dir(dir: PathBuf) {
    *LOGGER.dir.lock().unwrap_or_else(|e| e.into_inner()) = dir;
}

/// Log panics (the release build has no console to print them to).
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("Unhandled panic: {info}\n{backtrace}");
        default_hook(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_over_when_full() {
        let dir = std::env::temp_dir().join(format!("popm-log-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for i in 0..8 {
            append(&dir, &format!("line {i}"), 1).unwrap();
        }
        let read = |name: &str| fs::read_to_string(dir.join(name)).unwrap();
        assert_eq!(read("error.log"), "line 7\n");
        assert_eq!(read("error.log.1"), "line 6\n");
        assert_eq!(read("error.log.5"), "line 2\n");
        assert!(!dir.join("error.log.6").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn timestamp_layout() {
        let t = timestamp();
        assert_eq!(t.len(), "2024-01-31 13:45:07,123".len());
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[10..11], " ");
        assert_eq!(&t[19..20], ",");
    }
}
