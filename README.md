# Isle

A lightweight, fully local "Dynamic Island" for Windows: music controls, clipboard history, tasks, calendar with
reminders, a workspace dashboard (live network, digital wellbeing, keep-on-top window macros), in a pill that
springs open and closed.

## Features

- **Media Controls**: Control Spotify, YouTube, VLC, and any media app with album art display
- **Clipboard History**: Automatic text capture with search, pin, and restore
- **Tasks & Calendar**: Manage to-dos and events with reminder notifications
- **Workspace Dashboard**: Live network throughput and latency (LAN-only ping by default), a private log of the
  apps you actually used today, and a running-apps monitor with per-app CPU / memory / GPU load that doubles as a
  one-click "keep this window on top" macro list
- **Live Progress**: Monitor battery, downloads, and app installs with a visual ring plus the exact percentage
  in the pill ("Downloading… 42%")
- **System Integration**: Fullscreen detection, battery saver awareness, idle auto-hide
- **Weather chip**: current conditions and temperature in the pill for a place you pick, cached so it paints
  instantly; sourced from Open-Meteo (no key, no account)
- **Smooth motion**: pill drop-in, morphing panel, tab cross-fade, week strip that slides with the arrows, and
  hidden scrollbars
- **Privacy First**: 100% local data (SQLite), no telemetry. Two requests exist, both opt-in: the latency ping
  (your own gateway unless you choose a public host) and the weather fetch for the place you set

## Quick Start

