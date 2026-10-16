//! Clipboard history (text + files + images).
//!
//! EVENT-DRIVEN: `clipboard-master` blocks on the Win32 clipboard-format
//! listener, so this thread uses ~0 CPU until the clipboard actually changes.
//!
//! PRIVACY: copies flagged by password managers with the
//! "ExcludeClipboardContentFromMonitorProcessing" format are skipped.
//! Entries are capped at 20k chars; only the newest 300 unpinned items are kept.
//!
//! UI contract: event `clipboard://changed` (no payload); commands in commands.rs.

use tauri::{AppHandle, State};
use crate::db::Db;

pub fn start(app: AppHandle) {
    imp::start(app)
}

pub fn stop() {
    imp::stop()
}

pub fn paste_files_from_clipboard(db: State<Db>) -> Result<usize, String> {
    imp::paste_files_from_clipboard(db.inner())
}

pub fn set_enabled(enabled: bool) {
    imp::set_enabled(enabled)
}

pub fn is_enabled() -> bool {
    imp::is_enabled()
}

#[cfg(windows)]
mod imp {
    use crate::db::{now, Db};
    use clipboard_master::{CallbackResult, ClipboardHandler, Master};
    use tauri::{AppHandle, Emitter, Manager};
    use windows::{
        core::w,
        Win32::System::DataExchange::{IsClipboardFormatAvailable, RegisterClipboardFormatW},
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::process::Command;
    use std::os::windows::process::CommandExt;

    static CLIPBOARD_ENABLED: AtomicBool = AtomicBool::new(true);

    struct Handler {
        app: AppHandle,
    }

    impl ClipboardHandler for Handler {
        fn on_clipboard_change(&mut self) -> CallbackResult {
            if CLIPBOARD_ENABLED.load(Ordering::Relaxed) {
                capture(&self.app);
            }
            CallbackResult::Next
        }
        fn on_clipboard_error(&mut self, _e: std::io::Error) -> CallbackResult {
            CallbackResult::Next
        }
    }

    pub fn start(app: AppHandle) {
        std::thread::spawn(move || {
            let _ = Master::new(Handler { app }).run();
        });
    }

    pub fn stop() {
        CLIPBOARD_ENABLED.store(false, Ordering::Relaxed);
    }

    pub fn is_enabled() -> bool {
        CLIPBOARD_ENABLED.load(Ordering::Relaxed)
    }

    pub fn set_enabled(enabled: bool) {
        CLIPBOARD_ENABLED.store(enabled, Ordering::Relaxed);
    }

    pub fn paste_files_from_clipboard(db: &Db) -> Result<usize, String> {
        let files = read_files_via_powershell();
        if files.is_empty() {
            return Ok(0);
        }

        let mut count = 0;
        for file_path in files {
            let path = std::path::Path::new(&file_path);
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&file_path)
                .to_string();
            let file_type = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown")
                .to_string();

            db.with(|c| {
                c.execute(
                    "INSERT INTO clips(text,pinned,ts,file_path,file_name,file_type) VALUES(?1,0,?2,?3,?4,?5)",
                    rusqlite::params![&file_name, now(), &file_path, &file_name, &file_type],
                )?;
                Ok::<_, rusqlite::Error>(())
            }).map_err(|e| e.to_string())?;
            count += 1;
        }

        // Clean up old unpinned items
        let _ = db.with(|c| {
            c.execute(
                "DELETE FROM clips WHERE pinned=0 AND id NOT IN
                 (SELECT id FROM clips WHERE pinned=0 ORDER BY ts DESC LIMIT 300)",
                [],
            )?;
            Ok(())
        });

        Ok(count)
    }

    fn excluded() -> bool {
        unsafe {
            let f = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
            f != 0 && IsClipboardFormatAvailable(f).is_ok()
        }
    }

    const CF_DIB: u32 = 8;
    const CF_BITMAP: u32 = 2;

    fn has_image() -> bool {
        unsafe {
            IsClipboardFormatAvailable(CF_DIB).is_ok() || IsClipboardFormatAvailable(CF_BITMAP).is_ok()
        }
    }

