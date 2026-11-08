//! Dialogs: a sheet with a header (title and close button), a body that scrolls when the window
//! is too small, and a footer with the buttons that always stays in view.
//!
//! Most sheets are modal: the page behind them is dimmed and cannot be clicked. Progress windows
//! float instead, so several can be open and moved around at once.

use super::{icon_button_at, ButtonKind};
use crate::icons::Icon;
use crate::theme::{self, Elevation};
use eframe::egui::{self, vec2, Align, Color32, CornerRadius, FontId, Frame, Id, Key, Layout, Margin, Modifiers, Pos2, Rect, Response, ScrollArea, Sense, Stroke, Ui, UiBuilder};
use std::hash::Hash;
use std::sync::Arc;

/// Height of the header of a sheet.
pub const HEADER_H: f32 = 64.0;
/// Height of the footer of a sheet, hairline included.
pub const FOOTER_H: f32 = 73.0;
const SIDE: i8 = 24;
/// How long a dialog takes to fade and rise in.
pub const OPEN_SECS: f32 = 0.2;

/// A dialog being built.
pub struct Sheet {
    id: Id,
    title: String,
    width: f32,
    backdrop_closes: bool,
    floating_at: Option<Pos2>,
}

/// What came of showing a sheet.
pub struct SheetOut {
    /// The close button was pressed, or Escape (or, if allowed, the backdrop).
    pub close: bool,
}

/// Starts a sheet. `id_salt` must be unique among the sheets shown at the same time.
pub fn sheet(id_salt: impl Hash + std::fmt::Debug, title: impl Into<String>) -> Sheet {
    Sheet { id: Id::unique(("sdm_sheet", id_salt)), title: title.into(), width: 520.0, backdrop_closes: false, floating_at: None }
}

impl Sheet {
    /// The wanted width; the sheet is narrower when the window is.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// A click outside the sheet closes it (for sheets that only show information).
    pub fn backdrop_closes(mut self) -> Self {
        self.backdrop_closes = true;
        self
    }

    /// Not modal: a window that can be moved, first shown at `pos`.
    pub fn floating(mut self, pos: Pos2) -> Self {
        self.floating_at = Some(pos);
        self
    }

    pub fn show(self, ctx: &egui::Context, add: impl FnOnce(&mut SheetUi)) -> SheetOut {
        let (pal, d) = (theme::current(ctx), theme::design(ctx));
        let screen = ctx.content_rect();
        let width = self.width.min((screen.width() - 32.0).max(300.0));
        let body_max = (screen.height() - 32.0 - HEADER_H - FOOTER_H - 32.0).max(120.0);
        let edge = if ctx.global_style().visuals.dark_mode { pal.separator_strong } else { pal.separator };
        let frame = Frame::NONE.fill(pal.raised).corner_radius(CornerRadius::same(d.radius_xl)).stroke(Stroke::new(1.0, edge)).shadow(theme::shadow(ctx, Elevation::Dialog));
        let mut close = false;
        let Sheet { id, title, backdrop_closes, floating_at, .. } = self;
        // A dialog fades in over a fifth of a second (egui's own fade is a twelfth) and rises into place.
        let opened = super::anim::entrance(ctx, id.with("open"), OPEN_SECS);
        let normal_fade = ctx.global_style().animation_time;
        ctx.global_style_mut(|s| s.animation_time = OPEN_SECS);
        let shown = if let Some(pos) = floating_at {
            egui::Window::new(title.clone())
                .id(id)
                .title_bar(false)
                .frame(frame)
                .resizable(false)
                .collapsible(false)
                .constrain(true)
                .default_pos(pos)
                .show(ctx, |ui| contents(ui, &title, width, body_max, id, &mut close, add));
            None
        } else {
            let area = egui::Modal::default_area(id).anchor(egui::Align2::CENTER_CENTER, [0.0, super::anim::rise(opened)]);
            Some(egui::Modal::new(id).area(area).frame(frame).backdrop_color(pal.scrim).show(ctx, |ui| contents(ui, &title, width, body_max, id, &mut close, add)))
        };
        ctx.global_style_mut(|s| s.animation_time = normal_fade);
        // A floating window has no modal to listen to Escape, so only the button closes it.
        let Some(modal) = shown else { return SheetOut { close } };
        let esc = modal.is_top_modal && !modal.any_popup_open && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        if esc || (backdrop_closes && modal.backdrop_response.clicked()) {
            close = true;
        }
        SheetOut { close }
    }
}

