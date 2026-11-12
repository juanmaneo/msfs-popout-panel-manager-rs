//! Simulator state as observed through SimConnect, plus HUD bar value formatting.

use crate::model::HudBarType;

/// `Convert.ToInt32(double)` semantics (round half to even).
pub fn to_i32(v: f64) -> i32 {
    let r = v.round();
    if (v - v.trunc()).abs() == 0.5 && (r as i64) % 2 != 0 {
        (r - v.signum()) as i32
    } else {
        r as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraState {
    Cockpit = 2,
    External = 3,
    Drone = 4,
    LoadScreen = 11,
    HomeScreen = 15,
    #[default]
    Unknown = 0,
}

impl CameraState {
    pub fn from_i32(v: i32) -> Self {
        match v {
            2 => CameraState::Cockpit,
            3 => CameraState::External,
            4 => CameraState::Drone,
            11 => CameraState::LoadScreen,
            15 => CameraState::HomeScreen,
            _ => CameraState::Unknown,
        }
    }
}

/// Result of feeding a new camera state to [`FlightDetector`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlightDetection {
    /// `Some(true)` when the camera entered the cockpit, `Some(false)` when the flight ended.
    pub in_cockpit_changed: Option<bool>,
    pub flight_started: bool,
    pub flight_stopped: bool,
}

/// Port of `SimConnectProvider.DetectFlightStartedOrStopped`: a flight starts when the
/// camera goes from a load/home screen into the cockpit and stops on the way back.
#[derive(Debug, Default)]
pub struct FlightDetector {
    current: CameraState,
}

impl FlightDetector {
    pub fn update(&mut self, camera_state: CameraState) -> FlightDetection {
        let mut d = FlightDetection::default();
        if self.current == camera_state {
            return d;
        }
        if camera_state == CameraState::Cockpit {
            d.in_cockpit_changed = Some(true);
        }
        match self.current {
            CameraState::HomeScreen | CameraState::LoadScreen => {
                if camera_state == CameraState::Cockpit {
                    self.current = camera_state;
                    d.flight_started = true;
                }
            }
            CameraState::Cockpit => {
                if matches!(
                    camera_state,
                    CameraState::LoadScreen | CameraState::HomeScreen
                ) {
                    self.current = camera_state;
                    d.flight_stopped = true;
                    d.in_cockpit_changed = Some(false);
                }
            }
            _ => {}
        }
        if matches!(
            camera_state,
            CameraState::Cockpit | CameraState::HomeScreen | CameraState::LoadScreen
        ) {
            self.current = camera_state;
        }
        d
    }
}

/// Camera view type/index values used by the pop out workflow.
pub const CAMERA_VIEW_HOME_COCKPIT_MODE: i32 = 8;
pub const CAMERA_VIEW_CUSTOM_CAMERA: i32 = 7;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HudBarData {
    pub hud_bar_type: HudBarType,
    pub elevator_trim: f64,
    pub aileron_trim: f64,
    pub rudder_trim: f64,
    pub flap: f64,
    pub parking_brake: f64,
    pub gear_left: f64,
    pub gear_center: f64,
    pub gear_right: f64,
    pub sim_rate: f64,
}

fn map_gear(gear: f64) -> &'static str {
    if to_i32(gear) == 100 {
        "DOWN"
    } else if gear == 0.0 {
        "UP"
    } else {
        "MOVING"
    }
}

