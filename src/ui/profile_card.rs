//! The active profile: name, lock, aircraft binding and profile level options.

use super::snapshot::Snapshot;
use super::widgets::{ResponseExt, toggle_labeled};
use super::{edit, theme};
use crate::core::Core;
use crate::model::{HudBarType, UserProfile};
use crate::win::window;
use eframe::egui::{self, RichText, TextStyle, Ui};
use std::sync::Arc;

pub fn show(ui: &mut Ui, core: &Arc<Core>, snap: &Snapshot, profile: &UserProfile) {
    theme::card_frame(theme::CARD_FILL, egui::Color32::from_gray(70)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        title_row(ui, core, profile);
        aircraft_row(ui, core, snap);
        ui.separator();
        ui.add_enabled_ui(!profile.is_locked, |ui| options(ui, core, profile));
        hints(ui, snap, profile);
    });
}

fn title_row(ui: &mut Ui, core: &Arc<Core>, profile: &UserProfile) {
    ui.horizontal(|ui| {
        let mut name = profile.name.clone();
        let response = ui.add_enabled(
            !profile.is_locked,
            egui::TextEdit::singleline(&mut name)
                .font(TextStyle::Heading)
                .desired_width(340.0)
                .hint_text("Profile name"),
        );
        if response.changed() {
            let new_name = name.clone();
            edit::profile(core, profile.id, |p| p.name = new_name);
        }
        if response.lost_focus() {
            if name.trim().is_empty() {
                edit::profile(core, profile.id, |p| p.name = "Untitled".to_string());
            }
            core.profile_renamed();
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut locked = profile.is_locked;
            let can_lock = !profile.panel_configs.is_empty();
            let response = ui
                .add_enabled_ui(can_lock, |ui| toggle_labeled(ui, &mut locked, "Lock profile"))
                .inner
                .tip(if can_lock {
                    "Lock the profile to protect its panel layout; panels are re-applied after a pop out"
                } else {
                    "Add pop out panels to the profile before locking it"
                });
            if response.changed() {
                core.toggle_lock();
            }
        });
    });
}

fn aircraft_row(ui: &mut Ui, core: &Arc<Core>, snap: &Snapshot) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Active Aircraft:");
        match &snap.sim.aircraft {
            Some(name) => {
                let color = if snap.aircraft_bound {
                    theme::LIME_GREEN
                } else {
                    ui.visuals().text_color()
                };
                ui.label(RichText::new(name).color(color).strong());
            }
            None => {
                ui.label(RichText::new("N/A").color(theme::ANTIQUE_WHITE))
                    .on_hover_text("No aircraft has been loaded by the game yet");
            }
        }

        let can_bind = snap.sim.aircraft.is_some() && snap.binding_allowed && snap.sim.simulator_started;
        let mut bound = snap.aircraft_bound;
        let response = ui
            .add_enabled_ui(can_bind, |ui| {
                toggle_labeled(
                    ui,
                    &mut bound,
                    if snap.aircraft_bound {
                        "Aircraft bound to this profile"
                    } else {
                        "Aircraft not bound to this profile"
                    },
                )
            })
            .inner
            .tip(if snap.binding_allowed {
                "Bind the loaded aircraft to this profile so the profile is selected and popped out automatically"
            } else {
                "The loaded aircraft is already bound to another profile"
            });
        if response.changed() {
            core.toggle_aircraft_binding();
        }

        if snap.sim.aircraft.is_some() {
            if snap.auto_pop_out_enabled && snap.aircraft_bound {
                ui.colored_label(egui::Color32::LIGHT_GREEN, "(Auto Pop Out is Active)");
            } else {
                ui.colored_label(egui::Color32::RED, "(Auto Pop Out is Inactive)");
            }
        }
    });
}

