//! Buttons, in the iOS manner: fully round capsules that fade on hover and shrink a little while
//! pressed. The label is semibold, an optional icon sits in front of it, and every button of a
//! size has the same height, so a row of buttons always lines up.

use super::anim::fade;
use crate::icons::{self, Icon};
use crate::theme;
use eframe::egui::{self, vec2, Color32, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, Ui};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonKind {
    /// Solid coral, white text: the main action. One per view.
    Primary,
    /// A quiet grey fill that darkens on hover: the usual action.
    Secondary,
    /// A thin outline, no fill.
    Outline,
    /// Text only: the quietest, for Cancel and the like.
    Plain,
    /// A soft red wash for removing things.
    Danger,
}

/// Which of the two heights a button has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonSize {
    Regular,
    Small,
}

/// A text button.
pub fn button(ui: &mut Ui, kind: ButtonKind, label: &str) -> Response {
    paint_button(ui, kind, ButtonSize::Regular, None, label)
}

/// A small text button (inside rows, next to fields).
pub fn small_button(ui: &mut Ui, kind: ButtonKind, label: &str) -> Response {
    paint_button(ui, kind, ButtonSize::Small, None, label)
}

/// A button with an icon in front of its label.
pub fn icon_label_button(ui: &mut Ui, kind: ButtonKind, icon: Icon, label: &str) -> Response {
    paint_button(ui, kind, ButtonSize::Regular, Some(icon), label)
}

/// A small button with an icon in front of its label.
pub fn small_icon_label_button(ui: &mut Ui, kind: ButtonKind, icon: Icon, label: &str) -> Response {
    paint_button(ui, kind, ButtonSize::Small, Some(icon), label)
}

/// The width a button will have, so rows can reserve room for it.
pub fn button_width(ui: &Ui, size: ButtonSize, icon: bool, label: &str) -> f32 {
    let (font, pad, icon_w) = metrics(&theme::design(ui.ctx()), size);
    let text = ui.painter().layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER).size().x;
    2.0 * pad + text + if icon { icon_w + GAP } else { 0.0 }
}

const GAP: f32 = 8.0;

fn metrics(d: &theme::Design, size: ButtonSize) -> (FontId, f32, f32) {
    // The side padding is wide for capsules and narrower for the rectangular buttons of Files and Carbon.
    match size {
        ButtonSize::Regular => (FontId::new(d.body, theme::semibold()), 12.0 + 8.0 * d.pill, 18.0),
        ButtonSize::Small => (FontId::new(13.0, theme::semibold()), 10.0 + 4.0 * d.pill, 16.0),
    }
}

/// (fill, text, outline) of a disabled button.
fn disabled_look(kind: ButtonKind, pal: &theme::Palette) -> (Color32, Color32, Stroke) {
    match kind {
        ButtonKind::Outline => (Color32::TRANSPARENT, pal.disabled, Stroke::new(1.0, pal.separator_strong)),
        ButtonKind::Plain => (Color32::TRANSPARENT, pal.disabled, Stroke::NONE),
        ButtonKind::Primary | ButtonKind::Secondary | ButtonKind::Danger => (pal.surface2, pal.disabled, Stroke::NONE),
    }
}

