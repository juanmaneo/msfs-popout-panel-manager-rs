//! Panel configuration while popped out (port of `PanelConfigurationOrchestrator.cs` plus the
//! keyboard handling of `KeyboardOrchestrator.cs` and the arrow-key move/resize of
//! `PopOutPanelConfigCardViewModel.cs`).

use super::{Core, KeystrokeTarget};
use crate::keybinding::{
    CaptureResult, KeyCapture, MoveResizeAction, move_resize_action, vk_to_key_name,
};
use crate::model::{PanelHandle, PanelType};
use crate::win::hooks::HookEvent;
use crate::win::{input, window};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// Which property of a panel changed (the new value is already stored in the profile).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelProp {
    PanelName,
    Left,
    Top,
    Width,
    Height,
    AlwaysOnTop,
    HideTitlebar,
    FullScreen,
    TouchEnabled,
    AutoGameRefocus,
    /// Floating panel settings; only persisted, nothing to apply.
    FloatingPanel,
}

impl Core {
    /// Apply a changed panel property to the live pop out window.
    /// Runs on its own thread: window operations sleep between steps.
    pub fn panel_property_updated(self: &Arc<Self>, panel_id: Uuid, prop: PanelProp) {
        let core = self.clone();
        std::thread::spawn(move || core.apply_panel_property(panel_id, prop));
    }

    fn apply_panel_property(&self, panel_id: Uuid, prop: PanelProp) {
        let Some(c) = self.read(|s| {
            let p = s.active_profile()?;
            if p.is_locked || !p.is_popped_out {
                return None;
            }
            p.panel(panel_id).cloned()
        }) else {
            return;
        };
        let PanelHandle::Hwnd(h) = c.panel_handle else {
            return;
        };
        if c.panel_type == PanelType::RefocusDisplay {
            return;
        }

        match prop {
            PanelProp::FullScreen => {
                input::toggle_full_screen_panel(h);
                if c.full_screen {
                    self.set_host_monitor(panel_id);
                } else {
                    window::move_window(h, c.left, c.top, c.width, c.height);
                    self.write(|s| {
                        if let Some(p) = s.active_profile_mut().and_then(|p| p.panel_mut(panel_id))
                        {
                            p.full_screen_monitor = None;
                        }
                    });
                }
            }
            PanelProp::PanelName => {
                if c.panel_type == PanelType::CustomPopout && !c.panel_name.contains("(Custom)") {
                    window::set_window_text(h, &format!("{} (Custom)", c.panel_name));
                }
            }
            _ if c.full_screen => {}
            PanelProp::Left | PanelProp::Top => {
                window::move_window(h, c.left, c.top, c.width, c.height);
            }
            PanelProp::Width | PanelProp::Height => {
                if c.panel_type == PanelType::HudBarWindow {
                    return;
                }
                if c.hide_titlebar {
                    window::apply_hide_title_bar(h, false);
                    sleep(100);
                }
                window::move_window(h, c.left, c.top, c.width, c.height);
                if c.hide_titlebar {
                    sleep(100);
                    window::apply_hide_title_bar(h, true);
                    sleep(100);
                }
            }
            PanelProp::AlwaysOnTop => window::apply_always_on_top(h, c.always_on_top),
            PanelProp::HideTitlebar => window::apply_hide_title_bar(h, c.hide_titlebar),
            // the hooks follow from the panel flags (see `reconcile_hooks`)
            PanelProp::TouchEnabled | PanelProp::AutoGameRefocus | PanelProp::FloatingPanel => {}
        }
    }

    // --- keyboard -------------------------------------------------------------------------

    pub(super) fn handle_hook_event(self: &Arc<Self>, e: HookEvent, capture: &mut KeyCapture) {
        match e {
            HookEvent::Key {
                vk,
                down,
                ctrl,
                shift,
            } => {
                if down {
                    if let Some(name) = vk_to_key_name(vk) {
                        capture.key_down(name);
                    }
                } else {
                    self.on_key_up_for_move_resize(vk, ctrl, shift);
                    if let CaptureResult::Completed(binding) = capture.key_up() {
                        self.on_binding_detected(binding);
                    }
                }
            }
            HookEvent::MouseLeftDown { x, y } => self.on_panel_source_click(x, y),
            HookEvent::OverlayDragStarted(id) => self.on_overlay_drag_started(id),
            HookEvent::OverlayMoved { id, x, y } => self.on_overlay_moved(id, x, y),
            HookEvent::WinEvent { .. } => {} // routed to the window event worker
        }
    }

    /// Arrow keys move/resize the panel that is in keyboard move/resize mode.
    fn on_key_up_for_move_resize(self: &Arc<Self>, vk: u32, ctrl: bool, shift: bool) {
        let Some(action) = move_resize_action(vk, ctrl, shift) else {
            return;
        };
        let target = self.write(|s| {
            let p = s.active_profile_mut()?;
            let id = p.current_move_resize_panel_id;
            if id.is_nil() {
                return None;
            }
            let c = p.panel_mut(id)?;
            let prop = match action {
                MoveResizeAction::Top(d) => {
                    c.top += d;
                    PanelProp::Top
                }
                MoveResizeAction::Left(d) => {
                    c.left += d;
                    PanelProp::Left
                }
                MoveResizeAction::Height(d) => {
                    c.height += d;
                    PanelProp::Height
                }
                MoveResizeAction::Width(d) => {
                    c.width += d;
                    PanelProp::Width
                }
            };
            Some((id, prop))
        });
        if let Some((id, prop)) = target {
            self.panel_property_updated(id, prop);
        }
    }

