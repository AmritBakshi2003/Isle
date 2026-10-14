# AGENTS.md: read this first

Isle is a Dynamic-Island-style productivity overlay for **Windows 10/11**.
A small black pill sits at the top-centre of the screen; click it (or press
**Ctrl+Alt+Space**) and it morphs into a panel with Music, Clipboard, Tasks,
Calendar and Modes/Routines/Wallpaper. **All data is local** (one SQLite file). No network calls anywhere.

Stack: **Tauri 2** (Rust backend + WebView2) · **Svelte 5** (runes) · **Vite** · **SQLite** (rusqlite, bundled).

## Status: what is verified and what is not
- Frontend: `npm run build` passes. `npm run dev` runs the full UI in a browser against an in-memory mock (`src/lib/api.js`).
- **Rust has NOT been compiled** by the author (no Rust/Windows toolchain was available). Expect a few small compile
  fixes on first `cargo build`, most likely in `media.rs` (windows-rs 0.58 WinRT signatures), `clipboard.rs`
  (clipboard-master 3.x API) and `island.rs` (global-shortcut handler signature). Fix them, don't redesign.
- Nothing has been tested on a real desktop yet (hotkey, transparency, GSMTC, clipboard, wallpaper, toasts).

## Run it
```
npm install
npm run tauri dev      # needs Rust (rustup) + WebView2 (preinstalled on Win 11) + MSVC Build Tools
npm run tauri build    # NSIS installer in src-tauri/target/release/bundle/nsis
npm run dev            # UI only, in a browser, with mock data
```

## Architecture
```
 Svelte UI (src/)  --invoke(cmd,args)-->  Rust commands (commands.rs, island.rs, media.rs)
        ^                                        |
        |  events (listen)                        v
        +---- media://update, clipboard://changed, island://toggle ---- background threads:
                                                   media.rs      GSMTC listener   (event-driven)
                                                   clipboard.rs  clipboard listener (event-driven)
                                                   scheduler.rs  reminders/routines/wallpaper (sleeps until next due)
```
Backend files (src-tauri/src): `lib.rs` startup order · `db.rs` schema + `Db::with` · `commands.rs` all CRUD commands ·
`actions.rs` action runner + wallpaper · `scheduler.rs` timers · `media.rs` · `clipboard.rs` · `island.rs` window + hotkey.
UI files (src): `App.svelte` shell/animation · `views/*.svelte` one per tab (+ `Builder.svelte` for modes/routines) ·
`lib/api.js` backend bridge + browser mock · `lib/state.svelte.js` shared state · `app.css` tokens/styles.

## Command contract (UI -> Rust). JS uses camelCase args.
| Area | Commands |
|---|---|
| window | `island_resize(expanded)` |
| media | `media_get()`, `media_control(cmd: play_pause\|next\|prev)` |
| tasks | `task_list`, `task_add(title, due?)`, `task_toggle(id)`, `task_delete(id)`, `task_clear_done` |
| events | `event_list(from, to)`, `event_add(title, start, end?, remindMin)`, `event_delete(id)` |
| clipboard | `clip_list(query?)`, `clip_copy(id)`, `clip_pin(id)` (toggle), `clip_delete(id)`, `clip_clear` (keeps pinned) |
| modes | `mode_list`, `mode_save(id?, name, icon, actions)`, `mode_delete(id)`, `mode_activate(id)` |
| routines | `routine_list`, `routine_save(id?, name, trigger, actions)`, `routine_toggle(id, enabled)`, `routine_delete(id)`, `routine_run(id)` |
| misc | `wallpaper_next`, `setting_get(key)`, `setting_set(key, value)`, `autostart_get`, `autostart_set(enabled)` |

Events (Rust -> UI): `media://update` (MediaState), `clipboard://changed`, `island://toggle`.
Timestamps are **unix seconds**. Settings keys: `wallpaper_dir`, `wallpaper_interval` (min, 0=off), `wallpaper_idx`,
`wallpaper_last`, `active_mode`.

### Action JSON (modes + routines), implemented in `actions.rs`
`{"type":"open","target":"path|url"}` · `{"type":"wallpaper_next"}` · `{"type":"notify","title","body"}` ·
`{"type":"media","cmd":"play_pause|next|prev"}`. Routine trigger: `{"kind":"manual"}` or
`{"kind":"time","at":"HH:MM","days":[0..6]}` (0 = Sunday, empty = every day).

## Invariants: do not break these
1. **Idle must be ~0 CPU.** No polling loops, no `requestAnimationFrame` while collapsed. Use events; the scheduler sleeps
   until the next due item. The only recurring UI timers: 1/min clock, 500 ms progress (Music tab only while mounted).
2. **Window size = interaction area.** `island.rs` COLLAPSED/EXPANDED must match `.island` sizes in `App.svelte` + margins
   for spring overshoot and shadow. Expand: resize window, then animate. Collapse: animate, then resize (520 ms later).
3. **Animate transform/opacity** (the single `.island` box also animates width/height/radius; keep it that way).
4. **Never nest `Db::with`** (mutex deadlock). Blocking work (shell, wallpaper, clipboard write) goes in `async` commands.
5. **Local only.** No telemetry, no cloud calls. Any future sync must be opt-in.
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
- Actions: set volume, do-not-disturb, window layouts, run-on-app-launch triggers.
- Calendar: ICS import, recurring events, task due dates in UI (backend supports `due`).
- Media: seek, per-app session picker. Own icon set (current icons are placeholders: `npm run tauri icon <png>`).
- Tests: Rust unit tests for `scheduler::tick` logic (extract pure functions); Playwright smoke test against `npm run dev`.
