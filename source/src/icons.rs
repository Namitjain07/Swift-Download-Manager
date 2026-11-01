//! Simple drawn icons (no images are bundled).

use eframe::egui::{self, pos2, vec2, Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind};

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
    Grabber,
    Batch,
    Scheduler,
}

fn arrow_down(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let w = s * 0.22;
    let body = Rect::from_min_max(pos2(c.x - w, c.y - s * 0.45), pos2(c.x + w, c.y + s * 0.02));
    p.rect_filled(body, CornerRadius::same(2), col);
    p.add(egui::Shape::convex_polygon(
        vec![pos2(c.x - s * 0.45, c.y), pos2(c.x + s * 0.45, c.y), pos2(c.x, c.y + s * 0.48)],
        col,
        Stroke::NONE,
    ));
}

pub fn draw(p: &Painter, rect: Rect, icon: Icon, enabled: bool) {
    let c = rect.center();
    let s = rect.width().min(rect.height());
    let dim = |col: Color32| if enabled { col } else { Color32::from_gray(185) };
    let blue = dim(Color32::from_rgb(33, 118, 214));
    let green = dim(Color32::from_rgb(46, 164, 79));
    let red = dim(Color32::from_rgb(214, 59, 50));
    let gray = dim(Color32::from_rgb(110, 118, 128));
    let orange = dim(Color32::from_rgb(232, 145, 38));
    let white = Color32::WHITE;
    match icon {
        Icon::AddUrl => {
            p.circle_filled(c, s * 0.42, blue);
            let st = Stroke::new(1.4, Color32::from_rgba_unmultiplied(255, 255, 255, 170));
            p.circle_stroke(c, s * 0.42, Stroke::new(1.0, blue));
            p.line_segment([pos2(c.x - s * 0.42, c.y), pos2(c.x + s * 0.42, c.y)], st);
            p.line_segment([pos2(c.x, c.y - s * 0.42), pos2(c.x, c.y + s * 0.42)], st);
            let e = Rect::from_center_size(c, vec2(s * 0.42, s * 0.84));
            p.add(egui::Shape::ellipse_stroke(e.center(), e.size() / 2.0, st));
            let pc = pos2(c.x + s * 0.3, c.y + s * 0.3);
            p.circle_filled(pc, s * 0.2, green);
            let ps = Stroke::new(2.4, white);
            p.line_segment([pos2(pc.x - s * 0.11, pc.y), pos2(pc.x + s * 0.11, pc.y)], ps);
            p.line_segment([pos2(pc.x, pc.y - s * 0.11), pos2(pc.x, pc.y + s * 0.11)], ps);
        }
        Icon::Resume => {
            arrow_down(p, c, s, green);
            p.line_segment([pos2(c.x - s * 0.4, c.y + s * 0.47), pos2(c.x + s * 0.4, c.y + s * 0.47)], Stroke::new(2.5, green));
        }
        Icon::Stop => {
            let r = Rect::from_center_size(c, vec2(s * 0.7, s * 0.7));
            p.rect_filled(r, CornerRadius::same(4), red);
            p.rect_filled(Rect::from_center_size(c, vec2(s * 0.3, s * 0.3)), CornerRadius::same(1), white);
        }
        Icon::StopAll => {
            for (dx, dy) in [(-0.14, -0.14), (0.14, 0.14)] {
                let r = Rect::from_center_size(pos2(c.x + s * dx, c.y + s * dy), vec2(s * 0.56, s * 0.56));
                p.rect_filled(r, CornerRadius::same(4), red);
                p.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.5, white), StrokeKind::Inside);
            }
        }
        Icon::Delete => {
            let body = Rect::from_min_max(pos2(c.x - s * 0.3, c.y - s * 0.25), pos2(c.x + s * 0.3, c.y + s * 0.45));
            p.rect_filled(body, CornerRadius::same(3), gray);
            p.rect_filled(Rect::from_center_size(pos2(c.x, c.y - s * 0.35), vec2(s * 0.8, s * 0.1)), CornerRadius::same(2), gray);
            p.rect_filled(Rect::from_center_size(pos2(c.x, c.y - s * 0.44), vec2(s * 0.25, s * 0.08)), CornerRadius::same(1), gray);
            for dx in [-0.13, 0.0, 0.13] {
                p.line_segment([pos2(c.x + s * dx, c.y - s * 0.15), pos2(c.x + s * dx, c.y + s * 0.35)], Stroke::new(1.5, white));
            }
        }
        Icon::DeleteDone => {
            draw(p, Rect::from_center_size(pos2(c.x - s * 0.1, c.y), vec2(s * 0.85, s * 0.85)), Icon::Delete, enabled);
            let pc = pos2(c.x + s * 0.28, c.y + s * 0.28);
            p.circle_filled(pc, s * 0.2, green);
            p.line_segment([pos2(pc.x - s * 0.1, pc.y), pos2(pc.x - s * 0.02, pc.y + s * 0.08)], Stroke::new(2.0, white));
            p.line_segment([pos2(pc.x - s * 0.02, pc.y + s * 0.08), pos2(pc.x + s * 0.11, pc.y - s * 0.08)], Stroke::new(2.0, white));
        }
        Icon::Options => {
            let teeth = 8;
            let mut pts = Vec::new();
            for i in 0..teeth * 2 {
                let a = i as f32 / (teeth * 2) as f32 * std::f32::consts::TAU;
                let r = if i % 2 == 0 { s * 0.45 } else { s * 0.34 };
                pts.push(pos2(c.x + a.cos() * r, c.y + a.sin() * r));
            }
            p.add(egui::Shape::convex_polygon(pts, gray, Stroke::NONE));
            p.circle_filled(c, s * 0.3, gray);
            p.circle_filled(c, s * 0.13, white);
        }
        Icon::StartQueue => {
            for (i, dy) in [-0.3f32, 0.0, 0.3].iter().enumerate() {
                let y = c.y + s * dy;
                let col = if i == 1 { orange } else { gray };
                p.rect_filled(Rect::from_min_max(pos2(c.x - s * 0.45, y - s * 0.08), pos2(c.x + s * 0.05, y + s * 0.08)), CornerRadius::same(2), col);
            }
            p.add(egui::Shape::convex_polygon(
                vec![pos2(c.x + s * 0.1, c.y - s * 0.3), pos2(c.x + s * 0.48, c.y), pos2(c.x + s * 0.1, c.y + s * 0.3)],
                green,
                Stroke::NONE,
            ));
        }
        Icon::StopQueue => {
            for (i, dy) in [-0.3f32, 0.0, 0.3].iter().enumerate() {
                let y = c.y + s * dy;
                let col = if i == 1 { orange } else { gray };
                p.rect_filled(Rect::from_min_max(pos2(c.x - s * 0.45, y - s * 0.08), pos2(c.x + s * 0.05, y + s * 0.08)), CornerRadius::same(2), col);
            }
            p.rect_filled(Rect::from_center_size(pos2(c.x + s * 0.28, c.y), vec2(s * 0.36, s * 0.36)), CornerRadius::same(2), red);
        }
        Icon::Grabber => {
            arrow_down(p, c, s * 0.9, blue);
        }
        Icon::Batch => {
            for (k, dy) in [(0, -0.22f32), (1, 0.0), (2, 0.22)] {
                let r = Rect::from_center_size(pos2(c.x - s * 0.08 + dy * s * 0.5, c.y + dy * s), vec2(s * 0.62, s * 0.36));
                p.rect_filled(r, CornerRadius::same(3), if k == 2 { blue } else { dim(Color32::from_rgb(150, 185, 228)) });
                p.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.2, white), StrokeKind::Inside);
            }
            let pc = pos2(c.x + s * 0.3, c.y + s * 0.3);
            p.circle_filled(pc, s * 0.18, green);
            let ps = Stroke::new(2.2, white);
            p.line_segment([pos2(pc.x - s * 0.1, pc.y), pos2(pc.x + s * 0.1, pc.y)], ps);
            p.line_segment([pos2(pc.x, pc.y - s * 0.1), pos2(pc.x, pc.y + s * 0.1)], ps);
        }
        Icon::Scheduler => {
            p.circle_filled(c, s * 0.44, blue);
            p.circle_filled(c, s * 0.34, white);
            for k in 0..12 {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                let (r0, r1) = (s * 0.28, s * 0.33);
                p.line_segment([pos2(c.x + a.cos() * r0, c.y + a.sin() * r0), pos2(c.x + a.cos() * r1, c.y + a.sin() * r1)], Stroke::new(1.0, gray));
            }
            p.line_segment([c, pos2(c.x, c.y - s * 0.24)], Stroke::new(2.2, dim(Color32::from_rgb(40, 46, 56))));
            p.line_segment([c, pos2(c.x + s * 0.17, c.y + s * 0.06)], Stroke::new(2.2, dim(Color32::from_rgb(40, 46, 56))));
            p.circle_filled(c, s * 0.04, red);
        }
    }
}

/// Small folder icon for the categories list.
pub fn folder(p: &Painter, rect: Rect, col: Color32) {
    let r = Rect::from_center_size(rect.center(), vec2(rect.width() * 0.9, rect.height() * 0.7));
    let tab = Rect::from_min_size(pos2(r.left(), r.top() - 2.0), vec2(r.width() * 0.45, 4.0));
    p.rect_filled(tab, CornerRadius::same(1), col.gamma_multiply(0.8));
    p.rect_filled(r, CornerRadius::same(2), col);
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
    p.rect_filled(r, CornerRadius::same(2), Color32::WHITE);
    p.rect_stroke(r, CornerRadius::same(2), Stroke::new(1.0, col), StrokeKind::Inside);
    let band = Rect::from_min_max(pos2(r.left(), r.bottom() - r.height() * 0.4), r.max);
    p.rect_filled(band, CornerRadius::same(1), col);
}