fn paint_button(ui: &mut Ui, kind: ButtonKind, size: ButtonSize, icon: Option<Icon>, label: &str) -> Response {
    let enabled = ui.is_enabled();
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let (font, pad, icon_w) = metrics(&d, size);
    let height = if size == ButtonSize::Small { d.small_h } else { d.control_h };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER);
    let content_w = galley.size().x + if icon.is_some() { icon_w + GAP } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(2.0 * pad + content_w, height), if enabled { Sense::click() } else { Sense::hover() });
    let hover = fade(&ctx, resp.id.with("hover"), enabled && resp.hovered(), 0.12);
    let press = fade(&ctx, resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    // Pressed buttons shrink to 97%; only the painting changes, never the layout.
    let rect = rect.shrink2(rect.size() * (0.015 * press));

    let (fill, text, stroke) = match kind {
        ButtonKind::Primary => (pal.accent_solid.lerp_to_gamma(pal.accent_press, hover.max(press)), pal.on_accent, Stroke::NONE),
        ButtonKind::Secondary => (pal.surface2.lerp_to_gamma(pal.surface3, hover.max(press)), pal.label, Stroke::NONE),
        ButtonKind::Outline => {
            (Color32::TRANSPARENT.lerp_to_gamma(pal.hover, hover.max(press)), pal.label, Stroke::new(1.0, pal.control_border.lerp_to_gamma(pal.label, hover)))
        }
        ButtonKind::Plain => (Color32::TRANSPARENT.lerp_to_gamma(pal.hover, hover.max(press)), pal.label, Stroke::NONE),
        ButtonKind::Danger => (pal.red_tint.gamma_multiply(1.0 + 0.6 * hover.max(press)), pal.red_ink, Stroke::NONE),
    };
    // Disabled buttons are flat grey with grey text (never a faded copy of the coloured one, which would be unreadable).
    let (fill, text, stroke) = if enabled { (fill, text, stroke) } else { disabled_look(kind, &pal) };
    let dim = 1.0;
    let radius = CornerRadius::same(d.round(rect.height()));
    let p = ui.painter();
    if fill != Color32::TRANSPARENT {
        p.rect_filled(rect, radius, fill.gamma_multiply(dim));
    }
    if stroke != Stroke::NONE {
        p.rect_stroke(rect, radius, Stroke::new(stroke.width, stroke.color.gamma_multiply(dim)), StrokeKind::Inside);
    }
    // Icon and label are centred together as one block.
    let mut x = rect.center().x - content_w / 2.0;
    if let Some(icon) = icon {
        icons::draw_weight(p, egui::Rect::from_min_size(egui::pos2(x, rect.center().y - icon_w / 2.0), vec2(icon_w, icon_w)), icon, text, enabled, false);
        x += icon_w + GAP;
    }
    p.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, text.gamma_multiply(dim));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::themed;
    use crate::theme::ThemeMode;
    use eframe::egui::Rect;

    /// Lay out one button of each kind and report the rectangles it took.
    fn rects(width: f32, build: impl Fn(&mut Ui) -> Vec<Response>) -> Vec<Rect> {
        let ctx = themed(ThemeMode::Light);
        let mut out = Vec::new();
        for _ in 0..2 {
            let mut frame = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(width, 200.0))), ..Default::default() }, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        out = build(ui).iter().map(|r| r.rect).collect();
                    });
                });
            });
            frame.textures_delta.clear();
        }
        out
    }

    #[test]
    fn buttons_of_one_size_share_a_height_whatever_their_kind_or_icon() {
        let r = rects(900.0, |ui| {
            vec![
                button(ui, ButtonKind::Primary, "Start Download"),
                button(ui, ButtonKind::Secondary, "Later"),
                button(ui, ButtonKind::Outline, "Browse…"),
                button(ui, ButtonKind::Plain, "Cancel"),
                icon_label_button(ui, ButtonKind::Danger, Icon::Delete, "Remove"),
            ]
        });
        assert_eq!(r.len(), 5);
        for rect in &r {
            assert_eq!(rect.height(), theme::DESIGN.control_h, "{rect:?}");
        }
        // They sit on one line: every centre has the same y.
        for rect in &r {
            assert!((rect.center().y - r[0].center().y).abs() < 0.01, "{rect:?} is off the line");
        }
    }

    #[test]
    fn small_buttons_are_shorter_and_an_icon_makes_a_button_wider() {
        let r = rects(900.0, |ui| vec![small_button(ui, ButtonKind::Secondary, "Copy"), small_icon_label_button(ui, ButtonKind::Secondary, Icon::Copy, "Copy"), button(ui, ButtonKind::Secondary, "Copy")]);
        assert_eq!(r[0].height(), theme::DESIGN.small_h);
        assert!(r[1].width() > r[0].width(), "the icon needs room");
        assert!(r[2].height() > r[0].height());
    }

    #[test]
    fn button_width_matches_what_is_laid_out() {
        let ctx = themed(ThemeMode::Light);
        let mut predicted = 0.0;
        let mut actual = 0.0;
        for _ in 0..2 {
            let mut frame = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    predicted = button_width(ui, ButtonSize::Regular, true, "Open folder");
                    actual = icon_label_button(ui, ButtonKind::Secondary, Icon::Folder, "Open folder").rect.width();
                });
            });
            frame.textures_delta.clear();
        }
        assert!((predicted - actual).abs() < 0.01, "{predicted} vs {actual}");
    }

    #[test]
    fn layout_is_stable_from_frame_to_frame() {
        // Hover and press only change the painting, never the room a button takes.
        let a = rects(600.0, |ui| vec![button(ui, ButtonKind::Primary, "OK"), button(ui, ButtonKind::Secondary, "Next")]);
        let b = rects(600.0, |ui| vec![button(ui, ButtonKind::Primary, "OK"), button(ui, ButtonKind::Secondary, "Next")]);
        assert_eq!(a, b);
    }
}
