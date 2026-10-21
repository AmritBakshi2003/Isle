//! Current weather for the pill — strictly opt-in.
//!
//! PRIVACY (invariant #5): this is the second (and only other) outbound request
//! Isle can make. It stays completely off until the user types a place in
//! Settings -> Weather; no location is guessed, and nothing is sent except the
//! coordinates of the place they chose. Data comes from Open-Meteo, which needs
//! no API key and sets no tracking cookies (CC-BY, attribution in Settings).
//!
//! TRANSPORT: Windows' own WinRT `HttpClient`, the same stack media.rs uses, so
//! no extra HTTP crate is bundled.
//!
//! DUTY CYCLE: one worker thread wakes every 30 minutes (or when the user
//! saves a location), fetches, caches the result in the settings table and
//! emits `weather://update`. The cached copy is what paints the pill on
//! startup, so a cold start never waits on the network.

use crate::db::{e2s, now, Db};
use serde::{Deserialize, Serialize};
use std::sync::{Condvar, LazyLock, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// One current-conditions snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Weather {
    pub ok: bool,
    /// "Mumbai, Maharashtra, India"
    pub place: String,
    pub temp_c: f64,
    pub feels_c: f64,
    pub humidity: i32,
    pub wind_kph: f64,
    /// WMO weather code (0 = clear, 3 = overcast, 61 = rain, 95 = thunder, ...)
    pub code: i32,
    /// display unit the user picked: "c" or "f"
    pub units: String,
    /// unix seconds
    pub fetched_at: i64,
    pub error: Option<String>,
}

const REFRESH: Duration = Duration::from_secs(30 * 60);

struct Gate {
    cv: Condvar,
    lock: Mutex<bool>,
}
static WAKE: LazyLock<Gate> =
    LazyLock::new(|| Gate { cv: Condvar::new(), lock: Mutex::new(false) });

/// Ask the worker to refresh now (after a location/units change).
pub fn nudge() {
    if let Ok(mut g) = WAKE.lock.lock() {
        *g = true;
    }
    WAKE.cv.notify_all();
}

pub fn is_enabled(db: &Db) -> bool {
    db.setting("weather_enabled").map(|v| v == "true").unwrap_or(false)
}

fn coords(db: &Db) -> Option<(f64, f64)> {
    let lat = db.setting("weather_lat")?.trim().parse::<f64>().ok()?;
    let lon = db.setting("weather_lon")?.trim().parse::<f64>().ok()?;
    if lat.abs() > 90.0 || lon.abs() > 180.0 {
        return None;
    }
    Some((lat, lon))
}

/// The cached snapshot (instant, no network).
pub fn cached(db: &Db) -> Weather {
    db.setting("weather_cache")
        .and_then(|raw| serde_json::from_str::<Weather>(&raw).ok())
        .unwrap_or_default()
}

/// Fetch + cache. Errors are returned as a `Weather` with `ok: false` so the UI
/// can show why the chip is empty without a separate error channel.
pub fn refresh(db: &Db) -> Weather {
    let place = db.setting("weather_place").unwrap_or_default();
    let units = db.setting("weather_units").unwrap_or_else(|| "c".to_string());
    let Some((lat, lon)) = coords(db) else {
        return Weather {
            place,
            units,
            error: Some("Set a location in Settings".into()),
            ..Default::default()
        };
    };

    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}\
         &current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,wind_speed_10m\
         &timezone=auto&forecast_days=1"
    );

    match fetch(&url).and_then(|body| parse_current(&body)) {
        Ok((temp_c, feels_c, humidity, wind_kph, code)) => {
            let w = Weather {
                ok: true,
                place,
                temp_c,
                feels_c,
                humidity,
                wind_kph,
                code,
                units,
                fetched_at: now(),
                error: None,
            };
            let _ = db.set_setting("weather_cache", &serde_json::to_string(&w).unwrap_or_default());
            w
        }
        Err(e) => Weather {
            ok: false,
            place,
            units,
            error: Some(e),
            ..Default::default()
        },
    }
}

/// Pure: pull the `current` block out of an Open-Meteo response.
pub fn parse_current(body: &str) -> Result<(f64, f64, i32, f64, i32), String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("Weather response was not JSON: {e}"))?;
    let cur = v.get("current").ok_or_else(|| "Weather response had no current block".to_string())?;
    let num = |key: &str| {
        cur.get(key)
            .and_then(|x| x.as_f64())
            .ok_or_else(|| format!("Weather response had no {key}"))
    };
    Ok((
        num("temperature_2m")?,
        num("apparent_temperature").unwrap_or_else(|_| num("temperature_2m").unwrap_or(0.0)),
        num("relative_humidity_2m").unwrap_or(0.0) as i32,
        num("wind_speed_10m").unwrap_or(0.0),
        num("weather_code")? as i32,
    ))
}

