//! Live network throughput and latency for the Workspace dashboard.
//!
//! DESIGN (mirrors progress.rs): one sampler thread is the only producer. It
//! reads the kernel's interface table with `GetIfTable2` — Windows already
//! maintains the octet counters at the hardware level, so a read costs a few
//! microseconds — and emits one sample per second.
//!
//! DUTY CYCLE (AGENTS.md invariant #1, "no ungated polling"): the thread parks
//! on a condvar until the UI calls `net_sampler_set(true)`. Only the mounted
//! Workspace tab does that, so with the island collapsed this thread sleeps and
//! burns nothing at all. `wait_timeout_while` lets a disable wake it early.
//!
//! PRIVACY (invariant #5, "local only"): reading the interface table is purely
//! local. The optional latency probe is the ONLY packet Isle can ever send: it
//! defaults to the default gateway (LAN traffic) and a public target requires
//! the explicit `net_ping_public` opt-in.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Condvar, LazyLock, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// One sample, exactly as the UI receives it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NetSample {
    /// bytes/sec received across all eligible interfaces
    pub down_bps: u64,
    /// bytes/sec sent
    pub up_bps: u64,
    /// negotiated receive link speed (bits/sec), for context
    pub link_down_bps: u64,
    /// negotiated transmit link speed (bits/sec)
    pub link_up_bps: u64,
    /// friendly name of the busiest interface (e.g. "Wi-Fi")
    pub iface: String,
    /// round-trip time; None = no probe ran or it failed
    pub latency_ms: Option<u32>,
    /// unix seconds
    pub ts: i64,
}

/// A plain, windows-free row so the maths below is unit-testable anywhere.
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub index: u32,
    pub name: String,
    /// NET_IFTYPE (6 = Ethernet, 71 = 802.11, 24 = software loopback, 131 = PPP)
    pub kind: u32,
    /// NET_IF_OPER_STATUS (1 = up)
    pub oper: u32,
    pub link_down_bps: u64,
    pub link_up_bps: u64,
    pub in_octets: u64,
    pub out_octets: u64,
    /// 100ns time base for the delta (windows 0.58 projects no per-row
    /// Timestamp, so the sampler supplies a monotonic sample clock)
    pub ticks: u64,
}

// raw values, so nothing here depends on windows-rs naming
pub const IF_OPER_STATUS_UP: u32 = 1;
pub const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
pub const IF_TYPE_PPP: u32 = 131;

// -- duty-cycle gate ----------------------------------------------------------

struct Gate {
    on: Mutex<bool>,
    cv: Condvar,
}

static GATE: LazyLock<Gate> = LazyLock::new(|| Gate { on: Mutex::new(false), cv: Condvar::new() });
static LAST: LazyLock<Mutex<Option<NetSample>>> = LazyLock::new(|| Mutex::new(None));

/// Called by the Workspace tab on mount/unmount. True = sample at 1 Hz.
pub fn set_enabled(on: bool) {
    if let Ok(mut g) = GATE.on.lock() {
        *g = on;
    }
    GATE.cv.notify_all();
}

/// Spawn the sampler thread. It parks immediately and stays parked until the
/// Workspace tab asks for samples.
pub fn start(app: AppHandle) {
    #[cfg(windows)]
    std::thread::spawn(move || sampler_loop(app));
    #[cfg(not(windows))]
    let _ = app;
}

/// Last known sample, so the tab paints instantly on open instead of waiting
/// a full second for the first tick.
pub fn snapshot() -> Option<NetSample> {
    LAST.lock().ok().and_then(|l| l.clone())
}

// -- sampler loop -------------------------------------------------------------

/// True while the island window is actually on screen. Gating on this keeps a
/// hidden-but-expanded panel from sampling (and pinging) in the background.
#[cfg(windows)]
pub(crate) fn window_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

