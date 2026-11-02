//! Small custom widgets: the iOS-style kit (buttons, switch, ...) plus toolbar buttons,
//! category rows and progress bars.
// The kit is wired into the windows step by step.
#![allow(dead_code, unused_imports)]

pub mod anim;
pub mod button;
pub mod card;
pub mod progress;
pub mod segmented;
pub mod spinner;
pub mod switch;
pub mod wheel;

pub use button::{button, ButtonKind};
pub use card::{card, card_row};
pub use progress::{capsule_bar, mini_bar, segment_bar};
pub use spinner::spinner;
pub use segmented::segmented;
pub use switch::switch;
pub use wheel::{time_wheels, wheel};

use crate::engine::Segment;
use crate::icons::{self, Icon};
use crate::theme;
use eframe::egui::{self, Color32, CornerRadius, Sense, Stroke, StrokeKind};

pub fn tool_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool, tip: &str) -> bool {
    let size = egui::vec2(72.0, 58.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let ctx = ui.ctx().clone();
    let pal = theme::current(&ctx);
    let hover = anim::fade(&ctx, resp.id.with("hover"), enabled && resp.hovered(), 0.12);
    let press = anim::fade(&ctx, resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    // Pressed tiles shrink to 97%.
    let rect = rect.shrink2(rect.size() * (0.015 * press));
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(10), pal.separator.gamma_multiply(0.30 * hover + 0.25 * press));
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 20.0), egui::vec2(30.0, 30.0));
    icons::draw(p, icon_rect, icon, icons::tint(icon, &pal), enabled);
    p.text(
        egui::pos2(rect.center().x, rect.bottom() - 9.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.0),
        if enabled { pal.label } else { pal.secondary.gamma_multiply(0.6) },
    );
    let clicked = resp.clicked();
    resp.on_hover_text(tip);
    clicked && enabled
}

pub fn toolbar_sep(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 50.0), Sense::hover());
    let col = theme::current(ui.ctx()).separator;
    ui.painter().line_segment([egui::pos2(r.center().x, r.top() + 8.0), egui::pos2(r.center().x, r.bottom() - 8.0)], Stroke::new(1.0, col));
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

/// A small status-bar entry: optional coloured dot, then caption text. Clickable. It paints
/// its own layout, so it reads the same in left-to-right and right-to-left rows.
pub fn status_chip(ui: &mut egui::Ui, dot: Option<Color32>, text: &str, col: Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(11.0), col);
    let dot_w = if dot.is_some() { 13.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(dot_w + galley.size().x, 14.0), Sense::click());
    if let Some(d) = dot {
        ui.painter().circle_filled(egui::pos2(rect.left() + 3.5, rect.center().y), 3.5, d);
    }
    ui.painter().galley(egui::pos2(rect.left() + dot_w, rect.center().y - galley.size().y / 2.0), galley, col);
    resp
}
