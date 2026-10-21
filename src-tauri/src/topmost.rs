//! Window Management Macro: keep a schema diagram or notepad permanently on
//! top using the native SetWindowPos(HWND_TOPMOST) z-order flag.
//!
//! Windows owns the stacking order from then on — there is no repositioning
//! loop, no timer and no polling, which is the whole point of using the flag
//! rather than re-applying position every frame (AGENTS.md invariant #1).

use serde::Serialize;

/// Our own executable name, so Isle never lists itself (see `item_for`).
const APP_EXE: &str = "isle.exe";

/// One top-level window the user can float.
#[derive(Debug, Clone, Serialize)]
pub struct WinItem {
    pub hwnd: i64,
    /// owning process id (the resource monitor reads this process)
    pub pid: u32,
    pub title: String,
    pub process: String,
}

/// Windows we have floated and not yet released, so "Release all" is exact.
#[cfg(windows)]
static FLOATED: std::sync::LazyLock<std::sync::Mutex<Vec<i64>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

/// Top-level, visible, titled windows that are not Isle itself.
pub fn list() -> Result<Vec<WinItem>, String> {
    #[cfg(windows)]
    {
        list_win()
    }
    #[cfg(not(windows))]
    {
        Err("window macros require Windows".to_string())
    }
}

/// Float a window (HWND_TOPMOST).
pub fn float(hwnd: i64) -> Result<(), String> {
    set_topmost(hwnd, true)
}

/// Return a window to normal stacking (HWND_NOTOPMOST).
pub fn release(hwnd: i64) -> Result<(), String> {
    set_topmost(hwnd, false)
}

/// Float whatever the user was in before focusing Isle. The Wellbeing hook
/// already tracks it, so this needs no extra machinery.
pub fn float_last() -> Result<String, String> {
    let hwnd = crate::wellbeing::last_foreign_hwnd() as i64;
    if hwnd == 0 {
        return Err("No recent window yet — switch to it once, then try again".to_string());
    }
    let title = title_of(hwnd).unwrap_or_else(|| "Window".to_string());
    set_topmost(hwnd, true)?;
    Ok(title)
}

/// Is Isle currently keeping this window on top?
pub fn is_floating(hwnd: i64) -> bool {
    #[cfg(windows)]
    {
        FLOATED.lock().map(|g| g.contains(&hwnd)).unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        false
    }
}

/// Drop the topmost flag from everything Isle has floated.
pub fn release_all() -> Result<usize, String> {
    #[cfg(windows)]
    {
        let ids: Vec<i64> = match FLOATED.lock() {
            Ok(g) => g.clone(),
            Err(_) => return Ok(0),
        };
        let mut n = 0;
        for id in ids {
            if set_topmost_win(id, false).is_ok() {
                n += 1;
            }
        }
        Ok(n)
    }
    #[cfg(not(windows))]
    {
        Ok(0)
    }
}

fn title_of(hwnd: i64) -> Option<String> {
    #[cfg(windows)]
    {
        title_of_win(hwnd)
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        None
    }
}

fn set_topmost(hwnd: i64, on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        set_topmost_win(hwnd, on)
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, on);
        Err("window macros require Windows".to_string())
    }
}

#[cfg(windows)]
fn track(hwnd: i64, on: bool) {
    if let Ok(mut g) = FLOATED.lock() {
        if on {
            if !g.contains(&hwnd) {
                g.push(hwnd);
            }
        } else {
            g.retain(|&id| id != hwnd);
        }
    }
}

#[cfg(windows)]
fn set_topmost_win(hwnd: i64, on: bool) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        IsWindow, SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE,
    };
    let h = HWND(hwnd as *mut std::ffi::c_void);
    unsafe {
        if !IsWindow(h).as_bool() {
            return Err("That window no longer exists".to_string());
        }
        let after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
        SetWindowPos(h, after, 0, 0, 0, 0, flags)
            .map_err(|e| format!("SetWindowPos failed: {e}"))?;
    }
    track(hwnd, on);
    Ok(())
}

