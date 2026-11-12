//! One card per pop out panel (port of `PopOutPanelConfigCard.xaml`): name, position/size,
//! display toggles, floating panel section and reorder/delete controls.

use super::dialogs::Dialogs;
use super::edit;
use super::snapshot::Snapshot;
use super::theme;
use super::widgets::{Icon, ResponseExt, heading, icon_button, toggle_labeled};
use crate::core::Core;
use crate::core::config::PanelProp;
use crate::keybinding::display_binding;
use crate::model::{PanelConfig, PanelType, UserProfile};
use eframe::egui::{self, Align, Layout, RichText, ScrollArea, TextEdit, TextStyle, Ui};
use std::sync::Arc;
use uuid::Uuid;

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
            let count = profile.panel_configs.len();
            for (index, panel) in profile.panel_configs.iter().enumerate() {
                let mut ctx = CardCtx {
                    core,
                    snap,
                    profile,
                    index,
                    count,
                    dialogs,
                };
                show_panel_card(ui, &mut ctx, panel);
                ui.add_space(6.0);
            }
        });
}

/// Apply one changed persisted panel property to the core and notify the live window.
fn set_panel_field(
    core: &Arc<Core>,
    profile_id: Uuid,
    panel_id: Uuid,
    prop: PanelProp,
    change: impl FnOnce(&mut PanelConfig),
) {
    edit::panel(core, profile_id, panel_id, change);
    core.panel_property_updated(panel_id, prop);
}

/// Everything a panel card needs besides the panel itself.
struct CardCtx<'a> {
    core: &'a Arc<Core>,
    snap: &'a Snapshot,
    profile: &'a UserProfile,
    index: usize,
    count: usize,
    dialogs: &'a mut Dialogs,
}

fn show_panel_card(ui: &mut Ui, ctx: &mut CardCtx<'_>, panel: &PanelConfig) {
    let core = ctx.core;
    let profile = ctx.profile;
    let snap = ctx.snap;
    let dialogs = &mut *ctx.dialogs;
    let index = ctx.index;
    let count = ctx.count;
    let is_popped_out = profile.is_popped_out;
    let is_locked = profile.is_locked;
    let is_editing = profile.is_editing_panel_source;
    let is_selecting = snap.selecting_panel == Some(panel.id);
    let editable = !is_locked && !is_editing && !is_selecting;
    let id = panel.id;
    let profile_id = profile.id;

    // Upstream colors the card border red/green based on the pop out result.
    let border = match panel.is_pop_out_success() {
        Some(false) => theme::ERROR,
        Some(true) => theme::SUCCESS,
        None => egui::Color32::from_gray(70),
    };
    theme::card_frame(theme::CARD_FILL, border).show(ui, |ui| {
        ui.set_width(ui.available_width());

        // Header row: name, type, reorder + delete buttons
        ui.horizontal(|ui| {
            let mut name = panel.panel_name.clone();
            let response = ui.add_enabled(
                editable && panel.is_custom_pop_out(),
                TextEdit::singleline(&mut name)
                    .font(TextStyle::Button)
                    .desired_width(220.0)
                    .hint_text("Panel name"),
            );
            if response.changed() {
                set_panel_field(core, profile_id, id, PanelProp::PanelName, |p| {
                    p.panel_name = name.clone()
                });
            }

            ui.colored_label(theme::ACCENT, format!("[{:?}]", panel.panel_type));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, Icon::Close, "Delete panel").clicked() {
                    dialogs.open_delete_panel(id);
                }
                let down = icon_button(ui, Icon::Down, "Move panel down in the list");
                if index + 1 < count {
                    if down.clicked() {
                        edit::profile(core, profile_id, |p| p.move_panel_down(id));
                    }
                } else {
                    ui.add_enabled(false, egui::Button::new(""));
                }
                let up = icon_button(ui, Icon::Up, "Move panel up in the list");
                if index > 0 {
                    if up.clicked() {
                        edit::profile(core, profile_id, |p| p.move_panel_up(id));
                    }
                } else {
                    ui.add_enabled(false, egui::Button::new(""));
                }
            });
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                position_size_section(ui, core, snap, profile_id, panel, editable, is_popped_out);
            });

            ui.separator();

            ui.vertical(|ui| {
                display_options_section(ui, core, snap, profile_id, panel, editable);
            });
        });

        floating_section(ui, core, profile_id, panel, editable);

        if is_selecting {
            ui.horizontal(|ui| {
                ui.colored_label(
                    theme::ACCENT,
                    "Click the panel source location in the game...",
                );
                if ui.button("Cancel").clicked() {
                    core.cancel_panel_selection();
                }
            });
        } else if panel.is_custom_pop_out() && !panel.has_panel_source() && editable {
            ui.horizontal(|ui| {
                ui.colored_label(theme::NAVAJO_WHITE, "Panel source not identified.");
                if ui
                    .add_enabled(snap.sim.in_cockpit, egui::Button::new("Identify source"))
                    .tip("Click the panel location in the cockpit to set the panel source")
                    .clicked()
                {
                    core.start_panel_selection(id, profile.is_used_legacy_camera_system);
                }
            });
        }
        if panel.has_panel_source() {
            ui.label(
                RichText::new(format!("Source: {}", panel.source_coordinate_text()))
                    .small()
                    .weak(),
            );
        }
        if panel.is_pop_out_success() == Some(false) {
            ui.colored_label(
                theme::INDIAN_RED,
                "Pop out failed for this panel. Check the panel source location.",
            );
        }
        if !panel.has_panel_source() && panel.is_custom_pop_out() {
            ui.colored_label(theme::NAVAJO_WHITE, "Please identify panel source");
        }
        if panel.floating_panel.is_enabled {
            ui.colored_label(theme::LIME_GREEN, "Floating panel enabled");
        }
    });
}

