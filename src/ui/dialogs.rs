//! Modal dialogs (add profile, confirmations) and the help window.

use super::snapshot::Snapshot;
use super::widgets::heading;
use crate::core::Core;
use eframe::egui::{self, Context, Id, Key, Modal, RichText};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
pub struct Dialogs {
    add_profile: Option<AddProfile>,
    confirm_delete_profile: bool,
    confirm_delete_panel: Option<Uuid>,
    help_open: bool,
}

#[derive(Default)]
struct AddProfile {
    name: String,
    copy_from: Option<Uuid>,
    error: Option<String>,
}

enum Choice {
    Pending,
    Confirm,
    Cancel,
}

impl Dialogs {
    pub fn open_add_profile(&mut self) {
        self.add_profile = Some(AddProfile::default());
    }

    pub fn open_delete_profile(&mut self) {
        self.confirm_delete_profile = true;
    }

    pub fn open_delete_panel(&mut self, panel_id: Uuid) {
        self.confirm_delete_panel = Some(panel_id);
    }

    pub fn toggle_help(&mut self) {
        self.help_open = !self.help_open;
    }

    pub fn show(&mut self, ctx: &Context, core: &Arc<Core>, snap: &Snapshot) {
        self.show_add_profile(ctx, core, snap);

        if self.confirm_delete_profile {
            match confirm(
                ctx,
                "confirm_delete_profile",
                "WARNING! Are you sure you want to delete the profile?",
                "Delete",
            ) {
                Choice::Confirm => {
                    self.confirm_delete_profile = false;
                    core.delete_active_profile();
                }
                Choice::Cancel => self.confirm_delete_profile = false,
                Choice::Pending => {}
            }
        }

        if let Some(panel_id) = self.confirm_delete_panel {
            match confirm(
                ctx,
                "confirm_delete_panel",
                "Are you sure you want to delete the panel?",
                "Delete",
            ) {
                Choice::Confirm => {
                    self.confirm_delete_panel = None;
                    core.remove_panel(panel_id);
                }
                Choice::Cancel => self.confirm_delete_panel = None,
                Choice::Pending => {}
            }
        }

        self.show_help(ctx);
    }

    fn show_add_profile(&mut self, ctx: &Context, core: &Arc<Core>, snap: &Snapshot) {
        let Some(state) = self.add_profile.as_mut() else {
            return;
        };
        let mut accept = false;
        let mut cancel = false;

        Modal::new(Id::unique("add_profile_modal")).show(ctx, |ui| {
            ui.set_width(360.0);
            heading(ui, "Add Profile");
            ui.add_space(6.0);

            ui.label("Copy From Profile");
            let selected_text = state
                .copy_from
                .and_then(|id| snap.profiles.iter().find(|(pid, _)| *pid == id))
                .map_or("(None)", |(_, name)| name.as_str());
            egui::ComboBox::from_id_salt("copy_from_profile")
                .selected_text(selected_text)
                .width(340.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.copy_from, None, "(None)");
                    for (id, name) in &snap.profiles {
                        ui.selectable_value(&mut state.copy_from, Some(*id), name);
                    }
                });

            ui.add_space(4.0);
            ui.label("Profile Name");
            let edit = ui.add(
                egui::TextEdit::singleline(&mut state.name)
                    .desired_width(340.0)
                    .hint_text("Aircraft profile name"),
            );
            if !edit.has_focus() && state.name.is_empty() && state.error.is_none() {
                edit.request_focus();
            }
            if edit.lost_focus() && ui.ctx().input(|i| i.key_pressed(Key::Enter)) {
                accept = true;
            }

            if let Some(error) = &state.error {
                ui.colored_label(super::theme::ERROR, error);
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("ACCEPT").clicked() {
                    accept = true;
                }
                if ui.button("CANCEL").clicked() {
                    cancel = true;
                }
            });
        });

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            cancel = true;
        }

        if accept {
            let name = state.name.trim().to_string();
            state.error = if name.is_empty() {
                Some("Profile name is required".to_string())
            } else if snap
                .profiles
                .iter()
                .any(|(_, existing)| existing.trim().eq_ignore_ascii_case(&name))
            {
                Some("Profile name already exist".to_string())
            } else {
                None
            };
            if state.error.is_none() {
                core.add_profile(&name, state.copy_from);
                self.add_profile = None;
            }
        } else if cancel {
            self.add_profile = None;
        }
    }

    fn show_help(&mut self, ctx: &Context) {
        let mut open = self.help_open;
        egui::Window::new("Help")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("MSFS Pop Out Panel Manager")
                        .strong()
                        .size(18.0),
                );
                ui.label(RichText::new(format!(
                    "Version {} (Rust port)",
                    env!("CARGO_PKG_VERSION")
                ))
                .weak());
                ui.add_space(6.0);

                heading(ui, "Documentation");
                help_link(ui, "Getting Started", "https://github.com/hawkeye-stan/msfs-popout-panel-manager/blob/master/GETTING_STARTED.md");
                help_link(ui, "User Guide", "https://github.com/hawkeye-stan/msfs-popout-panel-manager#msfs-pop-out-panel-manager");
                help_link(ui, "Download Latest GitHub", "https://github.com/hawkeye-stan/msfs-popout-panel-manager/releases");
                help_link(ui, "Download Latest Flightsim.to", "https://flightsim.to/file/35759/msfs-pop-out-panel-manager");
                ui.add_space(4.0);

                heading(ui, "Licence");
                ui.label(
                    "This program is free software licensed under the GNU Affero General \
                     Public License v3.0 (AGPL-3.0-only). It is a derivative work of the \
                     upstream project above and comes with ABSOLUTELY NO WARRANTY.",
                );
                ui.add_space(8.0);
                ui.separator();
                heading(ui, "Move / resize with the keyboard");
                ui.label(
                    "Turn on the move/resize button of a popped out panel, then use the arrow \
                     keys to move it. Hold Shift for fine steps and Ctrl to resize instead \
                     of move.",
                );
                ui.add_space(4.0);
                heading(ui, "SimConnect");
                ui.label("SimConnect.dll must be located next to the executable.");
            });
        self.help_open = open;
    }
}

/// A labelled hyperlink that opens with the default Windows browser.
fn help_link(ui: &mut egui::Ui, label: &str, url: &str) {
    if ui
        .add(egui::Button::new(
            RichText::new(label)
                .color(ui.visuals().hyperlink_color)
                .underline(),
        ))
        .on_hover_text(url)
        .clicked()
    {
        crate::win::open_externally(url);
    }
}

fn confirm(ctx: &Context, id: &str, message: &str, confirm_label: &str) -> Choice {
    let mut choice = Choice::Pending;
    Modal::new(Id::unique(id)).show(ctx, |ui| {
        ui.set_width(340.0);
        ui.label(message);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(confirm_label).clicked() {
                choice = Choice::Confirm;
            }
            if ui.button("Cancel").clicked() {
                choice = Choice::Cancel;
            }
        });
    });
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        choice = Choice::Cancel;
    }
    choice
}
