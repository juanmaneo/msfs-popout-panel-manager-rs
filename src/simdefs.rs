//! SimConnect data definitions (port of `SimDataDefinitions.cs` and the id enums).

use crate::model::HudBarType;

/// A simulation variable to read: `(SimConnect variable name, units)`. All are FLOAT64.
pub type SimVar = (&'static str, &'static str);

// --- definition and request ids ----------------------------------------------------

pub const DEF_REQUIRED: u32 = 0;
pub const DEF_HUDBAR: u32 = 1;
pub const DEF_DYNAMIC_LOD: u32 = 2;
pub const DEF_WRITE_TRACK_IR: u32 = 3;
pub const DEF_WRITE_COCKPIT_CAMERA_ZOOM: u32 = 4;
pub const DEF_WRITE_CAMERA_STATE: u32 = 5;
pub const DEF_WRITE_CAMERA_REQUEST_ACTION: u32 = 6;
pub const DEF_WRITE_VIEW_TYPE_INDEX_0: u32 = 7;
pub const DEF_WRITE_VIEW_TYPE_INDEX_1: u32 = 8;

pub const REQ_REQUIRED: u32 = 0;
pub const REQ_HUDBAR: u32 = 1;
pub const REQ_DYNAMIC_LOD: u32 = 2;

pub const REQ_SYSTEM_STATE_AIRCRAFT_PATH: u32 = 0;

// System events (ids 0..=4 are reserved for them).
pub const EVT_SIM_START: u32 = 0;
pub const EVT_SIM_STOP: u32 = 1;
pub const EVT_AIRCRAFT_LOADED: u32 = 2;
pub const EVT_VIEW: u32 = 3;
pub const EVT_FRAME: u32 = 4;

/// Client events that are transmitted to the simulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionEvent {
    MasterBatterySet = 5,
    AvionicsMasterSet = 6,
    PauseSet = 7,
    SimRateDecr = 8,
    SimRateIncr = 9,
}

impl ActionEvent {
    pub const ALL: [ActionEvent; 5] = [
        ActionEvent::MasterBatterySet,
        ActionEvent::AvionicsMasterSet,
        ActionEvent::PauseSet,
        ActionEvent::SimRateDecr,
        ActionEvent::SimRateIncr,
    ];

    /// The SimConnect event name the id is mapped to.
    pub fn sim_name(self) -> &'static str {
        match self {
            ActionEvent::MasterBatterySet => "MASTER_BATTERY_SET",
            ActionEvent::AvionicsMasterSet => "AVIONICS_MASTER_SET",
            ActionEvent::PauseSet => "PAUSE_SET",
            ActionEvent::SimRateDecr => "SIM_RATE_DECR",
            ActionEvent::SimRateIncr => "SIM_RATE_INCR",
        }
    }
}

/// Variables that can be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritableVar {
    TrackIrEnable,
    CockpitCameraZoom,
    CameraState,
    CameraRequestAction,
    CameraViewTypeAndIndex0,
    CameraViewTypeAndIndex1,
}

impl WritableVar {
    pub fn definition_id(self) -> u32 {
        match self {
            WritableVar::TrackIrEnable => DEF_WRITE_TRACK_IR,
            WritableVar::CockpitCameraZoom => DEF_WRITE_COCKPIT_CAMERA_ZOOM,
            WritableVar::CameraState => DEF_WRITE_CAMERA_STATE,
            WritableVar::CameraRequestAction => DEF_WRITE_CAMERA_REQUEST_ACTION,
            WritableVar::CameraViewTypeAndIndex0 => DEF_WRITE_VIEW_TYPE_INDEX_0,
            WritableVar::CameraViewTypeAndIndex1 => DEF_WRITE_VIEW_TYPE_INDEX_1,
        }
    }

    pub const ALL: [WritableVar; 6] = [
        WritableVar::TrackIrEnable,
        WritableVar::CameraState,
        WritableVar::CockpitCameraZoom,
        WritableVar::CameraRequestAction,
        WritableVar::CameraViewTypeAndIndex0,
        WritableVar::CameraViewTypeAndIndex1,
    ];

    pub fn sim_var(self) -> SimVar {
        match self {
            WritableVar::TrackIrEnable => ("TRACK IR ENABLE", "bool"),
            WritableVar::CameraState => ("CAMERA STATE", "enum"),
            WritableVar::CockpitCameraZoom => ("COCKPIT CAMERA ZOOM", "percentage"),
            WritableVar::CameraRequestAction => ("CAMERA REQUEST ACTION", "enum"),
            WritableVar::CameraViewTypeAndIndex0 => ("CAMERA VIEW TYPE AND INDEX:0", "number"),
            WritableVar::CameraViewTypeAndIndex1 => ("CAMERA VIEW TYPE AND INDEX:1", "number"),
        }
    }
}

// --- required data -----------------------------------------------------------------

/// Order of the values in the "required" data block.
pub const REQUIRED_VARS: [SimVar; 10] = [
    ("ELECTRICAL MASTER BATTERY", "Bool"),
    ("AVIONICS MASTER SWITCH", "Bool"),
    ("TRACK IR ENABLE", "Bool"),
    ("ATC ON PARKING SPOT", "Bool"),
    ("CAMERA STATE", "Number"),
    ("COCKPIT CAMERA ZOOM", "Percentage"),
    ("CAMERA VIEW TYPE AND INDEX:0", "Enum"),
    ("CAMERA VIEW TYPE AND INDEX:1", "Enum"),
    ("CAMERA VIEW TYPE AND INDEX MAX:1", "Number"),
    ("CAMERA VIEW TYPE AND INDEX MAX:2", "Number"),
];