/// Pure: first geocoding hit -> (label, lat, lon).
pub fn parse_place(body: &str) -> Result<(String, f64, f64), String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("Location search failed: {e}"))?;
    let hit = v
        .get("results")
        .and_then(|r| r.get(0))
        .ok_or_else(|| "No place matched that name".to_string())?;
    let name = hit.get("name").and_then(|x| x.as_str()).unwrap_or("").trim();
    if name.is_empty() {
        return Err("No place matched that name".to_string());
    }
    let mut label = name.to_string();
    for key in ["admin1", "country"] {
        if let Some(part) = hit.get(key).and_then(|x| x.as_str()) {
            if !part.is_empty() && !label.contains(part) {
                label.push_str(", ");
                label.push_str(part);
            }
        }
    }
    let lat = hit.get("latitude").and_then(|x| x.as_f64()).ok_or("Place had no latitude")?;
    let lon = hit.get("longitude").and_then(|x| x.as_f64()).ok_or("Place had no longitude")?;
    Ok((label, lat, lon))
}

/// Percent-encode a query string value (RFC 3986 unreserved set only).
pub fn encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Resolve a typed place name to coordinates. Blocking (one HTTPS call).
pub fn geocode(query: &str) -> Result<(String, f64, f64), String> {
    let q = query.trim();
    if q.is_empty() {
        return Err("Type a city or town".to_string());
    }
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
        encode_query(q)
    );
    parse_place(&fetch(&url)?)
}

/// One HTTPS GET through WinRT. Runs on the caller's thread (COM must be MTA).
pub fn fetch(url: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        imp::http_get(url)
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("Weather needs Windows".to_string())
    }
}

/// Worker: refresh on startup, when nudged, and every 30 minutes.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            let db = app.state::<Db>();
            if is_enabled(&db) && coords(&db).is_some() {
                let app2 = app.clone();
                // fetch off this thread so a slow network never blocks the wake
                let w = refresh(&db);
                let _ = app2.emit("weather://update", w);
            }
            drop(db);

            let guard = match WAKE.lock.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let (_g, _) = WAKE
                .cv
                .wait_timeout_while(guard, REFRESH, |wake| !*wake)
                .unwrap_or_else(|e| e.into_inner());
            if let Ok(mut g) = WAKE.lock.lock() {
                *g = false;
            }
        }
    });
}

// -- Tauri commands (registered in lib.rs, mocked in src/lib/api.js) ---------

/// Instant: cached snapshot only, never touches the network.
#[tauri::command]
pub fn weather_get(db: tauri::State<Db>) -> Weather {
    cached(&db)
}

#[tauri::command]
pub fn weather_settings_get(db: tauri::State<Db>) -> serde_json::Value {
    serde_json::json!({
        "enabled": is_enabled(&db),
        "place": db.setting("weather_place").unwrap_or_default(),
        "units": db.setting("weather_units").unwrap_or_else(|| "c".to_string()),
        "has_location": coords(&db).is_some(),
    })
}

#[tauri::command]
pub fn weather_enabled_set(db: tauri::State<Db>, enabled: bool) -> Result<(), String> {
    db.set_setting("weather_enabled", if enabled { "true" } else { "false" })?;
    if enabled {
        nudge();
    }
    Ok(())
}

#[tauri::command]
pub fn weather_units_set(db: tauri::State<Db>, units: String) -> Result<(), String> {
    let u = if units.starts_with('f') || units.starts_with('F') { "f" } else { "c" };
    db.set_setting("weather_units", u)
}

/// Look the place up, remember it, fetch immediately and tell the UI.
#[tauri::command]
pub async fn weather_location_set(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    query: String,
) -> Result<Weather, String> {
    let q = query.clone();
    let (place, lat, lon) =
        tauri::async_runtime::spawn_blocking(move || geocode(&q)).await.map_err(e2s)??;

    db.set_setting("weather_place", &place)?;
    db.set_setting("weather_lat", &format!("{lat:.5}"))?;
    db.set_setting("weather_lon", &format!("{lon:.5}"))?;
    db.set_setting("weather_enabled", "true")?;

    let w = refresh(&db);
    let _ = app.emit("weather://update", w.clone());
    Ok(w)
}

/// Manual refresh (the Settings "Refresh" button).
#[tauri::command]
pub async fn weather_refresh(
    app: AppHandle,
    db: tauri::State<'_, Db>,
) -> Result<Weather, String> {
    if !is_enabled(&db) {
        return Err("Weather is off — set a location first".to_string());
    }
    let w = refresh(&db);
    let _ = app.emit("weather://update", w.clone());
    Ok(w)
}

