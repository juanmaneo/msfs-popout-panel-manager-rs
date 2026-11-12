//! The virtual NumPad viewport (port of `NumPad.xaml`): a borderless, draggable 4x5 keypad.
//! Row 0 holds the title and close button, "+" and "Ent" span two rows, "0" spans two columns.

use crate::core::Core;
use eframe::egui::{
    self, Align2, Button, Color32, FontId, Rect, RichText, Sense, Stroke, Ui, ViewportCommand,
    pos2, vec2,
};
use std::sync::Arc;

pub const NUM_PAD_TITLE: &str = "Virtual NumPad - POPM";

const HEADER_HEIGHT: f32 = 35.0;
const COLUMNS: f32 = 4.0;
const KEY_ROWS: f32 = 5.0;

/// One key: label, the virtual key name sent to the core and its grid cell.
struct Key {
    label: &'static str,
    name: &'static str,
    col: u8,
    row: u8,
    cols: u8,
    rows: u8,
}

impl Key {
    const fn new(
        label: &'static str,
        name: &'static str,
        col: u8,
        row: u8,
        cols: u8,
        rows: u8,
    ) -> Self {
        Self {
            label,
            name,
            col,
            row,
            cols,
            rows,
        }
    }
}

const KEYS: &[Key] = &[
    Key::new("Tab", "Tab", 0, 0, 1, 1),
    Key::new("/", "Divide", 1, 0, 1, 1),
    Key::new("*", "Multiply", 2, 0, 1, 1),
    Key::new("-", "Subtract", 3, 0, 1, 1),
    Key::new("7", "7", 0, 1, 1, 1),
    Key::new("8", "8", 1, 1, 1, 1),
    Key::new("9", "9", 2, 1, 1, 1),
    Key::new("+", "Add", 3, 1, 1, 2),
    Key::new("4", "4", 0, 2, 1, 1),
    Key::new("5", "5", 1, 2, 1, 1),
    Key::new("6", "6", 2, 2, 1, 1),
    Key::new("1", "1", 0, 3, 1, 1),
    Key::new("2", "2", 1, 3, 1, 1),
    Key::new("3", "3", 2, 3, 1, 1),
    Key::new("Ent", "Enter", 3, 3, 1, 2),
    Key::new("0", "0", 0, 4, 2, 1),
    Key::new(".", "Decimal", 2, 4, 1, 1),
];

pub fn show(ctx: &egui::Context, core: Arc<Core>) {
    let viewport_id = egui::ViewportId::from_hash_of(NUM_PAD_TITLE);
    let builder = egui::ViewportBuilder::default()
        .with_title(NUM_PAD_TITLE)
        .with_inner_size([300.0, 400.0])
        .with_min_inner_size([200.0, 260.0])
        .with_resizable(true)
        .with_decorations(false)
        .with_always_on_top()
        .with_active(true)
        .with_taskbar(false);

    let core = core.clone();
    ctx.show_viewport_deferred(viewport_id, builder, move |ctx, class| {
        if class != egui::ViewportClass::Deferred {
            return;
        }

        let start_drag = egui::CentralPanel::default()
            .show(ctx, |ui| {
                ui.visuals_mut().dark_mode = true;
                ui.visuals_mut().panel_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 220);
                let area = ui.max_rect();

                // Registered before the keys so that the keys take their own clicks; a drag
                // anywhere else moves the window (upstream `NumPad_MouseLeftButtonDown`).
                let drag = ui.interact(area, egui::Id::unique("numpad_drag"), Sense::drag());

                let header = Rect::from_min_size(area.min, vec2(area.width(), HEADER_HEIGHT));
                ui.painter().text(
                    header.min + vec2(10.0, HEADER_HEIGHT / 2.0),
                    Align2::LEFT_CENTER,
                    "Virtual NumPad",
                    FontId::proportional(14.0),
                    Color32::WHITE,
                );
                let close_rect = Rect::from_min_size(
                    pos2(header.max.x - 32.0, header.min.y + 4.0),
                    vec2(28.0, 28.0),
                );
                if ui
                    .put(
                        close_rect,
                        Button::new(RichText::new("×").size(14.0).color(Color32::WHITE))
                            .frame(false)
                            .min_size(close_rect.size()),
                    )
                    .on_hover_text("Close the virtual NumPad")
                    .clicked()
                {
                    core.close_num_pad();
                }

                let body = Rect::from_min_size(
                    pos2(area.min.x, area.min.y + HEADER_HEIGHT),
                    vec2(area.width(), (area.height() - HEADER_HEIGHT).max(0.0)),
                );
                for key in KEYS {
                    if key_button(ui, key_rect(body, key), key.label).clicked() {
                        core.num_pad_click(key.name);
                    }
                }

                drag.drag_started()
            })
            .inner;

        if start_drag {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    });
}

/// Screen rectangle of a key within the keypad body (the area below the header).
fn key_rect(body: Rect, key: &Key) -> Rect {
    let cell = vec2(body.width() / COLUMNS, body.height() / KEY_ROWS);
    Rect::from_min_size(
        body.min + vec2(cell.x * f32::from(key.col), cell.y * f32::from(key.row)),
        vec2(cell.x * f32::from(key.cols), cell.y * f32::from(key.rows)),
    )
    .shrink(1.0)
}

fn key_button(ui: &mut Ui, rect: Rect, label: &str) -> egui::Response {
    ui.put(
        rect,
        Button::new(
            RichText::new(label)
                .size(16.0)
                .strong()
                .color(Color32::WHITE),
        )
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, Color32::GRAY))
        .min_size(rect.size()),
    )
}
