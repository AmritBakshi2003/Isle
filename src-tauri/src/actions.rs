//! Native Windows notifications, shared by the scheduler.
//!
//! Modes, Routines and wallpaper rotation were removed in v0.3.0: their UI went
//! away in v2.0 and the backend followed once the Workspace dashboard replaced
//! them. Only `notify` remains, used by scheduler.rs for event reminders.
//!
//! TODO(agents): if a mode/routine surface ever returns, re-add the action
//! runner here ({"type":"notify"|"open"|"media"|"topmost"}) together with its
//! target validation.

use crate::db::e2s;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn notify(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    app.notification().builder().title(title).body(body).show().map_err(e2s)
}
