//! Tauri commands = the whole backend API the UI can call (see AGENTS.md for
//! the table). Naming: `<area>_<verb>`. JS passes camelCase args
//! (`remindMin`), Tauri maps them to these snake_case parameters.
//!
//! Rules for new commands
//!  - Cheap DB-only commands are plain `fn`s.
//!  - Anything that can block (shell, wallpaper, clipboard write, actions) is
//!    `async fn ... -> Result<_, String>` so it never freezes the UI thread.
//!  - Register every command in `generate_handler!` in lib.rs.
//!  - If it changes events/routines/settings used by the scheduler, call `sched.wake()`.

use crate::{
    actions,
    apps,
    clipboard,
    db::{e2s, now, Db},
    icon,
    island,
    logger,
    progress::ProgressHandle,
    scheduler::Scheduler,
};
use base64::Engine;
use rusqlite::params;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

fn json(s: String) -> Value {
    serde_json::from_str(&s).unwrap_or(Value::Null)
}

// ---------------------------------------------------------------- tasks

#[derive(Serialize)]
pub struct Task {
    id: i64,
    title: String,
    done: bool,
    due: Option<i64>,
    remind_min: i64,
    start_time: Option<i64>,
    end_time: Option<i64>,
}

#[tauri::command]
pub fn task_list(db: State<Db>) -> Result<Vec<Task>, String> {
    db.with(|c| {
        let mut s = c.prepare("SELECT id,title,done,due,remind_min,start_time,end_time FROM tasks ORDER BY done, due IS NULL, due, id DESC")?;
        let rows = s.query_map([], |r| {
            Ok(Task { id: r.get(0)?, title: r.get(1)?, done: r.get::<_, i64>(2)? != 0, due: r.get(3)?, remind_min: r.get(4)?, start_time: r.get(5)?, end_time: r.get(6)? })
        })?;
        rows.collect()
    })
}

#[tauri::command]
pub fn task_add(db: State<Db>, title: String, due: Option<i64>) -> Result<i64, String> {
    db.with(|c| {
        c.execute(
            "INSERT INTO tasks(title,due,created) VALUES(?1,?2,?3)",
            params![title.trim(), due, now()],
        )?;
        Ok(c.last_insert_rowid())
    })
}

#[tauri::command]
pub fn task_toggle(db: State<Db>, id: i64) -> Result<(), String> {
    db.with(|c| c.execute("UPDATE tasks SET done=1-done WHERE id=?1", [id]).map(|_| ()))
}

#[tauri::command]
pub fn task_delete(db: State<Db>, sched: State<Scheduler>, id: i64) -> Result<(), String> {
    db.with(|c| c.execute("DELETE FROM tasks WHERE id=?1", [id]).map(|_| ()))?;
    sched.wake();
    Ok(())
}

#[tauri::command]
pub fn task_clear_done(db: State<Db>) -> Result<(), String> {
    db.with(|c| c.execute("DELETE FROM tasks WHERE done=1", []).map(|_| ()))
}

/// Add a task with an optional reminder. If remind_min > 0, the scheduler will fire a toast.
#[tauri::command]
pub fn task_add_with_reminder(
    db: State<Db>,
    sched: State<Scheduler>,
    title: String,
    due: Option<i64>,
    remind_min: i64,
) -> Result<i64, String> {
    let id = db.with(|c| {
        c.execute(
            "INSERT INTO tasks(title,due,created,remind_min) VALUES(?1,?2,?3,?4)",
            params![title.trim(), due, now(), remind_min],
        )?;
        Ok(c.last_insert_rowid())
    })?;
    
    // Wake the scheduler so it recalculates next wake time for task reminders
    if due.is_some() && remind_min > 0 {
        sched.wake();
    }
    
    Ok(id)
}

/// Cancel a task's reminder by setting remind_min to 0.
#[tauri::command]
pub fn task_cancel_reminder(db: State<Db>, sched: State<Scheduler>, id: i64) -> Result<(), String> {
    db.with(|c| {
        c.execute("UPDATE tasks SET remind_min=0 WHERE id=?1", [id])
    })?;
    sched.wake();
    Ok(())
}

// --------------------------------------------------------------- events

#[derive(Serialize)]
pub struct Event {
    id: i64,
    title: String,
    start: i64,
    end: Option<i64>,
    remind_min: i64,
}

