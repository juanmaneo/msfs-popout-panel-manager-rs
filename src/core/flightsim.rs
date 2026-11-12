//! Simulator interaction (port of `FlightSimOrchestrator` and the control parts of
//! `SimConnectProvider`): turning sim state into [`FlightSimData`], detecting flights,
//! and the helper actions used by the pop out workflows.

use super::Core;
use crate::flight_sim::{CameraState, HudBarData, aircraft_name_from_path, to_i32};
use crate::lod::SIM_MODULE_NAME;
use crate::model::{CameraType, HudBarType, PanelType};
use crate::simconnect::{SimCmd, SimEvent};
use crate::simdefs::{ActionEvent, HudProp, LodData, RequiredData, WritableVar};
use crate::win::hooks::HookCmd;
use crate::win::process::{self, ProcessMemory};
use crate::win::window;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn truthy(v: f64) -> bool {
    v != 0.0
}

impl Core {
    // --- events from the SimConnect thread -------------------------------------------

    pub(super) fn handle_sim_event(self: &Arc<Self>, e: SimEvent) {
        match e {
            SimEvent::Connected => {
                self.write(|s| {
                    s.sim.is_sim_connect_active = true;
                    s.sim.is_simulator_started = true;
                    s.ui.sim_error = None;
                });
                // re-announce the HUD bar if it is open (the sim thread also remembers it)
                let hud = self.read(|s| {
                    s.ui.hud_bar_open.then(|| {
                        s.active_profile()
                            .map(|p| p.profile_setting.hud_bar_config.hud_bar_type)
                            .unwrap_or_default()
                    })
                });
                if let Some(t) = hud {
                    self.sim.send(SimCmd::HudBar(Some(t)));
                }
                self.start_dynamic_lod();
            }
            SimEvent::Disconnected => {
                self.write(|s| {
                    s.sim.is_sim_connect_data_received = false;
                    s.sim.is_sim_connect_active = false;
                    s.sim.reset();
                });
            }
            SimEvent::Required(d) => self.handle_required_data(d),
            SimEvent::HudBar(values) => self.write(|s| {
                let hud_on = s
                    .active_profile()
                    .is_some_and(|p| p.profile_setting.hud_bar_config.is_enabled);
                if !hud_on || !s.sim.is_flight_started {
                    return;
                }
                let h = &mut s.sim.hud_bar_data;
                for (prop, v) in values {
                    match prop {
                        HudProp::SimRate => h.sim_rate = v,
                        HudProp::ParkingBrake => h.parking_brake = v,
                        HudProp::GearLeft => h.gear_left = v,
                        HudProp::GearCenter => h.gear_center = v,
                        HudProp::GearRight => h.gear_right = v,
                        HudProp::ElevatorTrim => h.elevator_trim = v,
                        HudProp::AileronTrim => h.aileron_trim = v,
                        HudProp::RudderTrim => h.rudder_trim = v,
                        HudProp::Flap => h.flap = v,
                    }
                }
            }),
            SimEvent::Lod(d) => self.handle_lod_data(d),
            SimEvent::Frame(fps) => {
                let (enabled, started) = self.read(|s| {
                    (
                        s.settings.dynamic_lod_setting.is_enabled,
                        s.sim.is_flight_started,
                    )
                });
                if enabled && started {
                    self.current_fps.store(fps, Ordering::SeqCst);
                }
            }
            SimEvent::AircraftPath(path) => {
                let name = aircraft_name_from_path(&path);
                self.set_aircraft_name(name);
            }
            SimEvent::Error(msg) => self.write(|s| s.ui.sim_error = Some(msg)),
        }
        self.repaint();
    }

    fn set_aircraft_name(&self, name: Option<String>) {
        let name = name.filter(|n| !n.is_empty());
        let changed = self.write(|s| {
            if s.sim.aircraft_name == name {
                false
            } else {
                s.sim.aircraft_name = name;
                true
            }
        });
        if changed {
            self.auto_switch_profile();
            self.refresh_profile();
        }
    }