### For Users (Download Installer)
1. Download the latest NSIS installer from the [Releases](https://github.com/AmritBakshi2003/Isle/releases) page
2. Run `Isle_0.3.1_x64-setup.exe` (or latest version)
3. Press `Ctrl+Alt+Space` to show the pill
4. Click to expand and access all features

### For Developers
1. Install [Rust](https://rustup.rs), Node 20+, and Visual Studio Build Tools (C++ workload)
2. `npm install`
3. `npm run tauri dev` for development
4. `npm run tauri build` to create an installer

## Usage

- **Show/Hide**: Press `Ctrl+Alt+Space` or click the pill
- **Expand**: Click the pill (or hover if enabled in settings)
- **Close**: Press `Esc` or click outside
- **Switch Tabs**: Click tab icons or use `Alt+1..6`
- **Settings**: Click the Settings tab in the expanded island

## Changelog

### v0.3.1
**Performance**
- Idle cost, measured on Windows 11 with the island collapsed: the whole process tree used to sit at ~11 % of a
  core (WebView2's GPU process alone ~7.8 %) and ~561 MB of working set. The equaliser and charging bolt were
  infinite CSS animations, so the compositor never stopped; they now run a few cycles and hold a static frame.
- The Settings window is created the first time you open it and destroyed when you close it, instead of being
  pre-created hidden at startup. That removes a permanent WebView2 renderer (tens of MB) for users who never
  open Settings.
- Watcher threads no longer read SQLite on every tick: the idle watcher compares cached atomics (and wakes every
  15 s when auto-hide is off), the installer watcher checks its setting before querying the service manager, and
  the BITS watcher backs off to a minute when nothing is copying.
- Dropped two commands nothing called (`clipboard_enabled_get`, `update_hide_settings`).

**Calendar**
- The week strip is a real horizontal scroller: sweep a trackpad (or use the wheel, or the chevrons) to move
  between weeks. The scrollbar is hidden, swipes snap to whole weeks, and the strip recentres itself so it pages
  forever without growing the DOM.

**Weather**
- **Use my location**: reads the device position from the Windows location service (the same source as the
  Windows Weather app). Only the resulting coordinates go to Open-Meteo — nothing else, and only after you ask.
- Searching now shows candidates instead of silently taking the first hit, so "Springfield" or "Mumbai" can be
  disambiguated; results are cached in memory, the list is debounced, and a picked candidate saves the exact
  coordinates shown.
- Pasted coordinates (`19.07, 72.88`) are accepted directly — no geocoding request at all.

### v0.3.0
**Pill**
- Progress labels now carry the exact percent ("Installing… 64%"), laid out on one line with the weather chip and
  battery so nothing overlaps.
- Optional weather chip (current temperature + WMO icon) for a place chosen in Settings → Weather. Cached in the
  database and refreshed every 30 minutes; off by default.

**Music**
- Album art crossfades between tracks, title/artist rise in, the progress bar glides with a playhead, and the
  play/pause glyph scales in.
- If the "current" Windows media session is paused while another app is playing (e.g. YouTube in Chrome after
  Spotify paused), Isle now follows the app that is actually playing.

**Settings**
- New Weather section (on/off, place lookup, °C/°F, refresh now) and a Workspace module toggle.
- Media gained transport-control density; Modules now lists "Tasks & Calendar" and Workspace.

**Workspace dashboard**
- Network card: live throughput from `GetIfTable2`, ICMP latency, 48 s sparkline. The sampler is parked on a
  condvar and only runs while the Workspace tab is open.
- Digital wellbeing: an event-driven `SetWinEventHook` foreground log (no polling) with a per-app "today" view.
- Keep on top: a `SetWindowPos(HWND_TOPMOST)` macro for the last non-Isle window, or any window you pick.
- Resource monitor: one row per app with CPU (percent of the whole machine), working-set memory and GPU
  utilization from the `GPU Engine` performance counters, sorted heaviest first.
- Settings → Workspace: activity logging on/off, LAN vs public ping target, clear activity log.

**Removed**
- Modes, routines and wallpaper rotation: their UI was already gone, the backend, scheduler sections, action
  runner and the `wallpaper` crate are now deleted. Existing database rows are preserved, not dropped.

**Fixed**
- Throughput maths was off by 10× (wrong tick base); covered by a unit test now.
- The ICMP reply buffer was under-aligned, which is undefined behaviour; tests hit it as a hard abort.
- The play/pause button no longer shows both glyphs at once (the outgoing icon's fade referenced an unimported
  transition, so the old node was never removed); the swap is an enter-only CSS animation now.
- The pill's weather chip now appears only alongside the clock — while music plays, only the album art and
  equaliser are shown.

### v2.1
**Security & Reliability Improvements**
- Added path/URL validation to prevent arbitrary file/URL launch
- Implemented database corruption recovery with automatic backup
- Replaced unsafe unwrap/expect calls with proper error handling
- Added bounds checking for all array/vector indexing operations
- Added scheduler shutdown hook for clean thread termination
- Removed dead code and unused compiler warnings

**Performance Improvements**
- Added concurrency limit for icon extraction (max 4 concurrent operations)
- Optimized system polling to stop when features are disabled
- Reduced CPU usage when auto-hide features are turned off

**Accessibility Improvements**
- Added ARIA roles and keyboard navigation to modal overlays
- Added aria-label to all icon-only buttons
- Implemented reduced motion support for motion-sensitive users
- Improved screen reader compatibility

**Code Quality**
- Added named constants for magic numbers in scheduler
- Standardized version numbers across all configuration files
- Added CHANGELOG.md for tracking version history
- Fixed hotkey documentation inconsistency

**Build & Release**
- Version standardized to 0.2.4 across Cargo.toml, tauri.conf.json, and package.json
- All tests passing (23 Rust unit tests at the time; 38 as of v0.3.0)
- Frontend build with zero warnings

### v0.1.0
- Initial release with core Dynamic Island functionality
- Media controls, clipboard history, tasks, calendar
- Modes, routines, and wallpaper rotation
- System integration (fullscreen, battery, idle detection)

## Data Storage

All data is stored locally in `%APPDATA%\app.isle.island\isle.db` (SQLite). No data leaves your machine.

## Development

**Preview UI without Rust**: `npm run dev` and open http://localhost:1420 (uses mock data)

**Tests**: `cd src-tauri && cargo test` (includes real Win32 calls: interface table + loopback ICMP)

**Architecture**: See **AGENTS.md** for detailed architecture, command contracts, and invariants.

## Platform

Windows 10/11 only (uses Windows-specific APIs: GSMTC, BITS, Battery, Clipboard, Wallpaper)

## License

MIT License - see [LICENSE](LICENSE) file for details
