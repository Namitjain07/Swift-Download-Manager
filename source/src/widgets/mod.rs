//! Small custom widgets: the iOS-style kit (buttons, switch, ...) plus toolbar buttons,
//! category rows and progress bars.

pub mod anim;
pub mod button;
pub mod card;
pub mod components;
pub mod dialog;
pub mod progress;
pub mod segmented;
pub mod spinner;
pub mod switch;
pub mod wheel;

pub use button::{button, ButtonKind};
pub use card::{card, card_row, card_row_h};
pub use components::{empty_state, icon_button, icon_button_at, icon_segmented, stat_tile};
pub use dialog::{button_row, dialog_window, fit_field, fit_width, group_title, plain_button, primary_button, tinted_button, toggle, toggle_row};
pub use progress::{capsule_bar, segment_bar};
pub use spinner::spinner;
pub use segmented::segmented;
pub use switch::switch;
pub use wheel::time_wheels;

use crate::icons::{self, Icon};
use crate::theme::{self, ThemeStyle};
use eframe::egui::{self, Color32, CornerRadius, Sense, Stroke};

pub fn tool_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool, tip: &str) -> bool {
    let size = egui::vec2(70.0, 58.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let hover = anim::fade(&ctx, resp.id.with("hover"), enabled && resp.hovered(), 0.15);
    let press = anim::fade(&ctx, resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    // Pressed tiles shrink to 97%.
    let rect = rect.shrink2(rect.size() * (0.015 * press));
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md), pal.hover.gamma_multiply(hover + 0.8 * press));
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 20.0), egui::vec2(30.0, 30.0));
    icons::draw(p, icon_rect, icon, icons::tint(icon, &pal), enabled);
    p.text(
        egui::pos2(rect.center().x, rect.bottom() - 9.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(d.caption),
        if enabled { pal.label } else { pal.disabled },
    );
    let clicked = resp.clicked();
    resp.on_hover_text(tip);
    clicked && enabled
}

/// The toolbar tile without its label, for medium-width windows (the tooltip says what it does).
pub fn tool_button_icon(ui: &mut egui::Ui, icon: Icon, enabled: bool, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), if enabled { Sense::click() } else { Sense::hover() });
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let hover = anim::fade(&ctx, resp.id.with("hover"), enabled && resp.hovered(), 0.15);
    let press = anim::fade(&ctx, resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    let rect = rect.shrink2(rect.size() * (0.02 * press));
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md), pal.hover.gamma_multiply(hover + 0.8 * press));
    icons::draw(p, egui::Rect::from_center_size(rect.center(), egui::vec2(24.0, 24.0)), icon, icons::tint(icon, &pal), enabled);
    let clicked = resp.clicked();
    resp.on_hover_text(tip);
    clicked && enabled
}

pub fn toolbar_sep(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 50.0), Sense::hover());
    let col = theme::hairline(ui.ctx());
    ui.painter().line_segment([egui::pos2(r.center().x, r.top() + 8.0), egui::pos2(r.center().x, r.bottom() - 8.0)], Stroke::new(1.0, col));
}

/// What a sidebar row shows in front of its label.
#[derive(Clone, Copy, PartialEq)]
pub enum RowIcon {
    Folder,
    Dot,
}

fn pill_target_id() -> egui::Id {
    egui::Id::unique("sdm_sidebar_pill_target")
}

/// One row of the sidebar source list. The selection pill is not drawn here: the selected
/// row only reports where it is, and `sidebar_pill` slides one pill to it.
pub fn sidebar_row(ui: &mut egui::Ui, label: &str, count: usize, indent: f32, icon: RowIcon, col: Color32, selected: bool) -> egui::Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, d.nav_h), Sense::click());
    if selected {
        ctx.data_mut(|data| data.insert_temp(pill_target_id(), (rect.top(), rect.left(), rect.width(), d.nav_h)));
    }
    let hover = anim::fade(&ctx, resp.id.with("hover"), resp.hovered() && !selected, 0.15);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md), pal.hover.gamma_multiply(hover));
    let ic = egui::pos2(rect.left() + indent + 16.0, rect.center().y);
    match icon {
        RowIcon::Folder => icons::folder(p, egui::Rect::from_center_size(ic, egui::vec2(16.0, 14.0)), col),
        RowIcon::Dot => {
            p.circle_filled(ic, 4.0, col);
        }
    }
    // Drishti: the active row reads in blue-ink, the others in ink-2 (ink while hovered).
    let text = if d.style == ThemeStyle::Drishti {
        if selected { pal.blue_ink } else { pal.secondary.lerp_to_gamma(pal.label, hover) }
    } else {
        pal.label
    };
    p.text(egui::pos2(rect.left() + indent + 32.0, rect.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(d.body), text);
    if count > 0 {
        p.text(egui::pos2(rect.right() - 10.0, rect.center().y), egui::Align2::RIGHT_CENTER, count.to_string(), egui::FontId::proportional(d.caption), pal.tertiary);
    }
    resp
}

