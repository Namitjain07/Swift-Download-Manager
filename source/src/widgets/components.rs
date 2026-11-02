//! Small reusable components: icon buttons, an icon-only segmented control, stat tiles and the
//! empty state. They read the active design, so they work in Drishti and iOS, light and dark.

use super::anim::fade;
use super::{button, ButtonKind};
use crate::icons::{self, Icon};
use crate::theme;
use eframe::egui::{self, Align2, CornerRadius, FontId, Rect, Response, RichText, Sense, Ui, Vec2};

/// A square button with a glyph. Hover shows a soft fill; the tooltip says what it does.
pub fn icon_button(ui: &mut Ui, icon: Icon, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), if ui.is_enabled() { Sense::click() } else { Sense::hover() });
    paint_icon_button(ui, rect, &resp, icon);
    resp.on_hover_text(tip)
}

/// The same button at an exact place (used inside cards that lay themselves out).
pub fn icon_button_at(ui: &mut Ui, rect: Rect, icon: Icon, tip: &str, id: egui::Id) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    paint_icon_button(ui, rect, &resp, icon);
    resp.on_hover_text(tip)
}

fn paint_icon_button(ui: &Ui, rect: Rect, resp: &Response, icon: Icon) {
    let ctx = ui.ctx();
    let (pal, d) = (theme::current(ctx), theme::design(ctx));
    let enabled = ui.is_enabled();
    let hover = fade(ctx, resp.id.with("hover"), enabled && resp.hovered(), 0.15);
    let press = fade(ctx, resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    let rect = rect.shrink2(rect.size() * (0.03 * press));
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_sm), pal.hover.gamma_multiply(hover + 0.8 * press));
    icons::draw(p, Rect::from_center_size(rect.center(), Vec2::splat(18.0)), icon, icons::tint(icon, &pal), enabled);
}

/// Two or three glyph buttons in one rounded track, one of them selected (cards or table).
pub fn icon_segmented<T: PartialEq + Copy>(ui: &mut Ui, options: &[(T, Icon, &str)], value: &mut T) -> Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let item = Vec2::new(34.0, 28.0);
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(item.x * options.len() as f32 + 4.0, item.y + 4.0), Sense::click());
    let inner = rect.shrink(2.0);
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let i = (((pos.x - inner.left()) / item.x).floor().max(0.0) as usize).min(options.len() - 1);
            if options[i].0 != *value {
                *value = options[i].0;
                resp.mark_changed();
            }
        }
    }
    let selected = options.iter().position(|(v, _, _)| v == value).unwrap_or(0);
    let x = super::anim::glide(&ctx, resp.id.with("thumb"), inner.left() + item.x * selected as f32, 0.16);
    let track = if ui.layer_id().order != egui::Order::Background { pal.surface3 } else { pal.surface2 };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md), track);
    p.rect_filled(Rect::from_min_size(egui::pos2(x, inner.top()), item), CornerRadius::same(d.radius_sm.saturating_sub(1)), pal.segment_on);
    for (i, (_, icon, _)) in options.iter().enumerate() {
        let c = egui::pos2(inner.left() + item.x * (i as f32 + 0.5), inner.center().y);
        let tint = if i == selected { pal.label } else { pal.secondary };
        icons::draw(p, Rect::from_center_size(c, Vec2::splat(16.0)), *icon, tint, true);
    }
    let tip = options[selected].2;
    resp.on_hover_text(tip)
}

/// A card that holds one number: an overline label, the number, and its unit.
pub fn stat_tile(ui: &mut Ui, width: f32, label: &str, value: &str, unit: &str, accent: egui::Color32) {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 68.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_lg), pal.card);
    if d.card_border {
        p.rect_stroke(rect, CornerRadius::same(d.radius_lg), egui::Stroke::new(1.0, theme::hairline(&ctx)), egui::StrokeKind::Inside);
    }
    p.text(rect.left_top() + Vec2::new(16.0, 14.0), Align2::LEFT_TOP, label.to_uppercase(), FontId::proportional(d.overline), pal.tertiary);
    let metric = p.layout_no_wrap(value.to_owned(), FontId::proportional(24.0), accent);
    let size = metric.size();
    let at = rect.left_top() + Vec2::new(16.0, 32.0);
    p.galley(at, metric, accent);
    p.text(egui::pos2(at.x + size.x + 6.0, at.y + size.y - 3.0), Align2::LEFT_BOTTOM, unit, FontId::proportional(d.caption), pal.secondary);
}

/// Shown instead of a list when there is nothing to list: a glyph, a title, one sentence, and
/// at most one primary action. Returns true when that action was pressed.
pub fn empty_state(ui: &mut Ui, icon: Icon, title: &str, text: &str, action: Option<&str>) -> bool {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let mut pressed = false;
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() * 0.28).clamp(16.0, 120.0));
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(d.radius_lg), pal.blue_tint);
        icons::draw(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(36.0)), icon, pal.blue_ink, true);
        ui.add_space(14.0);
        ui.label(RichText::new(title).size(d.heading).color(pal.label));
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(text).size(d.body).color(pal.secondary)).wrap());
        if let Some(label) = action {
            ui.add_space(14.0);
            pressed = button(ui, ButtonKind::Primary, label).clicked();
        }
    });
    pressed
}
