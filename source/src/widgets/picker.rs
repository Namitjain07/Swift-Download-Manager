//! The colour theme picker: a grid of small pictures, each one a miniature window in the colours of
//! its theme, so that you can see what you are choosing. The chosen one gets a ring in its accent.

use super::anim::fade;
use crate::icons::{self, Icon};
use crate::theme::{self, ColorTheme};
use eframe::egui::{self, vec2, Align2, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui};

/// Smallest width of one swatch, and the height of its picture and of the whole swatch.
pub const SWATCH_MIN_W: f32 = 124.0;
pub const SWATCH_H: f32 = 92.0;
const PICTURE_H: f32 = 64.0;
const GAP: f32 = 10.0;

/// How many swatches fit in a row of `width` points.
pub fn columns(width: f32) -> usize {
    (((width + GAP) / (SWATCH_MIN_W + GAP)).floor() as usize).clamp(1, ColorTheme::ALL.len())
}

/// The picker. Returns a response that is `changed` when another theme was chosen.
pub fn theme_picker(ui: &mut Ui, value: &mut ColorTheme) -> Response {
    let ctx = ui.ctx().clone();
    let dark = ctx.global_style().visuals.dark_mode;
    let cols = columns(ui.available_width());
    let w = (ui.available_width() - GAP * (cols - 1) as f32) / cols as f32;
    let mut changed = false;
    let mut any = None;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        for row in ColorTheme::ALL.chunks(cols) {
            ui.horizontal(|ui| {
                for &theme in row {
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, SWATCH_H), Sense::click());
                    paint_swatch(ui, rect, &resp, theme, dark, *value == theme);
                    if resp.clicked() && *value != theme {
                        *value = theme;
                        changed = true;
                    }
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::RadioButton, true, theme.name()));
                    any = Some(resp);
                }
            });
        }
    });
    let mut resp = any.expect("there are themes");
    if changed {
        resp.mark_changed();
    }
    resp
}

