//! Wheel picker for times, like the iOS time picker.

use super::anim::glide;
use crate::theme;
use eframe::egui::{self, Align2, CornerRadius, FontId, Id, Response, Sense, Ui};
use std::sync::OnceLock;

const ROW: f32 = 32.0;

/// "07:05" as (7, 5). Anything unreadable gives (0, 0); out-of-range parts are clamped.
pub fn parse_hhmm(s: &str) -> (usize, usize) {
    let Some((h, m)) = s.trim().split_once(':') else { return (0, 0) };
    match (h.trim().parse::<usize>(), m.trim().parse::<usize>()) {
        (Ok(h), Ok(m)) => (h.min(23), m.min(59)),
        _ => (0, 0),
    }
}

pub fn format_hhmm(h: usize, m: usize) -> String {
    format!("{h:02}:{m:02}")
}

fn two_digits(n: usize) -> &'static [String] {
    static HOURS: OnceLock<Vec<String>> = OnceLock::new();
    static MINUTES: OnceLock<Vec<String>> = OnceLock::new();
    let (cell, count) = if n == 24 { (&HOURS, 24) } else { (&MINUTES, 60) };
    cell.get_or_init(|| (0..count).map(|i| format!("{i:02}")).collect())
}

/// One wheel: three rows are visible, the middle one is selected. Drag it, scroll it, or
/// click the row above or below. It wraps around (23 is followed by 00) and glides.
pub fn wheel(ui: &mut Ui, id: Id, items: &[String], index: &mut usize, width: f32) -> Response {
    let n = items.len().max(1);
    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(width, ROW * 3.0), Sense::click_and_drag());
    let ctx = ui.ctx().clone();
    let pal = theme::current(&ctx);

    // The target is unbounded so wrapping glides forward instead of spinning back.
    let tid = id.with("target");
    let mut target = ctx.data(|d| d.get_temp::<f32>(tid)).filter(|t| (t.rem_euclid(n as f32).round() as usize) % n == *index % n).unwrap_or(*index as f32);
    let mut steps = 0i32;
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            steps = if pos.y < rect.center().y - ROW / 2.0 { -1 } else if pos.y > rect.center().y + ROW / 2.0 { 1 } else { 0 };
        }
    }
    let aid = id.with("acc");
    let mut acc = ctx.data(|d| d.get_temp::<f32>(aid)).unwrap_or(0.0);
    acc += if resp.dragged() { -resp.drag_delta().y } else { 0.0 };
    if resp.hovered() {
        acc -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    while acc >= ROW {
        acc -= ROW;
        steps += 1;
    }
    while acc <= -ROW {
        acc += ROW;
        steps -= 1;
    }
    if !resp.dragged() && !resp.hovered() {
        acc = 0.0;
    }
    ctx.data_mut(|d| d.insert_temp(aid, acc));
    if steps != 0 {
        target += steps as f32;
        *index = (target.rem_euclid(n as f32).round() as usize) % n;
        resp.mark_changed();
    }
    ctx.data_mut(|d| d.insert_temp(tid, target));

    let shown = glide(&ctx, id.with("shown"), target, 0.16);
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(egui::Rect::from_center_size(rect.center(), egui::vec2(width, ROW)), CornerRadius::same(8), pal.separator.gamma_multiply(0.35));
    let base = shown.floor();
    let frac = shown - base;
    for k in -2..=3 {
        let y = rect.center().y + (k as f32 - frac) * ROW;
        let near = ((y - rect.center().y).abs() / (ROW * 1.5)).clamp(0.0, 1.0);
        let i = (base as i32 + k).rem_euclid(n as i32) as usize;
        p.text(egui::pos2(rect.center().x, y), Align2::CENTER_CENTER, &items[i], FontId::proportional(19.0), pal.label.gamma_multiply(1.0 - 0.75 * near));
    }
    resp
}

/// Width of `time_wheels`: two 56 px wheels, the colon and the gaps between them.
pub const TIME_WHEELS_WIDTH: f32 = 128.0;

/// Hour and minute wheels for an "HH:MM" string. True when the person changed it. It takes a
/// fixed block and lays the wheels out left to right inside it, so it reads the same even in
/// a right-to-left row.
pub fn time_wheels(ui: &mut Ui, id: Id, hhmm: &mut String) -> bool {
    let (mut h, mut m) = parse_hhmm(hhmm);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(TIME_WHEELS_WIDTH, ROW * 3.0), Sense::hover());
    let mut changed = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Min)), |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        changed |= wheel(ui, id.with("h"), two_digits(24), &mut h, 56.0).changed();
        let (colon, _) = ui.allocate_exact_size(egui::vec2(8.0, ROW * 3.0), Sense::hover());
        ui.painter().text(colon.center(), Align2::CENTER_CENTER, ":", FontId::proportional(19.0), theme::current(ui.ctx()).label);
        changed |= wheel(ui, id.with("m"), two_digits(60), &mut m, 56.0).changed();
    });
    if changed {
        *hhmm = format_hhmm(h, m);
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_wheels_round_trip() {
        assert_eq!(parse_hhmm("07:05"), (7, 5));
        assert_eq!(format_hhmm(7, 5), "07:05");
        assert_eq!(parse_hhmm("garbage"), (0, 0));
        assert_eq!(parse_hhmm("7:5"), (7, 5));
        assert_eq!(parse_hhmm("25:99"), (23, 59));
        assert_eq!(parse_hhmm(&format_hhmm(23, 59)), (23, 59));
    }
}
