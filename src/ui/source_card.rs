//! Source cards, shown instead of the panel cards while a profile's panel sources are edited
//! (port of `PopOutPanelSourceCard.xaml` and `PopOutPanelSourceLegacyCard.xaml`): panel name,
//! the identified source coordinates, the fixed camera and the "Identify" button.

use super::panel_card::set_panel_field;
use super::snapshot::Snapshot;
use super::widgets::{Icon, ResponseExt, icon_button, paint_icon};
use super::{edit, theme};
use crate::core::config::PanelProp;
use crate::core::{Core, fixed_camera_configs};
use crate::model::{FixedCameraConfig, PanelConfig, UserProfile, color_rgb};
use eframe::egui::{
    self, Button, Color32, CursorIcon, RichText, Sense, TextEdit, TextStyle, Ui, UiBuilder, Vec2,
};
use std::sync::Arc;
use uuid::Uuid;

/// Upstream `#525252`: card background of the panel whose source is being edited.
const HIGHLIGHT_FILL: Color32 = Color32::from_rgb(0x52, 0x52, 0x52);
const FIELD_WIDTH: f32 = 85.0;
const CAMERA_COMBO_WIDTH: f32 = 120.0;

pub fn show(
    ui: &mut Ui,
    core: &Arc<Core>,
    snap: &Snapshot,
    profile: &UserProfile,
    panel: &PanelConfig,
) {
    let legacy = profile.is_used_legacy_camera_system;
    // The fixed camera cards select a source by clicking the card (upstream: the card header).
    let clickable = !legacy && panel.is_custom_pop_out();
    let highlighted = panel.is_selected_panel_source || (!legacy && panel.is_editing_panel);
    let fill = if highlighted {
        HIGHLIGHT_FILL
    } else {
        theme::CARD_FILL
    };
    let sense = if clickable {
        Sense::click()
    } else {
        Sense::hover()
    };

    let card = ui.scope_builder(UiBuilder::new().sense(sense), |ui| {
        // Plain labels must not swallow the click meant for the card.
        ui.style_mut().interaction.selectable_labels = false;
        theme::card_frame(fill, Color32::from_gray(70)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            header_row(ui, core, snap, profile, panel);
            if snap.selecting_panel == Some(panel.id) {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        theme::ACCENT,
                        "Click the panel source location in the game...",
                    );
                    if ui.button("Cancel").clicked() {
                        core.cancel_panel_selection();
                    }
                });
            }
        });
    });

    if clickable {
        let response = card.response.on_hover_cursor(CursorIcon::PointingHand);
        if response.clicked() {
            core.select_panel_source_for_edit(panel.id);
        }
    }
}

fn header_row(
    ui: &mut Ui,
    core: &Arc<Core>,
    snap: &Snapshot,
    profile: &UserProfile,
    panel: &PanelConfig,
) {
    let legacy = profile.is_used_legacy_camera_system;
    ui.horizontal(|ui| {
        let mut name = panel.panel_name.clone();
        let response = ui.add_enabled(
            panel.is_custom_pop_out(),
            TextEdit::singleline(&mut name)
                .font(TextStyle::Button)
                .desired_width(260.0)
                .hint_text("Panel Name"),
        );
        if response.changed() {
            set_panel_field(core, profile.id, panel.id, PanelProp::PanelName, |p| {
                p.panel_name = name
            });
        }

        if panel.is_custom_pop_out() {
            read_only_field(ui, "Panel Src Left", panel.panel_source.x);
            read_only_field(ui, "Panel Src Top", panel.panel_source.y);
            if !legacy {
                camera_field(ui, core, snap, profile.id, panel);
            }
            if panel.panel_source.x.is_none() {
                let can_identify = !profile.is_selecting_panel_source && snap.sim.in_cockpit;
                let identify = ui
                    .add_enabled(can_identify, Button::new("Identify"))
                    .tip("Identify source aircraft instrumentation panel location");
                if identify.clicked() {
                    core.start_panel_selection(panel.id, legacy);
                }
            }
        }

        if panel.is_deletable() && panel.panel_source.x.is_some() {
            crosshair(ui, panel);
        }
    });
}

/// A value shown under a small caption (upstream: read-only floating hint text boxes).
fn read_only_field(ui: &mut Ui, caption: &str, value: Option<i32>) {
    ui.vertical(|ui| {
        ui.set_min_width(FIELD_WIDTH);
        ui.label(RichText::new(caption).small().weak());
        ui.label(value.map_or_else(|| "N/A".to_string(), |v| v.to_string()));
    });
}

