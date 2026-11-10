//! Profile management (port of `ProfileData` / `ProfileOrchestrator`).

use super::{Core, State};
use crate::model::{UserProfile, sort_profiles};
use crate::win::hooks::HookCmd;
use uuid::Uuid;

/// Choose the profile to show at startup (the last used one, falling back to the first).
pub(super) fn select_initial_profile(state: &mut State) {
    let last = state.settings.system_setting.last_used_profile_id;
    activate(state, Some(last));
}

/// Port of `ProfileData.SetActiveProfile(Guid)`: pick a profile and clear its runtime state.
/// `None` (or the nil id) selects the first profile.
pub(super) fn activate(state: &mut State, id: Option<Uuid>) {
    // Runtime state of the profile we leave is meaningless afterwards.
    if let Some(p) = state.active_profile_mut() {
        p.reset_runtime();
    }

    let wanted = id.filter(|i| !i.is_nil());
    let chosen = match wanted {
        Some(i) if state.profiles.iter().any(|p| p.id == i) => Some(i),
        _ => state.profiles.first().map(|p| p.id),
    };
    state.active = chosen;
    state.settings.system_setting.last_used_profile_id = chosen.unwrap_or_else(Uuid::nil);

    state.selecting_panel = None;
    state.editing_lock = false;
    state.ui.keystroke = None;
    state
        .settings
        .keyboard_shortcut_setting
        .is_detecting_keystroke = false;
    if let Some(p) = state.active_profile_mut() {
        p.reset_runtime();
    }
}

impl Core {
    /// Make `id` the active profile (user clicked it in the list).
    pub fn set_active_profile(&self, id: Uuid) {
        self.status.clear();
        self.hooks.send(HookCmd::CloseAllOverlays);
        self.write(|s| activate(s, Some(id)));
    }

    /// Create a profile (optionally a copy of another one) and make it active.
    pub fn add_profile(&self, name: &str, copy_from: Option<Uuid>) {
        self.hooks.send(HookCmd::CloseAllOverlays);
        self.write(|s| {
            if let Some(p) = s.active_profile_mut() {
                p.reset_runtime();
            }
            let new_profile = match copy_from.and_then(|id| s.profiles.iter().find(|p| p.id == id))
            {
                Some(src) => UserProfile::copy_of(name, src),
                None => UserProfile::new(name),
            };
            let new_id = new_profile.id;
            s.profiles.push(new_profile);
            sort_profiles(&mut s.profiles);
            activate(s, Some(new_id));

            // automatically bind the current aircraft if no other profile owns it
            if let Some(aircraft) = s.sim.aircraft_name.clone().filter(|a| !a.is_empty()) {
                let bound = s
                    .profiles
                    .iter()
                    .any(|p| p.aircraft_bindings.iter().any(|b| *b == aircraft));
                if !bound {
                    if let Some(p) = s.active_profile_mut() {
                        p.aircraft_bindings.push(aircraft);
                    }
                }
            }
        });
    }

    pub fn delete_active_profile(&self) {
        self.hooks.send(HookCmd::CloseAllOverlays);
        self.write(|s| {
            let Some(id) = s.active else { return };
            let Some(index) = s.profiles.iter().position(|p| p.id == id) else {
                return;
            };
            s.profiles.remove(index);
            let next = if s.profiles.is_empty() {
                None
            } else if index >= s.profiles.len() {
                Some(s.profiles[0].id)
            } else {
                Some(s.profiles[index].id)
            };
            s.active = None;
            activate(s, next);
        });
    }

    /// Re-sort after the active profile was renamed.
    pub fn profile_renamed(&self) {
        self.write(|s| sort_profiles(&mut s.profiles));
    }

    /// Lock/unlock the active profile (locked profiles enforce their panel layout).
    pub fn toggle_lock(&self) {
        let locked = self.write(|s| {
            let p = s.active_profile_mut()?;
            p.is_locked = !p.is_locked;
            Some(p.is_locked)
        });
        if locked == Some(true) {
            self.close_all_panel_sources();
        }
    }

    /// Bind / unbind the current aircraft to the active profile.
    pub fn toggle_aircraft_binding(&self) {
        self.write(|s| {
            let Some(aircraft) = s.sim.aircraft_name.clone().filter(|a| !a.is_empty()) else {
                return;
            };
            let allowed = s.is_allowed_add_aircraft_binding();
            let Some(p) = s.active_profile_mut() else {
                return;
            };
            if let Some(i) = p.aircraft_bindings.iter().position(|b| *b == aircraft) {
                p.aircraft_bindings.remove(i);
            } else if allowed {
                p.aircraft_bindings.push(aircraft);
            }
        });
    }

