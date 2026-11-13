//! The main window: status header, profile list, active profile and the action bar.

use super::dialogs::Dialogs;
use super::preferences::Preferences;
use super::snapshot::{Snapshot, capture};
use super::theme;
use super::widgets::{ResponseExt, heading, status_dot};
use super::{panel_card, profile_card};
use crate::core::Core;
use crate::model::UserProfile;
use eframe::egui::{
    self, Align, Atom, Button, CentralPanel, Layout, Panel, Response, RichText, ScrollArea,
    TextWrapMode, Ui, vec2,
};
use std::sync::Arc;

#[derive(Default)]
pub struct MainWindow {
    dialogs: Dialogs,
    preferences: Preferences,
    profile_filter: String,
}

impl MainWindow {
    pub fn show(&mut self, ui: &mut Ui, core: &Arc<Core>, on_status: &mut dyn FnMut()) {
        let snap = capture(core);

        // Escape abandons an in-progress keystroke detection (shortcut or floating panel).
        let detecting = core.read(|s| {
            s.settings.keyboard_shortcut_setting.is_detecting_keystroke
                || s.active_profile().is_some_and(|p| {
                    p.panel_configs
                        .iter()
                        .any(|c| c.floating_panel.is_detecting_keystroke)
                })
        });
        if detecting && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            core.cancel_detect_keystroke();
        }

        Panel::top("header").show(ui, |ui| self.header(ui, &snap, on_status));
        Panel::bottom("action_bar").show(ui, |ui| action_bar(ui, core, &snap));
        Panel::left("profile_list")
            .resizable(false)
            .exact_size(250.0)
            .show(ui, |ui| self.profile_list(ui, core, &snap));
        CentralPanel::default().show(ui, |ui| self.center(ui, core, &snap));

        let ctx = ui.ctx().clone();
        self.dialogs.show(&ctx, core, &snap);
        self.preferences.show(&ctx, core, &snap);
    }

    fn header(&mut self, ui: &mut Ui, snap: &Snapshot, on_status: &mut dyn FnMut()) {
        ui.horizontal(|ui| {
            let (color, state) = if snap.sim.connected {
                (theme::SUCCESS, "Connected")
            } else {
                (theme::ERROR, "Not connected")
            };
            status_dot(
                ui,
                color,
                "SimConnect connection to Microsoft Flight Simulator",
            );
            ui.label(RichText::new(format!("SimConnect: {state}")).strong());
            ui.separator();

            match &snap.sim.aircraft {
                Some(name) => ui.label(format!("Aircraft: {name}")),
                None => ui.label(RichText::new("Aircraft: none loaded").weak()),
            };
            ui.separator();
            if snap.sim.in_cockpit {
                ui.colored_label(theme::SUCCESS, "In cockpit");
            } else {
                ui.label(RichText::new("Not in cockpit").weak());
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Status").clicked() {
                    on_status();
                }
                if ui.button("Help").clicked() {
                    self.dialogs.toggle_help();
                }
                if ui.button("Preferences").clicked() {
                    self.preferences.open();
                }
            });
        });

        if let Some(error) = &snap.sim_error {
            ui.add_space(4.0);
            theme::card_frame(egui::Color32::from_rgb(74, 28, 28), theme::ERROR).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.colored_label(theme::INDIAN_RED, error);
            });
        }
    }

    fn profile_list(&mut self, ui: &mut Ui, core: &Arc<Core>, snap: &Snapshot) {
        // Search box (upstream "Find an aircraft profile" auto-complete)
        ui.horizontal(|ui| {
            let mut query = self.profile_filter.clone();
            let response = ui.add(
                egui::TextEdit::singleline(&mut query)
                    .desired_width(ui.available_width() - 10.0)
                    .hint_text("Search profile..."),
            );
            if response.changed() {
                self.profile_filter = query;
            }
        });
        ui.add_space(4.0);
        heading(ui, "Aircraft Profiles");
        ui.add_space(4.0);

        // Leave room for the two rows of buttons below the list.
        let list_height = (ui.available_height() - 72.0).max(60.0);
        ui.add_enabled_ui(!snap.pop_out_in_progress, |ui| {
            ScrollArea::vertical()
                .id_salt("profiles_scroll")
                .max_height(list_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let active = snap.active.as_ref().map(|p| p.id);
                    let filter = self.profile_filter.trim().to_lowercase();
                    for (id, name) in &snap.profiles {
                        if !filter.is_empty() && !name.to_lowercase().contains(&filter) {
                            continue;
                        }
                        let selected = Some(*id) == active;
                        let label = if name.trim().is_empty() {
                            "(unnamed)"
                        } else {
                            name
                        };
                        let response = ui.add_sized(
                            [ui.available_width(), 26.0],
                            Button::selectable(selected, label).truncate(),
                        );
                        if response.clicked() && !selected {
                            core.set_active_profile(*id);
                        }
                    }
                });
        });

        ui.add_space(4.0);
        ui.add_enabled_ui(!snap.pop_out_in_progress, |ui| {
            // Full-width buttons in two columns: the list is too narrow for a single row.
            let several = snap.profiles.len() > 1;
            let has_active = snap.active.is_some();
            ui.columns(2, |cols| {
                if full_width_button(&mut cols[0], true, "Add profile")
                    .tip("Add a new aircraft profile")
                    .clicked()
                {
                    self.dialogs.open_add_profile();
                }
                if full_width_button(&mut cols[1], has_active, "Delete profile")
                    .tip("Delete the selected aircraft profile")
                    .clicked()
                {
                    self.dialogs.open_delete_profile();
                }
            });
            ui.columns(2, |cols| {
                if full_width_button(&mut cols[0], several, "◀")
                    .tip("Previous profile")
                    .clicked()
                {
                    core.move_to_previous_profile();
                }
                if full_width_button(&mut cols[1], several, "▶")
                    .tip("Next profile")
                    .clicked()
                {
                    core.move_to_next_profile();
                }
            });
        });
    }

    fn center(&mut self, ui: &mut Ui, core: &Arc<Core>, snap: &Snapshot) {
        let Some(profile) = &snap.active else {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No profile").size(22.0).strong());
                ui.label("Get started by adding a new aircraft profile");
                ui.add_space(8.0);
                if ui.button("Add profile").clicked() {
                    self.dialogs.open_add_profile();
                }
            });
            return;
        };

        ScrollArea::vertical()
            .id_salt("center_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_enabled_ui(!snap.pop_out_in_progress, |ui| {
                    profile_card::show(ui, core, snap, profile);
                    ui.add_space(8.0);
                    panel_card::show_list(ui, core, snap, profile, &mut self.dialogs);
                });
            });
    }
}

