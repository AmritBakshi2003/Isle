//! Windows app enumeration using PowerShell Get-StartApps
//! Returns a single source of truth for all installed applications

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

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

/// PowerShell Get-StartApps output structure
#[derive(serde::Deserialize)]
#[allow(non_snake_case)]
struct StartApp {
    Name: String,
    AppID: String,
}

/// Cache for app enumeration results
struct AppCache {
    apps: Vec<AppShortcut>,
    timestamp: Instant,
}

static APP_CACHE: OnceLock<Mutex<AppCache>> = OnceLock::new();

/// Get cached apps if fresh (within 5 minutes), or None if stale/missing
fn get_cached_apps() -> Option<Vec<AppShortcut>> {
    let cache = APP_CACHE.get()?;
    let guard = cache.lock().ok()?;
    let elapsed = guard.timestamp.elapsed();
    
    if elapsed.as_secs() < 300 {
        Some(guard.apps.clone())
    } else {
        None
    }
}

/// Update the cache with new app list
fn update_cache(apps: Vec<AppShortcut>) {
    let cache = APP_CACHE.get_or_init(|| {
        Mutex::new(AppCache {
            apps: Vec::new(),
            timestamp: Instant::now(),
        })
    });
    
    if let Ok(mut guard) = cache.lock() {
        guard.apps = apps;
        guard.timestamp = Instant::now();
    }
}

/// Run PowerShell and return its output, or an error string
fn run_powershell_get_start_apps() -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    let output = Command::new("powershell.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8; @(Get-StartApps | Select-Object Name,AppID) | ConvertTo-Json -Compress"
        ])
        .output()
        .map_err(|e| format!("Failed to spawn PowerShell: {}", e))?;
    
    if !output.status.success() {
        return Err(format!("PowerShell exited with error: {:?}", output.status));
    }
    
    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| format!("PowerShell output was not valid UTF-8: {}", e))?;
    
    Ok(stdout)
}

/// Parse PowerShell JSON output, handling BOM, array, single object, and empty cases
fn parse_start_apps_json(json: &str) -> Result<Vec<StartApp>, String> {
    let json = json.trim();
    
    // Handle empty output
    if json.is_empty() || json == "[]" {
        return Ok(Vec::new());
    }
    
    // Strip UTF-8 BOM if present (character-safe)
    let json = json.strip_prefix('\u{FEFF}').unwrap_or(json);
    
    // Try parsing as array first
    if let Ok(apps) = serde_json::from_str::<Vec<StartApp>>(json) {
        return Ok(apps);
    }
    
    // Try parsing as single object (PowerShell returns object when there's only one item)
    if let Ok(app) = serde_json::from_str::<StartApp>(json) {
        return Ok(vec![app]);
    }
    
    Err(format!("Failed to parse JSON: {}", json))
}

/// Scan Start Menu for .lnk files (fallback when PowerShell fails)
fn fallback_scan_start_menu() -> Vec<AppShortcut> {
    let mut apps = Vec::new();
    
    let user_start_menu = std::env::var("APPDATA")
        .map(|p| PathBuf::from(p).join("Microsoft\\Windows\\Start Menu\\Programs"))
        .unwrap_or_else(|_| PathBuf::from("C:\\Users\\Default\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs"));
    
    let common_start_menu = PathBuf::from("C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs");
    
    if let Ok(entries) = fs::read_dir(&user_start_menu) {
        scan_directory_lnly(entries, &mut apps);
    }
    if let Ok(entries) = fs::read_dir(&common_start_menu) {
        scan_directory_lnly(entries, &mut apps);
    }
    
    apps
}

/// Recursively scan a directory for .lnk files (fallback only)
fn scan_directory_lnly(entries: fs::ReadDir, apps: &mut Vec<AppShortcut>) {
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            
            if path.is_dir() {
                if let Ok(sub_entries) = fs::read_dir(&path) {
                    scan_directory_lnly(sub_entries, apps);
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("lnk") {
                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                    apps.push(AppShortcut {
                        id: None,
                        name: name.to_string(),
                        path: path.to_string_lossy().to_string(),
                        icon: Some(path.to_string_lossy().to_string()),
                        icon_filename: None,
                        target: Some(path.to_string_lossy().to_string()),
                        folder_group: None,
                        hidden: false,
                    });
                }
            }
        }
    }
}

