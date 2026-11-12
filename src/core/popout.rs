//! The pop out workflow (port of `PanelPopOutOrchestrator.cs`).
//!
//! Everything here runs on a worker thread and uses plenty of sleeps: MSFS needs time to
//! react to the emulated input. See the module docs of [`super`] for the locking rules.

use super::{Core, MessageKind};
use crate::flight_sim::{CAMERA_VIEW_CUSTOM_CAMERA, CAMERA_VIEW_HOME_COCKPIT_MODE};
use crate::model::{AfterPopOutCameraViewType, PanelConfig, PanelHandle, PanelType};
use crate::status::StatusMessageType;
use crate::win::{input, window};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const READY_TO_FLY_BUTTON_APPEARANCE_DELAY: u64 = 2000;

fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

impl Core {
    /// Start the pop out for the active profile (button or keyboard shortcut).
    pub fn manual_pop_out(self: &Arc<Self>) {
        if !self.try_begin_pop_out(true) {
            return;
        }
        let core = self.clone();
        std::thread::spawn(move || {
            core.on_pop_out_started();
            core.core_steps();
        });
    }

    /// Pop out automatically after a flight started (if a profile is bound to the aircraft).
    pub(super) fn auto_pop_out(self: &Arc<Self>) {
        let core = self.clone();
        std::thread::spawn(move || {
            core.auto_switch_profile();

            let eligible = core.read(|s| {
                let Some(p) = s.active_profile() else {
                    return false;
                };
                let aircraft = s.sim.aircraft_name.as_deref().unwrap_or("");
                if !p.aircraft_bindings.iter().any(|b| b == aircraft) {
                    return false;
                }
                if p.panel_configs.is_empty() {
                    return false;
                }
                // every custom panel must have a panel source
                p.panel_configs
                    .iter()
                    .filter(|c| c.panel_type == PanelType::CustomPopout)
                    .all(|c| c.has_panel_source())
            });
            if !eligible || !core.try_begin_pop_out(false) {
                return;
            }

            core.status_enabled_wait_for_ready_to_fly();
            core.on_pop_out_started();
            core.core_steps();
        });
    }

    /// Mark a pop out as running. `require_cockpit` is checked for manual pop outs.
    fn try_begin_pop_out(&self, require_cockpit: bool) -> bool {
        self.write(|s| {
            if s.pop_out_in_progress || s.active_profile().is_none() {
                return false;
            }
            if require_cockpit && !s.sim.is_in_cockpit {
                return false;
            }
            s.pop_out_in_progress = true;
            true
        })
    }

    fn status_enabled_wait_for_ready_to_fly(&self) {
        // the "ready to fly" button plugin needs time to appear, then the cockpit view settles
        sleep(READY_TO_FLY_BUTTON_APPEARANCE_DELAY);
        let delay = self.read(|s| s.settings.auto_pop_out_setting.ready_to_fly_delay);
        if delay > 0 {
            sleep(delay as u64 * 1000);
        }
    }

    /// "Close Pop Outs" button.
    pub fn close_pop_out(&self) {
        self.close_all_pop_outs();
        self.write(|s| {
            if let Some(p) = s.active_profile_mut() {
                p.set_popped_out(false);
            }
        });
    }

    fn on_pop_out_started(&self) {
        let (minimize, messages) = self.read(|s| {
            (
                s.settings.pop_out_setting.minimize_during_pop_out,
                s.settings.pop_out_setting.enable_pop_out_messages,
            )
        });
        if minimize {
            let h = self.app_hwnd();
            if h != 0 && !window::is_minimized(h) {
                self.write(|s| s.ui.minimized_for_pop_out = true);
                window::minimize_window_async(h);
            }
        }
        if messages {
            self.write(|s| s.ui.message = Some(MessageKind::PopOut));
        }
        self.repaint();
    }

    fn on_pop_out_completed(&self) {
        sleep(1000);
        let (minimize_after, minimized_for, always_on_top) = self.write(|s| {
            s.ui.message = None;
            let m = std::mem::take(&mut s.ui.minimized_for_pop_out);
            (
                s.settings.pop_out_setting.minimize_after_pop_out,
                m,
                s.settings.general_setting.always_on_top,
            )
        });
        let h = self.app_hwnd();
        if h != 0 {
            if minimize_after {
                window::minimize_window_async(h);
            } else if minimized_for || !always_on_top {
                window::bring_to_foreground(h);
            }
        }
        self.repaint();
    }

