//! Spinner wheel: 12 spokes whose brightness trails behind a turning leader.

use crate::theme;
use eframe::egui::{pos2, vec2, CornerRadius, Pos2, Rect, Sense, Shape, Stroke, Ui};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::time::Duration;

const SPOKES: usize = 12;

/// Brightness (40..=255) of spoke `i` when the leader is at `phase` (0..1 is one turn).
pub fn spoke_alpha(i: usize, phase: f32) -> u8 {
    let lead = (phase.rem_euclid(1.0) * SPOKES as f32) as usize % SPOKES;
    let behind = (lead + SPOKES - i % SPOKES) % SPOKES;
    (255.0 - behind as f32 * (215.0 / (SPOKES - 1) as f32)).round() as u8
}

/// A small activity wheel, one turn per second. Repaints about 12 times a second while shown.
pub fn spinner(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let ctx = ui.ctx().clone();
    let pal = theme::current(&ctx);
    let phase = (ctx.input(|i| i.time) % 1.0) as f32;
    let (c, r0, r1) = (rect.center(), size * 0.26, size * 0.46);
    for i in 0..SPOKES {
        let a = i as f32 / SPOKES as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let (dx, dy) = (a.cos(), a.sin());
        let col = pal.secondary.gamma_multiply(spoke_alpha(i, phase) as f32 / 255.0);
        ui.painter().line_segment([pos2(c.x + dx * r0, c.y + dy * r0), pos2(c.x + dx * r1, c.y + dy * r1)], Stroke::new((size * 0.09).max(1.5), col));
    }
    ctx.request_repaint_after(Duration::from_millis(83));
}

/// How far round the circle (0..1) the head of the arc is at `time`, and how long the arc is (a quarter to
/// three quarters of the circle, breathing). The tail catches up with the head, so the arc seems to crawl.
pub fn arc_at(time: f64) -> (f32, f32) {
    let turn = (time * 0.9).rem_euclid(1.0) as f32;
    let breath = 0.5 + 0.5 * (time * 2.4).sin() as f32;
    (turn, 0.25 + 0.5 * breath)
}

/// A smooth loader: one arc that chases itself round a ring. For things that take a moment and give no
/// sign of progress. Repaints at the screen's pace while shown.
pub fn arc_loader(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let ctx = ui.ctx().clone();
    let pal = theme::current(&ctx);
    let (turn, len) = arc_at(ctx.input(|i| i.time));
    let (c, r) = (rect.center(), size * 0.5 - (size * 0.07).max(1.5));
    let width = (size * 0.12).max(2.0);
    let p = ui.painter();
    p.circle_stroke(c, r, Stroke::new(width, pal.track));
    let start = turn * TAU - FRAC_PI_2;
    let steps = ((len * 48.0) as usize).max(4);
    let points: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = start + len * TAU * i as f32 / steps as f32;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    // Round the two ends of the arc.
    for end in [points[0], points[steps]] {
        p.circle_filled(end, width / 2.0, pal.accent);
    }
    p.add(Shape::line(points, Stroke::new(width, pal.accent)));
    ctx.request_repaint_after(Duration::from_millis(16));
}

/// Where the sliding piece of an indeterminate bar is at `time`: its left end and its right end, as
/// fractions of the track (they run past both sides, so the piece slides in and out of sight). It
/// speeds up, then slows down.
pub fn slide_at(time: f64) -> (f32, f32) {
    let t = (time / 1.5).rem_euclid(1.0) as f32;
    let e = if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 };
    let len = 0.36;
    let left = -len + e * (1.0 + len);
    (left, left + len)
}

