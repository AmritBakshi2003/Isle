//! Windows app enumeration and shortcut management
//! Scans Start Menu for .lnk files and returns them as shortcuts

use std::fs;
use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AppShortcut {
    pub id: Option<i64>,
    pub name: String,
    pub path: String,
    pub icon: Option<String>,
    pub icon_filename: Option<String>,
}

/// Scan Windows Start Menu for installed applications
pub fn enumerate_apps() -> Result<Vec<AppShortcut>, String> {
    let mut apps = Vec::new();

    // Get Start Menu paths using environment variables
    let user_start_menu = std::env::var("APPDATA")
        .map(|p| PathBuf::from(p).join("Microsoft\\Windows\\Start Menu\\Programs"))
        .unwrap_or_else(|_| PathBuf::from("C:\\Users\\Default\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs"));

    let common_start_menu = PathBuf::from("C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs");

    // Scan both directories for .lnk files
    if let Ok(entries) = fs::read_dir(&user_start_menu) {
        scan_directory(entries, &mut apps);
    }
    if let Ok(entries) = fs::read_dir(&common_start_menu) {
        scan_directory(entries, &mut apps);
    }

    // Deduplicate by path
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps.dedup_by(|a, b| a.path == b.path);

    Ok(apps)
}

fn scan_directory(entries: fs::ReadDir, apps: &mut Vec<AppShortcut>) {
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Recursively scan subdirectories
            if let Ok(sub_entries) = fs::read_dir(&path) {
                scan_directory(sub_entries, apps);
            }
        } else if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("lnk")) {
            // Use the .lnk file path directly
            // Windows SHGetFileInfoW can extract icons from .lnk files natively
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();
            let path_str = path.to_string_lossy().to_string();
            apps.push(AppShortcut {
                id: None,
                name,
                path: path_str.clone(),
                icon: Some(path_str), // Use .lnk path for icon extraction (Windows handles it)
                icon_filename: None,
            });
        }
    }
}
