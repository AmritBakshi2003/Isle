# Isle

A lightweight, fully local "Dynamic Island" for Windows: music controls, clipboard history, tasks, calendar with
reminders, one-tap modes, time-based routines and wallpaper rotation, in a pill that springs open and closed.

**Quick start**
1. Install [Rust](https://rustup.rs), Node 20+, and Visual Studio Build Tools (C++ workload). WebView2 ships with Windows 11.
2. `npm install`
3. `npm run tauri dev`  (or `npm run tauri build` for an installer)

**Use it**: click the pill or press `Ctrl+Alt+Space`. `Esc` or clicking elsewhere closes it. `Alt+1..5` switches tabs.
Data lives in `%APPDATA%\app.isle.island\isle.db`.

**Preview the UI without Rust**: `npm run dev` and open http://localhost:1420 (uses mock data).

Contributors and AI agents: see **AGENTS.md** for architecture, contracts and invariants.
Note: the Rust half has not been compiled yet; see the Status section in AGENTS.md.
