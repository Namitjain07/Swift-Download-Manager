//! Toggle switch: the knob slides and the track fades to green.
//! Drishti: 44 x 26 with a 20 px thumb that casts the design system's thumb shadow. iOS: 51 x 31.

use super::anim::fade;
use crate::theme;
use eframe::egui::{self, pos2, vec2, Color32, CornerRadius, Response, Sense, Ui};

pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let enabled = ui.is_enabled();
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let (w, h) = d.switch;
    let (rect, mut resp) = ui.allocate_exact_size(vec2(w, h), if enabled { Sense::click() } else { Sense::hover() });
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let k = fade(&ctx, resp.id.with("knob"), *on, 0.2);
    let dim = if enabled { 1.0 } else { 0.4 };
    let margin = if d.sheet_shadow { 3.0 } else { 2.0 };
    let r = (h - 2.0 * margin) / 2.0;
    let x = egui::lerp((rect.left() + margin + r)..=(rect.right() - margin - r), k);
    let thumb = pos2(x, rect.center().y);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same((h / 2.0) as u8), pal.surface3.lerp_to_gamma(pal.green, k).gamma_multiply(dim));
    if d.sheet_shadow {
        // shadow-thumb: 0 2px 6px rgba(0,0,0,.35), as two soft discs under the thumb.
        p.circle_filled(thumb + vec2(0.0, 2.0), r + 1.5, Color32::from_black_alpha(28));
        p.circle_filled(thumb + vec2(0.0, 1.0), r + 0.5, Color32::from_black_alpha(36));
    }
    p.circle_filled(thumb, r, pal.on_accent.gamma_multiply(dim));
    let state = *on;
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Switch, enabled, state, ""));
    resp
}
