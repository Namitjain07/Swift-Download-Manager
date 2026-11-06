//! The iOS switch: a 51 x 31 capsule that fades to green while a white thumb slides across it.

use super::anim::fade;
use crate::theme;
use eframe::egui::{self, pos2, vec2, Color32, CornerRadius, Response, Sense, Ui, Vec2};

/// Width and height of a switch.
pub fn switch_size(ctx: &egui::Context) -> Vec2 {
    let (w, h) = theme::design(ctx).switch;
    vec2(w, h)
}

pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let enabled = ui.is_enabled();
    let ctx = ui.ctx().clone();
    let pal = theme::current(&ctx);
    let (rect, mut resp) = ui.allocate_exact_size(switch_size(&ctx), if enabled { Sense::click() } else { Sense::hover() });
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let k = fade(&ctx, resp.id.with("knob"), *on, 0.2);
    // iOS switches turn green; Fluent and Carbon ones turn the accent colour.
    let on_colour = pal.accent_solid.lerp_to_gamma(pal.green, theme::design(&ctx).pill);
    let dim = if enabled { 1.0 } else { 0.4 };
    let margin = 2.0;
    let r = (rect.height() - 2.0 * margin) / 2.0;
    let x = egui::lerp((rect.left() + margin + r)..=(rect.right() - margin - r), k);
    let thumb = pos2(x, rect.center().y);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same((rect.height() / 2.0) as u8), pal.surface3.lerp_to_gamma(on_colour, k).gamma_multiply(dim));
    // The thumb's soft shadow: two translucent discs under it.
    p.circle_filled(thumb + vec2(0.0, 2.0), r + 1.5, Color32::from_black_alpha(26));
    p.circle_filled(thumb + vec2(0.0, 1.0), r + 0.5, Color32::from_black_alpha(34));
    p.circle_filled(thumb, r, Color32::WHITE.gamma_multiply(dim));
    let state = *on;
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, state, ""));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::themed;
    use crate::theme::ThemeMode;

    #[test]
    fn a_switch_has_the_themes_size_and_never_flips_by_itself() {
        let ctx = themed(ThemeMode::Light);
        let mut on = false;
        let mut size = Vec2::ZERO;
        for _ in 0..2 {
            let mut out = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    size = switch(ui, &mut on).rect.size();
                });
            });
            out.textures_delta.clear();
        }
        assert_eq!(size, vec2(51.0, 31.0));
        assert!(!on);
    }
}
