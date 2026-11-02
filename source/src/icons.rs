//! Monoline icons (1.5 px strokes, no images bundled). They take a tint, so they work in
//! light and dark mode.

use crate::theme::Palette;
use eframe::egui::{pos2, vec2, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    AddUrl,
    Resume,
    Stop,
    StopAll,
    Delete,
    DeleteDone,
    Options,
    StartQueue,
    StopQueue,
    Batch,
    Scheduler,
}

/// The tint a toolbar button gives each icon: blue to add, green to go, red to stop or remove.
pub fn tint(icon: Icon, pal: &Palette) -> Color32 {
    match icon {
        Icon::AddUrl | Icon::Batch | Icon::Scheduler => pal.blue,
        Icon::Resume | Icon::StartQueue => pal.green,
        Icon::Stop | Icon::StopAll | Icon::StopQueue | Icon::Delete | Icon::DeleteDone => pal.red,
        Icon::Options => pal.secondary,
    }
}

/// Disabled icons are drawn at about a third of their strength.
fn dim_for(tint: Color32, enabled: bool) -> Color32 {
    if enabled { tint } else { tint.gamma_multiply(0.35) }
}

fn line(p: &Painter, pts: Vec<Pos2>, s: Stroke) {
    p.add(Shape::line(pts, s));
}

fn closed(p: &Painter, pts: Vec<Pos2>, s: Stroke) {
    p.add(Shape::closed_line(pts, s));
}

fn trash(p: &Painter, c: Pos2, s: f32, st: Stroke) {
    let pt = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    line(p, vec![pt(-0.38, -0.26), pt(0.38, -0.26)], st);
    line(p, vec![pt(-0.12, -0.26), pt(-0.12, -0.40), pt(0.12, -0.40), pt(0.12, -0.26)], st);
    line(p, vec![pt(-0.28, -0.26), pt(-0.24, 0.42), pt(0.24, 0.42), pt(0.28, -0.26)], st);
    for x in [-0.09, 0.09] {
        line(p, vec![pt(x, -0.12), pt(x, 0.28)], st);
    }
}

fn arrow_down(p: &Painter, c: Pos2, s: f32, st: Stroke) {
    let pt = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    line(p, vec![pt(0.0, -0.40), pt(0.0, 0.14)], st);
    line(p, vec![pt(-0.2, -0.06), pt(0.0, 0.14), pt(0.2, -0.06)], st);
}

fn queue_lines(p: &Painter, c: Pos2, s: f32, st: Stroke) {
    for y in [-0.28, 0.0, 0.28] {
        line(p, vec![pos2(c.x - 0.42 * s, c.y + y * s), pos2(c.x + 0.02 * s, c.y + y * s)], st);
    }
}

pub fn draw(p: &Painter, rect: Rect, icon: Icon, tint: Color32, enabled: bool) {
    let c = rect.center();
    let s = rect.width().min(rect.height());
    let st = Stroke::new(1.5, dim_for(tint, enabled));
    let pt = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    match icon {
        Icon::AddUrl => {
            let g = pt(-0.06, -0.06);
            let r = 0.34 * s;
            p.circle_stroke(g, r, st);
            line(p, vec![pos2(g.x - r, g.y), pos2(g.x + r, g.y)], st);
            p.add(Shape::ellipse_stroke(g, vec2(r * 0.45, r), st));
            line(p, vec![pt(0.20, 0.30), pt(0.44, 0.30)], st);
            line(p, vec![pt(0.32, 0.18), pt(0.32, 0.42)], st);
        }
        Icon::Resume => {
            arrow_down(p, c, s, st);
            line(p, vec![pt(-0.38, 0.20), pt(-0.38, 0.40), pt(0.38, 0.40), pt(0.38, 0.20)], st);
        }
        Icon::Stop => {
            p.circle_stroke(c, 0.42 * s, st);
            p.rect_stroke(Rect::from_center_size(c, vec2(0.28 * s, 0.28 * s)), CornerRadius::same(2), st, StrokeKind::Middle);
        }
        Icon::StopAll => {
            line(p, vec![pt(-0.10, 0.10), pt(-0.40, 0.10), pt(-0.40, -0.40), pt(0.10, -0.40), pt(0.10, -0.10)], st);
            p.rect_stroke(Rect::from_center_size(pt(0.15, 0.15), vec2(0.5 * s, 0.5 * s)), CornerRadius::same(3), st, StrokeKind::Middle);
        }
        Icon::Delete => trash(p, c, s, st),
        Icon::DeleteDone => {
            trash(p, pt(-0.12, -0.04), s * 0.82, st);
            line(p, vec![pt(0.14, 0.28), pt(0.24, 0.38), pt(0.42, 0.14)], st);
        }
        Icon::Options => {
            let mut pts = Vec::new();
            for k in 0..8 {
                let a = k as f32 / 8.0 * std::f32::consts::TAU;
                for (da, r) in [(-0.30, 0.33), (-0.14, 0.45), (0.14, 0.45), (0.30, 0.33)] {
                    pts.push(pos2(c.x + (a + da).cos() * r * s, c.y + (a + da).sin() * r * s));
                }
            }
            closed(p, pts, st);
            p.circle_stroke(c, 0.14 * s, st);
        }
        Icon::StartQueue => {
            queue_lines(p, c, s, st);
            closed(p, vec![pt(0.14, -0.30), pt(0.46, 0.0), pt(0.14, 0.30)], st);
        }
        Icon::StopQueue => {
            queue_lines(p, c, s, st);
            p.rect_stroke(Rect::from_center_size(pt(0.30, 0.0), vec2(0.32 * s, 0.32 * s)), CornerRadius::same(2), st, StrokeKind::Middle);
        }
        Icon::Batch => {
            for (i, y) in [-0.28f32, 0.0, 0.28].iter().enumerate() {
                let x = (i as f32 - 1.0) * 0.06;
                p.rect_stroke(Rect::from_center_size(pt(x, *y), vec2(0.64 * s, 0.22 * s)), CornerRadius::same(4), st, StrokeKind::Middle);
            }
        }
        Icon::Scheduler => {
            p.circle_stroke(c, 0.42 * s, st);
            line(p, vec![c, pt(0.0, -0.24)], st);
            line(p, vec![c, pt(0.17, 0.08)], st);
        }
    }
}

