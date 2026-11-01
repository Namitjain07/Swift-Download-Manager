//! Small custom widgets: toolbar buttons, category rows and progress bars.

use crate::engine::Segment;
use crate::icons::{self, Icon};
use eframe::egui::{self, Color32, CornerRadius, Sense, Stroke, StrokeKind};

pub fn tool_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool, tip: &str) -> bool {
    let size = egui::vec2(72.0, 58.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if enabled && resp.hovered() {
        let fill = if resp.is_pointer_button_down_on() { Color32::from_rgb(196, 220, 242) } else { Color32::from_rgb(225, 238, 250) };
        p.rect_filled(rect, CornerRadius::same(4), fill);
        p.rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0, Color32::from_rgb(160, 196, 230)), StrokeKind::Inside);
    }
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 20.0), egui::vec2(30.0, 30.0));
    icons::draw(p, icon_rect, icon, enabled);
    p.text(
        egui::pos2(rect.center().x, rect.bottom() - 9.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(12.0),
        if enabled { Color32::from_rgb(40, 46, 56) } else { Color32::from_gray(165) },
    );
    let clicked = resp.clicked();
    resp.on_hover_text(tip);
    clicked && enabled
}

pub fn toolbar_sep(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 50.0), Sense::hover());
    ui.painter().line_segment(
        [egui::pos2(r.center().x, r.top() + 4.0), egui::pos2(r.center().x, r.bottom() - 4.0)],
        Stroke::new(1.0, Color32::from_rgb(210, 214, 220)),
    );
}

pub fn cat_row(ui: &mut egui::Ui, label: &str, count: usize, indent: f32, col: Color32, selected: bool) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(3), Color32::from_rgb(204, 228, 247));
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(3), Color32::from_rgb(229, 238, 247));
    }
    let icon = egui::Rect::from_center_size(egui::pos2(rect.left() + indent + 12.0, rect.center().y), egui::vec2(16.0, 14.0));
    icons::folder(p, icon, col);
    p.text(
        egui::pos2(rect.left() + indent + 26.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.5),
        Color32::from_rgb(35, 40, 50),
    );
    if count > 0 {
        p.text(
            egui::pos2(rect.right() - 6.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            count.to_string(),
            egui::FontId::proportional(12.0),
            Color32::from_gray(120),
        );
    }
    resp
}

pub fn mini_bar(ui: &mut egui::Ui, frac: f32, col: Color32, text: &str) {
    let w = ui.available_width().min(110.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 16.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(3), Color32::from_rgb(228, 232, 236));
    let mut fill = rect;
    fill.set_width(rect.width() * frac.clamp(0.0, 1.0));
    p.rect_filled(fill, CornerRadius::same(3), col);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(11.5), Color32::from_rgb(20, 24, 30));
}

/// Overall progress plus a strip showing what each connection has fetched.
pub fn segment_bar(ui: &mut egui::Ui, frac: f32, total: u64, segs: &[Segment]) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 18.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(3), Color32::from_rgb(228, 232, 236));
    let mut fill = rect;
    fill.set_width(rect.width() * frac.clamp(0.0, 1.0));
    p.rect_filled(fill, CornerRadius::same(3), Color32::from_rgb(46, 164, 79));
    p.text(rect.center(), egui::Align2::CENTER_CENTER, format!("{:.1}%", frac * 100.0), egui::FontId::proportional(12.0), Color32::from_rgb(20, 24, 30));

    let (strip, _) = ui.allocate_exact_size(egui::vec2(w, 12.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(strip, CornerRadius::ZERO, Color32::from_rgb(236, 238, 242));
    if total > 0 {
        for s in segs {
            let x0 = strip.left() + strip.width() * (s.start as f32 / total as f32);
            let x1 = strip.left() + strip.width() * ((s.start + s.done.min(s.len())) as f32 / total as f32);
            p.rect_filled(
                egui::Rect::from_min_max(egui::pos2(x0, strip.top()), egui::pos2(x1.max(x0), strip.bottom())),
                CornerRadius::ZERO,
                Color32::from_rgb(33, 118, 214),
            );
            p.line_segment([egui::pos2(x0, strip.top()), egui::pos2(x0, strip.bottom())], Stroke::new(1.0, Color32::WHITE));
        }
    }
    p.rect_stroke(strip, CornerRadius::ZERO, Stroke::new(1.0, Color32::from_rgb(200, 205, 212)), StrokeKind::Inside);
}
