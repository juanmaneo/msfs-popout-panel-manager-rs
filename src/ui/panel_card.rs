use super::dialogs::Dialogs;
use super::snapshot::Snapshot;
use super::theme;
use super::widgets::{heading, toggle_labeled};
use crate::core::Core;
use crate::model::{PanelConfig, UserProfile};
use eframe::egui::{self, Align, Layout, RichText, ScrollArea, TextEdit, TextStyle, Ui};
use std::sync::Arc;

pub fn show_list(
    ui: &mut Ui,
    core: &Arc<Core>,
    snap: &Snapshot,
    profile: &UserProfile,
    dialogs: &mut Dialogs,
) {
    heading(ui, "Panels");
    ui.add_space(4.0);

    if profile.panel_configs.is_empty() {
        ui.label(RichText::new("No panels added yet").weak());
        return;
    }

    let list_height = (ui.available_height() - 44.0).max(60.0);
    ScrollArea::vertical()
        .id_salt("panels_scroll")
        .max_height(list_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for panel in &profile.panel_configs {
                show_panel_card(ui, core, snap, profile, panel, dialogs);
                ui.add_space(6.0);
            }
        });
}

fn show_panel_card(
    ui: &mut Ui,
    core: &Arc<Core>,
    snap: &Snapshot,
    profile: &UserProfile,
    panel: &PanelConfig,
    dialogs: &mut Dialogs,
) {
    let is_popped_out = profile.is_popped_out;
    let is_locked = profile.is_locked;
    let is_editing = profile.is_editing_panel_source;
    let is_selecting = snap.selecting_panel == Some(panel.id);

    theme::card_frame(theme::CARD_FILL, egui::Color32::from_gray(70)).show(ui, |ui| {
        ui.set_width(ui.available_width());

        // Header row with panel name and actions
        ui.horizontal(|ui| {
            // Panel name
            let mut name = panel.panel_name.clone();
            let response = ui.add_enabled(
                !is_locked && !is_editing && !is_selecting,
                TextEdit::singleline(&mut name)
                    .font(TextStyle::Button)
                    .desired_width(200.0)
                    .hint_text("Panel name"),
            );
            if response.changed() {
                core.write(|s| {
                    if let Some(p) = s.active_profile_mut() {
                        if let Some(panel) = p.panel_configs.iter_mut().find(|p| p.id == panel.id) {
                            panel.panel_name = name;
                        }
                    }
                });
            }

            // Panel type indicator
            ui.colored_label(theme::ACCENT, format!("[{:?}]", panel.panel_type));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Delete button
                if ui.button("×").clicked() {
                    dialogs.open_delete_panel(panel.id);
                }
            });
        });

        ui.add_space(4.0);

        // Panel properties - position and size
        ui.horizontal(|ui| {
            // Left column
            ui.vertical(|ui| {
                position_size_section(
                    ui,
                    core,
                    panel,
                    is_popped_out,
                    is_locked,
                    is_editing,
                    is_selecting,
                );
            });

            ui.separator();

            // Right column
            ui.vertical(|ui| {
                display_options_section(
                    ui,
                    core,
                    panel,
                    is_popped_out,
                    is_locked,
                    is_editing,
                    is_selecting,
                );
            });
        });

        // Status indicators
        if is_selecting {
            ui.colored_label(theme::ACCENT, "Selecting panel source in game...");
        }
        if panel.floating_panel.is_enabled {
            ui.colored_label(theme::LIME_GREEN, "Floating panel enabled");
        }
    });
}

