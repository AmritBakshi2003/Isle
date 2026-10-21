//! Live progress manager - single channel, all sources.
//!
//! DESIGN: every progress source sends Msg into a bounded sync_channel.
//! One manager thread is the sole consumer and emits Tauri events.
//! No source ever spawns its own emitter; there is exactly one emitter thread.
//!
//! Sources:
//!   Battery     - Windows.Devices.Power.Battery::ReportUpdated (true event-driven)
//!   BITS        - IBackgroundCopyManager polled every 5s ONLY while jobs are active;
//!                 widens to 15s when idle.
//!   App install - Windows Installer service (msiserver) 3s idle / 2s active poll.
//!   Manual      - progress_push / progress_clear commands via same channel.

use serde_json::json;
use std::sync::mpsc::{sync_channel, SyncSender};
use tauri::{AppHandle, Emitter, Manager};

// -- message ------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Msg {
    Progress { value: i64, label: String },
    Clear,
    Battery { charging: bool, level: Option<f64> },
}

// -- handle (shared with Tauri State + commands) ------------------------------

/// SyncSender<T: Send> is both Send AND Sync => works as tauri::State.
pub struct ProgressHandle(pub SyncSender<Msg>);

impl ProgressHandle {
    pub fn push(&self, value: i64, label: impl Into<String>) {
        let _ = self.0.try_send(Msg::Progress { value, label: label.into() });
    }
    pub fn clear(&self) {
        let _ = self.0.try_send(Msg::Clear);
    }
}

// -- entry point --------------------------------------------------------------

pub fn start(app: AppHandle) -> ProgressHandle {
    let (tx, rx) = sync_channel::<Msg>(64);

    #[cfg(target_os = "windows")]
    {
        start_battery_watcher(app.clone(), tx.clone());
        start_bits_watcher(app.clone(), tx.clone());
        start_install_watcher(app.clone(), tx.clone());
    }

    // Single consumer thread: emits all Tauri events.
    std::thread::spawn({
        let app = app;
        move || {
            for msg in rx {
                match msg {
                    Msg::Progress { value, label } => {
                        let _ = app.emit("progress://update", json!({ "value": value, "label": label }));
                    }
                    Msg::Clear => {
                        let _ = app.emit("progress://clear", ());
                    }
                    Msg::Battery { charging, level } => {
                        let _ = app.emit("battery://update", json!({ "charging": charging, "level": level }));
                    }
                }
            }
        }
    });

    ProgressHandle(tx)
}

// -- Windows implementations --------------------------------------------------

#[cfg(target_os = "windows")]
fn start_battery_watcher(_app: AppHandle, tx: SyncSender<Msg>) {
    use windows::{
        Devices::Power::Battery,
        Foundation::TypedEventHandler,
        System::Power::BatteryStatus,
        Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
    };

    std::thread::spawn(move || {
        unsafe { let _ = CoInitializeEx(None, COINIT_MULTITHREADED); }

        let bat = match Battery::AggregateBattery() {
            Ok(b) => b,
            Err(e) => { eprintln!("[isle] battery unavailable: {e}"); return; }
        };

        let read = |bat: &Battery| -> Msg {
            let Ok(rep) = bat.GetReport() else {
                return Msg::Battery { charging: false, level: None };
            };
            let charging = matches!(rep.Status(), Ok(BatteryStatus::Charging) | Ok(BatteryStatus::Idle));
            let level = (|| -> Option<f64> {
                let full = rep.FullChargeCapacityInMilliwattHours().ok()?.Value().ok()? as f64;
                let rem  = rep.RemainingCapacityInMilliwattHours().ok()?.Value().ok()? as f64;
                if full > 0.0 { Some((rem / full).clamp(0.0, 1.0)) } else { None }
            })();
            Msg::Battery { charging, level }
        };

        // Send initial state immediately.
        let _ = tx.try_send(read(&bat));

        // Subscribe - fired by Windows when charging state or level changes.
        // Zero CPU when battery is stable.
        let bat2 = bat.clone();
        let tx2  = tx.clone();
        let _ = bat.ReportUpdated(&TypedEventHandler::new(move |_, _| {
            let _ = tx2.try_send(read(&bat2));
            Ok(())
        }));

        // Keep thread alive; WinRT delivers callbacks on its own thread pool.
        loop { std::thread::park(); }
    });
}

