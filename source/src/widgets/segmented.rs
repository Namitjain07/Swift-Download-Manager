//! Segmented control: one rounded track with a white thumb that slides under the choice.

use super::anim::glide;
use crate::theme::{self, Elevation};
use eframe::egui::{self, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Ui};

/// The width of one segment of a control whose widest label is `widest` points wide.
pub fn segment_width(widest: f32) -> f32 {
    (widest + 24.0).max(64.0)
}

fn widest_label<'a>(ui: &Ui, labels: impl Iterator<Item = &'a str>) -> f32 {
    let font = FontId::new(theme::design(ui.ctx()).body - 1.0, theme::semibold());
    labels.map(|l| ui.painter().layout_no_wrap(l.to_owned(), font.clone(), Color32::PLACEHOLDER).size().x).fold(0.0, f32::max)
}

/// The width a control with these labels will have, so a row can keep room for it.
pub fn segmented_width(ui: &Ui, labels: &[&str]) -> f32 {
    segment_width(widest_label(ui, labels.iter().copied())) * labels.len() as f32 + 8.0
}

pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, options: &[(T, &str)], value: &mut T) -> Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let font = FontId::new(d.body - 1.0, theme::semibold());
    let seg_w = segment_width(widest_label(ui, options.iter().map(|(_, l)| *l)));
    let (rect, mut resp) = ui.allocate_exact_size(vec2(seg_w * options.len() as f32 + 8.0, d.control_h), Sense::click());
    let inner = rect.shrink(4.0);
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let i = (((pos.x - inner.left()) / seg_w).floor().max(0.0) as usize).min(options.len().saturating_sub(1));
        if options[i].0 != *value {
            *value = options[i].0;
            resp.mark_changed();
        }
    }
    let selected = options.iter().position(|(v, _)| v == value).unwrap_or(0);
    let x = glide(&ctx, resp.id.with("pill"), inner.left() + seg_w * selected as f32, 0.18);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.round(d.control_h)), pal.surface2);
    let thumb = Rect::from_min_size(egui::pos2(x, inner.top()), vec2(seg_w, inner.height()));
    let radius = CornerRadius::same(d.round(inner.height()));
    p.add(theme::shadow(&ctx, Elevation::Rest).as_shape(thumb, radius));
    p.rect_filled(thumb, radius, pal.segment_on);
    for (i, (_, label)) in options.iter().enumerate() {
        let c = Pos2::new(inner.left() + seg_w * (i as f32 + 0.5), inner.center().y);
        let (col, family) = if i == selected { (pal.label, theme::semibold()) } else { (pal.secondary, egui::FontFamily::Proportional) };
        p.text(c, Align2::CENTER_CENTER, label, FontId::new(font.size, family), col);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::themed;
    use crate::theme::ThemeMode;

    #[test]
    fn segments_are_at_least_64_wide_and_grow_with_the_longest_label() {
        assert_eq!(segment_width(10.0), 64.0);
        assert_eq!(segment_width(100.0), 124.0);
    }

    #[test]
    fn the_predicted_width_is_the_width_that_gets_laid_out() {
        let ctx = themed(ThemeMode::Light);
        let mut v = 0u8;
        let (mut predicted, mut actual) = (0.0, 0.0);
        for _ in 0..2 {
            let mut out = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    predicted = segmented_width(ui, &["Ask", "Keep running", "Quit"]);
                    actual = segmented(ui, &[(0u8, "Ask"), (1, "Keep running"), (2, "Quit")], &mut v).rect.width();
                });
            });
            out.textures_delta.clear();
        }
        assert!((predicted - actual).abs() < 0.01, "{predicted} vs {actual}");
    }

    #[test]
    fn the_control_is_a_button_high_capsule_that_keeps_its_value() {
        let ctx = themed(ThemeMode::Light);
        let mut v = 1u8;
        let mut rect = Rect::NOTHING;
        for _ in 0..2 {
            let mut out = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    rect = segmented(ui, &[(0u8, "Automatic"), (1, "Light"), (2, "Dark")], &mut v).rect;
                });
            });
            out.textures_delta.clear();
        }
        assert_eq!(rect.height(), theme::DESIGN.control_h);
        assert_eq!(v, 1);
    }
}
