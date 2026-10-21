//! Digital Wellbeing: an event-driven log of which tool you are actually in.
//!
//! HOW IT WORKS: SetWinEventHook(EVENT_SYSTEM_FOREGROUND, ...) asks Windows to
//! call us back ONLY when the foreground window changes. There is no clock, no
//! sampling and no enumeration — between switches the app is completely idle,
//! which is exactly what AGENTS.md invariant #1 asks for.
//!
//! THREADING (the part that is easy to get wrong): foreground events are
//! delivered to the message queue of the thread that installed the hook, so
//! the hooking thread MUST pump messages. We therefore use two threads:
//!   pump thread     — installs the hook and runs GetMessageW/DispatchMessageW
//!   consumer thread — owns the SQLite writes (never block the pump; never
//!                     nest Db::with, see db.rs)
//! The callback itself only pushes an HWND into a channel and must never panic
//! (release builds use panic = "abort").
//!
//! LIMITS (documented, not silent): a non-elevated hook does not receive
//! foreground events for elevated windows (UIPI), so admin tools will not
//! appear in the log.

use crate::db::{now, Db};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicIsize, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager};

/// One logged stretch in one application.
#[derive(Debug, Clone, Serialize)]
pub struct WellbeingRow {
    pub app: String,
    pub title: String,
    /// seconds spent in this app for the row
    pub seconds: i64,
    pub started: i64,
    pub ended: Option<i64>,
}

static ENABLED: AtomicBool = AtomicBool::new(true);
/// HWND of the last non-Isle foreground window — the Window Management Macro
/// floats this one, so both features share a single source of truth.
static LAST_FOREIGN: AtomicIsize = AtomicIsize::new(0);

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(v: bool) {
    ENABLED.store(v, Ordering::Relaxed);
}

pub fn last_foreign_hwnd() -> isize {
    LAST_FOREIGN.load(Ordering::Relaxed)
}

/// Start the hook + consumer. Called once from lib.rs setup().
pub fn start(app: AppHandle) {
    #[cfg(windows)]
    imp::start(app);
    #[cfg(not(windows))]
    let _ = app;
}

