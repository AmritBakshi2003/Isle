# AGENTS.md: read this first

Isle is a Dynamic-Island-style productivity overlay for **Windows 10/11**.
A small black pill sits at the top-centre of the screen; click it (or press
**Ctrl+Alt+Space**) and it morphs into a panel with Music, Clipboard, Tasks,
Calendar, Utility and **Workspace** (network, digital wellbeing, window macros).
**All data is local** (one SQLite file). Isle sends nothing unless you ask it to:
the optional latency ping (defaults to your own gateway) and the optional weather
chip (your own chosen place). Both are off until enabled in Settings.

Stack: **Tauri 2** (Rust backend + WebView2) · **Svelte 5** (runes) · **Vite** · **SQLite** (rusqlite, bundled).

## Status: what is verified and what is not
- `npm run build` passes; `npm run dev` runs the full UI in a browser against the in-memory mock
  (`src/lib/api.js`). Every tab was exercised end to end there (live samples, sparkline, per-app rows and
  float/release, calendar week slide, music crossfade, weather chip, settings sections).
- Rust **compiles clean** (`cargo check`: 0 errors, 0 warnings) and `cargo test` passes **56** tests,
  including the ones that hit real Windows APIs: `GetIfTable2` reads this machine's interface table
  and `IcmpSendEcho` round-trips on loopback.
- Still unverified on a real desktop: the global hotkey, transparency, GSMTC, clipboard capture,
  toast notifications, the wellbeing foreground hook and the topmost macro (all need an interactive
  session; the unit tests only cover the pure logic and the non-interactive Win32 calls).

## Run it
```
npm install
npm run tauri dev      # needs Rust (rustup) + WebView2 (preinstalled on Win 11) + MSVC Build Tools
npm run tauri build    # NSIS installer in src-tauri/target/release/bundle/nsis
npm run dev            # UI only, in a browser, with mock data
cd src-tauri && cargo test   # 56 unit tests (network maths, GPU/CPU maths, wellbeing SQL, weather parsing, ...)
```

## What it costs (measured, Windows 11, island collapsed)
| Piece | Idle cost | Notes |
|---|---|---|
| `isle.exe` (backend, 30 threads) | ~35 MB private / 83 MB working, <0.2 % CPU | SQLite + fixed buffers; the samplers are parked |
| WebView2 browser + GPU + utilities | ~200 MB working | fixed cost of WebView2 itself |
| WebView2 renderer per window | ~40–90 MB private | **why Settings is built on demand and destroyed on close** |
| Infinite CSS animation | **7.8 % of a core** (GPU process) | the equaliser used to loop forever — now 2–3 cycles |

Before the v0.3.x pass the whole tree sat at **561 MB working set / 239 MB private** and **11.4 % of one
core while idle**; almost all of that CPU was the compositor repainting a 210×46 transparent window for
animations that had long stopped conveying anything. Measure before claiming: `Get-Process isle,
msedgewebview2` and diff `TotalProcessorTime` over 10 s. Two WebView2 windows also mean two renderers
plus the browser process — never pre-create a window you might not need.

