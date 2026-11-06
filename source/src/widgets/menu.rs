//! Menus: the rows inside a menu or a drop-down, painted by hand so that every row has the same
//! height, the label always starts at the same x and shortcuts, check marks and arrows always end
//! at the same x. The hover highlight is a rounded wash that fills the whole row.

use super::anim::fade;
use crate::icons::{self, Icon};
use crate::theme::{self, Elevation};
use eframe::egui::{self, vec2, Align2, CornerRadius, FontId, Frame, InnerResponse, Margin, Pos2, Rect, Response, Sense, Stroke, Ui};

/// Width of the rows of a menu.
pub const MENU_ROW_W: f32 = 296.0;

/// What a menu row shows at its right end.
#[derive(Clone, Copy)]
enum Trailing<'a> {
    Nothing,
    Shortcut(&'a str),
    Check(bool),
    Arrow,
}

/// The frame of a floating menu: a raised white (or charcoal) sheet with 16 px corners and a soft shadow.
pub fn menu_surface(ctx: &egui::Context) -> Frame {
    let (pal, d) = (theme::current(ctx), theme::design(ctx));
    Frame::NONE
        .fill(pal.raised)
        .corner_radius(CornerRadius::same(d.radius_lg))
        .stroke(Stroke::new(1.0, pal.separator_strong))
        .shadow(theme::shadow(ctx, Elevation::Popup))
        .inner_margin(Margin::same(8))
}

/// Fixes the width of a menu so all of its rows agree. Call it first inside the menu.
pub fn menu_width(ui: &mut Ui) {
    ui.set_width(MENU_ROW_W);
    ui.spacing_mut().item_spacing.y = 2.0;
}

/// One row: a label, and the shortcut written at its right end.
pub fn menu_item(ui: &mut Ui, label: &str, shortcut: &str, enabled: bool) -> Response {
    row(ui, None, label, Trailing::Shortcut(shortcut), enabled, false)
}

/// A row with an icon in front of its label.
pub fn menu_icon_item(ui: &mut Ui, icon: Icon, label: &str, shortcut: &str, enabled: bool) -> Response {
    row(ui, Some(icon), label, Trailing::Shortcut(shortcut), enabled, false)
}

/// A row for a destructive action: the label is red.
pub fn menu_danger_item(ui: &mut Ui, label: &str, enabled: bool) -> Response {
    row(ui, None, label, Trailing::Nothing, enabled, true)
}

/// A row of a list of options. The chosen one has a check mark at its right end.
pub fn menu_check_row(ui: &mut Ui, label: &str, checked: bool) -> Response {
    row(ui, None, label, Trailing::Check(checked), true, false)
}

/// A row that switches something on and off; a check mark shows when it is on.
pub fn menu_toggle(ui: &mut Ui, label: &str, on: &mut bool) -> Response {
    let mut r = row(ui, None, label, Trailing::Check(*on), true, false);
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    r
}

/// A row that opens a submenu at its right.
pub fn menu_submenu<R>(ui: &mut Ui, label: &str, enabled: bool, content: impl FnOnce(&mut Ui) -> R) -> Option<InnerResponse<R>> {
    let resp = row(ui, None, label, Trailing::Arrow, enabled, false);
    egui::containers::menu::SubMenu::new().show(ui, &resp, |ui| {
        menu_width(ui);
        content(ui)
    })
}

/// A hairline between groups of rows.
pub fn menu_separator(ui: &mut Ui) {
    let pal = theme::current(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline((rect.left() + 4.0)..=(rect.right() - 4.0), rect.center().y, Stroke::new(1.0, pal.separator));
}

/// A small grey caption above a group of rows.
pub fn menu_caption(ui: &mut Ui, text: &str) {
    let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    ui.painter().text(Pos2::new(rect.left() + 12.0, rect.bottom() - 6.0), Align2::LEFT_BOTTOM, text, FontId::new(d.caption, theme::semibold()), pal.tertiary);
}

fn row(ui: &mut Ui, icon: Option<Icon>, label: &str, trailing: Trailing, enabled: bool, danger: bool) -> Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), d.menu_row_h), if enabled { Sense::click() } else { Sense::hover() });
    let hover = fade(&ctx, resp.id.with("hover"), enabled && (resp.hovered() || resp.has_focus()), 0.1);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md.min(10)), pal.hover.gamma_multiply(hover));
    let ink = if !enabled {
        pal.disabled
    } else if danger {
        pal.red_ink
    } else {
        pal.label
    };
    let mut x = rect.left() + 12.0;
    if let Some(icon) = icon {
        icons::draw(p, Rect::from_center_size(Pos2::new(x + 10.0, rect.center().y), vec2(20.0, 20.0)), icon, if enabled { pal.secondary } else { pal.disabled }, enabled);
        x += 32.0;
    }
    p.text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(d.body), ink);
    let end = rect.right() - 12.0;
    match trailing {
        Trailing::Nothing => {}
        Trailing::Shortcut(s) if !s.is_empty() => {
            p.text(Pos2::new(end, rect.center().y), Align2::RIGHT_CENTER, s, FontId::proportional(d.caption), if enabled { pal.tertiary } else { pal.disabled });
        }
        Trailing::Shortcut(_) => {}
        Trailing::Check(on) => {
            if on {
                icons::draw_weight(p, Rect::from_center_size(Pos2::new(end - 8.0, rect.center().y), vec2(18.0, 18.0)), Icon::Check, pal.accent_ink, enabled, false);
            }
        }
        Trailing::Arrow => {
            icons::draw(p, Rect::from_center_size(Pos2::new(end - 7.0, rect.center().y), vec2(16.0, 16.0)), Icon::CaretRight, if enabled { pal.tertiary } else { pal.disabled }, enabled);
        }
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::themed;
    use crate::theme::ThemeMode;

    #[test]
    fn rows_of_every_kind_have_the_same_height_and_width() {
        let ctx = themed(ThemeMode::Light);
        let mut on = true;
        let mut rects: Vec<Rect> = Vec::new();
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(600.0, 600.0))), ..Default::default() }, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.vertical(|ui| {
                        menu_width(ui);
                        rects = vec![
                            menu_item(ui, "Add new download…", "Ctrl+N", true).rect,
                            menu_icon_item(ui, Icon::Folder, "Open folder", "", true).rect,
                            menu_check_row(ui, "Dark", true).rect,
                            menu_toggle(ui, "Show categories", &mut on).rect,
                            menu_danger_item(ui, "Remove", true).rect,
                            menu_item(ui, "Disabled", "", false).rect,
                        ];
                    });
                });
            });
            out.textures_delta.clear();
        }
        for r in &rects {
            assert_eq!(r.height(), theme::DESIGN.menu_row_h, "{r:?}");
            assert_eq!(r.width(), MENU_ROW_W, "{r:?}");
            assert_eq!(r.left(), rects[0].left(), "rows start at the same x");
        }
        // Rows stack with the 2 px gap and nothing else between them.
        for pair in rects.windows(2) {
            assert!((pair[1].top() - pair[0].bottom() - 2.0).abs() < 0.01, "{pair:?}");
        }
    }

    #[test]
    fn a_toggle_row_flips_its_value_only_when_clicked() {
        let ctx = themed(ThemeMode::Light);
        let mut on = false;
        let mut out = ctx.run_ui(Default::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                menu_toggle(ui, "Show categories", &mut on);
            });
        });
        out.textures_delta.clear();
        assert!(!on, "just drawing it changes nothing");
    }
}
