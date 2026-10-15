//! System state detection: fullscreen, battery saver, idle timer
//!
//! Monitors Windows system state and emits events to the UI to hide/show the island.

use tauri::{AppHandle, Emitter, State, Manager};
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::db::Db;

#[cfg(target_os = "windows")]
use windows::{
    Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, IsZoomed, GetSystemMetrics,
        SM_CXSCREEN, SM_CYSCREEN,
    },
    Win32::UI::Input::KeyboardAndMouse::{
        GetLastInputInfo, LASTINPUTINFO,
    },
    Win32::System::Power::GetSystemPowerStatus,
};

/// Global flags for system state
#[derive(Clone)]
pub struct SystemState {
    pub is_fullscreen: Arc<AtomicBool>,
    pub is_battery_saver: Arc<AtomicBool>,
    pub auto_hide_idle: Arc<AtomicBool>,
    pub should_hide: Arc<AtomicBool>,
    // Cached settings for hide conditions
    pub hide_fullscreen: Arc<AtomicBool>,
    pub hide_battery_saver: Arc<AtomicBool>,
    pub auto_hide: Arc<AtomicBool>,
}

impl SystemState {
    pub fn new() -> Self {
        Self {
            is_fullscreen: Arc::new(AtomicBool::new(false)),
            is_battery_saver: Arc::new(AtomicBool::new(false)),
            auto_hide_idle: Arc::new(AtomicBool::new(false)),
            should_hide: Arc::new(AtomicBool::new(false)),
            hide_fullscreen: Arc::new(AtomicBool::new(false)),
            hide_battery_saver: Arc::new(AtomicBool::new(false)),
            auto_hide: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub fn start(app: AppHandle) -> SystemState {
    let state = SystemState::new();

    #[cfg(target_os = "windows")]
    {
        // Initialize hide settings from DB
        update_cached_settings(&state, &app);

        // Start fullscreen detection (polling every 1s)
        start_fullscreen_watcher(app.clone(), state.clone());

        // Start battery saver detection (via Windows Power API)
        start_battery_saver_watcher(app.clone(), state.clone());

        // Start idle detection (polling based on mouse/keyboard activity)
        start_idle_watcher(app.clone(), state.clone());
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
    }

    state
}

#[cfg(target_os = "windows")]
fn start_fullscreen_watcher(app: AppHandle, state: SystemState) {
    std::thread::spawn(move || {
        loop {
            let is_fullscreen = detect_fullscreen();
            let was_fullscreen = state.is_fullscreen.load(Ordering::Relaxed);

            if is_fullscreen != was_fullscreen {
                state.is_fullscreen.store(is_fullscreen, Ordering::Relaxed);
                check_hide_condition(&state, &app);
            }

            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

#[cfg(target_os = "windows")]
fn detect_fullscreen() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return false;
        }

        // Exclude desktop shell windows (Progman, WorkerW)
        let mut class_buf = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_buf);
        let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
        if class_name == "Progman" || class_name == "WorkerW" {
            return false;
        }

        let mut rect = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }

        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        // Check if window is maximized
        if IsZoomed(hwnd).as_bool() {
            return true;
        }

        // Check if window covers the entire screen
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        width >= screen_width && height >= screen_height
    }
}

#[cfg(target_os = "windows")]
fn start_battery_saver_watcher(app: AppHandle, state: SystemState) {
    std::thread::spawn(move || {
        // For now, use polling as a simpler approach
        // Full event-driven implementation would require window handle and complex notification handling
        loop {
            // Simple check via Windows Power Management API
            let battery_saver_enabled = detect_battery_saver();
            let was_enabled = state.is_battery_saver.load(Ordering::Relaxed);

            if battery_saver_enabled != was_enabled {
                state.is_battery_saver.store(battery_saver_enabled, Ordering::Relaxed);
                check_hide_condition(&state, &app);
            }

            std::thread::sleep(Duration::from_secs(5));
        }
    });
}

#[cfg(target_os = "windows")]
fn detect_battery_saver() -> bool {
    unsafe {
        let mut status = std::mem::zeroed();
        if GetSystemPowerStatus(&mut status).is_ok() {
            // SystemStatusFlag bit 0 indicates battery saver is on
            return (status.SystemStatusFlag & 1) == 1;
        }
    }
    false
}

#[cfg(target_os = "windows")]
fn start_idle_watcher(app: AppHandle, state: SystemState) {
    std::thread::spawn(move || {
        loop {
            // Check if auto_hide is enabled
            let db_handle = app.state::<Db>();
            let auto_hide_enabled = db_handle.setting("auto_hide")
                .map(|v| v == "true")
                .unwrap_or(false);

            if auto_hide_enabled {
                // Get idle threshold from DB
                let idle_threshold = db_handle.setting("auto_hide_time")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30);

                // Check idle time using GetLastInputInfo
                unsafe {
                    let mut last_input = LASTINPUTINFO {
                        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                        dwTime: 0,
                    };

                    if GetLastInputInfo(&mut last_input).as_bool() {
                        let idle_ms = get_tick_count() - last_input.dwTime;
                        let idle_seconds = idle_ms / 1000;

                        let was_should_hide = state.should_hide.load(Ordering::Relaxed);
                        let should_hide = idle_seconds > idle_threshold;

                        if should_hide != was_should_hide {
                            state.should_hide.store(should_hide, Ordering::Relaxed);
                            check_hide_condition(&state, &app);
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

#[cfg(target_os = "windows")]
fn get_tick_count() -> u32 {
    unsafe {
        // Use GetTickCount64 for 64-bit systems, truncate to u32
        let tick_count = windows::Win32::System::SystemInformation::GetTickCount64();
        (tick_count & 0xFFFFFFFF) as u32
    }
}

#[cfg(target_os = "windows")]
fn check_hide_condition(state: &SystemState, app: &AppHandle) {
    let hide_fullscreen = state.hide_fullscreen.load(Ordering::Relaxed);
    let hide_battery_saver = state.hide_battery_saver.load(Ordering::Relaxed);
    let auto_hide = state.auto_hide.load(Ordering::Relaxed);

    let is_fullscreen = state.is_fullscreen.load(Ordering::Relaxed);
    let is_battery_saver = state.is_battery_saver.load(Ordering::Relaxed);
    let should_hide_idle = state.should_hide.load(Ordering::Relaxed);

    let should_hide = (is_fullscreen && hide_fullscreen)
        || (is_battery_saver && hide_battery_saver)
        || (should_hide_idle && auto_hide);

    let _ = app.emit("system://hide", should_hide);
}

/// Update cached hide settings from DB and recompute hide condition
#[tauri::command]
pub fn update_hide_settings(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if let Some(state) = app.try_state::<SystemState>() {
            update_cached_settings(&state, &app);
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn update_cached_settings(state: &SystemState, app: &AppHandle) {
    let db = app.state::<Db>();
    let hide_fullscreen = db.setting("hide_fullscreen")
        .map(|v| v == "true")
        .unwrap_or(true);
    let hide_battery_saver = db.setting("hide_battery_saver")
        .map(|v| v == "true")
        .unwrap_or(false);
    let auto_hide = db.setting("auto_hide")
        .map(|v| v == "true")
        .unwrap_or(false);

    state.hide_fullscreen.store(hide_fullscreen, Ordering::Relaxed);
    state.hide_battery_saver.store(hide_battery_saver, Ordering::Relaxed);
    state.auto_hide.store(auto_hide, Ordering::Relaxed);

    check_hide_condition(state, app);
}

// ------------------ Commands for Settings UI ------------------

/// Check if auto-hide when idle is enabled
#[tauri::command]
pub fn auto_hide_get(db: State<Db>) -> bool {
    db.setting("auto_hide").map(|v| v == "true").unwrap_or(false)
}

/// Set auto-hide when idle setting
#[tauri::command]
pub fn auto_hide_set(db: State<Db>, app: AppHandle, enabled: bool) -> Result<(), String> {
    db.set_setting("auto_hide", if enabled { "true" } else { "false" })?;
    #[cfg(target_os = "windows")]
    {
        if let Some(state) = app.try_state::<SystemState>() {
            update_cached_settings(&state, &app);
        }
    }
    Ok(())
}

/// Get auto-hide idle time in seconds
#[tauri::command]
pub fn auto_hide_time_get(db: State<Db>) -> u32 {
    db.setting("auto_hide_time")
        .and_then(|s| s.parse().ok())
        .unwrap_or(30)
}

/// Set auto-hide idle time in seconds
#[tauri::command]
pub fn auto_hide_time_set(db: State<Db>, seconds: u32) -> Result<(), String> {
    db.set_setting("auto_hide_time", &seconds.to_string())
}

/// Check if hide on fullscreen is enabled
#[tauri::command]
pub fn hide_fullscreen_get(db: State<Db>) -> bool {
    db.setting("hide_fullscreen").map(|v| v == "true").unwrap_or(true)
}

/// Set hide on fullscreen setting
#[tauri::command]
pub fn hide_fullscreen_set(db: State<Db>, app: AppHandle, enabled: bool) -> Result<(), String> {
    db.set_setting("hide_fullscreen", if enabled { "true" } else { "false" })?;
    #[cfg(target_os = "windows")]
    {
        if let Some(state) = app.try_state::<SystemState>() {
            update_cached_settings(&state, &app);
        }
    }
    Ok(())
}

/// Check if hide on battery saver is enabled
#[tauri::command]
pub fn hide_battery_saver_get(db: State<Db>) -> bool {
    db.setting("hide_battery_saver").map(|v| v == "true").unwrap_or(false)
}

/// Set hide on battery saver setting
#[tauri::command]
pub fn hide_battery_saver_set(db: State<Db>, app: AppHandle, enabled: bool) -> Result<(), String> {
    db.set_setting("hide_battery_saver", if enabled { "true" } else { "false" })?;
    #[cfg(target_os = "windows")]
    {
        if let Some(state) = app.try_state::<SystemState>() {
            update_cached_settings(&state, &app);
        }
    }
    Ok(())
}
