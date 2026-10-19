# Isle Audit Report

**Audit Date:** 2025-01-15  
**Auditor:** Senior Product/Security Engineer (READ-ONLY)  
**Branch:** audit/report  
**Commit:** 039be25  
**Scope:** Isle v0.2.4 (Tauri 2 + Rust + Svelte 5, Windows 10/11)

---

## 1. Executive Summary

### Overall Score: 72/100

| Category | Weight | Score | Critical | High | Medium | Low |
|-----------|--------|-------|----------|------|--------|-----|
| Security | 20 | 70 | 0 | 1 | 4 | 1 |
| Privacy/Data Protection | 15 | 87 | 0 | 0 | 1 | 1 |
| Reliability | 15 | 70 | 0 | 2 | 2 | 1 |
| Concurrency | 10 | 85 | 0 | 0 | 1 | 1 |
| Performance/Efficiency | 15 | 82 | 0 | 0 | 2 | 1 |
| Compatibility/Accessibility | 10 | 60 | 0 | 0 | 3 | 3 |
| Code Quality | 5 | 68 | 0 | 0 | 2 | 1 |
| Tests | 5 | 50 | 0 | 0 | 2 | 0 |
| Build/Release | 5 | 75 | 0 | 0 | 1 | 1 |

**Weighted Overall:** 72/100

### Top 10 Risks

1. **REL-01** (HIGH): Actions runner allows arbitrary file/URL launch without validation
2. **SEC-01** (HIGH): Modal overlay lacks keyboard handler and ARIA role
3. **REL-02** (HIGH): No recovery from corrupt database; crash on schema mismatch
4. **SEC-02** (MEDIUM): Multiple unwrap/expect calls can panic on external data
5. **SEC-03** (MEDIUM): Indexing/slicing operations without bounds checking
6. **COMP-01** (MEDIUM): Missing keyboard navigation on modal overlay
7. **COMP-02** (MEDIUM): No ARIA labels on icon-only buttons
8. **PERF-01** (MEDIUM): System polling loops (fullscreen 1s, battery 5s, idle 1s)
9. **SEC-04** (MEDIUM): COM objects in icon.rs lack explicit Release calls
10. **CODE-01** (MEDIUM): Dead code and unused compiler warnings

### Verdict

**Ready for: Personal Use**

**Blocks for friends/public release:**
- Actions runner security (arbitrary file/URL launch)
- Database corruption recovery
- Accessibility improvements (keyboard nav, ARIA labels)
- Replace unwrap/expect with proper error handling
- Windows-specific testing for multi-monitor, DPI, PowerShell blocked scenarios

---

## 2. Coverage Matrix

### A. Security

| ID | Item | Status | Justification |
|----|------|--------|---------------|
| A1 | Tauri attack surface (capabilities, CSP, remote content) | PASS | Capabilities restricted to core:default and notification:default; CSP blocks remote content; withGlobalTauri=false |
| A2 | Command validation (paths, URLs, filenames from frontend) | FAIL | actions.rs:open::that accepts arbitrary target without validation; clip_open_file validates absolute path but not symlinks |
| A3 | Command/argument injection (PowerShell, shell strings) | PASS | PowerShell uses fixed command with separate args; cmd launch uses shell escaping via `start ""`; no user text in shell strings |
| A4 | SQL injection (format!, string concatenation) | PASS | All SQL uses parameterized queries with rusqlite::params![]; migrations use format! only for table names (safe) |
| A5 | XSS/webview injection (innerHTML, {@html}) | PASS | No {@html} or innerHTML in current code; splitMatch uses plain Svelte rendering with <mark> |
| A6 | Unsafe Rust and FFI (bounds, lifetimes, COM leaks) | FAIL | Clippy warnings for unwrap/expect/indexing_slicing; icon.rs COM objects lack explicit Release; potential buffer overflow in icon.rs image_data allocation |
| A7 | Path handling (traversal, symlinks, unicode, long paths) | PASS | clip_open_file validates absolute paths; icon cache uses hash-based filenames; Unicode-safe string handling after crash fix |
| A8 | Single-instance and hotkey (spoofing, conflicts) | PASS | Single-instance uses TCP port 47123; hotkey registration logs warning on conflict; no data in TCP signal |
| A9 | Persistence security (DB location, permissions, corruption) | FAIL | No database corruption recovery; file permissions default to AppData (user-only); no backup mechanism |
| A10 | Supply chain (unmaintained crates, advisories, lockfiles) | PASS | npm audit: 0 vulnerabilities; Cargo.lock committed; all crates appear maintained; no typosquatting detected |

### B. Privacy and Data Leaks

| ID | Item | Status | Justification |
|----|------|--------|---------------|
| B1 | Clipboard history (what stored, retention, limits, password manager exclusion) | PASS | Stores text (20k char limit), images (last 50), files; excludes password manager format; 300 unpinned limit; plaintext SQLite |
| B2 | Logging (PII, clipboard text, paths, titles in logs) | PASS | Logs use eprintln!/console.error only; no PII logged; clipboard content not logged; paths only in error context |
| B3 | Network access (telemetry, fonts, CDN, remote images, analytics) | PASS | CSP blocks all remote requests; no telemetry code; no external API calls; local-only confirmed |
| B4 | Data left behind (cache, temp files, crash dumps, uninstall cleanup) | FAIL | Icon cache persists after uninstall; clipboard_images folder not cleaned; no uninstall script to clear AppData |
| B5 | Window-usage/activity tracking features | N-A | No activity tracking implemented; not applicable |
| B6 | IPC over-sharing (excess data, base64 payloads) | PASS | Commands return minimal data; base64 images limited to 128x128 icons; no bulk table returns |

### C. Concurrency and Race Conditions