/// Events whose start is in [from, to) (unix seconds).
#[tauri::command]
pub fn event_list(db: State<Db>, from: i64, to: i64) -> Result<Vec<Event>, String> {
    db.with(|c| {
        let mut s = c.prepare(
            "SELECT id,title,starts,ends,remind_min FROM events WHERE starts>=?1 AND starts<?2 ORDER BY starts",
        )?;
        let rows = s.query_map([from, to], |r| {
            Ok(Event { id: r.get(0)?, title: r.get(1)?, start: r.get(2)?, end: r.get(3)?, remind_min: r.get(4)? })
        })?;
        rows.collect()
    })
}

#[tauri::command]
pub fn event_add(
    db: State<Db>,
    sched: State<Scheduler>,
    title: String,
    start: i64,
    end: Option<i64>,
    remind_min: i64,
) -> Result<i64, String> {
    let id = db.with(|c| {
        c.execute(
            "INSERT INTO events(title,starts,ends,remind_min) VALUES(?1,?2,?3,?4)",
            params![title.trim(), start, end, remind_min],
        )?;
        Ok(c.last_insert_rowid())
    })?;
    sched.wake();
    Ok(id)
}

#[tauri::command]
pub fn event_delete(db: State<Db>, sched: State<Scheduler>, id: i64) -> Result<(), String> {
    db.with(|c| c.execute("DELETE FROM events WHERE id=?1", [id]).map(|_| ()))?;
    sched.wake();
    Ok(())
}

// ------------------------------------------------------------ clipboard

#[derive(Serialize)]
pub struct Clip {
    id: i64,
    text: String, // truncated to 400 chars for the list; full text via clip_copy
    pinned: bool,
    ts: i64,
    file_path: Option<String>,
    file_name: Option<String>,
    file_type: Option<String>,
}

#[tauri::command]
pub fn clip_list(db: State<Db>, query: Option<String>) -> Result<Vec<Clip>, String> {
    let q = query.unwrap_or_default().replace(['%', '_'], "");
    let like = format!("%{q}%");
    db.with(|c| {
        let mut s = c.prepare(
            "SELECT id,substr(text,1,400),pinned,ts,file_path,file_name,file_type FROM clips WHERE text LIKE ?1
             ORDER BY pinned DESC, ts DESC LIMIT 100",
        )?;
        let rows = s.query_map([like], |r| {
            Ok(Clip { 
                id: r.get(0)?, 
                text: r.get(1)?, 
                pinned: r.get::<_, i64>(2)? != 0, 
                ts: r.get(3)?,
                file_path: r.get(4)?,
                file_name: r.get(5)?,
                file_type: r.get(6)?,
            })
        })?;
        rows.collect()
    })
}

/// Put an item back on the system clipboard (the listener then bumps its timestamp).
#[tauri::command]
pub async fn clip_copy(db: State<'_, Db>, id: i64) -> Result<(), String> {
    let text: String = db.with(|c| c.query_row("SELECT text FROM clips WHERE id=?1", [id], |r| r.get(0)))?;
    arboard::Clipboard::new().map_err(e2s)?.set_text(text).map_err(e2s)
}

#[tauri::command]
pub fn clip_pin(db: State<Db>, id: i64) -> Result<(), String> {
    db.with(|c| c.execute("UPDATE clips SET pinned=1-pinned WHERE id=?1", [id]).map(|_| ()))
}

#[tauri::command]
pub fn clip_delete(db: State<Db>, id: i64) -> Result<(), String> {
    db.with(|c| c.execute("DELETE FROM clips WHERE id=?1", [id]).map(|_| ()))
}

/// Clears everything except pinned items.
#[tauri::command]
pub fn clip_clear(db: State<Db>) -> Result<(), String> {
    db.with(|c| c.execute("DELETE FROM clips WHERE pinned=0", []).map(|_| ()))
}

/// Open a file from clipboard by path
#[tauri::command]
pub fn clip_open_file(path: String) -> Result<(), String> {
    use std::path::Path;

    let p = Path::new(&path);

    // Validate path exists and is a file
    if !p.exists() {
        return Err("File does not exist".to_string());
    }
    if !p.is_file() {
        return Err("Path is not a file".to_string());
    }

    // Validate path is absolute (prevent relative path traversal)
    if !p.is_absolute() {
        return Err("Only absolute file paths are allowed".to_string());
    }

    open::that(&path).map_err(e2s)
}

