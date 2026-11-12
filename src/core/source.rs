//! Panel source editing (port of `PanelSourceOrchestrator.cs`): choosing where on screen
//! each custom panel is popped out from, shown as coloured ring overlays.

use super::{Core, MessageKind};
use crate::flight_sim::{CAMERA_VIEW_CUSTOM_CAMERA, CAMERA_VIEW_HOME_COCKPIT_MODE};
use crate::model::{CameraType, FixedCameraConfig, PanelType, color_rgb};
use crate::win::hooks::HookCmd;
use crate::win::overlay::OverlaySpec;
use crate::win::{input, window};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// Fixed camera choices offered for a panel (`GetFixedCameraConfigs`).
pub fn fixed_camera_configs(instrument_count: i32) -> Vec<FixedCameraConfig> {
    let mut v = vec![
        FixedCameraConfig {
            id: 0,
            camera_type: CameraType::Cockpit,
            camera_index: 1,
            name: "Cockpit Pilot".into(),
        },
        FixedCameraConfig {
            id: 1,
            camera_type: CameraType::Cockpit,
            camera_index: 5,
            name: "Cockpit Copilot".into(),
        },
    ];
    for i in 0..instrument_count.max(0) {
        v.push(FixedCameraConfig {
            id: i + 2,
            camera_type: CameraType::Instrument,
            camera_index: i,
            name: format!("Instrument {}", i + 1),
        });
    }
    v
}

impl Core {
    /// Show (or move) the ring marking where `panel_id` is popped out from.
    fn show_overlay(&self, panel_id: Uuid, editable: bool) {
        let spec = self.read(|s| {
            let c = s.active_profile()?.panel(panel_id)?;
            if c.panel_type != PanelType::CustomPopout {
                return None;
            }
            Some(OverlaySpec {
                id: panel_id,
                x: c.panel_source.x?,
                y: c.panel_source.y?,
                rgb: color_rgb(c.panel_source.color.as_deref()),
                editable,
            })
        });
        if let Some(spec) = spec {
            self.hooks.send(HookCmd::ShowOverlay(spec));
        }
    }

    /// Show the marker of one panel only (selecting it for editing).
    pub fn show_panel_source_for_edit(&self, panel_id: Uuid) {
        self.hooks.send(HookCmd::CloseAllOverlays);
        let has_source = self.write(|s| {
            let p = s.active_profile_mut()?;
            for c in &mut p.panel_configs {
                c.is_editing_panel = false;
            }
            let c = p.panel_mut(panel_id)?;
            c.is_editing_panel = true;
            Some(c.has_panel_source())
        });
        if has_source == Some(true) {
            self.show_overlay(panel_id, true);
        }
    }

    /// The user pressed "set source" on a panel: the next click outside our window defines it.
    pub fn start_panel_selection(&self, panel_id: Uuid, legacy_camera: bool) {
        if !legacy_camera {
            self.show_panel_source_for_edit(panel_id);
        }
        self.write(|s| {
            let Some(p) = s.active_profile_mut() else {
                return;
            };
            if p.is_selecting_panel_source {
                return;
            }
            p.is_selecting_panel_source = true;
            if let Some(c) = p.panel_mut(panel_id) {
                if legacy_camera {
                    c.is_selected_panel_source = true;
                } else {
                    c.is_editing_panel = true;
                }
            }
            s.selecting_panel = Some(panel_id);
        });
    }

    /// Abort a pending "click to set the panel source".
    pub fn cancel_panel_selection(&self) {
        self.write(|s| {
            s.selecting_panel = None;
            if let Some(p) = s.active_profile_mut() {
                p.is_selecting_panel_source = false;
                for c in &mut p.panel_configs {
                    c.is_selected_panel_source = false;
                }
            }
        });
    }

    /// A left click was captured while selecting a panel source.
    pub(super) fn on_panel_source_click(&self, x: i32, y: i32) {
        let Some(panel_id) = self.read(|s| s.selecting_panel) else {
            return;
        };

        // clicks on our own window (e.g. the button itself) do not count
        let app = self.app_hwnd();
        if app == 0 || window::rect_dwm(app).contains(x, y) {
            return;
        }

        let auto_resize = self.write(|s| {
            s.selecting_panel = None;
            let auto_resize = s
                .settings
                .windowed_mode_setting
                .auto_resize_msfs_game_window;
            if let Some(p) = s.active_profile_mut() {
                if let Some(c) = p.panel_mut(panel_id) {
                    c.panel_source.x = Some(x);
                    c.panel_source.y = Some(y);
                    c.is_selected_panel_source = false;
                }
                p.is_selecting_panel_source = false;
            }
            auto_resize
        });

        self.show_overlay(panel_id, true);
        if auto_resize {
            self.save_msfs_game_window_config();
        }
        self.repaint();
    }