fn position_size_section(
    ui: &mut Ui,
    core: &Arc<Core>,
    snap: &Snapshot,
    profile_id: Uuid,
    panel: &PanelConfig,
    editable: bool,
    is_popped_out: bool,
) {
    let id = panel.id;
    ui.add_enabled_ui(editable, |ui| {
        let row = |ui: &mut Ui, label: &str, value: i32, prop: PanelProp, set: fn(&mut PanelConfig, i32)| {
            ui.horizontal(|ui| {
                ui.label(label);
                let mut v = value;
                if ui.add(egui::DragValue::new(&mut v).speed(1.0)).changed() {
                    set_panel_field(core, profile_id, id, prop, |p| set(p, v));
                }
            });
        };
        row(ui, "Left:", panel.left, PanelProp::Left, |p, v| p.left = v);
        row(ui, "Top:", panel.top, PanelProp::Top, |p, v| p.top = v);
        row(ui, "Width:", panel.width, PanelProp::Width, |p, v| p.width = v);
        row(ui, "Height:", panel.height, PanelProp::Height, |p, v| p.height = v);

        // Move/resize mode + camera preview
        ui.horizontal(|ui| {
            let mut moving = panel.is_editing_panel;
            let r = toggle_labeled(ui, &mut moving, "Move/Resize")
                .tip("Enable keyboard move/resize: arrow keys move the panel (Shift = 1px), Ctrl+arrows resize");
            if r.changed() {
                core.set_move_resize(id, moving);
            }
            let cam = ui
                .add_enabled(
                    is_popped_out && panel.panel_type != PanelType::RefocusDisplay,
                    egui::Button::new("Camera"),
                )
                .tip("Show the camera view of this panel in the simulator");
            if cam.clicked() {
                core.set_camera_for_panel(id);
            }
        });

        camera_picker(ui, core, snap, profile_id, panel, editable);
    });
}

/// Fixed camera chooser shown while a custom panel source is being identified
/// (upstream `PanelTargetField.xaml` combo + prev/next buttons).
fn camera_picker(
    ui: &mut Ui,
    core: &Arc<Core>,
    snap: &Snapshot,
    profile_id: Uuid,
    panel: &PanelConfig,
    editable: bool,
) {
    if !panel.is_custom_pop_out() {
        return;
    }
    let id = panel.id;
    ui.add_enabled_ui(editable, |ui| {
        ui.label("Camera:");
        let configs = crate::core::fixed_camera_configs(snap.sim.instrument_count);
        let selected = configs
            .iter()
            .find(|c| {
                c.camera_type == panel.fixed_camera_config.camera_type
                    && c.camera_index == panel.fixed_camera_config.camera_index
            })
            .cloned()
            .unwrap_or_else(|| panel.fixed_camera_config.clone());
        let mut chosen = selected.clone();
        let combo = egui::ComboBox::from_id_salt(format!("camera_{id}"))
            .selected_text(&selected.name)
            .width(160.0);
        combo.show_ui(ui, |ui| {
            for cfg in &configs {
                ui.selectable_value(&mut chosen, cfg.clone(), &cfg.name);
            }
        });
        if chosen.camera_type != selected.camera_type
            || chosen.camera_index != selected.camera_index
        {
            edit::panel(core, profile_id, id, |p| {
                p.fixed_camera_config = chosen.clone()
            });
            // preview the new camera in the simulator
            core.set_camera_for_panel(id);
        }
        ui.label(
            RichText::new("Camera used to view the panel source during pop out")
                .small()
                .weak(),
        );
    });
}

