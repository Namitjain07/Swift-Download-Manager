//! The widgets of the app: buttons, inputs, menus, dialogs, cards and the pieces of the main
//! window (toolbar tiles, sidebar rows, state chips). Every one of them reads its colours and sizes
//! from `theme`, so they agree with each other.

pub mod anim;
pub mod button;
pub mod card;
pub mod components;
pub mod dialog;
pub mod field;
#[cfg(test)]
mod interaction;
pub mod menu;
pub mod progress;
pub mod segmented;
pub mod spinner;
pub mod switch;
pub mod wheel;

pub use button::{button, button_width, icon_label_button, small_button, small_icon_label_button, ButtonKind, ButtonSize};
pub use card::{card, disclosure, form_field, hint, kv, kv_text, message, notice, section_title};
pub use components::{empty_state, icon_button, icon_button_at, icon_button_primary, icon_segmented, stat_tile};
pub use dialog::{check_row, plain_button, primary_button, secondary_button, sheet, truncated};
pub use field::{dropdown, field, field_full, field_multiline, number_field, search_field, stepper};
pub use menu::{menu_caption, menu_check_row, menu_danger_item, menu_icon_item, menu_item, menu_separator, menu_submenu, menu_toggle, menu_width};
pub use progress::{capsule_bar, segment_bar};
pub use segmented::{segmented, segmented_width};
pub use spinner::spinner;
pub use switch::switch;
pub use wheel::time_wheels;

use crate::icons::{self, Icon};
use crate::theme::{self, Palette};
use eframe::egui::{self, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke};

/// The tool tile of the toolbar: an icon over a label. Hover shows a soft wash.
pub fn tool_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool, tip: &str) -> bool {
    let size = vec2(76.0, TOOL_TILE_H);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let hover = anim::fade(&ctx, resp.id.with("hover"), enabled && resp.hovered(), 0.12);
    let press = anim::fade(&ctx, resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    // Pressed tiles shrink to 97%.
    let rect = rect.shrink2(rect.size() * (0.015 * press));
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md), pal.hover.gamma_multiply(hover).lerp_to_gamma(pal.press, press));
    icons::draw(p, Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 24.0), vec2(24.0, 24.0)), icon, icons::tint(icon, &pal), enabled);
    p.text(Pos2::new(rect.center().x, rect.bottom() - 13.0), Align2::CENTER_CENTER, label, FontId::new(d.caption, theme::semibold()), if enabled { pal.secondary } else { pal.disabled });
    let clicked = resp.clicked();
    resp.on_hover_text(tip);
    clicked && enabled
}

/// The toolbar tile without its label, for medium-width windows (the tooltip says what it does).
pub fn tool_button_icon(ui: &mut egui::Ui, icon: Icon, enabled: bool, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(44.0, TOOL_TILE_ICON_H), if enabled { Sense::click() } else { Sense::hover() });
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let hover = anim::fade(&ctx, resp.id.with("hover"), enabled && resp.hovered(), 0.12);
    let press = anim::fade(&ctx, resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    let rect = rect.shrink2(rect.size() * (0.02 * press));
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md), pal.hover.gamma_multiply(hover).lerp_to_gamma(pal.press, press));
    icons::draw(p, Rect::from_center_size(rect.center(), vec2(24.0, 24.0)), icon, icons::tint(icon, &pal), enabled);
    let clicked = resp.clicked();
    resp.on_hover_text(tip);
    clicked && enabled
}

/// Height of a labelled toolbar tile, and of an icon-only one.
pub const TOOL_TILE_H: f32 = 64.0;
pub const TOOL_TILE_ICON_H: f32 = 44.0;

/// A thin vertical line between groups of tools. It is exactly as tall as the tiles beside it: a
/// taller one would make the row grow after the first tiles were placed, and leave them off-centre.
pub fn toolbar_sep(ui: &mut egui::Ui, height: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(13.0, height), Sense::hover());
    let col = theme::hairline(ui.ctx());
    let inset = (height * 0.25).round();
    ui.painter().line_segment([Pos2::new(r.center().x, r.top() + inset), Pos2::new(r.center().x, r.bottom() - inset)], Stroke::new(1.0, col));
    resp
}

/// What a sidebar row shows in front of its label: an icon in a colour.
#[derive(Clone, Copy, PartialEq)]
pub struct RowIcon {
    pub icon: Icon,
    pub color: Color32,
}

fn pill_target_id() -> egui::Id {
    egui::Id::unique("sdm_sidebar_pill_target")
}

