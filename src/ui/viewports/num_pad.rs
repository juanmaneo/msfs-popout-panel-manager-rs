//! The virtual NumPad viewport (port of `NumPad.xaml`): on-screen numeric keypad
//! laid out like the upstream grid where "+" and "Ent" span two rows and "0" spans
//! two columns.

use crate::core::Core;
use eframe::egui::{self, Button, Color32, Layout, RichText, Ui};
use std::sync::Arc;

pub const NUM_PAD_TITLE: &str = "Virtual NumPad - POPM";

pub fn show(ctx: &egui::Context, core: Arc<Core>) {
    let viewport_id = egui::ViewportId::from_hash_of(NUM_PAD_TITLE);
    let builder = egui::ViewportBuilder::default()
        .with_title(NUM_PAD_TITLE)
        .with_inner_size([280.0, 340.0])
        .with_min_inner_size([280.0, 340.0])
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
            ui.visuals_mut().dark_mode = true;
            ui.visuals_mut().panel_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 220);

            // Header row: title left, close right (upstream row 0 spans the grid width).
            ui.horizontal(|ui| {
                ui.label(RichText::new("Virtual NumPad").strong());
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(Button::new(RichText::new("×").size(14.0)).frame(false))
                        .on_hover_text("Close the virtual NumPad")
                        .clicked()
                    {
                        core.close_num_pad();
                    }
                });
            });
            ui.add_space(4.0);

            // 4-column grid; rows 2-3 and 4-5 share the tall "+" / "Ent" buttons.
            // Row heights are driven by the tall button allocated across two rows.
            let cell = egui::vec2(58.0, 46.0);
            let tall = egui::vec2(58.0, 46.0 * 2.0 + 6.0);

            egui::Grid::new("numpad_grid")
                .num_columns(4)
                .spacing([6.0, 6.0])
                .show(ui, |ui| {
                    // Row 1: Tab / * -
                    for (label, name) in [
                        ("Tab", "Tab"),
                        ("/", "Divide"),
                        ("*", "Multiply"),
                        ("-", "Subtract"),
                    ] {
                        if key(ui, label, cell).clicked() {
                            core.num_pad_click(name);
                        }
                    }
                    ui.end_row();

                    // Row 2: 7 8 9 + (tall)
                    for digit in ["7", "8", "9"] {
                        if key(ui, digit, cell).clicked() {
                            core.num_pad_click(digit);
                        }
                    }
                    if key(ui, "+", tall).clicked() {
                        core.num_pad_click("Add");
                    }
                    ui.end_row();

                    // Row 3: 4 5 6 (+ continues)
                    for digit in ["4", "5", "6"] {
                        if key(ui, digit, cell).clicked() {
                            core.num_pad_click(digit);
                        }
                    }
                    ui.label(""); // cell occupied by the tall "+" button
                    ui.end_row();

                    // Row 4: 1 2 3 Ent (tall)
                    for digit in ["1", "2", "3"] {
                        if key(ui, digit, cell).clicked() {
                            core.num_pad_click(digit);
                        }
                    }
                    if key(ui, "Ent", tall).clicked() {
                        core.num_pad_click("Enter");
                    }
                    ui.end_row();

                    // Row 5: 0 (wide, 2 cols) . (Ent continues)
                    if ui
                        .add_sized(
                            egui::vec2(cell.x * 2.0 + 6.0, cell.y),
                            Button::new(RichText::new("0").size(16.0)),
                        )
                        .clicked()
                    {
                        core.num_pad_click("0");
                    }
                    if key(ui, ".", cell).clicked() {
                        core.num_pad_click("Decimal");
                    }
                    ui.label(""); // cell occupied by the tall "Ent" button
                    ui.end_row();
                });
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    });
}

fn key(ui: &mut Ui, label: &str, size: egui::Vec2) -> egui::Response {
    ui.add_sized(size, Button::new(RichText::new(label).size(16.0)))
}
