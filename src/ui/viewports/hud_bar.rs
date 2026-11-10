//! The HUD bar viewport: timer, sim rate and aircraft data.

use crate::core::Core;
use eframe::egui::{self, Align, Button, Color32, Layout, RichText};
use std::sync::Arc;
use std::time::Duration;

pub const HUD_BAR_TITLE: &str = "HudBar - POPM";

pub fn show(ctx: &egui::Context, core: Arc<Core>) {
    let core_clone = core.clone();
    let _snap = core.read(|s| s.sim.hud_bar_data.clone());

    let viewport_id = egui::ViewportId::from_hash_of(HUD_BAR_TITLE);
    let builder = egui::ViewportBuilder::default()
        .with_title(HUD_BAR_TITLE)
        .with_inner_size([1920.0, 40.0])
        .with_resizable(false)
        .with_decorations(false)
        .with_always_on_top()
        .with_active(false)
        .with_mouse_passthrough(true)
        .with_taskbar(false);

    ctx.show_viewport_deferred(viewport_id, builder, move |ctx, class| {
        if class != egui::ViewportClass::Deferred {
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Set dark theme for HUD bar
            ui.visuals_mut().dark_mode = true;
            ui.visuals_mut().panel_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 200);

            ui.horizontal(|ui| {
                // Timer section
                ui.label(RichText::new("Timer: 00:00:00").size(14.0).monospace());

                // Sim rate section
                ui.add_space(20.0);
                ui.label(RichText::new("Sim Rate: 1.0x").size(14.0).monospace());

                // Aircraft data section
                ui.add_space(20.0);
                ui.label(RichText::new("Aircraft Data").size(14.0).monospace());

                // Spacer to push buttons to the right
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(20.0);

                    // Close button
                    if ui
                        .add(Button::new(RichText::new("×").size(16.0)).frame(false))
                        .clicked()
                    {
                        core_clone.close_hud_bar();
                    }

                    // Sim rate buttons
                    if ui
                        .add(Button::new(RichText::new("-").size(14.0)).frame(false))
                        .clicked()
                    {
                        core_clone.decrease_sim_rate();
                    }
                    if ui
                        .add(Button::new(RichText::new("+").size(14.0)).frame(false))
                        .clicked()
                    {
                        core_clone.increase_sim_rate();
                    }
                });
            });
        });

        // Request repaint to keep the HUD bar updated
        ctx.request_repaint_after(Duration::from_millis(250));
    });
}
