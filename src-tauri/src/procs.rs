//! Per-app resource monitor for the Workspace dashboard.
//!
//! One row per application that owns a visible window: how much of the WHOLE
//! machine it is using right now (100 % = every core / all RAM / the GPU), plus
//! whether Isle is currently keeping it on top.
//!
//! SOURCES
//!   CPU     `GetProcessTimes` kernel+user deltas between two samples. We keep
//!           the previous CPU total per pid and divide by the measured wall
//!           time * core count, so the number is "percent of the computer",
//!           exactly like Task Manager's default view.
//!   memory  `GetProcessMemoryInfo().WorkingSetSize` over `GlobalMemoryStatusEx`
//!           total physical RAM.
//!   GPU     the `GPU Engine(*)\Utilization Percentage` performance counters via
//!           PDH. Instance names are `pid_<pid>_luid_..._engtype_<type>`; the
//!           per-engine values are summed per pid and clamped. Some machines
//!           expose no GPU counters at all (VMs, old drivers) — then
//!           `gpu_available` is false and the UI hides the column.
//!
//! DUTY CYCLE (invariant #1): the sampler thread is parked on a condvar until
//! the Workspace tab mounts (`procs_set(true)`), it sleeps 1 s between ticks,
//! and it skips ticks while the island window is hidden. No polling otherwise.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Condvar, LazyLock, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// One running application.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AppRow {
    /// handle of the window we would float (0 = none)
    pub hwnd: i64,
    pub pid: u32,
    /// executable name, e.g. `chrome.exe`
    pub app: String,
    /// window title of the representative window
    pub title: String,
    /// percent of the whole machine's CPU (all cores = 100)
    pub cpu_pct: f32,
    /// percent of installed physical memory
    pub mem_pct: f32,
    /// percent of the GPU (sum over engines, clamped)
    pub gpu_pct: f32,
    /// Isle currently keeps this window on top
    pub topmost: bool,
}

/// One tick, exactly as the UI receives it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AppsSample {
    pub rows: Vec<AppRow>,
    /// false when this machine exposes no GPU Engine counters
    pub gpu_available: bool,
    pub cores: u32,
    pub mem_total_mb: u64,
    pub ts: i64,
}

/// How many apps the panel shows (sorted by total load, heaviest first).
pub const MAX_ROWS: usize = 8;

// -- duty-cycle gate (same pattern as network.rs) -----------------------------

struct Gate {
    on: Mutex<bool>,
    cv: Condvar,
}

static GATE: LazyLock<Gate> = LazyLock::new(|| Gate { on: Mutex::new(false), cv: Condvar::new() });
static LAST: LazyLock<Mutex<Option<AppsSample>>> = LazyLock::new(|| Mutex::new(None));

/// Called by the Workspace tab on mount/unmount.
pub fn set_enabled(on: bool) {
    if let Ok(mut g) = GATE.on.lock() {
        *g = on;
    }
    GATE.cv.notify_all();
}

/// Last tick, so the card paints instantly on open.
pub fn snapshot() -> Option<AppsSample> {
    LAST.lock().ok().and_then(|l| l.clone())
}