    fn core_steps(&self) {
        let abort = self.read(|s| match s.active_profile() {
            None => true,
            Some(p) => p.is_editing_panel_source || p.has_unidentified_panel_source(),
        });
        if abort {
            self.write(|s| s.pop_out_in_progress = false);
            self.on_pop_out_completed();
            return;
        }

        self.status.set_enabled(true);
        self.status.clear();
        self.status.write_line(
            "Pop out in progress. Please wait and do not move your mouse.",
            StatusMessageType::Info,
        );

        self.step_pop_out_prep();

        // *** THIS MUST BE DONE FIRST: the built-in panels are what we compare against later
        let built_in = window::windows_by_panel_type(&[PanelType::BuiltInPopout]);

        self.step_add_custom_panels(&built_in);
        self.step_add_built_in_panels(built_in);
        self.step_add_hud_bar();
        self.step_add_num_pad();
        self.setup_refocus_display();
        self.step_apply_panel_config();
        self.step_post_pop_out();

        self.write(|s| s.pop_out_in_progress = false);
        self.on_pop_out_completed();
        self.status.set_enabled(false);
    }

    // --- steps -------------------------------------------------------------------------

    fn step_pop_out_prep(&self) {
        self.write(|s| {
            s.selecting_panel = None;
            s.editing_lock = false;
            if let Some(p) = s.active_profile_mut() {
                p.reset_runtime();
            }
        });
        self.close_all_pop_outs();
        self.close_all_panel_sources();
    }

    fn step_add_custom_panels(&self, built_in: &[isize]) {
        if !self.read(|s| s.active_profile().is_some_and(|p| p.has_custom_panels())) {
            return;
        }

        if !self.step_pre_custom_panel_pop_out() {
            self.write(|s| {
                if let Some(p) = s.active_profile_mut() {
                    for c in p
                        .panel_configs
                        .iter_mut()
                        .filter(|c| c.panel_type == PanelType::CustomPopout)
                    {
                        c.panel_handle = PanelHandle::Failed;
                    }
                }
            });
            return;
        }

        self.step_custom_panel_pop_out(built_in);
        self.step_post_custom_panel_pop_out();
    }

    fn step_pre_custom_panel_pop_out(&self) -> bool {
        let (
            auto_resize,
            power_required,
            auto_panning,
            key,
            legacy,
            zoom_factor,
            turbo,
            game_window,
        ) = self.read(|s| {
            let p = s.active_profile();
            (
                s.settings
                    .windowed_mode_setting
                    .auto_resize_msfs_game_window,
                p.is_some_and(|p| p.profile_setting.power_on_required_for_cold_start),
                s.settings.pop_out_setting.auto_panning.is_enabled,
                s.settings.pop_out_setting.auto_panning.key_binding.clone(),
                p.is_some_and(|p| p.is_used_legacy_camera_system),
                p.map(|p| p.panel_source_cockpit_zoom_factor).unwrap_or(50),
                s.settings.general_setting.turbo_mode,
                p.map(|p| p.msfs_game_window_config.clone())
                    .unwrap_or_default(),
            )
        });

        if auto_resize && window::is_msfs_windowed() {
            self.status
                .step_ok("Moving and resizing MSFS game window", false, || {
                    window::set_msfs_game_window_location(&game_window);
                    sleep(1000);
                });
        }

        // some mods report the battery as on although the plane is cold and dark
        if power_required {
            self.turn_on_power();
            self.turn_on_avionics();
        }

        self.turn_off_track_ir();
        self.turn_on_active_pause();

        if auto_panning && legacy {
            self.status
                .write_line("Setting auto panning camera view", StatusMessageType::Info);

            if self.read(|s| s.sim.camera_view_type_and_index1) == CAMERA_VIEW_HOME_COCKPIT_MODE {
                self.set_cockpit_zoom_level(zoom_factor, turbo);
            } else {
                self.status.step_ok("Resetting camera view", true, || {
                    self.reset_cockpit_view(turbo)
                });

                let ok = self.status.step("Loading custom camera view", true, || {
                    self.load_custom_view(&key, turbo, false)
                });
                if !ok {
                    return false;
                }

                self.status.step_ok("Setting camera zoom level", true, || {
                    self.set_cockpit_zoom_level(50, turbo)
                });
            }
        }
        true
    }

