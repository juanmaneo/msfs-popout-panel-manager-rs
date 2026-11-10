//! Panel source marker windows: a small, topmost, per-pixel-alpha window drawing a coloured
//! ring where a panel will be popped out from. In edit mode the ring can be dragged.
//!
//! All overlay windows must be created, pumped and destroyed on the hook thread.

use super::hooks::{HookEvent, emit};
use super::wide;
use crate::overlay_raster::{self, CENTER_OFFSET, SIZE};
use std::sync::Once;
use uuid::Uuid;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE as WinSize, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC,
    SelectObject,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA,
    GetWindowLongPtrW, GetWindowRect, HTCAPTION, IDC_ARROW, LoadCursorW, MA_NOACTIVATE,
    RegisterClassW, SW_SHOWNOACTIVATE, SetWindowLongPtrW, ShowWindow, ULW_ALPHA,
    UpdateLayeredWindow, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_MOUSEACTIVATE, WM_NCCREATE,
    WM_NCDESTROY, WM_NCHITTEST, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const CLASS_NAME: &str = "POPMPanelSourceOverlay";

#[derive(Debug, Clone)]
pub struct OverlaySpec {
    pub id: Uuid,
    /// Screen position of the panel source point (the ring is centred on it).
    pub x: i32,
    pub y: i32,
    pub rgb: u32,
    pub editable: bool,
}

struct OverlayData {
    id: Uuid,
    editable: bool,
}

static REGISTER: Once = Once::new();

fn register_class() {
    REGISTER.call_once(|| unsafe {
        let class = wide(CLASS_NAME);
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(overlay_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(std::ptr::null()),
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
    });
}

unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayData;
            if !p.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(p));
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_NCHITTEST => {
            let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const OverlayData;
            if !p.is_null() && (*p).editable {
                HTCAPTION as LRESULT
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_ENTERSIZEMOVE => {
            let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const OverlayData;
            if !p.is_null() {
                emit(HookEvent::OverlayDragStarted((*p).id));
            }
            0
        }
        WM_EXITSIZEMOVE => {
            let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const OverlayData;
            if !p.is_null() {
                let mut r = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                GetWindowRect(hwnd, &mut r);
                emit(HookEvent::OverlayMoved {
                    id: (*p).id,
                    x: r.left + CENTER_OFFSET,
                    y: r.top + CENTER_OFFSET,
                });
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Paint the ring into the layered window.
unsafe fn paint(hwnd: HWND, rgb: u32, editable: bool) {
    let screen_dc = GetDC(std::ptr::null_mut());
    let mem_dc = CreateCompatibleDC(screen_dc);

    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: SIZE as i32,
        biHeight: -(SIZE as i32), // top-down
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        biSizeImage: 0,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };

    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let dib = CreateDIBSection(
        mem_dc,
        &bmi,
        DIB_RGB_COLORS,
        &mut bits,
        std::ptr::null_mut(),
        0,
    );
    if !dib.is_null() && !bits.is_null() {
        let pixels = overlay_raster::render(rgb, editable);
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u32, pixels.len());

        let old = SelectObject(mem_dc, dib);
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let size = WinSize {
            cx: SIZE as i32,
            cy: SIZE as i32,
        };
        let src = POINT { x: 0, y: 0 };
        UpdateLayeredWindow(
            hwnd,
            screen_dc,
            std::ptr::null(),
            &size,
            mem_dc,
            &src,
            0,
            &blend,
            ULW_ALPHA,
        );
        SelectObject(mem_dc, old);
    }
    if !dib.is_null() {
        DeleteObject(dib);
    }
    DeleteDC(mem_dc);
    ReleaseDC(std::ptr::null_mut(), screen_dc);
}

/// Create and show an overlay. Must be called on the hook thread.
pub fn create(spec: &OverlaySpec) -> Option<isize> {
    register_class();
    unsafe {
        let class = wide(CLASS_NAME);
        let title = wide("POPM Panel Source");
        let data = Box::into_raw(Box::new(OverlayData {
            id: spec.id,
            editable: spec.editable,
        }));
        let mut ex = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        if !spec.editable {
            ex |= WS_EX_TRANSPARENT;
        }
        let hwnd = CreateWindowExW(
            ex,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            spec.x - CENTER_OFFSET,
            spec.y - CENTER_OFFSET,
            SIZE as i32,
            SIZE as i32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            data as *const core::ffi::c_void,
        );
        if hwnd.is_null() {
            drop(Box::from_raw(data));
            return None;
        }
        paint(hwnd, spec.rgb, spec.editable);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        Some(hwnd as isize)
    }
}

pub fn destroy(hwnd: isize) {
    unsafe {
        DestroyWindow(hwnd as HWND);
    }
}
