//! The action runner shared by Modes, Routines and the scheduler.
//!
//! An ACTION is a JSON object with a "type". To add a new action type:
//!   1. use `match s("type").as_str()` in `run` below,
//!   2. add it to ACTION_TYPES in src/views/Builder.svelte so users can pick it.
//!
//! Supported actions
//!   {"type":"open","target":"C:\\Path\\app.exe" | "https://..." | "C:\\file.pdf"}
//!   {"type":"wallpaper_next"}                       next image in the wallpaper folder
//!   {"type":"notify","title":"..","body":".."}      native Windows toast
//!   {"type":"media","cmd":"play_pause|next|prev"}
//! TODO(agents): set_volume (IAudioEndpointVolume), window layouts, dnd, night_light, battery_saver.

use crate::db::{e2s, now, Db};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use std::path::Path;

/// Validate a target path/URL for the "open" action
/// Returns Ok(()) if safe, Err with reason if unsafe
fn validate_target(target: &str) -> Result<(), String> {
    // Allow web URLs
    if target.starts_with("http://") || target.starts_with("https://") {
        return Ok(());
    }
    
    // For file paths, require absolute path
    let path = Path::new(target);
    if !path.is_absolute() {
        return Err("Only absolute file paths are allowed".to_string());
    }
    
    // Block suspicious extensions
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        let blocked = ["bat", "cmd", "com", "exe", "scr", "vbs", "js", "jar"];
        if blocked.contains(&ext_lower.as_str()) {
            return Err(format!("File type .{} is not allowed for security reasons", ext_lower));
        }
    }
    
    // Block paths to sensitive system directories
    if let Some(parent) = path.parent() {
        let parent_str = parent.to_string_lossy().to_lowercase();
        let blocked_dirs = [
            "c:\\windows\\system32",
            "c:\\windows\\system",
            "c:\\windows\\syswow64",
        ];
        for blocked in &blocked_dirs {
            if parent_str.starts_with(blocked) {
                return Err(format!("Access to {} is not allowed", parent.display()));
            }
        }
    }
    
    Ok(())
}

/// Runs every action, collecting errors so one failure does not stop the rest.
pub fn run(app: &AppHandle, actions: &Value) -> Result<(), String> {
    let mut errs = vec![];
    for a in actions.as_array().cloned().unwrap_or_default() {
        // Safe JSON accessor with .get()
        let s = |k: &str| a.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let r = match s("type").as_str() {
            "open" => {
                let target = s("target");
                // Validate target before opening
                validate_target(&target)?;
                open::that(target).map_err(e2s)
            }
            "wallpaper_next" => wallpaper_next(app),
            "notify" => notify(app, &s("title"), &s("body")),
            "media" => {
                let c = s("cmd");
                crate::media::control(if c.is_empty() { "play_pause" } else { &c })
            }
            other => Err(format!("unknown action: {other}")),
        };
        if let Err(e) = r {
            errs.push(e);
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs.join("; ")) }
}

pub fn notify(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    app.notification().builder().title(title).body(body).show().map_err(e2s)
}

/// Cycles through images (jpg/png/bmp) in settings key `wallpaper_dir`, in
/// filename order. Index is persisted in `wallpaper_idx`.
pub fn wallpaper_next(app: &AppHandle) -> Result<(), String> {
    let db = app.state::<Db>();
    let dir = db
        .setting("wallpaper_dir")
        .filter(|d| !d.trim().is_empty())
        .ok_or("Set a wallpaper folder first")?;
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map_err(e2s)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|x| x.to_str()).map(|s| s.to_ascii_lowercase()).as_deref(),
                Some("jpg" | "jpeg" | "png" | "bmp")
            )
        })
        .collect();
    if files.is_empty() {
        return Err("No jpg, png or bmp images in the wallpaper folder".into());
    }
    files.sort();
    let idx = db
        .setting("wallpaper_idx")
        .and_then(|s| s.parse::<usize>().ok())
        .map(|i| i + 1)
        .unwrap_or(0)
        % files.len();
    // Safe indexing with bounds check
    let wallpaper_path = files.get(idx)
        .and_then(|p| p.to_str())
        .ok_or("Invalid wallpaper path")?;
    wallpaper::set_from_path(wallpaper_path).map_err(|e| e.to_string())?;
    db.set_setting("wallpaper_idx", &idx.to_string())?;
    db.set_setting("wallpaper_last", &now().to_string())
}
