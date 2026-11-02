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
    icons::draw(p, icon_rect, icon, icons::tint(icon, &crate::theme::current(ui.ctx())), enabled);
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
