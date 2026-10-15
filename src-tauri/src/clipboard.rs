//! Clipboard history (text only for now).
//!
//! EVENT-DRIVEN: `clipboard-master` blocks on the Win32 clipboard-format
//! listener, so this thread uses ~0 CPU until the clipboard actually changes.
//!
//! PRIVACY: copies flagged by password managers with the
//! "ExcludeClipboardContentFromMonitorProcessing" format are skipped.
//! Entries are capped at 20k chars; only the newest 300 unpinned items are kept.
//!
//! UI contract: event `clipboard://changed` (no payload); commands in commands.rs.
//! TODO(agents): image history (store small PNG thumbnails + hash), file lists.

use tauri::AppHandle;

pub fn start(app: AppHandle) {
    imp::start(app)
}

pub fn stop() {
    imp::stop()
}

pub fn set_enabled(enabled: bool) {
    imp::set_enabled(enabled)
}

pub fn is_enabled() -> bool {
    imp::is_enabled()
}

#[cfg(windows)]
mod imp {
    use crate::db::{now, Db};
    use clipboard_master::{CallbackResult, ClipboardHandler, Master};
    use tauri::{AppHandle, Emitter, Manager};
    use windows::{
        core::w,
        Win32::System::DataExchange::{IsClipboardFormatAvailable, RegisterClipboardFormatW},
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    static CLIPBOARD_ENABLED: AtomicBool = AtomicBool::new(true);

    struct Handler {
        app: AppHandle,
    }

    impl ClipboardHandler for Handler {
        fn on_clipboard_change(&mut self) -> CallbackResult {
            if CLIPBOARD_ENABLED.load(Ordering::Relaxed) {
                capture(&self.app);
            }
            CallbackResult::Next
        }
        fn on_clipboard_error(&mut self, _e: std::io::Error) -> CallbackResult {
            CallbackResult::Next
        }
    }

    pub fn start(app: AppHandle) {
        std::thread::spawn(move || {
            let _ = Master::new(Handler { app }).run();
        });
    }

    pub fn stop() {
        CLIPBOARD_ENABLED.store(false, Ordering::Relaxed);
    }

    pub fn is_enabled() -> bool {
        CLIPBOARD_ENABLED.load(Ordering::Relaxed)
    }

    pub fn set_enabled(enabled: bool) {
        CLIPBOARD_ENABLED.store(enabled, Ordering::Relaxed);
    }

    fn excluded() -> bool {
        unsafe {
            let f = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
            f != 0 && IsClipboardFormatAvailable(f).is_ok()
        }
    }

    fn read_text() -> Option<String> {
        // The clipboard can be briefly locked by the app that just wrote to it.
        for _ in 0..3 {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                if let Ok(t) = cb.get_text() {
                    return Some(t);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        None
    }

    fn capture(app: &AppHandle) {
        if excluded() {
            return;
        }
        let Some(text) = read_text() else { return };
        let t = text.trim_end();
        if t.trim().is_empty() || t.len() > 20_000 {
            return;
        }
        let db = app.state::<Db>();
        let _ = db.with(|c| {
            c.execute(
                "INSERT INTO clips(text,pinned,ts) VALUES(?1,0,?2)
                 ON CONFLICT(text) DO UPDATE SET ts=excluded.ts",
                rusqlite::params![t, now()],
            )?;
            c.execute(
                "DELETE FROM clips WHERE pinned=0 AND id NOT IN
                 (SELECT id FROM clips WHERE pinned=0 ORDER BY ts DESC LIMIT 300)",
                [],
            )?;
            Ok(())
        });
        let _ = app.emit("clipboard://changed", ());
    }
}

#[cfg(not(windows))]
mod imp {
    use tauri::AppHandle;
    pub fn start(_: AppHandle) {}
}