/// Check if a pattern matches as a whole word using character-safe splitting
fn matches_whole_word(text: &str, pattern: &str) -> bool {
    let text_lower = text.to_lowercase();
    let pattern_lower = pattern.to_lowercase();
    
    // Exact match
    if text_lower == pattern_lower {
        return true;
    }
    
    // Split text into words by non-alphanumeric characters
    let words: Vec<&str> = text_lower.split(|c: char| !c.is_alphanumeric()).collect();
    
    // Check if pattern matches any whole word
    for word in words {
        if word == pattern_lower {
            return true;
        }
    }
    
    false
}

/// Filter out unwanted apps by name patterns
fn filter_by_name_patterns(apps: Vec<AppShortcut>) -> Vec<AppShortcut> {
    // Patterns that require whole-word matching
    const WHOLE_WORD_PATTERNS: &[&str] = &["help", "manual", "license"];
    
    // Patterns that allow substring matching
    const SUBSTRING_PATTERNS: &[&str] = &[
        "uninstall",
        "readme",
        "documentation",
        "release notes",
        "website",
        "web site",
        "debug",
        "changelog",
        "visit ",
        "what's new",
        "about ",
    ];
    
    apps.into_iter()
        .filter(|app| {
            let name_lower = app.name.to_lowercase();
            
            // Check whole-word patterns
            for pattern in WHOLE_WORD_PATTERNS {
                if matches_whole_word(&app.name, pattern) {
                    return false;
                }
            }
            
            // Check substring patterns
            for pattern in SUBSTRING_PATTERNS {
                if name_lower.contains(&pattern.to_lowercase()) {
                    return false;
                }
            }
            
            true
        })
        .collect()
}

