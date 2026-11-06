//! Forms: grouped settings rows, section titles and labelled fields.
//!
//! A `card` is a rounded group with a thin border. Its rows are laid out by hand: the title (and
//! an optional description) is on the left, the control at the right end, both centred on the
//! row. Because the row computes its own height, nothing in it can stretch or drift.

use super::{switch, switch::switch_size};
use crate::theme;
use eframe::egui::{self, vec2, Align, Align2, CornerRadius, FontId, Layout, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder};

/// Smallest height of a row.
pub const ROW_MIN_H: f32 = 56.0;
/// Space between the text of a row and its control.
const GAP: f32 = 16.0;

/// The rows added so far to a card.
pub struct Group<'u> {
    ui: &'u mut Ui,
    rows: usize,
    enabled: bool,
}

/// A rounded group with a thin border. Fill it with rows.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Group) -> R) -> R {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let radius = CornerRadius::same(d.radius_lg);
    let width = ui.available_width();
    // The fill and border are laid down first and resized once the rows are known.
    let bg = ui.painter().add(egui::Shape::Noop);
    let inner = ui.scope(|ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 0.0;
        add(&mut Group { ui, rows: 0, enabled: true })
    });
    let rect = inner.response.rect;
    ui.painter().set(bg, egui::epaint::RectShape::new(rect, radius, pal.card, Stroke::new(1.0, pal.separator_strong), StrokeKind::Inside));
    inner.inner
}

impl Group<'_> {
    /// Shows the rows added from now on greyed out and out of reach (or back to normal).
    pub fn enable(&mut self, on: bool) {
        self.enabled = on;
    }

    fn hairline_if_needed(&mut self) {
        if self.rows > 0 {
            let (pal, d) = (theme::current(self.ui.ctx()), theme::design(self.ui.ctx()));
            let rect = self.ui.cursor();
            let x0 = rect.left() + d.inset;
            self.ui.painter().hline(x0..=self.ui.max_rect().right(), rect.top() - 0.5, Stroke::new(1.0, pal.separator));
        }
        self.rows += 1;
    }

    /// A row with a title (and a description under it) on the left and a control at the right end.
    /// `control_w` is the room the control needs; it is added right to left, so the first widget
    /// added ends up at the right edge.
    pub fn row<R>(&mut self, title: &str, description: Option<&str>, control_w: f32, control: impl FnOnce(&mut Ui) -> R) -> R {
        self.hairline_if_needed();
        let ctx = self.ui.ctx().clone();
        let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
        let width = self.ui.available_width();
        let text_w = (width - 2.0 * d.inset - control_w - if control_w > 0.0 { GAP } else { 0.0 }).max(80.0);
        let (title_col, desc_col) = if self.enabled { (pal.label, pal.tertiary) } else { (pal.disabled, pal.disabled) };
        let title_g = self.ui.painter().layout(title.to_owned(), FontId::proportional(d.body), title_col, text_w);
        let desc_g = description.map(|t| self.ui.painter().layout(t.to_owned(), FontId::proportional(d.caption), desc_col, text_w));
        let text_h = title_g.size().y + desc_g.as_ref().map_or(0.0, |g| g.size().y + 2.0);
        // Switches are 31 high and fields 40: a row has room for either without growing.
        let height = (text_h.max(32.0) + 2.0 * 12.0).max(ROW_MIN_H);
        let (rect, _) = self.ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let top = rect.center().y - text_h / 2.0;
        let p = self.ui.painter();
        let h = title_g.size().y;
        p.galley(Pos2::new(rect.left() + d.inset, top), title_g, title_col);
        if let Some(g) = desc_g {
            p.galley(Pos2::new(rect.left() + d.inset, top + h + 2.0), g, desc_col);
        }
        let area = Rect::from_min_max(Pos2::new(rect.right() - d.inset - control_w, rect.top()), Pos2::new(rect.right() - d.inset, rect.bottom()));
        let mut child = self.ui.new_child(UiBuilder::new().max_rect(area).layout(Layout::right_to_left(Align::Center)));
        if !(self.enabled && self.ui.is_enabled()) {
            child.disable();
        }
        control(&mut child)
    }

    /// A row with a switch at its right end.
    pub fn toggle(&mut self, title: &str, description: Option<&str>, on: &mut bool) -> Response {
        {
            let w = switch_size(self.ui.ctx()).x;
            self.row(title, description, w, |ui| switch(ui, on))
        }
    }

    /// A row that holds anything at all, `height` tall, with the usual side padding.
    pub fn custom<R>(&mut self, height: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
        self.hairline_if_needed();
        let d = theme::design(self.ui.ctx());
        let width = self.ui.available_width();
        let (rect, _) = self.ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let mut child = self.ui.new_child(UiBuilder::new().max_rect(rect.shrink2(vec2(d.inset, 0.0))).layout(Layout::left_to_right(Align::Center)));
        add(&mut child)
    }

    /// Room for one more kind of content: a row with a custom body that decides its own height.
    pub fn free<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> R {
        self.hairline_if_needed();
        let d = theme::design(self.ui.ctx());
        egui::Frame::NONE.inner_margin(egui::Margin { left: d.inset as i8, right: d.inset as i8, top: 12, bottom: 12 }).show(self.ui, add).inner
    }
}

