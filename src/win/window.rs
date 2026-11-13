//! Window management (port of `WindowActionManager.cs` and the relevant parts of `PInvoke.cs`).

use super::{Rect, from_hwnd, from_wide, to_hwnd, wide};
use crate::model::{MonitorInfo, MsfsGameWindowConfig, PanelType};
use std::time::Duration;
use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_CAPTION_COLOR, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute, DwmSetWindowAttribute,
};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows_sys::Win32::System::Threading::{
    AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GWL_STYLE, GetClassNameW, GetCursorPos, GetForegroundWindow,
    GetWindowLongPtrW, GetWindowPlacement, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    HWND_NOTOPMOST, HWND_TOPMOST, IsIconic, MoveWindow, PostMessageW, SW_MINIMIZE, SW_RESTORE,
    SW_SHOW, SWP_NOMOVE, SWP_NOSIZE, SetCursorPos, SetForegroundWindow, SetWindowLongPtrW,
    SetWindowPos, SetWindowTextW, ShowWindow, ShowWindowAsync, WINDOWPLACEMENT, WM_CLOSE,
    WS_CAPTION, WS_SIZEBOX,
};
use windows_sys::core::BOOL;

pub const SW_SHOWNORMAL: i32 = 1;
pub const SW_SHOWMINIMIZED: i32 = 2;
pub const SW_SHOWMAXIMIZED: i32 = 3;

fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

// --- basic queries ----------------------------------------------------------------

pub fn class_name(h: isize) -> String {
    let mut buf = [0u16; 256];
    let n = unsafe { GetClassNameW(to_hwnd(h), buf.as_mut_ptr(), buf.len() as i32) };
    from_wide(&buf[..n.max(0) as usize])
}

pub fn window_text(h: isize) -> String {
    let mut buf = [0u16; 512];
    let n = unsafe { GetWindowTextW(to_hwnd(h), buf.as_mut_ptr(), buf.len() as i32) };
    from_wide(&buf[..n.max(0) as usize])
}

pub fn set_window_text(h: isize, text: &str) {
    let w = wide(text);
    unsafe {
        SetWindowTextW(to_hwnd(h), w.as_ptr());
    }
}

pub fn window_pid(h: isize) -> u32 {
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(to_hwnd(h), &mut pid);
    }
    pid
}

/// Window frame bounds without the invisible resize shadow (what the user sees).
pub fn rect_dwm(h: isize) -> Rect {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe {
        DwmGetWindowAttribute(
            to_hwnd(h),
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            &mut r as *mut RECT as *mut _,
            std::mem::size_of::<RECT>() as u32,
        );
    }
    Rect {
        x: r.left,
        y: r.top,
        width: r.right - r.left,
        height: r.bottom - r.top,
    }
}

fn rect_raw(h: isize) -> RECT {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe {
        GetWindowRect(to_hwnd(h), &mut r);
    }
    r
}

/// Port of `PInvoke.GetWindowRectShadow`: the delta between the full window rectangle and
/// the visible frame (`x`/`y` = left/top delta, `width`/`height` = total size delta).
pub fn rect_shadow(h: isize) -> Rect {
    let vis = rect_dwm(h);
    let full = rect_raw(h);
    let left = full.left - vis.x;
    let right = full.right - vis.right();
    let top = full.top - vis.y;
    let bottom = full.bottom - vis.bottom();
    Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}

