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
mod apps;
mod clipboard;
mod commands;
mod db;
mod icon;
mod island;
mod logger;
mod media;
mod progress;
mod scheduler;
mod system;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
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
            // Note: ensure_single_instance uses default port (47123) since DB isn't available yet
            if !island::ensure_single_instance(&handle) {
                eprintln!("[isle] another instance is already running — signalled it to show. Exiting.");
                app.handle().exit(0);
                return Ok(());
            }

            // 1. Database
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = db::Db::open(&dir.join("isle.db"))?;
            app.manage(db);

            // Re-extract icons with old cache versions (non-fatal)
            {
                let db = app.state::<db::Db>();
                let _ = commands::reextract_old_icons(app.handle().clone(), db);
            }

            // 2-4. Background services
            app.manage(scheduler::start(handle.clone()));
            media::start(handle.clone());
            clipboard::start(handle.clone());
            app.manage(progress::start(handle.clone())); // single-channel progress manager



            // Check clipboard module setting
            {
                let db = app.state::<db::Db>();
                let clipboard_enabled = db.setting("module_clipboard")
                    .map(|v| v == "true")
                    .unwrap_or(true);
                clipboard::set_enabled(clipboard_enabled);
            }

            // 5. System state detection (fullscreen, battery saver, idle)
            app.manage(system::start(handle.clone()));

            // Initialize logger from DB settings (after DB is managed)
            logger::init_from_db(&handle);

            // 5. Window placement
            island::resize(&handle, false);

            // 6. Register hotkey from DB (non-fatal — conflict just logs a warning)
            {
                let db = app.state::<db::Db>();
                let toggle_hk = db.setting("hotkey_toggle")
                    .unwrap_or_else(|| "ctrl+alt+space".to_string());
                island::register_hotkey(&handle, &toggle_hk);
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
            commands::event_update,
            commands::events_for_day,
            commands::month_event_days,
            commands::clip_list,
            commands::clip_copy,
            commands::clip_pin,
            commands::clip_delete,
            commands::clip_clear,
            commands::clip_open_file,
            commands::clip_delete_file,
            commands::clip_paste_files,
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
            commands::window_hide,
            commands::window_show,
            commands::autostart_get,
            commands::autostart_set,
            commands::progress_push,
            commands::progress_clear,
            commands::battery_get,
            commands::emit_settings_changed,
            system::auto_hide_get,
            system::auto_hide_set,
            system::auto_hide_time_get,
            system::auto_hide_time_set,
            system::hide_fullscreen_get,
            system::hide_fullscreen_set,
            system::hide_battery_saver_get,
            system::hide_battery_saver_set,
            system::update_hide_settings,
            commands::reregister_hotkeys,
            commands::reload_logger,
            commands::clipboard_enabled_get,
            commands::clipboard_enabled_set,
            commands::open_native,
            commands::enumerate_apps,
            commands::app_shortcuts_list,
            commands::app_shortcut_add,
            commands::app_shortcut_remove,
            commands::app_shortcut_launch,
            commands::app_shortcut_icon,
            commands::clear_all_icons,
            commands::regenerate_shortcut_icon,
            commands::reextract_old_icons,
            commands::extract_app_icon,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Isle");
}
