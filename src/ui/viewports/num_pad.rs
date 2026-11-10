//! The virtual NumPad viewport: on-screen numeric keypad.

use crate::core::Core;
use eframe::egui::{self, Button, Color32, RichText};
use std::sync::Arc;

pub const NUM_PAD_TITLE: &str = "Virtual NumPad - POPM";

pub fn show(ctx: &egui::Context, core: Arc<Core>) {
    let viewport_id = egui::ViewportId::from_hash_of(NUM_PAD_TITLE);
    let builder = egui::ViewportBuilder::default()
        .with_title(NUM_PAD_TITLE)
        .with_inner_size([300.0, 400.0])
        .with_resizable(false)
        .with_decorations(true)
        .with_always_on_top()
        .with_active(true)
        .with_taskbar(false);

    let core = core.clone();
    ctx.show_viewport_deferred(viewport_id, builder, move |ctx, class| {
        if class != egui::ViewportClass::Deferred {
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Set dark theme for NumPad
            ui.visuals_mut().dark_mode = true;
            ui.visuals_mut().panel_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 220);

            // NumPad buttons in a 4-column grid
            let button_size = egui::vec2(60.0, 50.0);

            // First row
            ui.horizontal(|ui| {
                if ui
                    .add_sized(button_size, Button::new(RichText::new("Tab").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("Tab");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new("/").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("/");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new("*").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("*");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new("-").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("-");
                }
            });

            // Second row
            ui.horizontal(|ui| {
                if ui
                    .add_sized(button_size, Button::new(RichText::new("7").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("7");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new("8").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("8");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new("9").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("9");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new("+").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("+");
                }
            });

            // Third row
            ui.horizontal(|ui| {
                if ui
                    .add_sized(button_size, Button::new(RichText::new("4").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("4");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new("5").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("5");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new("6").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("6");
                }
            });

            // Fourth row
            ui.horizontal(|ui| {
                if ui
                    .add_sized(button_size, Button::new(RichText::new("1").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("1");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new("2").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("2");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new("3").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("3");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new("Enter").size(14.0)))
                    .clicked()
                {
                    core.num_pad_click("Enter");
                }
            });

            // Fifth row
            ui.horizontal(|ui| {
                if ui
                    .add_sized([130.0, 50.0], Button::new(RichText::new("0").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click("0");
                }
                if ui
                    .add_sized(button_size, Button::new(RichText::new(".").size(16.0)))
                    .clicked()
                {
                    core.num_pad_click(".");
                }
            });

            // Close button
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if ui.button("Close").clicked() {
                    core.close_num_pad();
                }
            });
        });
    });
}