#[cfg(windows)]
fn sampler_loop(app: AppHandle) {
    let mut prev: HashMap<u32, (u64, u64, u64)> = HashMap::new();
    let mut priming = true;
    loop {
        // Park until the UI asks for samples.
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

        // Hidden island: the window is not on screen (system hide, tray hide),
        // so do not sample and do not ping. The counters simply age; when the
        // panel comes back the first delta is the average rate over the gap,
        // because the rate is computed from our own clock, not a fixed 1 s.
        if !window_visible(&app) {
            let guard = match GATE.on.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let (_g, _t) = GATE
                .cv
                .wait_timeout_while(guard, Duration::from_secs(1), |on| *on)
                .unwrap_or_else(|e| e.into_inner());
            continue;
        }

        tick(&app, &mut prev, &mut priming);

        // Sleep ~1s, but return instantly if the tab unmounts meanwhile.
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

#[cfg(windows)]
fn tick(app: &AppHandle, prev: &mut HashMap<u32, (u64, u64, u64)>, priming: &mut bool) {
    let rows = match read_rows(now_100ns()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[isle] net sample failed: {e}");
            return;
        }
    };
    let (down, up, link_down, link_up, iface) = aggregate(&rows, prev, *priming);
    *priming = false;

    let sample = NetSample {
        down_bps: down,
        up_bps: up,
        link_down_bps: link_down,
        link_up_bps: link_up,
        iface,
        latency_ms: latency_for(app),
        ts: crate::db::now(),
    };

    if let Ok(mut l) = LAST.lock() {
        *l = Some(sample.clone());
    }
    let _ = app.emit("workspace://network", sample);
}

/// Read the settings, decide what to ping, ping it. None when the probe is
/// disabled or unavailable.
fn latency_for(app: &AppHandle) -> Option<u32> {
    let db = app.state::<crate::db::Db>();
    let public = db.setting("net_ping_public").map(|v| v == "1").unwrap_or(false);
    let target = if public {
        Some(
            db.setting("net_ping_host")
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "1.1.1.1".to_string()),
        )
    } else {
        default_gateway()
    }?;
    probe_latency(&target).ok()
}

// -- pure maths (unit-tested) -------------------------------------------------

/// Worth counting when the interface is up and is neither a software loopback
/// nor a PPP dial-up adapter (both otherwise dominate the numbers).
pub fn eligible(r: &Row) -> bool {
    r.oper == IF_OPER_STATUS_UP && r.kind != IF_TYPE_SOFTWARE_LOOPBACK && r.kind != IF_TYPE_PPP
}

/// bytes -> bits/sec over a 100ns tick delta. None when the delta is zero
/// (two reads inside the same clock tick would divide by zero).
pub fn bps_from_delta(prev: u64, cur: u64, delta_ticks_100ns: u64) -> Option<u64> {
    if delta_ticks_100ns == 0 {
        return None;
    }
    let bytes = cur.wrapping_sub(prev) as u128; // counters can wrap at 2^64
    // 1 s = 10_000_000 ticks of 100 ns
    let bps = bytes * 8 * 10_000_000 / delta_ticks_100ns as u128;
    Some(bps.min(u64::MAX as u128) as u64)
}

/// Sum throughput across every eligible interface and pick the busiest one for
/// display. `prev` is replaced with the current counters on each call.
/// When `priming` is true only the baselines are recorded (no rates yet).
pub fn aggregate(
    rows: &[Row],
    prev: &mut HashMap<u32, (u64, u64, u64)>,
    priming: bool,
) -> (u64, u64, u64, u64, String) {
    let mut down = 0u64;
    let mut up = 0u64;
    let mut next = HashMap::new();
    let mut best: Option<&Row> = None;
    let mut best_score = 0u64;

    for r in rows.iter().filter(|r| eligible(r)) {
        if !priming {
            if let Some(&(pin, pout, pts)) = prev.get(&r.index) {
                let dt = r.ticks.saturating_sub(pts);
                if let Some(b) = bps_from_delta(pin, r.in_octets, dt) {
                    down = down.saturating_add(b);
                }
                if let Some(b) = bps_from_delta(pout, r.out_octets, dt) {
                    up = up.saturating_add(b);
                }
            }
        }
        next.insert(r.index, (r.in_octets, r.out_octets, r.ticks));

        let score = r.in_octets.saturating_add(r.out_octets);
        if score >= best_score {
            best_score = score;
            best = Some(r);
        }
    }

    *prev = next;
    match best {
        Some(r) => (down, up, r.link_down_bps, r.link_up_bps, r.name.clone()),
        None => (0, 0, 0, 0, String::new()),
    }
}