// BITS: poll every 5s while active, 15s when idle.
#[cfg(target_os = "windows")]
fn start_bits_watcher(app: AppHandle, tx: SyncSender<Msg>) {
    use std::time::Duration;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    std::thread::spawn(move || {
        unsafe { let _ = CoInitializeEx(None, COINIT_MULTITHREADED); }
        let mut was_active = false;

        loop {
            match poll_bits() {
                Ok((v, label)) if v >= 0 => {
                    was_active = true;
                    // OPT-IN: this reports every BITS job on the machine, not
                    // just Isle's. Windows Update, the Store, browser and
                    // Defender updates all use BITS, so on by default it made
                    // the pill announce downloads the user never started.
                    let enabled = app.try_state::<crate::db::Db>()
                        .and_then(|db| db.setting("progress_download"))
                        .map(|s| s == "true")
                        .unwrap_or(false);
                    if enabled {
                        let _ = tx.try_send(Msg::Progress { value: v, label });
                    }
                    std::thread::sleep(Duration::from_secs(5));
                }
                _ => {
                    if was_active {
                        let _ = tx.try_send(Msg::Clear);
                        was_active = false;
                    }
                    // Nothing copying: a COM round-trip every 15 s was showing up
                    // as a permanent wake-up. A minute is plenty to notice a new
                    // job (the first active poll reports it immediately).
                    std::thread::sleep(Duration::from_secs(60));
                }
            }
        }
    });
}

#[cfg(target_os = "windows")]
fn poll_bits() -> windows::core::Result<(i64, String)> {
    use windows::{
        core::GUID,
        Win32::Networking::BackgroundIntelligentTransferService::*,
        Win32::System::Com::*,
    };

    #[allow(non_upper_case_globals)]
    const CLSID_BackgroundCopyManager: GUID = GUID::from_values(
        0x4991_D34B, 0x80A1, 0x4291,
        [0x83, 0xB6, 0x33, 0x28, 0x36, 0x6B, 0x90, 0x97],
    );

    let mgr: IBackgroundCopyManager = unsafe {
        CoCreateInstance(&CLSID_BackgroundCopyManager, None, CLSCTX_LOCAL_SERVER)?
    };
    let jobs = unsafe { mgr.EnumJobs(0)? };

    let (mut xferred, mut total, mut active) = (0u64, 0u64, 0u32);
    let mut names: Vec<String> = Vec::new();

    loop {
        // windows-rs 0.58: Next takes &mut [Option<T>] slice (no celt arg).
        let mut slot = [None::<IBackgroundCopyJob>];
        let mut fetched: u32 = 0;
        let _ = unsafe { jobs.Next(&mut slot, &mut fetched) };
        if fetched == 0 { break; }
        let job = match slot.into_iter().next().flatten() { Some(j) => j, None => break };

        let state = unsafe { job.GetState()? };
        if state != BG_JOB_STATE_TRANSFERRING && state != BG_JOB_STATE_QUEUED { continue; }

        // GetProgress is an out-pointer in windows-rs 0.58.
        let mut p: BG_JOB_PROGRESS = unsafe { std::mem::zeroed() };
        unsafe { job.GetProgress(&mut p)? };
        if p.BytesTotal == u64::MAX { continue; }
        xferred += p.BytesTransferred;
        total   += p.BytesTotal;
        active  += 1;
        // NAME THE SOURCE. "Downloading..." on its own is unattributable —
        // the job usually belongs to Windows or a browser, not to Isle.
        if names.len() < 2 {
            if let Some(n) = job_display_name(&job) {
                if !n.is_empty() { names.push(n); }
            }
        }
    }

    if active == 0 || total == 0 { return Ok((-1, String::new())); }

    let pct = ((xferred as f64 / total as f64) * 100.0).round() as i64;
    Ok((pct.clamp(0, 99), bits_label(active, &names)))
}

/// Label for the pill: name the BITS job so "Downloading..." is never
/// unattributable. Split out from the COM code so the wording is testable.
#[cfg(target_os = "windows")]
fn bits_label(active: u32, names: &[String]) -> String {
    let first = names.first().map(|s| s.trim()).filter(|s| !s.is_empty());
    if active == 0 { return String::new(); }
    match first {
        Some(name) if active == 1 => format!("Downloading {name}..."),
        Some(name)              => format!("Downloading {name} (+{} more)...", active - 1),
        None if active == 1     => "Downloading...".into(),
        None                    => format!("{active} downloads..."),
    }
}

