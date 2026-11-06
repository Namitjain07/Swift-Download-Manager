//! Small reusable components: round icon buttons, an icon-only segmented control, summary tiles
//! and the empty state.

use super::anim::fade;
use super::{button, ButtonKind};
use crate::icons::{self, Icon};
use crate::theme::{self, Elevation};
use eframe::egui::{self, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

/// Height of a summary tile.
pub const STAT_TILE_H: f32 = 84.0;

/// A round button with an icon. Hover shows a soft grey disc; the tooltip says what it does.
pub fn icon_button(ui: &mut Ui, icon: Icon, tip: &str) -> Response {
    let side = theme::design(ui.ctx()).icon_btn;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), if ui.is_enabled() { Sense::click() } else { Sense::hover() });
    paint_icon_button(ui, rect, &resp, icon, None);
    resp.on_hover_text(tip)
}

/// The main action as a round coral button with a white icon: the narrow-window form of the "Add download" capsule.
pub fn icon_button_primary(ui: &mut Ui, icon: Icon, tip: &str) -> Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let enabled = ui.is_enabled();
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(d.control_h), if enabled { Sense::click() } else { Sense::hover() });
    let hover = fade(&ctx, resp.id.with("hover"), enabled && resp.hovered(), 0.12);
    let press = fade(&ctx, resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    let side = rect.width() * (1.0 - 0.03 * press);
    let p = ui.painter();
    p.rect_filled(Rect::from_center_size(rect.center(), Vec2::splat(side)), CornerRadius::same(d.round(side)), pal.accent_solid.lerp_to_gamma(pal.accent_press, hover.max(press)));
    icons::draw_weight(p, Rect::from_center_size(rect.center(), Vec2::splat(22.0)), icon, pal.on_accent, enabled, false);
    resp.on_hover_text(tip)
}

/// The same button at an exact place (used inside cards that lay themselves out).
pub fn icon_button_at(ui: &mut Ui, rect: Rect, icon: Icon, tip: &str, id: egui::Id) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    paint_icon_button(ui, rect, &resp, icon, None);
    resp.on_hover_text(tip)
}

fn paint_icon_button(ui: &Ui, rect: Rect, resp: &Response, icon: Icon, tint: Option<Color32>) {
    let ctx = ui.ctx();
    let pal = theme::current(ctx);
    let enabled = ui.is_enabled();
    let hover = fade(ctx, resp.id.with("hover"), enabled && resp.hovered(), 0.12);
    let press = fade(ctx, resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    let c = rect.center();
    let side = rect.width().min(rect.height()) * (1.0 - 0.04 * press);
    let d = theme::design(ctx);
    let p = ui.painter();
    p.rect_filled(Rect::from_center_size(c, Vec2::splat(side)), CornerRadius::same(d.round(side)), pal.hover.gamma_multiply(hover).lerp_to_gamma(pal.press, press));
    icons::draw(p, Rect::from_center_size(c, Vec2::splat(20.0)), icon, tint.unwrap_or(pal.secondary.lerp_to_gamma(pal.label, hover)), enabled);
}

/// Two or three icon buttons in one rounded track, one of them selected (cards or table).
pub fn icon_segmented<T: PartialEq + Copy>(ui: &mut Ui, options: &[(T, Icon, &str)], value: &mut T) -> Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let item = Vec2::new(40.0, d.control_h - 8.0);
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(item.x * options.len() as f32 + 8.0, d.control_h), Sense::click());
    let inner = rect.shrink(4.0);
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let i = (((pos.x - inner.left()) / item.x).floor().max(0.0) as usize).min(options.len().saturating_sub(1));
        if options[i].0 != *value {
            *value = options[i].0;
            resp.mark_changed();
        }
    }
    let selected = options.iter().position(|(v, _, _)| v == value).unwrap_or(0);
    let x = super::anim::glide(&ctx, resp.id.with("thumb"), inner.left() + item.x * selected as f32, 0.16);
    let p = ui.painter();
    let full = CornerRadius::same(d.round(d.control_h));
    p.rect_filled(rect, full, pal.surface2);
    let thumb = Rect::from_min_size(egui::pos2(x, inner.top()), item);
    p.add(theme::shadow(&ctx, Elevation::Rest).as_shape(thumb, CornerRadius::same(d.round(item.y))));
    p.rect_filled(thumb, CornerRadius::same(d.round(item.y)), pal.segment_on);
    for (i, (_, icon, _)) in options.iter().enumerate() {
        let c = Pos2::new(inner.left() + item.x * (i as f32 + 0.5), inner.center().y);
        let tint = if i == selected { pal.label } else { pal.tertiary };
        icons::draw_weight(p, Rect::from_center_size(c, Vec2::splat(18.0)), *icon, tint, true, i == selected);
    }
    let tip = options[selected].2;
    resp.on_hover_text(tip)
}