/// One row of the sidebar. The selection pill is not drawn here: the selected row only reports
/// where it is, and `sidebar_pill` slides one pill to it.
pub fn sidebar_row(ui: &mut egui::Ui, label: &str, count: usize, indent: f32, icon: RowIcon, selected: bool) -> egui::Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, d.nav_h), Sense::click());
    if selected {
        ctx.data_mut(|data| data.insert_temp(pill_target_id(), (rect.top(), rect.left(), rect.width(), d.nav_h)));
    }
    let hover = anim::fade(&ctx, resp.id.with("hover"), resp.hovered() && !selected, 0.12);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md), pal.hover.gamma_multiply(hover));
    let ic = Pos2::new(rect.left() + indent + 24.0, rect.center().y);
    icons::draw_weight(p, Rect::from_center_size(ic, vec2(20.0, 20.0)), icon.icon, icon.color, true, selected);
    let (col, family) = if selected { (pal.label, theme::semibold()) } else { (pal.secondary.lerp_to_gamma(pal.label, hover), egui::FontFamily::Proportional) };
    p.text(Pos2::new(rect.left() + indent + 44.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::new(d.body, family), col);
    if count > 0 {
        p.text(Pos2::new(rect.right() - 14.0, rect.center().y), Align2::RIGHT_CENTER, count.to_string(), FontId::new(d.caption, theme::semibold()), pal.tertiary);
    }
    resp
}

/// A sidebar row in the narrow icon rail: just the icon, with the name and count in a tooltip.
pub fn rail_row(ui: &mut egui::Ui, label: &str, count: usize, icon: RowIcon, selected: bool) -> egui::Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), d.nav_h + 4.0), Sense::click());
    if selected {
        ctx.data_mut(|data| data.insert_temp(pill_target_id(), (rect.top(), rect.left(), rect.width(), d.nav_h + 4.0)));
    }
    let hover = anim::fade(&ctx, resp.id.with("hover"), resp.hovered() && !selected, 0.12);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md), pal.hover.gamma_multiply(hover));
    icons::draw_weight(p, Rect::from_center_size(rect.center(), vec2(22.0, 22.0)), icon.icon, icon.color, true, selected);
    resp.on_hover_text(if count > 0 { format!("{label} ({count})") } else { label.to_string() })
}

/// Paints the selection pill behind the sidebar rows. Call it first, before the rows; it
/// glides to wherever the selected row was last frame.
pub fn sidebar_pill(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let Some((top, left, width, height)) = ctx.data(|d| d.get_temp::<(f32, f32, f32, f32)>(pill_target_id())) else { return };
    let y = anim::glide(&ctx, egui::Id::unique("sdm_sidebar_pill_y"), top, 0.18);
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    ui.painter().rect_filled(Rect::from_min_size(Pos2::new(left, y), vec2(width, height)), CornerRadius::same(d.radius_md), pal.surface2);
}

/// A small status-bar entry: optional coloured dot, then caption text. Clickable. It paints
/// its own layout, so it reads the same in left-to-right and right-to-left rows.
pub fn status_chip(ui: &mut egui::Ui, dot: Option<Color32>, text: &str, col: Color32) -> egui::Response {
    let font = FontId::proportional(theme::design(ui.ctx()).caption);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER);
    let dot_w = if dot.is_some() { 14.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(dot_w + galley.size().x, 20.0), Sense::click());
    if let Some(d) = dot {
        ui.painter().circle_filled(Pos2::new(rect.left() + 3.5, rect.center().y), 3.5, d);
    }
    ui.painter().galley(Pos2::new(rect.left() + dot_w, rect.center().y - galley.size().y / 2.0), galley, col);
    resp
}

/// What a state chip is about: decides its colours.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hue {
    Accent,
    Green,
    Orange,
    Red,
    Neutral,
}

impl Hue {
    /// (fill, dot, ink) of a chip of this hue.
    pub fn colors(self, pal: &Palette) -> (Color32, Color32, Color32) {
        match self {
            Hue::Accent => (pal.accent_tint, pal.accent, pal.accent_ink),
            Hue::Green => (pal.green_tint, pal.green, pal.green_ink),
            Hue::Orange => (pal.orange_tint, pal.orange, pal.orange_ink),
            Hue::Red => (pal.red_tint, pal.red, pal.red_ink),
            Hue::Neutral => (pal.surface2, pal.tertiary, pal.secondary),
        }
    }
}

/// Width of a state chip showing `text`.
fn chip_width(galley_w: f32) -> f32 {
    12.0 + 6.0 + 6.0 + galley_w + 12.0
}