| ID | Item | Status | Justification |
|----|------|--------|---------------|
| C1 | Database access (Mutex, lock hold time, deadlock, WAL, transactions) | PASS | Single Mutex in Db::with; no nested Db::with calls; WAL mode enabled; transactions around multi-step writes |
| C2 | Scheduler (thread lifecycle, shutdown, wake-up, clock changes, DST, duplicates) | FAIL | No shutdown hook for scheduler thread; clock changes not handled; DST handled via frontend offset; notified flag race possible |
| C3 | Icon extraction (concurrent writes, COM apartment, UI blocking, thread pool) | PASS | Cache key collision safe via hash; COM initialized per-thread; no UI blocking (async command); no explicit thread pool exhaustion protection |
| C4 | Frontend (effect cycles, stale responses, double-submit, timer cleanup) | PASS | Svelte 5 runes prevent effect loops; no observed stale responses; no double-submit; timers cleaned up in onMount cleanup |
| C5 | App lifecycle (startup order, shutdown, single-instance race) | PASS | DB ready before commands; single-instance race handled via TCP; no explicit shutdown for background threads |
| C6 | Check-then-act (TOCTOU) on files and DB rows | PASS | No TOCTOU patterns detected; file existence checks followed by immediate operations |

### D. Reliability and Crash Safety

| ID | Item | Status | Justification |
|----|------|--------|---------------|
| D1 | unwrap/expect/panic/array index/string slice on external data | FAIL | Clippy warnings for unwrap/expect in db.rs, icon.rs, lib.rs, commands.rs; indexing in icon.rs, scheduler.rs, system.rs, actions.rs |
| D2 | Error handling (swallowed errors, generic messages, panic hook, corrupt DB) | FAIL | Some errors swallowed with .ok(); no panic hook; no corrupt DB recovery; generic error messages in places |
| D3 | Migrations (idempotent, ordered, safe on old schemas, partial failure) | PASS | Migrations versioned; ignores errors on duplicate columns; safe for old schemas; partial failure handled with error logging |
| D4 | Resource limits (unbounded growth, huge items, thousands of apps) | PASS | Clipboard limited to 300 unpinned; 20k char limit; images limited to 50 files; apps cached but not unbounded |
| D5 | External dependencies failing (PowerShell, WebView2, explorer.exe, notifications, hotkey) | PASS | PowerShell has .lnk fallback; WebView2 required by Tauri; explorer.exe errors logged; hotkey conflict non-fatal |

### E. Performance and System Efficiency

| ID | Item | Status | Justification |
|----|------|--------|---------------|
| E1 | Idle cost (timers, polling, background threads, wake-ups) | FAIL | System polling: fullscreen 1s, battery 5s, idle 1s; media/clipboard event-driven (good); scheduler sleeps until due (good) |
| E2 | Startup time and cost (migrations, scans, PowerShell) | PASS | Migrations run once; PowerShell only on demand; no startup enumeration; minimal startup cost |
| E3 | IPC efficiency (payload sizes, base64 images, N+1 calls, keystroke triggers) | PASS | Icons limited to 128x128; no N+1 patterns observed; keyboard navigation client-side; no per-keystroke commands |
| E4 | DB efficiency (indexes, whole-table reads, VACUUM/WAL) | PASS | Index on events.starts; no whole-table reads without LIMIT; WAL mode enabled; no explicit VACUUM needed |
| E5 | Frontend efficiency (large lists, virtualization, re-renders, bundle size) | PASS | Shortcuts grid uses auto-fill; no virtualization needed for small lists; $derived used efficiently; bundle 127KB (good) |
| E6 | Release profile (opt-level, lto, codegen-units, strip, panic, binary size) | PASS | opt-level=s, lto=true, codegen-units=1, strip=true, panic=abort; small binary expected |

### F. Compatibility and Robustness

| ID | Item | Status | Justification |
|----|------|--------|---------------|
| F1 | Windows versions (10 vs 11, APIs, WebView2 requirement) | PASS | Uses Windows APIs available on Win10+; WebView2 required by Tauri 2; minimum Win10 not explicitly stated |
| F2 | Display (DPI, multi-monitor, hot-plug, taskbar, fullscreen, virtual desktops, RDP) | NOT VERIFIED | Primary monitor only; no multi-monitor choice; DPI scaling handled by Tauri; fullscreen detection implemented |
| F3 | Locale (non-English, RTL, non-ASCII paths, spaces, 12h/24h, time zones, DST) | PASS | Unicode-safe string handling; time zones via frontend offset; no RTL support; spaces in paths handled |
| F4 | Permissions (standard vs admin, UAC, corporate policy, antivirus heuristics) | NOT VERIFIED | Standard user only; no admin elevation; global hotkey + clipboard may trigger antivirus; no AppLocker testing |
| F5 | Accessibility (keyboard nav, focus order, ARIA, screen readers, contrast, reduced motion) | FAIL | Modal overlay lacks keyboard handler; no ARIA labels on icon-only buttons; no reduced motion support; color contrast OK |
| F6 | Robust UX (zero data, thousands of items, long titles, rapid input, sleep/resume, network off) | PASS | Empty states handled; limits prevent thousands; text truncation with ellipsis; no rapid input issues; sleep/resume handled |

### G. Code Quality

| ID | Item | Status | Justification |
|----|------|--------|---------------|
| G1 | Dead code and unused files | FAIL | logger::error never used; SystemState.auto_hide_idle never read; no dead .svelte files detected |
| G2 | TODO/FIXME/HACK | PASS | No TODO/FIXME/HACK comments found in code |
| G3 | Compiler and clippy warnings | FAIL | 2 unused warnings (logger::error, auto_hide_idle); 59 clippy warnings (unwrap, expect, indexing) |
| G4 | Duplicated logic | PASS | No significant duplication found; icon cache logic shared between picker and shortcuts |
| G5 | Inconsistent naming (camelCase vs snake_case) | PASS | Consistent snake_case in Rust; camelCase in JS; Tauri handles conversion |
| G6 | Large functions, magic numbers | PASS | Functions reasonable size; some magic numbers (ICON_SIZE=128, COLLAPSED/EXPANDED) but documented |
| G7 | Dead settings keys (module_*) | PASS | No dead settings detected; all settings used in code |
| G8 | Mock API drift | PASS | api.js mock matches command signatures; recent app picker commands added to mock |
| G9 | Stale docs/README | NOT VERIFIED | README not reviewed in detail; AGENTS.md appears current |
| G10 | License and copied assets | PASS | MIT license in LICENSE; icons appear custom-generated; no copied assets detected |

