//! Icon extraction from Windows executables
//! Extracts actual native icons from .exe/.lnk files using Windows API

use std::fs;
use std::hash::Hasher;
use std::path::PathBuf;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::UI::Shell::SHGetFileInfoW;
use windows::Win32::UI::Shell::SHFILEINFOW;
use windows::Win32::UI::Shell::SHGFI_ICON;
use windows::Win32::UI::Shell::SHGFI_LARGEICON;
use windows::Win32::UI::Shell::SHGFI_USEFILEATTRIBUTES;
use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;
use windows::Win32::UI::WindowsAndMessaging::DrawIconEx;
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::core::PCWSTR;

/// Extract icon from an executable and save as PNG
/// Returns the filename of the saved icon, or error if failed
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

    // Try to extract actual Windows icon
    if let Ok(()) = extract_windows_icon(exe_path, &icon_path) {
        return Ok(icon_filename);
    }

    // Fallback to gradient icon if Windows icon extraction fails
    create_app_icon(&icon_path, exe_path).map_err(|e| e.to_string())?;

    Ok(icon_filename)
}

/// Extract actual Windows icon from executable using SHGetFileInfoW
fn extract_windows_icon(exe_path: &str, output_path: &PathBuf) -> Result<(), String> {
    use std::ffi::OsStr;

    // Convert path to wide string for Windows API
    let path_wide: Vec<u16> = OsStr::new(exe_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut shfi = SHFILEINFOW::default();

    unsafe {
        let result = SHGetFileInfoW(
            PCWSTR(path_wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut shfi),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
        );

        if result == 0 {
            return Err("SHGetFileInfoW failed".to_string());
        }

        if shfi.hIcon.is_invalid() {
            return Err("Invalid icon handle".to_string());
        }

        // Convert HICON to PNG
        let result = icon_to_png(shfi.hIcon, output_path);

        let _ = DestroyIcon(shfi.hIcon);

        result
    }
}

/// Convert Windows HICON to PNG file
fn icon_to_png(hicon: windows::Win32::UI::WindowsAndMessaging::HICON, output_path: &PathBuf) -> Result<(), String> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC,
        SelectObject,
        DeleteObject,
        DeleteDC,
        GetDC,
        ReleaseDC,
        CreateDIBSection,
        BITMAPINFO,
        BITMAPINFOHEADER,
        BI_RGB,
        DIB_RGB_COLORS,
    };
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::DI_NORMAL;

    unsafe {
        let hdc = GetDC(HWND::default());
        if hdc.is_invalid() {
            return Err("GetDC failed".to_string());
        }

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 48,
                biHeight: 48 * 2, // Positive for bottom-up DIB
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

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbitmap = CreateDIBSection(
            hdc,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        ).map_err(|e| e.to_string())?;

        let hdc_mem = CreateCompatibleDC(hdc);
        if hdc_mem.is_invalid() {
            ReleaseDC(HWND::default(), hdc);
            let _ = DeleteObject(hbitmap);
            return Err("CreateCompatibleDC failed".to_string());
        }

        let old_bitmap = SelectObject(hdc_mem, hbitmap);

        // Draw icon to bitmap
        let _ = DrawIconEx(
            hdc_mem,
            0,
            0,
            hicon,
            48,
            48,
            0,
            None,
            DI_NORMAL,
        );

        SelectObject(hdc_mem, old_bitmap);
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(HWND::default(), hdc);

        // Convert bitmap data to PNG
        if bits.is_null() {
            let _ = DeleteObject(hbitmap);
            return Err("Null bitmap bits".to_string());
        }

        let width = 48;
        let height = 48;
        let mut image_data = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height {
            for x in 0..width {
                let pixel_offset = ((height - 1 - y) * width + x) * 4;
                let bits_u8 = bits as *const u8;
                let b = *bits_u8.add(pixel_offset as usize);
                let g = *bits_u8.add((pixel_offset + 1) as usize);
                let r = *bits_u8.add((pixel_offset + 2) as usize);
                let a = *bits_u8.add((pixel_offset + 3) as usize);

                // Pre-multiply alpha
                let a_f = a as f32 / 255.0;
                image_data.push((r as f32 * a_f) as u8);
                image_data.push((g as f32 * a_f) as u8);
                image_data.push((b as f32 * a_f) as u8);
                image_data.push(a);
            }
        }

        let image: image::RgbaImage = image::ImageBuffer::from_raw(width, height, image_data)
            .ok_or("Failed to create image buffer")?;

        image.save(output_path).map_err(|e| e.to_string())?;

        let _ = DeleteObject(hbitmap);

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

