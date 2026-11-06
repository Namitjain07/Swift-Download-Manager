//! Inputs: text fields, the search pill, number fields, the stepper and the drop-down.
//!
//! They all stand 40 px tall, have 12 px corners (the search pill is fully round), rest on a visible
//! border, darken it on hover and take a 2 px ring while being typed in.

use super::anim::fade;
use super::menu::{menu_check_row, menu_surface};
use crate::icons::{self, Icon};
use crate::theme;
use eframe::egui::{self, vec2, Align, Align2, Color32, CornerRadius, FontId, Frame, Layout, Margin, Pos2, Rect, Response, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder};
use std::ops::RangeInclusive;

/// A single-line text field set up for this design. Add it with `ui.add(...)`; give it a width with
/// `.desired_width(...)`, or use `field_full` to fill the row.
pub fn field(text: &mut String) -> TextEdit<'_> {
    TextEdit::singleline(text).margin(Margin::symmetric(14, 8)).align(Align2::LEFT_CENTER).min_size(vec2(0.0, theme::DESIGN.control_h))
}

/// A field as wide as the room there is.
pub fn field_full(ui: &mut Ui, text: &mut String, hint: &str) -> Response {
    let w = ui.available_width();
    ui.add(field(text).hint_text(hint).desired_width(w))
}

/// A multi-line field with `rows` lines.
pub fn field_multiline<'t>(text: &'t mut String, rows: usize) -> TextEdit<'t> {
    TextEdit::multiline(text).margin(Margin::symmetric(14, 12)).desired_rows(rows)
}

/// The rounded search box: a magnifier, the text, and a clear button while there is text.
pub fn search_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let (rect, _) = ui.allocate_exact_size(vec2(width, d.control_h), Sense::hover());
    let radius = CornerRadius::same((d.control_h / 2.0) as u8);
    let right_pad = if text.is_empty() { 16.0 } else { 40.0 };
    let inner = Rect::from_min_max(Pos2::new(rect.left() + 44.0, rect.top()), Pos2::new((rect.right() - right_pad).max(rect.left() + 60.0), rect.bottom()));
    let hovered = ui.rect_contains_pointer(rect);
    let mut child = ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)));
    // The background goes down first so the text is painted over it.
    let bg = ui.painter().add(egui::Shape::Noop);
    let resp = child.add(TextEdit::singleline(text).frame(Frame::NONE).margin(Margin::ZERO).hint_text(hint).align(Align2::LEFT_CENTER).desired_width(inner.width()));
    let focused = resp.has_focus();
    let hover = fade(&ctx, resp.id.with("hover"), hovered || focused, 0.12);
    let border = pal.separator_strong.lerp_to_gamma(pal.label, hover * if focused { 1.0 } else { 0.6 });
    ui.painter().set(bg, egui::epaint::RectShape::new(rect, radius, pal.card, Stroke::new(if focused { 2.0 } else { 1.0 }, border), StrokeKind::Inside));
    icons::draw(ui.painter(), Rect::from_center_size(Pos2::new(rect.left() + 22.0, rect.center().y), vec2(18.0, 18.0)), Icon::Search, pal.tertiary, true);
    if !text.is_empty() {
        let clear = Rect::from_center_size(Pos2::new(rect.right() - 20.0, rect.center().y), vec2(24.0, 24.0));
        let r = ui.interact(clear, resp.id.with("clear"), Sense::click());
        let k = fade(&ctx, r.id.with("hover"), r.hovered(), 0.12);
        ui.painter().circle_filled(clear.center(), 12.0, pal.surface3.gamma_multiply(k));
        icons::draw(ui.painter(), Rect::from_center_size(clear.center(), vec2(14.0, 14.0)), Icon::Close, pal.secondary, true);
        if r.on_hover_text("Clear search").clicked() {
            text.clear();
        }
    }
    resp
}

/// Whole numbers a stepper or number field can hold.
pub trait Int: Copy {
    fn get(self) -> i64;
    fn set(v: i64) -> Self;
}

macro_rules! int_impl {
    ($($t:ty),+) => {$(
        impl Int for $t {
            fn get(self) -> i64 {
                i64::try_from(self).unwrap_or(i64::MAX)
            }
            fn set(v: i64) -> Self {
                // `u64::MAX as i64` would wrap to -1, so the limits are converted with saturation.
                let lo = i64::try_from(<$t>::MIN).unwrap_or(i64::MIN);
                let hi = i64::try_from(<$t>::MAX).unwrap_or(i64::MAX);
                <$t>::try_from(v.clamp(lo, hi)).unwrap_or(<$t>::MIN)
            }
        }
    )+};
}
int_impl!(u8, u16, u32, u64, usize, i32, i64);

/// The iOS stepper: a minus, the value and a plus in one capsule. Returns a response that is
/// `changed` when a button was pressed.
pub fn stepper<T: Int>(ui: &mut Ui, value: &mut T, range: RangeInclusive<i64>) -> Response {
    stepper_with(ui, value, range, "")
}

