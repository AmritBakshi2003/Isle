//! Now-playing + transport controls via Windows' System Media Transport
//! Controls (GSMTC). Works with Spotify, browsers (YouTube etc.), VLC, and any
//! app that registers with Windows media overlay. No API keys, no polling.
//!
//! EVENT-DRIVEN: we subscribe to "current session changed" on the manager and
//! "media properties / playback info changed" on the active session. We do NOT
//! subscribe to timeline changes (some players fire them every second).
//! Instead we send `position_ms` + `sampled_at_ms` and the UI extrapolates.
//!
//! UI contract: event `media://update` (payload MediaState) and commands
//! `media_get`, `media_control(cmd)` where cmd in {play_pause, next, prev}.
//!
//! KNOWN LIMITS: handlers capture the session (small leak per session change);
//! the album-art data URL can be ~1MB; no seek support yet.

use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize, Clone, Default)]
pub struct MediaState {
    pub active: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    /// unix ms when `position_ms` was valid. UI adds (now - sampled_at) while playing.
    pub sampled_at_ms: i64,
    /// data: URL of the album art, if the player supplies one.
    pub art: Option<String>,
}

#[tauri::command]
pub async fn media_get() -> MediaState {
    imp::current()
}

#[tauri::command]
pub async fn media_control(cmd: String) -> Result<(), String> {
    imp::control(&cmd)
}

pub fn start(app: AppHandle) {
    imp::start(app)
}

pub fn control(cmd: &str) -> Result<(), String> {
    imp::control(cmd)
}

#[cfg(windows)]
mod imp {
    use super::MediaState;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::sync::Mutex;
    use tauri::{AppHandle, Emitter};
    use windows::{
        Foundation::TypedEventHandler,
        Media::Control::*,
        Storage::Streams::DataReader,
        Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
    };

    type Session = GlobalSystemMediaTransportControlsSession;
    type Manager = GlobalSystemMediaTransportControlsSessionManager;

    struct Sess(Session);
    // WinRT GSMTC objects are agile (free-threaded); wrapping makes that explicit.
    unsafe impl Send for Sess {}

    static SESSION: Mutex<Option<Sess>> = Mutex::new(None);
    /// (cache key, art) so we only decode the thumbnail when the track changes.
    static ART: Mutex<(String, Option<String>)> = Mutex::new((String::new(), None));

    pub fn start(app: AppHandle) {
        std::thread::spawn(move || {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            if let Err(e) = run(app) {
                eprintln!("[isle] media listener stopped: {e}");
            }
        });
    }

    fn run(app: AppHandle) -> windows::core::Result<()> {
        let mgr = Manager::RequestAsync()?.get()?;
        attach(&app, &mgr);
        let (a, m) = (app.clone(), mgr.clone());
        mgr.CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
            attach(&a, &m);
            Ok(())
        }))?;
        // Keep `mgr` alive; events arrive on the WinRT thread pool.
        loop {
            std::thread::park();
        }
    }

    macro_rules! hook {
        ($s:expr, $app:expr, $ev:ident) => {{
            let (a, s) = ($app.clone(), $s.clone());
            let _ = $s.$ev(&TypedEventHandler::new(move |_, _| {
                publish(&a, Some(&s));
                Ok(())
            }));
        }};
    }

    fn attach(app: &AppHandle, mgr: &Manager) {
        let session = mgr.GetCurrentSession().ok();
        if let Some(s) = &session {
            hook!(s, app, MediaPropertiesChanged);
            hook!(s, app, PlaybackInfoChanged);
        }
        *SESSION.lock().unwrap() = session.clone().map(Sess);
        publish(app, session.as_ref());
    }

    fn publish(app: &AppHandle, s: Option<&Session>) {
        let st = s.and_then(|s| read(s).ok()).unwrap_or_default();
        let _ = app.emit("media://update", st);
    }

    pub fn current() -> MediaState {
        let g = SESSION.lock().unwrap();
        g.as_ref().and_then(|s| read(&s.0).ok()).unwrap_or_default()
    }

    fn unix_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    fn read(s: &Session) -> windows::core::Result<MediaState> {
        let p = s.TryGetMediaPropertiesAsync()?.get()?;
        let (title, artist, album) = (
            p.Title()?.to_string(),
            p.Artist()?.to_string(),
            p.AlbumTitle()?.to_string(),
        );
        let playing = s.GetPlaybackInfo()?.PlaybackStatus()?
            == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
        let tl = s.GetTimelineProperties()?;
        let position_ms = tl.Position()?.Duration / 10_000; // 100ns ticks -> ms
        let duration_ms = (tl.EndTime()?.Duration - tl.StartTime()?.Duration) / 10_000;
        // LastUpdatedTime is a FILETIME-like value: 100ns since 1601-01-01.
        let mut sampled = tl.LastUpdatedTime()?.UniversalTime / 10_000 - 11_644_473_600_000;
        let now = unix_ms();
        if sampled <= 0 || sampled > now {
            sampled = now;
        }

        let key = format!("{title}\u{1}{artist}\u{1}{album}");
        let art = {
            let mut c = ART.lock().unwrap();
            if c.0 != key || c.1.is_none() {
                c.1 = thumbnail(&p).ok().flatten();
                c.0 = key;
            }
            c.1.clone()
        };
        Ok(MediaState {
            active: !title.is_empty() || !artist.is_empty(),
            title,
            artist,
            album,
            playing,
            position_ms,
            duration_ms,
            sampled_at_ms: sampled,
            art,
        })
    }

    fn thumbnail(p: &GlobalSystemMediaTransportControlsSessionMediaProperties) -> windows::core::Result<Option<String>> {
        let Ok(r) = p.Thumbnail() else { return Ok(None) };
        let stream = r.OpenReadAsync()?.get()?;
        let size = stream.Size()? as u32;
        if size == 0 || size > 1_500_000 {
            return Ok(None);
        }
        let reader = DataReader::CreateDataReader(&stream)?;
        reader.LoadAsync(size)?.get()?;
        let mut buf = vec![0u8; size as usize];
        reader.ReadBytes(&mut buf)?;
        let mime = stream.ContentType()?.to_string();
        let mime = if mime.is_empty() { "image/jpeg".to_string() } else { mime };
        Ok(Some(format!("data:{mime};base64,{}", STANDARD.encode(buf))))
    }

    pub fn control(cmd: &str) -> Result<(), String> {
        let g = SESSION.lock().map_err(|e| e.to_string())?;
        let Some(Sess(s)) = g.as_ref() else {
            return Err("Nothing is playing".into());
        };
        let op = match cmd {
            "play_pause" => s.TryTogglePlayPauseAsync(),
            "next" => s.TrySkipNextAsync(),
            "prev" => s.TrySkipPreviousAsync(),
            other => return Err(format!("unknown media command: {other}")),
        };
        op.map(|_| ()).map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::MediaState;
    use tauri::AppHandle;
    pub fn start(_: AppHandle) {}
    pub fn current() -> MediaState {
        MediaState::default()
    }
    pub fn control(_: &str) -> Result<(), String> {
        Err("Media controls need Windows".into())
    }
}