    fn read_text() -> Option<String> {
        // The clipboard can be briefly locked by the app that just wrote to it.
        for _ in 0..3 {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                if let Ok(t) = cb.get_text() {
                    return Some(t);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        None
    }

    fn read_image(app: &AppHandle) -> Option<String> {
        // Try to read image from clipboard using arboard
        for _ in 0..3 {
            if let Ok(mut cb) = arboard::Clipboard::new() {
                if let Ok(image) = cb.get_image() {
                    // Save image to disk
                    if let Ok(dir) = app.path().app_data_dir() {
                        let images_dir = dir.join("clipboard_images");
                        let _ = std::fs::create_dir_all(&images_dir);

                        let timestamp = now();
                        let filename = format!("screenshot_{}.png", timestamp);
                        let filepath = images_dir.join(&filename);

                        // Convert arboard Image to PNG bytes
                        // arboard returns ImageData with width, height, and RGBA bytes
                        let png_bytes = match image_to_png(&image) {
                            Ok(bytes) => bytes,
                            Err(_) => continue,
                        };

                        if std::fs::write(&filepath, &png_bytes).is_ok() {
                            return Some(filepath.to_string_lossy().to_string());
                        }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        None
    }

    fn image_to_png(image: &arboard::ImageData) -> Result<Vec<u8>, String> {
        use image::{ImageBuffer, ImageEncoder, RgbaImage};
        use std::io::Cursor;

        let width = image.width as u32;
        let height = image.height as u32;
        let rgba_data = &image.bytes;

        // Create RgbaImage from bytes (arboard provides RGBA data)
        // Need to convert Cow to Vec for ImageBuffer
        let img: RgbaImage = ImageBuffer::from_raw(width, height, rgba_data.to_vec())
            .ok_or("Failed to create image buffer from clipboard data")?;

        // Encode as PNG
        let mut buffer = Vec::new();
        {
            let mut cursor = Cursor::new(&mut buffer);
            let encoder = image::codecs::png::PngEncoder::new_with_quality(
                &mut cursor,
                image::codecs::png::CompressionType::Fast,
                image::codecs::png::FilterType::Adaptive,
            );
            encoder
                .write_image(
                    img.as_raw(),
                    width,
                    height,
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(|e| format!("PNG encoding failed: {}", e))?;
        }

        Ok(buffer)
    }

    fn read_files_via_powershell() -> Vec<String> {
        // Use PowerShell to read file paths from clipboard (hidden window)
        match Command::new("powershell")
            .args([
                "-WindowStyle",
                "Hidden",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Clipboard]::GetFileDropList() | ForEach-Object { $_ }"
            ])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            Err(_) => Vec::new(),
        }
    }

    fn capture(app: &AppHandle) {
        if excluded() {
            return;
        }

        let db = app.state::<Db>();
        let mut captured = false;

        // Check for images first (screenshots, snipping tool)
        if has_image() {
            if let Some(image_path) = read_image(app) {
                let _ = db.with(|c| {
                    c.execute(
                        "INSERT INTO clips(text,pinned,ts,file_path,file_name,file_type) VALUES(?1,0,?2,?3,?4,?5)",
                        rusqlite::params!["Screenshot", now(), &image_path, "Screenshot", "image"],
                    )?;
                    Ok::<_, rusqlite::Error>(())
                });
                captured = true;
            }
        }

        // If no image, try text
        if !captured {
            let Some(text) = read_text() else { return };
            let t = text.trim_end();
            if t.trim().is_empty() || t.len() > 20_000 {
                return;
            }
            let _ = db.with(|c| {
                c.execute(
                    "INSERT INTO clips(text,pinned,ts) VALUES(?1,0,?2)
                     ON CONFLICT(text) DO UPDATE SET ts=excluded.ts",
                    rusqlite::params![t, now()],
                )?;
                Ok::<_, rusqlite::Error>(())
            });
            captured = true;
        }

        if captured {
            // Clean up old unpinned items
            let _ = db.with(|c| {
                c.execute(
                    "DELETE FROM clips WHERE pinned=0 AND id NOT IN
                     (SELECT id FROM clips WHERE pinned=0 ORDER BY ts DESC LIMIT 300)",
                    [],
                )?;
                Ok(())
            });
            let _ = app.emit("clipboard://changed", ());
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use tauri::AppHandle;
    pub fn start(_: AppHandle) {}
}
