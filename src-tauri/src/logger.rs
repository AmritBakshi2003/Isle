//! Simple logging system with configurable log levels.
//!
//! Log levels: none, error, info
//! The default is 'error' - only critical errors are logged.
//! Set via the 'log_level' setting in the database.

use tauri::Manager;

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

/// Update the global log level from a string
pub fn set_level(level: &str) {
    let mut guard = LOG_LEVEL.write().unwrap();
    *guard = LogLevel::from_str(level);
}

/// Get the current log level
pub fn get_level() -> LogLevel {
    let guard = LOG_LEVEL.read().unwrap();
    *guard
}

/// Log an error message (always shown unless level is None)
pub fn error(msg: &str) {
    if get_level() >= LogLevel::Error {
        eprintln!("[isle ERROR] {}", msg);
    }
}

/// Log an info message (only shown if level is Info)
pub fn info(msg: &str) {
    if get_level() >= LogLevel::Info {
        eprintln!("[isle INFO] {}", msg);
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
    info(&format!("Logger initialized with level: {}", level));
}
