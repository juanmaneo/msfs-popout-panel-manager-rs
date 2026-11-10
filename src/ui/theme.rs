//! Colours and global egui styling.

use eframe::egui::{self, Color32, Margin, Stroke, Theme, ThemePreference, vec2};

pub const LIME: Color32 = Color32::from_rgb(0, 255, 0);
pub const LIME_GREEN: Color32 = Color32::from_rgb(50, 205, 50);
pub const INDIAN_RED: Color32 = Color32::from_rgb(205, 92, 92);
pub const NAVAJO_WHITE: Color32 = Color32::from_rgb(255, 222, 173);
pub const ANTIQUE_WHITE: Color32 = Color32::from_rgb(250, 235, 215);
pub const DIM_GRAY: Color32 = Color32::from_rgb(105, 105, 105);
pub const SLATE_GRAY: Color32 = Color32::from_rgb(112, 128, 144);
pub const SUCCESS: Color32 = Color32::from_rgb(76, 175, 80);
pub const ERROR: Color32 = Color32::from_rgb(229, 57, 53);
pub const UNKNOWN: Color32 = Color32::from_rgb(130, 130, 130);
pub const ACCENT: Color32 = Color32::from_rgb(33, 150, 243);
pub const CARD_FILL: Color32 = Color32::from_rgb(45, 45, 48);
pub const CARD_FILL_SELECTED: Color32 = Color32::from_rgb(82, 82, 82);
pub const BAR_FILL: Color32 = Color32::from_rgb(24, 24, 26);

/// `0xRRGGBB` to a colour.
pub fn rgb(value: u32) -> Color32 {
    Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

/// Parse an `RRGGBB` / `#RRGGBB` string (as used for the title bar colour).
pub fn parse_hex_color(text: &str) -> Option<Color32> {
    let digits = text.trim().trim_start_matches('#');
    if digits.len() != 6 {
        return None;
    }
    u32::from_str_radix(digits, 16).ok().map(rgb)
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(ThemePreference::Dark);
    ctx.style_mut_of(Theme::Dark, |style| {
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(10.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        style.spacing.combo_width = 140.0;

        let visuals = &mut style.visuals;
        visuals.panel_fill = Color32::from_rgb(32, 32, 34);
        visuals.window_fill = Color32::from_rgb(40, 40, 43);
        visuals.extreme_bg_color = Color32::from_rgb(24, 24, 26);
        visuals.selection.bg_fill = ACCENT;
        visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        visuals.hyperlink_color = Color32::from_rgb(100, 181, 246);
    });
}

pub fn card_frame(fill: Color32, border: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(6)
        .inner_margin(Margin::same(10))
}