fn paint_swatch(ui: &Ui, rect: Rect, resp: &Response, theme: ColorTheme, dark: bool, selected: bool) {
    let ctx = ui.ctx();
    let (pal, d) = (theme::current(ctx), theme::design(ctx));
    let sw = theme.palette(dark);
    let hover = fade(ctx, resp.id.with("hover"), resp.hovered(), 0.12);
    let on = fade(ctx, resp.id.with("on"), selected, 0.18);
    let p = ui.painter();
    let picture = Rect::from_min_size(rect.min, vec2(rect.width(), PICTURE_H));
    let radius = CornerRadius::same(d.radius_lg.min(12));
    // The picture: the theme's own window ground, with a header strip, a card and an accent button.
    p.rect_filled(picture, radius, sw.bg);
    p.rect_stroke(picture, radius, Stroke::new(1.0, pal.separator_strong.lerp_to_gamma(pal.label, 0.35 * hover)), StrokeKind::Inside);
    let inner = picture.shrink(8.0);
    let strip = Rect::from_min_size(inner.min, vec2(inner.width(), 10.0));
    p.rect_filled(strip, CornerRadius::same(3), sw.chrome);
    p.circle_filled(Pos2::new(strip.left() + 5.0, strip.center().y), 2.5, sw.accent);
    let card = Rect::from_min_max(Pos2::new(inner.left(), strip.bottom() + 5.0), inner.max);
    let card_radius = CornerRadius::same(theme.shape().design().radius_lg.min(6));
    p.rect_filled(card, card_radius, sw.card);
    p.rect_stroke(card, card_radius, Stroke::new(1.0, sw.separator), StrokeKind::Inside);
    for (i, frac) in [0.62, 0.38].into_iter().enumerate() {
        let line = Rect::from_min_size(Pos2::new(card.left() + 6.0, card.top() + 6.0 + i as f32 * 6.0), vec2((card.width() - 12.0) * frac, 3.0));
        p.rect_filled(line, CornerRadius::same(2), if i == 0 { sw.label.gamma_multiply(0.7) } else { sw.tertiary.gamma_multiply(0.8) });
    }
    let button_h = 8.0;
    let button = Rect::from_min_size(Pos2::new(card.right() - 6.0 - 26.0, card.bottom() - 6.0 - button_h), vec2(26.0, button_h));
    p.rect_filled(button, CornerRadius::same(theme.shape().design().round(button_h).min(4)), sw.accent_solid);
    // The ring of the chosen theme, and its name.
    if on > 0.0 {
        p.rect_stroke(picture.expand(2.5), CornerRadius::same(radius.nw + 3), Stroke::new(2.0, pal.accent.gamma_multiply(on)), StrokeKind::Outside);
    }
    let name = FontId::new(d.caption + 0.5, if selected { theme::semibold() } else { egui::FontFamily::Proportional });
    p.text(Pos2::new(rect.left() + 2.0, picture.bottom() + 15.0), Align2::LEFT_CENTER, theme.name(), name, if selected { pal.label } else { pal.secondary });
    if on > 0.0 {
        icons::draw_weight(p, Rect::from_center_size(Pos2::new(rect.right() - 10.0, picture.bottom() + 15.0), vec2(14.0, 14.0)), Icon::Check, pal.accent_ink.gamma_multiply(on), true, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::{click, run, themed};
    use crate::theme::ThemeMode;
    use std::cell::Cell;

    #[test]
    fn the_number_of_columns_follows_the_width_and_never_leaves_a_swatch_too_narrow() {
        assert_eq!(columns(100.0), 1, "at least one");
        assert_eq!(columns(124.0), 1);
        assert_eq!(columns(258.0), 2, "two swatches and one gap need 258");
        assert_eq!(columns(572.0), 4);
        assert_eq!(columns(10_000.0), ColorTheme::ALL.len(), "never more than there are themes");
        for width in [200.0_f32, 400.0, 572.0, 900.0] {
            let c = columns(width);
            let each = (width - GAP * (c - 1) as f32) / c as f32;
            assert!(each >= SWATCH_MIN_W - 0.01, "{width}: {each}");
        }
    }

    #[test]
    fn the_picker_lists_every_theme_inside_the_width_it_is_given() {
        let ctx = themed(ThemeMode::Light);
        let mut v = ColorTheme::Airbnb;
        let (mut top, mut last) = (Pos2::ZERO, Rect::NOTHING);
        for _ in 0..2 {
            run(&ctx, vec2(572.0, 900.0), vec![], |ui| {
                top = ui.cursor().min;
                // The central panel always fills the screen, so measure the swatches themselves.
                last = theme_picker(ui, &mut v).rect;
            });
        }
        let rows = ColorTheme::ALL.len().div_ceil(columns(572.0));
        let want = rows as f32 * SWATCH_H + (rows - 1) as f32 * GAP;
        assert!((last.bottom() - top.y - want).abs() < 0.5, "{rows} rows should take {want}, took {}", last.bottom() - top.y);
        assert!(last.right() <= 572.0, "the last swatch ends at {}, past the width given", last.right());
        assert!(last.width() >= SWATCH_MIN_W - 0.01, "{last:?}");
    }

    #[test]
    fn clicking_a_swatch_chooses_that_theme() {
        let ctx = themed(ThemeMode::Light);
        let value = Cell::new(ColorTheme::Airbnb);
        let place = Cell::new((Pos2::ZERO, 0.0_f32));
        let mut add = |ui: &mut Ui| {
            let mut v = value.get();
            place.set((ui.cursor().min, ui.available_width()));
            theme_picker(ui, &mut v);
            value.set(v);
        };
        run(&ctx, vec2(572.0, 900.0), vec![], &mut add);
        // The second swatch of the first row is the Files theme.
        let (top, room) = place.get();
        let cols = columns(room);
        let w = (room - GAP * (cols - 1) as f32) / cols as f32;
        let at = top + vec2(w + GAP + 20.0, 20.0);
        click(&ctx, vec2(572.0, 900.0), at, &mut add);
        assert_eq!(value.get(), ColorTheme::Files);
        // Clicking the chosen one again changes nothing.
        click(&ctx, vec2(572.0, 900.0), at, &mut add);
        assert_eq!(value.get(), ColorTheme::Files);
    }

    #[test]
    fn a_picker_draws_in_every_theme_and_both_modes_without_panicking() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            for theme in ColorTheme::ALL {
                let ctx = crate::theme::testing::themed_with(mode, theme);
                let mut v = theme;
                run(&ctx, vec2(572.0, 900.0), vec![], |ui| {
                    theme_picker(ui, &mut v);
                });
                assert_eq!(v, theme);
            }
        }
    }
}