/// A sidebar row in the narrow icon rail: just the glyph, with the name and count in a tooltip.
pub fn rail_row(ui: &mut egui::Ui, label: &str, count: usize, icon: RowIcon, col: Color32, selected: bool) -> egui::Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), d.nav_h + 4.0), Sense::click());
    if selected {
        ctx.data_mut(|data| data.insert_temp(pill_target_id(), (rect.top(), rect.left(), rect.width(), d.nav_h + 4.0)));
    }
    let hover = anim::fade(&ctx, resp.id.with("hover"), resp.hovered() && !selected, 0.15);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md), pal.hover.gamma_multiply(hover));
    match icon {
        RowIcon::Folder => icons::folder(p, egui::Rect::from_center_size(rect.center(), egui::vec2(18.0, 15.0)), col),
        RowIcon::Dot => {
            p.circle_filled(rect.center(), 4.5, col);
        }
    }
    resp.on_hover_text(if count > 0 { format!("{label} ({count})") } else { label.to_string() })
}

/// Paints the selection pill behind the sidebar rows. Call it first, before the rows; it
/// glides to wherever the selected row was last frame.
pub fn sidebar_pill(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let Some((top, left, width, height)) = ctx.data(|d| d.get_temp::<(f32, f32, f32, f32)>(pill_target_id())) else { return };
    let y = anim::glide(&ctx, egui::Id::unique("sdm_sidebar_pill_y"), top, 0.18);
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(left, y), egui::vec2(width, height)), CornerRadius::same(d.radius_md), pal.blue_tint);
}

/// A small status-bar entry: optional coloured dot, then caption text. Clickable. It paints
/// its own layout, so it reads the same in left-to-right and right-to-left rows.
pub fn status_chip(ui: &mut egui::Ui, dot: Option<Color32>, text: &str, col: Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(theme::design(ui.ctx()).caption), col);
    let dot_w = if dot.is_some() { 13.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(dot_w + galley.size().x, 14.0), Sense::click());
    if let Some(d) = dot {
        ui.painter().circle_filled(egui::pos2(rect.left() + 3.5, rect.center().y), 3.5, d);
    }
    ui.painter().galley(egui::pos2(rect.left() + dot_w, rect.center().y - galley.size().y / 2.0), galley, col);
    resp
}

/// What a state chip is about: decides its colours.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hue {
    Blue,
    Green,
    Orange,
    Red,
    Neutral,
}

/// Paints a state chip with its top-right corner at `right_top` and returns its width.
pub fn paint_state_chip(p: &egui::Painter, right_top: egui::Pos2, pal: &theme::Palette, d: &theme::Design, hue: Hue, text: &str) -> f32 {
    let (fill, dot, ink) = match hue {
        Hue::Blue => (pal.blue_tint, pal.blue, pal.blue_ink),
        Hue::Green => (pal.green_tint, pal.green, pal.green_ink),
        Hue::Orange => (pal.orange_tint, pal.orange, pal.orange_ink),
        Hue::Red => (pal.red_tint, pal.red, pal.red_ink),
        Hue::Neutral => (pal.surface2, pal.tertiary, pal.secondary),
    };
    let galley = p.layout_no_wrap(text.to_owned(), egui::FontId::proportional(d.caption), ink);
    let w = 12.0 + 9.0 + galley.size().x + 8.0;
    let rect = egui::Rect::from_min_size(egui::pos2(right_top.x - w, right_top.y), egui::vec2(w, 22.0));
    p.rect_filled(rect, CornerRadius::same(11), fill);
    p.circle_filled(egui::pos2(rect.left() + 12.0, rect.center().y), 3.0, dot);
    p.galley(egui::pos2(rect.left() + 21.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    w
}

/// Drishti's state chip: a 22 px capsule with a tinted fill, a dot in the hue and the word in its ink
/// colour. Status is always a word first and a colour second.
pub fn state_chip(ui: &mut egui::Ui, hue: Hue, text: &str) -> egui::Response {
    let pal = theme::current(ui.ctx());
    let (fill, dot, ink) = match hue {
        Hue::Blue => (pal.blue_tint, pal.blue, pal.blue_ink),
        Hue::Green => (pal.green_tint, pal.green, pal.green_ink),
        Hue::Orange => (pal.orange_tint, pal.orange, pal.orange_ink),
        Hue::Red => (pal.red_tint, pal.red, pal.red_ink),
        Hue::Neutral => (pal.surface2, pal.tertiary, pal.secondary),
    };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(theme::design(ui.ctx()).caption), ink);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(8.0 + 6.0 + 6.0 + galley.size().x + 8.0 - 6.0, 22.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(11), fill);
    p.circle_filled(egui::pos2(rect.left() + 12.0, rect.center().y), 3.0, dot);
    p.galley(egui::pos2(rect.left() + 21.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    resp
}