/// Delete a file from clipboard by path
#[tauri::command]
pub fn clip_delete_file(db: State<Db>, path: String) -> Result<(), String> {
    db.with(|c| c.execute("DELETE FROM clips WHERE file_path=?1", [path]).map(|_| ()))
}

/// Paste files from Windows clipboard
#[tauri::command]
pub fn clip_paste_files(db: State<Db>) -> Result<usize, String> {
    crate::clipboard::paste_files_from_clipboard(db)
}

// ---------------------------------------------------------------- modes

#[derive(Serialize)]
pub struct Mode {
    id: i64,
    name: String,
    icon: String,
    actions: Value,
}

#[tauri::command]
pub fn mode_list(db: State<Db>) -> Result<Vec<Mode>, String> {
    db.with(|c| {
        let mut s = c.prepare("SELECT id,name,icon,actions FROM modes ORDER BY id")?;
        let rows = s.query_map([], |r| {
            Ok(Mode { id: r.get(0)?, name: r.get(1)?, icon: r.get(2)?, actions: json(r.get(3)?) })
        })?;
        rows.collect()
    })
}

#[tauri::command]
pub fn mode_save(db: State<Db>, id: Option<i64>, name: String, icon: String, actions: Value) -> Result<i64, String> {
    db.with(|c| {
        let a = actions.to_string();
        match id {
            Some(i) => {
                c.execute("UPDATE modes SET name=?1,icon=?2,actions=?3 WHERE id=?4", params![name, icon, a, i])?;
                Ok(i)
            }
            None => {
                c.execute("INSERT INTO modes(name,icon,actions) VALUES(?1,?2,?3)", params![name, icon, a])?;
                Ok(c.last_insert_rowid())
            }
        }
    })
}

#[tauri::command]
pub fn mode_delete(db: State<Db>, id: i64) -> Result<(), String> {
    db.with(|c| c.execute("DELETE FROM modes WHERE id=?1", [id]).map(|_| ()))
}

/// Run a mode's actions and remember it as the active mode (`active_mode` setting).
/// If deactivating (id matches current active mode), just clear the active mode.
#[tauri::command]
pub fn mode_activate(app: AppHandle, db: State<Db>, id: i64) -> Result<(), String> {
    let current_mode = db.setting("active_mode");
    let is_deactivating = current_mode.as_deref() == Some(&id.to_string());
    
    if is_deactivating {
        db.set_setting("active_mode", "")?;
    } else {
        let a: String = db.with(|c| c.query_row("SELECT actions FROM modes WHERE id=?1", [id], |r| r.get(0)))?;
        db.set_setting("active_mode", &id.to_string())?;
        actions::run(&app, &json(a))?;
    }
    
    Ok(())
}

// ------------------------------------------------------------- routines

#[derive(Serialize)]
pub struct Routine {
    id: i64,
    name: String,
    trigger: Value, // {"kind":"manual"} | {"kind":"time","at":"HH:MM","days":[0-6]}
    actions: Value,
    enabled: bool,
}

#[tauri::command]
pub fn routine_list(db: State<Db>) -> Result<Vec<Routine>, String> {
    db.with(|c| {
        let mut s = c.prepare("SELECT id,name,trig,actions,enabled FROM routines ORDER BY id")?;
        let rows = s.query_map([], |r| {
            Ok(Routine {
                id: r.get(0)?,
                name: r.get(1)?,
                trigger: json(r.get(2)?),
                actions: json(r.get(3)?),
                enabled: r.get::<_, i64>(4)? != 0,
            })
        })?;
        rows.collect()
    })
}

#[tauri::command]
pub fn routine_save(
    db: State<Db>,
    sched: State<Scheduler>,
    id: Option<i64>,
    name: String,
    trigger: Value,
    actions: Value,
) -> Result<i64, String> {
    let r = db.with(|c| {
        let (t, a) = (trigger.to_string(), actions.to_string());
        match id {
            Some(i) => {
                c.execute("UPDATE routines SET name=?1,trig=?2,actions=?3 WHERE id=?4", params![name, t, a, i])?;
                Ok(i)
            }
            None => {
                c.execute("INSERT INTO routines(name,trig,actions) VALUES(?1,?2,?3)", params![name, t, a])?;
                Ok(c.last_insert_rowid())
            }
        }
    })?;
    sched.wake();
    Ok(r)
}

