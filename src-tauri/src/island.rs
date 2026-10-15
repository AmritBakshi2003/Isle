//! The island window: size, position and the global hotkey.
//!
//! WHY the OS window is resized: a transparent always-on-top window still
//! captures mouse clicks over its whole rectangle. So the window is only as big
//! as the island needs. The UI (see src/App.svelte) must call `island_resize`
//! BEFORE it starts the expand animation, and AFTER the collapse animation ends.
//!
//! Sizes are LOGICAL pixels and must match the CSS in src/app.css:
//!   collapsed pill 190x38, expanded pill 440x326, plus transparent margins that
//!   leave room for the spring overshoot and the soft shadow.

use tauri::{
    plugin::TauriPlugin, AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Wry,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const COLLAPSED: (f64, f64) = (210.0, 46.0);
pub const EXPANDED:  (f64, f64) = (500.0, 352.0);

// ── single-instance guard ────────────────────────────────────────────────────
// Try to bind a local TCP port. If it's already taken, another Isle is running.
// First instance: binds port 47123, spawns a listener thread that shows the
// pill when a connection arrives from a second instance.
// Second instance: connect to port 47123 (wakes first instance), then exit.

pub fn ensure_single_instance(app: &AppHandle) -> bool {
    use std::net::TcpListener;
    match TcpListener::bind("127.0.0.1:47123") {
        Ok(listener) => {
            // We are the first instance. Spawn a listener thread that
            // activates the pill whenever a second instance tries to connect.
            let app2 = app.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    drop(stream); // we don't need any data, just the signal
                    if let Some(w) = app2.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                    let _ = app2.emit("island://toggle", ());
                    eprintln!("[isle] second instance detected — showing existing window");
                }
            });
            true  // we are the first instance
        }
        Err(_) => {
            // Port taken → another Isle is running. Signal it to show itself.
            use std::net::TcpStream;
            let _ = TcpStream::connect("127.0.0.1:47123");
            false
        }
    }
}

// ── window helpers ────────────────────────────────────────────────────────────

/// Resize the window and re-centre it at the top of the primary monitor.
pub fn resize(app: &AppHandle, expanded: bool) {
    let Some(w) = app.get_webview_window("main") else { return };
    let (lw, lh) = if expanded { EXPANDED } else { COLLAPSED };
    let _ = w.set_size(LogicalSize::new(lw, lh));
    if let Ok(Some(m)) = w.primary_monitor() {
        let sf = m.scale_factor();
        let pw = (lw * sf) as i32;
        let x  = m.position().x + (m.size().width as i32 - pw) / 2;
        let _  = w.set_position(PhysicalPosition::new(x, m.position().y));
    }
}

#[tauri::command]
pub fn island_resize(app: AppHandle, expanded: bool) {
    resize(&app, expanded);
}

// ── settings window (pre-created in tauri.conf.json, hidden by default) ─────
// The window is created at startup alongside the main pill but starts hidden.
// Close events are intercepted to hide instead of destroy, so it can be reopened.

/// Show the settings window. The window is pre-created in tauri.conf.json but hidden.
pub fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
    } else {
        eprintln!("[isle] settings window not found in config");
    }
}

// ── hotkey plugin ─────────────────────────────────────────────────────────────
//
// We build the plugin WITHOUT pre-registering any shortcuts (no .with_shortcuts).
// The actual shortcuts are registered in register_hotkey() / register_settings_hotkey(),
// called from lib.rs setup(). This way a registration failure is non-fatal.

pub fn hotkey_plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed { return; }
            let s = shortcut.into_string().to_lowercase();
            if s.contains("space") {
                // Toggle shortcut → show/focus the pill
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_focus();
                }
                let _ = app.emit("island://toggle", ());
            } else {
                // Settings shortcut → open settings window (lazy creation)
                show_settings(app);
            }
        })
        .build()
}

/// Register Ctrl+Alt+Space after the app is fully set up.
/// If the shortcut is already taken by another app (or a stale Isle instance),
/// we log a warning but do NOT crash.
pub fn register_hotkey(app: &AppHandle) {
    match app.global_shortcut().register("ctrl+alt+space") {
        Ok(_)  => eprintln!("[isle] hotkey Ctrl+Alt+Space registered"),
        Err(e) => eprintln!("[isle] hotkey registration skipped ({}). Use tray to toggle.", e),
    }
}

/// Register the settings hotkey (default: ctrl+alt+comma) read from the DB.
/// Uses the same plugin handler registered in hotkey_plugin() — any shortcut
/// that does NOT contain "space" will open the settings window.
pub fn register_settings_hotkey(app: &AppHandle, hotkey: &str) {
    match app.global_shortcut().register(hotkey) {
        Ok(_)  => eprintln!("[isle] settings hotkey {} registered", hotkey),
        Err(e) => eprintln!("[isle] settings hotkey skipped ({})", e),
    }
}
