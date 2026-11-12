//! Touch support (port of `TouchEventManager.cs`).
//!
//! Touching a pop out window produces mouse events that MSFS pop outs do not handle well.
//! The low-level mouse hook swallows those events (identified by the touch signature in
//! `dwExtraInfo`) and replays them as clean synthesized mouse clicks from worker threads.
//! Events we synthesize ourselves carry [`INJECT_TAG`] and always pass through.

use super::input::{self, INJECT_TAG};
use super::window;
use crate::model::PanelType;
use crate::touch::{TouchPanel, find_touch_panel, is_touch_event};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

/// Everything the mouse hook needs; replaced wholesale by the core thread.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TouchSnapshot {
    pub panels: Vec<TouchPanel>,
    pub touch_down_up_delay_ms: u64,
    pub refocus_enabled: bool,
    pub refocus_delay_ms: u64,
}

static SNAPSHOT: RwLock<TouchSnapshot> = RwLock::new(TouchSnapshot {
    panels: Vec::new(),
    touch_down_up_delay_ms: 0,
    refocus_enabled: false,
    refocus_delay_ms: 0,
});

static TOUCH_DOWN_COMPLETED: AtomicBool = AtomicBool::new(true);
static TOUCH_UP_COMPLETED: AtomicBool = AtomicBool::new(true);
static DRAGGED: AtomicBool = AtomicBool::new(false);
static REFOCUS_INDEX: AtomicU32 = AtomicU32::new(0);

pub fn set_snapshot(s: TouchSnapshot) {
    *SNAPSHOT.write().unwrap_or_else(|e| e.into_inner()) = s;
}

/// Forget in-flight touch state (when the hook is removed).
pub fn reset() {
    TOUCH_DOWN_COMPLETED.store(true, Ordering::SeqCst);
    TOUCH_UP_COMPLETED.store(true, Ordering::SeqCst);
    DRAGGED.store(false, Ordering::SeqCst);
}

fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn sim_center() -> Option<(isize, i32, i32)> {
    let h = window::msfs_game_window()?;
    let r = window::rect_dwm(h);
    Some((h, r.x + r.width / 2, r.y + r.height / 2))
}

/// Decide whether the hook should swallow a mouse event. Must return quickly.
pub fn on_mouse_event(msg: u32, x: i32, y: i32, extra_info: usize) -> bool {
    if extra_info == INJECT_TAG {
        return false;
    }

    if !is_touch_event(extra_info) {
        // regular mouse clicks are ignored while a touch is being replayed
        return matches!(msg, WM_LBUTTONDOWN | WM_LBUTTONUP)
            && !TOUCH_DOWN_COMPLETED.load(Ordering::SeqCst);
    }

    let snap = SNAPSHOT.read().unwrap_or_else(|e| e.into_inner());
    let Some(panel) = find_touch_panel(&snap.panels, x, y).cloned() else {
        return false;
    };
    let delay = snap.touch_down_up_delay_ms;
    let refocus_enabled = snap.refocus_enabled;
    let refocus_delay = snap.refocus_delay_ms;
    drop(snap);

    match msg {
        WM_RBUTTONDOWN | WM_RBUTTONUP => true,
        WM_LBUTTONDOWN => {
            if TOUCH_DOWN_COMPLETED.load(Ordering::SeqCst)
                && TOUCH_UP_COMPLETED.load(Ordering::SeqCst)
            {
                REFOCUS_INDEX.fetch_add(1, Ordering::SeqCst);
                if panel.panel_type == PanelType::RefocusDisplay {
                    return true;
                }
                TOUCH_DOWN_COMPLETED.store(false, Ordering::SeqCst);
                TOUCH_UP_COMPLETED.store(false, Ordering::SeqCst);

                std::thread::spawn(move || {
                    window::set_cursor_pos(x, y);
                    input::left_up(); // focus the window
                    sleep_ms(delay + 10);
                    input::left_down();
                    sleep_ms(delay + 25);
                    TOUCH_DOWN_COMPLETED.store(true, Ordering::SeqCst);
                });
            }
            true
        }
        WM_LBUTTONUP => {
            if panel.panel_type == PanelType::RefocusDisplay {
                std::thread::spawn(move || {
                    TOUCH_DOWN_COMPLETED.store(true, Ordering::SeqCst);
                    DRAGGED.store(false, Ordering::SeqCst);
                    TOUCH_UP_COMPLETED.store(true, Ordering::SeqCst);

                    if refocus_enabled && panel.auto_game_refocus {
                        let idx = REFOCUS_INDEX.load(Ordering::SeqCst);
                        sleep_ms(refocus_delay);
                        if idx == REFOCUS_INDEX.load(Ordering::SeqCst)
                            && let Some((_, cx, cy)) = sim_center() {
                                input::left_click(cx, cy);
                            }
                    }
                });
            } else {
                std::thread::spawn(move || {
                    let start = Instant::now();
                    while !TOUCH_DOWN_COMPLETED.load(Ordering::SeqCst)
                        && start.elapsed() < Duration::from_millis(250)
                    {
                        std::thread::sleep(Duration::from_millis(1));
                    }

                    window::set_cursor_pos(x, y);
                    if DRAGGED.load(Ordering::SeqCst) {
                        // needed for PMS GTN750
                        input::left_up();
                        sleep_ms(delay + 25);
                    }
                    input::left_up();

                    DRAGGED.store(false, Ordering::SeqCst);
                    TOUCH_DOWN_COMPLETED.store(true, Ordering::SeqCst);
                    TOUCH_UP_COMPLETED.store(true, Ordering::SeqCst);

                    if refocus_enabled && panel.auto_game_refocus {
                        let idx = REFOCUS_INDEX.load(Ordering::SeqCst);
                        sleep_ms(refocus_delay);
                        if idx == REFOCUS_INDEX.load(Ordering::SeqCst)
                            && let Some((h, cx, cy)) = sim_center() {
                                window::set_foreground(h);
                                window::set_cursor_pos(cx, cy);
                            }
                    }
                });
            }
            true
        }
        WM_MOUSEMOVE
            if !TOUCH_DOWN_COMPLETED.load(Ordering::SeqCst) => {
                DRAGGED.store(true, Ordering::SeqCst);
                true
            }
        _ => false,
    }
}