    fn handle_required_data(self: &Arc<Self>, d: RequiredData) {
        let camera_state = CameraState::from_i32(to_i32(d.camera_state));
        self.write(|s| {
            let f = &mut s.sim;
            f.electrical_master_battery = truthy(d.electrical_master_battery);
            f.avionics_master_switch = truthy(d.avionics_master_switch);
            f.track_ir_status = truthy(d.track_ir_enable);
            f.plane_in_parking_spot = truthy(d.plane_in_parking_spot);
            f.camera_state = camera_state;
            f.cockpit_camera_zoom = to_i32(d.cockpit_camera_zoom);
            f.camera_view_type_and_index0 = to_i32(d.camera_view_type_and_index0);
            f.camera_view_type_and_index1 = to_i32(d.camera_view_type_and_index1);
            f.camera_view_type_and_index1_max = to_i32(d.camera_view_type_and_index1_max);
            f.camera_view_type_and_index2_max = to_i32(d.camera_view_type_and_index2_max);
            f.is_sim_connect_data_received = true;
        });

        let detection = self
            .detector
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .update(camera_state);

        if let Some(in_cockpit) = detection.in_cockpit_changed {
            self.write(|s| {
                s.sim.is_in_cockpit = in_cockpit;
                if in_cockpit {
                    s.sim.is_flight_started = true;
                }
            });
        }
        if detection.flight_started {
            self.on_flight_started();
        }
        if detection.flight_stopped {
            self.on_flight_stopped();
        }
    }

    fn on_flight_started(self: &Arc<Self>) {
        let auto = self.read(|s| s.settings.auto_pop_out_setting.is_enabled);
        if auto {
            self.auto_pop_out();
        }
        self.write(|s| s.sim.is_flight_started = true);
        self.start_dynamic_lod();
    }

    fn on_flight_stopped(&self) {
        // pop outs are gone with the flight
        self.write(|s| {
            if let Some(p) = s.active_profile_mut() {
                p.reset_runtime();
            }
            s.selecting_panel = None;
            s.editing_lock = false;
        });
        self.close_all_panel_sources();
        self.close_all_pop_outs();
        self.write(|s| {
            s.sim.hud_bar_data.clear();
            s.sim.is_flight_started = false;
            s.sim.is_in_cockpit = false;
            s.sim.dynamic_lod = Default::default();
        });
        self.sim.send(SimCmd::HudBar(None));
        self.stop_dynamic_lod();
        self.fps.lock().unwrap_or_else(|e| e.into_inner()).reset();
        self.current_fps.store(0, Ordering::SeqCst);
        self.repaint();
    }

    /// Close every window we opened (pop outs, HUD bar, NumPad); floating panels become
    /// "floating" again so that the next pop out starts clean.
    pub(crate) fn close_all_pop_outs(&self) {
        self.write(|s| {
            if let Some(p) = s.active_profile_mut() {
                for c in &mut p.panel_configs {
                    if c.floating_panel.is_enabled && !c.is_floating {
                        c.is_floating = true;
                    }
                }
            }
            s.ui.hud_bar_open = false;
            s.ui.num_pad_open = false;
        });
        self.sim.send(SimCmd::HudBar(None));
        window::close_all_custom_pop_outs();
        self.repaint();
    }

    // --- dynamic LOD -----------------------------------------------------------------

    pub(super) fn start_dynamic_lod(&self) {
        self.attach_dynamic_lod();
        self.sim.send(SimCmd::DynamicLod(true));
    }

    pub(super) fn stop_dynamic_lod(&self) {
        self.detach_dynamic_lod();
        self.sim.send(SimCmd::DynamicLod(false));
    }

    /// Attach to the simulator's memory when Dynamic LOD is enabled and a flight is running.
    fn attach_dynamic_lod(&self) {
        let enabled = self.read(|s| s.settings.dynamic_lod_setting.is_enabled);
        if !enabled {
            return;
        }
        let mut rt = self.lod.lock().unwrap_or_else(|e| e.into_inner());
        if rt.lod.is_active() {
            return;
        }
        let Some(pid) = process::find_process_id(process::SIM_PROCESS_NAME) else {
            return;
        };
        let Some(base) = process::module_base(pid, SIM_MODULE_NAME) else {
            return;
        };
        let Some(mem) = ProcessMemory::open(pid) else {
            return;
        };
        match rt.lod.attach(&mem, base, Instant::now()) {
            Ok(()) => rt.mem = Some(mem),
            Err(e) => log::error!("{e}"),
        }
    }

