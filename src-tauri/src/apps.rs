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
    pub target: Option<String>,
    #[serde(rename = "group")]
    pub folder_group: Option<String>,
    pub hidden: bool,
}

/// Folders to hide apps from unless explicitly wanted
const HIDDEN_FOLDERS: &[&str] = &[
    "Administrative Tools",
    "Windows Administrative Tools",
    "Accessibility",
];

/// Unwanted name patterns
const UNWANTED_PATTERNS: &[&str] = &[
    "uninstall",
    "help",
    "readme",
    "documentation",
    "manual",
    "license",
    "release notes",
    "website",
    "web site",
    "debug",
    "changelog",
    "visit ",
    "what's new",
    "about ",
];

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
        scan_directory(entries, &mut apps, None);
    }
    if let Ok(entries) = fs::read_dir(&common_start_menu) {
        scan_directory(entries, &mut apps, None);
    }

    // Add built-in shell items
    apps.push(AppShortcut {
        id: None,
        name: "Recycle Bin".to_string(),
        path: "shell:::{645FF040-5081-101B-9F08-00AA002F954E}".to_string(),
        icon: None,
        icon_filename: None,
        target: None,
        folder_group: None,
        hidden: false,
    });

    apps.push(AppShortcut {
        id: None,
        name: "Control Panel".to_string(),
        path: "shell:::{21EC2020-3AEA-1069-A2DD-08002B30309D}".to_string(),
        icon: None,
        icon_filename: None,
        target: None,
        folder_group: None,
        hidden: false,
    });

    Ok(filter_and_dedupe_apps(apps))
}

/// Recursively scan a directory for .lnk files
fn scan_directory(entries: fs::ReadDir, apps: &mut Vec<AppShortcut>, group: Option<String>) {
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();

            if path.is_dir() {
                // Recurse into subdirectories, using folder name as group
                if let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(sub_entries) = fs::read_dir(&path) {
                        scan_directory(sub_entries, apps, Some(folder_name.to_string()));
                    }
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("lnk") {
                // Add .lnk file as an app shortcut
                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                    apps.push(AppShortcut {
                        id: None,
                        name: name.to_string(),
                        path: path.to_string_lossy().to_string(),
                        icon: None,
                        icon_filename: None,
                        target: Some(path.to_string_lossy().to_string()), // TODO: resolve .lnk target via lnk-crate or COM
                        folder_group: group.clone(),
                        hidden: false,
                    });
                }
            }
        }
    }
}

/// Filter out unwanted apps and deduplicate by name
fn filter_and_dedupe_apps(mut apps: Vec<AppShortcut>) -> Vec<AppShortcut> {
    // Filter by name patterns and folder
    apps.retain(|app| {
        // Skip unwanted name patterns
        for pattern in UNWANTED_PATTERNS {
            if app.name.to_lowercase().contains(&pattern.to_lowercase()) {
                return false;
            }
        }

        // Skip apps in hidden folders
        if let Some(folder) = &app.folder_group {
            for hidden_folder in HIDDEN_FOLDERS {
                if folder.to_lowercase() == hidden_folder.to_lowercase() {
                    return false;
                }
            }
        }

        true
    });

    // Deduplicate by name (case-insensitive)
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());

    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_unwanted_patterns() {
        let apps = vec![
            AppShortcut {
                id: None,
                name: "Uninstall MyApp".to_string(),
                path: "C:\\test\\uninstall.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "MyApp Help".to_string(),
                path: "C:\\test\\help.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "MyApp".to_string(),
                path: "C:\\test\\app.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
        ];

        let filtered = filter_and_dedupe_apps(apps);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "MyApp");
    }

    #[test]
    fn test_dedupe_by_name() {
        let apps = vec![
            AppShortcut {
                id: None,
                name: "MyApp".to_string(),
                path: "C:\\test\\app.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "MYAPP".to_string(),
                path: "C:\\test2\\app.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
        ];

        let filtered = filter_and_dedupe_apps(apps);
        assert_eq!(filtered.len(), 1);
    }
}
