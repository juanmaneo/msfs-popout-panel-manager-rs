//! Thin wrappers around the Win32 APIs used by the application.
//!
//! Window handles are passed around as `isize` so they can live in the (Send + Sync)
//! application model without wrapping raw pointers.

#![allow(unsafe_op_in_unsafe_fn)]

pub mod hooks;
pub mod input;
pub mod overlay;
pub mod process;
pub mod touch_hook;
pub mod window;

use windows_sys::Win32::Foundation::HWND;

#[inline]
pub fn to_hwnd(h: isize) -> HWND {
    h as HWND
}

#[inline]
pub fn from_hwnd(h: HWND) -> isize {
    h as isize
}

/// NUL terminated UTF-16 string.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Integer rectangle (`x`, `y`, `width`, `height`) like `System.Drawing.Rectangle`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }

    pub fn intersects(&self, o: &Rect) -> bool {
        o.x < self.right() && self.x < o.right() && o.y < self.bottom() && self.y < o.bottom()
    }
}

/// Make the process per-monitor DPI aware so that all coordinates we handle are physical pixels.
pub fn set_dpi_aware() {
    use windows_sys::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

/// Simple modal error/info box (used before the GUI is available).
pub fn message_box(title: &str, text: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
    let (t, m) = (wide(title), wide(text));
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            m.as_ptr(),
            t.as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// Open a URL (or a file / folder path) with the default Windows handler.
pub fn open_externally(target: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let verb = wide("open");
    let file = wide(target);
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1, // SW_SHOWNORMAL
        )
    };
    // ShellExecute reports success with a pseudo-handle greater than 32.
    let code = result as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(format!(
            "could not open {target} (ShellExecute code {code})"
        ))
    }
}

/// Create a named mutex; returns `false` when another instance already owns it.
pub fn acquire_single_instance(name: &str) -> bool {
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let n = wide(name);
    unsafe {
        let h = CreateMutexW(std::ptr::null(), 0, n.as_ptr());
        // intentionally leaked: it must live as long as the process
        !(h.is_null() || GetLastError() == ERROR_ALREADY_EXISTS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_helpers() {
        let r = Rect {
            x: 10,
            y: 10,
            width: 100,
            height: 50,
        };
        assert!(r.contains(10, 10));
        assert!(r.contains(110, 60));
        assert!(!r.contains(111, 60));
        assert!(r.intersects(&Rect {
            x: 100,
            y: 50,
            width: 100,
            height: 100
        }));
        assert!(!r.intersects(&Rect {
            x: 110,
            y: 10,
            width: 10,
            height: 10
        }));
    }

    #[test]
    fn wide_round_trip() {
        let w = wide("héllo");
        assert_eq!(*w.last().unwrap(), 0);
        assert_eq!(from_wide(&w), "héllo");
    }
}