// -- latency probe ------------------------------------------------------------

/// Resolve a host or literal to IPv4. Uses std only, so no extra WinSock
/// plumbing is needed on this path.
pub fn resolve_v4(host: &str) -> Result<std::net::Ipv4Addr, String> {
    if let Ok(a) = host.parse::<std::net::Ipv4Addr>() {
        return Ok(a);
    }
    use std::net::ToSocketAddrs;
    (host, 0u16)
        .to_socket_addrs()
        .map_err(|e| format!("cannot resolve {host}: {e}"))?
        .filter_map(|a| match a {
            std::net::SocketAddr::V4(v) => Some(*v.ip()),
            _ => None,
        })
        .next()
        .ok_or_else(|| format!("no IPv4 address for {host}"))
}

/// One ICMP echo. The IcmpSendEcho API needs no administrator rights.
pub fn probe_latency(host: &str) -> Result<u32, String> {
    let ip = resolve_v4(host)?;
    #[cfg(windows)]
    {
        icmp_ping(&ip)
    }
    #[cfg(not(windows))]
    {
        let _ = ip;
        Err("latency probe requires Windows".to_string())
    }
}

#[cfg(windows)]
fn icmp_ping(ip: &std::net::Ipv4Addr) -> Result<u32, String> {
    use windows::Win32::NetworkManagement::IpHelper::{
        IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY,
    };
    const IP_SUCCESS: u32 = 0;
    /// ICMP_ECHO_REPLY embeds 8-byte fields, so the reply buffer must be
    /// aligned — a plain [u8; N] is only 1-byte aligned and dereferencing it
    /// is undefined behaviour (caught by the debug alignment check).
    #[repr(C, align(8))]
    struct ReplyBuf([u8; 128]);

    unsafe {
        let h = IcmpCreateFile().map_err(|e| format!("IcmpCreateFile: {e}"))?;
        // ICMP_ECHO_REPLY + 8 bytes payload, with room to spare
        let mut buf = ReplyBuf([0u8; 128]);
        let payload = [0u8; 8];
        let dest = u32::from_ne_bytes(ip.octets()); // network-order bit pattern
        let n = IcmpSendEcho(
            h,
            dest,
            payload.as_ptr() as *const _,
            payload.len() as u16,
            None,
            buf.0.as_mut_ptr() as *mut _,
            buf.0.len() as u32,
            1000,
        );
        let _ = IcmpCloseHandle(h);
        if n == 0 {
            return Err("no echo reply".to_string());
        }
        let reply = &*(buf.0.as_ptr() as *const ICMP_ECHO_REPLY);
        if reply.Status != IP_SUCCESS {
            return Err(format!("ping failed with status {}", reply.Status));
        }
        Ok(reply.RoundTripTime)
    }
}