/// Read a BITS job's display name. The string is allocated by COM, so it has
/// to be released with CoTaskMemFree or every 5-second poll would leak.
#[cfg(target_os = "windows")]
fn job_display_name(
    job: &windows::Win32::Networking::BackgroundIntelligentTransferService::IBackgroundCopyJob,
) -> Option<String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    let ptr = unsafe { job.GetDisplayName() }.ok()?;
    let s = unsafe { ptr.to_string() }.ok()?;
    unsafe { CoTaskMemFree(Some(ptr.as_ptr() as *const _)) };
    Some(s)
}

// App install: watch Windows Installer service. 3s idle poll, 2s active poll.
#[cfg(target_os = "windows")]
fn start_install_watcher(app: AppHandle, tx: SyncSender<Msg>) {
    use std::time::Duration;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    std::thread::spawn(move || {
        unsafe { let _ = CoInitializeEx(None, COINIT_MULTITHREADED); }

        loop {
            std::thread::sleep(Duration::from_secs(3));

            // Read the setting BEFORE touching the service manager: with install
            // reporting off we should not be querying the SCM every 3 seconds for
            // the whole session. (msiserver is idle almost all of the time.)
            // OPT-IN: msiserver is started by Windows Update, Office, drivers
            // and any MSI/MSIX install, so this is off unless asked for.
            let enabled = app.try_state::<crate::db::Db>()
                .and_then(|db| db.setting("progress_app_install"))
                .map(|s| s == "true")
                .unwrap_or(false);

            if !enabled {
                std::thread::sleep(Duration::from_secs(27));
                continue;
            }

            if !msiserver_running() { continue; }

            let _ = tx.try_send(Msg::Progress { value: 20, label: "Installing...".into() });

            let mut secs: u64 = 0;
            while msiserver_running() {
                std::thread::sleep(Duration::from_secs(2));
                secs += 2;
                // Logarithmic creep 20->90 so user sees movement without false 100%.
                // Cast to i64 before negating (u64 cannot be negated directly).
                let v = (20.0 + 70.0 * (1.0 - (-(secs as i64) as f64 / 120.0).exp())) as i64;
                let _ = tx.try_send(Msg::Progress { value: v.clamp(20, 90), label: "Installing...".into() });
            }

            let _ = tx.try_send(Msg::Progress { value: 100, label: "Installed!".into() });
            std::thread::sleep(Duration::from_secs(2));
            let _ = tx.try_send(Msg::Clear);
        }
    });
}

#[cfg(target_os = "windows")]
fn msiserver_running() -> bool {
    use windows::{ core::w, Win32::System::Services::* };
    unsafe {
        let scm = match OpenSCManagerW(None, None, SC_MANAGER_CONNECT) {
            Ok(h) => h, Err(_) => return false,
        };
        let svc = match OpenServiceW(scm, w!("msiserver"), SERVICE_QUERY_STATUS) {
            Ok(h) => h,
            Err(_) => { let _ = CloseServiceHandle(scm); return false; }
        };
        let mut st = SERVICE_STATUS::default();
        let running = QueryServiceStatus(svc, &mut st).is_ok()
            && st.dwCurrentState == SERVICE_RUNNING;
        let _ = CloseServiceHandle(svc);
        let _ = CloseServiceHandle(scm);
        running
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::bits_label;

    fn names(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn bits_label_names_a_single_job() {
        assert_eq!(bits_label(1, &names(&["Windows Update"])), "Downloading Windows Update...");
    }

    #[test]
    fn bits_label_summarises_extra_jobs() {
        assert_eq!(
            bits_label(3, &names(&["Microsoft Store"])),
            "Downloading Microsoft Store (+2 more)..."
        );
    }

    #[test]
    fn bits_label_falls_back_when_the_name_is_missing() {
        assert_eq!(bits_label(1, &names(&[])), "Downloading...");
        assert_eq!(bits_label(2, &names(&["   "])), "2 downloads...");
    }

    #[test]
    fn bits_label_is_empty_when_nothing_is_copying() {
        assert_eq!(bits_label(0, &names(&["x"])), "");
    }
}