#[tauri::command]
pub fn routine_toggle(db: State<Db>, sched: State<Scheduler>, id: i64, enabled: bool) -> Result<(), String> {
    db.with(|c| c.execute("UPDATE routines SET enabled=?1 WHERE id=?2", params![enabled as i64, id]).map(|_| ()))?;
    sched.wake();
    Ok(())
}

#[tauri::command]
pub fn routine_delete(db: State<Db>, sched: State<Scheduler>, id: i64) -> Result<(), String> {
    db.with(|c| c.execute("DELETE FROM routines WHERE id=?1", [id]).map(|_| ()))?;
    sched.wake();
    Ok(())
}

#[tauri::command]
pub async fn routine_run(app: AppHandle, db: State<'_, Db>, id: i64) -> Result<(), String> {
    let a: String = db.with(|c| c.query_row("SELECT actions FROM routines WHERE id=?1", [id], |r| r.get(0)))?;
    actions::run(&app, &json(a))
}

// ------------------------------------------------- wallpaper & settings

#[tauri::command]
pub async fn wallpaper_next(app: AppHandle) -> Result<(), String> {
    actions::wallpaper_next(&app)
}

#[tauri::command]
pub fn setting_get(db: State<Db>, key: String) -> Option<String> {
    db.setting(&key)
}

/// Known keys: wallpaper_dir, wallpaper_interval (minutes), active_mode.
#[tauri::command]
pub fn setting_set(db: State<Db>, sched: State<Scheduler>, _app: AppHandle, key: String, value: String) -> Result<(), String> {
    db.set_setting(&key, &value)?;
    sched.wake();

    // Special handling for module_clipboard to keep runtime flag in sync
    if key == "module_clipboard" {
        let enabled = value == "true";
        clipboard::set_enabled(enabled);
    }

    Ok(())
}

/// Return all rows from the settings table as a JSON object {key: value, …}.
/// Used by Settings.svelte on mount to bulk-load every preference in one call.
#[tauri::command]
pub fn settings_get_all(db: State<Db>) -> Result<serde_json::Value, String> {
    db.with(|c| {
        let mut s = c.prepare("SELECT k, v FROM settings")?;
        let map: rusqlite::Result<serde_json::Map<String, serde_json::Value>> = s
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .map(|res| res.map(|(k, v)| (k, serde_json::Value::String(v))))
            .collect();
        map.map(serde_json::Value::Object)
    })
}

/// Show the settings window.
/// The window is pre-created in tauri.conf.json but hidden by default.
#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    crate::island::show_settings(&app);
    Ok(())
}

