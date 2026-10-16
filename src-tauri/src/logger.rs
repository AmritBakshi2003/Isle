//! Simple logging system with configurable log levels.
//!
//! Log levels: none, error, info
//! The default is 'error' - only critical errors are logged.
//! Set via the 'log_level' setting in the database.
//! Logs to stderr and optionally to a file in the app data directory.

use tauri::Manager;
use std::fs::OpenOptions;
use std::sync::Mutex;
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum LogLevel {
    None,
    Error,
    Info,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "none" => LogLevel::None,
            "error" => LogLevel::Error,
            "info" => LogLevel::Info,
            _ => LogLevel::Error, // Default to error for unknown values
        }
    }
}

// Global log level - can be updated at runtime
static LOG_LEVEL: std::sync::RwLock<LogLevel> = std::sync::RwLock::new(LogLevel::Error);

// Optional file handle for logging
static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Update the global log level from a string
pub fn set_level(level: &str) {
    if let Ok(mut guard) = LOG_LEVEL.write() {
        *guard = LogLevel::from_str(level);
    } else {
        eprintln!("[isle WARNING] Failed to acquire log level lock, using default");
    }
}

/// Get the current log level
pub fn get_level() -> LogLevel {
    LOG_LEVEL.read()
        .map(|guard| *guard)
        .unwrap_or(LogLevel::Error) // Default to error if lock is poisoned
}

/// Log an error message (always shown unless level is None)
pub fn error(msg: &str) {
    if get_level() >= LogLevel::Error {
        let log_msg = format!("[isle ERROR] {}", msg);
        eprintln!("{}", log_msg);
        write_to_file(&log_msg);
    }
}

/// Log an info message (only shown if level is Info)
pub fn info(msg: &str) {
    if get_level() >= LogLevel::Info {
        let log_msg = format!("[isle INFO] {}", msg);
        eprintln!("{}", log_msg);
        write_to_file(&log_msg);
    }
}

fn write_to_file(msg: &str) {
    if let Ok(mut file) = LOG_FILE.lock() {
        if let Some(ref mut f) = *file {
            let _ = writeln!(f, "{}", msg);
            let _ = f.flush();
        }
    }
}

/// Initialize logger from database setting
pub fn init_from_db(app: &tauri::AppHandle) {
    let level = if let Some(db) = app.try_state::<crate::db::Db>() {
        db.setting("log_level").unwrap_or_else(|| "error".to_string())
    } else {
        "error".to_string()
    };
    set_level(&level);

    // Set up file logging if log_to_file is enabled
    if let Some(db) = app.try_state::<crate::db::Db>() {
        let log_to_file = db.setting("log_to_file")
            .map(|s| s == "true")
            .unwrap_or(false);

        if log_to_file {
            if let Ok(dir) = app.path().app_data_dir() {
                let log_path = dir.join("isle.log");
                if let Ok(file) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                {
                    if let Ok(mut log_file) = LOG_FILE.lock() {
                        *log_file = Some(file);
                    } else {
                        eprintln!("[isle WARNING] Failed to acquire log file lock");
                    }
                }
            }
        }
    }

    info(&format!("Logger initialized with level: {}", level));
}
