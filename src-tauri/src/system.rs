//! System state settings (saved to DB)
//!
//! These settings are saved to the database but the actual detection
//! (fullscreen, battery saver, idle) would require Windows API integration.
//! For now, these are placeholder implementations that save the settings.

use crate::db::Db;
use tauri::State;

/// Check if auto-hide when idle is enabled
#[tauri::command]
pub fn auto_hide_get(db: State<Db>) -> bool {
    db.setting("auto_hide").map(|v| v == "true").unwrap_or(false)
}

/// Set auto-hide when idle setting
#[tauri::command]
pub fn auto_hide_set(db: State<Db>, enabled: bool) -> Result<(), String> {
    db.set_setting("auto_hide", if enabled { "true" } else { "false" })
}

/// Check if hide on fullscreen is enabled
#[tauri::command]
pub fn hide_fullscreen_get(db: State<Db>) -> bool {
    db.setting("hide_fullscreen").map(|v| v == "true").unwrap_or(true)
}

/// Set hide on fullscreen setting
#[tauri::command]
pub fn hide_fullscreen_set(db: State<Db>, enabled: bool) -> Result<(), String> {
    db.set_setting("hide_fullscreen", if enabled { "true" } else { "false" })
}

/// Check if hide on battery saver is enabled
#[tauri::command]
pub fn hide_battery_saver_get(db: State<Db>) -> bool {
    db.setting("hide_battery_saver").map(|v| v == "true").unwrap_or(false)
}

/// Set hide on battery saver setting
#[tauri::command]
pub fn hide_battery_saver_set(db: State<Db>, enabled: bool) -> Result<(), String> {
    db.set_setting("hide_battery_saver", if enabled { "true" } else { "false" })
}
