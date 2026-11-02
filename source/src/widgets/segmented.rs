//! iOS segmented control: one rounded track with a highlight that slides under the choice.

use super::anim::glide;
use crate::theme;
use eframe::egui::{self, Align2, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui};

pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, options: &[(T, &str)], value: &mut T) -> Response {
    let font = FontId::proportional(13.0);
    let widest = options.iter().map(|(_, l)| ui.painter().layout_no_wrap((*l).to_owned(), font.clone(), egui::Color32::WHITE).size().x).fold(0.0, f32::max);
    let seg_w = (widest + 28.0).max(64.0);
    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(seg_w * options.len() as f32 + 4.0, 32.0), Sense::click());
    let inner = rect.shrink(2.0);
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let i = (((pos.x - inner.left()) / seg_w).floor().max(0.0) as usize).min(options.len().saturating_sub(1));
            if options[i].0 != *value {
                *value = options[i].0;
                resp.mark_changed();
            }
        }
    }
    let ctx = ui.ctx().clone();
    let pal = theme::current(&ctx);
    let selected = options.iter().position(|(v, _)| v == value).unwrap_or(0);
    let x = glide(&ctx, resp.id.with("pill"), inner.left() + seg_w * selected as f32, 0.18);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(9), pal.separator.gamma_multiply(0.35));
    let pill = Rect::from_min_size(egui::pos2(x, inner.top()), egui::vec2(seg_w, inner.height()));
    p.rect_filled(pill, CornerRadius::same(7), pal.raised);
    p.rect_stroke(pill, CornerRadius::same(7), Stroke::new(1.0, pal.separator.gamma_multiply(0.6)), StrokeKind::Inside);
    for (i, (_, label)) in options.iter().enumerate() {
        let c = egui::pos2(inner.left() + seg_w * (i as f32 + 0.5), inner.center().y);
        let col = if i == selected { pal.label } else { pal.secondary };
        p.text(c, Align2::CENTER_CENTER, label, font.clone(), col);
    }
    resp
}
