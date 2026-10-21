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

use rusqlite::Connection;
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

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS settings(k TEXT PRIMARY KEY, v TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS clips(
  id INTEGER PRIMARY KEY, text TEXT NOT NULL UNIQUE,
  pinned INTEGER NOT NULL DEFAULT 0, ts INTEGER NOT NULL,
  file_path TEXT, file_name TEXT, file_type TEXT);
CREATE TABLE IF NOT EXISTS tasks(
  id INTEGER PRIMARY KEY, title TEXT NOT NULL, done INTEGER NOT NULL DEFAULT 0,
  due INTEGER, created INTEGER NOT NULL, toast_id TEXT, remind_min INTEGER DEFAULT 0,
  day TEXT, done_at INTEGER, notified INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS events(
  id INTEGER PRIMARY KEY, title TEXT NOT NULL, starts INTEGER NOT NULL, ends INTEGER,
  remind_min INTEGER NOT NULL DEFAULT 10, notified INTEGER NOT NULL DEFAULT 0, color TEXT);
CREATE INDEX IF NOT EXISTS events_starts ON events(starts);
CREATE TABLE IF NOT EXISTS modes(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, icon TEXT NOT NULL, actions TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS routines(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, trig TEXT NOT NULL, actions TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1, last_run INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS app_shortcuts(
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL UNIQUE, icon TEXT, icon_filename TEXT, target TEXT, folder_group TEXT, hidden INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS wellbeing(
  id INTEGER PRIMARY KEY, app TEXT NOT NULL, path TEXT, title TEXT,
  started INTEGER NOT NULL, ended INTEGER);
CREATE INDEX IF NOT EXISTS wellbeing_started ON wellbeing(started);
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
    // Note: Column now in base SCHEMA, this migration only for legacy databases
    ("9", "ALTER TABLE app_shortcuts ADD COLUMN icon TEXT"),
    // v10: Add target, folder_group, and hidden columns for app picker filtering
    // Note: Columns now in base SCHEMA, these migrations only for legacy databases
    ("10", "ALTER TABLE app_shortcuts ADD COLUMN target TEXT"),
    ("10", "ALTER TABLE app_shortcuts ADD COLUMN folder_group TEXT"),
    ("10", "ALTER TABLE app_shortcuts ADD COLUMN hidden INTEGER DEFAULT 0"),
    // v11: Add day and done_at columns to tasks, and color column to events for unified TaskCalendar
    // Backfill day from due or created, and done_at from created for done tasks
    ("11", "ALTER TABLE tasks ADD COLUMN day TEXT"),
    ("11", "ALTER TABLE tasks ADD COLUMN done_at INTEGER"),
    ("11", "ALTER TABLE tasks ADD COLUMN notified INTEGER DEFAULT 0"),
    ("11", "UPDATE tasks SET day = CASE WHEN due IS NOT NULL THEN date(due, 'unixepoch', 'localtime') ELSE date(created, 'unixepoch', 'localtime') END WHERE day IS NULL"),
    ("11", "UPDATE tasks SET done_at = created WHERE done = 1 AND done_at IS NULL"),
    ("11", "ALTER TABLE events ADD COLUMN color TEXT"),
    // v12: wellbeing (digital wellbeing log) is now in the base SCHEMA above
    // (the index statement keeps legacy databases in sync).
    ("12", "CREATE INDEX IF NOT EXISTS wellbeing_started ON wellbeing(started)"),
];

impl Db {
    pub fn open(path: &std::path::Path) -> rusqlite::Result<Self> {
        // Try to open the database
        let c = match Connection::open(path) {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("Failed to open database at {:?}: {}", path, e);
                // Attempt recovery by creating a new database
                return Self::open_with_recovery(path);
            }
        };

        // Set up journal mode
        if let Err(e) = c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;") {
            eprintln!("Failed to set journal mode: {}", e);
        }

        // Run integrity check
        if let Err(e) = Self::check_integrity(&c) {
            eprintln!("Database integrity check failed: {}", e);
            // Attempt recovery by backing up and recreating
            return Self::open_with_recovery(path);
        }

        // Apply schema and migrations
        if let Err(e) = c.execute_batch(SCHEMA) {
            eprintln!("Failed to apply schema: {}", e);
            return Self::open_with_recovery(path);
        }

        if let Err(e) = run_migrations(&c) {
            eprintln!("Failed to run migrations: {}", e);
            return Self::open_with_recovery(path);
        }

        Ok(Db(Mutex::new(c)))
    }

    /// Check database integrity using PRAGMA integrity_check
    fn check_integrity(c: &Connection) -> rusqlite::Result<()> {
        let mut stmt = c.prepare("PRAGMA integrity_check")?;
        let result: String = stmt.query_row([], |r| r.get(0))?;
        if result != "ok" {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
                Some(result),
            ));
        }
        Ok(())
    }

    /// Attempt to recover from corrupt database by backing up and recreating
    fn open_with_recovery(path: &std::path::Path) -> rusqlite::Result<Self> {
        eprintln!("Attempting database recovery for {:?}", path);
        
        // Create backup of corrupt database
        let backup_path = path.with_extension("db.corrupt");
        if path.exists() {
            if let Err(e) = std::fs::copy(path, &backup_path) {
                eprintln!("Failed to create backup: {}", e);
            } else {
                eprintln!("Corrupt database backed up to {:?}", backup_path);
            }
        }

        // Remove corrupt database
        if path.exists() {
            if let Err(e) = std::fs::remove_file(path) {
                eprintln!("Failed to remove corrupt database: {}", e);
            }
        }

        // Remove WAL files if they exist
        let wal_path = path.with_extension("db-wal");
        let shm_path = path.with_extension("db-shm");
        let _ = std::fs::remove_file(&wal_path);
        let _ = std::fs::remove_file(&shm_path);

        // Create fresh database
        let c = Connection::open(path)?;
        c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        c.execute_batch(SCHEMA)?;
        run_migrations(&c)?;
        
        eprintln!("Database recovery complete - fresh database created");
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
    
    // Special case: Check if app_shortcuts table exists and is missing columns (old schema)
    // This handles databases created before v9/v10 migrations
    let table_exists: bool = c.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_shortcuts'",
        [],
        |r| r.get::<_, i64>(0)
    ).map(|count| count > 0).unwrap_or(false);
    
    if table_exists {
        // Check for missing columns using PRAGMA table_info
        let check_column = |col_name: &str| -> bool {
            c.prepare(&format!("SELECT COUNT(*) FROM pragma_table_info('app_shortcuts') WHERE name='{}'", col_name))
                .and_then(|mut stmt| stmt.query_row([], |r| r.get::<_, i64>(0)))
                .map(|count| count == 0)
                .unwrap_or(false)
        };
        
        if check_column("icon") {
            let _ = c.execute("ALTER TABLE app_shortcuts ADD COLUMN icon TEXT", []);
            eprintln!("Migration: Added 'icon' column to app_shortcuts");
        }
        if check_column("target") {
            let _ = c.execute("ALTER TABLE app_shortcuts ADD COLUMN target TEXT", []);
            eprintln!("Migration: Added 'target' column to app_shortcuts");
        }
        if check_column("folder_group") {
            let _ = c.execute("ALTER TABLE app_shortcuts ADD COLUMN folder_group TEXT", []);
            eprintln!("Migration: Added 'folder_group' column to app_shortcuts");
        }
        if check_column("hidden") {
            let _ = c.execute("ALTER TABLE app_shortcuts ADD COLUMN hidden INTEGER DEFAULT 0", []);
            eprintln!("Migration: Added 'hidden' column to app_shortcuts");
        }
    }
    
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    
    #[test]
    fn test_migration_adds_missing_columns() {
        // Create an in-memory database with old 4-column schema
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("
            CREATE TABLE app_shortcuts(
              id INTEGER PRIMARY KEY, 
              name TEXT NOT NULL, 
              path TEXT NOT NULL UNIQUE, 
              icon_filename TEXT
            );
        ").unwrap();
        
        // Run migrations
        run_migrations(&c).unwrap();
        
        // Check that new columns exist
        let check_column = |col_name: &str| -> bool {
            c.prepare(&format!("SELECT COUNT(*) FROM pragma_table_info('app_shortcuts') WHERE name='{}'", col_name))
                .and_then(|mut stmt| stmt.query_row([], |r| r.get::<_, i64>(0)))
                .map(|count| count > 0)
                .unwrap_or(false)
        };
        
        assert!(check_column("icon"), "icon column should exist");
        assert!(check_column("target"), "target column should exist");
        assert!(check_column("folder_group"), "folder_group column should exist");
        assert!(check_column("hidden"), "hidden column should exist");
        
        // Test insert with all columns
        c.execute(
            "INSERT INTO app_shortcuts(name,path,icon,icon_filename,target,folder_group,hidden) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params!["Test App", "C:\\test.exe", "C:\\test.exe", "icon_v3_123.png", "C:\\test.exe", "Test Group", 0],
        ).unwrap();
        
        // Test select
        let name: String = c.query_row("SELECT name FROM app_shortcuts WHERE name=?1", ["Test App"], |r| r.get(0)).unwrap();
        assert_eq!(name, "Test App");
    }
    
    #[test]
    fn test_migration_with_icon_column_already_present() {
        // Create an in-memory database with schema that has icon but not the other columns
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("
            CREATE TABLE app_shortcuts(
              id INTEGER PRIMARY KEY, 
              name TEXT NOT NULL, 
              path TEXT NOT NULL UNIQUE, 
              icon TEXT,
              icon_filename TEXT
            );
        ").unwrap();
        
        // Run migrations
        run_migrations(&c).unwrap();
        
        // Check that missing columns were added
        let check_column = |col_name: &str| -> bool {
            c.prepare(&format!("SELECT COUNT(*) FROM pragma_table_info('app_shortcuts') WHERE name='{}'", col_name))
                .and_then(|mut stmt| stmt.query_row([], |r| r.get::<_, i64>(0)))
                .map(|count| count > 0)
                .unwrap_or(false)
        };
        
        assert!(check_column("icon"), "icon column should still exist");
        assert!(check_column("target"), "target column should exist");
        assert!(check_column("folder_group"), "folder_group column should exist");
        assert!(check_column("hidden"), "hidden column should exist");
    }
}