/// A card that holds one number: a caption, the number big and its unit, and a round icon in the
/// `accent` colour.
pub fn stat_tile(ui: &mut Ui, width: f32, icon: Icon, label: &str, value: &str, unit: &str, accent: Color32) -> Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, STAT_TILE_H), Sense::hover());
    let p = ui.painter();
    let radius = CornerRadius::same(d.radius_lg);
    p.add(theme::shadow(&ctx, Elevation::Rest).as_shape(rect, radius));
    p.rect_filled(rect, radius, pal.card);
    p.rect_stroke(rect, radius, Stroke::new(1.0, pal.separator), StrokeKind::Inside);
    p.text(rect.left_top() + vec2(20.0, 16.0), Align2::LEFT_TOP, label, FontId::new(d.caption, theme::semibold()), pal.tertiary);
    let metric = p.layout_no_wrap(value.to_owned(), FontId::new(28.0, theme::semibold()), Color32::PLACEHOLDER);
    let size = metric.size();
    let at = rect.left_bottom() + vec2(20.0, -14.0 - size.y);
    p.galley(at, metric, pal.label);
    p.text(Pos2::new(at.x + size.x + 6.0, at.y + size.y - 5.0), Align2::LEFT_BOTTOM, unit, FontId::proportional(d.caption), pal.secondary);
    // A round icon top right.
    let c = Pos2::new(rect.right() - 20.0 - 16.0, rect.top() + 18.0 + 16.0);
    p.circle_filled(c, 16.0, accent.gamma_multiply(0.12));
    icons::draw(p, Rect::from_center_size(c, Vec2::splat(18.0)), icon, accent, true);
    resp
}

/// The picture above the words of an empty state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Art {
    /// A glyph on a round tint.
    Icon(Icon),
    /// Streams running into one file, quietly, as an invitation to add a download.
    Flow,
}

/// What the idle picture shows: five streams of different pace.
const IDLE_CHANNELS: [super::flow::Channel; 5] = [
    super::flow::Channel { rate: 0.9, done: false },
    super::flow::Channel { rate: 0.4, done: false },
    super::flow::Channel { rate: 0.7, done: false },
    super::flow::Channel { rate: 0.2, done: false },
    super::flow::Channel { rate: 0.55, done: false },
];

/// Shown instead of a list when there is nothing to list: a picture, a title, one sentence, and
/// at most one primary action. Returns true when that action was pressed.
pub fn empty_state(ui: &mut Ui, art: Art, title: &str, text: &str, action: Option<&str>) -> bool {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let mut pressed = false;
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() * 0.18).clamp(12.0, 90.0));
        match art {
            Art::Icon(icon) => {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(88.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 44.0, pal.accent_tint);
                icons::draw(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(40.0)), icon, pal.accent_ink, true);
            }
            Art::Flow => {
                ui.scope(|ui| {
                    ui.set_max_width(340.0);
                    super::flow::flow(ui, "empty", &IDLE_CHANNELS, 0.0, super::flow::State::Running, 124.0);
                });
            }
        }
        ui.add_space(12.0);
        ui.label(RichText::new(title).size(d.title + 3.0).family(theme::semibold()).color(pal.label));
        ui.add_space(2.0);
        ui.scope(|ui| {
            ui.set_max_width(380.0);
            ui.add(egui::Label::new(RichText::new(text).size(d.body).color(pal.secondary)).wrap());
        });
        if let Some(label) = action {
            ui.add_space(16.0);
            pressed = button(ui, ButtonKind::Primary, label).clicked();
        }
    });
    pressed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::themed;
    use crate::theme::ThemeMode;

    #[test]
    fn icon_buttons_are_square_and_the_segmented_control_is_a_capsule() {
        let ctx = themed(ThemeMode::Light);
        let mut v = 0u8;
        let (mut a, mut b) = (Rect::NOTHING, Rect::NOTHING);
        for _ in 0..2 {
            let mut out = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        a = icon_button(ui, Icon::Menu, "Menu").rect;
                        b = icon_segmented(ui, &[(0u8, Icon::Cards, "Cards"), (1, Icon::List, "List")], &mut v).rect;
                    });
                });
            });
            out.textures_delta.clear();
        }
        assert_eq!(a.size(), Vec2::splat(theme::DESIGN.icon_btn));
        assert_eq!(b.height(), theme::DESIGN.control_h, "the segmented control is as tall as a button");
        assert!((a.center().y - b.center().y).abs() < 0.01, "and they share a centre line");
    }

    #[test]
    fn a_stat_tile_is_exactly_as_wide_as_asked_and_a_fixed_height() {
        let ctx = themed(ThemeMode::Light);
        let mut rect = Rect::NOTHING;
        for _ in 0..2 {
            let mut out = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    rect = stat_tile(ui, 220.0, Icon::Speed, "Speed", "6.46", "MB/s", Color32::RED).rect;
                });
            });
            out.textures_delta.clear();
        }
        assert_eq!(rect.size(), vec2(220.0, STAT_TILE_H));
    }
}