/// Close the currently-open session, if any (used on shutdown so the last
/// stretch is not left with a NULL end time).
pub fn close_open_session(db: &Db) {
    let t = now();
    let _ = db
        .with(|c| c.execute("UPDATE wellbeing SET ended=?1 WHERE ended IS NULL", [t]).map(|_| ()));
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, BOOL, HANDLE, HMODULE, HWND};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
        TranslateMessage, MSG, OBJID_WINDOW, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    };

    /// The hook callback publishes HWNDs here; the consumer thread drains it.
    static TX: OnceLock<std::sync::mpsc::Sender<isize>> = OnceLock::new();
    /// Cheap "rows since last prune" counter.
    static INSERTS: AtomicI64 = AtomicI64::new(0);

    const EVENT_FOREGROUND: u32 = 0x0003; // EVENT_SYSTEM_FOREGROUND
    const MAX_TITLE: usize = 200;
    const MAX_ROWS: i64 = 5_000;
    const PRUNE_EVERY: i64 = 100;

    pub fn start(app: AppHandle) {
        let (tx, rx) = std::sync::mpsc::channel::<isize>();

        // consumer: the only thread that touches SQLite for this feature
        let worker = app.clone();
        std::thread::spawn(move || {
            for hwnd in rx {
                record(&worker, hwnd);
            }
        });

        // pump thread: owns the hook and the message queue
        std::thread::spawn(move || {
            if TX.set(tx).is_err() {
                return;
            }
            unsafe {
                let _hook: HWINEVENTHOOK = SetWinEventHook(
                    EVENT_FOREGROUND,
                    EVENT_FOREGROUND,
                    HMODULE(std::ptr::null_mut()),
                    Some(win_event_proc),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                );
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        });
    }

    /// Runs on the hook's thread; only forwards. Must not panic.
    unsafe extern "system" fn win_event_proc(
        _hook: HWINEVENTHOOK,
        _event: u32,
        hwnd: HWND,
        idobject: i32,
        _idchild: i32,
        _thread: u32,
        _time: u32,
    ) {
        if hwnd.0.is_null() || idobject != OBJID_WINDOW.0 {
            return;
        }
        if let Some(tx) = TX.get() {
            let _ = tx.send(hwnd.0 as isize);
        }
    }

    #[derive(Debug, Clone)]
    struct WinInfo {
        pid: u32,
        path: String,
        title: String,
    }

    fn resolve(hwnd: isize) -> Option<WinInfo> {
        let h = HWND(hwnd as *mut std::ffi::c_void);
        unsafe {
            if !IsWindowVisible(h).as_bool() {
                return None;
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(h, Some(&mut pid as *mut u32));
            if pid == 0 {
                return None;
            }
            let path = process_path(pid).unwrap_or_default();
            Some(WinInfo { pid, path, title: window_title(h) })
        }
    }

    unsafe fn process_path(pid: u32) -> Option<String> {
        let h: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, BOOL(0), pid).ok()?;
        let mut buf = [0u16; 512];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            h,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(h);
        ok.ok()?;
        // clamp: a bad length here would abort the process (panic = "abort")
        Some(String::from_utf16_lossy(&buf[..(size as usize).min(buf.len())]))
    }

    unsafe fn window_title(h: HWND) -> String {
        let mut buf = [0u16; MAX_TITLE + 1];
        let n = GetWindowTextW(h, &mut buf);
        let n = (n as usize).min(buf.len());
        String::from_utf16_lossy(&buf[..n]).trim().chars().take(MAX_TITLE).collect()
    }

    fn record(app: &AppHandle, hwnd: isize) {
        let Some(info) = resolve(hwnd) else { return };
        if info.pid == std::process::id() {
            return; // never log Isle itself
        }

        // The window macro floats this handle, so track it even when activity
        // logging is switched off — the two features must not be coupled.
        LAST_FOREIGN.store(hwnd, Ordering::Relaxed);

        if !is_enabled() {
            return;
        }

        let app_name =
            if info.path.is_empty() { info.title.clone() } else { base_name(&info.path) };
        if app_name.is_empty() {
            return;
        }

        let t = now();
        let db = app.state::<Db>();
        let ok = db
            .with(|c| {
                c.execute("UPDATE wellbeing SET ended=?1 WHERE ended IS NULL", [t])?;
                c.execute(
                    "INSERT INTO wellbeing(app,path,title,started) VALUES(?1,?2,?3,?4)",
                    rusqlite::params![app_name, info.path, info.title, t],
                )?;
                Ok(())
            })
            .is_ok();
        if !ok {
            return;
        }

        if INSERTS.fetch_add(1, Ordering::Relaxed) % PRUNE_EVERY == 0 {
            let _ = db.with(|c| {
                c.execute(
                    "DELETE FROM wellbeing WHERE id NOT IN \
                     (SELECT id FROM wellbeing ORDER BY started DESC, id DESC LIMIT ?1)",
                    [MAX_ROWS],
                )
                .map(|_| ())
            });
        }

        let _ = app.emit("wellbeing://changed", ());
    }

    pub fn base_name(path: &str) -> String {
        path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
    }
}

/// Aggregate rows for the UI: total seconds per application in a range,
/// longest first. A still-open session counts up to `now`.
pub fn summarize(db: &Db, from: i64, to: i64) -> Result<Vec<WellbeingRow>, String> {
    db.with(|c| {
        let mut s = c.prepare(
            "SELECT app,
                    MAX(title)                           AS title,
                    SUM(COALESCE(ended, ?3) - started)   AS secs,
                    MIN(started)                         AS started,
                    MAX(ended)                           AS ended
               FROM wellbeing
              WHERE started >= ?1 AND started < ?2
              GROUP BY app
              ORDER BY secs DESC
              LIMIT 12",
        )?;
        let rows = s.query_map(rusqlite::params![from, to, now()], |r| {
            Ok(WellbeingRow {
                app: r.get(0)?,
                title: r.get(1)?,
                seconds: r.get::<_, i64>(2)?.max(0),
                started: r.get(3)?,
                ended: r.get(4)?,
            })
        })?;
        rows.collect()
    })
}

// -- Tauri commands (registered in lib.rs, mocked in src/lib/api.js) ---------

#[tauri::command]
pub fn wellbeing_enabled_get(db: tauri::State<Db>) -> bool {
    db.setting("wellbeing_enabled").map(|v| v == "true").unwrap_or(true)
}

#[tauri::command]
pub fn wellbeing_enabled_set(db: tauri::State<Db>, enabled: bool) -> Result<(), String> {
    set_enabled(enabled);
    db.set_setting("wellbeing_enabled", if enabled { "true" } else { "false" })
}

/// Midnight of the current LOCAL day, as unix seconds.
///
/// Do NOT use `NaiveDateTime::and_utc()` here: that treats local midnight as
/// if it were UTC and shifts the day by the UTC offset (on a UTC+05:30 machine
/// "today" would not start until 05:30 local, hiding the whole morning). Convert
/// through the local zone instead, taking the earliest instant so the DST
/// fall-back night still resolves.
fn local_midnight() -> Result<i64, String> {
    use chrono::TimeZone;
    let naive = chrono::Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| "could not resolve local midnight".to_string())?;
    chrono::Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|d| d.timestamp())
        .ok_or_else(|| "could not resolve local midnight".to_string())
}