/// A title above a group of settings.
pub fn section_title(ui: &mut Ui, text: &str) {
    let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
    ui.add_space(14.0);
    ui.label(RichText::new(text).size(d.title).family(theme::semibold()).color(pal.label));
    ui.add_space(6.0);
}

/// Small grey text under a group, explaining it.
pub fn hint(ui: &mut Ui, text: &str) {
    let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
    ui.add_space(6.0);
    ui.add(egui::Label::new(RichText::new(text).size(d.caption).color(pal.tertiary)).wrap());
}

/// A caption over a field, and the field under it.
pub fn form_field<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
    ui.label(RichText::new(label).size(d.caption).family(theme::semibold()).color(pal.secondary));
    ui.add_space(-2.0);
    let r = add(ui);
    ui.add_space(6.0);
    r
}

/// Width of the label column of `kv` rows.
pub const KV_LABEL_W: f32 = 132.0;

/// A line of facts: a grey label in a fixed-width column, and the value (anything) next to it.
/// Because the label column has a fixed width, the values of a list of rows line up.
pub fn kv<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 26.0;
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(KV_LABEL_W, 26.0), Sense::hover());
            ui.painter().text(rect.left_center(), Align2::LEFT_CENTER, label, FontId::proportional(d.caption + 1.0), pal.tertiary);
            add(ui)
        })
        .inner
    })
    .inner
}

/// A `kv` row whose value is plain text.
pub fn kv_text(ui: &mut Ui, label: &str, value: &str) {
    let pal = theme::current(ui.ctx());
    kv(ui, label, |ui| {
        ui.add(egui::Label::new(RichText::new(value).color(pal.label)).truncate());
    });
}

/// A row that opens and closes a part of the form: a caret and a title. Returns what `add` made
/// while it is open.
pub fn disclosure<R>(ui: &mut Ui, id_salt: impl std::hash::Hash + std::fmt::Debug, title: &str, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let id = ui.make_persistent_id(id_salt);
    let mut open = ctx.data(|data| data.get_temp::<bool>(id)).unwrap_or(false);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    if resp.clicked() {
        open = !open;
    }
    ctx.data_mut(|data| data.insert_temp(id, open));
    let hover = super::anim::fade(&ctx, id.with("hover"), resp.hovered(), 0.1);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_sm), pal.hover.gamma_multiply(hover));
    crate::icons::draw(p, Rect::from_center_size(Pos2::new(rect.left() + 18.0, rect.center().y), vec2(16.0, 16.0)), if open { crate::icons::Icon::CaretDown } else { crate::icons::Icon::CaretRight }, pal.secondary, true);
    p.text(Pos2::new(rect.left() + 34.0, rect.center().y), Align2::LEFT_CENTER, title, FontId::new(d.body, theme::semibold()), pal.label);
    open.then(|| add(ui))
}