    /// An overlay ring was dragged to a new place.
    pub(super) fn on_overlay_moved(&self, id: Uuid, x: i32, y: i32) {
        self.write(|s| {
            if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(id)) {
                c.panel_source.x = Some(x);
                c.panel_source.y = Some(y);
                c.is_selected_panel_source = false;
            }
        });
        self.repaint();
    }

    pub(super) fn on_overlay_drag_started(&self, id: Uuid) {
        self.write(|s| {
            if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(id)) {
                c.is_selected_panel_source = true;
            }
        });
    }

    /// Remove a panel from the active profile.
    pub fn remove_panel(&self, panel_id: Uuid) {
        self.hooks.send(HookCmd::CloseOverlay(panel_id));
        self.write(|s| {
            s.selecting_panel = None;
            if let Some(p) = s.active_profile_mut() {
                p.current_move_resize_panel_id = Uuid::nil();
                p.is_selecting_panel_source = false;
                p.panel_configs.retain(|c| c.id != panel_id);
            }
        });
    }

    /// Toggle the "edit panel sources" mode of the active profile.
    pub fn toggle_edit_panel_sources(self: &Arc<Self>) {
        let editing = self.write(|s| {
            let p = s.active_profile_mut()?;
            p.is_editing_panel_source = !p.is_editing_panel_source;
            Some(p.is_editing_panel_source)
        });
        let core = self.clone();
        match editing {
            Some(true) => {
                std::thread::spawn(move || core.start_edit_panel_sources());
            }
            Some(false) => {
                std::thread::spawn(move || core.end_edit_panel_sources());
            }
            None => {}
        }
    }

    /// "Add panel": create a custom panel and start editing right away.
    pub fn add_panel_and_edit(self: &Arc<Self>) {
        let started = self.write(|s| {
            let p = s.active_profile_mut()?;
            p.add_custom_panel();
            let already = p.is_editing_panel_source;
            p.is_editing_panel_source = true;
            Some(already)
        });
        if started == Some(false) {
            let core = self.clone();
            std::thread::spawn(move || core.start_edit_panel_sources());
        }
    }

    fn start_edit_panel_sources(&self) {
        if self.write(|s| std::mem::replace(&mut s.editing_lock, true)) {
            return;
        }
        self.turn_off_track_ir();

        let legacy = self.read(|s| {
            s.active_profile()
                .is_some_and(|p| p.is_used_legacy_camera_system)
        });
        if legacy {
            self.start_edit_legacy_camera();
        }
    }

    fn start_edit_legacy_camera(&self) {
        let messages = self.read(|s| s.settings.pop_out_setting.enable_pop_out_messages);
        if messages {
            self.write(|s| s.ui.message = Some(MessageKind::Regular));
        }
        self.status.set_enabled(true);
        self.status.clear();
        self.status.write(
            "Loading camera view. Please wait......",
            crate::status::StatusMessageType::Info,
        );
        self.repaint();

        let (auto_resize, game_window, auto_panning, key, zoom_factor) = self.read(|s| {
            let p = s.active_profile();
            (
                s.settings
                    .windowed_mode_setting
                    .auto_resize_msfs_game_window,
                p.map(|p| p.msfs_game_window_config.clone())
                    .unwrap_or_default(),
                s.settings.pop_out_setting.auto_panning.is_enabled,
                s.settings.pop_out_setting.auto_panning.key_binding.clone(),
                p.map(|p| p.panel_source_cockpit_zoom_factor).unwrap_or(50),
            )
        });

        if auto_resize {
            window::set_msfs_game_window_location(&game_window);
        }

        if auto_panning {
            if self.read(|s| s.sim.camera_view_type_and_index1) == CAMERA_VIEW_HOME_COCKPIT_MODE {
                self.set_cockpit_camera_zoom_level(zoom_factor);
            } else {
                self.load_custom_view_for_editing(&key);
                self.set_cockpit_camera_zoom_level(50);
            }
            let app = self.app_hwnd();
            if app != 0 {
                window::bring_to_foreground(app);
            }
        }

        let with_source: Vec<Uuid> = self.read(|s| {
            s.active_profile()
                .map(|p| {
                    p.panel_configs
                        .iter()
                        .filter(|c| c.has_panel_source())
                        .map(|c| c.id)
                        .collect()
                })
                .unwrap_or_default()
        });
        for id in with_source {
            self.show_overlay(id, true);
        }

        sleep(500);
        self.status.set_enabled(false);
        self.write(|s| s.ui.message = None);
        self.repaint();
    }

    fn load_custom_view_for_editing(&self, key: &str) {
        for _ in 0..3 {
            if let Some(h) = window::msfs_game_window() {
                input::load_custom_view(h, key);
            }
            sleep(750);
            if self.read(|s| s.sim.camera_view_type_and_index1) == CAMERA_VIEW_CUSTOM_CAMERA {
                break;
            }
        }
    }

    fn end_edit_panel_sources(&self) {
        if !self.read(|s| s.editing_lock) {
            return;
        }

        let legacy = self.read(|s| {
            s.active_profile()
                .is_some_and(|p| p.is_used_legacy_camera_system)
        });
        if legacy {
            self.end_edit_legacy_camera();
        } else {
            self.reset_camera_view();
        }

        self.hooks.send(HookCmd::CloseAllOverlays);
        self.write(|s| {
            s.editing_lock = false;
            s.selecting_panel = None;
            if let Some(p) = s.active_profile_mut() {
                for c in &mut p.panel_configs {
                    c.is_editing_panel = false;
                    c.is_selected_panel_source = false;
                }
                p.is_selecting_panel_source = false;
            }
        });

        let app = self.app_hwnd();
        if app != 0 {
            window::bring_to_foreground(app);
        }
        self.turn_on_track_ir();
        self.repaint();
    }

    fn end_edit_legacy_camera(&self) {
        let (auto_panning, auto_resize, key) = self.read(|s| {
            (
                s.settings.pop_out_setting.auto_panning.is_enabled,
                s.settings
                    .windowed_mode_setting
                    .auto_resize_msfs_game_window,
                s.settings.pop_out_setting.auto_panning.key_binding.clone(),
            )
        });

        if auto_panning {
            if auto_resize {
                self.save_msfs_game_window_config();
            }

            if self.read(|s| s.sim.camera_view_type_and_index1) == CAMERA_VIEW_HOME_COCKPIT_MODE {
                let zoom = self.read(|s| s.sim.cockpit_camera_zoom);
                self.write(|s| {
                    if let Some(p) = s.active_profile_mut() {
                        p.panel_source_cockpit_zoom_factor = zoom;
                    }
                });
            } else {
                // without setting the zoom everything ends up a few pixels off
                let zoom = self.read(|s| s.sim.cockpit_camera_zoom);
                for _ in 0..3 {
                    self.set_cockpit_camera_zoom_level(zoom);
                    sleep(750);
                    if self.read(|s| s.sim.cockpit_camera_zoom) == zoom {
                        break;
                    }
                }
                if let Some(h) = window::msfs_game_window() {
                    input::save_custom_view(h, &key);
                }
            }
        }
        sleep(500); // wait for the custom view to be saved
    }

    /// Preview the fixed camera of a panel in the simulator.
    pub fn set_camera_for_panel(self: &Arc<Self>, panel_id: Uuid) {
        if !self.read(|s| s.sim.is_in_cockpit) {
            return;
        }
        let Some(cfg) = self.read(|s| {
            s.active_profile()
                .and_then(|p| p.panel(panel_id))
                .map(|c| c.fixed_camera_config.clone())
        }) else {
            return;
        };
        // the camera change takes a few hundred milliseconds; do not block the GUI
        let me = self.clone();
        std::thread::spawn(move || {
            if cfg.camera_type == CameraType::Cockpit {
                me.reset_camera_view();
                sleep(250);
            }
            me.set_fixed_camera(cfg.camera_type, cfg.camera_index);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_camera_list_matches_instrument_count() {
        let v = fixed_camera_configs(3);
        assert_eq!(v.len(), 5);
        assert_eq!(v[0].name, "Cockpit Pilot");
        assert_eq!(v[1].camera_index, 5);
        assert_eq!(v[4].name, "Instrument 3");
        assert_eq!(v[4].id, 4);
        assert_eq!(v[4].camera_type, CameraType::Instrument);
        assert_eq!(fixed_camera_configs(0).len(), 2);
        assert_eq!(fixed_camera_configs(-1).len(), 2);
    }
}