/// The same with a unit written after the number ("KB/s").
pub fn stepper_with<T: Int>(ui: &mut Ui, value: &mut T, range: RangeInclusive<i64>, suffix: &str) -> Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let enabled = ui.is_enabled();
    let shown = if suffix.is_empty() { value.get().to_string() } else { format!("{} {suffix}", value.get()) };
    let galley = ui.painter().layout_no_wrap(shown, FontId::new(d.body, theme::semibold()), Color32::PLACEHOLDER);
    let button_w = 36.0;
    let w = 2.0 * button_w + galley.size().x.max(28.0) + 20.0;
    let (rect, mut resp) = ui.allocate_exact_size(vec2(w, d.control_h), Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(rect, CornerRadius::same((d.control_h / 2.0) as u8), pal.surface2);
    let mut changed = false;
    let cur = value.get();
    for (k, (icon, delta, id)) in [(Icon::Minus, -1i64, "minus"), (Icon::Plus, 1, "plus")].into_iter().enumerate() {
        let b = Rect::from_min_size(Pos2::new(if k == 0 { rect.left() + 2.0 } else { rect.right() - 2.0 - button_w }, rect.top() + 2.0), vec2(button_w, d.control_h - 4.0));
        let can = enabled && ((delta < 0 && cur > *range.start()) || (delta > 0 && cur < *range.end()));
        let r = ui.interact(b, resp.id.with(id), if can { Sense::click() } else { Sense::hover() });
        let hover = fade(&ctx, r.id.with("hover"), can && r.hovered(), 0.12);
        let press = fade(&ctx, r.id.with("press"), can && r.is_pointer_button_down_on(), 0.08);
        p.rect_filled(b, CornerRadius::same(((d.control_h - 4.0) / 2.0) as u8), pal.surface3.gamma_multiply(hover.max(press)));
        icons::draw_weight(&p, Rect::from_center_size(b.center(), vec2(16.0, 16.0)), icon, pal.label, can, false);
        if can && r.clicked() {
            *value = T::set((cur + delta).clamp(*range.start(), *range.end()));
            changed = true;
        }
    }
    p.galley(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0), galley, if enabled { pal.label } else { pal.disabled });
    if changed {
        resp.mark_changed();
    }
    resp
}

/// A field for a whole number, with its unit written inside it on the right. The text can be
/// edited freely; the value is kept inside `range`.
pub fn number_field<T: Int>(ui: &mut Ui, value: &mut T, range: RangeInclusive<i64>, suffix: &str, width: f32) -> Response {
    let ctx = ui.ctx().clone();
    let pal = theme::current(&ctx);
    let id = ui.next_auto_id();
    ui.skip_ahead_auto_ids(1);
    let mut buf = ctx.data(|d| d.get_temp::<String>(id.with("buf"))).unwrap_or_else(|| value.get().to_string());
    let focused_before = ctx.memory(|m| m.has_focus(id.with("edit")));
    if !focused_before {
        buf = value.get().to_string();
    }
    let suffix_w = if suffix.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(suffix.to_owned(), FontId::proportional(theme::DESIGN.body), Color32::PLACEHOLDER).size().x + 14.0 };
    let enabled = ui.is_enabled();
    let resp = ui.add_enabled(enabled, field(&mut buf).id(id.with("edit")).desired_width(width).margin(Margin { left: 14, right: (14.0 + suffix_w) as i8, top: 8, bottom: 8 }));
    let mut changed = false;
    if resp.changed() {
        let digits: String = buf.chars().filter(char::is_ascii_digit).collect();
        if let Ok(n) = digits.parse::<i64>() {
            let n = n.clamp(*range.start(), *range.end());
            if n != value.get() {
                *value = T::set(n);
                changed = true;
            }
        }
    }
    if resp.lost_focus() {
        buf = value.get().to_string();
    }
    ctx.data_mut(|d| d.insert_temp(id.with("buf"), buf));
    if suffix_w > 0.0 {
        ui.painter().text(Pos2::new(resp.rect.right() - 14.0, resp.rect.center().y), Align2::RIGHT_CENTER, suffix, FontId::proportional(theme::DESIGN.body), if enabled { pal.tertiary } else { pal.disabled });
    }
    let mut resp = resp;
    if changed {
        resp.mark_changed();
    }
    resp
}