    /// Upstream `AutoSwitchProfile`: select the profile bound to the current aircraft.
    pub(super) fn auto_switch_profile(&self) {
        let switched = self.write(|s| {
            let name = s.sim.aircraft_name.clone().filter(|n| !n.is_empty())?;
            let id = s
                .profiles
                .iter()
                .find(|p| p.aircraft_bindings.iter().any(|b| *b == name))?
                .id;
            activate(s, Some(id));
            Some(())
        });
        if switched.is_some() {
            self.hooks.send(HookCmd::CloseAllOverlays);
        }
    }

    /// Re-activate the active profile (resets runtime state).
    pub(super) fn refresh_profile(&self) {
        self.hooks.send(HookCmd::CloseAllOverlays);
        self.write(|s| {
            let id = s.active;
            activate(s, id);
        });
    }

    /// Remember the position of the MSFS window (windowed mode) in the active profile.
    pub(super) fn save_msfs_game_window_config(&self) {
        let cfg = crate::win::window::msfs_game_window_location();
        if cfg.is_valid() {
            self.write(|s| {
                if let Some(p) = s.active_profile_mut() {
                    p.msfs_game_window_config = cfg;
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight_sim::FlightSimData;
    use crate::model::ApplicationSetting;

    fn state(names: &[&str]) -> State {
        let mut profiles: Vec<UserProfile> = names.iter().map(|n| UserProfile::new(n)).collect();
        sort_profiles(&mut profiles);
        State {
            settings: ApplicationSetting::default(),
            profiles,
            active: None,
            sim: FlightSimData::default(),
            ui: Default::default(),
            selecting_panel: None,
            pop_out_in_progress: false,
            editing_lock: false,
        }
    }

    #[test]
    fn initial_selection_prefers_last_used() {
        let mut s = state(&["a", "b", "c"]);
        s.settings.system_setting.last_used_profile_id = s.profiles[1].id;
        select_initial_profile(&mut s);
        assert_eq!(s.active, Some(s.profiles[1].id));
    }

    #[test]
    fn unknown_last_used_falls_back_to_first() {
        let mut s = state(&["a", "b"]);
        s.settings.system_setting.last_used_profile_id = Uuid::new_v4();
        select_initial_profile(&mut s);
        assert_eq!(s.active, Some(s.profiles[0].id));
        assert_eq!(
            s.settings.system_setting.last_used_profile_id,
            s.profiles[0].id
        );
    }

    #[test]
    fn empty_list_selects_nothing() {
        let mut s = state(&[]);
        select_initial_profile(&mut s);
        assert_eq!(s.active, None);
        assert!(s.settings.system_setting.last_used_profile_id.is_nil());
    }

    #[test]
    fn activate_clears_runtime_state() {
        let mut s = state(&["a", "b"]);
        let a = s.profiles[0].id;
        activate(&mut s, Some(a));
        {
            let p = s.active_profile_mut().unwrap();
            p.is_popped_out = true;
            p.is_editing_panel_source = true;
            p.current_move_resize_panel_id = Uuid::new_v4();
        }
        s.selecting_panel = Some(Uuid::new_v4());
        s.editing_lock = true;
        let b = s.profiles[1].id;
        activate(&mut s, Some(b));
        assert!(!s.profiles[0].is_popped_out);
        assert!(!s.profiles[0].is_editing_panel_source);
        assert!(s.profiles[0].current_move_resize_panel_id.is_nil());
        assert!(s.selecting_panel.is_none());
        assert!(!s.editing_lock);
    }

    #[test]
    fn aircraft_binding_rules() {
        let mut s = state(&["a", "b"]);
        s.sim.aircraft_name = Some("CESSNA".into());
        s.profiles[0].aircraft_bindings.push("CESSNA".into());
        let a = s.profiles[0].id;
        let b = s.profiles[1].id;
        s.active = Some(a);
        assert!(s.is_aircraft_bound_to_active_profile());
        assert!(s.is_allowed_add_aircraft_binding());
        s.active = Some(b);
        assert!(!s.is_aircraft_bound_to_active_profile());
        assert!(!s.is_allowed_add_aircraft_binding());
    }
}
