//! Buttons. The colour fades on hover and the button shrinks to 97% while pressed. The shape
//! follows the design: capsules in iOS, 10 px rounded rectangles in Drishti.

use super::anim::fade;
use crate::theme;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Response, Sense, Ui};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Solid blue, white text: the main action. One per view.
    Primary,
    /// Neutral fill that darkens on hover: the usual action in Drishti.
    Secondary,
    /// Soft blue wash, blue text.
    Tinted,
    /// Blue text only: the quietest.
    Plain,
}

pub fn button(ui: &mut Ui, kind: ButtonKind, label: &str) -> Response {
    let enabled = ui.is_enabled();
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let font = FontId::proportional(d.body);
    let text_size = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), Color32::WHITE).size();
    let pad = if d.capsule_buttons { 14.0 } else { 16.0 };
    let size = egui::vec2(text_size.x + 2.0 * pad, d.control_h.max(text_size.y + 12.0));
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let hover = fade(&ctx, resp.id.with("hover"), enabled && resp.hovered(), 0.15);
    let press = fade(&ctx, resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    let rect = rect.shrink2(rect.size() * (0.015 * press));

    // On a floating sheet (a dialog) the popover colour can equal `surface-2` (it does in dark), so
    // the neutral fill steps up one surface to stay visible.
    let in_sheet = ui.layer_id().order != egui::Order::Background;
    let (rest, over) = if in_sheet { (pal.surface3, pal.surface3.lerp_to_gamma(pal.label, 0.12)) } else { (pal.surface2, pal.surface3) };
    let (fill, text) = match kind {
        ButtonKind::Primary => (pal.blue_solid.lerp_to_gamma(Color32::WHITE, 0.10 * hover).lerp_to_gamma(Color32::BLACK, 0.14 * press), pal.on_accent),
        ButtonKind::Secondary => (rest.lerp_to_gamma(over, hover.max(press)), pal.label),
        ButtonKind::Tinted => (pal.blue_tint.gamma_multiply(1.0 + 0.5 * hover + 0.5 * press), pal.blue_ink),
        ButtonKind::Plain => (pal.hover.gamma_multiply(hover + 0.5 * press), pal.blue_ink),
    };
    let dim = if enabled { 1.0 } else { 0.4 };
    let radius = if d.capsule_buttons { CornerRadius::same((rect.height() * 0.5) as u8) } else { CornerRadius::same(d.radius_md) };
    let p = ui.painter();
    p.rect_filled(rect, radius, fill.gamma_multiply(dim));
    p.text(rect.center(), Align2::CENTER_CENTER, label, font, text.gamma_multiply(dim));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    resp
}
