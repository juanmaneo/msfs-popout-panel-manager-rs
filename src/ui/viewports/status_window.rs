//! The status window viewport (port of the upstream pop-out progress message window):
//! shows the `StatusLog` messages with per-level colors plus a Clear button.

use crate::core::Core;
use crate::status::{StatusMessage, StatusMessageType};
use eframe::egui::{self, Color32, RichText, ScrollArea, TextStyle};
use std::sync::Arc;

pub const STATUS_TITLE: &str = "POPM Status";

fn color_of(kind: StatusMessageType) -> Color32 {
    match kind {
        StatusMessageType::Info => Color32::WHITE,
        StatusMessageType::Success => Color32::from_rgb(0, 255, 0),
        StatusMessageType::Failure => Color32::RED,
        StatusMessageType::Executing => Color32::from_rgb(255, 222, 173),
    }
}

pub fn show(ctx: &egui::Context, core: Arc<Core>) {
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

            let (_version, messages) = core.status.snapshot();
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if messages.is_empty() {
                        ui.label(RichText::new("No status messages yet").weak());
                    }
                    for msg in render_lines(&messages) {
                        ui.label(
                            RichText::new(msg.text)
                                .text_style(TextStyle::Monospace)
                                .color(color_of(msg.kind)),
                        );
                    }
                });

            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                ui.label(RichText::new(format!("Version: {}", env!("CARGO_PKG_VERSION"))).weak());
            });
        });

        if close_requested.get() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    });
}

/// Flatten the message log into display lines (`new_line` starts a new egui label).
fn render_lines(messages: &[StatusMessage]) -> Vec<StatusMessage> {
    messages.to_vec()
}
