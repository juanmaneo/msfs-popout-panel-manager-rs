//! Small reusable widgets: painted icons, toggle switches and tooltip helpers.

use eframe::egui::{
    self, Color32, Painter, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, Ui, Vec2,
    Role, WidgetInfo, pos2, vec2,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Up,
    Down,
    Close,
    Plus,
    Minus,
    Crosshair,
    Alert,
}

pub fn paint_icon(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.28;
    let stroke = Stroke::new(1.8, color);
    match icon {
        Icon::Up => {
            painter.add(Shape::convex_polygon(
                vec![
                    pos2(c.x - r, c.y + r * 0.6),
                    pos2(c.x + r, c.y + r * 0.6),
                    pos2(c.x, c.y - r * 0.8),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Down => {
            painter.add(Shape::convex_polygon(
                vec![
                    pos2(c.x - r, c.y - r * 0.6),
                    pos2(c.x + r, c.y - r * 0.6),
                    pos2(c.x, c.y + r * 0.8),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Close => {
            painter.line_segment([pos2(c.x - r, c.y - r), pos2(c.x + r, c.y + r)], stroke);
            painter.line_segment([pos2(c.x - r, c.y + r), pos2(c.x + r, c.y - r)], stroke);
        }
        Icon::Plus => {
            painter.line_segment([pos2(c.x - r, c.y), pos2(c.x + r, c.y)], stroke);
            painter.line_segment([pos2(c.x, c.y - r), pos2(c.x, c.y + r)], stroke);
        }
        Icon::Minus => {
            painter.line_segment([pos2(c.x - r, c.y), pos2(c.x + r, c.y)], stroke);
        }
        Icon::Crosshair => {
            painter.circle_stroke(c, r * 0.8, stroke);
            let tick = r * 1.35;
            painter.line_segment([pos2(c.x - tick, c.y), pos2(c.x - r * 0.3, c.y)], stroke);
            painter.line_segment([pos2(c.x + r * 0.3, c.y), pos2(c.x + tick, c.y)], stroke);
            painter.line_segment([pos2(c.x, c.y - tick), pos2(c.x, c.y - r * 0.3)], stroke);
            painter.line_segment([pos2(c.x, c.y + r * 0.3), pos2(c.x, c.y + tick)], stroke);
        }
        Icon::Alert => {
            painter.circle_stroke(c, r * 1.3, stroke);
            painter.line_segment([pos2(c.x, c.y - r * 0.7), pos2(c.x, c.y + r * 0.2)], stroke);
            painter.circle_filled(pos2(c.x, c.y + r * 0.65), 1.2, color);
        }
    }
}

/// Square button showing a painted icon.
pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str) -> Response {
    icon_button_colored(ui, icon, tooltip, None)
}

pub fn icon_button_colored(
    ui: &mut Ui,
    icon: Icon,
    tooltip: &str,
    color: Option<Color32>,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        if response.hovered() || response.has_focus() {
            ui.painter().rect_filled(rect, 4, visuals.weak_bg_fill);
        }
        paint_icon(
            ui.painter(),
            rect,
            icon,
            color.unwrap_or(visuals.fg_stroke.color),
        );
    }
    response.tip(tooltip)
}

/// Filled circle used as a status indicator.
pub fn status_dot(ui: &mut Ui, color: Color32, tooltip: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 5.5, color);
    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip.to_owned())
    }
}

/// Switch-style boolean widget. `changed()` is set when the user flipped it.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let size = vec2(34.0, 18.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| WidgetInfo::selected(Role::CheckBox, ui.is_enabled(), *on, ""));

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let visuals = ui.style().interact_selectable(&response, *on);
        let track = if *on {
            ui.visuals().selection.bg_fill
        } else {
            visuals.bg_fill
        };
        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, radius, track);
        let knob_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        ui.painter().circle_filled(
            Pos2::new(knob_x, rect.center().y),
            radius - 3.0,
            visuals.fg_stroke.color,
        );
    }
    response
}

/// Toggle switch followed by a clickable label.
pub fn toggle_labeled(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let mut flipped = false;
    let mut response = ui
        .horizontal(|ui| {
            flipped |= toggle(ui, on).changed();
            if ui
                .add(egui::Label::new(label).sense(Sense::click()))
                .clicked()
            {
                *on = !*on;
                flipped = true;
            }
        })
        .response;
    if flipped {
        response.mark_changed();
    }
    response
}

pub fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).strong().size(15.0));
}

pub trait ResponseExt: Sized {
    /// Tooltip that is also shown while the widget is disabled.
    fn tip(self, text: &str) -> Self;
}

impl ResponseExt for Response {
    fn tip(self, text: &str) -> Self {
        if text.is_empty() {
            return self;
        }
        self.on_hover_text(text.to_owned())
            .on_disabled_hover_text(text.to_owned())
    }
}
