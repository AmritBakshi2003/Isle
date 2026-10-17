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
    pub group: Option<String>,
    pub hidden: bool,
}

/// Folders to hide apps from unless explicitly wanted
const HIDDEN_FOLDERS: &[&str] = &[
    "Administrative Tools",
    "Windows Administrative Tools",
    "Accessibility",
    "System Tools",
    "Windows System",
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
        icon: Some("shell:::{645FF040-5081-101B-9F08-00AA002F954E}".to_string()),
        icon_filename: None,
        target: None,
        group: None,
        hidden: false,
    });

    apps.push(AppShortcut {
        id: None,
        name: "Control Panel".to_string(),
        path: "shell:::{21EC2020-3AEA-1069-A2DD-08002B30309D}".to_string(),
        icon: Some("shell:::{21EC2020-3AEA-1069-A2DD-08002B30309D}".to_string()),
        icon_filename: None,
        target: None,
        group: None,
        hidden: false,
    });

    // Filter unwanted entries
    apps = filter_apps(apps);

    // Dedupe by target path, then by name
    apps = dedupe_apps(apps);

    // Apply folder collapsing
    apps = collapse_folders(apps);

    // Sort alphabetically by name
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(apps)
}

fn scan_directory(entries: fs::ReadDir, apps: &mut Vec<AppShortcut>, group: Option<String>) {
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Get folder name for grouping
            let folder_name = path.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());

            // Recursively scan subdirectories
            if let Ok(sub_entries) = fs::read_dir(&path) {
                scan_directory(sub_entries, apps, folder_name);
            }
        } else if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("lnk")) {
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();
            let path_str = path.to_string_lossy().to_string();

            // TODO: Add proper .lnk resolution using lnk crate or Windows COM (IShellLinkW + IPersistFile)
            // For now, use .lnk path as target - this works for icon extraction with IShellItemImageFactory
            apps.push(AppShortcut {
                id: None,
                name,
                path: path_str.clone(),
                icon: Some(path_str.clone()),
                icon_filename: None,
                target: Some(path_str.clone()),
                group: group.clone(),
                hidden: false,
            });
        }
    }
}

/// Filter out unwanted apps
fn filter_apps(apps: Vec<AppShortcut>) -> Vec<AppShortcut> {
    apps.into_iter()
        .filter(|app| {
            // Skip unwanted name patterns
            let name_lower = app.name.to_lowercase();
            for pattern in UNWANTED_PATTERNS {
                if name_lower.contains(pattern) {
                    return false;
                }
            }

            // Skip if in hidden folder (unless shell item)
            if let Some(group) = &app.group {
                if HIDDEN_FOLDERS.iter().any(|f| f.eq_ignore_ascii_case(group)) {
                    return false;
                }
            }

            true
        })
        .collect()
}

/// Deduplicate by name (lowercase), preferring shorter path
fn dedupe_apps(mut apps: Vec<AppShortcut>) -> Vec<AppShortcut> {
    apps.sort_by(|a, b| {
        let name_cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
        if name_cmp == std::cmp::Ordering::Equal {
            a.path.len().cmp(&b.path.len())
        } else {
            name_cmp
        }
    });
    apps.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());

    apps
}