fn contents(ui: &mut Ui, title: &str, width: f32, body_max: f32, id: Id, close: &mut bool, add: impl FnOnce(&mut SheetUi)) {
    let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
    ui.set_width(width);
    // The three sections stack with no gap between them; each one sets its own inner spacing.
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    // Header: the title at the left, the close button at the right.
    let (hdr, _) = ui.allocate_exact_size(vec2(width, HEADER_H), Sense::hover());
    let close_rect = Rect::from_center_size(Pos2::new(hdr.right() - 14.0 - 18.0, hdr.center().y), vec2(36.0, 36.0));
    let galley = truncated(ui.painter(), title, FontId::new(d.title + 3.0, theme::semibold()), pal.label, width - 2.0 * SIDE as f32 - 36.0 - 8.0);
    ui.painter().galley(Pos2::new(hdr.left() + SIDE as f32, hdr.center().y - galley.size().y / 2.0), galley, pal.label);
    if icon_button_at(ui, close_rect, Icon::Close, "Close", id.with("close")).clicked() {
        *close = true;
    }
    ui.painter().hline(hdr.x_range(), hdr.bottom(), Stroke::new(1.0, pal.separator));
    add(&mut SheetUi { ui, body_max });
}

/// The inside of a sheet, handed to the closure of `Sheet::show`.
pub struct SheetUi<'u> {
    ui: &'u mut Ui,
    body_max: f32,
}

impl SheetUi<'_> {
    /// The body: it scrolls when the window is too small to show it all.
    pub fn body<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> R {
        let max = self.body_max;
        Frame::NONE
            .inner_margin(Margin::symmetric(SIDE, 16))
            .show(self.ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                ScrollArea::vertical().max_height(max).auto_shrink([false, true]).show(ui, |ui| add(ui)).inner
            })
            .inner
    }

    /// The footer for a row of buttons at the right. The first button added ends up at the right
    /// edge, so add the main action first.
    pub fn footer<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> R {
        self.footer_split(|_| {}, add).1
    }

    /// A footer with something at the left (usually Cancel) and the main buttons at the right.
    pub fn footer_split<L, R>(&mut self, left: impl FnOnce(&mut Ui) -> L, right: impl FnOnce(&mut Ui) -> R) -> (L, R) {
        let (pal, d) = (theme::current(self.ui.ctx()), theme::design(self.ui.ctx()));
        let w = self.ui.available_width();
        let y = self.ui.cursor().top();
        self.ui.painter().hline(self.ui.max_rect().x_range(), y, Stroke::new(1.0, pal.separator));
        self.ui.add_space(1.0);
        Frame::NONE
            .inner_margin(Margin::symmetric(SIDE, 16))
            .show(self.ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                let (rect, _) = ui.allocate_exact_size(vec2((w - 2.0 * SIDE as f32).max(0.0), d.control_h), Sense::hover());
                let mut l = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
                let a = left(&mut l);
                let mut r = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::right_to_left(Align::Center)));
                let b = right(&mut r);
                (a, b)
            })
            .inner
    }
}

/// One line of text cut off with an ellipsis when it does not fit `max_width`.
pub fn truncated(p: &egui::Painter, text: &str, font: FontId, color: Color32, max_width: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap.max_width = max_width;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    p.layout_job(job)
}

/// Hover tooltip with the whole text for a line that `truncated` cut short with an ellipsis, so a long file name
/// can always be read in full. Nothing happens when the whole text fits.
pub fn elided_tooltip(ui: &Ui, galley: &egui::Galley, at: Pos2, full: &str, id_salt: impl Hash + std::fmt::Debug) {
    if galley.elided {
        ui.interact(Rect::from_min_size(at, galley.size()), ui.scope_id().with(("elided_tooltip", id_salt)), Sense::hover()).on_hover_text(full);
    }
}

/// The main action of a dialog: solid coral.
pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    super::button(ui, ButtonKind::Primary, label)
}

/// Any other action: a quiet grey capsule.
pub fn secondary_button(ui: &mut Ui, label: &str) -> Response {
    super::button(ui, ButtonKind::Secondary, label)
}

/// The quietest action, such as Cancel.
pub fn plain_button(ui: &mut Ui, label: &str) -> Response {
    super::button(ui, ButtonKind::Plain, label)
}

