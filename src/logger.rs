//! Minimal file logger. Errors are appended to `LogFiles/error.log` in the data folder
//! (like upstream) and info messages to `LogFiles/info.log`.

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

struct FileLogger {
    path: Mutex<PathBuf>,
}

static LOGGER: FileLogger = FileLogger {
    path: Mutex::new(PathBuf::new()),
};

impl Log for FileLogger {
    fn enabled(&self, m: &Metadata) -> bool {
        m.level() <= Level::Info
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let path = self.path.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if path.as_os_str().is_empty() {
            return;
        }
        let file = if r.level() == Level::Error {
            "error.log"
        } else {
            "info.log"
        };
        let _ = std::fs::create_dir_all(&path);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path.join(file))
        {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(f, "{secs} [{}] {}", r.level(), r.args());
        }
    }

    fn flush(&self) {}
}

/// Install the logger (idempotent). `dir` is the `LogFiles` directory.
pub fn init(dir: PathBuf) {
    set_dir(dir);
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(LevelFilter::Info);
}

/// Change the log directory (e.g. when the data location setting changes).
pub fn set_dir(dir: PathBuf) {
    *LOGGER.path.lock().unwrap_or_else(|e| e.into_inner()) = dir;
}
