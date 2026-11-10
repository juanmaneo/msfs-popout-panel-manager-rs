//! Per-frame copy of the state the widgets render against. Widgets never write this back;
//! each user edit is applied to the core individually (see `mod.rs` for the rules).

use crate::core::Core;
use crate::model::UserProfile;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct SimView {
    pub connected: bool,
    pub aircraft: Option<String>,
    pub simulator_started: bool,
    pub in_cockpit: bool,
    pub instrument_count: i32,
}

#[derive(Clone, Default)]
pub struct Snapshot {
    pub profiles: Vec<(Uuid, String)>,
    pub active: Option<UserProfile>,
    pub sim: SimView,
    pub selecting_panel: Option<Uuid>,
    pub pop_out_in_progress: bool,
    pub aircraft_bound: bool,
    pub binding_allowed: bool,
    pub auto_pop_out_enabled: bool,
    pub sim_error: Option<String>,
}

pub fn capture(core: &Core) -> Snapshot {
    core.read(|s| Snapshot {
        profiles: s.profiles.iter().map(|p| (p.id, p.name.clone())).collect(),
        active: s.active_profile().cloned(),
        sim: SimView {
            connected: s.sim.is_sim_connect_active,
            aircraft: s.sim.aircraft_name.clone().filter(|n| !n.is_empty()),
            simulator_started: s.sim.is_simulator_started,
            in_cockpit: s.sim.is_in_cockpit,
            instrument_count: s.sim.camera_view_type_and_index2_max,
        },
        selecting_panel: s.selecting_panel,
        pop_out_in_progress: s.pop_out_in_progress,
        aircraft_bound: s.is_aircraft_bound_to_active_profile(),
        binding_allowed: s.is_allowed_add_aircraft_binding(),
        auto_pop_out_enabled: s.settings.auto_pop_out_setting.is_enabled,
        sim_error: s.ui.sim_error.clone(),
    })
}