/// Paints a state chip with its top-right corner at `right_top` and returns its width.
pub fn paint_state_chip(p: &egui::Painter, right_top: Pos2, pal: &Palette, d: &theme::Design, hue: Hue, text: &str) -> f32 {
    let (fill, dot, ink) = hue.colors(pal);
    let galley = p.layout_no_wrap(text.to_owned(), FontId::new(d.caption, theme::semibold()), Color32::PLACEHOLDER);
    let w = chip_width(galley.size().x);
    let rect = Rect::from_min_size(Pos2::new(right_top.x - w, right_top.y), vec2(w, 24.0));
    p.rect_filled(rect, CornerRadius::same(12), fill);
    p.circle_filled(Pos2::new(rect.left() + 12.0 + 3.0, rect.center().y), 3.0, dot);
    p.galley(Pos2::new(rect.left() + 12.0 + 6.0 + 6.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    w
}

/// A state chip: a 24 px capsule with a tinted fill, a dot in the hue and the word in its ink
/// colour. Status is always a word first and a colour second.
pub fn state_chip(ui: &mut egui::Ui, hue: Hue, text: &str) -> egui::Response {
    let d = theme::design(ui.ctx());
    let pal = theme::current(ui.ctx());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::new(d.caption, theme::semibold()), Color32::PLACEHOLDER);
    let (rect, resp) = ui.allocate_exact_size(vec2(chip_width(galley.size().x), 24.0), Sense::hover());
    let (fill, dot, ink) = hue.colors(&pal);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(12), fill);
    p.circle_filled(Pos2::new(rect.left() + 12.0 + 3.0, rect.center().y), 3.0, dot);
    p.galley(Pos2::new(rect.left() + 12.0 + 6.0 + 6.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::themed;
    use crate::theme::ThemeMode;

    #[test]
    fn a_painted_chip_and_a_laid_out_chip_agree_on_their_width() {
        let ctx = themed(ThemeMode::Light);
        let pal = Palette::LIGHT;
        let mut painted = 0.0;
        let mut laid = 0.0;
        for _ in 0..2 {
            let mut out = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    painted = paint_state_chip(ui.painter(), Pos2::new(300.0, 10.0), &pal, &theme::DESIGN, Hue::Green, "Downloading");
                    laid = state_chip(ui, Hue::Green, "Downloading").rect.width();
                });
            });
            out.textures_delta.clear();
        }
        assert!((painted - laid).abs() < 0.01, "{painted} vs {laid}");
    }

    #[test]
    fn every_hue_has_readable_ink_on_its_own_fill() {
        use crate::theme::palette::{contrast_ratio, over};
        for (mode, pal) in [("light", Palette::LIGHT), ("dark", Palette::DARK)] {
            for hue in [Hue::Accent, Hue::Green, Hue::Orange, Hue::Red, Hue::Neutral] {
                let (fill, _, ink) = hue.colors(&pal);
                let c = contrast_ratio(ink, over(fill, pal.card));
                assert!(c >= 4.5, "{hue:?} in {mode} is only {c:.2}:1");
            }
        }
    }

    /// Lays out a toolbar the way the main window does (tiles in groups with separators between them) and
    /// returns the vertical centre of every icon that was painted.
    fn toolbar_icon_ys(labelled: bool, separator_height: f32) -> Vec<f32> {
        let ctx = themed(ThemeMode::Light);
        let mut ys = Vec::new();
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 200.0))), ..Default::default() }, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for group in 0..3 {
                            if group > 0 {
                                toolbar_sep(ui, separator_height);
                            }
                            for _ in 0..2 {
                                if labelled {
                                    tool_button(ui, Icon::Options, "Options", true, "tip");
                                } else {
                                    tool_button_icon(ui, Icon::Options, true, "tip");
                                }
                            }
                        }
                    });
                });
            });
            out.textures_delta.clear();
            ys.clear();
            for s in &out.shapes {
                // Icons are single private-use characters of the icon font.
                if let egui::Shape::Text(t) = &s.shape {
                    let text = t.galley.text();
                    if text.chars().count() == 1 && ('\u{e000}'..='\u{f8ff}').contains(&text.chars().next().unwrap()) {
                        ys.push(t.visual_bounding_rect().center().y);
                    }
                }
            }
        }
        ys
    }

    #[test]
    fn the_icons_of_the_toolbar_share_one_centre_line() {
        // The first two icons of the narrow toolbar used to sit 10 points above the rest: the separators
        // were taller than the icon tiles, the row grew after they had been placed, and egui never re-centres.
        for (labelled, tile) in [(true, TOOL_TILE_H), (false, TOOL_TILE_ICON_H)] {
            let ys = toolbar_icon_ys(labelled, tile);
            assert_eq!(ys.len(), 6, "labelled={labelled}");
            assert!(ys.iter().all(|y| (y - ys[0]).abs() < 0.6), "labelled={labelled}: {ys:?}");
        }
    }

    #[test]
    fn the_centre_line_test_really_fails_with_separators_of_the_wrong_height() {
        // The old toolbar: 44 point tiles and 64 point separators. If this stopped being off-centre, the test above would prove nothing.
        let ys = toolbar_icon_ys(false, TOOL_TILE_H);
        assert_eq!(ys.len(), 6);
        assert!(ys.iter().any(|y| (y - ys[0]).abs() > 1.0), "{ys:?}");
    }

    #[test]
    fn a_separator_is_exactly_as_tall_as_the_tiles_it_stands_between() {
        let ctx = themed(ThemeMode::Light);
        for (height, label) in [(TOOL_TILE_H, "labelled"), (TOOL_TILE_ICON_H, "icon-only")] {
            let mut size = vec2(0.0, 0.0);
            let mut out = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    size = toolbar_sep(ui, height).rect.size();
                });
            });
            out.textures_delta.clear();
            assert_eq!(size.y, height, "{label}");
        }
    }
}