impl HudBarData {
    pub fn new(t: HudBarType) -> Self {
        Self {
            hud_bar_type: t,
            ..Self::default()
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new(self.hud_bar_type);
    }

    pub fn elevator_trim_text(&self) -> String {
        format!("{:.2}", self.elevator_trim / 10.0)
    }

    pub fn aileron_trim_text(&self) -> String {
        format!("{:.2}", self.aileron_trim / 16384.0 * 57.0)
    }

    pub fn rudder_trim_text(&self) -> String {
        format!("{:.2}", self.rudder_trim * 2.0 * 10.0)
    }

    pub fn flap_text(&self) -> String {
        match self.hud_bar_type {
            HudBarType::Pmdg737 => match to_i32(self.flap) {
                0 => "0",
                1 => "1",
                2 => "2",
                3 => "5",
                4 => "10",
                5 => "15",
                6 => "25",
                7 => "30",
                8 => "40",
                _ => "5",
            }
            .to_string(),
            _ => format!("{:.0}", self.flap),
        }
    }

    pub fn parking_brake_text(&self) -> &'static str {
        if self.parking_brake > 0.0 {
            "Engaged"
        } else {
            "Disengaged"
        }
    }

    pub fn gear_left_text(&self) -> &'static str {
        map_gear(self.gear_left)
    }

    pub fn gear_center_text(&self) -> &'static str {
        map_gear(self.gear_center)
    }

    pub fn gear_right_text(&self) -> &'static str {
        map_gear(self.gear_right)
    }

    /// Shortest round-trip form of the rate ("4", "0.5", "0.25"), like upstream's double text.
    pub fn sim_rate_text(&self) -> String {
        self.sim_rate.to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicLodSimData {
    pub fps: i32,
    pub tlod: i32,
    pub olod: i32,
    pub agl: f64,
    pub cloud_quality: String,
    pub alt_above_ground: f64,
    pub alt_above_ground_minus_cg: f64,
    pub ground_velocity: f64,
    pub plane_on_ground: bool,
}

impl Default for DynamicLodSimData {
    fn default() -> Self {
        Self {
            fps: 0,
            tlod: 0,
            olod: 0,
            agl: 0.0,
            cloud_quality: "N/A".into(),
            alt_above_ground: 0.0,
            alt_above_ground_minus_cg: 0.0,
            ground_velocity: 0.0,
            plane_on_ground: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlightSimData {
    pub is_sim_connect_active: bool,
    pub aircraft_name: Option<String>,
    pub electrical_master_battery: bool,
    pub avionics_master_switch: bool,
    pub track_ir_status: bool,
    pub camera_state: CameraState,
    pub cockpit_camera_zoom: i32,
    pub camera_view_type_and_index0: i32,
    pub camera_view_type_and_index1: i32,
    pub camera_view_type_and_index1_max: i32,
    pub camera_view_type_and_index2_max: i32,
    pub plane_in_parking_spot: bool,
    pub is_simulator_started: bool,
    pub is_sim_connect_data_received: bool,
    pub is_in_cockpit: bool,
    pub is_flight_started: bool,
    pub hud_bar_data: HudBarData,
    pub dynamic_lod: DynamicLodSimData,
}

impl FlightSimData {
    /// Upstream `FlightSimData.Reset()` (does not touch flight started / hud / lod data).
    pub fn reset(&mut self) {
        self.aircraft_name = None;
        self.electrical_master_battery = false;
        self.avionics_master_switch = false;
        self.track_ir_status = false;
        self.is_in_cockpit = false;
        self.plane_in_parking_spot = false;
        self.camera_state = CameraState::Unknown;
        self.is_simulator_started = false;
        self.camera_view_type_and_index1_max = 0;
        self.camera_view_type_and_index2_max = 0;
    }
}

/// Derive the aircraft name from a SimConnect aircraft file path
/// (`...\Some_Aircraft\aircraft.cfg` -> `SOME AIRCRAFT`).
pub fn aircraft_name_from_path(path: &str) -> Option<String> {
    let tokens: Vec<&str> = path.split('\\').collect();
    if tokens.len() > 1 {
        Some(tokens[tokens.len() - 2].replace('_', " ").to_uppercase())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banker_rounding() {
        assert_eq!(to_i32(0.5), 0);
        assert_eq!(to_i32(1.5), 2);
        assert_eq!(to_i32(2.5), 2);
        assert_eq!(to_i32(-1.5), -2);
        assert_eq!(to_i32(99.6), 100);
    }

    #[test]
    fn flight_start_and_stop_detection() {
        let mut f = FlightDetector::default();
        // app started while the sim is on the home screen
        let d = f.update(CameraState::HomeScreen);
        assert_eq!(d, FlightDetection::default());
        let d = f.update(CameraState::LoadScreen);
        assert_eq!(d, FlightDetection::default());
        let d = f.update(CameraState::Cockpit);
        assert!(d.flight_started && !d.flight_stopped);
        assert_eq!(d.in_cockpit_changed, Some(true));
        // external view does not end the flight
        let d = f.update(CameraState::External);
        assert_eq!(d, FlightDetection::default());
        // ...and returning to the cockpit is not a new event either
        let d = f.update(CameraState::Cockpit);
        assert_eq!(d, FlightDetection::default());
        let d = f.update(CameraState::HomeScreen);
        assert!(d.flight_stopped);
        assert_eq!(d.in_cockpit_changed, Some(false));
    }

    #[test]
    fn attaching_while_already_in_cockpit_is_not_a_flight_start() {
        let mut f = FlightDetector::default();
        let d = f.update(CameraState::Cockpit);
        assert!(!d.flight_started);
        assert_eq!(d.in_cockpit_changed, Some(true));
    }

    #[test]
    fn camera_state_from_int() {
        assert_eq!(CameraState::from_i32(2), CameraState::Cockpit);
        assert_eq!(CameraState::from_i32(15), CameraState::HomeScreen);
        assert_eq!(CameraState::from_i32(99), CameraState::Unknown);
    }

    #[test]
    fn hud_bar_formatting() {
        let mut d = HudBarData::new(HudBarType::GenericAircraft);
        d.elevator_trim = 25.0;
        d.aileron_trim = 16384.0 / 57.0;
        d.rudder_trim = 0.05;
        d.flap = 20.0;
        d.parking_brake = 1.0;
        d.gear_left = 100.0;
        d.gear_center = 0.0;
        d.gear_right = 50.0;
        assert_eq!(d.elevator_trim_text(), "2.50");
        assert_eq!(d.aileron_trim_text(), "1.00");
        assert_eq!(d.rudder_trim_text(), "1.00");
        assert_eq!(d.flap_text(), "20");
        assert_eq!(d.parking_brake_text(), "Engaged");
        assert_eq!(d.gear_left_text(), "DOWN");
        assert_eq!(d.gear_center_text(), "UP");
        assert_eq!(d.gear_right_text(), "MOVING");

        d.hud_bar_type = HudBarType::Pmdg737;
        d.flap = 3.0;
        assert_eq!(d.flap_text(), "5");
        d.flap = 8.0;
        assert_eq!(d.flap_text(), "40");
    }

    #[test]
    fn sim_rate_text() {
        let d = HudBarData {
            sim_rate: 4.0,
            ..Default::default()
        };
        assert_eq!(d.sim_rate_text(), "4");
        let d = HudBarData {
            sim_rate: 0.25,
            ..Default::default()
        };
        assert_eq!(d.sim_rate_text(), "0.25");
    }

    #[test]
    fn aircraft_name_parsing() {
        assert_eq!(
            aircraft_name_from_path(r"C:\Community\asobo-aircraft-c172sp\Cessna_172\aircraft.cfg")
                .as_deref(),
            Some("CESSNA 172")
        );
        assert_eq!(aircraft_name_from_path("nopath"), None);
    }

    #[test]
    fn reset_clears_aircraft() {
        let mut f = FlightSimData {
            aircraft_name: Some("X".into()),
            is_in_cockpit: true,
            is_flight_started: true,
            ..Default::default()
        };
        f.reset();
        assert!(f.aircraft_name.is_none());
        assert!(!f.is_in_cockpit);
        assert!(f.is_flight_started); // untouched, like upstream
    }
}
