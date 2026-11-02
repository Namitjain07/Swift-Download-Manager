//! Grouped card with inset rows, as in iOS Settings.

use crate::theme;
use eframe::egui::{self, Align, CornerRadius, Layout, Margin, Stroke, Ui};

/// A rounded group. Fill it with `card_row`s.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let pal = theme::current(ui.ctx());
    egui::Frame::NONE
        .fill(pal.card)
        .corner_radius(CornerRadius::same(12))
        .stroke(Stroke::new(1.0, pal.separator.gamma_multiply(0.6)))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.set_min_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// One row of a card, at least 44 px tall. `last` hides the hairline under it.
pub fn card_row<R>(ui: &mut Ui, last: bool, add: impl FnOnce(&mut Ui) -> R) -> R {
    let pal = theme::current(ui.ctx());
    let out = egui::Frame::NONE
        .inner_margin(Margin { left: 14, right: 14, top: 6, bottom: 6 })
        .show(ui, |ui| {
            ui.set_min_size(egui::vec2(ui.available_width(), 32.0));
            ui.with_layout(Layout::left_to_right(Align::Center), add).inner
        });
    if !last {
        let r = out.response.rect;
        ui.painter().hline((r.left() + 14.0)..=r.right(), r.bottom() - 0.5, Stroke::new(1.0, pal.separator.gamma_multiply(0.7)));
    }
    out.inner
}