/// Small folder outline for the categories list.
pub fn folder(p: &Painter, rect: Rect, col: Color32) {
    let r = Rect::from_center_size(rect.center(), vec2(rect.width() * 0.9, rect.height() * 0.72));
    let tab = r.width() * 0.42;
    closed(
        p,
        vec![
            pos2(r.left(), r.bottom()),
            pos2(r.left(), r.top()),
            pos2(r.left() + tab, r.top()),
            pos2(r.left() + tab + 2.0, r.top() + 2.5),
            pos2(r.right(), r.top() + 2.5),
            pos2(r.right(), r.bottom()),
        ],
        Stroke::new(1.3, col),
    );
}

/// Colour used for a category / file type.
pub fn category_color(cat: &str) -> Color32 {
    match cat {
        "Compressed" => Color32::from_rgb(196, 140, 40),
        "Documents" => Color32::from_rgb(52, 120, 210),
        "Music" => Color32::from_rgb(170, 80, 190),
        "Programs" => Color32::from_rgb(70, 150, 90),
        "Video" => Color32::from_rgb(210, 70, 70),
        "General" => Color32::from_rgb(120, 130, 145),
        other => {
            // Your own categories get a steady colour from their name.
            const P: [(u8, u8, u8); 6] = [(0, 150, 160), (220, 110, 40), (120, 90, 200), (190, 60, 120), (90, 140, 40), (60, 100, 170)];
            let h = other.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
            let (r, g, b) = P[(h % 6) as usize];
            Color32::from_rgb(r, g, b)
        }
    }
}

/// File-type badge drawn in the first column of the list.
pub fn file_badge(p: &Painter, rect: Rect, cat: &str) {
    let r = Rect::from_center_size(rect.center(), vec2(rect.height() * 0.7, rect.height() * 0.85));
    let col = category_color(cat);
    let card = crate::theme::current(p.ctx()).card;
    p.rect_filled(r, CornerRadius::same(2), card);
    p.rect_stroke(r, CornerRadius::same(2), Stroke::new(1.0, col), StrokeKind::Inside);
    let band = Rect::from_min_max(pos2(r.left(), r.bottom() - r.height() * 0.4), r.max);
    p.rect_filled(band, CornerRadius::same(1), col);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Palette;
    use eframe::egui;

    const ALL: [Icon; 11] = [
        Icon::AddUrl,
        Icon::Resume,
        Icon::Stop,
        Icon::StopAll,
        Icon::Delete,
        Icon::DeleteDone,
        Icon::Options,
        Icon::StartQueue,
        Icon::StopQueue,
        Icon::Batch,
        Icon::Scheduler,
    ];

    #[test]
    fn every_icon_paints_in_both_states_with_finite_geometry() {
        for icon in ALL {
            for enabled in [true, false] {
                let ctx = egui::Context::default();
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    let rect = Rect::from_center_size(pos2(40.0, 40.0), vec2(30.0, 30.0));
                    draw(ui.painter(), rect, icon, tint(icon, &Palette::LIGHT), enabled);
                });
                out.textures_delta.clear();
                assert!(!out.shapes.is_empty(), "icon painted nothing (enabled={enabled})");
                for s in &out.shapes {
                    let r = s.shape.visual_bounding_rect();
                    assert!(r.min.x.is_finite() && r.max.y.is_finite(), "non-finite geometry");
                    // Stays inside its 30x30 box (plus the stroke width).
                    assert!(r.width() <= 32.0 && r.height() <= 32.0, "icon spills out of its box: {r:?}");
                }
            }
        }
    }

    #[test]
    fn disabled_tint_is_dimmer_than_enabled() {
        let on = tint(Icon::Stop, &Palette::LIGHT);
        assert!(dim_for(on, false).a() < dim_for(on, true).a());
    }
}