    fn step_custom_panel_pop_out(&self, built_in: &[isize]) {
        let app = self.app_hwnd();
        // remember where our own window is, to put it back afterwards
        let app_rect = (app != 0 && !window::is_minimized(app)).then(|| window::rect_dwm(app));

        let (panel_count, legacy, turbo) = self.read(|s| {
            (
                s.active_profile()
                    .map(|p| p.panel_configs.len())
                    .unwrap_or(0),
                s.active_profile()
                    .is_some_and(|p| p.is_used_legacy_camera_system),
                s.settings.general_setting.turbo_mode,
            )
        });
        if panel_count > 0 {
            self.status
                .write_line("Popping out user defined panels", StatusMessageType::Info);
        }

        if !legacy {
            self.reset_camera_view();
            sleep(250);
        }

        let customs: Vec<PanelConfig> = self.read(|s| {
            s.active_profile()
                .map(|p| {
                    p.panel_configs
                        .iter()
                        .filter(|c| c.panel_type == PanelType::CustomPopout)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        });

        for (index, panel) in customs.iter().enumerate() {
            self.status.step(&panel.panel_name, true, || {
                if !legacy {
                    self.set_fixed_camera(
                        panel.fixed_camera_config.camera_type,
                        panel.fixed_camera_config.camera_index,
                    );
                }

                self.write(|s| {
                    if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(panel.id)) {
                        c.is_selected_panel_source = true;
                    }
                });

                if let (Some(x), Some(y)) = (panel.panel_source.x, panel.panel_source.y)
                    && let Some(sim_hwnd) = window::msfs_game_window() {
                        input::prepare_to_pop_out_panel(sim_hwnd, self.app_hwnd(), x, y, turbo);
                    }

                self.execute_custom_pop_out(panel.id, built_in, index as i32);
                self.apply_panel_location(panel.id);

                self.write(|s| {
                    if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(panel.id)) {
                        c.is_selected_panel_source = false;
                    }
                });

                self.read(|s| {
                    s.active_profile()
                        .and_then(|p| p.panel(panel.id))
                        .map(|c| c.is_pop_out_success() != Some(false))
                        .unwrap_or(false)
                })
            });
        }

        if let Some(r) = app_rect
            && r.width > 0 && r.height > 0 {
                window::move_window_rect(app, r);
            }
    }

    fn step_post_custom_panel_pop_out(&self) {
        if self.read(|s| {
            s.active_profile()
                .is_some_and(|p| p.profile_setting.power_on_required_for_cold_start)
        }) {
            self.turn_off_avionics();
            self.turn_off_power();
        }

        self.turn_on_track_ir();
        sleep(500);

        self.turn_off_active_pause();
        sleep(500);

        self.return_to_after_pop_out_camera_view();
    }

