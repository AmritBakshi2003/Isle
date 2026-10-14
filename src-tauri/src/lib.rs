//! Isle backend entry point.
//!
//! READ AGENTS.md at the repo root first: it documents the architecture, the
//! command/event contract with the Svelte UI, and the DB schema.
//!
//! Startup order:
//!   1. open SQLite + manage(Db)
//!   2. start scheduler (needs Db)
//!   3. start media + clipboard listeners (event-driven, no polling)
//!   4. start progress manager (single channel, battery/BITS/install watchers)
//!   5. place window at top-centre in collapsed size
//!   6. register hotkeys
//!   7. build system tray (right-click → Show/Hide | Quit)

mod actions;
mod clipboard;
mod commands;
mod db;
mod island;
mod media;
mod progress;
mod scheduler;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(island::hotkey_plugin())
        .setup(|app| {
            let handle = app.handle().clone();

            // Single-instance guard: if Isle is already running, signal the
            // existing instance to show itself, then exit cleanly.
            if !island::ensure_single_instance(&handle) {
                eprintln!("[isle] another instance is already running — signalled it to show. Exiting.");
                app.handle().exit(0);
                return Ok(());
            }

            // 1. Database
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(db::Db::open(&dir.join("isle.db"))?);

            // 2-4. Background services
            app.manage(scheduler::start(handle.clone()));
            media::start(handle.clone());
            clipboard::start(handle.clone());
            app.manage(progress::start(handle.clone())); // single-channel progress manager

            // 5. Window placement
            island::resize(&handle, false);

            // 6. Register toggle hotkey (non-fatal — conflict just logs a warning)
            island::register_hotkey(&handle);

            // 7. Register settings hotkey from DB (default: ctrl+alt+comma)
            {
                let db = app.state::<db::Db>();
                let settings_hk = db.setting("hotkey_settings")
                    .unwrap_or_else(|| "ctrl+alt+comma".to_string());
                island::register_settings_hotkey(&handle, &settings_hk);
            }

            // 8. System tray
            let show = MenuItem::with_id(app, "show", "Show / Hide", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Isle",   true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &settings, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Isle — Dynamic Island for Windows")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        // Actually quit - destroy both windows
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.destroy();
                        }
                        if let Some(w) = app.get_webview_window("settings") {
                            let _ = w.destroy();
                        }
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        let _ = app.emit("island://toggle", ());
                    }
                    "settings" => {
                        island::show_settings(app);
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        // Close → hide for both windows (both live permanently in background).
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Both main pill and settings window: hide instead of closing
                window.hide().unwrap_or(());
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            island::island_resize,
            media::media_get,
            media::media_control,
            commands::task_list,
            commands::task_add,
            commands::task_toggle,
            commands::task_delete,
            commands::task_clear_done,
            commands::event_list,
            commands::event_add,
            commands::event_delete,
            commands::clip_list,
            commands::clip_copy,
            commands::clip_pin,
            commands::clip_delete,
            commands::clip_clear,
            commands::mode_list,
            commands::mode_save,
            commands::mode_delete,
            commands::mode_activate,
            commands::routine_list,
            commands::routine_save,
            commands::routine_toggle,
            commands::routine_delete,
            commands::routine_run,
            commands::wallpaper_next,
            commands::setting_get,
            commands::setting_set,
            commands::settings_get_all,
            commands::open_settings,
            commands::apply_window_settings,
            commands::autostart_get,
            commands::autostart_set,
            commands::progress_push,
            commands::progress_clear,
            commands::battery_get,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Isle");
}
