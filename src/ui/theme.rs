//! Colours and global egui styling.

use eframe::egui::{self, Color32, Margin, Stroke, Theme, ThemePreference, vec2};

pub const LIME_GREEN: Color32 = Color32::from_rgb(50, 205, 50);
pub const INDIAN_RED: Color32 = Color32::from_rgb(205, 92, 92);
pub const NAVAJO_WHITE: Color32 = Color32::from_rgb(255, 222, 173);
pub const ANTIQUE_WHITE: Color32 = Color32::from_rgb(250, 235, 215);
pub const SUCCESS: Color32 = Color32::from_rgb(76, 175, 80);
pub const ERROR: Color32 = Color32::from_rgb(229, 57, 53);
pub const ACCENT: Color32 = Color32::from_rgb(33, 150, 243);
pub const CARD_FILL: Color32 = Color32::from_rgb(45, 45, 48);

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

/// egui's proportional font lacks the arrow glyphs the UI uses (`◀ ▶ ▲ ▼`), and without the
/// `system_fonts` feature there is no OS fallback, so fall back to the bundled monospace
/// font, which has them.
fn fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        proportional.push("Hack".to_owned());
    }
    fonts
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_font_is_a_proportional_fallback() {
        let fonts = fonts();
        let proportional = &fonts.families[&egui::FontFamily::Proportional];
        assert!(proportional.iter().any(|f| f == "Hack"));
        assert!(fonts.font_data.contains_key("Hack"));
    }
}