## Architecture
```
 Svelte UI (src/)  --invoke(cmd,args)-->  Rust commands (commands.rs, island.rs, media.rs,
        ^                                        network.rs, wellbeing.rs, topmost.rs, weather.rs)
        |  events (listen)                       |
        +---- media://update, clipboard://changed, island://toggle, weather://update,
              workspace://network, workspace://apps, wellbeing://changed
                                                 v
                                background threads:
                                  media.rs      GSMTC listener        (event-driven)
                                  clipboard.rs  clipboard listener    (event-driven)
                                  network.rs    sampler              (1 Hz, only while Workspace is open)
                                  procs.rs      CPU/RAM/GPU per app  (1 Hz, only while Workspace is open)
                                  wellbeing.rs  WinEvent foreground hook (event-driven)
                                  system.rs     fullscreen/battery/idle (gated by settings)
                                  scheduler.rs  event reminders        (sleeps until next due)
                                  weather.rs    current conditions      (wakes every 30 min, no-op while off)
```
Backend files (src-tauri/src): `lib.rs` startup order · `db.rs` schema + `Db::with` · `commands.rs` all CRUD commands ·
`actions.rs` notifications (used by reminders) · `scheduler.rs` timers · `media.rs` · `clipboard.rs` · `island.rs`
window + hotkey · `network.rs` throughput + ICMP · `wellbeing.rs` foreground log · `topmost.rs` window macros ·
`procs.rs` per-app CPU/RAM/GPU sampling (PDH GPU Engine counters + GetProcessTimes) · `weather.rs` Open-Meteo
current conditions over WinHTTP (the WinRT HttpClient fails TLS on this machine, so don't switch it back).
UI files (src): `App.svelte` shell/animation · `views/*.svelte` one per tab · `lib/api.js` backend bridge + browser
mock · `lib/state.svelte.js` shared state · `app.css` tokens/styles.

## Command contract (UI -> Rust). JS uses camelCase args.
| Area | Commands |
|---|---|
| window | `island_resize(expanded)` |
| media | `media_get()`, `media_control(cmd: play_pause\|next\|prev)` |
| tasks | `task_list`, `task_add(title, due?)`, `task_toggle(id)`, `task_delete(id)`, `task_clear_done` |
| events | `event_list(from, to)`, `event_add(title, start, end?, remindMin)`, `event_delete(id)` |
| clipboard | `clip_list(query?)`, `clip_copy(id)`, `clip_pin(id)` (toggle), `clip_delete(id)`, `clip_clear` (keeps pinned) |
| network | `net_sampler_set(enabled)`, `net_snapshot()`, `net_settings_get()`, `net_settings_set(public, host)` |
| wellbeing | `wellbeing_today()`, `wellbeing_range(from, to)`, `wellbeing_enabled_get/set`, `wellbeing_clear` |
| window macros | `topmost_list()`, `topmost_float(hwnd)`, `topmost_release(hwnd)`, `topmost_float_last()`, `topmost_release_all()` |
| resources | `procs_set(enabled)`, `procs_snapshot()` |
| weather | `weather_get()` (cache only), `weather_settings_get()`, `weather_enabled_set(enabled)`, `weather_units_set(units)`, `weather_location_set(query)` (name *or* `"lat, lon"`; geocode unless coords, save, fetch), `weather_place_search(query)` (candidates, cached in memory), `weather_location_pick(place, lat, lon)` (saves exactly that hit), `weather_use_device_location()` (Windows.Devices.Geolocation), `weather_refresh()` |
| misc | `setting_get(key)`, `setting_set(key, value)`, `autostart_get`, `autostart_set(enabled)` |

Events (Rust -> UI): `media://update` (MediaState), `clipboard://changed`, `island://toggle`,
`workspace://network` (NetSample, 1 Hz), `workspace://apps` (AppsSample, 1 Hz), `wellbeing://changed`
(unit; re-query, do not assume a row shape), `weather://update` (Weather: `ok`, `place`, `temp_c`, `feels_c`,
`humidity`, `wind_kph`, `code` (WMO), `units`, `error`).

Timestamps are **unix seconds**. Settings keys used by the Workspace tab: `wellbeing_enabled` (default on),
`net_ping_public` (`'1'` = ping `net_ping_host`, `'0'` = ping the default gateway; note `'1'`/`'0'`, **not**
`'true'`/`'false'`), `net_ping_host` (default `1.1.1.1`). Weather keeps `weather_enabled` (`'true'`/`'false'`),
`weather_place`, `weather_lat`, `weather_lon`, `weather_units` (`'c'`/`'f'`) and the serialized `weather_cache` so the
pill paints instantly on startup. Everything else lives under `settings` too — read it with `settings_get_all` rather
than guessing.

### Modes, routines and wallpaper rotation are gone (v0.3.0)
Their UI disappeared in v2.0 and the backend followed: `mode_*`, `routine_*`, `wallpaper_next`, the action runner
and the rotation timer were deleted, along with the now-unused `wallpaper` crate. The `modes`/`routines` tables and
their rows are still in the database on purpose (data loss is worse than dead tables); nothing reads them.
`actions.rs` is now just notifications for the reminder scheduler.

## Invariants: do not break these
1. **Idle must be ~0 CPU.** No polling loops, no `requestAnimationFrame` while collapsed. Use events; the scheduler
   sleeps until the next due item. The only recurring UI timers: 1/min clock, 500 ms progress (Music tab only while
   mounted). `network.rs` and `procs.rs` are **parked on condvars** until the Workspace tab calls
   `net_sampler_set(true)` / `procs_set(true)` on mount, and both skip every tick while the island window is hidden —
   never turn them into free-running loops.
   **No infinite CSS animations in the pill.** Measured on this machine: `animation … infinite` on the equaliser
   kept WebView2's GPU process at ~7.8 % of a core forever (11 % for the whole tree) with the island idle. The
   equaliser and charging bolt now run 2-3 cycles (`animation: eq .9s ease-in-out 3`) and hold a static frame.
   `prefers-reduced-motion` still collapses everything. Same rule for the settings window (`pulse-rec` only while
   recording) and any future badge. Watcher threads must read cached atomics, not SQLite, per tick (`system.rs`).
2. **Window size = interaction area.** `island.rs` COLLAPSED/EXPANDED must match `.island` sizes in `App.svelte` + margins
   for spring overshoot and shadow. Expand: resize window, then animate. Collapse: animate, then resize (520 ms later).
   The expanded body is **500 × 278**; the Workspace tab spends it on a 104 px top row (Network | Today, 50/50) plus
   one full-width running-apps card, so a view that needs more scrolls inside `.view > .scroll`.
3. **Animate transform/opacity** (the single `.island` box also animates width/height/radius; keep it that way).
   For content swaps (tab pane, music title, calendar week) use an **enter-only CSS animation**, not a Svelte
   `out:` transition: outros keep the outgoing node in the DOM until their last frame, so a throttled/hidden
   WebView can leave two panes stacked and push the new one out of the fixed 278 px body.
4. **Never nest `Db::with`** (mutex deadlock). Blocking work (shell, clipboard write, Win32 calls) goes in `async`
   commands or in the feature's own worker thread (see `wellbeing.rs`: hook thread only forwards, one consumer writes).
5. **Local only.** No telemetry, no cloud calls. Any future sync must be opt-in. Exactly two requests exist, both
   off by default: the ICMP probe (LAN-only unless the user opts into a public host in Settings → Workspace) and the
   weather fetch (only after the user picks a place in Settings → Weather; coordinates are the whole payload). The
   place may come from a typed name, pasted coordinates, or `weather_use_device_location` — that last one reads the
   position from the Windows location service locally and sends only the resulting coordinates, same as a typed
   place. Never send the user's IP, device id or a place name together with a forecast.
6. Add every new command to `generate_handler!` in `lib.rs` AND to the mock in `lib/api.js`.
7. Keep SQL column names free of keywords (see `db.rs` header). Schema changes: add `ALTER TABLE` migration steps, never edit
   the `CREATE TABLE`s in place for existing users.

## How to add a feature (example: a "quick notes" tab)
1. `db.rs`: add table to `SCHEMA`. 2. `commands.rs`: add `note_list/note_add/...` and register in `lib.rs`.
3. `lib/api.js`: add mock cases. 4. `views/Notes.svelte`: new view. 5. `App.svelte`: add to `tabs` (+ `grid-template-columns`
   and `.pill` width calc use the tab count) and the `{#if ui.tab ...}` chain. 6. Update this file.

## Known gaps / good next tasks
- Hide the island over fullscreen apps (`SHQueryUserNotificationState`) and keep it out of Alt-Tab (`WS_EX_TOOLWINDOW`).
- Multi-monitor choice (currently primary monitor only). Re-centre on display/DPI change events.
- Clipboard: images and files; per-app ignore list.
- Wellbeing: a non-elevated `SetWinEventHook` never sees elevated windows (UIPI), so admin tools are missing from the
  log. Options: an elevated helper, or a documented opt-in ETW session. Also no per-app ignore list or idle timeout yet.
- Network: pick which adapter is shown (currently the busiest), and remember that per-row hardware timestamps are
  unavailable in windows-rs 0.58 — deltas use our own monotonic clock.
- Resources: GPU needs the `GPU Engine` performance counters; some machines/VMs expose none, in which case
  `procs.rs` reports `gpu_available: false` and the UI hides that column. Sampling is per-process, so a process
  with several visible windows shows once (the heaviest row wins).
- Calendar: ICS import, recurring events, task due dates in UI (backend supports `due`).
- Media: seek, per-app session picker. Own icon set (current icons are placeholders: `npm run tauri icon <png>`).
- Weather: only current conditions — no hourly/daily forecast, no pull-to-refresh gesture in the pill. Device
  location exists now, but it is fetched once when the user presses **Use my location**; movement is not tracked and
  there is no re-centre on travel (the 30-min refresh reuses the saved coordinates).
- Media: no seek bar interaction and no per-session picker; with two apps playing at once Isle shows the first
  playing session Windows reports.
- Tests: extract `scheduler::tick` into pure functions and test them; Playwright smoke test against `npm run dev`.
- Repo hygiene: committed blobs are mixed CRLF/LF while `core.autocrlf=true`, so touching an old file shows a
  whole-file diff. A `.gitattributes` with `* text=auto eol=crlf` would fix that once and for all.