#[cfg(windows)]
fn list_win() -> Result<Vec<WinItem>, String> {
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, WNDENUMPROC};
    let mut out: Vec<WinItem> = Vec::new();
    unsafe {
        let cb: WNDENUMPROC = Some(enum_proc);
        EnumWindows(cb, LPARAM(&mut out as *mut Vec<WinItem> as isize))
            .map_err(|e| format!("EnumWindows: {e}"))?;
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

#[cfg(windows)]
unsafe extern "system" fn enum_proc(
    hwnd: windows::Win32::Foundation::HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    use windows::Win32::Foundation::BOOL;
    let out = &mut *(lparam.0 as *mut Vec<WinItem>);
    if let Some(item) = item_for(hwnd) {
        out.push(item);
    }
    BOOL(1) // keep enumerating
}

#[cfg(windows)]
unsafe fn item_for(hwnd: windows::Win32::Foundation::HWND) -> Option<WinItem> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
    };
    if !IsWindowVisible(hwnd).as_bool() {
        return None;
    }
    // skip owned windows: popups, menus, dialogs
    let owned = GetWindow(hwnd, GW_OWNER).map(|o| !o.0.is_null()).unwrap_or(false);
    if owned {
        return None;
    }
    let title = title_of_win(hwnd.0 as i64).unwrap_or_default();
    if title.trim().is_empty() {
        return None;
    }
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
    if pid == 0 || pid == std::process::id() {
        return None; // never offer Isle itself
    }
    let process = process_name(pid);
    // Also skip *any other* Isle process: the single-instance guard normally
    // makes this impossible, but a stale build or a dev instance should never
    // show up as something the user can float.
    if process.eq_ignore_ascii_case(APP_EXE) {
        return None;
    }
    Some(WinItem { hwnd: hwnd.0 as i64, pid, title, process })
}

#[cfg(windows)]
fn title_of_win(hwnd: i64) -> Option<String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let h = HWND(hwnd as *mut std::ffi::c_void);
    unsafe {
        let mut buf = [0u16; 256];
        let n = GetWindowTextW(h, &mut buf);
        if n == 0 {
            return None;
        }
        let n = (n as usize).min(buf.len()); // never index past the buffer
        Some(String::from_utf16_lossy(&buf[..n]).trim().to_string())
    }
}

#[cfg(windows)]
fn process_name(pid: u32) -> String {
    use windows::Win32::Foundation::{CloseHandle, BOOL, HANDLE};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let h: HANDLE = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, BOOL(0), pid) {
            Ok(h) => h,
            Err(_) => return String::new(),
        };
        let mut buf = [0u16; 512];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            h,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(h);
        if ok.is_err() {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..(size as usize).min(buf.len())]);
        path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string()
    }
}

// -- Tauri commands (registered in lib.rs, mocked in src/lib/api.js) ---------

#[tauri::command]
pub fn topmost_list() -> Result<Vec<WinItem>, String> {
    list()
}

#[tauri::command]
pub fn topmost_float(hwnd: i64) -> Result<(), String> {
    float(hwnd)
}

#[tauri::command]
pub fn topmost_release(hwnd: i64) -> Result<(), String> {
    release(hwnd)
}

/// The macro button: floats the window the user was last in (tracked by the
/// Wellbeing hook), so no picker is needed for the common case.
#[tauri::command]
pub fn topmost_float_last() -> Result<String, String> {
    float_last()
}

#[tauri::command]
pub fn topmost_release_all() -> Result<usize, String> {
    release_all()
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn enumeration_finds_real_windows_without_isle() {
        // On any interactive desktop there is at least one visible top-level
        // window; in a headless session the list is simply empty. What matters
        // is that the call succeeds and never lists our own process.
        let items = super::list().expect("EnumWindows must not fail");
        for w in &items {
            assert!(w.hwnd != 0);
            assert!(!w.title.trim().is_empty(), "untitled windows must be filtered: {w:?}");
            assert!(w.process != "isle.exe", "Isle must never offer itself: {w:?}");
        }
    }

    #[test]
    fn floating_an_invalid_handle_fails_cleanly() {
        // 0x42 is not a window; the macro must return Err, not panic.
        assert!(super::float(0x42).is_err(), "an invalid HWND must be rejected");
    }
}
