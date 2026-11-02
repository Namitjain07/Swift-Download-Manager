//! Dialog pieces: the window frame, the primary and secondary actions, switch rows.

use super::{button, card_row, switch, ButtonKind};
use crate::theme::{self, ThemeStyle};
use eframe::egui::{self, epaint::Shadow, Align, CornerRadius, Layout, Margin, Response, RichText, Stroke, Ui, WidgetText};

/// A window in the sheet style of the design in use: Drishti has 20 px corners, a strong edge and
/// the design system's sheet shadow; iOS has 14 px corners and a hairline. Padding is 20 px.
/// egui fades new windows in over its animation time (0.2 s).
pub fn dialog_window(ctx: &egui::Context, title: impl Into<WidgetText>) -> egui::Window<'static> {
    let (pal, d) = (theme::current(ctx), theme::design(ctx));
    let edge = if d.card_border { pal.separator } else { pal.separator_strong };
    let shadow = if d.sheet_shadow { ctx.global_style().visuals.window_shadow } else { Shadow::NONE };
    let frame = egui::Frame::NONE
        .fill(pal.raised)
        .corner_radius(CornerRadius::same(d.radius_xl))
        .stroke(Stroke::new(1.0, edge))
        .shadow(shadow)
        .inner_margin(Margin::same(20));
    // Narrow or short windows: the dialog never grows past the window, and scrolls when it must.
    let screen = ctx.content_rect();
    egui::Window::new(title)
        .frame(frame)
        .collapsible(false)
        .resizable(false)
        .fade_in(true)
        .max_width((screen.width() - 24.0).max(280.0))
        .max_height((screen.height() - 24.0).max(240.0))
        .vscroll(true)
}

/// A preferred dialog width, cut down so the dialog always fits inside a narrow window.
pub fn fit_width(ctx: &egui::Context, preferred: f32) -> f32 {
    preferred.min((ctx.content_rect().width() - 24.0).max(280.0))
}

/// A preferred width for a field inside a dialog, cut down so the dialog fits a narrow window
/// (the margins, the label and the buttons next to it take about 270 points).
pub fn fit_field(ui: &Ui, preferred: f32) -> f32 {
    preferred.min(ui.ctx().content_rect().width() - 270.0).max(120.0)
}

/// The main action of a dialog: solid blue.
pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    button(ui, ButtonKind::Primary, label)
}

/// Any other action: a neutral button in Drishti, a soft blue wash in iOS.
pub fn tinted_button(ui: &mut Ui, label: &str) -> Response {
    let kind = if theme::design(ui.ctx()).style == ThemeStyle::Drishti { ButtonKind::Secondary } else { ButtonKind::Tinted };
    button(ui, kind, label)
}

/// The quietest action, such as Cancel: blue text, a faint wash on hover.
pub fn plain_button(ui: &mut Ui, label: &str) -> Response {
    button(ui, ButtonKind::Plain, label)
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
    ui.label(RichText::new(text.to_uppercase()).size(theme::design(ui.ctx()).overline).color(pal.tertiary));
    ui.add_space(2.0);
}

/// A row of right-aligned actions at the bottom of a dialog. Its height is fixed, so the row
/// cannot stretch to fill the window the way a bare right-to-left layout does.
pub fn button_row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width();
    ui.allocate_ui_with_layout(egui::vec2(w, theme::design(ui.ctx()).control_h + 2.0), Layout::right_to_left(Align::Center), add).inner
}
