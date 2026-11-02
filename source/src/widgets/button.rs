//! Capsule buttons: filled, tinted and plain. The colour fades on hover and the button
//! shrinks to 97% while pressed.

use super::anim::fade;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Response, Sense, Ui};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Solid colour, white text: the main action.
    Filled,
    /// Soft colour wash, coloured text: secondary actions.
    Tinted,
    /// Coloured text only: the quietest.
    Plain,
}

pub fn button(ui: &mut Ui, kind: ButtonKind, tint: Color32, label: &str) -> Response {
    let enabled = ui.is_enabled();
    let font = FontId::proportional(13.0);
    let text_size = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), Color32::WHITE).size();
    let size = egui::vec2(text_size.x + 28.0, (text_size.y + 12.0).max(30.0));
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let ctx = ui.ctx().clone();
    let hover = fade(&ctx, resp.id.with("hover"), enabled && resp.hovered(), 0.12);
    let press = fade(&ctx, resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    let rect = rect.shrink2(rect.size() * (0.015 * press));

    let (fill, text) = match kind {
        ButtonKind::Filled => (tint.lerp_to_gamma(Color32::WHITE, 0.10 * hover).lerp_to_gamma(Color32::BLACK, 0.14 * press), Color32::WHITE),
        ButtonKind::Tinted => (tint.gamma_multiply(0.14 + 0.08 * hover + 0.08 * press), tint),
        ButtonKind::Plain => (tint.gamma_multiply(0.10 * hover + 0.10 * press), tint),
    };
    let dim = if enabled { 1.0 } else { 0.4 };
    let radius = CornerRadius::same((rect.height() * 0.5) as u8);
    let p = ui.painter();
    p.rect_filled(rect, radius, fill.gamma_multiply(dim));
    p.text(rect.center(), Align2::CENTER_CENTER, label, font, text.gamma_multiply(dim));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    resp
}
