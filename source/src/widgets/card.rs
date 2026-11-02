//! Grouped card with inset rows. iOS draws a hairline border around it; Drishti steps up one
//! surface instead and has none.

use crate::theme;
use eframe::egui::{self, Align, CornerRadius, Layout, Margin, Stroke, Ui};

/// A rounded group. Fill it with `card_row`s.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
    let stroke = if d.card_border { Stroke::new(1.0, theme::hairline(ui.ctx())) } else { Stroke::NONE };
    egui::Frame::NONE
        .fill(pal.card)
        .corner_radius(CornerRadius::same(d.radius_lg))
        .stroke(stroke)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.set_min_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// One row of a card, at least 44 px tall. `last` hides the hairline under it.
pub fn card_row<R>(ui: &mut Ui, last: bool, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_row_h(ui, last, 32.0, add)
}

/// A card row whose content area is `height` tall (for wheels and other tall controls). The
/// height is fixed so that centred and right-aligned content cannot stretch the row.
pub fn card_row_h<R>(ui: &mut Ui, last: bool, height: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let d = theme::design(ui.ctx());
    let inset = d.inset as i8;
    let out = egui::Frame::NONE.inner_margin(Margin { left: inset, right: inset, top: 6, bottom: 6 }).show(ui, |ui| {
        let w = ui.available_width();
        ui.allocate_ui_with_layout(egui::vec2(w, height), Layout::left_to_right(Align::Center), add).inner
    });
    if !last {
        let r = out.response.rect;
        ui.painter().hline((r.left() + d.inset)..=r.right(), r.bottom() - 0.5, Stroke::new(1.0, theme::hairline(ui.ctx())));
    }
    out.inner
}