/// The default gateway, found via the 0.0.0.0/0 route. This is what keeps the
/// default latency probe strictly inside the local network.
pub fn default_gateway() -> Option<String> {
    #[cfg(windows)]
    {
        default_gateway_win()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn default_gateway_win() -> Option<String> {
    use windows::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIpForwardTable2, MIB_IPFORWARD_ROW2, MIB_IPFORWARD_TABLE2,
    };
    use windows::Win32::Networking::WinSock::AF_INET;
    unsafe {
        let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
        if GetIpForwardTable2(AF_INET, &mut table).0 != 0 || table.is_null() {
            return None;
        }
        let n = (*table).NumEntries as usize;
        let base = (*table).Table.as_ptr();
        let mut gw = None;
        for i in 0..n {
            let r: &MIB_IPFORWARD_ROW2 = &*base.add(i);
            if r.DestinationPrefix.PrefixLength != 0 {
                continue; // not the default route
            }
            let ip = std::net::Ipv4Addr::from(r.NextHop.Ipv4.sin_addr.S_un.S_addr.to_ne_bytes());
            if ip.is_unspecified() {
                continue;
            }
            gw = Some(ip.to_string());
            break;
        }
        FreeMibTable(table as *const _);
        gw
    }
}

// -- Win32 table read ---------------------------------------------------------

#[cfg(windows)]
fn wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// 100ns units since the first sample; stands in for MIB_IF_ROW2.Timestamp,
/// which windows-rs 0.58 does not project.
#[cfg(windows)]
fn now_100ns() -> u64 {
    static START: LazyLock<std::time::Instant> = LazyLock::new(std::time::Instant::now);
    (START.elapsed().as_nanos() / 100) as u64
}

#[cfg(windows)]
fn read_rows(ticks: u64) -> Result<Vec<Row>, String> {
    use windows::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIfTable2, MIB_IF_ROW2, MIB_IF_TABLE2,
    };
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).0 != 0 || table.is_null() {
            return Err("GetIfTable2 failed".to_string());
        }
        let n = (*table).NumEntries as usize;
        let base = (*table).Table.as_ptr();
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let r: &MIB_IF_ROW2 = &*base.add(i);
            let name = wide(&r.Alias);
            let name = if name.is_empty() { wide(&r.Description) } else { name };
            out.push(Row {
                index: r.InterfaceIndex,
                name,
                kind: r.Type,
                oper: r.OperStatus.0 as u32,
                link_down_bps: r.ReceiveLinkSpeed,
                link_up_bps: r.TransmitLinkSpeed,
                in_octets: r.InOctets,
                out_octets: r.OutOctets,
                ticks,
            });
        }
        FreeMibTable(table as *const _);
        Ok(out)
    }
}

// -- Tauri commands (registered in lib.rs, mocked in src/lib/api.js) ---------

/// Called by the Workspace tab on mount/unmount. This is the only thing that
/// wakes the sampler, which is what keeps invariant #1 honest.
#[tauri::command]
pub fn net_sampler_set(enabled: bool) -> Result<(), String> {
    set_enabled(enabled);
    Ok(())
}

/// Cached last sample so the panel paints before the first tick lands.
#[tauri::command]
pub fn net_snapshot() -> Option<NetSample> {
    snapshot()
}

#[tauri::command]
pub fn net_settings_get(db: tauri::State<crate::db::Db>) -> serde_json::Value {
    serde_json::json!({
        "public": db.setting("net_ping_public").map(|v| v == "1").unwrap_or(false),
        "host": db.setting("net_ping_host").unwrap_or_else(|| "1.1.1.1".to_string()),
        "gateway": default_gateway().unwrap_or_default(),
    })
}

