//! SQLite storage. Everything the user creates lives in ONE local file:
//! `%APPDATA%\app.isle.island\isle.db`. Nothing leaves the machine.
//!
//! Conventions
//!  - All timestamps are unix SECONDS (i64), UTC. The UI converts to local time.
//!  - Column names avoid SQL keywords (`trig`, `starts`, `ends`, `k`, `v`).
//!  - `modes.actions` / `routines.actions` / `routines.trig` are JSON text. See
//!    actions.rs for the action schema.
//!  - Access the connection only through `Db::with` so locking stays in one place.
//!    NEVER call `Db::with` while already inside another `Db::with` (deadlock).

use rusqlite::{params, Connection};
use std::sync::Mutex;

pub struct Db(pub Mutex<Connection>);

pub fn e2s<E: ToString>(e: E) -> String {
    e.to_string()
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS settings(k TEXT PRIMARY KEY, v TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS clips(
  id INTEGER PRIMARY KEY, text TEXT NOT NULL UNIQUE,
  pinned INTEGER NOT NULL DEFAULT 0, ts INTEGER NOT NULL,
  file_path TEXT, file_name TEXT, file_type TEXT);
CREATE TABLE IF NOT EXISTS tasks(
  id INTEGER PRIMARY KEY, title TEXT NOT NULL, done INTEGER NOT NULL DEFAULT 0,
  due INTEGER, created INTEGER NOT NULL, toast_id TEXT, remind_min INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS events(
  id INTEGER PRIMARY KEY, title TEXT NOT NULL, starts INTEGER NOT NULL, ends INTEGER,
  remind_min INTEGER NOT NULL DEFAULT 10, notified INTEGER NOT NULL DEFAULT 0);
CREATE INDEX IF NOT EXISTS events_starts ON events(starts);
CREATE TABLE IF NOT EXISTS modes(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, icon TEXT NOT NULL, actions TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS routines(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, trig TEXT NOT NULL, actions TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1, last_run INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS app_shortcuts(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL UNIQUE, icon_filename TEXT);
";

/// Migration steps to evolve the schema without breaking existing databases.
/// Each migration is run once; the version is stored in the settings table.
const MIGRATIONS: &[(&str, &str)] = &[
    // v1: Add toast_id and remind_min columns to tasks table for scheduled reminders
    // Use ALTER TABLE IF NOT EXISTS to handle cases where columns might already exist
    ("1", "ALTER TABLE tasks ADD COLUMN toast_id TEXT"),
    ("1", "ALTER TABLE tasks ADD COLUMN remind_min INTEGER DEFAULT 0"),
    // v2: Fix transparent bg_color values in settings
    ("2", "UPDATE settings SET v='rgba(26, 26, 26, 0.95)' WHERE k='bg_color' AND v LIKE '%, 0)'"),
    // v3: Add file columns to clips table for drag-drop file support
    ("3", "ALTER TABLE clips ADD COLUMN file_path TEXT"),
    ("3", "ALTER TABLE clips ADD COLUMN file_name TEXT"),
    ("3", "ALTER TABLE clips ADD COLUMN file_type TEXT"),
    // v4: Add start_time and end_time columns to tasks for unified timeline view
    ("4", "ALTER TABLE tasks ADD COLUMN start_time INTEGER"),
    ("4", "ALTER TABLE tasks ADD COLUMN end_time INTEGER"),
    // v5: Add app_shortcuts table for custom app shortcuts
    ("5", "CREATE TABLE IF NOT EXISTS app_shortcuts(id INTEGER PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL UNIQUE)"),
    // v6: Add icon_filename column to app_shortcuts
    ("6", "ALTER TABLE app_shortcuts ADD COLUMN icon_filename TEXT"),
    // v7: Clear icon filenames to force regeneration with new color scheme
    ("7", "UPDATE app_shortcuts SET icon_filename = NULL"),
    // v8: Clear icon filenames again to force regeneration with native Windows icon extraction
    ("8", "UPDATE app_shortcuts SET icon_filename = NULL"),
    // v9: Add icon column to store resolved target path for icon extraction
    ("9", "ALTER TABLE app_shortcuts ADD COLUMN icon TEXT"),
];

impl Db {
    pub fn open(path: &std::path::Path) -> rusqlite::Result<Self> {
        let c = Connection::open(path)?;
        c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        c.execute_batch(SCHEMA)?;
        run_migrations(&c)?;
        seed(&c)?;
        Ok(Db(Mutex::new(c)))
    }

    /// Run a closure with the connection locked. Errors become Strings for the UI.
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, String> {
        let c = self.0.lock().map_err(e2s)?;
        f(&c).map_err(e2s)
    }

    pub fn setting(&self, k: &str) -> Option<String> {
        self.with(|c| c.query_row("SELECT v FROM settings WHERE k=?1", [k], |r| r.get(0)))
            .ok()
    }

    pub fn set_setting(&self, k: &str, v: &str) -> Result<(), String> {
        self.with(|c| {
            c.execute(
                "INSERT INTO settings(k,v) VALUES(?1,?2) ON CONFLICT(k) DO UPDATE SET v=excluded.v",
                [k, v],
            )
            .map(|_| ())
        })
    }
}

/// Run pending migrations. Each migration is run once; the version is stored in settings.
fn run_migrations(c: &Connection) -> rusqlite::Result<()> {
    c.execute("CREATE TABLE IF NOT EXISTS schema_version(version INTEGER PRIMARY KEY)", [])?;
    let current_version: i64 = c.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))?;
    
    for (version, sql) in MIGRATIONS {
        let v: i64 = version.parse().unwrap();
        if v > current_version {
            // Try to execute migration; ignore errors if column already exists
            let _ = c.execute(sql, []);
            // Use INSERT OR IGNORE to handle duplicate version numbers
            c.execute("INSERT OR IGNORE INTO schema_version(version) VALUES(?1)", [v])?;
        }
    }
    Ok(())
}

/// First-run defaults so the Modes tab is not empty.
fn seed(c: &Connection) -> rusqlite::Result<()> {
    let n: i64 = c.query_row("SELECT COUNT(*) FROM modes", [], |r| r.get(0))?;
    if n == 0 {
        for (name, icon, actions) in [
            ("Focus", "🎯", r#"[{"type":"notify","title":"Focus on","body":"One thing at a time."}]"#),
            ("Break", "☕", r#"[{"type":"notify","title":"Break time","body":"Step away from the screen."}]"#),
        ] {
            c.execute(
                "INSERT INTO modes(name,icon,actions) VALUES(?1,?2,?3)",
                params![name, icon, actions],
            )?;
        }
    }
    Ok(())
}
