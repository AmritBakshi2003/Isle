# Isle

A lightweight, fully local "Dynamic Island" for Windows: music controls, clipboard history, tasks, calendar with
reminders, one-tap modes, time-based routines and wallpaper rotation, in a pill that springs open and closed.

## Features

- **Media Controls**: Control Spotify, YouTube, VLC, and any media app with album art display
- **Clipboard History**: Automatic text capture with search, pin, and restore
- **Tasks & Calendar**: Manage to-dos and events with reminder notifications
- **Modes & Routines**: Create custom action presets and time-based automation
- **Wallpaper Rotation**: Automatic cycling through your wallpaper folder
- **Live Progress**: Monitor battery, downloads, and app installs with visual ring
- **System Integration**: Fullscreen detection, battery saver awareness, idle auto-hide
- **Privacy First**: 100% local data (SQLite), no telemetry, no network calls

## Quick Start

### For Users (Download Installer)
1. Download the latest NSIS installer from the [Releases](https://github.com/AmritBakshi2003/Isle/releases) page
2. Run `Isle_0.2.4_x64-setup.exe` (or latest version)
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

### v2.1 (Current)
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
- All tests passing (23 Rust unit tests)
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

**Architecture**: See **AGENTS.md** for detailed architecture, command contracts, and invariants.

## Platform

Windows 10/11 only (uses Windows-specific APIs: GSMTC, BITS, Battery, Clipboard, Wallpaper)

## License

MIT License - see [LICENSE](LICENSE) file for details