    pub(super) fn detach_dynamic_lod(&self) {
        let setting = self.read(|s| s.settings.dynamic_lod_setting.clone());
        let mut rt = self.lod.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mem) = rt.mem.take() {
            rt.lod.detach(&mem, &setting);
        }
    }

    /// Called regularly: follows the "enabled" preference while a flight is running.
    pub(super) fn maintain_dynamic_lod(&self) {
        let (enabled, started) = self.read(|s| {
            (
                s.settings.dynamic_lod_setting.is_enabled,
                s.sim.is_flight_started,
            )
        });
        let active = self
            .lod
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .lod
            .is_active();
        if started && enabled && !active {
            self.attach_dynamic_lod();
        } else if !enabled && active {
            self.detach_dynamic_lod();
        }
    }

    fn handle_lod_data(&self, d: LodData) {
        let (setting, started, in_cockpit, camera) = self.read(|s| {
            (
                s.settings.dynamic_lod_setting.clone(),
                s.sim.is_flight_started,
                s.sim.is_in_cockpit,
                s.sim.camera_state,
            )
        });
        if !setting.is_enabled || !started {
            return;
        }

        let paused = (setting.pause_when_msfs_lose_focus && !window::is_msfs_in_focus())
            || (setting.pause_outside_cockpit_view && camera != CameraState::Cockpit);

        let now = Instant::now();
        let mut rt = self.lod.lock().unwrap_or_else(|e| e.into_inner());

        if rt.paused_previously && !paused {
            if now.duration_since(rt.last_pause) <= Duration::from_secs(3) {
                return;
            }
            rt.last_pause = now;
            rt.paused_previously = false;
        } else if paused {
            rt.paused_previously = true;
            return;
        }

        // take an FPS sample every 0.4 seconds
        if now.duration_since(rt.last_sample) <= Duration::from_millis(400) {
            return;
        }
        rt.last_sample = now;

        let rt = &mut *rt;
        let Some(mem) = rt.mem.as_ref() else { return };

        let tlod = rt.lod.read_tlod(mem);
        let olod = rt.lod.read_olod(mem);
        let cloud = rt.lod.read_cloud_quality(mem).to_string();
        let frame_gen = rt.lod.read_is_fg(mem);
        let raw_fps = self.current_fps.load(Ordering::SeqCst);
        let fps = self
            .fps
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .average(if frame_gen { raw_fps * 2 } else { raw_fps });

        let sim_data = self.write(|s| {
            let l = &mut s.sim.dynamic_lod;
            l.agl = d.plane_alt_above_ground;
            l.alt_above_ground = d.plane_alt_above_ground;
            l.alt_above_ground_minus_cg = d.plane_alt_above_ground_minus_cg;
            l.ground_velocity = d.ground_velocity;
            l.plane_on_ground = truthy(d.sim_on_ground);
            l.tlod = tlod;
            l.olod = olod;
            l.cloud_quality = cloud;
            l.fps = fps;
            l.clone()
        });

        rt.lod
            .update(mem, &setting, &sim_data, started && in_cockpit, now);
    }

    // --- helpers used by workflows (these block; call from worker threads) -------------------

    fn flag_values(&self) -> (bool, bool) {
        self.read(|s| (s.sim.track_ir_status, s.sim.is_sim_connect_data_received))
    }

    pub(crate) fn turn_off_track_ir(&self) {
        if !self.read(|s| s.settings.track_ir_setting.auto_disable_track_ir) {
            return;
        }
        self.status.step("Turning off TrackIR", false, || {
            for _ in 0..5 {
                let (on, received) = self.flag_values();
                if received && on {
                    self.sim.set_data(WritableVar::TrackIrEnable, 0.0);
                    self.flags
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .track_ir_managed = true;
                    sleep(200);
                }
                sleep(500);
                if !self.read(|s| s.sim.track_ir_status) {
                    break;
                }
            }
            !self.read(|s| s.sim.track_ir_status)
        });
    }

    pub(crate) fn turn_on_track_ir(&self) {
        if !self.read(|s| s.settings.track_ir_setting.auto_disable_track_ir) {
            return;
        }
        // only restore TrackIR if we were the ones who switched it off
        if !self
            .flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .track_ir_managed
        {
            return;
        }
        self.status.step("Turning on TrackIR", false, || {
            for _ in 0..5 {
                self.sim.set_data(WritableVar::TrackIrEnable, 1.0);
                sleep(500);
                if self.read(|s| s.sim.track_ir_status) {
                    break;
                }
            }
            let on = self.read(|s| s.sim.track_ir_status);
            if on {
                self.flags
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .track_ir_managed = false;
            }
            on
        });
    }

    /// Transmit a client event and give the simulator time to process it.
    fn transmit(&self, e: ActionEvent, data: u32) {
        self.sim.transmit(e, data);
        sleep(200);
    }

    pub(crate) fn turn_on_power(&self) {
        let (required, battery) = self.read(|s| {
            (
                s.active_profile()
                    .is_some_and(|p| p.profile_setting.power_on_required_for_cold_start),
                s.sim.electrical_master_battery,
            )
        });
        if battery || !self.read(|s| s.active_profile().is_some()) {
            return;
        }
        self.flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .turned_on_power = true;
        self.status.step("Turning on battery", false, || {
            for _ in 0..10 {
                if required && self.read(|s| s.sim.is_sim_connect_data_received) {
                    // wait for "on parking spot" to refresh
                    sleep(1000);
                    if self.read(|s| s.sim.plane_in_parking_spot) {
                        self.flags
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .power_on_for_pop_out = true;
                        self.transmit(ActionEvent::MasterBatterySet, 1);
                    }
                }
                sleep(500);
                if self.read(|s| s.sim.electrical_master_battery) {
                    break;
                }
            }
            self.read(|s| s.sim.electrical_master_battery)
        });
    }

    pub(crate) fn turn_off_power(&self) {
        if !self
            .flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .turned_on_power
        {
            return;
        }
        self.status.step("Turning off battery", false, || {
            for _ in 0..10 {
                let mine = self
                    .flags
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .power_on_for_pop_out;
                if mine {
                    self.transmit(ActionEvent::MasterBatterySet, 0);
                    self.flags
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .power_on_for_pop_out = false;
                }
                sleep(500);
                if !self.read(|s| s.sim.electrical_master_battery) {
                    break;
                }
            }
            !self.read(|s| s.sim.electrical_master_battery)
        });
        self.flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .turned_on_power = false;
    }

    pub(crate) fn turn_on_avionics(&self) {
        let (required, avionics) = self.read(|s| {
            (
                s.active_profile()
                    .is_some_and(|p| p.profile_setting.power_on_required_for_cold_start),
                s.sim.avionics_master_switch,
            )
        });
        if avionics || !self.read(|s| s.active_profile().is_some()) {
            return;
        }
        self.flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .turned_on_avionics = true;
        self.status.step("Turning on avionics", false, || {
            for _ in 0..10 {
                if required
                    && self.read(|s| s.sim.is_sim_connect_data_received)
                    && self.read(|s| s.sim.plane_in_parking_spot)
                {
                    self.flags
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .avionics_on_for_pop_out = true;
                    self.transmit(ActionEvent::AvionicsMasterSet, 1);
                }
                sleep(500);
                if self.read(|s| s.sim.avionics_master_switch) {
                    break;
                }
            }
            self.read(|s| s.sim.avionics_master_switch)
        });
    }

    pub(crate) fn turn_off_avionics(&self) {
        if !self
            .flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .turned_on_avionics
        {
            return;
        }
        self.status.step("Turning off avionics", false, || {
            for _ in 0..10 {
                let mine = self
                    .flags
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .avionics_on_for_pop_out;
                if mine {
                    self.transmit(ActionEvent::AvionicsMasterSet, 0);
                    self.flags
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .avionics_on_for_pop_out = false;
                }
                sleep(500);
                if !self.read(|s| s.sim.avionics_master_switch) {
                    break;
                }
            }
            !self.read(|s| s.sim.avionics_master_switch)
        });
        self.flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .turned_on_avionics = false;
    }

    pub(crate) fn turn_on_active_pause(&self) {
        if !self.read(|s| s.settings.pop_out_setting.auto_active_pause) {
            return;
        }
        self.status.step_ok("Turning on active pause", false, || {
            self.transmit(ActionEvent::PauseSet, 1)
        });
    }

    pub(crate) fn turn_off_active_pause(&self) {
        if !self.read(|s| s.settings.pop_out_setting.auto_active_pause) {
            return;
        }
        self.status.step_ok("Turning off active pause", false, || {
            self.transmit(ActionEvent::PauseSet, 0)
        });
    }

    /// HUD bar "faster" button.
    pub fn increase_sim_rate(&self) {
        if to_i32(self.read(|s| s.sim.hud_bar_data.sim_rate)) == 16 {
            return;
        }
        self.sim.transmit(ActionEvent::SimRateIncr, 1);
    }

    /// HUD bar "slower" button.
    pub fn decrease_sim_rate(&self) {
        if format!("{:.2}", self.read(|s| s.sim.hud_bar_data.sim_rate)) == "0.25" {
            return;
        }
        self.sim.transmit(ActionEvent::SimRateDecr, 1);
    }

    pub(crate) fn set_cockpit_camera_zoom_level(&self, zoom: i32) {
        self.sim
            .set_data(WritableVar::CockpitCameraZoom, zoom as f64);
    }

    pub(crate) fn reset_camera_view(&self) {
        self.sim.set_data(WritableVar::CameraRequestAction, 1.0);
    }

    pub(crate) fn set_fixed_camera(&self, camera_type: CameraType, index: i32) {
        if self.read(|s| s.sim.camera_state) != CameraState::Cockpit {
            self.sim
                .set_data(WritableVar::CameraState, CameraState::Cockpit as i32 as f64);
            sleep(250);
            self.reset_camera_view();
            sleep(250);
        }
        self.sim.set_data(
            WritableVar::CameraViewTypeAndIndex0,
            camera_type as i32 as f64,
        );
        sleep(250);
        self.sim
            .set_data(WritableVar::CameraViewTypeAndIndex1, index as f64);
    }

    // --- HUD bar / NumPad windows -----------------------------------------------------------

    /// Open the HUD bar window (the GUI creates it) and start polling its data.
    pub(crate) fn open_hud_bar(&self) {
        let t = self.write(|s| {
            let t = s
                .active_profile()
                .map(|p| p.profile_setting.hud_bar_config.hud_bar_type)
                .unwrap_or(HudBarType::GenericAircraft);
            s.sim.hud_bar_data = HudBarData::new(t);
            s.ui.hud_bar_open = true;
            t
        });
        self.sim.send(SimCmd::HudBar(Some(t)));
        self.repaint();
    }

    /// The user closed the HUD bar (or it is no longer wanted).
    pub fn close_hud_bar(&self) {
        self.write(|s| s.ui.hud_bar_open = false);
        self.sim.send(SimCmd::HudBar(None));
    }

    pub(crate) fn open_num_pad(&self) {
        self.write(|s| s.ui.num_pad_open = true);
        self.repaint();
    }

    pub fn close_num_pad(&self) {
        self.write(|s| s.ui.num_pad_open = false);
    }

    /// Header "Status" button: show or hide the status window.
    pub fn toggle_status_window(&self) {
        self.write(|s| s.ui.status_open = !s.ui.status_open);
    }

    /// Close the status window, including any pop out message it is showing.
    pub fn close_status_window(&self) {
        self.write(|s| {
            s.ui.status_open = false;
            s.ui.message = None;
        });
    }

    /// Keep the auxiliary windows consistent with the profile (called from the core loop).
    pub(super) fn sync_auxiliary_windows(&self) {
        let (hud_open, hud_wanted, pad_open, pad_wanted) = self.read(|s| {
            let has = |t: PanelType| {
                s.active_profile()
                    .is_some_and(|p| p.panel_configs.iter().any(|c| c.panel_type == t))
            };
            (
                s.ui.hud_bar_open,
                has(PanelType::HudBarWindow),
                s.ui.num_pad_open,
                has(PanelType::NumPadWindow),
            )
        });
        if hud_open && !hud_wanted {
            self.close_hud_bar();
        }
        if pad_open && !pad_wanted {
            self.close_num_pad();
        }
    }

    /// Send a virtual NumPad key to the simulator.
    pub fn num_pad_click(&self, name: &str) {
        let name = name.to_string();
        std::thread::spawn(move || {
            if let Some(h) = window::msfs_game_window() {
                crate::win::input::numpad_click(h, &name);
            }
        });
    }

    /// Stop all pop-out windows and overlays when asked from the UI.
    pub(crate) fn close_all_panel_sources(&self) {
        self.hooks.send(HookCmd::CloseAllOverlays);
        self.write(|s| {
            s.editing_lock = false;
            s.selecting_panel = None;
            if let Some(p) = s.active_profile_mut() {
                p.is_editing_panel_source = false;
                p.is_selecting_panel_source = false;
                for c in &mut p.panel_configs {
                    c.is_editing_panel = false;
                    c.is_selected_panel_source = false;
                    c.is_shown_panel_source = false;
                }
            }
        });
    }
}
