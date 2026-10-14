//! One background thread that owns all time-based work:
//!   - event reminders (toast `remind_min` minutes before `events.starts`)
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
use chrono::{Datelike, Local};
use serde_json::Value;
use std::{
    sync::{
        mpsc::{channel, RecvTimeoutError, Sender},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Manager};

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
    std::thread::spawn(move || loop {
        let wait = tick(&app);
        if let Err(RecvTimeoutError::Disconnected) = rx.recv_timeout(wait) {
            break;
        }
    });
    Scheduler(Mutex::new(tx))
}

/// Do everything that is due now; return how long to sleep.
fn tick(app: &AppHandle) -> Duration {
    let db = app.state::<Db>();
    let now = now();
    let mut next = now + 300;

    // 1) Event reminders. Skip stale ones (more than 5 min past start) silently.
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
        let _ = db.with(|c| c.execute("UPDATE events SET notified=1 WHERE id=?1", [id]));
        if starts + 300 >= now {
            let mins = ((starts - now) / 60).max(0);
            let body = if mins == 0 { "Starting now".to_string() } else { format!("In {mins} min") };
            let _ = actions::notify(app, &title, &body);
        }
    }
    if let Ok(Some(t)) = db.with(|c| {
        c.query_row("SELECT MIN(starts - remind_min*60) FROM events WHERE notified=0", [], |r| {
            r.get::<_, Option<i64>>(0)
        })
    }) {
        next = next.min(t);
    }

    // 2) Time-triggered routines (checked once per minute, only if any exist).
    let routines: Vec<(i64, String, i64, Value, Value)> = db
        .with(|c| {
            let mut s = c.prepare("SELECT id,name,last_run,trig,actions FROM routines WHERE enabled=1")?;
            let rows = s.query_map([], |r| {
                let (t, a): (String, String) = (r.get(3)?, r.get(4)?);
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    serde_json::from_str(&t).unwrap_or(Value::Null),
                    serde_json::from_str(&a).unwrap_or(Value::Null),
                ))
            })?;
            rows.collect()
        })
        .unwrap_or_default();
    let local = Local::now();
    let (hhmm, dow) = (local.format("%H:%M").to_string(), local.weekday().num_days_from_sunday() as i64);
    let mut has_time_routine = false;
    for (id, _name, last_run, trig, acts) in routines {
        if trig["kind"] != "time" {
            continue;
        }
        has_time_routine = true;
        let days_ok = trig["days"]
            .as_array()
            .map(|d| d.is_empty() || d.iter().any(|x| x.as_i64() == Some(dow)))
            .unwrap_or(true);
        if trig["at"].as_str() == Some(hhmm.as_str()) && days_ok && last_run / 60 != now / 60 {
            let _ = db.with(|c| c.execute("UPDATE routines SET last_run=?1 WHERE id=?2", [now, id]));
            let _ = actions::run(app, &acts);
        }
    }
    if has_time_routine {
        next = next.min(now + 60 - now % 60);
    }

    // 3) Wallpaper rotation.
    let interval = db.setting("wallpaper_interval").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    if interval > 0 {
        let last = db.setting("wallpaper_last").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
        let mut due_at = last + interval * 60;
        if now >= due_at {
            if actions::wallpaper_next(app).is_err() {
                // e.g. no folder set: back off instead of retrying every tick.
                let _ = db.set_setting("wallpaper_last", &now.to_string());
            }
            due_at = now + interval * 60;
        }
        next = next.min(due_at);
    }

    Duration::from_secs((next - now).max(1) as u64)
}
