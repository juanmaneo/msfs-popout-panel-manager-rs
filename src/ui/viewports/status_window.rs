//! The status window viewports (port of the upstream pop-out progress message window):
//! * `View::Message` is upstream's `MessageWindow`: borderless, click-through, always on top,
//!   centred on the simulator window and sized for the kind of message.
//! * `View::Manual` is the interactive window opened from the header "Status" button, with
//!   a Clear and a Close button.

use crate::core::{Core, MessageKind};
use crate::status::StatusMessageType;
use eframe::egui::{self, Color32, RichText, ScrollArea, TextStyle};
use std::sync::Arc;

pub const STATUS_TITLE: &str = "POPM Status";
pub const MESSAGE_TITLE: &str = "POPM Status Message";

const HEADER_HEIGHT: f32 = 30.0;
/// Upstream `Colors.SlateGray`.
const HEADER_FILL: Color32 = Color32::from_rgb(112, 128, 144);
const BODY_FILL: Color32 = Color32::from_rgb(33, 33, 33);

/// Which status window to show.
#[derive(Clone, Copy)]
pub enum View {
    Manual,
    Message {
        kind: MessageKind,
        /// Top-left in egui points; `None` lets the OS place the window.
        position: Option<egui::Pos2>,
    },
}

/// Inner size (points) of the message window: upstream `WINDOW_WIDTH/HEIGHT_*_MESSAGE`.
pub fn message_size(kind: MessageKind) -> egui::Vec2 {
    match kind {
        MessageKind::PopOut => egui::vec2(400.0, 225.0),
        MessageKind::Regular => egui::vec2(300.0, 75.0),
    }
}

fn color_of(kind: StatusMessageType) -> Color32 {
    match kind {
        StatusMessageType::Info => Color32::WHITE,
        // Upstream LimeGreen / IndianRed / NavajoWhite.
        StatusMessageType::Success => Color32::from_rgb(50, 205, 50),
        StatusMessageType::Failure => Color32::from_rgb(205, 92, 92),
        StatusMessageType::Executing => Color32::from_rgb(255, 222, 173),
    }
}

/// The message list in a scroll area that keeps the newest line in view.
fn messages_ui(ui: &mut egui::Ui, core: &Core, manual: bool) {
    let (_version, messages) = core.status.snapshot();
    let mut area = ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true);
    if !manual {
        area = area.scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    }
    area.show(ui, |ui| {
        if messages.is_empty() && manual {
            ui.label(RichText::new("No status messages yet").weak());
        }
        for msg in messages {
            ui.label(
                RichText::new(msg.text)
                    .text_style(TextStyle::Monospace)
                    .color(color_of(msg.kind)),
            );
        }
    });
}

pub fn show(ctx: &egui::Context, core: Arc<Core>, view: View) {
    match view {
        View::Manual => show_manual(ctx, core),
        View::Message { kind, position } => show_message(ctx, core, kind, position),
    }
}

fn show_message(
    ctx: &egui::Context,
    core: Arc<Core>,
    kind: MessageKind,
    position: Option<egui::Pos2>,
) {
    let viewport_id = egui::ViewportId::from_hash_of(MESSAGE_TITLE);
    let mut builder = egui::ViewportBuilder::default()
        .with_title(MESSAGE_TITLE)
        .with_inner_size(message_size(kind))
        .with_resizable(false)
        .with_decorations(false)
        .with_always_on_top()
        .with_mouse_passthrough(true)
        .with_active(false)
        .with_taskbar(false);
    if let Some(p) = position {
        builder = builder.with_position(p);
    }

    ctx.show_viewport_deferred(viewport_id, builder, move |ctx, class| {
        if class != egui::ViewportClass::Deferred {
            return;
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BODY_FILL))
            .show(ctx, |ui| {
                ui.visuals_mut().dark_mode = true;
                egui::Frame::NONE.fill(HEADER_FILL).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(HEADER_HEIGHT);
                    ui.horizontal_centered(|ui| {
                        ui.add_space(10.0);
                        ui.label(RichText::new("MSFS POP OUT PANEL MANAGER").color(Color32::WHITE));
                    });
                });
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(20, 5))
                    .show(ui, |ui| messages_ui(ui, &core, false));
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    });
}

fn show_manual(ctx: &egui::Context, core: Arc<Core>) {
    let viewport_id = egui::ViewportId::from_hash_of(STATUS_TITLE);
    let builder = egui::ViewportBuilder::default()
        .with_title(STATUS_TITLE)
        .with_inner_size([520.0, 300.0])
        .with_min_inner_size([360.0, 180.0])
        .with_resizable(true)
        .with_decorations(true)
        .with_always_on_top()
        .with_active(true)
        .with_taskbar(false);

    ctx.show_viewport_deferred(viewport_id, builder, move |ctx, class| {
        if class != egui::ViewportClass::Deferred {
            return;
        }
        let close_requested = std::cell::Cell::new(false);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.visuals_mut().dark_mode = true;
            ui.visuals_mut().panel_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 220);

            ui.horizontal(|ui| {
                ui.label(RichText::new("Status Messages").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Clear").clicked() {
                        core.status.clear();
                    }
                    if ui.button("Close").clicked() {
                        close_requested.set(true);
                    }
                });
            });
            ui.separator();

            messages_ui(ui, &core, true);

            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                ui.label(RichText::new(format!("Version: {}", env!("CARGO_PKG_VERSION"))).weak());
            });
        });

        if close_requested.get() || ctx.input(|i| i.viewport().close_requested()) {
            core.close_status_window();
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    });
}