pub fn cursor_pos() -> (i32, i32) {
    let mut p = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

pub fn set_cursor_pos(x: i32, y: i32) {
    unsafe {
        SetCursorPos(x, y);
    }
}

pub fn is_minimized(h: isize) -> bool {
    unsafe { IsIconic(to_hwnd(h)) != 0 }
}

pub fn foreground_window() -> isize {
    unsafe { from_hwnd(GetForegroundWindow()) }
}

pub fn show_cmd(h: isize) -> i32 {
    let mut wp: WINDOWPLACEMENT = unsafe { std::mem::zeroed() };
    wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
    unsafe {
        GetWindowPlacement(to_hwnd(h), &mut wp);
    }
    wp.showCmd as i32
}

// --- actions ------------------------------------------------------------------------

/// Move/resize so that the *visible* frame ends up at the requested rectangle.
pub fn move_window(h: isize, x: i32, y: i32, w: i32, ht: i32) {
    let s = rect_shadow(h);
    unsafe {
        MoveWindow(to_hwnd(h), x + s.x, y + s.y, w + s.width, ht + s.height, 1);
    }
}

pub fn move_window_rect(h: isize, r: Rect) {
    move_window(h, r.x, r.y, r.width, r.height);
}

/// `MoveWindow` without shadow compensation (used for the game window).
pub fn move_window_raw(h: isize, x: i32, y: i32, w: i32, ht: i32) {
    unsafe {
        MoveWindow(to_hwnd(h), x, y, w, ht, 1);
    }
}

pub fn apply_hide_title_bar(h: isize, hide: bool) {
    if hide {
        let r = rect_dwm(h);
        let s = rect_shadow(h);
        move_window(h, r.x - s.x, r.y, r.width - s.width, r.height - s.height);
        sleep(250);
    }

    unsafe {
        let mut style = GetWindowLongPtrW(to_hwnd(h), GWL_STYLE) as u32;
        if hide {
            style &= !(WS_CAPTION | WS_SIZEBOX);
        } else {
            style |= WS_CAPTION | WS_SIZEBOX;
        }
        SetWindowLongPtrW(to_hwnd(h), GWL_STYLE, style as isize);
    }
    sleep(250);

    if !hide {
        let r = rect_dwm(h);
        let s = rect_shadow(h);
        move_window(h, r.x + s.x, r.y, r.width + s.width, r.height + s.height);
    }
}

pub fn apply_always_on_top(h: isize, on_top: bool) {
    let r = rect_dwm(h);
    unsafe {
        if on_top {
            SetWindowPos(
                to_hwnd(h),
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        } else {
            SetWindowPos(to_hwnd(h), HWND_NOTOPMOST, r.x, r.y, r.width, r.height, 0);
            sleep(250);
            move_window(h, r.x, r.y, r.width, r.height);
        }
    }
}

/// Ask a window to close. Posted (not sent) so an unresponsive window cannot block us.
pub fn close_window(h: isize) {
    unsafe {
        PostMessageW(to_hwnd(h), WM_CLOSE, 0, 0);
    }
}

pub fn minimize_window(h: isize) {
    unsafe {
        ShowWindow(to_hwnd(h), SW_MINIMIZE);
    }
}

pub fn restore_window(h: isize) {
    unsafe {
        ShowWindow(to_hwnd(h), SW_RESTORE);
    }
}

pub fn show_window(h: isize, cmd: i32) {
    unsafe {
        ShowWindow(to_hwnd(h), cmd);
    }
}

/// Non-blocking variants for windows owned by our own GUI thread.
pub fn minimize_window_async(h: isize) {
    unsafe {
        ShowWindowAsync(to_hwnd(h), SW_MINIMIZE);
    }
}

pub fn bring_to_foreground(h: isize) {
    unsafe {
        ShowWindowAsync(to_hwnd(h), SW_RESTORE);
        SetForegroundWindow(to_hwnd(h));
    }
}

pub fn set_foreground(h: isize) {
    unsafe {
        SetForegroundWindow(to_hwnd(h));
    }
}

/// Port of `WindowActionManager.SetWindowFocus`: give one of our own windows focus even
/// while another application (the simulator) owns the foreground, by temporarily sharing
/// input state with the foreground thread. Must be called on the window's own thread.
pub fn set_window_focus(h: isize) {
    unsafe {
        let hwnd = to_hwnd(h);
        let foreground_thread =
            GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
        let own_thread = GetCurrentThreadId();
        let attach = foreground_thread != 0 && foreground_thread != own_thread;
        if attach {
            AttachThreadInput(foreground_thread, own_thread, 1);
        }
        BringWindowToTop(hwnd);
        ShowWindow(hwnd, SW_SHOW);
        if attach {
            AttachThreadInput(foreground_thread, own_thread, 0);
        }
    }
}

pub fn set_title_bar_color(h: isize, hex: &str) {
    if let Ok(color) = i32::from_str_radix(hex.trim().trim_start_matches('#'), 16) {
        unsafe {
            DwmSetWindowAttribute(
                to_hwnd(h),
                DWMWA_CAPTION_COLOR as u32,
                &color as *const i32 as *const _,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }
}

// --- enumeration and classification -------------------------------------------------

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam as *mut Vec<isize>);
    list.push(from_hwnd(hwnd));
    1
}

pub fn top_level_windows() -> Vec<isize> {
    let mut list: Vec<isize> = Vec::new();
    unsafe {
        EnumWindows(Some(enum_proc), &mut list as *mut Vec<isize> as LPARAM);
    }
    list
}

/// Classify a window the way upstream does (MSFS windows use the `AceApp` class).
pub fn classify(h: isize) -> PanelType {
    if class_name(h) != "AceApp" {
        return PanelType::Unknown;
    }
    let caption = window_text(h);
    classify_ace_app_caption(&caption)
}

pub fn classify_ace_app_caption(caption: &str) -> PanelType {
    if caption.is_empty() || caption.contains("(Custom)") {
        PanelType::CustomPopout
    } else if caption.contains("Microsoft Flight Simulator") {
        PanelType::FlightSimMainWindow
    } else if caption.contains("WINDOW") {
        PanelType::MultiMonitorWindow
    } else {
        PanelType::BuiltInPopout
    }
}

pub fn windows_by_panel_type(types: &[PanelType]) -> Vec<isize> {
    top_level_windows()
        .into_iter()
        .filter(|&h| types.contains(&classify(h)))
        .collect()
}

/// Close every custom pop out window of the simulator.
pub fn close_all_custom_pop_outs() {
    for h in windows_by_panel_type(&[PanelType::CustomPopout]) {
        close_window(h);
    }
}

pub fn msfs_game_window() -> Option<isize> {
    top_level_windows()
        .into_iter()
        .rev() // upstream keeps the last match
        .find(|&h| {
            class_name(h) == "AceApp" && window_text(h).contains("Microsoft Flight Simulator")
        })
}

pub fn is_msfs_windowed() -> bool {
    match msfs_game_window() {
        Some(h) => unsafe { (GetWindowLongPtrW(to_hwnd(h), GWL_STYLE) as u32 & WS_CAPTION) != 0 },
        None => false,
    }
}

pub fn msfs_game_window_location() -> MsfsGameWindowConfig {
    if is_msfs_windowed()
        && let Some(h) = msfs_game_window()
    {
        let r = rect_dwm(h);
        return MsfsGameWindowConfig {
            left: r.x,
            top: r.y,
            width: r.width,
            height: r.height,
        };
    }
    MsfsGameWindowConfig::default()
}

pub fn set_msfs_game_window_location(cfg: &MsfsGameWindowConfig) {
    if is_msfs_windowed()
        && cfg.is_valid()
        && let Some(h) = msfs_game_window()
    {
        move_window_raw(h, cfg.left, cfg.top, cfg.width, cfg.height);
    }
}

pub fn is_msfs_in_focus() -> bool {
    let text = window_text(foreground_window());
    text.chars().count() >= 26
        && text
            .chars()
            .take(26)
            .collect::<String>()
            .eq_ignore_ascii_case("Microsoft Flight Simulator")
}

/// Find one of *our own* top-level windows by its exact title.
pub fn find_own_window(title: &str) -> Option<isize> {
    let pid = unsafe { GetCurrentProcessId() };
    top_level_windows()
        .into_iter()
        .find(|&h| window_pid(h) == pid && window_text(h) == title)
}

// --- monitors -------------------------------------------------------------------------

unsafe extern "system" fn monitor_proc(
    m: HMONITOR,
    _dc: HDC,
    _r: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let list = &mut *(lparam as *mut Vec<MonitorInfo>);
    let mut info: MONITORINFOEXW = std::mem::zeroed();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(m, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) != 0 {
        let device = from_wide(&info.szDevice);
        let name = device.rsplit('\\').next().unwrap_or(&device).to_string();
        let r = info.monitorInfo.rcMonitor;
        list.push(MonitorInfo {
            name,
            is_selected: false,
            x: r.left,
            y: r.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
        });
    }
    1
}

pub fn monitors() -> Vec<MonitorInfo> {
    let mut list: Vec<MonitorInfo> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(monitor_proc),
            &mut list as *mut Vec<MonitorInfo> as LPARAM,
        );
    }
    list
}