/// public = false (the default) keeps every probe on the local network.
///
/// Async so the one-off DNS lookup for an opt-in public host runs off the main
/// thread (invariant 4). Validating here means a typo fails loudly once instead
/// of the sampler quietly reporting "- ms" forever.
#[tauri::command]
pub async fn net_settings_set(
    db: tauri::State<'_, crate::db::Db>,
    public: bool,
    host: String,
) -> Result<(), String> {
    let host = host.trim().to_string();
    if public {
        if host.is_empty() {
            return Err("Enter a host to ping".to_string());
        }
        // one DNS lookup, up front: better a clear error than a silent "— ms"
        resolve_v4(&host)
            .map_err(|_| format!("Cannot resolve \"{host}\" — check the hostname or IP"))?;
    }
    db.set_setting("net_ping_public", if public { "1" } else { "0" })?;
    db.set_setting(
        "net_ping_host",
        if host.is_empty() { "1.1.1.1" } else { host.as_str() },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(index: u32, kind: u32, oper: u32, din: u64, dout: u64, ticks: u64) -> Row {
        Row {
            index,
            name: format!("iface{index}"),
            kind,
            oper,
            link_down_bps: 1_000_000_000,
            link_up_bps: 1_000_000_000,
            in_octets: din,
            out_octets: dout,
            ticks,
        }
    }

    #[test]
    fn first_sample_primes_without_rates() {
        let rows = vec![row(1, 6, IF_OPER_STATUS_UP, 10_000, 5_000, 100)];
        let mut prev = HashMap::new();
        let (down, up, _, _, name) = aggregate(&rows, &mut prev, true);
        assert_eq!((down, up), (0, 0), "priming must not invent traffic");
        assert_eq!(name, "iface1");
        assert_eq!(prev.len(), 1);
    }

    #[test]
    fn octets_per_second_over_one_second() {
        // 1_000_000 bytes in over exactly one second (10_000_000 x 100ns)
        // => 8_000_000 bits/sec down, 4_000_000 up
        let mut prev = HashMap::new();
        aggregate(&[row(1, 6, IF_OPER_STATUS_UP, 0, 0, 100)], &mut prev, true);
        let second = vec![row(1, 6, IF_OPER_STATUS_UP, 1_000_000, 500_000, 100 + 10_000_000)];
        let (down, up, _, _, _) = aggregate(&second, &mut prev, false);
        assert_eq!(down, 8_000_000);
        assert_eq!(up, 4_000_000);
    }

    #[test]
    fn bps_rejects_zero_tick_delta() {
        assert_eq!(bps_from_delta(0, 1_000, 0), None);
    }

    #[test]
    fn bps_handles_counter_wrap() {
        // u64 counter wraps to a small value; the delta must still be sane
        let bps = bps_from_delta(u64::MAX - 99, 100, 10_000_000).unwrap();
        assert_eq!(bps, 200 * 8); // 200 bytes over 1s
    }

    #[test]
    fn loopback_and_ppp_are_ignored() {
        assert!(!eligible(&row(1, IF_TYPE_SOFTWARE_LOOPBACK, IF_OPER_STATUS_UP, 0, 0, 0)));
        assert!(!eligible(&row(2, IF_TYPE_PPP, IF_OPER_STATUS_UP, 0, 0, 0)));
        assert!(!eligible(&row(3, 6, 2 /* down */, 0, 0, 0)));
        assert!(eligible(&row(4, 6, IF_OPER_STATUS_UP, 0, 0, 0)));
        assert!(eligible(&row(5, 71 /* 802.11 */, IF_OPER_STATUS_UP, 0, 0, 0)));
    }

    #[test]
    fn traffic_sums_across_nics_and_picks_the_busiest() {
        let mut prev = HashMap::new();
        let a = row(1, 6, IF_OPER_STATUS_UP, 0, 0, 100);
        let b = row(2, 71, IF_OPER_STATUS_UP, 0, 0, 100);
        aggregate(&[a, b], &mut prev, true);

        let a2 = row(1, 6, IF_OPER_STATUS_UP, 1_000_000, 0, 100 + 10_000_000);
        let b2 = row(2, 71, IF_OPER_STATUS_UP, 2_000_000, 0, 100 + 10_000_000);
        let (down, up, _, _, name) = aggregate(&[a2, b2], &mut prev, false);
        assert_eq!(down, 24_000_000); // 3 MB -> 24 Mbit/s
        assert_eq!(up, 0);
        assert_eq!(name, "iface2", "the busiest NIC is the one shown");
    }

    #[test]
    fn resolve_accepts_literals() {
        assert_eq!(resolve_v4("127.0.0.1").unwrap().to_string(), "127.0.0.1");
    }

    #[cfg(windows)]
    #[test]
    fn read_table_from_the_real_kernel() {
        let rows = read_rows(1).expect("GetIfTable2 must work on Windows");
        // A machine always has at least the loopback adapter, so the call must
        // return something even when nothing is eligible.
        assert!(!rows.is_empty(), "expected at least one interface");
        for r in &rows {
            assert!(r.ticks > 0 || r.in_octets > 0, "row {r:?} looks unpopulated");
        }
    }

    #[cfg(windows)]
    #[test]
    fn icmp_round_trip_on_loopback() {
        // 127.0.0.1 always answers; this proves the ICMP handle/alloc path.
        let rtt = probe_latency("127.0.0.1");
        assert!(rtt.is_ok(), "loopback ping failed: {rtt:?}");
    }
}
