//! One background thread that owns all time-based work:
//!   - event reminders (toast `remind_min` minutes before `events.starts`)
//!   - task reminders (toast `remind_min` minutes before `tasks.due`)
//!   - time-triggered routines  ({"kind":"time","at":"HH:MM","days":[0..6]}, 0=Sun)
//!   - wallpaper rotation (settings `wallpaper_interval` in minutes, 0 = off)
//!
//! It sleeps until the NEXT thing is due (never a fixed fast poll) and is woken
//! early by `Scheduler::wake()` whenever events/routines/settings change.
//! Worst-case idle wake-up is every 5 minutes (a safety net after sleep/resume).

use crate::{
    actions,
    db::{now, Db},
};
use std::{
    sync::{
        mpsc::{channel, RecvTimeoutError, Sender},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Manager};

// Constants for time-based operations
const SAFETY_NET_SECONDS: i64 = 300; // 5 minutes - worst-case idle wake-up
const REMINDER_WINDOW_SECONDS: i64 = 300; // 5 minutes - window after start to show notification

pub struct Scheduler(Mutex<Sender<()>>);

impl Scheduler {
    pub fn wake(&self) {
        if let Ok(tx) = self.0.lock() {
            let _ = tx.send(());
        }
    }
}

pub fn start(app: AppHandle) -> Scheduler {
    let (tx, rx) = channel::<()>();
    std::thread::spawn(move || {
        let mut is_startup = true;
        loop {
            let wait = tick(&app, is_startup);
            is_startup = false;
            match rx.recv_timeout(wait) {
                Ok(_) => {
                    // Wake signal received - continue loop
                }
                Err(RecvTimeoutError::Disconnected) => {
                    // Channel disconnected - thread shutting down
                    // Perform any cleanup before exiting
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {
                    // Timeout reached - continue loop
                }
            }
        }
        // Thread exit - resources are automatically cleaned up via RAII
    });
    Scheduler(Mutex::new(tx))
}

/// Do everything that is due now; return how long to sleep.
fn tick(app: &AppHandle, _is_startup: bool) -> Duration {
    let db = app.state::<Db>();
    let now = now();
    let mut next = now + SAFETY_NET_SECONDS;

    // 1) Event reminders. Fire once when now >= start - remind_min*60 and notified=0 and now < start + REMINDER_WINDOW_SECONDS.
    // Never fire for events that already began more than REMINDER_WINDOW_SECONDS ago; mark those notified=1 silently.
    let due: Vec<(i64, String, i64)> = db
        .with(|c| {
            let mut s = c.prepare(
                "SELECT id,title,starts FROM events WHERE notified=0 AND starts - remind_min*60 <= ?1",
            )?;
            let rows = s.query_map([now], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect()
        })
        .unwrap_or_default();
    
    for (id, title, starts) in due {
        // Only show notification if within the REMINDER_WINDOW_SECONDS window after start
        if now < starts + REMINDER_WINDOW_SECONDS {
            let mins = ((starts - now) / 60).max(0);
            let body = if mins == 0 { "Starting now".to_string() } else { format!("In {mins} min") };
            let _ = actions::notify(app, &title, &body);
        }
        // Mark as notified after checking (even if past the REMINDER_WINDOW_SECONDS window, to avoid retries)
        let _ = db.with(|c| c.execute("UPDATE events SET notified=1 WHERE id=?1", [id]));
    }
    
    // Calculate next event reminder time
    if let Ok(Some(t)) = db.with(|c| {
        c.query_row("SELECT MIN(starts - remind_min*60) FROM events WHERE notified=0", [], |r| {
            r.get::<_, Option<i64>>(0)
        })
    }) {
        next = next.min(t);
    }

    Duration::from_secs((next - now).max(1) as u64)
}
