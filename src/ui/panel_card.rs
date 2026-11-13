//! One card per pop out panel (port of `PopOutPanelConfigCard.xaml`): name, position/size,
//! display toggles, floating panel section and reorder/delete controls. While the panel sources
//! are edited the list shows source cards instead (see `source_card`).

use super::dialogs::Dialogs;
use super::edit;
use super::snapshot::Snapshot;
use super::source_card;
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
                if profile.is_editing_panel_source {
                    source_card::show(ui, core, snap, profile, panel);
                } else {
                    let mut ctx = CardCtx {
                        core,
                        snap,
                        profile,
                        index,
                        count,
                        dialogs,
                    };
                    show_panel_card(ui, &mut ctx, panel);
                }
                ui.add_space(6.0);
            }
        });
}

/// Apply one changed persisted panel property to the core and notify the live window.
pub(super) fn set_panel_field(
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
    let editable = !profile.is_locked;
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
                    p.panel_name = name
                });
            }

            ui.colored_label(theme::ACCENT, format!("[{:?}]", panel.panel_type));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if panel.is_deletable() && icon_button(ui, Icon::Close, "Delete panel").clicked() {
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
                display_options_section(ui, core, profile_id, panel, editable);
            });
        });

        floating_section(ui, core, profile_id, panel, editable);

        if panel.has_panel_source() {
            ui.label(
                RichText::new(format!("Source: {}", panel.source_coordinate_text()))
                    .small()
                    .weak(),
            );
        }
        if panel.is_pop_out_success() == Some(false) {
            ui.colored_label(theme::INDIAN_RED, pop_out_failure_text(panel));
        }
        if panel.floating_panel.is_enabled {
            ui.colored_label(theme::LIME_GREEN, "Floating panel enabled");
        }
    });
}

/// Upstream shows the position fields only once the panel has a source (built-in and HUD / NumPad
/// windows always have one).
fn has_position_fields(panel: &PanelConfig) -> bool {
    !panel.is_custom_pop_out() || panel.has_panel_source()
}

/// Upstream error popup text, which depends on the panel type.
fn pop_out_failure_text(panel: &PanelConfig) -> &'static str {
    if panel.is_custom_pop_out() {
        "Unable to pop out this panel. Please check the source panel circle defined for this panel is at the correct location and not blocked by other window. Also please check if this panel is a duplicate with another panel. Lastly, please close all instrumentation pop outs that were opened manually."
    } else {
        "Unable to configure this built-in panel. Please make sure this panel has been opened and popped out by the game."
    }
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
        if !has_position_fields(panel) {
            ui.colored_label(theme::NAVAJO_WHITE, "Please identify panel source");
            return;
        }

        let not_full_screen = !panel.full_screen;
        let row = |ui: &mut Ui, label: &str, value: i32, prop: PanelProp, set: fn(&mut PanelConfig, i32), enabled: bool| {
            ui.horizontal(|ui| {
                ui.label(label);
                let mut v = value;
                let response = ui.add_enabled(
                    enabled && not_full_screen,
                    egui::DragValue::new(&mut v).speed(1.0),
                );
                if response.changed() {
                    set_panel_field(core, profile_id, id, prop, |p| set(p, v));
                }
            });
        };
        let is_refocus = panel.panel_type == PanelType::RefocusDisplay;
        let is_hud = panel.panel_type == PanelType::HudBarWindow;
        row(ui, "Left:", panel.left, PanelProp::Left, |p, v| p.left = v, !is_refocus);
        row(ui, "Top:", panel.top, PanelProp::Top, |p, v| p.top = v, !is_refocus);
        row(ui, "Width:", panel.width, PanelProp::Width, |p, v| p.width = v, !is_hud && !is_refocus);
        row(ui, "Height:", panel.height, PanelProp::Height, |p, v| p.height = v, !is_hud && !is_refocus);

        if !is_refocus {
            let mut moving = panel.is_editing_panel;
            let can_move_resize = can_move_resize(panel, snap, is_popped_out);
            let r = ui.add_enabled_ui(can_move_resize || moving, |ui| {
                toggle_labeled(ui, &mut moving, "Move/Resize")
                    .tip("Enable keyboard move/resize: arrow keys move the panel (Shift = 1px), Ctrl+arrows resize")
            });
            if r.inner.changed() {
                core.set_move_resize(id, moving);
            }
        }
    });
}

/// Upstream `MoveAndResizePanelCommand` gating: only one panel can be moved at a time, and only
/// while the pop out window exists and is not in full screen.
fn can_move_resize(panel: &PanelConfig, snap: &Snapshot, is_popped_out: bool) -> bool {
    let another_panel_moving = snap.active.as_ref().is_some_and(|p| {
        !p.current_move_resize_panel_id.is_nil() && p.current_move_resize_panel_id != panel.id
    });
    snap.sim.in_cockpit
        && panel.is_pop_out_success().is_some()
        && panel.panel_handle.hwnd().is_some()
        && !panel.full_screen
        && is_popped_out
        && !another_panel_moving
}

fn display_options_section(
    ui: &mut Ui,
    core: &Arc<Core>,
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
        ui.add_enabled_ui(not_full_screen, |ui| {
            toggle(
                ui,
                "Hide title bar",
                panel.hide_titlebar,
                PanelProp::HideTitlebar,
                |p, v| p.hide_titlebar = v,
            );
        });
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
