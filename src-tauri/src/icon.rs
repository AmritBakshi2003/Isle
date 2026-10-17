//! Icon extraction from Windows executables
//! Extracts icons from .exe/.lnk files and saves them as PNG

use std::fs;
use std::hash::Hasher;
use std::path::PathBuf;

/// Extract icon from an executable and save as PNG
/// Returns the filename of the saved icon, or error if failed
/// Currently uses smart fallback icons based on app name/path
pub fn extract_icon_from_path(exe_path: &str, app_data_dir: &std::path::Path) -> Result<String, String> {
    // Create icons directory if it doesn't exist
    let icons_dir = app_data_dir.join("icons");
    fs::create_dir_all(&icons_dir).map_err(|e| e.to_string())?;

    // Generate unique filename based on executable path hash
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(exe_path.as_bytes());
    let hash_value = hasher.finish();
    let icon_filename = format!("icon_{}.png", hash_value);
    let icon_path = icons_dir.join(&icon_filename);

    // Check if icon already exists
    if icon_path.exists() {
        return Ok(icon_filename);
    }

    // Create app-specific icon based on name/path
    create_app_icon(&icon_path, exe_path).map_err(|e| e.to_string())?;

    Ok(icon_filename)
}

/// Create an icon based on the app name/path
fn create_app_icon(path: &PathBuf, exe_path: &str) -> Result<(), String> {
    let app_name_lower = exe_path.to_lowercase();
    
    // Determine icon color and pattern based on app name
    let (r, g, b) = get_app_color(&app_name_lower);
    
    let mut image = image::RgbaImage::new(48, 48);
    for y in 0..48 {
        for x in 0..48 {
            // Create a simple gradient with the app's color
            let x_ratio = x as f32 / 48.0;
            let y_ratio = y as f32 / 48.0;
            let pixel_r = (r as f32 * (1.0 - x_ratio * 0.3)) as u8;
            let pixel_g = (g as f32 * (1.0 - y_ratio * 0.3)) as u8;
            let pixel_b = (b as f32 * (1.0 - (x_ratio + y_ratio) * 0.2)) as u8;
            image.put_pixel(x, y, image::Rgba([pixel_r, pixel_g, pixel_b, 255]));
        }
    }
    image.save(path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get a color based on the app name/path
fn get_app_color(app_name: &str) -> (u8, u8, u8) {
    // Color mappings for common apps
    if app_name.contains("chrome") || app_name.contains("edge") {
        return (100, 149, 237); // Blue
    }
    if app_name.contains("firefox") {
        return (255, 140, 0); // Orange
    }
    if app_name.contains("excel") || app_name.contains("spreadsheet") {
        return (33, 150, 243); // Excel Green
    }
    if app_name.contains("word") || app_name.contains("document") {
        return (66, 133, 244); // Word Blue
    }
    if app_name.contains("powerpoint") || app_name.contains("presentation") {
        return (247, 107, 28); // PowerPoint Orange
    }
    if app_name.contains("outlook") || app_name.contains("mail") {
        return (0, 120, 212); // Outlook Blue
    }
    if app_name.contains("visual studio") || app_name.contains("vscode") || app_name.contains("code") {
        return (30, 144, 255); // VS Code Blue
    }
    if app_name.contains("spotify") {
        return (30, 215, 96); // Spotify Green
    }
    if app_name.contains("discord") {
        return (114, 137, 218); // Discord Purple
    }
    if app_name.contains("slack") {
        return (240, 224, 66); // Slack Yellow
    }
    if app_name.contains("steam") {
        return (102, 126, 234); // Steam Blue
    }
    if app_name.contains("photoshop") || app_name.contains("adobe") {
        return (255, 31, 76); // Adobe Red
    }
    if app_name.contains("recycle") || app_name.contains("trash") {
        return (108, 117, 125); // Gray
    }
    if app_name.contains("control") || app_name.contains("settings") {
        return (148, 163, 184); // Settings Gray
    }
    if app_name.contains("calculator") {
        return (50, 50, 50); // Dark Gray
    }
    if app_name.contains("notepad") || app_name.contains("text") {
        return (150, 150, 150); // Light Gray
    }
    
    // Default purple gradient
    (147, 112, 219)
}

/// Delete icon file from icons directory
pub fn delete_icon(icon_filename: &str, app_data_dir: &std::path::Path) -> Result<(), String> {
    let icon_path = app_data_dir.join("icons").join(icon_filename);
    if icon_path.exists() {
        fs::remove_file(icon_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Clear all icon files (for testing/regeneration)
pub fn clear_all_icons(app_data_dir: &std::path::Path) -> Result<(), String> {
    let icons_dir = app_data_dir.join("icons");
    if icons_dir.exists() {
        fs::remove_dir_all(&icons_dir).map_err(|e| e.to_string())?;
        fs::create_dir_all(&icons_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Regenerate icon for a specific shortcut
pub fn regenerate_icon(icon_filename: &str, app_data_dir: &std::path::Path) -> Result<(), String> {
    let icon_path = app_data_dir.join("icons").join(icon_filename);
    if icon_path.exists() {
        fs::remove_file(icon_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