/// A round tick box and its label; the whole line can be clicked. For lists of choices.
pub fn check_row(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let galley = truncated(ui.painter(), label, FontId::proportional(d.body), pal.label, (ui.available_width() - 22.0 - 12.0).max(40.0));
    let h = 32.0f32.max(galley.size().y + 8.0);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let hover = super::anim::fade(&ctx, resp.id.with("hover"), resp.hovered(), 0.1);
    let k = super::anim::fade(&ctx, resp.id.with("on"), *on, 0.12);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_sm), pal.hover.gamma_multiply(hover));
    // A round tick in the capsule shape, a rounded square in Files and Carbon.
    let c = Pos2::new(rect.left() + 8.0 + 11.0, rect.center().y);
    // WinUI 3: a 20 px box with 4 px corners and a 1 px outline.
    let side = if d.is_winui() { 20.0 } else { 22.0 };
    let boxr = Rect::from_center_size(c, vec2(side, side));
    let corner = CornerRadius::same(d.round(side).min(11));
    p.rect_filled(boxr, corner, pal.accent_solid.gamma_multiply(k));
    p.rect_stroke(boxr.shrink(0.75), corner, Stroke::new(if d.is_winui() { 1.0 } else { 1.5 }, pal.control_border.gamma_multiply(1.0 - k)), egui::StrokeKind::Inside);
    if k > 0.01 {
        crate::icons::draw_weight(p, Rect::from_center_size(c, vec2(14.0, 14.0)), Icon::Check, pal.on_accent.gamma_multiply(k), true, false);
    }
    p.galley(Pos2::new(rect.left() + 8.0 + 22.0 + 12.0, rect.center().y - galley.size().y / 2.0), galley, pal.label);
    let state = *on;
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, state, label));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::themed;
    use crate::theme::ThemeMode;

    fn shown_sheet(screen: egui::Vec2, body_height: f32) -> (Rect, Rect) {
        let ctx = themed(ThemeMode::Light);
        let mut footer_rect = Rect::NOTHING;
        let mut sheet_rect = Rect::NOTHING;
        for _ in 0..3 {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)), ..Default::default() }, |ui| {
                let o = sheet("t", "Scheduler").width(440.0).show(ui.ctx(), |s| {
                    s.body(|ui| {
                        ui.allocate_space(vec2(100.0, body_height));
                    });
                    s.footer(|ui| {
                        footer_rect = primary_button(ui, "Save").rect;
                    });
                });
                assert!(!o.close);
            });
            out.textures_delta.clear();
            if let Some(area) = ctx.memory(|m| m.area_rect(Id::unique(("sdm_sheet", "t")))) {
                sheet_rect = area;
            }
        }
        (sheet_rect, footer_rect)
    }

    #[test]
    fn the_footer_buttons_stay_inside_the_window_even_when_the_body_is_too_tall() {
        // This was the Scheduler bug: a tall body pushed the Save button out of view.
        for screen in [vec2(1180.0, 800.0), vec2(700.0, 500.0), vec2(420.0, 420.0)] {
            let (sheet, footer) = shown_sheet(screen, 2000.0);
            let window = Rect::from_min_size(Pos2::ZERO, screen);
            assert!(window.contains_rect(footer), "footer {footer:?} outside {window:?} (sheet {sheet:?})");
            assert!(sheet.height() <= screen.y, "the sheet {sheet:?} is taller than the window {screen:?}");
        }
    }

    #[test]
    fn a_short_body_makes_a_short_sheet_and_the_sheet_is_never_wider_than_the_window() {
        let (tall, _) = shown_sheet(vec2(1180.0, 800.0), 500.0);
        let (short, _) = shown_sheet(vec2(1180.0, 800.0), 50.0);
        assert!(short.height() < tall.height());
        let (narrow, _) = shown_sheet(vec2(360.0, 800.0), 50.0);
        assert!(narrow.width() <= 360.0, "{narrow:?}");
    }

    #[test]
    fn the_header_and_footer_heights_match_what_the_layout_reserves() {
        let (sheet, _) = shown_sheet(vec2(1180.0, 800.0), 0.0);
        // header + body padding + footer, with a zero-height body.
        let expected = HEADER_H + 32.0 + FOOTER_H;
        assert!((sheet.height() - expected).abs() < 6.0, "{} vs {expected}", sheet.height());
    }

    #[test]
    fn truncated_text_ends_with_an_ellipsis_and_fits() {
        let ctx = themed(ThemeMode::Light);
        let mut w = 0.0;
        let (mut rows, mut last, mut shown) = (0, None, 0);
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let g = truncated(ui.painter(), "a-very-long-file-name-that-will-not-fit-in-a-small-space.iso", FontId::proportional(14.0), Color32::BLACK, 120.0);
            w = g.size().x;
            rows = g.rows.len();
            last = g.rows[0].row.glyphs.last().map(|x| x.chr);
            shown = g.rows[0].row.glyphs.len();
        });
        out.textures_delta.clear();
        assert!(w <= 121.0, "{w}");
        assert_eq!(rows, 1, "it stays on one line");
        assert_eq!(last, Some('…'), "the cut is marked with an ellipsis");
        assert!(shown < "a-very-long-file-name-that-will-not-fit-in-a-small-space.iso".len());
    }
}
