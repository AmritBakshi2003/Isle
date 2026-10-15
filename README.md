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
2. Run `Isle_0.1.0_x64-setup.exe`
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

## Data Storage

All data is stored locally in `%APPDATA%\app.isle.island\isle.db` (SQLite). No data leaves your machine.

## Development

**Preview UI without Rust**: `npm run dev` and open http://localhost:1420 (uses mock data)

**Architecture**: See **AGENTS.md** for detailed architecture, command contracts, and invariants.

## Platform

Windows 10/11 only (uses Windows-specific APIs: GSMTC, BITS, Battery, Clipboard, Wallpaper)

## License

MIT License - see [LICENSE](LICENSE) file for details
