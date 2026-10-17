//! Icon extraction from Windows executables
//! Extracts actual native icons from .exe/.lnk files using Windows API

use std::fs;
use std::hash::Hasher;
use std::path::PathBuf;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::UI::Shell::SHCreateItemFromParsingName;
use windows::Win32::UI::Shell::IShellItemImageFactory;
use windows::Win32::UI::Shell::SIIGBF_BIGGERSIZEOK;
use windows::Win32::UI::Shell::SIIGBF_ICONONLY;
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;
use windows::Win32::Graphics::Gdi::GetDIBits;
use windows::Win32::Graphics::Gdi::DeleteObject;
use windows::Win32::Graphics::Gdi::GetDC;
use windows::Win32::Graphics::Gdi::ReleaseDC;
use windows::Win32::Graphics::Gdi::BITMAPINFO;
use windows::Win32::Graphics::Gdi::BITMAPINFOHEADER;
use windows::Win32::Graphics::Gdi::BI_RGB;
use windows::Win32::Graphics::Gdi::DIB_RGB_COLORS;
use windows::Win32::Foundation::HWND;
use windows::core::PCWSTR;

const ICON_SIZE: i32 = 128;
const CACHE_VERSION: &str = "v3";

/// Extract icon from an executable and save as PNG
/// Returns the filename of the saved icon, or error if failed
pub fn extract_icon_from_path(exe_path: &str, app_data_dir: &std::path::Path) -> Result<String, String> {
    // Create icons directory if it doesn't exist
    let icons_dir = app_data_dir.join("icons");
    fs::create_dir_all(&icons_dir).map_err(|e| e.to_string())?;

    // Generate unique filename based on executable path hash + cache version
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(exe_path.as_bytes());
    hasher.write(CACHE_VERSION.as_bytes());
    let hash_value = hasher.finish();
    let icon_filename = format!("icon_{}.png", hash_value);
    let icon_path = icons_dir.join(&icon_filename);

    // Check if icon already exists
    if icon_path.exists() {
        return Ok(icon_filename);
    }

    // Initialize COM (ignore "already initialized" error)
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    // Try to extract actual Windows icon
    match extract_windows_icon(exe_path, &icon_path) {
        Ok(()) => Ok(icon_filename),
        Err(e) => {
            eprintln!("Icon extraction failed for {}: {}", exe_path, e);
            // Fallback to gradient icon if Windows icon extraction fails
            create_app_icon(&icon_path, exe_path).map_err(|e| e.to_string())?;
            Ok(icon_filename)
        }
    }
}

/// Extract actual Windows icon from executable using IShellItemImageFactory
fn extract_windows_icon(exe_path: &str, output_path: &PathBuf) -> Result<(), String> {
    use std::ffi::OsStr;

    // Convert path to wide string for Windows API
    let path_wide: Vec<u16> = OsStr::new(exe_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        // Create IShellItem from parsing name
        let shell_item: IShellItemImageFactory = SHCreateItemFromParsingName(
            PCWSTR(path_wide.as_ptr()),
            None,
        ).map_err(|e| format!("SHCreateItemFromParsingName failed: {}", e))?;

        // Get image at 128x128
        let hbitmap = shell_item.GetImage(
            windows::Win32::Foundation::SIZE { cx: ICON_SIZE, cy: ICON_SIZE },
            SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
        ).map_err(|e| format!("GetImage failed: {}", e))?;

        // Convert HBITMAP to PNG
        let result = bitmap_to_png(hbitmap, output_path);

        let _ = DeleteObject(hbitmap);

        result
    }
}

/// Convert Windows HBITMAP to PNG file
fn bitmap_to_png(hbitmap: windows::Win32::Graphics::Gdi::HBITMAP, output_path: &PathBuf) -> Result<(), String> {
    unsafe {
        let hdc = GetDC(HWND::default());
        if hdc.is_invalid() {
            return Err("GetDC failed".to_string());
        }

        // Get bitmap info
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: ICON_SIZE,
                biHeight: -ICON_SIZE, // Negative for top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default()],
        };

        let mut bits: Vec<u8> = vec![0; (ICON_SIZE * ICON_SIZE * 4) as usize];

        let lines_copied = GetDIBits(
            hdc,
            hbitmap,
            0,
            ICON_SIZE as u32,
            Some(bits.as_mut_ptr() as *mut std::ffi::c_void),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        ReleaseDC(HWND::default(), hdc);

        if lines_copied == 0 {
            return Err("GetDIBits failed".to_string());
        }

        // Convert premultiplied BGRA to straight-alpha RGBA
        let mut has_alpha = false;
        let mut image_data = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);

        for i in (0..bits.len()).step_by(4) {
            let b = bits[i];
            let g = bits[i + 1];
            let r = bits[i + 2];
            let a = bits[i + 3];

            if a > 0 {
                has_alpha = true;
            }

            let (r, g, b, a) = if a == 0 {
                (r, g, b, 255) // Fully transparent pixels become opaque
            } else if a == 255 {
                (r, g, b, a) // Already straight alpha
            } else {
                // Convert premultiplied to straight alpha
                let a_f = a as f32 / 255.0;
                (
                    (r as f32 / a_f).min(255.0) as u8,
                    (g as f32 / a_f).min(255.0) as u8,
                    (b as f32 / a_f).min(255.0) as u8,
                    a,
                )
            };

            image_data.push(r);
            image_data.push(g);
            image_data.push(b);
            image_data.push(a);
        }

        // If no alpha channel found, set all to opaque
        if !has_alpha {
            for i in (3..image_data.len()).step_by(4) {
                image_data[i] = 255;
            }
        }

        let image: image::RgbaImage = image::ImageBuffer::from_raw(ICON_SIZE as u32, ICON_SIZE as u32, image_data)
            .ok_or("Failed to create image buffer")?;

        image.save(output_path).map_err(|e| e.to_string())?;

        Ok(())
    }
}

/// Create an icon based on the app name/path (fallback)
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

/// Check if icon filename uses the current cache version
pub fn is_current_cache_version(icon_filename: &str) -> bool {
    icon_filename.starts_with("icon_") && icon_filename.contains("_")
}

/// Re-extract icons for shortcuts with old cache versions
pub fn reextract_old_icons(
    shortcuts: &[(i64, String, Option<String>)],
    app_data_dir: &std::path::Path,
) -> Vec<(i64, String)> {
    let mut updated = Vec::new();

    for (id, icon_path, icon_filename) in shortcuts {
        let needs_reextract = match icon_filename {
            Some(filename) => {
                // Check if file exists and uses current cache version
                let icon_full_path = app_data_dir.join("icons").join(filename);
                !icon_full_path.exists() || !is_current_cache_version(filename)
            }
            None => true, // No icon at all, needs extraction
        };

        if needs_reextract {
            match extract_icon_from_path(icon_path, app_data_dir) {
                Ok(new_filename) => {
                    eprintln!("Re-extracted icon for {}: {}", icon_path, new_filename);
                    updated.push((*id, new_filename));
                }
                Err(e) => {
                    eprintln!("Failed to re-extract icon for {}: {}", icon_path, e);
                }
            }
        }
    }

    updated
}

