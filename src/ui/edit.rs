//! Targeted state edits. Each helper takes the lock briefly and changes one profile / panel
//! looked up by id, so concurrent runtime updates from worker threads are never clobbered.

use crate::core::Core;
use crate::model::{PanelConfig, UserProfile};
use uuid::Uuid;

pub fn profile(core: &Core, profile_id: Uuid, change: impl FnOnce(&mut UserProfile)) {
    core.write(|s| {
        if let Some(p) = s.profiles.iter_mut().find(|p| p.id == profile_id) {
            change(p);
        }
    });
}

pub fn panel(core: &Core, profile_id: Uuid, panel_id: Uuid, change: impl FnOnce(&mut PanelConfig)) {
    profile(core, profile_id, |p| {
        if let Some(c) = p.panel_mut(panel_id) {
            change(c);
        }
    });
}