    fn step_add_built_in_panels(&self, mut built_in: Vec<isize>) {
        if !self.read(|s| {
            s.active_profile()
                .is_some_and(|p| p.profile_setting.include_in_game_panels)
        }) {
            return;
        }

        self.status.step("Configuring built-in panel", false, || {
            let mut count = 0;
            while built_in.is_empty() && count < 5 {
                built_in = window::windows_by_panel_type(&[PanelType::BuiltInPopout]);
                count += 1;
            }

            for h in &built_in {
                let caption = window::window_text(*h);
                let existing = self.read(|s| {
                    s.active_profile()
                        .and_then(|p| p.panel_configs.iter().find(|c| c.panel_name == caption))
                        .map(|c| c.id)
                });
                match existing {
                    None => {
                        let locked = self.read(|s| s.active_profile().is_some_and(|p| p.is_locked));
                        if !locked {
                            let r = window::rect_dwm(*h);
                            let mut c = PanelConfig::new(PanelType::BuiltInPopout, &caption);
                            c.panel_handle = PanelHandle::Hwnd(*h);
                            c.top = r.y;
                            c.left = r.x;
                            c.width = r.width;
                            c.height = r.height;
                            c.auto_game_refocus = false;
                            self.write(|s| {
                                if let Some(p) = s.active_profile_mut() {
                                    p.panel_configs.push(c);
                                }
                            });
                        }
                    }
                    Some(id) => {
                        self.write(|s| {
                            if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(id)) {
                                c.panel_handle = PanelHandle::Hwnd(*h);
                            }
                        });
                        // MSFS only takes this setting when it is applied twice
                        self.apply_panel_location(id);
                        self.apply_panel_location(id);
                    }
                }
            }

            // built-in panels that are not open
            self.write(|s| {
                if let Some(p) = s.active_profile_mut() {
                    for c in p.panel_configs.iter_mut() {
                        if c.panel_type == PanelType::BuiltInPopout
                            && c.panel_handle == PanelHandle::Unset
                        {
                            c.panel_handle = PanelHandle::Failed;
                        }
                    }
                }
            });

            self.read(|s| {
                let Some(p) = s.active_profile() else {
                    return false;
                };
                let built: Vec<&PanelConfig> = p
                    .panel_configs
                    .iter()
                    .filter(|c| c.panel_type == PanelType::BuiltInPopout)
                    .collect();
                !built.iter().any(|c| c.is_pop_out_success() == Some(false)) && !built.is_empty()
            })
        });
    }

    fn step_add_hud_bar(&self) {
        let panel = self.read(|s| {
            let p = s.active_profile()?;
            if !p.profile_setting.hud_bar_config.is_enabled {
                return None;
            }
            p.panel_configs
                .iter()
                .find(|c| c.panel_type == PanelType::HudBarWindow)
                .map(|c| c.id)
        });
        let Some(id) = panel else { return };
        self.status.step_ok("Opening HUD Bar", false, || {
            self.open_auxiliary_window(id, true)
        });
    }

    fn step_add_num_pad(&self) {
        let panel = self.read(|s| {
            let p = s.active_profile()?;
            if !p.profile_setting.num_pad_config.is_enabled {
                return None;
            }
            p.panel_configs
                .iter()
                .find(|c| c.panel_type == PanelType::NumPadWindow)
                .map(|c| c.id)
        });
        let Some(id) = panel else { return };
        self.status.step_ok("Opening Virtual NumPad", false, || {
            self.open_auxiliary_window(id, false)
        });
    }

    /// Ask the GUI for the HUD bar / NumPad window, wait for its handle to be registered
    /// and move it to the stored location.
    fn open_auxiliary_window(&self, panel_id: Uuid, hud: bool) {
        if hud {
            self.open_hud_bar();
        } else {
            self.open_num_pad();
        }

        // the GUI registers the window handle once the window exists
        let mut handle = None;
        for _ in 0..40 {
            handle = self.read(|s| {
                s.active_profile()
                    .and_then(|p| super::panel_hwnd(p, panel_id))
            });
            if handle.is_some() {
                break;
            }
            sleep(100);
        }
        let Some(h) = handle else {
            self.write(|s| {
                if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(panel_id)) {
                    c.panel_handle = PanelHandle::Failed;
                }
            });
            return;
        };

        sleep(1000);
        let (l, t, w, ht) = self.read(|s| {
            s.active_profile()
                .and_then(|p| p.panel(panel_id))
                .map(|c| (c.left, c.top, c.width, c.height))
                .unwrap_or_default()
        });
        window::move_window(h, l, t, w, ht);
        window::move_window(h, l, t, w, ht);
    }

    fn setup_refocus_display(&self) {
        let panels: Vec<(Uuid, String)> = self.read(|s| {
            let Some(p) = s.active_profile() else {
                return Vec::new();
            };
            if !p.profile_setting.refocus_on_display.is_enabled {
                return Vec::new();
            }
            p.panel_configs
                .iter()
                .filter(|c| c.panel_type == PanelType::RefocusDisplay)
                .map(|c| (c.id, c.panel_name.clone()))
                .collect()
        });
        if panels.is_empty() {
            return;
        }
        self.status.write_line(
            "Configuring monitors for auto refocus on touch",
            StatusMessageType::Info,
        );
        for (id, name) in panels {
            self.status.step_ok(&name, true, || {
                self.write(|s| {
                    if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(id)) {
                        c.panel_handle = PanelHandle::Hwnd(1); // virtual handle
                    }
                });
            });
        }
    }

    fn step_apply_panel_config(&self) {
        // MSFS overwrites things like "always on top" while popping out, so apply the rest last
        let ids: Vec<Uuid> = self.read(|s| {
            s.active_profile()
                .map(|p| p.panel_configs.iter().map(|c| c.id).collect())
                .unwrap_or_default()
        });
        for id in ids {
            self.apply_panel_config(id);
        }
    }

    fn step_post_pop_out(&self) {
        self.write(|s| {
            if let Some(p) = s.active_profile_mut() {
                p.is_popped_out = true;
            }
        });
        // hooks (window events, touch) follow from the state; see `reconcile_hooks`

        let error = self.read(|s| {
            let Some(p) = s.active_profile() else {
                return false;
            };
            let attempted = p
                .panel_configs
                .iter()
                .filter(|c| c.is_pop_out_success().is_some())
                .count();
            let ok = p
                .panel_configs
                .iter()
                .filter(|c| c.is_pop_out_success() == Some(true))
                .count();
            ok != attempted
        });
        self.status.write_line(
            if error {
                "Pop out has been completed with error."
            } else {
                "Pop out has been completed successfully."
            },
            StatusMessageType::Info,
        );
        sleep(1000);
    }

    // --- single panel operations ----------------------------------------------------------

    fn execute_custom_pop_out(&self, panel_id: Uuid, built_in: &[isize], index: i32) {
        let (source, use_secondary, turbo, name, width, height) = self.read(|s| {
            let c = s.active_profile().and_then(|p| p.panel(panel_id));
            (
                c.map(|c| c.panel_source.clone()).unwrap_or_default(),
                s.settings.pop_out_setting.use_left_right_control_to_pop_out,
                s.settings.general_setting.turbo_mode,
                c.map(|c| c.panel_name.clone()).unwrap_or_default(),
                c.map(|c| c.width).unwrap_or(0),
                c.map(|c| c.height).unwrap_or(0),
            )
        });

        let set_failed = |core: &Core| {
            core.write(|s| {
                if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(panel_id)) {
                    c.panel_handle = PanelHandle::Failed;
                }
            })
        };

        let handle =
            self.try_pop_out_custom_panel(source.x, source.y, built_in, use_secondary, turbo);
        let Some(h) = handle else {
            set_failed(self);
            return;
        };

        // the handle must be new: not one of our other panels and not the game window itself
        let duplicate = self.read(|s| {
            s.active_profile().is_some_and(|p| {
                p.panel_configs
                    .iter()
                    .any(|c| c.panel_handle == PanelHandle::Hwnd(h))
            })
        }) || window::msfs_game_window() == Some(h);
        if duplicate {
            set_failed(self);
            return;
        }

        self.write(|s| {
            if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(panel_id)) {
                c.panel_handle = PanelHandle::Hwnd(h);
            }
        });
        window::set_window_text(h, &format!("{name} (Custom)"));

        // first time popping out: remember the size MSFS gave the window
        if width == 0 && height == 0 {
            let r = window::rect_dwm(h);
            self.write(|s| {
                if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(panel_id)) {
                    c.top = index * 30;
                    c.left = index * 30;
                    c.width = r.width;
                    c.height = r.height;
                }
            });
        }
    }

    /// Try up to five times (half a second apart) to pop the panel out.
    fn try_pop_out_custom_panel(
        &self,
        x: Option<i32>,
        y: Option<i32>,
        built_in: &[isize],
        use_secondary_keys: bool,
        turbo: bool,
    ) -> Option<isize> {
        for _ in 0..5 {
            if let (Some(x), Some(y)) = (x, y) {
                input::pop_out_panel(x, y, use_secondary_keys, turbo);
            }
            let latest = window::windows_by_panel_type(&[PanelType::BuiltInPopout]);
            // exactly one window should be new compared to the built-in panels we started with
            if let Some(h) = latest.into_iter().find(|h| !built_in.contains(h)) {
                return Some(h);
            }
            sleep(500);
        }
        None
    }

    fn apply_panel_location(&self, panel_id: Uuid) {
        let Some((h, l, t, w, ht)) = self.read(|s| {
            let c = s.active_profile().and_then(|p| p.panel(panel_id))?;
            match c.panel_handle {
                PanelHandle::Hwnd(h) => Some((h, c.left, c.top, c.width, c.height)),
                _ => None,
            }
        }) else {
            return;
        };
        window::move_window(h, l, t, w, ht);
    }

    fn apply_panel_config(&self, panel_id: Uuid) {
        let Some((c, title_color)) = self.read(|s| {
            let c = s.active_profile().and_then(|p| p.panel(panel_id))?.clone();
            let tc = &s.settings.pop_out_setting.pop_out_title_bar_customization;
            Some((c, tc.is_enabled.then(|| tc.hex_color.clone())))
        }) else {
            return;
        };
        if c.panel_type == PanelType::RefocusDisplay {
            return;
        }
        let PanelHandle::Hwnd(h) = c.panel_handle else {
            return;
        };

        if !c.full_screen {
            if let Some(color) = title_color {
                window::set_title_bar_color(h, &color);
            }
            if c.hide_titlebar {
                window::apply_hide_title_bar(h, true);
                sleep(250);
            }
            // must be applied last
            if c.always_on_top {
                window::apply_always_on_top(h, true);
                sleep(250);
            }
        }

        if c.full_screen && !c.always_on_top && !c.hide_titlebar {
            sleep(500);
            input::toggle_full_screen_panel(h);
            sleep(250);
            if c.touch_enabled {
                self.set_host_monitor(panel_id);
            }
        }

        if c.floating_panel.is_enabled
            && c.floating_panel.has_keyboard_binding()
            && c.floating_panel.is_hidden_on_start
        {
            self.write(|s| {
                if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(panel_id)) {
                    c.is_floating = false;
                }
            });
            window::minimize_window(h);
        }
    }

    /// Remember which monitor a full screen panel is on (needed for touch).
    pub(crate) fn set_host_monitor(&self, panel_id: Uuid) {
        let Some((l, t, w, h)) = self.read(|s| {
            s.active_profile()
                .and_then(|p| p.panel(panel_id))
                .map(|c| (c.left, c.top, c.width, c.height))
        }) else {
            return;
        };
        let monitor = window::host_monitor(l, t, w, h);
        self.write(|s| {
            if let Some(c) = s.active_profile_mut().and_then(|p| p.panel_mut(panel_id)) {
                c.full_screen_monitor = monitor;
            }
        });
    }

    // --- camera helpers ---------------------------------------------------------------------

    fn return_to_after_pop_out_camera_view(&self) {
        let (enabled, view, key, turbo) = self.read(|s| {
            let c = &s.settings.pop_out_setting.after_pop_out_camera_view;
            (
                c.is_enabled,
                c.camera_view,
                c.key_binding.clone(),
                s.settings.general_setting.turbo_mode,
            )
        });
        if !enabled {
            return;
        }
        self.status.write_line(
            "Applying cockpit view after pop out",
            StatusMessageType::Info,
        );

        if self.read(|s| s.sim.camera_view_type_and_index1) == CAMERA_VIEW_HOME_COCKPIT_MODE {
            self.reset_camera_view();
            return;
        }

        match view {
            AfterPopOutCameraViewType::CockpitCenterView => {
                self.status.step_ok("Resetting camera view", true, || {
                    self.reset_cockpit_view(turbo)
                });
            }
            AfterPopOutCameraViewType::CustomCameraView => {
                self.status.step_ok("Resetting camera view", true, || {
                    self.reset_cockpit_view(turbo)
                });
                self.status.step("Loading custom camera view", true, || {
                    self.load_custom_view(&key, turbo, true)
                });
            }
        }
    }

    pub(super) fn reset_cockpit_view(&self, turbo: bool) {
        for _ in 0..5 {
            self.reset_camera_view();
            sleep(if turbo { 600 } else { 1000 }); // wait for the sim data to refresh
            if matches!(self.read(|s| s.sim.camera_view_type_and_index1), 0 | 1) {
                break;
            }
        }
    }

    pub(super) fn load_custom_view(&self, key: &str, turbo: bool, ignore_error: bool) -> bool {
        self.reset_camera_view();
        sleep(250);
        for _ in 0..5 {
            if let Some(h) = window::msfs_game_window() {
                input::load_custom_view(h, key);
            }
            sleep(if turbo { 1000 } else { 1200 });
            if self.read(|s| s.sim.camera_view_type_and_index1) == CAMERA_VIEW_CUSTOM_CAMERA {
                return true;
            }
        }
        ignore_error
    }

    pub(super) fn set_cockpit_zoom_level(&self, zoom: i32, turbo: bool) {
        for _ in 0..10 {
            self.set_cockpit_camera_zoom_level(zoom);
            sleep(if turbo { 600 } else { 1000 });
            if self.read(|s| s.sim.cockpit_camera_zoom) == zoom {
                break;
            }
        }
    }
}