/// A message under a field: red for errors, grey for notes. Wraps at the dialog's width.
pub fn message(ui: &mut Ui, text: &str, error: bool) {
    let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
    ui.add(egui::Label::new(RichText::new(text).size(d.caption + 1.0).color(if error { pal.red_ink } else { pal.secondary })).wrap());
}

/// A card-shaped notice: an icon, a line of text, tinted with the hue of the message.
pub fn notice(ui: &mut Ui, icon: crate::icons::Icon, text: &str, tint: egui::Color32, ink: egui::Color32) {
    let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
    let width = ui.available_width();
    let galley = ui.painter().layout(text.to_owned(), FontId::proportional(d.body - 0.5), pal.label, width - 2.0 * 16.0 - 28.0);
    let h = (galley.size().y + 24.0).max(48.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(d.radius_md), tint);
    crate::icons::draw(p, Rect::from_center_size(Pos2::new(rect.left() + 16.0 + 10.0, rect.top() + 12.0 + galley.size().y.min(20.0) / 2.0 + 1.0), vec2(20.0, 20.0)), icon, ink, true);
    p.galley(Pos2::new(rect.left() + 16.0 + 28.0, rect.top() + 12.0), galley, pal.label);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::themed;
    use crate::theme::ThemeMode;

    fn laid_out(width: f32, mut build: impl FnMut(&mut Ui) -> Vec<Rect>) -> Vec<Rect> {
        let ctx = themed(ThemeMode::Light);
        let mut rects = Vec::new();
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 900.0))), ..Default::default() }, |ui| {
                egui::CentralPanel::default().show(ui, |ui| rects = build(ui));
            });
            out.textures_delta.clear();
        }
        rects
    }

    #[test]
    fn rows_are_at_least_56_tall_and_one_described_row_is_taller() {
        let mut on = true;
        let mut also = false;
        let rows = laid_out(640.0, |ui| {
            let mut v = Vec::new();
            card(ui, |g| {
                let a = g.ui.cursor().top();
                g.toggle("One", None, &mut on);
                let b = g.ui.cursor().top();
                g.toggle("Two", Some("A long description that has to wrap onto a second and a third line because the dialog is only so wide, honestly."), &mut also);
                let c = g.ui.cursor().top();
                v.push(Rect::from_min_max(Pos2::new(0.0, a), Pos2::new(1.0, b)));
                v.push(Rect::from_min_max(Pos2::new(0.0, b), Pos2::new(1.0, c)));
            });
            v
        });
        assert_eq!(rows[0].height(), ROW_MIN_H, "a plain row has the minimum height");
        assert!(rows[1].height() > ROW_MIN_H, "a described row grows to fit its text");
    }

    #[test]
    fn the_control_sits_at_the_right_edge_of_every_row() {
        let (mut a, mut b, mut c) = (true, false, true);
        let r = laid_out(600.0, |ui| {
            let mut v = Vec::new();
            card(ui, |g| {
                v.push(g.toggle("First", None, &mut a).rect);
                v.push(g.toggle("Second, with a very long title that wraps around when the room runs out", Some("and a description"), &mut b).rect);
                v.push(g.toggle("Third", None, &mut c).rect);
            });
            v
        });
        for rect in &r {
            assert_eq!(rect.right(), r[0].right(), "every switch ends at the same x: {r:?}");
            assert_eq!(rect.size(), vec2(theme::DESIGN.switch.0, theme::DESIGN.switch.1), "a switch has the size the design gives it");
        }
    }

    #[test]
    fn a_card_is_as_wide_as_the_room_it_is_given() {
        let r = laid_out(500.0, |ui| {
            let w = ui.available_width();
            let before = ui.cursor().min;
            let mut on = true;
            card(ui, |g| {
                g.toggle("Only row", None, &mut on);
            });
            vec![Rect::from_min_size(before, vec2(w, 1.0)), Rect::from_min_max(before, Pos2::new(before.x + ui.min_rect().width(), before.y + 1.0))]
        });
        assert_eq!(r[0].width(), r[1].width());
    }
}
