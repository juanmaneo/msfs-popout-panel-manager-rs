//! Reacts to WinEvents of the pop out windows (port of `WindowEventManager.cs` and
//! `GameRefocusManager.cs`): keeps the stored coordinates in sync while a profile is
//! unlocked, enforces the layout while it is locked, and refocuses the game after a click
//! on a pop out.

use super::{Core, WinMsg};
use crate::hook_plan::{
    EVENT_OBJECT_STATECHANGE, EVENT_SYSTEM_CAPTUREEND, EVENT_SYSTEM_CAPTURESTART,
    EVENT_SYSTEM_MOVESIZEEND,
};
use crate::model::{PanelConfig, PanelHandle, PanelType};
use crate::touch::{is_refocus_press_allowed, is_refocus_release_allowed};
use crate::win::window::{self, SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, SW_SHOWNORMAL};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Receiver;
use std::time::Duration;

fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// What to do with a locked panel whose window state changed (`prev` = state before the
/// change, `now` = state after).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LockedAction {
    None,
    /// `ShowWindow(cmd)`
    Show(i32),
    /// Move the window back to the stored rectangle.
    MoveBack,
}

pub(super) fn locked_state_action(prev: i32, now: i32) -> LockedAction {
    match (now, prev) {
        (SW_SHOWMAXIMIZED, SW_SHOWNORMAL) => LockedAction::Show(9), // SW_RESTORE
        (SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED) => LockedAction::Show(SW_SHOWMINIMIZED),
        (SW_SHOWMINIMIZED, SW_SHOWNORMAL) => LockedAction::Show(9),
        (SW_SHOWMINIMIZED, SW_SHOWMAXIMIZED) | (SW_SHOWNORMAL, SW_SHOWMAXIMIZED) => {
            LockedAction::Show(SW_SHOWMAXIMIZED)
        }
        (SW_SHOWNORMAL, SW_SHOWMINIMIZED) => LockedAction::Show(SW_SHOWMINIMIZED),
        (SW_SHOWNORMAL, SW_SHOWNORMAL) => LockedAction::MoveBack,
        _ => LockedAction::None,
    }
}

impl Core {
    pub(super) fn run_win_event_worker(self: Arc<Self>, rx: Receiver<WinMsg>) {
        let mut prev_show_cmd: Option<i32> = None;
        let refocus = Arc::new(RefocusState::default());

        while let Ok(msg) = rx.recv() {
            match msg {
                WinMsg::Shutdown => break,
                WinMsg::Reset => prev_show_cmd = None,
                WinMsg::Event { event, hwnd } => {
                    self.handle_win_event(event, hwnd, &mut prev_show_cmd, &refocus)
                }
            }
        }
    }

    fn handle_win_event(
        &self,
        event: u32,
        hwnd: isize,
        prev_show_cmd: &mut Option<i32>,
        refocus: &Arc<RefocusState>,
    ) {
        let Some((panel, locked)) = self.read(|s| {
            let p = s.active_profile()?;
            Some((p.panel_by_handle(hwnd)?.clone(), p.is_locked))
        }) else {
            return;
        };

        match event {
            EVENT_OBJECT_STATECHANGE => {
                if !locked {
                    sleep(300);
                    self.update_panel_coordinates(&panel);
                    return;
                }

                // the pop out was closed
                let rect = window::rect_dwm(hwnd);
                if rect.width == 0 && rect.height == 0 {
                    self.set_panel_handle(panel.id, PanelHandle::Unset);
                    *prev_show_cmd = None;
                    return;
                }

                let now = window::show_cmd(hwnd);
                let Some(prev) = *prev_show_cmd else {
                    *prev_show_cmd = Some(now);
                    sleep(250);
                    return;
                };

                if panel.floating_panel.is_enabled && !panel.is_floating {
                    return;
                }

                match locked_state_action(prev, now) {
                    LockedAction::Show(cmd) => window::show_window(hwnd, cmd),
                    LockedAction::MoveBack => {
                        window::move_window(hwnd, panel.left, panel.top, panel.width, panel.height)
                    }
                    LockedAction::None => {}
                }
                *prev_show_cmd = None;
            }
            EVENT_SYSTEM_MOVESIZEEND => {
                if locked {
                    // move the window back where it belongs
                    window::move_window(hwnd, panel.left, panel.top, panel.width, panel.height);
                } else {
                    self.update_panel_coordinates(&panel);
                }
            }
            EVENT_SYSTEM_CAPTURESTART => self.refocus_mouse_down(&panel, refocus),
            EVENT_SYSTEM_CAPTUREEND => self.refocus_mouse_up(&panel, refocus),
            _ => {}
        }
    }