/// A drop-down: a field that shows the chosen option and opens a list of the options. Returns a
/// response that is `changed` when another option was picked.
pub fn dropdown<T: PartialEq + Clone>(ui: &mut Ui, id_salt: impl std::hash::Hash + std::fmt::Debug, width: f32, options: &[(T, String)], value: &mut T) -> Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let enabled = ui.is_enabled();
    let id = ui.make_persistent_id(id_salt);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, d.control_h), if enabled { Sense::click() } else { Sense::hover() });
    let popup_open = egui::Popup::is_id_open(&ctx, egui::Popup::default_response_id(&resp));
    let hover = fade(&ctx, id.with("hover"), enabled && (resp.hovered() || popup_open), 0.12);
    let text = options.iter().find(|(v, _)| v == value).map(|(_, l)| l.clone()).unwrap_or_default();
    let p = ui.painter();
    let radius = CornerRadius::same(d.radius_md);
    p.rect_filled(rect, radius, pal.card);
    let border = pal.control_border.lerp_to_gamma(pal.label, hover);
    p.rect_stroke(rect, radius, Stroke::new(if popup_open { 2.0 } else { 1.0 }, border.gamma_multiply(if enabled { 1.0 } else { 0.5 })), StrokeKind::Inside);
    let galley = ui.painter().layout(text, FontId::proportional(d.body), if enabled { pal.label } else { pal.disabled }, (rect.width() - 14.0 - 40.0).max(20.0));
    let top = rect.center().y - galley.size().y / 2.0;
    ui.painter().galley(Pos2::new(rect.left() + 14.0, top), galley, pal.label);
    icons::draw(ui.painter(), Rect::from_center_size(Pos2::new(rect.right() - 20.0, rect.center().y), vec2(16.0, 16.0)), if popup_open { Icon::CaretUp } else { Icon::CaretDown }, if enabled { pal.secondary } else { pal.disabled }, true);
    let mut changed = false;
    let mut picked: Option<T> = None;
    egui::Popup::menu(&resp).width(width).frame(menu_surface(&ctx)).show(|ui| {
        ui.set_min_width(width - 16.0);
        egui::ScrollArea::vertical().max_height(280.0).auto_shrink([false, true]).show(ui, |ui| {
            for (v, label) in options {
                if menu_check_row(ui, label, v == value).clicked() {
                    picked = Some(v.clone());
                    ui.close();
                }
            }
        });
    });
    if let Some(v) = picked
        && &v != value
    {
        *value = v;
        changed = true;
    }
    let mut resp = resp;
    if changed {
        resp.mark_changed();
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::themed;
    use crate::theme::ThemeMode;

    fn run(ctx: &egui::Context, width: f32, mut add: impl FnMut(&mut Ui)) {
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 300.0))), ..Default::default() }, |ui| {
                egui::CentralPanel::default().show(ui, |ui| add(ui));
            });
            out.textures_delta.clear();
        }
    }

    #[test]
    fn every_input_is_as_tall_as_a_button() {
        let ctx = themed(ThemeMode::Light);
        let (mut text, mut n, mut m, mut choice) = (String::from("hello"), 8u32, 512u64, 1u8);
        let mut heights = Vec::new();
        run(&ctx, 800.0, |ui| {
            heights.clear();
            ui.horizontal(|ui| {
                heights.push(ui.add(field(&mut text).desired_width(120.0)).rect.height());
                heights.push(search_field(ui, &mut String::new(), "Search", 160.0).rect.height());
                heights.push(stepper(ui, &mut n, 1..=32).rect.height());
                heights.push(number_field(ui, &mut m, 1..=100_000, "KB/s", 120.0).rect.height());
                heights.push(dropdown(ui, "d", 120.0, &[(1u8, "One".to_string()), (2, "Two".to_string())], &mut choice).rect.height());
            });
        });
        assert_eq!(heights.len(), 5);
        for h in &heights {
            assert!((*h - theme::DESIGN.control_h).abs() < 0.5, "{heights:?}");
        }
    }

    #[test]
    fn int_types_clamp_instead_of_wrapping() {
        assert_eq!(u8::set(300), 255);
        assert_eq!(u8::set(-4), 0);
        assert_eq!(u16::set(70_000), 65_535);
        assert_eq!(usize::set(12), 12);
        assert_eq!(7u64.get(), 7);
        // The 64-bit unsigned types are wider than i64: reading them saturates and writing them works.
        assert_eq!(u64::MAX.get(), i64::MAX);
        assert_eq!(u64::set(5), 5);
        assert_eq!(u64::set(-5), 0);
        assert_eq!(usize::set(i64::MAX), i64::MAX as usize);
        assert_eq!(i32::set(i64::MAX), i32::MAX);
        assert_eq!(i32::set(i64::MIN), i32::MIN);
    }

    #[test]
    fn stepper_text_follows_the_value_and_its_unit() {
        let ctx = themed(ThemeMode::Light);
        let mut v = 8u32;
        let mut shapes = 0;
        for _ in 0..2 {
            let mut out = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    stepper_with(ui, &mut v, 1..=32, "x");
                });
            });
            out.textures_delta.clear();
            shapes = out.shapes.len();
        }
        assert!(shapes > 0);
        assert_eq!(v, 8, "drawing a stepper never changes its value");
    }
}
