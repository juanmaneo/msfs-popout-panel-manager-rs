//! Keyboard and mouse input emulation (port of `InputEmulationManager.cs`).
//!
//! All synthesized events carry [`INJECT_TAG`] in `dwExtraInfo` so our own low-level hooks
//! can recognise (and ignore) them.

use super::window;
use super::Rect;
use std::time::Duration;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT, SendInput,
};

/// Marker written to `dwExtraInfo` of every event we inject ("POPM").
pub const INJECT_TAG: usize = 0x504F_504D;

pub const VK_LMENU: u16 = 0xA4;
pub const VK_RMENU: u16 = 0xA5;
pub const VK_LCONTROL: u16 = 0xA2;
pub const VK_RCONTROL: u16 = 0xA3;
pub const VK_RETURN: u16 = 0x0D;
pub const VK_TAB: u16 = 0x09;

fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn is_extended(vk: u16) -> bool {
    matches!(
        vk,
        VK_RMENU | VK_RCONTROL | 0x21..=0x28 | 0x2D | 0x2E | 0x5B | 0x5C
    )
}

fn send(inputs: &[INPUT]) {
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

pub fn key_event(vk: u16, up: bool) {
    let mut flags = if up { KEYEVENTF_KEYUP } else { 0 };
    if is_extended(vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INJECT_TAG,
            },
        },
    };
    send(&[input]);
}

pub fn key_down(vk: u16) {
    key_event(vk, false);
}

pub fn key_up(vk: u16) {
    key_event(vk, true);
}

fn mouse_event(flags: u32) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INJECT_TAG,
            },
        },
    };
    send(&[input]);
}

/// Left button press at the current cursor position.
pub fn left_down() {
    mouse_event(MOUSEEVENTF_LEFTDOWN);
}

pub fn left_up() {
    mouse_event(MOUSEEVENTF_LEFTUP);
}

pub fn left_click(x: i32, y: i32) {
    window::set_cursor_pos(x, y);
    sleep(300);
    left_down();
    sleep(200);
    left_up();
    sleep(200);
}

/// Prepare the sim window for popping out a panel at the given source point.
pub fn prepare_to_pop_out_panel(sim_hwnd: isize, app_hwnd: isize, x: i32, y: i32, turbo: bool) {
    window::set_foreground(sim_hwnd);
    sleep(if turbo { 0 } else { 250 });

    move_app_window_away_from_point(app_hwnd, x, y);

    left_click(x, y); // click outside the circle area to focus the game window

    window::set_cursor_pos(x, y);
    sleep(if turbo { 50 } else { 500 });
}

/// Pop out the panel under `(x, y)` with Alt (or Ctrl) + left click.
pub fn pop_out_panel(x: i32, y: i32, use_secondary_keys: bool, turbo: bool) {
    window::set_cursor_pos(x, y);
    if use_secondary_keys {
        key_down(VK_LCONTROL);
        key_down(VK_RCONTROL);
        sleep(if turbo { 0 } else { 500 });
        left_down();
        sleep(200);
        left_up();
        key_up(VK_RCONTROL);
        key_up(VK_LCONTROL);
        sleep(100);
        key_up(VK_RCONTROL); // resend to make sure Ctrl is up
        key_up(VK_LCONTROL);
    } else {
        key_down(VK_RMENU);
        sleep(if turbo { 0 } else { 500 });
        left_down();
        sleep(200);
        left_up();
        key_up(VK_RMENU);
        sleep(100);
        key_up(VK_RMENU); // resend to make sure Alt is up
    }
}

fn custom_view_key(binding: &str) -> u16 {
    0x30 + binding.trim().parse::<u16>().unwrap_or(0).min(9)
}

/// Save the current camera as custom view with Ctrl+Alt+<n>.
pub fn save_custom_view(sim_hwnd: isize, binding: &str) {
    let key = custom_view_key(binding);
    window::set_foreground(sim_hwnd);
    sleep(500);

    key_down(VK_LCONTROL);
    key_down(VK_LMENU);
    key_down(key);
    sleep(200);
    key_up(key);
    key_up(VK_LMENU);
    key_up(VK_LCONTROL);
    sleep(200);
}

/// Load a custom camera view with Alt+<n>.
pub fn load_custom_view(sim_hwnd: isize, binding: &str) {
    window::set_foreground(sim_hwnd);
    sleep(200);
    let key = custom_view_key(binding);

    key_down(VK_LMENU);
    key_down(key);
    sleep(200);
    key_up(key);
    key_up(VK_LMENU);
    sleep(200);
}

/// Alt+Enter on a pop out window toggles its full screen mode.
pub fn toggle_full_screen_panel(hwnd: isize) {
    window::set_foreground(hwnd);
    sleep(500);

    key_down(VK_LMENU);
    key_down(VK_RETURN);
    sleep(200);
    key_up(VK_RETURN);
    key_up(VK_LMENU);
    sleep(200);
}

/// Map a virtual NumPad button name to the virtual key it sends.
pub fn numpad_vk(name: &str) -> u16 {
    match name.to_uppercase().as_str() {
        "0" => 0x60,
        "1" => 0x61,
        "2" => 0x62,
        "3" => 0x63,
        "4" => 0x64,
        "5" => 0x65,
        "6" => 0x66,
        "7" => 0x67,
        "8" => 0x68,
        "9" => 0x69,
        "ADD" => 0x6B,
        "SUBTRACT" => 0x6D,
        "DIVIDE" => 0x6F,
        "MULTIPLY" => 0x6A,
        "TAB" => VK_TAB,
        "ENTER" => VK_RETURN,
        _ => 0x6E, // decimal
    }
}

pub fn numpad_click(sim_hwnd: isize, name: &str) {
    window::set_foreground(sim_hwnd);
    sleep(200);
    let vk = numpad_vk(name);
    key_down(vk);
    sleep(200);
    key_up(vk);
}

/// If the click point lies within our own window, move the window above the point so
/// it does not cover the panel that is about to be popped out.
fn move_app_window_away_from_point(app_hwnd: isize, x: i32, y: i32) {
    if app_hwnd == 0 {
        return;
    }
    let r: Rect = window::rect_dwm(app_hwnd);
    if r.contains(x, y) {
        let top = y - r.height - 50;
        window::move_window(app_hwnd, r.x, top, r.width, r.height);
        sleep(1000);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_view_keys() {
        assert_eq!(custom_view_key("0"), 0x30);
        assert_eq!(custom_view_key("9"), 0x39);
        assert_eq!(custom_view_key("x"), 0x30);
        assert_eq!(custom_view_key("42"), 0x39);
    }

    #[test]
    fn numpad_mapping() {
        assert_eq!(numpad_vk("7"), 0x67);
        assert_eq!(numpad_vk("Enter"), 0x0D);
        assert_eq!(numpad_vk("Decimal"), 0x6E);
        assert_eq!(numpad_vk("Add"), 0x6B);
    }
}