fn can_start_pop_out(profile: &UserProfile, snap: &Snapshot) -> bool {
    let setting = &profile.profile_setting;
    (!profile.panel_configs.is_empty()
        || setting.include_in_game_panels
        || setting.hud_bar_config.is_enabled)
        && !profile.has_unidentified_panel_source()
        && !profile.is_editing_panel_source
        && !snap.pop_out_in_progress
        && snap.sim.in_cockpit
}

fn full_width_button(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    // The growing atoms on both sides keep the label centred in the stretched button.
    ui.add_enabled(
        enabled,
        Button::new((Atom::grow(), text, Atom::grow()))
            .wrap_mode(TextWrapMode::Truncate)
            .min_size(vec2(ui.available_width(), 24.0)),
    )
}

fn action_bar(ui: &mut Ui, core: &Arc<Core>, snap: &Snapshot) {
    let profile = snap.active.as_ref();
    let in_cockpit = snap.sim.in_cockpit;
    let has_panels = profile.is_some_and(|p| !p.panel_configs.is_empty());
    let locked = profile.is_some_and(|p| p.is_locked);
    let editing = profile.is_some_and(|p| p.is_editing_panel_source);

    ui.horizontal(|ui| {
        let add_panel = ui
            .add_enabled(
                profile.is_some() && !locked && in_cockpit,
                Button::new("Add Panel"),
            )
            .tip("Add a new pop out panel by identifying its location on the aircraft instrumentation. You must be in cockpit view.");
        if add_panel.clicked() {
            core.add_panel_and_edit();
        }

        let edit_sources = ui
            .add_enabled(
                has_panels && !locked && in_cockpit,
                Button::new("Edit Panel Sources").selected(editing),
            )
            .tip("Show and edit the location of the panel sources in the game");
        if edit_sources.clicked() {
            core.toggle_edit_panel_sources();
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let close = ui
                .add_enabled(
                    has_panels && profile.is_some_and(|p| p.is_popped_out) && in_cockpit,
                    Button::new("Close Pop Outs"),
                )
                .tip("Close all pop out panels");
            if close.clicked() {
                core.close_pop_out();
            }

            let start = ui
                .add_enabled(
                    profile.is_some_and(|p| can_start_pop_out(p, snap)),
                    Button::new(RichText::new("Start Pop Out").strong()).fill(theme::ACCENT),
                )
                .tip("Start popping out the panels of this profile. You must be in cockpit view.");
            if start.clicked() {
                core.manual_pop_out();
            }
        });
    });
}