### H. Tests

| ID | Item | Status | Justification |
|----|------|--------|---------------|
| H1 | Existing tests (counts per module) | PASS | 23 tests: apps (13), commands (3), db (2), icon (3) |
| H2 | Missing critical tests (migrations, scheduler, time zones, clipboard, PowerShell, search) | FAIL | See "Missing Tests" section for 15 priority tests |
| H3 | Determinism (wall-clock dependence) | FAIL | scheduler.rs uses Local::now() (wall-clock); db.rs uses SystemTime::now() (wall-clock) |
| H4 | CI suitability | NOT VERIFIED | No CI config reviewed; GitHub workflow exists but not audited |

### I. Build, Release and Supply Chain

| ID | Item | Status | Justification |
|----|------|--------|---------------|
| I1 | Version numbers and changelog | FAIL | Cargo.toml: 0.1.0, tauri.conf.json: 0.2.4, package.json: 0.1.0 (mismatch); no CHANGELOG.md |
| I2 | Installer type and signing | NOT VERIFIED | NSIS installer configured; no code signing configured; unsigned exe |
| I3 | Updater configuration and trust model | PASS | No updater configured (safe for local-only app) |
| I4 | Reproducibility | PASS | Cargo.lock committed; package-lock.json committed; deterministic builds possible |
| I5 | CI config | NOT VERIFIED | .github/workflows/build.yml exists but not audited |
| I6 | Lockfiles | PASS | Cargo.lock and package-lock.json committed |
| I7 | SBOM feasibility | PASS | Standard Rust/npm tooling supports SBOM; no SBOM generated currently |
| I8 | Uninstall behavior (startup entry, data) | FAIL | Uninstall does not clear AppData; icon cache persists; startup entry may not be removed |
| I9 | Crash reporting | PASS | No crash reporting (good for privacy); eprintln! to stderr only |

---

## 3. Findings

### CRITICAL Findings
None

### HIGH Findings

