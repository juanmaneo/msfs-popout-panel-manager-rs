//! The virtual NumPad viewport (port of `NumPad.xaml`): on-screen numeric keypad
//! laid out like the upstream grid where "+" and "Ent" span two rows and "0" spans
//! two columns.

use crate::core::Core;
use eframe::egui::{self, Button, Color32, Layout, RichText, Ui};
use std::sync::Arc;

pub const NUM_PAD_TITLE: &str = "Virtual NumPad - POPM";

/// One key: label plus the virtual key name sent to the core.
const KEYS: &[(&str, &str)] = &[
    ("Tab", "Tab"),
    ("/", "/"),
    ("*", "*"),
    ("-", "-"),
    ("7", "7"),
    ("8", "8"),
    ("9", "9"),
    ("+", "+"),
    ("4", "4"),
    ("5", "5"),
    ("6", "6"),
    ("1", "1"),
    ("2", "2"),
    ("3", "3"),
    ("Ent", "Enter"),
    ("0", "0"),
    (".", "."),
];

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
                    for (label, name) in &KEYS[0..4] {
                        if key(ui, &core, label, name, cell).clicked() {
                            core.num_pad_click(name);
                        }
                    }
                    ui.end_row();

                    // Row 2: 7 8 9 + (tall)
                    for (label, name) in &KEYS[4..7] {
                        if key(ui, &core, label, name, cell).clicked() {
                            core.num_pad_click(name);
                        }
                    }
                    if key(ui, &core, "+", "+", tall).clicked() {
                        core.num_pad_click("+");
                    }
                    ui.end_row();

                    // Row 3: 4 5 6 (+ continues)
                    for (label, name) in &KEYS[8..11] {
                        if key(ui, &core, label, name, cell).clicked() {
                            core.num_pad_click(name);
                        }
                    }
                    ui.label(""); // cell occupied by the tall "+" button
                    ui.end_row();

                    // Row 4: 1 2 3 Ent (tall)
                    for (label, name) in &KEYS[11..14] {
                        if key(ui, &core, label, name, cell).clicked() {
                            core.num_pad_click(name);
                        }
                    }
                    if key(ui, &core, "Ent", "Enter", tall).clicked() {
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
                    if key(ui, &core, ".", ".", cell).clicked() {
                        core.num_pad_click(".");
                    }
                    ui.label(""); // cell occupied by the tall "Ent" button
                    ui.end_row();
                });
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    });
}

fn key(
    ui: &mut Ui,
    _core: &Arc<Core>,
    label: &str,
    _name: &str,
    size: egui::Vec2,
) -> egui::Response {
    ui.add_sized(size, Button::new(RichText::new(label).size(16.0)))
}
