//! iOS toggle switch: the knob slides and the track fades to green.

use super::anim::fade;
use crate::theme;
use eframe::egui::{self, pos2, vec2, Color32, CornerRadius, Response, Sense, Ui};

pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let enabled = ui.is_enabled();
    let (rect, mut resp) = ui.allocate_exact_size(vec2(51.0, 31.0), if enabled { Sense::click() } else { Sense::hover() });
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let ctx = ui.ctx().clone();
    let k = fade(&ctx, resp.id.with("knob"), *on, 0.18);
    let pal = theme::current(&ctx);
    let dim = if enabled { 1.0 } else { 0.45 };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(16), pal.separator.lerp_to_gamma(pal.green, k).gamma_multiply(dim));
    let r = 13.5;
    let x = egui::lerp((rect.left() + 2.0 + r)..=(rect.right() - 2.0 - r), k);
    p.circle_filled(pos2(x, rect.center().y), r, Color32::WHITE.gamma_multiply(dim));
    let state = *on;
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Switch, enabled, state, ""));
    resp
}