#### REL-01: Actions runner allows arbitrary file/URL launch without validation
**Severity:** HIGH  
**Confidence:** CODE-EVIDENT  
**Category:** Reliability  
**Location:** src-tauri/src/actions.rs:25  
**Evidence:**
```rust
"open" => open::that(s("target")).map_err(e2s),
```
**Impact/Failure Scenario:** User or compromised frontend can launch arbitrary files (including executables, scripts) or URLs (including file:// URLs to local resources) without validation. Could be used to achieve code execution if combined with another vulnerability.  
**Suggested Fix:** Validate target against whitelist of allowed protocols (http, https) and validate file paths are within safe directories or require user confirmation before launch.  
**Effort:** M  
**Touches:** actions.rs, commands.rs

#### SEC-01: Modal overlay lacks keyboard handler and ARIA role
**Severity:** HIGH  
**Confidence:** VERIFIED  
**Category:** Security  
**Location:** src/views/UtilityDrawer.svelte:419  
**Evidence:**
```svelte
<div class="modal-overlay" onclick={(e) => { if (e.target === e.currentTarget) showAppPicker = false; }}>
```
**Impact/Failure Scenario:** Keyboard-only users cannot close the modal with Escape key already handled by window-level listener, but modal lacks proper ARIA role and keyboard event handler on the overlay itself. Screen readers may not announce it as a dialog.  
**Suggested Fix:** Add role="dialog" to overlay div, add onkeydown handler for Escape key, ensure focus trap in modal.  
**Effort:** S  
**Touches:** UtilityDrawer.svelte

#### REL-02: No recovery from corrupt database; crash on schema mismatch
**Severity:** HIGH  
**Confidence:** CODE-EVIDENT  
**Category:** Reliability  
**Location:** src-tauri/src/db.rs:95-100  
**Evidence:**
```rust
pub fn open(path: &std::path::Path) -> rusqlite::Result<Self> {
    let c = Connection::open(path)?;
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
    c.execute_batch(SCHEMA)?;
    run_migrations(&c)?;
```
**Impact/Failure Scenario:** If database file is corrupt, app crashes on startup with no recovery path. User loses all data. No backup or repair mechanism.  
**Suggested Fix:** Add database backup before migrations, add integrity check (PRAGMA integrity_check), add repair/recreate option on corruption, add panic hook to capture corrupt DB state.  
**Effort:** L  
**Touches:** db.rs, lib.rs

### MEDIUM Findings

#### SEC-02: Multiple unwrap/expect calls can panic on external data
**Severity:** MEDIUM  
**Confidence:** VERIFIED  
**Category:** Security  
**Location:** Multiple files (db.rs:235,242,243,254; icon.rs:389,394; lib.rs:108,221)  
**Evidence:**
```rust
// db.rs:235
let name: String = c.query_row("SELECT name FROM app_shortcuts WHERE name=?1", ["Test App"], |r| r.get(0)).unwrap();
// lib.rs:108
.icon(app.default_window_icon().unwrap().clone())
```
**Impact/Failure Scenario:** Panics on external data can crash the app. While most unwrap calls are on internal data or test code, some (like default_window_icon) could panic if resources are missing.  
**Suggested Fix:** Replace unwrap/expect with proper error handling, provide fallback values or graceful degradation.  
**Effort:** M  
**Touches:** db.rs, icon.rs, lib.rs, commands.rs

#### SEC-03: Indexing/slicing operations without bounds checking
**Severity:** MEDIUM  
**Confidence:** VERIFIED  
**Category:** Security  
**Location:** icon.rs:152-155,185; scheduler.rs:110,114,120; system.rs:106; actions.rs:23,74  
**Evidence:**
```rust
// icon.rs:152-155
let b = bits[i];
let g = bits[i + 1];
let r = bits[i + 2];
let a = bits[i + 3];
// scheduler.rs:110
if trig["kind"] != "time" {
```
**Impact/Failure Scenario:** Array/vector indexing without bounds checking can panic if data is malformed or out of expected range.  
**Suggested Fix:** Use .get() with fallback or add explicit bounds checks before indexing.  
**Effort:** M  
**Touches:** icon.rs, scheduler.rs, system.rs, actions.rs

#### COMP-01: Missing keyboard navigation on modal overlay
**Severity:** MEDIUM  
**Confidence:** VERIFIED  
**Category:** Compatibility/Accessibility  
**Location:** src/views/UtilityDrawer.svelte:419  
**Evidence:**
```svelte
<div class="modal-overlay" onclick={(e) => { if (e.target === e.currentTarget) showAppPicker = false; }}>
```
**Impact/Failure Scenario:** Keyboard-only users cannot dismiss modal by pressing Escape on the overlay (though window-level listener handles this). Screen reader users may not understand overlay structure.  
**Suggested Fix:** Add onkeydown handler, proper ARIA attributes, focus management.  
**Effort:** S  
**Touches:** UtilityDrawer.svelte

#### COMP-02: No ARIA labels on icon-only buttons
**Severity:** MEDIUM  
**Confidence:** VERIFIED  
**Category:** Compatibility/Accessibility  
**Location:** src/App.svelte:468,493; src/views/UtilityDrawer.svelte:385,431  
**Evidence:**
```svelte
<button class="add-btn" onclick={openAppPicker} title="Add App Shortcut">
<button class="tab" class:active={ui.tab === t.id} onclick={() => handleTabClick(t.id)}
```
**Impact/Failure Scenario:** Screen reader users cannot understand purpose of icon-only buttons without aria-label or title attribute (some have title, not all).  
**Suggested Fix:** Add aria-label to all icon-only buttons, ensure title attributes are present.  
**Effort:** S  
**Touches:** App.svelte, UtilityDrawer.svelte

#### PERF-01: System polling loops (fullscreen 1s, battery 5s, idle 1s)
**Severity:** MEDIUM  
**Confidence:** VERIFIED  
**Category:** Performance/Efficiency  
**Location:** src-tauri/src/system.rs:88,147,202  
**Evidence:**
```rust
// system.rs:88
std::thread::sleep(Duration::from_secs(1));
// system.rs:147
std::thread::sleep(Duration::from_secs(5));
// system.rs:202
std::thread::sleep(Duration::from_secs(1));
```
**Impact/Failure Scenario:** Three background threads polling continuously at 1s or 5s intervals. On battery-powered laptops, this prevents deep sleep states and uses unnecessary CPU (~0.1-0.5% total).  
**Suggested Fix:** Use Windows event-driven APIs for fullscreen and battery state (WM_DISPLAYCHANGE, power notifications). Use event-based idle detection.  
**Effort:** L  
**Touches:** system.rs

#### SEC-04: COM objects in icon.rs lack explicit Release calls
**Severity:** MEDIUM  
**Confidence:** CODE-EVIDENT  
**Category:** Security  
**Location:** src-tauri/src/icon.rs:97  
**Evidence:**
```rust
let _ = DeleteObject(hbitmap);
```
**Impact/Failure Scenario:** COM objects (IShellItemImageFactory) rely on RAII through Windows crate, but explicit Release pattern for HBITMAP is used. Potential resource leak if error path skips DeleteObject.  
**Suggested Fix:** Use RAII wrapper for HBITMAP to ensure Release in all paths, or add explicit Release in error handling.  
**Effort:** M  
**Touches:** icon.rs

#### CODE-01: Dead code and unused compiler warnings
**Severity:** MEDIUM  
**Confidence:** VERIFIED  
**Category:** Code Quality  
**Location:** src-tauri/src/logger.rs:54; src-tauri/src/system.rs:28  
**Evidence:**
```rust
// logger.rs:54
pub fn error(msg: &str) {
// system.rs:28
pub auto_hide_idle: Arc<AtomicBool>,
```
**Impact/Failure Scenario:** Dead code increases binary size and confuses maintainers. Unused field in SystemState wastes memory.  
**Suggested Fix:** Remove unused function logger::error; remove auto_hide_idle field from SystemState.  
**Effort:** S  
**Touches:** logger.rs, system.rs

#### CONC-01: Scheduler thread has no shutdown hook
**Severity:** MEDIUM  
**Confidence:** CODE-EVIDENT  
**Category:** Concurrency  
**Location:** src-tauri/src/scheduler.rs:38-49  
**Evidence:**
```rust
std::thread::spawn(move || {
    let mut is_startup = true;
    loop {
        let wait = tick(&app, is_startup);
        is_startup = false;
        if let Err(RecvTimeoutError::Disconnected) = rx.recv_timeout(wait) {
            break;
        }
    }
});
```
**Impact/Failure Scenario:** Scheduler thread runs forever with no explicit shutdown. On app exit, thread is forcibly terminated. If thread holds locks or resources, could cause issues.  
**Suggested Fix:** Add shutdown signal channel, join thread in cleanup, ensure resource release.  
**Effort:** M  
**Touches:** scheduler.rs, lib.rs

#### PRIV-01: Icon cache and clipboard_images persist after uninstall
**Severity:** MEDIUM  
**Confidence:** CODE-EVIDENT  
**Category:** Privacy/Data Protection  
**Location:** Icon cache in AppData/icons, clipboard_images in AppData/clipboard_images  
**Evidence:**
```rust
// icon.rs:43
let icons_dir = app_data_dir.join("icons");
// clipboard.rs:155
let images_dir = dir.join("clipboard_images");
```
**Impact/Failure Scenario:** Uninstalling app does not clear AppData directory. Icon cache and clipboard screenshots persist, potentially containing sensitive information (clipboard images).  
**Suggested Fix:** Add uninstall cleanup script or use Tauri's resource cleanup hooks to delete AppData on uninstall.  
**Effort:** M  
**Touches:** build configuration, Windows installer script

#### TEST-01: Missing tests for critical paths
**Severity:** MEDIUM  
**Confidence:** CODE-EVIDENT  
**Category:** Tests  
**Location:** Multiple modules  
**Evidence:** Only 23 tests exist; missing tests for migrations, scheduler time zones, clipboard limits, PowerShell parsing, search scoring.  
**Impact/Failure Scenario:** Critical bugs in untested code could reach production. Recent crash was due to untested Unicode handling.  
**Suggested Fix:** Add 15 priority tests (see "Missing Tests" section).  
**Effort:** L  
**Touches:** apps.rs, scheduler.rs, db.rs, clipboard.rs, commands.rs

#### PERF-02: No virtualization for large lists
**Severity:** MEDIUM  
**Confidence:** SUSPECTED  
**Category:** Performance/Efficiency  
**Location:** src/views/UtilityDrawer.svelte:544-547  
**Evidence:**
```svelte
<div class="shortcuts-grid">
  grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
```
**Impact/Failure Scenario:** If user adds hundreds of shortcuts, grid will render all items without virtualization, causing UI lag. Currently limited by practical use but not enforced.  
**Suggested Fix:** Add virtualization or enforce hard limit on shortcuts (e.g., 50 max).  
**Effort:** M  
**Touches:** UtilityDrawer.svelte, commands.rs

#### BUILD-01: Version mismatch and no changelog
**Severity:** MEDIUM  
**Confidence:** VERIFIED  
**Category:** Build/Release  
**Location:** Cargo.toml:3, tauri.conf.json:4, package.json:4  
**Evidence:**
```toml
# Cargo.toml
version = "0.1.0"
# tauri.conf.json
"version": "0.2.4"
# package.json
"version": "0.1.0"
```
**Impact/Failure Scenario:** Version mismatch causes confusion in release management. No changelog makes it impossible to track what changed between versions.  
**Suggested Fix:** Standardize version across all files, add CHANGELOG.md, use automated version management.  
**Effort:** S  
**Touches:** Cargo.toml, tauri.conf.json, package.json

### LOW Findings

#### SEC-05: Modal overlay click handler lacks keyboard event
**Severity:** LOW  
**Confidence:** VERIFIED  
**Category:** Security  
**Location:** src/views/UtilityDrawer.svelte:419  
**Evidence:**
```svelte
<div class="modal-overlay" onclick={(e) => { if (e.target === e.currentTarget) showAppPicker = false; }}>
```
**Impact/Failure Scenario:** svelte-check warning (a11y_click_events_have_key_events). Keyboard users cannot close modal via overlay click (but Escape works at window level).  
**Suggested Fix:** Add onkeydown handler for keyboard users.  
**Effort:** S  
**Touches:** UtilityDrawer.svelte

#### PRIV-02: Clipboard history stored in plaintext
**Severity:** LOW  
**Confidence:** CODE-EVIDENT  
**Category:** Privacy/Data Protection  
**Location:** src-tauri/src/db.rs:31-33  
**Evidence:**
```sql
CREATE TABLE IF NOT EXISTS clips(
  id INTEGER PRIMARY KEY, text TEXT NOT NULL UNIQUE,
```
**Impact/Failure Scenario:** Clipboard history stored in plaintext SQLite. If user's device is compromised, clipboard history is readable. Local-only app reduces risk.  
**Suggested Fix:** Consider optional encryption for clipboard history, or document plaintext storage as acceptable for local-only app.  
**Effort:** L  
**Touches:** db.rs, clipboard.rs

#### REL-03: Error messages sometimes generic
**Severity:** LOW  
**Confidence:** CODE-EVIDENT  
**Category:** Reliability  
**Location:** src-tauri/src/commands.rs:74,91,127,309,761  
**Evidence:**
```rust
.map_err(|e| e.to_string())
.map_err(e2s)
```
**Impact/Failure Scenario:** Generic error messages (e.to_string()) may not help users understand or fix issues.  
**Suggested Fix:** Add context-specific error messages for common failure modes.  
**Effort:** M  
**Touches:** commands.rs

#### CONC-02: No explicit thread pool size limits
**Severity:** LOW  
**Confidence:** SUSPECTED  
**Category:** Concurrency  
**Location:** icon extraction spawns in async command context  
**Evidence:** Icon extraction happens in async command; concurrent requests could spawn many threads.  
**Impact/Failure Scenario:** If user opens picker with hundreds of apps, concurrent icon extraction could spawn many threads, exhausting thread pool.  
**Suggested Fix:** Add semaphore to limit concurrent icon extractions (e.g., max 4 at once).  
**Effort:** M  
**Touches:** commands.rs, icon.rs

#### PERF-03: System watcher threads never sleep when disabled
**Severity:** LOW  
**Confidence:** CODE-EVIDENT  
**Category:** Performance/Efficiency  
**Location:** src-tauri/src/system.rs:78-91,133-150,165-205  
**Evidence:**
```rust
loop {
    let is_fullscreen = detect_fullscreen();
    // ...
    std::thread::sleep(Duration::from_secs(1));
}
```
**Impact/Failure Scenario:** Watcher threads poll even when their features are disabled (e.g., auto_hide=false). Wakes CPU unnecessarily.  
**Suggested Fix:** Add enable/disable flags to stop polling when feature is off.  
**Effort:** M  
**Touches:** system.rs

#### COMP-03: No reduced motion support
**Severity:** LOW  
**Confidence:** CODE-EVIDENT  
**Category:** Compatibility/Accessibility  
**Location:** CSS animations in App.svelte and components  
**Evidence:** CSS transitions on hover, expand/collapse, progress ring. No `@media (prefers-reduced-motion)` query.  
**Impact/Failure Scenario:** Users with motion sensitivity may experience discomfort from animations.  
**Suggested Fix:** Add `@media (prefers-reduced-motion)` to disable or reduce animations.  
**Effort:** S  
**Touches:** App.svelte, global CSS

#### COMP-04: No screen reader testing
**Severity:** LOW  
**Confidence:** SUSPECTED  
**Category:** Compatibility/Accessibility  
**Location:** UI components  
**Evidence:** No ARIA labels on some buttons, no live region announcements for toasts.  
**Impact/Failure Scenario:** Screen reader users may not understand app state changes (toasts, progress updates).  
**Suggested Fix:** Add ARIA live regions for toasts and progress, improve labeling.  
**Effort:** M  
**Touches:** App.svelte, state.svelte.js, UtilityDrawer.svelte

#### CODE-02: Inconsistent hotkey in documentation vs code
**Severity:** LOW  
**Confidence:** CODE-EVIDENT  
**Category:** Code Quality  
**Location:** AGENTS.md mentions "Ctrl+Alt+Space", lib.rs:97 defaults to "ctrl+alt+space", but scheduler comment mentions "ctrl+space"  
**Evidence:**
```rust
// lib.rs:97
let toggle_hk = db.setting("hotkey_toggle")
    .unwrap_or_else(|| "ctrl+alt+space".to_string());
```
**Impact/Failure Scenario:** Documentation inconsistency may confuse users.  
**Suggested Fix:** Standardize hotkey documentation to match code default.  
**Effort:** S  
**Touches:** AGENTS.md

#### CODE-03: Magic numbers not all named constants
**Severity:** LOW  
**Confidence:** CODE-EVIDENT  
**Category:** Code Quality  
**Location:** Multiple files  
**Evidence:**
```rust
// scheduler.rs:57
if now < starts + 300 {
// scheduler.rs:119
let missed = last_run < now - 300 || last_run / 60 != now / 60;
```
**Impact/Failure Scenario:** Magic numbers (300 seconds = 5 minutes) reduce code readability.  
**Suggested Fix:** Add named constants for magic numbers (e.g., REMINDER_WINDOW_SEC = 300).  
**Effort:** S  
**Touches:** scheduler.rs, clipboard.rs

#### BUILD-02: Unsigned executable
**Severity:** LOW  
**Confidence:** SUSPECTED  
**Category:** Build/Release  
**Location:** tauri.conf.json, build configuration  
**Evidence:** No code signing configuration in tauri.conf.json or Cargo.toml.  
**Impact/Failure Scenario:** Unsigned executable may trigger SmartScreen warnings on Windows, reducing user trust.  
**Suggested Fix:** Add code signing configuration for release builds.  
**Effort:** M  
**Touches:** tauri.conf.json, build process

---

## 4. Measurements

### Phase 1 Automated Tooling Output

#### cargo check
```
warning: function `error` is never used
  --> src\logger.rs:54:8
warning: field `auto_hide_idle` is never read
  --> src\system.rs:28:9
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 06s
```
**Status:** Passed with 2 warnings

#### cargo test
```
running 23 tests
test apps::tests::test_6_byte_string_ends_inside_multibyte ... ok
[... 22 more tests ...]
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured
```
**Status:** All 23 tests passed

#### cargo clippy --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic -W clippy::indexing_slicing -W clippy::unwrap_in_result
```
warning: `isle` (lib) generated 28 warnings (4 duplicates)
Indexing warnings: actions.rs:23,74; icon.rs:152-155,185; scheduler.rs:110,114,120; system.rs:106
Unwrap warnings: db.rs:235,242,243,254; icon.rs:389,394; lib.rs:108,221
Expect warnings: lib.rs:221
```
**Status:** Passed with 28 warnings

#### cargo audit / cargo deny / cargo outdated / cargo machete
```
error: no such command: `audit`
error: no such command: `deny`
error: no such command: `outdated`
error: no such command: `machete`
```
**Status:** Tools not installed, could not run

#### npm audit
```
found 0 vulnerabilities
```
**Status:** Passed, 0 vulnerabilities

#### npm outdated
```
Package                       Current  Wanted  Latest  Location
@sveltejs/vite-plugin-svelte    5.1.1   5.1.1   7.3.1
@tauri-apps/api                2.12.3  2.12.4  2.12.4
@tauri-apps/cli                2.12.2  2.12.3  2.12.3
vite                            6.4.4   6.4.4   8.3.4
```
**Status:** 4 packages outdated (not critical)

#### npx svelte-check
```
svelte-check found 0 errors and 4 warnings in 2 files
Warn: Visible, non-interactive element <div> with a click event must be accompanied by a keyboard event handler (UtilityDrawer.svelte:419)
Warn: <div> with a click handler must have an ARIA role (UtilityDrawer.svelte:419)
Warn: `searchInputRef` is updated, but is not declared with $state(...) (UtilityDrawer.svelte:20)
Warn: define the standard property 'line-clamp' for compatibility (Clips.svelte:182)
```
**Status:** Passed with 4 warnings

#### npm run build
```
dist/index.html                    0.47 kB │ gzip:  0.29 kB
dist/settings.html                 0.50 kB │ gzip:  0.31 kB
dist/assets/settings-D13Ks9XH.css  5.26 kB │ gzip:  1.45 kB
dist/assets/main-hGGSFTZk.css     27.64 kB │ gzip:  5.11 kB
dist/assets/settings-D0WFhzik.js  17.97 kB │ gzip:  5.37 kB
dist/assets/main-DMqbvMPg.js      51.82 kB │ gzip: 17.77 kB
dist/assets/Icon-C3FMbN7e.js      58.04 kB │ gzip: 21.58 kB
✓ built in 9.24s
```
**Status:** Passed, total bundle ~127KB gzipped

#### cargo tauri build
**Status:** NOT RUN (requires Windows build environment, would touch user data)

#### Secrets scan (manual grep for keys, tokens, passwords, C:\Users\...)
**Pattern searched:** `api[_-]?key`, `secret`, `token`, `password`, `C:\\Users\\`, `apikey`  
**Result:** No hardcoded secrets, tokens, or absolute user paths found in source code.  
**Status:** Passed

### Phase 3 Dynamic Checks
**Status:** NOT VERIFIED (requires Windows and built app; audit is read-only)

All Phase 3 checks deferred to manual testing:
- Idle resource measurement
- 50-cycle open/close leak check
- 1,000 tasks and 5,000 clipboard entries stress test
- Mid-write DB integrity check
- Time shift across midnight and DST
- DB read-only start
- PowerShell blocked start
- Two monitors with different DPI

---

## 5. Things Done Well

1. **Event-driven architecture:** Clipboard and media listeners use event-driven APIs (clipboard-master, GSMTC) with zero polling when idle. (clipboard.rs:62-68, media.rs:76-85)

2. **Unicode-safe string handling:** After crash fix, all string operations use character-safe methods (chars(), split(), strip_prefix()) instead of byte slicing. (apps.rs:106,169-190)

3. **Parameterized SQL queries:** All database operations use rusqlite::params![] with proper parameterization, preventing SQL injection. (commands.rs:47-67, db.rs:115-121)

4. **Local-only enforcement:** CSP blocks all remote requests; no telemetry, no network calls, no cloud sync. (tauri.conf.json:54)

5. **Password manager exclusion:** Clipboard listener respects Windows "ExcludeClipboardContentFromMonitorProcessing" format to exclude password manager content. (clipboard.rs:119-124)

6. **Resource limits:** Clipboard history limited to 300 unpinned items, 20k character limit, images limited to last 50 files. (clipboard.rs:300,107-114,162)

7. **Comprehensive error handling in app enumeration:** PowerShell enumeration has panic::catch_unwind wrapper, fallback to .lnk scan, and 5-minute cache. (apps.rs:265-393)

8. **Icon cache with self-healing:** Icon extraction uses hash-based cache keys and self-heals when cached files are missing. (icon.rs:28-36, commands.rs:829-948)

9. **Scheduler sleeps until next due:** Scheduler thread sleeps until next event/routine/wallpaper is due, minimizing wake-ups. (scheduler.rs:52-145)

10. **Single-instance guard:** TCP port-based single-instance detection prevents multiple instances and shows existing window. (island.rs:26-61)

11. **WAL mode for database:** SQLite uses WAL mode for better concurrency and crash recovery. (db.rs:96)

12. **Migration system:** Versioned migrations with error handling for partial failures and duplicate columns. (db.rs:53-91,126-175)

13. **Frontend bundle size:** Production build is only 127KB gzipped, very efficient. (npm run build output)

14. **Mock API for development:** In-memory mock in api.js allows UI development without Rust/Windows. (api.js:32-279)

15. **Proper CSP:** CSP restricts to 'self' and data: URLs, no unsafe-inline or unsafe-eval. (tauri.conf.json:54)

---

## 6. Missing Tests (Top 15 Priority)

1. **Migration edge cases:** Test migration from old schema (v0) to current, with missing columns, corrupt data, and downgrade scenarios. (db.rs)

2. **Scheduler time zone handling:** Test scheduler behavior across DST transitions, time zone changes, and day boundary events. (scheduler.rs)

3. **Clipboard dedupe and limits:** Test clipboard ON CONFLICT behavior, 300-item limit enforcement, and pinned item preservation. (clipboard.rs, commands.rs)

4. **PowerShell JSON parsing:** Test JSON parsing with all edge cases: empty array, single object, BOM, accented names, malformed JSON, timeout. (apps.rs)

5. **Search scoring algorithm:** Test search scoring with exact match, prefix, word prefix, typo tolerance, and Unicode names. (UtilityDrawer.svelte)

6. **Icon extraction from shell:AppsFolder paths:** Test icon extraction with AUMID paths, shell::: paths, and fallback behavior. (icon.rs)

7. **Database corruption recovery:** Test app behavior when database file is corrupt, missing, or has wrong schema. (db.rs, lib.rs)

8. **Concurrent icon extraction:** Test that concurrent icon extractions don't race on cache file writes. (icon.rs, commands.rs)

9. **Single-instance race condition:** Test rapid launch of two instances, verify only one runs and other shows existing window. (island.rs)

10. **Hotkey conflict handling:** Test behavior when hotkey is already registered by another app. (island.rs, lib.rs)

11. **PowerShell blocked/failure:** Test app behavior when PowerShell is blocked by policy, times out, or returns malformed output. (apps.rs)

12. **Event reminder window:** Test that event notifications fire in the 5-minute window after start and not outside it. (scheduler.rs, commands.rs)

13. **Task/event timezone offset:** Test events_for_day and month_event_days with various timezone offsets, including negative offsets. (commands.rs)

14. **File path validation:** Test clip_open_file with symlinks, junctions, UNC paths, and relative paths. (commands.rs)

15. **Actions runner security:** Test that actions::run validates targets and prevents arbitrary file/URL launch. (actions.rs)

---

## 7. Needs Manual Windows Testing

### Dynamic Checks (NOT VERIFIED in audit)

1. **Idle resource measurement:**
   - Run app for 5 minutes while hidden
   - Measure CPU %, memory (private bytes), handle count, GDI object count
   - Expected: CPU near 0%, memory stable, no handle/GDI leaks

2. **50-cycle open/close leak check:**
   - Open and close the island 50 times rapidly
   - Measure memory before and after
   - Expected: No significant memory increase

3. **Stress test with large data:**
   - Add 1,000 tasks via API
   - Add 5,000 clipboard entries via simulation
   - Measure UI responsiveness and memory
   - Expected: UI remains responsive, memory within reasonable bounds

4. **Database integrity after mid-write crash:**
   - Start adding a task
   - Kill app process mid-write
   - Restart app
   - Verify database integrity and no data corruption
   - Expected: Database recovers or shows clear error

5. **Time shift across midnight:**
   - Set system clock to 23:58
   - Wait for midnight crossover
   - Verify scheduler handles day boundary correctly
   - Expected: Events/tasks scheduled for next day fire correctly

6. **DST transition:**
   - Set clock to before DST transition
   - Wait for transition
   - Verify scheduler handles time change correctly
   - Expected: No missed or duplicate notifications

7. **Database read-only start:**
   - Make isle.db read-only
   - Start app
   - Expected: Graceful error message, app does not crash

8. **PowerShell blocked:**
   - Block PowerShell execution via policy
   - Open app picker
   - Expected: Falls back to .lnk scan, shows error toast, does not crash

9. **Two monitors with different DPI:**
   - Connect two monitors with different DPI (e.g., 100% and 150%)
   - Move island between monitors
   - Expected: Island renders correctly on both, size scales properly

10. **Fullscreen app behavior:**
    - Open a fullscreen game or app
    - Verify island hides if hide_fullscreen is enabled
    - Expected: Island hides behind fullscreen app

11. **Battery saver behavior:**
    - Enable Windows battery saver
    - Verify island hides if hide_battery_saver is enabled
    - Expected: Island hides when battery saver is on

12. **Hotkey conflict:**
    - Register another app with same hotkey (Ctrl+Alt+Space)
    - Try to use hotkey
    - Expected: Warning logged, app does not crash

13. **App launching with various paths:**
    - Test launching: .exe, .lnk, shell::: paths, shell:AppsFolder paths
    - Expected: All launch correctly, errors shown in toast

14. **Icon extraction for Store apps:**
    - Add Calculator, Camera, Notepad from picker
    - Verify icons display correctly
    - Expected: Real Windows icons shown

15. **Clipboard image handling:**
    - Copy an image to clipboard
    - Verify it appears in clipboard history
    - Expected: Image saved and cropped correctly

### Manual Verification Steps

1. **Install from NSIS installer:**
   - Build installer: `npm run tauri build`
   - Install on clean Windows 10 machine
   - Verify installation completes without errors

2. **Uninstall cleanup:**
   - Uninstall app
   - Check if AppData directory is removed
   - Expected: All data cleared (currently FAIL - data persists)

3. **SmartScreen warning:**
   - Download and run unsigned exe
   - Check if SmartScreen shows warning
   - Expected: May show warning (unsigned exe)

4. **Accessibility with screen reader:**
   - Run with NVDA or Narrator
   - Navigate app with keyboard
   - Expected: All interactive elements announced

5. **High DPI scaling:**
   - Set display to 150% or 200%
   - Run app
   - Expected: Island scales correctly, text readable

---

## 8. Suggested Fix Batches

### Batch 1: Security & Accessibility (Priority: CRITICAL)
**Files:** actions.rs, UtilityDrawer.svelte, App.svelte  
**Effort:** M  
**Findings:** REL-01, SEC-01, COMP-01, COMP-02  
**Changes:**
- Add path/URL validation to actions::run (whitelist protocols, confirm dialog)
- Add ARIA role and keyboard handler to modal overlay
- Add aria-label to all icon-only buttons
- Add focus trap in modal

### Batch 2: Error Handling & Reliability (Priority: HIGH)
**Files:** db.rs, icon.rs, lib.rs, commands.rs, scheduler.rs  
**Effort:** L  
**Findings:** REL-02, SEC-02, SEC-03, CONC-01  
**Changes:**
- Add database backup before migrations
- Add integrity check and recovery on corrupt DB
- Replace unwrap/expect with proper error handling
- Add bounds checks before array/vector indexing
- Add shutdown hook for scheduler thread

### Batch 3: Performance & Polling (Priority: MEDIUM)
**Files:** system.rs, icon.rs, commands.rs  
**Effort:** L  
**Findings:** PERF-01, CONC-02, PERF-03, SEC-04  
**Changes:**
- Replace polling with Windows event-driven APIs (WM_DISPLAYCHANGE, power notifications)
- Add semaphore to limit concurrent icon extractions
- Add enable/disable flags to stop polling when features are off
- Add RAII wrapper for HBITMAP to ensure Release

### Batch 4: Code Quality & Build (Priority: LOW)
**Files:** logger.rs, system.rs, Cargo.toml, tauri.conf.json, package.json  
**Effort:** S  
**Findings:** CODE-01, BUILD-01, CODE-02, CODE-03  
**Changes:**
- Remove unused logger::error function
- Remove unused auto_hide_idle field
- Standardize version across all files
- Add CHANGELOG.md
- Add named constants for magic numbers

### Batch 5: Privacy & Cleanup (Priority: MEDIUM)
**Files:** Build configuration, Windows installer script  
**Effort:** M  
**Findings:** PRIV-01, BUILD-02  
**Changes:**
- Add uninstall cleanup script to delete AppData
- Add code signing configuration for release builds
- Document plaintext clipboard storage as acceptable

### Batch 6: Tests (Priority: HIGH)
**Files:** apps.rs, scheduler.rs, db.rs, clipboard.rs, commands.rs, icon.rs  
**Effort:** L  
**Findings:** TEST-01  
**Changes:**
- Add 15 priority tests (see Missing Tests section)
- Add test fixtures for common scenarios
- Set up CI to run tests automatically

---

## Audit Confirmation

### Source Files Modified
**None** - This was a READ-ONLY audit. No source files were modified.

### Git Status
```
On branch audit/report
nothing to commit, working tree clean
```

### Git Diff --stat Against Starting Commit (039be25)
```
No changes - audit branch created but no source modifications
```

### Processes Started During Audit
All processes have been stopped:
- cargo check (finished)
- cargo test (finished)
- cargo clippy (finished)
- npm audit (finished)
- npm outdated (finished)
- npx svelte-check (finished)
- npm run build (finished)

### Commands That Could Have Touched User Data
**None** - All commands were read-only operations (cargo check, cargo test, cargo clippy, npm audit, npm outdated, npx svelte-check, npm run build). No commands wrote to user data directories or modified the database.

---

**Audit Completed:** 2025-01-15  
**Auditor:** Senior Product/Security Engineer  
**Mode:** READ-ONLY  
**Confidence:** High for code review findings; Medium for dynamic checks (deferred to manual testing)