fn options(ui: &mut Ui, core: &Arc<Core>, profile: &UserProfile) {
    let id = profile.id;
    let setting = &profile.profile_setting;

    ui.columns(2, |columns| {
        let left = &mut columns[0];

        let mut legacy = profile.is_used_legacy_camera_system;
        let response = left
            .add_enabled_ui(!profile.is_editing_panel_source, |ui| {
                toggle_labeled(ui, &mut legacy, "Use legacy custom camera to select panel source")
            })
            .inner
            .tip("Identify panel source locations with the legacy custom camera instead of fixed cameras");
        if response.changed() {
            edit::profile(core, id, |p| p.is_used_legacy_camera_system = legacy);
        }

        let mut include = setting.include_in_game_panels;
        let response = toggle_labeled(left, &mut include, "Include in-game panels").tip(
            "Add in-game menu bar panels such as VFR Map, Checklist, ATC, etc. to the profile. \
             Panels that are open in the game are popped out and can be positioned.",
        );
        if response.changed() {
            edit::profile(core, id, |p| {
                p.profile_setting.include_in_game_panels = include;
                p.on_include_in_game_panels_changed();
            });
        }

        let mut power_on = setting.power_on_required_for_cold_start;
        let response = toggle_labeled(left, &mut power_on, "Power on required for cold start").tip(
            "Turn on the aircraft battery before popping out panels when the aircraft starts \
             from a cold and dark state",
        );
        if response.changed() {
            edit::profile(core, id, |p| p.profile_setting.power_on_required_for_cold_start = power_on);
        }

        let mut num_pad = setting.num_pad_config.is_enabled;
        let response = toggle_labeled(left, &mut num_pad, "Virtual NumPad").tip(
            "Add a virtual keyboard NumPad to the game that has MSFS focused before sending key command",
        );
        if response.changed() {
            edit::profile(core, id, |p| {
                p.profile_setting.num_pad_config.is_enabled = num_pad;
                p.on_num_pad_changed();
            });
        }

        let right = &mut columns[1];
        hud_bar_row(right, core, profile);
        refocus_section(right, core, profile);
    });
}

fn hud_bar_row(ui: &mut Ui, core: &Arc<Core>, profile: &UserProfile) {
    let id = profile.id;
    let config = &profile.profile_setting.hud_bar_config;

    ui.horizontal(|ui| {
        let mut enabled = config.is_enabled;
        let response =
            toggle_labeled(ui, &mut enabled, "HUD bar").tip("Add an HUD bar to the game");
        if response.changed() {
            edit::profile(core, id, |p| {
                p.profile_setting.hud_bar_config.is_enabled = enabled;
                p.on_hud_bar_changed();
            });
        }

        ui.add_enabled_ui(config.is_enabled, |ui| {
            let mut selected = config.hud_bar_type;
            egui::ComboBox::from_id_salt("hud_bar_type")
                .selected_text(selected.label())
                .width(150.0)
                .show_ui(ui, |ui| {
                    for kind in HudBarType::SELECTABLE {
                        ui.selectable_value(&mut selected, kind, kind.label());
                    }
                });
            if selected != config.hud_bar_type {
                edit::profile(core, id, |p| {
                    p.profile_setting.hud_bar_config.hud_bar_type = selected
                });
            }
        });
    });
}

fn refocus_section(ui: &mut Ui, core: &Arc<Core>, profile: &UserProfile) {
    let id = profile.id;
    let refocus = &profile.profile_setting.refocus_on_display;

    let mut enabled = refocus.is_enabled;
    let response = toggle_labeled(ui, &mut enabled, "Refocus on display").tip(
        "Automatically set focus back to the game window when the selected displays are touched or clicked",
    );
    if response.changed() {
        let monitors = window::monitors_sorted();
        edit::profile(core, id, |p| {
            p.profile_setting.refocus_on_display.is_enabled = enabled;
            p.on_refocus_display_changed(&monitors);
        });
    }

    if !refocus.is_enabled {
        return;
    }

    ui.indent("refocus_monitors", |ui| {
        for monitor in &refocus.monitors {
            let mut selected = monitor.is_selected;
            let label = format!("{} ({}x{})", monitor.name, monitor.width, monitor.height);
            if ui.checkbox(&mut selected, label).changed() {
                let name = monitor.name.clone();
                edit::profile(core, id, |p| {
                    if let Some(m) = p
                        .profile_setting
                        .refocus_on_display
                        .monitors
                        .iter_mut()
                        .find(|m| m.name == name)
                    {
                        m.is_selected = selected;
                    }
                    p.on_refocus_display_selection_changed(&name);
                });
            }
        }
        if ui.button("Refresh Display List").clicked() {
            let monitors = window::monitors_sorted();
            edit::profile(core, id, |p| p.on_refocus_display_refreshed(&monitors));
        }
    });
}

fn hints(ui: &mut Ui, snap: &Snapshot, profile: &UserProfile) {
    let message = if snap.pop_out_in_progress {
        Some("Pop out in progress. Please wait and do not move your mouse.")
    } else if profile.has_unidentified_panel_source() {
        Some("Please identify all source panel locations to enable Start Pop Out")
    } else if profile.is_editing_panel_source {
        Some(
            "Please complete editing of source panel locations by clicking Edit Panel Sources to enable Start Pop Out",
        )
    } else {
        None
    };
    if let Some(message) = message {
        ui.add_space(4.0);
        theme::card_frame(egui::Color32::from_rgb(60, 52, 30), theme::NAVAJO_WHITE).show(
            ui,
            |ui| {
                ui.set_width(ui.available_width());
                ui.colored_label(theme::NAVAJO_WHITE, message);
            },
        );
    }
}
