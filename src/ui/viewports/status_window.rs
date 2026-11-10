//! The status window viewport: shows recent status messages.

use crate::core::Core;
use eframe::egui::{self, Color32, RichText, ScrollArea, TextStyle};
use std::sync::Arc;

pub const STATUS_TITLE: &str = "POPM Status";

pub fn show(ctx: &egui::Context, core: Arc<Core>) {
    let _core_clone = core.clone();
    let (version, _messages) = core.read(|s| ("1.0.0".to_string(), s.ui.message.clone()));

    let viewport_id = egui::ViewportId::from_hash_of(STATUS_TITLE);
    let builder = egui::ViewportBuilder::default()
        .with_title(STATUS_TITLE)
        .with_inner_size([400.0, 225.0])
        .with_min_inner_size([300.0, 150.0])
        .with_resizable(true)
        .with_decorations(true)
        .with_always_on_top()
        .with_active(true)
        .with_taskbar(false);

    ctx.show_viewport_deferred(viewport_id, builder, move |ctx, class| {
        if class != egui::ViewportClass::Deferred {
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Set dark theme for status window
            ui.visuals_mut().dark_mode = true;
            ui.visuals_mut().panel_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 220);

            ui.horizontal(|ui| {
                ui.label(RichText::new("Status Messages").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Clear").clicked() {
                        // Status message clearing not implemented
                    }
                });
            });

            ui.separator();

            // Status messages area
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Status messages will appear here")
                            .text_style(TextStyle::Monospace),
                    );
                });

            ui.separator();

            // Version info
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                ui.label(RichText::new(format!("Version: {}", version)).weak());
            });
        });
    });
}