/// Today, in the machine's local time, up to and including the open session.
#[tauri::command]
pub fn wellbeing_today(db: tauri::State<Db>) -> Result<Vec<WellbeingRow>, String> {
    summarize(&db, local_midnight()?, now() + 1)
}

/// An arbitrary local day range, computed by the UI (same pattern as the
/// calendar's events_for_day).
#[tauri::command]
pub fn wellbeing_range(
    db: tauri::State<Db>,
    from: i64,
    to: i64,
) -> Result<Vec<WellbeingRow>, String> {
    summarize(&db, from, to)
}

#[tauri::command]
pub fn wellbeing_clear(db: tauri::State<Db>) -> Result<(), String> {
    db.with(|c| c.execute("DELETE FROM wellbeing", []).map(|_| ()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn mem_db() -> Db {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch(crate::db::SCHEMA).unwrap();
        Db(std::sync::Mutex::new(c))
    }

    fn insert(db: &Db, app: &str, started: i64, ended: Option<i64>) {
        db.with(|c| {
            c.execute(
                "INSERT INTO wellbeing(app,path,title,started,ended) VALUES(?1,NULL,'',?2,?3)",
                rusqlite::params![app, started, ended],
            )
            .map(|_| ())
        })
        .unwrap();
    }

    #[test]
    fn groups_by_app_and_sorts_longest_first() {
        let db = mem_db();
        let t = 1_700_000_000;
        insert(&db, "Code.exe", t, Some(t + 600));
        insert(&db, "Code.exe", t + 600, Some(t + 700));
        insert(&db, "Alteryx.exe", t + 700, Some(t + 760));

        let rows = summarize(&db, t, t + 1000).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].app, "Code.exe");
        assert_eq!(rows[0].seconds, 700, "sessions of the same app must add up");
        assert_eq!(rows[1].app, "Alteryx.exe");
        assert_eq!(rows[1].seconds, 60);
    }

    #[test]
    fn open_session_counts_up_to_now() {
        let db = mem_db();
        let t = now() - 120;
        insert(&db, "X.exe", t, None);
        let rows = summarize(&db, t - 1, t + 10_000).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].seconds >= 120, "an open session must count to now, got {}", rows[0].seconds);
    }

    #[test]
    fn today_starts_at_local_midnight_not_utc() {
        use chrono::Timelike;
        let start = local_midnight().unwrap();

        // (a) Seconds elapsed since the start of "today" must match the local
        //     wall clock. This is independent of how the start was computed, and
        //     the old `.and_utc()` code was off by the UTC offset (5.5 h here).
        //     3700 s of slack covers a DST transition day.
        let elapsed = now() - start;
        let clock = chrono::Local::now().time().num_seconds_from_midnight() as i64;
        assert!(
            (elapsed - clock).abs() <= 3700,
            "local midnight is off by {} s (elapsed {elapsed}, clock {clock})",
            elapsed - clock
        );

        // (b) And it must differ from reading the naive date as UTC by exactly
        //     the local offset — that is the bug this guards against.
        let naive_utc = chrono::Local::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        let offset = chrono::Local::now().offset().local_minus_utc() as i64;
        assert_eq!(
            start - naive_utc,
            -offset,
            "local midnight must be shifted from the UTC reading by the offset"
        );
    }

    #[test]
    fn today_includes_a_session_from_a_minute_ago() {
        let db = mem_db();
        let t = now() - 60;
        insert(&db, "Code.exe", t, None);
        let rows = summarize(&db, local_midnight().unwrap(), now() + 1).unwrap();
        assert_eq!(rows.len(), 1, "a session from a minute ago must count as today");
    }

    #[test]
    fn ranges_are_half_open() {
        let db = mem_db();
        let t = 1_700_000_000;
        insert(&db, "A.exe", t, Some(t + 1));
        assert_eq!(summarize(&db, t, t + 1).unwrap().len(), 1);
        assert!(summarize(&db, t + 1, t + 2).unwrap().is_empty(), "started == to must be excluded");
    }

    #[cfg(windows)]
    #[test]
    fn base_name_handles_both_separators() {
        assert_eq!(imp::base_name("C:\\Program Files\\Code\\Code.exe"), "Code.exe");
        assert_eq!(imp::base_name("Code.exe"), "Code.exe");
    }
}