/// Deduplicate by AppID (extracted from shell:AppsFolder paths)
fn dedupe_by_appid(mut apps: Vec<AppShortcut>) -> Vec<AppShortcut> {
    apps.sort_by(|a, b| {
        // Extract AppID for comparison if present in shell:AppsFolder path
        let a_id = extract_appid_from_path(&a.path);
        let b_id = extract_appid_from_path(&b.path);
        
        match (a_id, b_id) {
            (Some(id_a), Some(id_b)) => id_a.cmp(&id_b),
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    
    apps.dedup_by(|a, b| {
        match (extract_appid_from_path(&a.path), extract_appid_from_path(&b.path)) {
            (Some(id_a), Some(id_b)) => id_a.to_lowercase() == id_b.to_lowercase(),
            _ => false,
        }
    });
    
    apps
}

/// Extract AppID from shell:AppsFolder\... path
fn extract_appid_from_path(path: &str) -> Option<String> {
    path.strip_prefix("shell:AppsFolder\\")
        .map(|s| s.to_string())
}

/// Scan Windows Start Menu for installed applications
pub fn enumerate_apps() -> Result<Vec<AppShortcut>, String> {
    use std::panic;
    
    // Check cache first
    if let Some(cached) = get_cached_apps() {
        return Ok(cached);
    }
    
    // Wrap the entire enumeration in catch_unwind to prevent panics from crashing the app
    let result = panic::catch_unwind(|| {
        let mut apps = Vec::new();
        
        // Try PowerShell enumeration
        let ps_result = run_powershell_get_start_apps();
        
        match ps_result {
            Ok(json_output) => {
                match parse_start_apps_json(&json_output) {
                    Ok(start_apps) => {
                        // Convert StartApp to AppShortcut
                        for start_app in start_apps {
                            let name = start_app.Name.trim();
                            let appid = start_app.AppID.trim();
                            
                            // Skip empty entries
                            if name.is_empty() || appid.is_empty() {
                                continue;
                            }
                            
                            apps.push(AppShortcut {
                                id: None,
                                name: name.to_string(),
                                path: format!("shell:AppsFolder\\{}", appid),
                                icon: Some(format!("shell:AppsFolder\\{}", appid)),
                                icon_filename: None,
                                target: None,
                                folder_group: None,
                                hidden: false,
                            });
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to parse PowerShell JSON: {}, falling back to .lnk scan", e);
                        apps = fallback_scan_start_menu();
                    }
                }
            }
            Err(e) => {
                eprintln!("PowerShell failed: {}, falling back to .lnk scan", e);
                apps = fallback_scan_start_menu();
            }
        }
        
        // Add built-in shell items (Recycle Bin, This PC)
        let recycle_bin = AppShortcut {
            id: None,
            name: "Recycle Bin".to_string(),
            path: "shell:::{645FF040-5081-101B-9F08-00AA002F954E}".to_string(),
            icon: None,
            icon_filename: None,
            target: None,
            folder_group: None,
            hidden: false,
        };
        
        let this_pc = AppShortcut {
            id: None,
            name: "This PC".to_string(),
            path: "shell:::{20D04FE0-3AEA-1069-A2DD-08002B30309D}".to_string(),
            icon: None,
            icon_filename: None,
            target: None,
            folder_group: None,
            hidden: false,
        };
        
        // Add them if they don't already exist
        let has_recycle_bin = apps.iter().any(|a| a.name.to_lowercase() == "recycle bin");
        if !has_recycle_bin {
            apps.push(recycle_bin);
        }
        
        let has_this_pc = apps.iter().any(|a| a.name.to_lowercase() == "this pc");
        if !has_this_pc {
            apps.push(this_pc);
        }
        
        // Check if Control Panel exists in the list
        let has_control_panel = apps.iter().any(|a| a.name.to_lowercase() == "control panel");
        if !has_control_panel {
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
        }
        
        // Filter by name patterns
        apps = filter_by_name_patterns(apps);
        
        // Deduplicate by AppID
        apps = dedupe_by_appid(apps);
        
        // Sort by name (case-insensitive)
        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        
        apps
    });
    
    match result {
        Ok(apps) => {
            // Update cache
            update_cache(apps.clone());
            Ok(apps)
        }
        Err(_) => {
            // On panic, fall back to .lnk scan with a clear error message
            eprintln!("Panic during app enumeration, falling back to .lnk scan");
            let apps = fallback_scan_start_menu();
            update_cache(apps.clone());
            Ok(apps)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_json_array() {
        let json = r#"[{"Name":"Calculator","AppID":"Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"},{"Name":"Camera","AppID":"Microsoft.WindowsCamera_8wekyb3d8bbwe!App"}]"#;
        let result = parse_start_apps_json(json).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].Name, "Calculator");
        assert_eq!(result[1].Name, "Camera");
    }
    
    #[test]
    fn test_parse_json_single_object() {
        let json = r#"{"Name":"Calculator","AppID":"Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"}"#;
        let result = parse_start_apps_json(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].Name, "Calculator");
    }
    
    #[test]
    fn test_parse_json_empty_array() {
        let json = r#"[]"#;
        let result = parse_start_apps_json(json).unwrap();
        assert_eq!(result.len(), 0);
    }
    
    #[test]
    fn test_parse_json_empty_string() {
        let json = "";
        let result = parse_start_apps_json(json).unwrap();
        assert_eq!(result.len(), 0);
    }
    
    #[test]
    fn test_parse_json_with_bom() {
        let json = "\u{FEFF}[{\"Name\":\"Calculator\",\"AppID\":\"Microsoft.WindowsCalculator_8wekyb3d8bbwe!App\"}]";
        let result = parse_start_apps_json(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].Name, "Calculator");
    }
    
    #[test]
    fn test_parse_json_with_accents() {
        let json = r#"[{"Name":"Mémoires","AppID":"Test.AppID"},{"Name":"日本語","AppID":"Test2.AppID"}]"#;
        let result = parse_start_apps_json(json).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].Name, "Mémoires");
        assert_eq!(result[1].Name, "日本語");
    }
    
    #[test]
    fn test_filter_uninstall() {
        let apps = vec![
            AppShortcut {
                id: None,
                name: "Uninstall MyApp".to_string(),
                path: "test.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "MyApp".to_string(),
                path: "test2.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
        ];
        
        let filtered = filter_by_name_patterns(apps);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "MyApp");
    }
    
    #[test]
    fn test_filter_help_whole_word() {
        let apps = vec![
            AppShortcut {
                id: None,
                name: "Help".to_string(),
                path: "test.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Helper Tool".to_string(),
                path: "test2.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "HelpDesk Client".to_string(),
                path: "test3.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
        ];
        
        let filtered = filter_by_name_patterns(apps);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "Helper Tool");
        assert_eq!(filtered[1].name, "HelpDesk Client");
    }
    
    #[test]
    fn test_filter_manual_whole_word() {
        let apps = vec![
            AppShortcut {
                id: None,
                name: "Manual".to_string(),
                path: "test.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "ManualMode".to_string(),
                path: "test2.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
        ];
        
        let filtered = filter_by_name_patterns(apps);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "ManualMode");
    }
    
    #[test]
    fn test_dedupe_by_appid() {
        let apps = vec![
            AppShortcut {
                id: None,
                name: "Calculator".to_string(),
                path: "shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Calculator".to_string(),
                path: "shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Camera".to_string(),
                path: "shell:AppsFolder\\Microsoft.WindowsCamera_8wekyb3d8bbwe!App".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
        ];
        
        let deduped = dedupe_by_appid(apps);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].name, "Calculator");
        assert_eq!(deduped[1].name, "Camera");
    }
    
    #[test]
    fn test_extract_appid_from_path() {
        assert_eq!(
            extract_appid_from_path("shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"),
            Some("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".to_string())
        );
        assert_eq!(
            extract_appid_from_path("C:\\test\\app.lnk"),
            None
        );
    }
    
    #[test]
    fn test_path_building() {
        let appid = "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App";
        let path = format!("shell:AppsFolder\\{}", appid);
        assert_eq!(path, "shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App");
    }
    
    #[test]
    fn test_filter_multibyte_names() {
        let apps = vec![
            AppShortcut {
                id: None,
                name: "Microsoft® Word".to_string(),
                path: "test.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Intel® Graphics Command Center".to_string(),
                path: "test2.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Zoom™".to_string(),
                path: "test3.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Café Reader".to_string(),
                path: "test4.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Uninstall Tool®".to_string(),
                path: "test5.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Help Center".to_string(),
                path: "test6.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "Helper".to_string(),
                path: "test7.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
            AppShortcut {
                id: None,
                name: "🎮 Game".to_string(),
                path: "test8.lnk".to_string(),
                icon: None,
                icon_filename: None,
                target: None,
                folder_group: None,
                hidden: false,
            },
        ];
        
        let filtered = filter_by_name_patterns(apps);
        // Should drop "Uninstall Tool®" and "Help Center", keep others
        assert_eq!(filtered.len(), 6);
        assert!(filtered.iter().any(|a| a.name == "Microsoft® Word"));
        assert!(filtered.iter().any(|a| a.name == "Helper"));
        assert!(!filtered.iter().any(|a| a.name == "Uninstall Tool®"));
        assert!(!filtered.iter().any(|a| a.name == "Help Center"));
    }
    
    #[test]
    fn test_first_letter_multibyte() {
        assert_eq!("®Tool".chars().next(), Some('®'));
        assert_eq!("日本".chars().next(), Some('日'));
    }
    
    #[test]
    fn test_6_byte_string_ends_inside_multibyte() {
        // "®" is 3 bytes in UTF-8, so "®Tool" is 6 bytes
        // This tests that we don't panic when slicing at byte boundaries
        let s = "®Tool";
        assert_eq!(s.len(), 6);
        // Should not panic when iterating chars
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 5);
    }
}
