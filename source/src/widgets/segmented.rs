//! Segmented control: one rounded track with a thumb that slides under the choice.

use super::anim::glide;
use crate::theme;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, vec2};

pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, options: &[(T, &str)], value: &mut T) -> Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let font = FontId::proportional(d.body - 1.0);
    let widest = options.iter().map(|(_, l)| ui.painter().layout_no_wrap((*l).to_owned(), font.clone(), Color32::WHITE).size().x).fold(0.0, f32::max);
    let seg_w = (widest + 28.0).max(64.0);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(seg_w * options.len() as f32 + 4.0, 32.0), Sense::click());
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
    let selected = options.iter().position(|(v, _)| v == value).unwrap_or(0);
    let x = glide(&ctx, resp.id.with("pill"), inner.left() + seg_w * selected as f32, 0.18);
    // On a floating sheet the track steps up one surface so it stays visible (see `button`).
    let track = if ui.layer_id().order != egui::Order::Background { pal.surface3 } else { pal.surface2 };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md), track);
    let thumb = Rect::from_min_size(egui::pos2(x, inner.top()), vec2(seg_w, inner.height()));
    let radius = CornerRadius::same(d.radius_sm.saturating_sub(1));
    if d.sheet_shadow {
        p.rect_filled(thumb.translate(vec2(0.0, 1.5)).expand(0.5), radius, Color32::from_black_alpha(40));
    }
    p.rect_filled(thumb, radius, pal.segment_on);
    if d.card_border {
        p.rect_stroke(thumb, radius, Stroke::new(1.0, theme::hairline(&ctx)), StrokeKind::Inside);
    }
    for (i, (_, label)) in options.iter().enumerate() {
        let c = egui::pos2(inner.left() + seg_w * (i as f32 + 0.5), inner.center().y);
        let col = if i == selected { pal.label } else { pal.secondary };
        p.text(c, Align2::CENTER_CENTER, label, font.clone(), col);
    }
    resp
}
