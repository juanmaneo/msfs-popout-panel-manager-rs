//! Modal dialogs (add profile, confirmations) and the help window.

use super::snapshot::Snapshot;
use super::widgets::{heading, link};
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
    help_topic: HelpTopic,
    show_license: bool,
}

#[derive(Default)]
struct AddProfile {
    name: String,
    copy_from: Option<Uuid>,
    /// The name has been edited at least once (errors are only shown after that).
    touched: bool,
}

/// Validate a new profile name like upstream's `ProfileNameValidationRule`; returns the
/// trimmed name to use.
fn validate_profile_name<'a>(
    name: &str,
    mut existing: impl Iterator<Item = &'a str>,
) -> Result<String, &'static str> {
    let name = name.trim();
    if existing.any(|other| other.trim().to_lowercase() == name.to_lowercase()) {
        Err("Profile name already exist")
    } else if name.is_empty() {
        Err("Profile name is required")
    } else {
        Ok(name.to_string())
    }
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
        let mut validated = None;

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
            if edit.changed() {
                state.touched = true;
            }
            if !edit.has_focus() && !state.touched {
                edit.request_focus();
            }
            if edit.lost_focus() && ui.ctx().input(|i| i.key_pressed(Key::Enter)) {
                accept = true;
            }

            validated = Some(validate_profile_name(
                &state.name,
                snap.profiles.iter().map(|(_, name)| name.as_str()),
            ));
            if state.touched
                && let Some(Err(error)) = &validated
            {
                ui.colored_label(super::theme::ERROR, *error);
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let valid = validated.as_ref().is_some_and(Result::is_ok);
                if ui.add_enabled(valid, egui::Button::new("ACCEPT")).clicked() {
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
            state.touched = true;
            if let Some(Ok(name)) = validated {
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
            .resizable(true)
            .default_size([720.0, 520.0])
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for topic in HelpTopic::ALL {
                        if ui
                            .selectable_label(self.help_topic == topic, topic.label())
                            .clicked()
                        {
                            self.help_topic = topic;
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.help_topic {
                        HelpTopic::KeyboardCommands => keyboard_commands(ui),
                        HelpTopic::UserGuide => {
                            heading(ui, "User Guide");
                            link(ui, "Getting Started", GETTING_STARTED_URL);
                            ui.add_space(6.0);
                            link(ui, "User Guide", USER_GUIDE_URL);
                        }
                        HelpTopic::DownloadLatest => {
                            heading(ui, "Download Latest Release");
                            link(ui, "Download latest release (GitHub)", GITHUB_RELEASES_URL);
                            ui.add_space(6.0);
                            link(
                                ui,
                                "Download latest release (Flightsim.to)",
                                FLIGHTSIM_TO_URL,
                            );
                        }
                        HelpTopic::Support => {
                            heading(ui, "Support");
                            ui.label(
                                "SimConnect.dll (from the MSFS SDK) must be located next to the \
                                 executable. SimConnect also requires the Microsoft Visual C++ \
                                 Redistributable to be installed.",
                            );
                        }
                        HelpTopic::About => self.about(ui),
                    });
            });
        self.help_open = open;
    }

    fn about(&mut self, ui: &mut egui::Ui) {
        heading(ui, "About");
        ui.label(format!("Version {} (Rust port)", env!("CARGO_PKG_VERSION")));
        ui.add_space(4.0);
        ui.label("Original application © 2022 Stanley Kwok. All rights reserved.");
        link(ui, "Original project on GitHub", UPSTREAM_URL);
        ui.add_space(8.0);
        ui.label(
            "This program is free software licensed under the GNU Affero General Public \
             License v3.0 (AGPL-3.0-only). It is a derivative work of the original project \
             and comes with ABSOLUTELY NO WARRANTY.",
        );
        let label = if self.show_license {
            "Hide Public Release License"
        } else {
            "Public Release License"
        };
        if ui.link(label).clicked() {
            self.show_license = !self.show_license;
        }
        if self.show_license {
            egui::ScrollArea::vertical()
                .id_salt("license_text")
                .max_height(320.0)
                .show(ui, |ui| {
                    ui.label(RichText::new(LICENSE_TEXT).monospace().small());
                });
        }
    }
}