    /// A key chord was released. Used to record shortcuts, trigger the pop out and toggle
    /// floating panels.
    fn on_binding_detected(self: &Arc<Self>, binding: String) {
        // recording a shortcut takes precedence over everything else
        let recorded = self.write(|s| match s.ui.keystroke.take() {
            Some(KeystrokeTarget::Panel(id)) => {
                if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(id)) {
                    c.floating_panel.keyboard_binding = Some(binding.clone());
                    c.floating_panel.is_detecting_keystroke = false;
                }
                true
            }
            Some(KeystrokeTarget::Shortcut) => {
                s.settings
                    .keyboard_shortcut_setting
                    .pop_out_keyboard_binding = Some(binding.clone());
                s.settings.keyboard_shortcut_setting.is_detecting_keystroke = false;
                true
            }
            None => false,
        });
        if recorded {
            self.repaint();
            return;
        }

        let (flight, shortcut_on, shortcut) = self.read(|s| {
            (
                s.sim.is_flight_started,
                s.settings.keyboard_shortcut_setting.is_enabled,
                s.settings
                    .keyboard_shortcut_setting
                    .pop_out_keyboard_binding
                    .clone(),
            )
        });
        if !flight {
            return;
        }

        if shortcut_on && shortcut.as_deref() == Some(binding.as_str()) {
            self.manual_pop_out();
        }
        self.toggle_float_panel(&binding);
    }

    /// Show/hide floating panels bound to `binding`.
    fn toggle_float_panel(self: &Arc<Self>, binding: &str) {
        let panels: Vec<(Uuid, isize, bool)> = self.read(|s| {
            let Some(p) = s.active_profile() else {
                return Vec::new();
            };
            p.panel_configs
                .iter()
                .filter(|c| {
                    c.floating_panel.is_enabled
                        && c.floating_panel.keyboard_binding.as_deref() == Some(binding)
                        && matches!(
                            c.panel_type,
                            PanelType::CustomPopout | PanelType::BuiltInPopout
                        )
                })
                .filter_map(|c| match c.panel_handle {
                    PanelHandle::Hwnd(h) => Some((c.id, h, c.is_floating)),
                    _ => None,
                })
                .collect()
        });
        if panels.is_empty() {
            return;
        }

        let core = self.clone();
        std::thread::spawn(move || {
            for (id, h, floating) in panels {
                core.write(|s| {
                    if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(id)) {
                        c.is_floating = !floating;
                    }
                });
                if floating {
                    window::minimize_window(h);
                } else {
                    window::restore_window(h);
                    window::apply_always_on_top(h, true);
                }
            }
        });
    }

    // --- recording shortcuts ---------------------------------------------------------------

    /// Record the next key chord as the floating panel toggle key of a panel.
    pub fn start_detect_keystroke(&self, panel_id: Uuid) {
        self.write(|s| {
            s.settings.keyboard_shortcut_setting.is_detecting_keystroke = false;
            if let Some(p) = s.active_profile_mut() {
                for c in &mut p.panel_configs {
                    c.floating_panel.is_detecting_keystroke = c.id == panel_id;
                }
            }
            s.ui.keystroke = Some(KeystrokeTarget::Panel(panel_id));
        });
    }

    /// Record the next key chord as the "start pop out" shortcut.
    pub fn start_detect_shortcut(&self) {
        self.write(|s| {
            if let Some(p) = s.active_profile_mut() {
                for c in &mut p.panel_configs {
                    c.floating_panel.is_detecting_keystroke = false;
                }
            }
            s.settings.keyboard_shortcut_setting.is_detecting_keystroke = true;
            s.ui.keystroke = Some(KeystrokeTarget::Shortcut);
        });
    }

    pub fn cancel_detect_keystroke(&self) {
        self.write(|s| {
            s.ui.keystroke = None;
            s.settings.keyboard_shortcut_setting.is_detecting_keystroke = false;
            if let Some(p) = s.active_profile_mut() {
                for c in &mut p.panel_configs {
                    c.floating_panel.is_detecting_keystroke = false;
                }
            }
        });
    }

    /// Enter/leave keyboard move/resize mode for a panel (arrow keys; Shift = fine,
    /// Ctrl = resize).
    pub fn set_move_resize(&self, panel_id: Uuid, on: bool) {
        self.write(|s| {
            let Some(p) = s.active_profile_mut() else {
                return;
            };
            if on {
                for c in &mut p.panel_configs {
                    c.is_editing_panel = c.id == panel_id;
                }
                p.current_move_resize_panel_id = panel_id;
            } else {
                if let Some(c) = p.panel_mut(panel_id) {
                    c.is_editing_panel = false;
                }
                p.current_move_resize_panel_id = Uuid::nil();
            }
        });
    }

    /// The GUI created the window of a panel (HUD bar / NumPad): register its handle.
    pub fn set_panel_window_handle(&self, panel_id: Uuid, hwnd: isize) {
        self.write(|s| {
            if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(panel_id)) {
                c.panel_handle = PanelHandle::Hwnd(hwnd);
            }
        });
    }
}