    fn set_panel_handle(&self, id: uuid::Uuid, handle: PanelHandle) {
        self.write(|s| {
            if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(id)) {
                c.panel_handle = handle;
            }
        });
    }

    /// Store the window's current rectangle in the profile (unlocked profiles follow the
    /// windows).
    fn update_panel_coordinates(&self, panel: &PanelConfig) {
        let PanelHandle::Hwnd(h) = panel.panel_handle else {
            return;
        };
        let rect = window::rect_dwm(h);

        if rect.width == 0 && rect.height == 0 {
            self.set_panel_handle(panel.id, PanelHandle::Unset);
            return;
        }

        self.write(|s| {
            let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(panel.id)) else {
                return;
            };
            // hidden floating panels keep their last visible position
            if c.floating_panel.is_enabled && !c.is_floating {
                return;
            }
            c.left = rect.x;
            c.top = rect.y;
            if c.panel_type == PanelType::HudBarWindow {
                return;
            }
            c.width = rect.width;
            c.height = rect.height;
        });
    }

    // --- refocus the game after clicking a pop out --------------------------------------------

    fn refocus_mouse_down(&self, panel: &PanelConfig, st: &Arc<RefocusState>) {
        if st.mouse_down.load(Ordering::SeqCst) {
            return;
        }
        let (x, y) = window::cursor_pos();
        // ignore presses on the title bar and the borders (resizing / dragging)
        if is_refocus_press_allowed(panel, x, y) {
            st.mouse_down.store(true, Ordering::SeqCst);
        }
    }

    fn refocus_mouse_up(&self, panel: &PanelConfig, st: &Arc<RefocusState>) {
        if !st.mouse_down.load(Ordering::SeqCst) {
            return;
        }
        let (delay, refocus_on, refocus_delay) = self.read(|s| {
            let r = &s.settings.refocus_setting.refocus_game_window;
            (
                s.settings.touch_setting.touch_down_up_delay,
                r.is_enabled,
                r.delay,
            )
        });
        sleep(delay.max(0) as u64);
        st.mouse_down.store(false, Ordering::SeqCst);

        let (_, y) = window::cursor_pos();
        if !is_refocus_release_allowed(panel, y) {
            return;
        }

        let ticket = st.ticket.fetch_add(1, Ordering::SeqCst) + 1;
        // touch panels refocus through the touch logic instead
        if ticket == st.ticket.load(Ordering::SeqCst)
            && refocus_on
            && panel.auto_game_refocus
            && !panel.touch_enabled
        {
            let st = st.clone();
            std::thread::spawn(move || {
                sleep((refocus_delay * 1000.0).max(0.0) as u64);
                if ticket != st.ticket.load(Ordering::SeqCst)
                    || st.mouse_down.load(Ordering::SeqCst)
                {
                    return;
                }
                if let Some(h) = window::msfs_game_window() {
                    window::set_foreground(h);
                    let r = window::rect_dwm(h);
                    window::set_cursor_pos(r.x + r.width / 2, r.y + r.height / 2);
                }
            });
        }
    }
}

#[derive(Default)]
pub(super) struct RefocusState {
    mouse_down: AtomicBool,
    ticket: AtomicU32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_window_state_transitions() {
        use LockedAction::*;
        assert_eq!(
            locked_state_action(SW_SHOWNORMAL, SW_SHOWMAXIMIZED),
            Show(9)
        );
        assert_eq!(
            locked_state_action(SW_SHOWMINIMIZED, SW_SHOWMAXIMIZED),
            Show(SW_SHOWMINIMIZED)
        );
        assert_eq!(
            locked_state_action(SW_SHOWNORMAL, SW_SHOWMINIMIZED),
            Show(9)
        );
        assert_eq!(
            locked_state_action(SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED),
            Show(SW_SHOWMAXIMIZED)
        );
        assert_eq!(
            locked_state_action(SW_SHOWMAXIMIZED, SW_SHOWNORMAL),
            Show(SW_SHOWMAXIMIZED)
        );
        assert_eq!(
            locked_state_action(SW_SHOWMINIMIZED, SW_SHOWNORMAL),
            Show(SW_SHOWMINIMIZED)
        );
        assert_eq!(locked_state_action(SW_SHOWNORMAL, SW_SHOWNORMAL), MoveBack);
        assert_eq!(
            locked_state_action(SW_SHOWMAXIMIZED, SW_SHOWMAXIMIZED),
            None
        );
    }
}