fn position_size_section(
    ui: &mut Ui,
    core: &Arc<Core>,
    panel: &PanelConfig,
    _is_popped_out: bool,
    is_locked: bool,
    is_editing: bool,
    is_selecting: bool,
) {
    ui.add_enabled_ui(!is_locked && !is_editing && !is_selecting, |ui| {
        ui.horizontal(|ui| {
            ui.label("Left:");
            let mut left = panel.left;
            if ui.add(egui::DragValue::new(&mut left).speed(1.0)).changed() {
                core.write(|s| {
                    if let Some(p) = s.active_profile_mut() {
                        if let Some(panel) = p.panel_configs.iter_mut().find(|p| p.id == panel.id) {
                            panel.left = left;
                        }
                    }
                });
            }
        });

        ui.horizontal(|ui| {
            ui.label("Top:");
            let mut top = panel.top;
            if ui.add(egui::DragValue::new(&mut top).speed(1.0)).changed() {
                core.write(|s| {
                    if let Some(p) = s.active_profile_mut() {
                        if let Some(panel) = p.panel_configs.iter_mut().find(|p| p.id == panel.id) {
                            panel.top = top;
                        }
                    }
                });
            }
        });

        ui.horizontal(|ui| {
            ui.label("Width:");
            let mut width = panel.width;
            if ui
                .add(egui::DragValue::new(&mut width).speed(1.0))
                .changed()
            {
                core.write(|s| {
                    if let Some(p) = s.active_profile_mut() {
                        if let Some(panel) = p.panel_configs.iter_mut().find(|p| p.id == panel.id) {
                            panel.width = width;
                        }
                    }
                });
            }
        });

        ui.horizontal(|ui| {
            ui.label("Height:");
            let mut height = panel.height;
            if ui
                .add(egui::DragValue::new(&mut height).speed(1.0))
                .changed()
            {
                core.write(|s| {
                    if let Some(p) = s.active_profile_mut() {
                        if let Some(panel) = p.panel_configs.iter_mut().find(|p| p.id == panel.id) {
                            panel.height = height;
                        }
                    }
                });
            }
        });
    });
}

fn display_options_section(
    ui: &mut Ui,
    core: &Arc<Core>,
    panel: &PanelConfig,
    _is_popped_out: bool,
    is_locked: bool,
    is_editing: bool,
    is_selecting: bool,
) {
    ui.add_enabled_ui(!is_locked && !is_editing && !is_selecting, |ui| {
        let mut always_on_top = panel.always_on_top;
        if toggle_labeled(ui, &mut always_on_top, "Always on top").changed() {
            core.write(|s| {
                if let Some(p) = s.active_profile_mut() {
                    if let Some(panel) = p.panel_configs.iter_mut().find(|p| p.id == panel.id) {
                        panel.always_on_top = always_on_top;
                    }
                }
            });
        }

        let mut hide_titlebar = panel.hide_titlebar;
        if toggle_labeled(ui, &mut hide_titlebar, "Hide title bar").changed() {
            core.write(|s| {
                if let Some(p) = s.active_profile_mut() {
                    if let Some(panel) = p.panel_configs.iter_mut().find(|p| p.id == panel.id) {
                        panel.hide_titlebar = hide_titlebar;
                    }
                }
            });
        }

        let mut full_screen = panel.full_screen;
        if toggle_labeled(ui, &mut full_screen, "Full screen").changed() {
            core.write(|s| {
                if let Some(p) = s.active_profile_mut() {
                    if let Some(panel) = p.panel_configs.iter_mut().find(|p| p.id == panel.id) {
                        panel.full_screen = full_screen;
                    }
                }
            });
        }

        let mut touch_enabled = panel.touch_enabled;
        if toggle_labeled(ui, &mut touch_enabled, "Touch enabled").changed() {
            core.write(|s| {
                if let Some(p) = s.active_profile_mut() {
                    if let Some(panel) = p.panel_configs.iter_mut().find(|p| p.id == panel.id) {
                        panel.touch_enabled = touch_enabled;
                    }
                }
            });
        }

        let mut auto_game_refocus = panel.auto_game_refocus;
        if toggle_labeled(ui, &mut auto_game_refocus, "Auto game refocus").changed() {
            core.write(|s| {
                if let Some(p) = s.active_profile_mut() {
                    if let Some(panel) = p.panel_configs.iter_mut().find(|p| p.id == panel.id) {
                        panel.auto_game_refocus = auto_game_refocus;
                    }
                }
            });
        }
    });
}
