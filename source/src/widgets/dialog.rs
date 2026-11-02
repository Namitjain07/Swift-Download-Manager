//! Dialog pieces: the window frame, the primary and secondary actions, switch rows.

use super::{button, card_row, switch, ButtonKind};
use crate::theme;
use eframe::egui::{self, epaint::Shadow, Align, CornerRadius, Layout, Margin, Response, RichText, Stroke, Ui, WidgetText};

/// A window in the iOS sheet style: radius 14, hairline border, 20 px padding, no shadow.
/// egui fades new windows in over its animation time (0.2 s).
pub fn dialog_window(ctx: &egui::Context, title: impl Into<WidgetText>) -> egui::Window<'static> {
    let pal = theme::current(ctx);
    let frame = egui::Frame::NONE
        .fill(pal.raised)
        .corner_radius(CornerRadius::same(14))
        .stroke(Stroke::new(1.0, pal.separator))
        .shadow(Shadow::NONE)
        .inner_margin(Margin::same(20));
    egui::Window::new(title).frame(frame).collapsible(false).resizable(false).fade_in(true)
}

/// The main action of a dialog: a filled blue capsule.
pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    let tint = theme::current(ui.ctx()).blue;
    button(ui, ButtonKind::Filled, tint, label)
}

/// Any other action: a soft blue capsule.
pub fn tinted_button(ui: &mut Ui, label: &str) -> Response {
    let tint = theme::current(ui.ctx()).blue;
    button(ui, ButtonKind::Tinted, tint, label)
}

/// Switch followed by its label, for use inside a row.
pub fn toggle(ui: &mut Ui, on: &mut bool, label: impl Into<WidgetText>) -> Response {
    ui.horizontal(|ui| {
        let r = switch(ui, on);
        ui.label(label);
        r
    })
    .inner
}

/// Row of a card: label on the left, switch on the right.
pub fn toggle_row(ui: &mut Ui, label: &str, on: &mut bool, last: bool) -> Response {
    card_row(ui, last, |ui| {
        ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| switch(ui, on)).inner
    })
}

/// Small grey title above a group, like the headers in iOS Settings.
pub fn group_title(ui: &mut Ui, text: &str) {
    let pal = theme::current(ui.ctx());
    ui.add_space(8.0);
    ui.label(RichText::new(text.to_uppercase()).size(11.0).color(pal.secondary));
    ui.add_space(2.0);
}