/// Collapse folder groups (keep one primary entry per folder)
/// Only hide siblings that share the first word with the primary entry or folder name
fn collapse_folders(mut apps: Vec<AppShortcut>) -> Vec<AppShortcut> {
    // Group by folder
    let mut groups: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();

    for (i, app) in apps.iter().enumerate() {
        let group_key = app.group.as_deref().unwrap_or("root").to_lowercase();
        groups.entry(group_key).or_default().push(i);
    }

    // For each folder with >1 entry, mark one as primary
    for (group_name, indices) in groups {
        if indices.len() > 1 && group_name != "root" {
            // Find primary: name equals or contains folder name, or shortest name
            let primary_idx = *indices.iter()
                .min_by_key(|&&i| {
                    let app = &apps[i];
                    let name_lower = app.name.to_lowercase();
                    let group_lower = group_name.to_lowercase();

                    if name_lower == group_lower {
                        return 0; // Exact match = highest priority
                    }
                    if name_lower.contains(&group_lower) {
                        return 1; // Contains = medium priority
                    }
                    2 // Otherwise, sort by name length
                })
                .unwrap_or(&indices[0]);

            let primary_app = &apps[primary_idx];
            let primary_first_word = primary_app.name.split_whitespace().next()
                .map(|w| w.to_lowercase())
                .unwrap_or_else(|| "".to_string());

            let folder_first_word = group_name.split_whitespace().next()
                .map(|w| w.to_lowercase())
                .unwrap_or_else(|| "".to_string());

            // Mark others as hidden ONLY if they share the first word
            for &idx in &indices {
                if idx != primary_idx {
                    let app = &apps[idx];
                    let app_first_word = app.name.split_whitespace().next()
                        .map(|w| w.to_lowercase())
                        .unwrap_or_else(|| "".to_string());

                    // Hide if first word matches primary OR folder
                    if app_first_word == primary_first_word || app_first_word == folder_first_word {
                        apps[idx].hidden = true;
                    }
                }
            }
        }
    }

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
                name: "Uninstall My App".to_string(),
                path: "C:\\test.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: Some("C:\\Program Files\\MyApp\\uninstall.exe".to_string()),
                group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "My App Help".to_string(),
                path: "C:\\test.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: Some("C:\\Program Files\\MyApp\\myapp.exe".to_string()),
                group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "My App".to_string(),
                path: "C:\\test.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: Some("C:\\Program Files\\MyApp\\myapp.exe".to_string()),
                group: None,
                hidden: false,
            },
        ];

        let filtered = filter_apps(apps);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "My App");
    }

    #[test]
    fn test_dedupe_by_name() {
        let apps = vec![
            AppShortcut {
                id: None,
                name: "My App".to_string(),
                path: "C:\\folder1\\app.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: Some("C:\\Program Files\\MyApp\\app.exe".to_string()),
                group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "My App".to_string(),
                path: "C:\\folder2\\app.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: Some("C:\\Program Files\\MyApp\\app.exe".to_string()),
                group: None,
                hidden: false,
            },
        ];

        let deduped = dedupe_apps(apps);
        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn test_folder_collapse_same_first_word() {
        let apps = vec![
            AppShortcut {
                id: None,
                name: "Git Bash".to_string(),
                path: "C:\\Git\\Git Bash.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: Some("C:\\Program Files\\Git\\bin\\bash.exe".to_string()),
                group: Some("Git".to_string()),
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Git CMD".to_string(),
                path: "C:\\Git\\Git CMD.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: Some("C:\\Program Files\\Git\\cmd\\git.exe".to_string()),
                group: Some("Git".to_string()),
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Git GUI".to_string(),
                path: "C:\\Git\\Git GUI.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: Some("C:\\Program Files\\Git\\cmd\\git-gui.exe".to_string()),
                group: Some("Git".to_string()),
                hidden: false,
            },
        ];

        let collapsed = collapse_folders(apps);
        assert_eq!(collapsed.len(), 3);
        // Git Bash should be primary (contains "Git" in name)
        let git_bash = collapsed.iter().find(|a| a.name == "Git Bash").unwrap();
        assert!(!git_bash.hidden);
        // Git CMD and Git GUI should be hidden (same first word "Git")
        let git_cmd = collapsed.iter().find(|a| a.name == "Git CMD").unwrap();
        assert!(git_cmd.hidden);
        let git_gui = collapsed.iter().find(|a| a.name == "Git GUI").unwrap();
        assert!(git_gui.hidden);
    }

    #[test]
    fn test_folder_collapse_different_first_words() {
        let apps = vec![
            AppShortcut {
                id: None,
                name: "Word".to_string(),
                path: "C:\\Office\\Word.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: Some("C:\\Program Files\\Office\\WINWORD.EXE".to_string()),
                group: Some("Microsoft Office".to_string()),
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Excel".to_string(),
                path: "C:\\Office\\Excel.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: Some("C:\\Program Files\\Office\\EXCEL.EXE".to_string()),
                group: Some("Microsoft Office".to_string()),
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "PowerPoint".to_string(),
                path: "C:\\Office\\PowerPoint.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: Some("C:\\Program Files\\Office\\POWERPNT.EXE".to_string()),
                group: Some("Microsoft Office".to_string()),
                hidden: false,
            },
        ];

        let collapsed = collapse_folders(apps);
        assert_eq!(collapsed.len(), 3);
        // All should be visible (different first words: Word, Excel, PowerPoint)
        for app in &collapsed {
            assert!(!app.hidden, "{} should not be hidden", app.name);
        }
    }
}