/// The source marker icon in the colour of the panel's ring in the game.
fn crosshair(ui: &mut Ui, panel: &PanelConfig) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
    let color = theme::rgb(color_rgb(panel.panel_source.color.as_deref()));
    paint_icon(ui.painter(), rect, Icon::Crosshair, color);
    response.tip("Click the panel to show its source location in the game");
}

/// Index of `current` in `configs` (cameras are identified by type and index; the list grows
/// with the aircraft's instrument count).
fn camera_index(configs: &[FixedCameraConfig], current: &FixedCameraConfig) -> Option<usize> {
    configs.iter().position(|c| {
        c.camera_type == current.camera_type && c.camera_index == current.camera_index
    })
}

/// The camera one step before/after `current`, wrapping around. A camera that is not in the list
/// (e.g. after switching aircraft) steps to the first/last one.
fn step_camera(count: usize, current: Option<usize>, forward: bool) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(match (current, forward) {
        (None, true) => 0,
        (None, false) => last,
        (Some(i), true) => (i + 1) % count,
        (Some(i), false) => i.checked_sub(1).unwrap_or(last),
    })
}

/// Upstream `PanelTargetField`: the camera name while idle; previous / combo / next while this
/// panel's source is being edited. Any change previews the camera in the simulator.
fn camera_field(
    ui: &mut Ui,
    core: &Arc<Core>,
    snap: &Snapshot,
    profile_id: Uuid,
    panel: &PanelConfig,
) {
    let current_config = &panel.fixed_camera_config;
    if !panel.is_editing_panel {
        ui.vertical(|ui| {
            ui.set_min_width(FIELD_WIDTH + 55.0);
            ui.label(RichText::new("Fixed Camera").small().weak());
            let name = current_config.name.trim();
            ui.label(if name.is_empty() { "N/A" } else { name });
        });
        return;
    }

    let configs = fixed_camera_configs(snap.sim.instrument_count);
    let current = camera_index(&configs, current_config);
    let mut chosen = None;
    ui.horizontal(|ui| {
        if icon_button(ui, Icon::Left, "Previous fixed camera").clicked() {
            chosen = step_camera(configs.len(), current, false);
        }
        let shown = current
            .and_then(|i| configs.get(i))
            .map_or(current_config.name.as_str(), |c| c.name.as_str());
        egui::ComboBox::from_id_salt(("fixed_camera", panel.id))
            .selected_text(shown)
            .width(CAMERA_COMBO_WIDTH)
            .show_ui(ui, |ui| {
                for (i, config) in configs.iter().enumerate() {
                    if ui
                        .selectable_label(current == Some(i), &config.name)
                        .clicked()
                    {
                        chosen = Some(i);
                    }
                }
            });
        if icon_button(ui, Icon::Right, "Next fixed camera").clicked() {
            chosen = step_camera(configs.len(), current, true);
        }
    });

    if let Some(index) = chosen
        && current != Some(index)
        && let Some(config) = configs.get(index)
    {
        let config = config.clone();
        edit::panel(core, profile_id, panel.id, |p| {
            p.fixed_camera_config = config
        });
        core.set_camera_for_panel(panel.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CameraType;

    #[test]
    fn stepping_wraps_around_both_ways() {
        assert_eq!(step_camera(4, Some(3), true), Some(0));
        assert_eq!(step_camera(4, Some(0), false), Some(3));
        assert_eq!(step_camera(4, Some(1), true), Some(2));
        assert_eq!(step_camera(4, Some(2), false), Some(1));
    }

    #[test]
    fn stepping_from_an_unknown_camera_enters_the_list() {
        assert_eq!(step_camera(4, None, true), Some(0));
        assert_eq!(step_camera(4, None, false), Some(3));
    }

    #[test]
    fn stepping_an_empty_list_does_nothing() {
        assert_eq!(step_camera(0, None, true), None);
        assert_eq!(step_camera(0, Some(0), false), None);
    }

    #[test]
    fn a_single_camera_steps_onto_itself() {
        assert_eq!(step_camera(1, Some(0), true), Some(0));
        assert_eq!(step_camera(1, Some(0), false), Some(0));
    }

    #[test]
    fn cameras_are_found_by_type_and_index() {
        let configs = fixed_camera_configs(2);
        let copilot = FixedCameraConfig {
            id: 99,
            camera_type: CameraType::Cockpit,
            camera_index: 5,
            name: "stale name".into(),
        };
        assert_eq!(camera_index(&configs, &copilot), Some(1));
        let missing = FixedCameraConfig {
            id: 7,
            camera_type: CameraType::Instrument,
            camera_index: 9,
            name: String::new(),
        };
        assert_eq!(camera_index(&configs, &missing), None);
    }
}