fn display_options_section(
    ui: &mut Ui,
    core: &Arc<Core>,
    snap: &Snapshot,
    profile_id: Uuid,
    panel: &PanelConfig,
    editable: bool,
) {
    let id = panel.id;
    ui.add_enabled_ui(editable, |ui| {
        let toggle = |ui: &mut Ui,
                      label: &str,
                      value: bool,
                      prop: PanelProp,
                      set: fn(&mut PanelConfig, bool)| {
            let mut v = value;
            if toggle_labeled(ui, &mut v, label).changed() {
                set_panel_field(core, profile_id, id, prop, |p| set(p, v));
            }
        };
        let not_full_screen = !panel.full_screen;
        ui.add_enabled_ui(not_full_screen, |ui| {
            toggle(
                ui,
                "Always on top",
                panel.always_on_top,
                PanelProp::AlwaysOnTop,
                |p, v| p.always_on_top = v,
            );
        });
        toggle(
            ui,
            "Hide title bar",
            panel.hide_titlebar,
            PanelProp::HideTitlebar,
            |p, v| p.hide_titlebar = v,
        );
        toggle(
            ui,
            "Full screen",
            panel.full_screen,
            PanelProp::FullScreen,
            |p, v| p.set_full_screen(v),
        );
        if panel.is_touch_enable_panel() {
            toggle(
                ui,
                "Touch enabled",
                panel.touch_enabled,
                PanelProp::TouchEnabled,
                |p, v| p.set_touch_enabled(v),
            );
        }
        toggle(
            ui,
            "Auto game refocus",
            panel.auto_game_refocus,
            PanelProp::AutoGameRefocus,
            |p, v| p.auto_game_refocus = v,
        );

        let _ = snap;
    });
}

/// Floating panel section (port of the `TglBtnAllowFloatPanel` block in the panel card).
fn floating_section(
    ui: &mut Ui,
    core: &Arc<Core>,
    profile_id: Uuid,
    panel: &PanelConfig,
    editable: bool,
) {
    let id = panel.id;
    ui.add_space(4.0);
    ui.add_enabled_ui(editable, |ui| {
        ui.horizontal(|ui| {
            let mut enabled = panel.floating_panel.is_enabled;
            if toggle_labeled(ui, &mut enabled, "Floating Panel Toggle")
                .tip("Set the pop out panel to float above other windows")
                .changed()
            {
                edit::panel(core, profile_id, id, |p| {
                    p.floating_panel.set_enabled(enabled)
                });
            }

            if panel.floating_panel.is_enabled {
                ui.label("Keyboard Binding:");
                if panel.floating_panel.is_detecting_keystroke {
                    ui.colored_label(theme::ACCENT, "Scanning...");
                } else {
                    let text = panel
                        .floating_panel
                        .keyboard_binding
                        .as_deref()
                        .map(display_binding)
                        .unwrap_or_else(|| "(none)".to_string());
                    ui.label(RichText::new(text).monospace());
                }
                if ui
                    .add_enabled(
                        !panel.floating_panel.is_detecting_keystroke,
                        egui::Button::new("Detect"),
                    )
                    .tip("Press the key combination that toggles this floating panel")
                    .clicked()
                {
                    core.start_detect_keystroke(id);
                }
                if panel.floating_panel.has_keyboard_binding() {
                    let mut hidden = panel.floating_panel.is_hidden_on_start;
                    if toggle_labeled(ui, &mut hidden, "Hide on Start")
                        .tip("Hide the panel when pop out initially")
                        .changed()
                    {
                        edit::panel(core, profile_id, id, |p| {
                            p.floating_panel.is_hidden_on_start = hidden
                        });
                    }
                }
            }
        });
    });
}