/// Apply window-level behavior settings at runtime (no restart needed).
/// Currently handles: always_on_top.
#[tauri::command]
pub fn apply_window_settings(app: AppHandle, always_on_top: bool) -> Result<(), String> {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        w.set_always_on_top(always_on_top).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Hide or show the main island window.
/// Used by fullscreen/battery saver detection to temporarily hide the pill.
#[tauri::command]
pub fn window_hide(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        w.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn window_show(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        w.show().map_err(|e| e.to_string())?;
        // Do NOT set_focus() - this prevents stealing keyboard focus
        // Only hotkey and tray should focus the pill
    }
    Ok(())
}

#[tauri::command]
pub fn autostart_get(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn autostart_set(app: AppHandle, enabled: bool) -> Result<(), String> {
    let m = app.autolaunch();
    if enabled { m.enable() } else { m.disable() }.map_err(e2s)
}

// --------------------------------------------------------- live progress

/// Push a progress update (0-100) to the pill ring.
/// Sends to the single shared channel; the manager thread emits the event.
#[tauri::command]
pub fn progress_push(handle: State<'_, ProgressHandle>, value: f64, label: Option<String>) -> Result<(), String> {
    handle.push(value.round() as i64, label.unwrap_or_default());
    Ok(())
}

/// Immediately hide the progress ring.
#[tauri::command]
pub fn progress_clear(handle: State<'_, ProgressHandle>) -> Result<(), String> {
    handle.clear();
    Ok(())
}

/// Battery state for the browser dev mock.
/// In Tauri mode the battery watcher in progress.rs pushes battery://update events
/// automatically; this command is only useful in npm-run-dev browser mode.
#[tauri::command]
pub fn battery_get() -> serde_json::Value {
    use serde_json::json;
    // In Tauri mode the watcher handles this; return null so the UI knows to
    // rely on events instead.
    #[cfg(target_os = "windows")]
    return json!({ "charging": false, "level": null }); // events handle it
    #[cfg(not(target_os = "windows"))]
    json!({ "charging": false, "level": null })
}

/// Emit event to notify main island that settings have changed
#[tauri::command]
pub fn emit_settings_changed(app: AppHandle) -> Result<(), String> {
    let _ = app.emit("settings://changed", ());
    Ok(())
}

// ----------------------------------------------------------- performance settings

/// Re-register hotkeys after they've been changed in settings
#[tauri::command]
pub fn reregister_hotkeys(app: AppHandle, db: State<Db>) -> Result<(), String> {
    let toggle_hk = db.setting("hotkey_toggle").unwrap_or_else(|| "ctrl+alt+space".to_string());

    // Unregister existing hotkeys first
    let _ = app.global_shortcut().unregister_all();

    // Re-register with new value
    island::register_hotkey(&app, &toggle_hk);

    Ok(())
}

/// Reload logger when log_level setting changes
#[tauri::command]
pub fn reload_logger(app: AppHandle) -> Result<(), String> {
    logger::init_from_db(&app);
    Ok(())
}

/// Check if clipboard module is enabled
#[tauri::command]
pub fn clipboard_enabled_get() -> bool {
    clipboard::is_enabled()
}

/// Enable or disable clipboard module
#[tauri::command]
pub fn clipboard_enabled_set(enabled: bool) -> Result<(), String> {
    clipboard::set_enabled(enabled);
    Ok(())
}

// ------------------------------------------------------ utility drawer

/// Open native Windows applications
#[tauri::command]
pub async fn open_native(app: String) -> Result<(), String> {
    use std::process::Command;
    match app.as_str() {
        "calc" => {
            Command::new("calc.exe").spawn().map_err(|e| e.to_string())?;
        }
        "notepad" => {
            Command::new("notepad.exe").spawn().map_err(|e| e.to_string())?;
        }
        "settings" => {
            Command::new("cmd")
                .args(["/c", "start", "ms-settings:"])
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        _ => return Err(format!("Unknown app: {}", app)),
    }
    Ok(())
}

/// Enumerate installed Windows applications from Start Menu
#[tauri::command]
pub fn enumerate_apps() -> Result<Vec<apps::AppShortcut>, String> {
    apps::enumerate_apps()
}

/// Get user's saved app shortcuts
#[tauri::command]
pub fn app_shortcuts_list(db: State<Db>) -> Result<Vec<apps::AppShortcut>, String> {
    db.with(|c| {
        let mut s = c.prepare("SELECT id,name,path,icon,icon_filename,target,folder_group,hidden FROM app_shortcuts ORDER BY name")?;
        let rows = s.query_map([], |r| {
            Ok(apps::AppShortcut {
                id: Some(r.get(0)?),
                name: r.get(1)?,
                path: r.get(2)?,
                icon: r.get(3)?,
                icon_filename: r.get(4)?,
                target: r.get(5)?,
                folder_group: r.get(6)?,
                hidden: r.get(7)?,
            })
        })?;
        rows.collect()
    })
}

/// Add an app shortcut
#[tauri::command]
pub fn app_shortcut_add(app: AppHandle, db: State<Db>, name: String, path: String, icon: Option<String>, target: Option<String>, folder_group: Option<String>, hidden: Option<bool>) -> Result<i64, String> {
    // Extract icon from the icon path (or fallback to path)
    let app_data_dir = app.path().app_data_dir().map_err(e2s)?;
    let icon_target = icon.as_ref().unwrap_or(&path);
    let icon_filename = match icon::extract_icon_from_path(icon_target, &app_data_dir) {
        Ok(filename) => Some(filename),
        Err(e) => {
            eprintln!("Failed to extract icon for {}: {}", icon_target, e);
            None
        }
    };

    db.with(|c| {
        c.execute(
            "INSERT INTO app_shortcuts(name,path,icon,icon_filename,target,folder_group,hidden) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![name, path, icon, icon_filename, target, folder_group, hidden.unwrap_or(false)],
        )?;
        Ok(c.last_insert_rowid())
    }).map_err(|e| {
        // Check for UNIQUE constraint violation (path already exists)
        if e.to_string().contains("UNIQUE constraint failed") {
            format!("{} is already added", name)
        } else {
            e.to_string()
        }
    })
}

/// Remove an app shortcut
#[tauri::command]
pub fn app_shortcut_remove(app: AppHandle, db: State<Db>, id: i64) -> Result<(), String> {
    // Get icon filename before deleting
    let icon_filename: Option<String> = db.with(|c| {
        c.query_row("SELECT icon_filename FROM app_shortcuts WHERE id=?1", [id], |r| r.get(0))
    }).ok();

    // Delete from database
    db.with(|c| c.execute("DELETE FROM app_shortcuts WHERE id=?1", [id]).map(|_| ()))?;

    // Delete icon file if it exists
    if let Some(filename) = icon_filename {
        let app_data_dir = app.path().app_data_dir().map_err(e2s)?;
        let _ = icon::delete_icon(&filename, &app_data_dir);
    }

    Ok(())
}

/// Launch an app shortcut
#[tauri::command]
pub async fn app_shortcut_launch(path: String) -> Result<(), String> {
    use std::process::Command;
    // Handle shell items (Recycle Bin, Control Panel)
    if path.starts_with("shell:::") {
        if cfg!(windows) {
            Command::new("explorer.exe")
                .arg(&path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    } else {
        // Use 'start' command to launch .lnk files or executables
        if cfg!(windows) {
            Command::new("cmd")
                .args(["/c", "start", "", &path])
                .spawn()
                .map_err(|e| e.to_string())?;
        } else {
            Command::new(&path).spawn().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Get icon file URL for a shortcut
#[tauri::command]
pub fn app_shortcut_icon(app: AppHandle, db: State<Db>, icon_filename: String, id: Option<i64>) -> Result<String, String> {
    let app_data_dir = app.path().app_data_dir().map_err(e2s)?;
    
    // If icon_filename is empty, trigger self-heal immediately
    if icon_filename.is_empty() {
        eprintln!("[icon] icon_filename is empty, triggering self-heal");
        if let Some(shortcut_id) = id {
            let (icon_opt, path): (Option<String>, Option<String>) = db.with(|c| {
                c.query_row(
                    "SELECT icon, path FROM app_shortcuts WHERE id=?1",
                    [shortcut_id],
                    |r| Ok((r.get(0)?, r.get(1)?))
                )
            }).map_err(|e| {
                eprintln!("[icon] Self-heal failed to query database: {}", e);
                format!("Failed to query database: {}", e)
            })?;
            
            let path = path.unwrap_or_default();
            if path.is_empty() {
                eprintln!("[icon] Self-heal failed: path is empty for id={}", shortcut_id);
                return Err("Shortcut has no path".to_string());
            }

            let icon_target = icon_opt.as_ref().unwrap_or(&path);
            eprintln!("[icon] Self-heal: attempting re-extraction for id={}, path={}", shortcut_id, icon_target);
            
            match icon::extract_icon_from_path(icon_target, &app_data_dir) {
                Ok(new_filename) => {
                    eprintln!("[icon] Self-heal: re-extracted icon for id={}, path={}, new_filename={}", shortcut_id, icon_target, new_filename);
                    
                    // Update database with new filename
                    let _ = db.with(|c| {
                        c.execute(
                            "UPDATE app_shortcuts SET icon_filename=?1 WHERE id=?2",
                            params![&new_filename, shortcut_id]
                        )
                    }).map_err(|e| eprintln!("[icon] Failed to update icon_filename in database: {}", e));
                    
                    // Read the new file and return data URL
                    let new_icon_path = app_data_dir.join("icons").join(&new_filename);
                    let png_data = std::fs::read(&new_icon_path).map_err(|e| format!("Failed to read re-extracted icon: {}", e))?;
                    let base64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
                    return Ok(format!("data:image/png;base64,{}", base64));
                }
                Err(e) => {
                    eprintln!("[icon] Self-heal failed for id={}, path={}: {}", shortcut_id, icon_target, e);
                    return Err(format!("Icon extraction failed: {}", e));
                }
            }
        } else {
            eprintln!("[icon] No id provided for empty icon_filename");
            return Err("Icon filename is empty and no id for self-heal".to_string());
        }
    }
    
    let icon_path = app_data_dir.join("icons").join(&icon_filename);
    
    // Check if cached file exists
    if !icon_path.exists() {
        eprintln!("[icon] Cache miss: file does not exist for icon_filename={}", icon_filename);
        
        // Self-heal: try to re-extract icon from stored icon/path
        if let Some(shortcut_id) = id {
            let (icon_opt, path): (Option<String>, Option<String>) = db.with(|c| {
                c.query_row(
                    "SELECT icon, path FROM app_shortcuts WHERE id=?1",
                    [shortcut_id],
                    |r| Ok((r.get(0)?, r.get(1)?))
                )
            }).map_err(|e| {
                eprintln!("[icon] Self-heal failed to query database: {}", e);
                format!("Failed to query database: {}", e)
            })?;
            
            let path = path.unwrap_or_default();
            if path.is_empty() {
                eprintln!("[icon] Self-heal failed: path is empty for id={}", shortcut_id);
                return Err("Shortcut has no path".to_string());
            }

            let icon_target = icon_opt.as_ref().unwrap_or(&path);
            eprintln!("[icon] Self-heal: attempting re-extraction for id={}, path={}", shortcut_id, icon_target);
            
            match icon::extract_icon_from_path(icon_target, &app_data_dir) {
                Ok(new_filename) => {
                    eprintln!("[icon] Self-heal: re-extracted icon for id={}, path={}, new_filename={}", shortcut_id, icon_target, new_filename);
                    
                    // Update database with new filename
                    let _ = db.with(|c| {
                        c.execute(
                            "UPDATE app_shortcuts SET icon_filename=?1 WHERE id=?2",
                            params![&new_filename, shortcut_id]
                        )
                    }).map_err(|e| eprintln!("[icon] Failed to update icon_filename in database: {}", e));
                    
                    // Read the new file and return data URL
                    let new_icon_path = app_data_dir.join("icons").join(&new_filename);
                    let png_data = std::fs::read(&new_icon_path).map_err(|e| format!("Failed to read re-extracted icon: {}", e))?;
                    let base64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
                    return Ok(format!("data:image/png;base64,{}", base64));
                }
                Err(e) => {
                    eprintln!("[icon] Self-heal failed for id={}, path={}: {}", shortcut_id, icon_target, e);
                    return Err(format!("Icon extraction failed: {}", e));
                }
            }
        } else {
            eprintln!("[icon] No id provided, cannot self-heal missing file for icon_filename={}", icon_filename);
            return Err("Icon file not found and no id for self-heal".to_string());
        }
    } else {
        eprintln!("[icon] Serving from cache: icon_filename={}", icon_filename);
    }
    
    // Read PNG file and convert to base64 data URI
    let png_data = std::fs::read(&icon_path).map_err(|e| format!("Failed to read icon: {}", e))?;
    let base64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
    Ok(format!("data:image/png;base64,{}", base64))
}

/// Clear all cached icons (for testing/regeneration)
#[tauri::command]
pub fn clear_all_icons(app: AppHandle) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(e2s)?;
    icon::clear_all_icons(&app_data_dir)
}

/// Regenerate icon for a specific shortcut
#[tauri::command]
pub fn regenerate_shortcut_icon(app: AppHandle, _id: i64, icon_filename: String) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(e2s)?;
    icon::regenerate_icon(&icon_filename, &app_data_dir)
}

/// Re-extract icons for shortcuts with old cache versions
#[tauri::command]
pub fn reextract_old_icons(app: AppHandle, db: State<Db>) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(e2s)?;

    // Get all shortcuts with their icon paths and filenames
    let shortcuts: Vec<(i64, String, Option<String>)> = db.with(|c| {
        let mut s = c.prepare("SELECT id,icon,icon_filename FROM app_shortcuts")?;
        let rows = s.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
        rows.collect()
    }).map_err(|e| e.to_string())?;

    // Re-extract icons that need it
    let updated = icon::reextract_old_icons(&shortcuts, &app_data_dir);

    // Update database with new filenames
    for (id, new_filename) in updated {
        db.with(|c| {
            c.execute(
                "UPDATE app_shortcuts SET icon_filename = ?1 WHERE id = ?2",
                params![new_filename, id],
            )
        }).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Extract icon for a path without adding to database (for picker lazy loading)
#[tauri::command]
pub fn extract_app_icon(app: AppHandle, path: String) -> Result<String, String> {
    let app_data_dir = app.path().app_data_dir().map_err(e2s)?;
    let icon_filename = icon::extract_icon_from_path(&path, &app_data_dir)?;
    
    // Read the PNG file and convert to base64 data URI
    let icon_path = app_data_dir.join("icons").join(&icon_filename);
    let png_data = std::fs::read(&icon_path).map_err(|e| format!("Failed to read icon: {}", e))?;
    let base64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
    Ok(format!("data:image/png;base64,{}", base64))
}