/// Parsed "required" data block.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RequiredData {
    pub electrical_master_battery: f64,
    pub avionics_master_switch: f64,
    pub track_ir_enable: f64,
    pub plane_in_parking_spot: f64,
    pub camera_state: f64,
    pub cockpit_camera_zoom: f64,
    pub camera_view_type_and_index0: f64,
    pub camera_view_type_and_index1: f64,
    pub camera_view_type_and_index1_max: f64,
    pub camera_view_type_and_index2_max: f64,
}

impl RequiredData {
    pub fn from_values(v: &[f64]) -> Option<Self> {
        if v.len() < REQUIRED_VARS.len() {
            return None;
        }
        Some(Self {
            electrical_master_battery: v[0],
            avionics_master_switch: v[1],
            track_ir_enable: v[2],
            plane_in_parking_spot: v[3],
            camera_state: v[4],
            cockpit_camera_zoom: v[5],
            camera_view_type_and_index0: v[6],
            camera_view_type_and_index1: v[7],
            camera_view_type_and_index1_max: v[8],
            camera_view_type_and_index2_max: v[9],
        })
    }
}

// --- dynamic LOD ---------------------------------------------------------------------

pub const DYNAMIC_LOD_VARS: [SimVar; 4] = [
    ("PLANE ALT ABOVE GROUND", "feet"),
    ("PLANE ALT ABOVE GROUND MINUS CG", "feet"),
    ("SIM ON GROUND", "Bool"),
    ("GROUND VELOCITY", "knots"),
];

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LodData {
    pub plane_alt_above_ground: f64,
    pub plane_alt_above_ground_minus_cg: f64,
    pub sim_on_ground: f64,
    pub ground_velocity: f64,
}

impl LodData {
    pub fn from_values(v: &[f64]) -> Option<Self> {
        if v.len() < DYNAMIC_LOD_VARS.len() {
            return None;
        }
        Some(Self {
            plane_alt_above_ground: v[0],
            plane_alt_above_ground_minus_cg: v[1],
            sim_on_ground: v[2],
            ground_velocity: v[3],
        })
    }
}

// --- HUD bar --------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudProp {
    SimRate,
    ParkingBrake,
    GearLeft,
    GearCenter,
    GearRight,
    ElevatorTrim,
    AileronTrim,
    RudderTrim,
    Flap,
}

/// HUD bar variables in definition order for the given HUD bar type.
pub fn hud_bar_vars(t: HudBarType) -> Vec<(HudProp, SimVar)> {
    let mut v = vec![
        (HudProp::SimRate, ("SIMULATION RATE", "Number")),
        (HudProp::ParkingBrake, ("BRAKE PARKING INDICATOR", "Bool")),
        (HudProp::GearLeft, ("GEAR LEFT POSITION", "Percent")),
        (HudProp::GearCenter, ("GEAR CENTER POSITION", "Percent")),
        (HudProp::GearRight, ("GEAR RIGHT POSITION", "Percent")),
    ];
    match t {
        HudBarType::GenericAircraft => v.extend([
            (HudProp::ElevatorTrim, ("ELEVATOR TRIM PCT", "percent")),
            (HudProp::AileronTrim, ("AILERON TRIM", "radians")),
            (HudProp::RudderTrim, ("RUDDER TRIM", "radians")),
            (HudProp::Flap, ("TRAILING EDGE FLAPS LEFT ANGLE", "degrees")),
        ]),
        HudBarType::Pmdg737 => v.extend([
            (HudProp::ElevatorTrim, ("L:switch_690_73X", "number")),
            (HudProp::AileronTrim, ("AILERON POSITION", "Position 16k")),
            (
                HudProp::RudderTrim,
                ("RUDDER DEFLECTION PCT", "Percent Over 100"),
            ),
            (HudProp::Flap, ("FLAPS HANDLE INDEX", "Number")),
        ]),
        HudBarType::None => return Vec::new(),
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_block_parses() {
        let vals: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let r = RequiredData::from_values(&vals).unwrap();
        assert_eq!(r.camera_state, 4.0);
        assert_eq!(r.camera_view_type_and_index2_max, 9.0);
        assert!(RequiredData::from_values(&vals[..5]).is_none());
    }

    #[test]
    fn hud_bar_definitions() {
        let g = hud_bar_vars(HudBarType::GenericAircraft);
        assert_eq!(g.len(), 9);
        assert_eq!(g[0].0, HudProp::SimRate);
        assert_eq!(g[5].1.0, "ELEVATOR TRIM PCT");
        let p = hud_bar_vars(HudBarType::Pmdg737);
        assert_eq!(p[5].1.0, "L:switch_690_73X");
        assert!(hud_bar_vars(HudBarType::None).is_empty());
    }

    #[test]
    fn action_events_have_reserved_ids() {
        assert_eq!(ActionEvent::MasterBatterySet as u32, 5);
        assert_eq!(ActionEvent::SimRateIncr as u32, 9);
        assert!(
            ActionEvent::ALL
                .iter()
                .all(|e| (e.clone() as u32) > EVT_FRAME)
        );
    }

    #[test]
    fn writable_definitions_are_unique() {
        let mut ids: Vec<u32> = WritableVar::ALL.iter().map(|w| w.definition_id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 6);
    }
}