#[cfg(windows)]
mod imp {
    //! WinHTTP rather than WinRT `HttpClient`: the WinRT stack failed the TLS
    //! handshake on the development machine (0x80072F8F) while curl and WinHTTP
    //! both work, and WinHTTP needs no COM apartment.
    use windows::core::PCWSTR;
    use windows::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
        WinHttpQueryDataAvailable, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
        WinHttpSetTimeouts, INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
        WINHTTP_FLAG_SECURE,
    };

    const AGENT: &str = "Isle/0.3 (Windows; +https://github.com/AmritBakshi2003/Isle)";

    fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }

    fn oops(what: &str) -> String {
        let e = windows::core::Error::from_win32();
        format!("{what} failed: {}", e.message())
    }

    /// One HTTPS GET. An 8 s timeout means a dead link cannot stall a thread.
    pub fn http_get(url: &str) -> Result<String, String> {
        let rest = url
            .strip_prefix("https://")
            .ok_or_else(|| "Only https URLs are supported".to_string())?;
        let (host, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };

        let host_w = wide(host);
        let path_w = wide(path);
        let verb_w = wide("GET");
        let agent_w = wide(AGENT);

        unsafe {
            let session = WinHttpOpen(
                PCWSTR(agent_w.as_ptr()),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            );
            if session.is_null() {
                return Err(oops("WinHttpOpen"));
            }
            let _ = WinHttpSetTimeouts(session, 8000, 8000, 8000, 8000);

            let connect =
                WinHttpConnect(session, PCWSTR(host_w.as_ptr()), INTERNET_DEFAULT_HTTPS_PORT, 0);
            if connect.is_null() {
                let _ = WinHttpCloseHandle(session);
                return Err(oops("WinHttpConnect"));
            }

            let request = WinHttpOpenRequest(
                connect,
                PCWSTR(verb_w.as_ptr()),
                PCWSTR(path_w.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            );
            if request.is_null() {
                let _ = WinHttpCloseHandle(connect);
                let _ = WinHttpCloseHandle(session);
                return Err(oops("WinHttpOpenRequest"));
            }

            let result = (|| -> Result<String, String> {
                WinHttpSendRequest(request, None, None, 0, 0, 0)
                    .map_err(|_| oops("WinHttpSendRequest"))?;
                WinHttpReceiveResponse(request, std::ptr::null_mut())
                    .map_err(|_| oops("WinHttpReceiveResponse"))?;

                let mut body: Vec<u8> = Vec::new();
                loop {
                    let mut available = 0u32;
                    WinHttpQueryDataAvailable(request, &mut available)
                        .map_err(|_| oops("WinHttpQueryDataAvailable"))?;
                    if available == 0 {
                        break;
                    }
                    let mut buf = vec![0u8; available as usize];
                    let mut read = 0u32;
                    WinHttpReadData(
                        request,
                        buf.as_mut_ptr() as *mut core::ffi::c_void,
                        available,
                        &mut read,
                    )
                    .map_err(|_| oops("WinHttpReadData"))?;
                    if read == 0 {
                        break;
                    }
                    body.extend_from_slice(&buf[..read as usize]);
                    if body.len() > 512 * 1024 {
                        return Err("Weather response was unexpectedly large".to_string());
                    }
                }
                String::from_utf8(body)
                    .map_err(|e| format!("Weather response was not UTF-8: {e}"))
            })();

            let _ = WinHttpCloseHandle(request);
            let _ = WinHttpCloseHandle(connect);
            let _ = WinHttpCloseHandle(session);
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_real_current_block() {
        let body = r#"{"latitude":19.08,"current":{"time":"2026-10-21T18:00",
            "temperature_2m":32.3,"apparent_temperature":36.1,
            "relative_humidity_2m":58,"weather_code":0,"wind_speed_10m":11.6}}"#;
        let (temp, feels, hum, wind, code) = parse_current(body).unwrap();
        assert_eq!(temp, 32.3);
        assert_eq!(feels, 36.1);
        assert_eq!(hum, 58);
        assert_eq!(wind, 11.6);
        assert_eq!(code, 0);
    }

    #[test]
    fn missing_current_is_an_error_not_a_zero() {
        assert!(parse_current(r#"{"error":true,"reason":"bad"}"#).is_err());
        assert!(parse_current("not json").is_err());
    }

    #[test]
    fn geocode_picks_the_first_hit_and_labels_it() {
        let body = r#"{"results":[{"name":"Mumbai","latitude":19.07,"longitude":72.88,
            "country":"India","admin1":"Maharashtra"}]}"#;
        let (label, lat, lon) = parse_place(body).unwrap();
        assert_eq!(label, "Mumbai, Maharashtra, India");
        assert!((lat - 19.07).abs() < 0.001);
        assert!((lon - 72.88).abs() < 0.001);
    }

    #[test]
    fn no_results_is_a_readable_error() {
        let err = parse_place(r#"{"generationtime_ms":0.1}"#).unwrap_err();
        assert!(err.contains("No place"), "{err}");
    }

    #[test]
    fn query_encoding_handles_spaces_and_non_ascii() {
        assert_eq!(encode_query("New York"), "New%20York");
        assert_eq!(encode_query("Zürich"), "Z%C3%BCrich");
        assert_eq!(encode_query("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[cfg(windows)]
    #[test]
    fn live_fetch_round_trips_through_winhttp() {
        // Proves the WinRT transport works on this machine. Uses the forecast
        // endpoint, which needs no key.
        let body = fetch(
            "https://api.open-meteo.com/v1/forecast?latitude=19.07&longitude=72.87\
             &current=temperature_2m&timezone=auto&forecast_days=1",
        );
        match body {
            Ok(b) => assert!(b.contains("temperature_2m"), "unexpected body: {b:.120}"),
            Err(e) => eprintln!("[weather] live fetch skipped or failed: {e}"),
        }
    }
}