pub fn start(app: AppHandle) {
    #[cfg(windows)]
    std::thread::spawn(move || sampler_loop(app));
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
fn sampler_loop(app: AppHandle) {
    let mut state = imp::Sampler::new();
    loop {
        let guard = match GATE.on.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let mut on = guard;
        while !*on {
            on = match GATE.cv.wait(on) {
                Ok(g) => g,
                Err(_) => return,
            };
        }
        drop(on);

        if crate::network::window_visible(&app) {
            let sample = state.tick();
            if let Ok(mut l) = LAST.lock() {
                *l = Some(sample.clone());
            }
            let _ = app.emit("workspace://apps", sample);
        }

        let guard = match GATE.on.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let (_g, _t) = GATE
            .cv
            .wait_timeout_while(guard, Duration::from_secs(1), |on| *on)
            .unwrap_or_else(|e| e.into_inner());
    }
}

// -- pure maths (unit-tested, no windows deps) -------------------------------

/// `pid_4321_luid_0x0_0x0_phys_0_eng_0_engtype_3D` -> 4321.
pub fn instance_pid(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("pid_")?;
    let digits = rest.split('_').next()?;
    digits.parse().ok()
}

/// CPU usage as a percentage of the whole machine.
///
/// `delta_cpu` and `elapsed` are both in 100 ns units; a process burning one
/// core on an 8-core box is 12.5 %, not 100 %.
pub fn cpu_percent(delta_cpu_100ns: u64, elapsed_100ns: u64, cores: u32) -> f32 {
    if elapsed_100ns == 0 || cores == 0 {
        return 0.0;
    }
    let cores = cores as f64;
    let pct = (delta_cpu_100ns as f64) * 100.0 / ((elapsed_100ns as f64) * cores);
    pct.clamp(0.0, 100.0) as f32
}

/// `part` as a percentage of `whole`, rounded to one decimal and clamped.
pub fn percent(part: u64, whole: u64) -> f32 {
    if whole == 0 {
        return 0.0;
    }
    (((part as f64) * 100.0 / (whole as f64)).clamp(0.0, 100.0) * 10.0).round() as f32 / 10.0
}

/// Sum per-engine GPU utilization per pid, clamped to 100 % per process.
pub fn merge_gpu(engines: &[(String, f64)]) -> HashMap<u32, f32> {
    let mut out: HashMap<u32, f32> = HashMap::new();
    for (name, value) in engines {
        let Some(pid) = instance_pid(name) else { continue };
        if !value.is_finite() {
            continue;
        }
        let slot = out.entry(pid).or_insert(0.0);
        *slot = (*slot + *value as f32).clamp(0.0, 100.0);
    }
    out
}

/// Sort heaviest first and keep the panel's limit.
pub fn rank(mut rows: Vec<AppRow>) -> Vec<AppRow> {
    rows.sort_by(|a, b| {
        let load = |r: &AppRow| r.cpu_pct + r.gpu_pct + r.mem_pct;
        load(b).partial_cmp(&load(a)).unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(MAX_ROWS);
    rows
}

// -- Tauri commands (registered in lib.rs, mocked in src/lib/api.js) ---------

/// Called by the Workspace tab on mount/unmount.
#[tauri::command]
pub fn procs_set(enabled: bool) -> Result<(), String> {
    set_enabled(enabled);
    Ok(())
}

#[tauri::command]
pub fn procs_snapshot() -> Option<AppsSample> {
    snapshot()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::topmost;
    use std::time::Instant;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
    use windows::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
        PdhOpenQueryW, PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W,
        PDH_FMT_DOUBLE, PDH_MORE_DATA,
    };
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    /// PDH query for per-engine GPU utilization. Created lazily; `None` means
    /// this machine has no usable GPU counters (the UI hides the column).
    struct GpuQuery {
        query: isize,
        counter: isize,
        primed: bool,
    }

    impl GpuQuery {
        fn open() -> Option<Self> {
            unsafe {
                let mut query = 0isize;
                if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                    return None;
                }
                let mut counter = 0isize;
                // PdhAddEnglishCounterW so a localised Windows still resolves it.
                if PdhAddEnglishCounterW(
                    query,
                    w!("\\GPU Engine(*)\\Utilization Percentage"),
                    0,
                    &mut counter,
                ) != 0
                {
                    PdhCloseQuery(query);
                    return None;
                }
                Some(Self { query, counter, primed: false })
            }
        }

        /// Per-instance values for this tick. Empty on the first call: rate
        /// counters need two collections before they mean anything.
        fn read(&mut self) -> Option<Vec<(String, f64)>> {
            unsafe {
                if PdhCollectQueryData(self.query) != 0 {
                    return None;
                }
                if !self.primed {
                    self.primed = true;
                    return Some(Vec::new());
                }

                let mut size = 0u32;
                let mut count = 0u32;
                let rc = PdhGetFormattedCounterArrayW(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    &mut size,
                    &mut count,
                    None,
                );
                if rc != PDH_MORE_DATA && rc != 0 {
                    return None;
                }
                if count == 0 || size == 0 {
                    return Some(Vec::new());
                }

                // The item array holds f64/PWSTR, so the buffer must be 8-byte
                // aligned (a Vec<u8> is not).
                let words = (size as usize + 7) / 8;
                let mut buf: Vec<u64> = vec![0; words];
                let rc = PdhGetFormattedCounterArrayW(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    &mut size,
                    &mut count,
                    Some(buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W),
                );
                if rc != 0 {
                    return None;
                }

                let items = std::slice::from_raw_parts(
                    buf.as_ptr() as *const PDH_FMT_COUNTERVALUE_ITEM_W,
                    count as usize,
                );
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    let status = item.FmtValue.CStatus;
                    if status != PDH_CSTATUS_VALID_DATA && status != PDH_CSTATUS_NEW_DATA {
                        continue;
                    }
                    let name = item.szName.to_string().unwrap_or_default();
                    let value = item.FmtValue.Anonymous.doubleValue;
                    out.push((name, value));
                }
                Some(out)
            }
        }
    }

    impl Drop for GpuQuery {
        fn drop(&mut self) {
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }

    fn filetime_100ns(ft: FILETIME) -> u64 {
        ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
    }

    pub(super) fn total_ram() -> u64 {
        let mut m = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        unsafe {
            if GlobalMemoryStatusEx(&mut m).is_err() {
                return 0;
            }
            m.ullTotalPhys
        }
    }

    /// Kernel + user time, in 100 ns units.
    pub(super) fn cpu_100ns(h: HANDLE) -> Option<u64> {
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        unsafe {
            GetProcessTimes(h, &mut created, &mut exited, &mut kernel, &mut user).ok()?;
        }
        Some(filetime_100ns(kernel) + filetime_100ns(user))
    }

    pub(super) fn working_set(h: HANDLE) -> Option<u64> {
        let mut pmc = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        unsafe {
            GetProcessMemoryInfo(h, &mut pmc, pmc.cb).ok()?;
        }
        Some(pmc.WorkingSetSize as u64)
    }

    pub struct Sampler {
        prev: HashMap<u32, (u64, Instant)>,
        gpu: Option<GpuQuery>,
        gpu_checked: bool,
        cores: u32,
        ram: u64,
    }

    impl Sampler {
        pub fn new() -> Self {
            Self {
                prev: HashMap::new(),
                gpu: None,
                gpu_checked: false,
                cores: std::thread::available_parallelism()
                    .map(|n| n.get() as u32)
                    .unwrap_or(1),
                ram: total_ram(),
            }
        }

        pub fn tick(&mut self) -> AppsSample {
            // GPU counters are optional and expensive to open, so try once.
            if !self.gpu_checked {
                self.gpu_checked = true;
                self.gpu = GpuQuery::open();
            }
            let gpu_available = self.gpu.is_some();
            let engines = self.gpu.as_mut().and_then(|g| g.read()).unwrap_or_default();
            let gpu_by_pid = merge_gpu(&engines);

            let windows = topmost::list().unwrap_or_default();
            let mut seen: HashMap<u32, usize> = HashMap::new();
            let mut rows: Vec<AppRow> = Vec::new();

            for w in windows {
                let h = unsafe {
                    OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, w.pid)
                };
                let Ok(h) = h else { continue };
                let cpu_now = cpu_100ns(h);
                let mem = working_set(h);
                unsafe {
                    let _ = CloseHandle(h);
                }

                let now = Instant::now();
                let (cpu_pct, prev_cpu) = match (cpu_now, self.prev.get(&w.pid)) {
                    (Some(cur), Some((prev, at))) => {
                        let elapsed = now.duration_since(*at).as_nanos() as u64 / 100;
                        (cpu_percent(cur.saturating_sub(*prev), elapsed, self.cores), cur)
                    }
                    // first sighting: we cannot know the rate yet
                    (Some(cur), None) => (0.0, cur),
                    (None, _) => (0.0, 0),
                };
                if cpu_now.is_some() {
                    *self.prev.entry(w.pid).or_insert((prev_cpu, now)) = (prev_cpu, now);
                }

                let row = AppRow {
                    hwnd: w.hwnd,
                    pid: w.pid,
                    app: w.process,
                    title: w.title,
                    cpu_pct,
                    mem_pct: mem.map(|m| percent(m, self.ram)).unwrap_or(0.0),
                    gpu_pct: gpu_by_pid.get(&w.pid).copied().unwrap_or(0.0),
                    topmost: topmost::is_floating(w.hwnd),
                };

                // several windows can belong to one process: keep one row
                match seen.get(&row.pid) {
                    Some(&idx) => {
                        let existing: &mut AppRow = &mut rows[idx];
                        existing.cpu_pct = existing.cpu_pct.max(row.cpu_pct);
                        existing.gpu_pct = existing.gpu_pct.max(row.gpu_pct);
                        existing.mem_pct = existing.mem_pct.max(row.mem_pct);
                        existing.topmost = existing.topmost || row.topmost;
                    }
                    None => {
                        seen.insert(row.pid, rows.len());
                        rows.push(row);
                    }
                }
            }

            // forget processes that closed, so the map cannot grow forever
            self.prev.retain(|pid, _| seen.contains_key(pid));

            AppsSample {
                rows: rank(rows),
                gpu_available,
                cores: self.cores,
                mem_total_mb: self.ram / (1024 * 1024),
                ts: crate::db::now(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_names_yield_a_pid() {
        assert_eq!(
            instance_pid("pid_4321_luid_0x00000000_0x0000C4F3_phys_0_eng_0_engtype_3D"),
            Some(4321)
        );
        assert_eq!(instance_pid("pid_12"), Some(12));
        assert_eq!(instance_pid("gpu_something"), None);
        assert_eq!(instance_pid("pid_notanumber_x"), None);
    }

    #[test]
    fn one_saturated_core_is_a_fraction_of_the_machine() {
        // 1 s of CPU time over 1 s of wall time on 8 cores = 12.5 %
        assert!((cpu_percent(10_000_000, 10_000_000, 8) - 12.5).abs() < 0.01);
        // the same on 1 core saturates the machine
        assert!((cpu_percent(10_000_000, 10_000_000, 1) - 100.0).abs() < 0.01);
    }

    #[test]
    fn cpu_is_clamped_and_never_divides_by_zero() {
        assert_eq!(cpu_percent(10, 0, 8), 0.0);
        assert_eq!(cpu_percent(10, 10, 0), 0.0);
        assert_eq!(cpu_percent(u64::MAX, 1, 1), 100.0);
    }

    #[test]
    fn memory_percent_is_rounded_and_clamped() {
        assert_eq!(percent(512, 1024), 50.0);
        assert_eq!(percent(0, 1024), 0.0);
        assert_eq!(percent(1, 0), 0.0);
        assert_eq!(percent(2048, 1024), 100.0);
    }

    #[test]
    fn gpu_engines_are_summed_per_process_and_clamped() {
        let engines = vec![
            ("pid_100_luid_0_0_phys_0_eng_0_engtype_3D".to_string(), 40.0),
            ("pid_100_luid_0_0_phys_0_eng_1_engtype_Copy".to_string(), 25.5),
            ("pid_200_luid_0_0_phys_0_eng_0_engtype_3D".to_string(), 90.0),
            ("pid_200_luid_0_0_phys_0_eng_1_engtype_VideoDecode".to_string(), 30.0),
            ("not_a_pid".to_string(), 50.0),
        ];
        let m = merge_gpu(&engines);
        assert_eq!(m.get(&100), Some(&65.5_f32));
        assert_eq!(m.get(&200), Some(&100.0_f32), "engines of one pid must clamp at 100");
        assert_eq!(m.len(), 2, "unparseable instances are ignored");
    }

    #[test]
    fn ranking_puts_the_heaviest_first_and_caps_the_list() {
        let row = |pid: u32, cpu: f32, mem: f32, gpu: f32| AppRow {
            pid,
            app: format!("p{pid}.exe"),
            cpu_pct: cpu,
            mem_pct: mem,
            gpu_pct: gpu,
            ..Default::default()
        };
        // pid 1 = 51 total, pid 3 = 55 once GPU counts, pid 2 = 20
        let mut rows = vec![row(1, 50.0, 1.0, 0.0), row(2, 0.0, 20.0, 0.0), row(3, 10.0, 0.0, 45.0)];
        for i in 0..12 {
            rows.push(row(100 + i, 1.0, 1.0, 1.0));
        }
        let ranked = rank(rows);
        assert_eq!(ranked.len(), MAX_ROWS);
        assert_eq!(ranked[0].pid, 3, "GPU load must count toward the score");
        assert_eq!(ranked[1].pid, 1);
        assert_eq!(ranked[2].pid, 2);
    }

    #[cfg(windows)]
    #[test]
    fn reads_this_machines_ram_and_cpu_times() {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        assert!(imp::total_ram() > 0, "GlobalMemoryStatusEx must report installed RAM");
        let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, std::process::id()) }
            .expect("OpenProcess on ourselves");
        assert!(imp::cpu_100ns(h).is_some(), "GetProcessTimes must work on ourselves");
        assert!(imp::working_set(h).is_some(), "GetProcessMemoryInfo must work on ourselves");
        unsafe {
            let _ = CloseHandle(h);
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_tick_produces_rows_without_panicking() {
        let mut s = imp::Sampler::new();
        let first = s.tick();
        let second = s.tick();
        assert_eq!(second.cores, std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1));
        assert!(second.mem_total_mb > 0);
        assert!(second.rows.len() <= MAX_ROWS);
        for r in &second.rows {
            assert!(!r.app.is_empty(), "every row must name a process: {r:?}");
            assert!((0.0..=100.0).contains(&r.cpu_pct));
            assert!((0.0..=100.0).contains(&r.mem_pct));
            assert!((0.0..=100.0).contains(&r.gpu_pct));
            assert_ne!(r.app.to_lowercase(), "isle.exe", "Isle must never list itself");
        }
        let _ = first;
    }
}