/// Monitors sorted by name (the order the UI presents them in).
pub fn monitors_sorted() -> Vec<MonitorInfo> {
    let mut m = monitors();
    m.sort_by(|a, b| a.name.cmp(&b.name));
    m
}

/// Monitor hosting a panel (last intersecting monitor wins, like upstream).
pub fn host_monitor(left: i32, top: i32, width: i32, height: i32) -> Option<MonitorInfo> {
    let panel = Rect {
        x: left,
        y: top,
        width,
        height,
    };
    let mut found = None;
    for m in monitors() {
        let bounds = Rect {
            x: m.x,
            y: m.y,
            width: m.width,
            height: m.height,
        };
        if bounds.intersects(&panel) {
            found = Some(m);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_classification() {
        assert_eq!(classify_ace_app_caption(""), PanelType::CustomPopout);
        assert_eq!(
            classify_ace_app_caption("PFD (Custom)"),
            PanelType::CustomPopout
        );
        assert_eq!(
            classify_ace_app_caption("Microsoft Flight Simulator - 1.2.3"),
            PanelType::FlightSimMainWindow
        );
        assert_eq!(
            classify_ace_app_caption("WINDOW 2"),
            PanelType::MultiMonitorWindow
        );
        assert_eq!(classify_ace_app_caption("ATC"), PanelType::BuiltInPopout);
    }
}