const UPSTREAM_URL: &str = "https://github.com/hawkeye-stan/msfs-popout-panel-manager";
const GETTING_STARTED_URL: &str =
    "https://github.com/hawkeye-stan/msfs-popout-panel-manager/blob/master/GETTING_STARTED.md";
const USER_GUIDE_URL: &str =
    "https://github.com/hawkeye-stan/msfs-popout-panel-manager#msfs-pop-out-panel-manager";
const GITHUB_RELEASES_URL: &str =
    "https://github.com/hawkeye-stan/msfs-popout-panel-manager/releases";
const FLIGHTSIM_TO_URL: &str = "https://flightsim.to/file/35759/msfs-pop-out-panel-manager";
const LICENSE_TEXT: &str = include_str!("../../LICENSE");

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum HelpTopic {
    #[default]
    KeyboardCommands,
    UserGuide,
    DownloadLatest,
    Support,
    About,
}

impl HelpTopic {
    const ALL: [HelpTopic; 5] = [
        HelpTopic::KeyboardCommands,
        HelpTopic::UserGuide,
        HelpTopic::DownloadLatest,
        HelpTopic::Support,
        HelpTopic::About,
    ];

    fn label(self) -> &'static str {
        match self {
            HelpTopic::KeyboardCommands => "Keyboard Commands",
            HelpTopic::UserGuide => "User Guide",
            HelpTopic::DownloadLatest => "Download Latest Release",
            HelpTopic::Support => "Support",
            HelpTopic::About => "About",
        }
    }
}

/// Arrow-key move/resize commands of a panel in move/resize mode (see
/// [`crate::keybinding::move_resize_action`]), grouped like upstream's help drawer.
const KEYBOARD_COMMANDS: [&[(&str, &str)]; 4] = [
    &[
        ("Up Arrow", "Move panel up by 10 pixels"),
        ("Down Arrow", "Move panel down by 10 pixels"),
        ("Left Arrow", "Move panel left by 10 pixels"),
        ("Right Arrow", "Move panel right by 10 pixels"),
    ],
    &[
        ("Shift + Up Arrow", "Move panel up by 1 pixel"),
        ("Shift + Down Arrow", "Move panel down by 1 pixel"),
        ("Shift + Left Arrow", "Move panel left by 1 pixel"),
        ("Shift + Right Arrow", "Move panel right by 1 pixel"),
    ],
    &[
        ("Ctrl + Up Arrow", "Decrease height by 10 pixels"),
        ("Ctrl + Down Arrow", "Increase height by 10 pixels"),
        ("Ctrl + Left Arrow", "Decrease width by 10 pixels"),
        ("Ctrl + Right Arrow", "Increase width by 10 pixels"),
    ],
    &[
        ("Shift + Ctrl + Up Arrow", "Decrease height by 1 pixel"),
        ("Shift + Ctrl + Down Arrow", "Increase height by 1 pixel"),
        ("Shift + Ctrl + Left Arrow", "Decrease width by 1 pixel"),
        ("Shift + Ctrl + Right Arrow", "Increase width by 1 pixel"),
    ],
];

fn keyboard_commands(ui: &mut egui::Ui) {
    heading(ui, "Keyboard Commands");
    ui.label(
        "To configure a pop out panel, first turn on the Move/Resize switch of the panel. \
         You can then use the keyboard commands below to adjust the pop out panel while the \
         switch is on. To end panel configuration using keyboard commands, turn the switch \
         off again.",
    );
    ui.add_space(10.0);
    let key_color = super::theme::rgb(0x87CEFA); // LightSkyBlue
    for group in KEYBOARD_COMMANDS {
        for (keys, action) in group {
            ui.horizontal(|ui| {
                ui.label(RichText::new(*keys).color(key_color));
                ui.label(format!("- {action}"));
            });
        }
        ui.add_space(8.0);
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

#[cfg(test)]
mod tests {
    use super::validate_profile_name;

    #[test]
    fn profile_name_validation() {
        let existing = ["Cessna 172", "A320"];
        let check = |name: &str| validate_profile_name(name, existing.iter().copied());
        assert_eq!(check("  "), Err("Profile name is required"));
        assert_eq!(check("cessna 172"), Err("Profile name already exist"));
        assert_eq!(check(" A320 "), Err("Profile name already exist"));
        assert_eq!(check(" Baron "), Ok("Baron".to_string()));
    }
}