/// A capsule bar for waiting without a known end: a piece slides across the track again and again.
pub fn indeterminate_bar(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let ctx = ui.ctx().clone();
    let pal = theme::current(&ctx);
    let radius = CornerRadius::same((height / 2.0).ceil() as u8);
    let p = ui.painter();
    p.rect_filled(rect, radius, pal.track);
    let (a, b) = slide_at(ctx.input(|i| i.time));
    let piece = Rect::from_min_max(pos2(rect.left() + rect.width() * a.max(0.0), rect.top()), pos2(rect.left() + rect.width() * b.min(1.0), rect.bottom()));
    if piece.width() > 0.5 {
        p.with_clip_rect(rect).rect_filled(piece, radius, pal.accent);
    }
    ctx.request_repaint_after(Duration::from_millis(16));
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    #[test]
    fn spoke_alpha_fades_around_the_wheel() {
        for step in 0..12 {
            let phase = step as f32 / 12.0 + 0.01;
            assert_eq!(spoke_alpha(step, phase), 255, "leader at step {step}");
            assert!(spoke_alpha((step + 11) % 12, phase) < 255, "the spoke behind the leader is dimmer");
            for i in 0..12 {
                assert!(spoke_alpha(i, phase) >= 40, "never fully invisible");
            }
        }
    }

    #[test]
    fn the_arc_keeps_turning_and_breathes_between_a_quarter_and_three_quarters_of_the_ring() {
        let (mut min, mut max) = (1.0_f32, 0.0_f32);
        let mut last_turn = -1.0_f32;
        let mut wrapped = false;
        for step in 0..600 {
            let (turn, len) = arc_at(step as f64 / 60.0);
            assert!((0.0..1.0).contains(&turn), "{turn}");
            min = min.min(len);
            max = max.max(len);
            wrapped |= turn < last_turn;
            last_turn = turn;
        }
        assert!(min >= 0.25 - 1e-4 && max <= 0.75 + 1e-4, "{min}..{max}");
        assert!(max - min > 0.4, "it really does breathe: {min}..{max}");
        assert!(wrapped, "the head goes round more than once in ten seconds");
    }

    #[test]
    fn the_sliding_piece_enters_from_the_left_and_leaves_on_the_right() {
        let (l0, r0) = slide_at(0.0);
        assert!(r0 <= 0.0 + 1e-6 && l0 < 0.0, "out of sight at the start: {l0}..{r0}");
        let (l1, _) = slide_at(1.4999);
        assert!(l1 >= 0.99, "out of sight at the end: {l1}");
        let mut last = f32::MIN;
        for step in 0..150 {
            let (l, r) = slide_at(step as f64 / 100.0);
            assert!(l >= last - 1e-6, "it only moves right (until it starts again)");
            assert!((r - l - 0.36).abs() < 1e-5, "its length stays the same");
            last = l;
        }
        // Faster in the middle than at the ends.
        let speed = |t: f64| slide_at(t + 0.01).0 - slide_at(t).0;
        assert!(speed(0.7) > speed(0.05) && speed(0.7) > speed(1.4));
    }

    #[test]
    fn the_loaders_take_the_room_they_say_and_draw_in_every_theme() {
        use crate::theme::testing::{input, themed_with};
        use crate::theme::{ColorTheme, ThemeMode};
        for theme in ColorTheme::ALL {
            for mode in [ThemeMode::Light, ThemeMode::Dark] {
                let ctx = themed_with(mode, theme);
                let mut sizes = Vec::new();
                for _ in 0..2 {
                    sizes.clear();
                    let mut out = ctx.run_ui(input(vec2(400.0, 300.0), vec![]), |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            let w = ui.available_width();
                            let before = ui.cursor().min;
                            arc_loader(ui, 24.0);
                            let a = ui.cursor().min - before;
                            let before = ui.cursor().min;
                            let before_x = ui.available_width();
                            indeterminate_bar(ui, 6.0);
                            let c = ui.cursor().min - before;
                            sizes.extend([a.y, c.y, w, ui.spacing().item_spacing.y, before_x]);
                        });
                    });
                    out.textures_delta.clear();
                    assert!(!out.shapes.is_empty());
                }
                // Each is as tall as asked, plus the gap below it.
                let gap = sizes[3];
                assert!((sizes[0] - (24.0 + gap)).abs() < 0.01 && (sizes[1] - (6.0 + gap)).abs() < 0.01, "{sizes:?}");
                assert_eq!(sizes[2], sizes[4], "the bar spans the whole row: {sizes:?}");
            }
        }
    }
}
