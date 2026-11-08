//! The card view of the downloads, the summary tiles above it, and the empty state.

use super::layout::{card_columns, card_width, Breakpoint, CARD_GAP};
use super::view::row_menu;
use super::*;
use crate::icons::{self, Icon};
use crate::theme::Elevation;
use crate::widgets::components::Art;
use crate::widgets::{anim, capsule_bar, elided_tooltip, empty_state, icon_button_at, indeterminate_bar, paint_state_chip, stat_tile, truncated, Hue};
use eframe::egui::{CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};

/// Height of one download card.
pub const CARD_H: f32 = 120.0;
const PAD: f32 = 16.0;
const TILE: f32 = 44.0;
/// Side of the round buttons at the bottom right of a card.
const BUTTON: f32 = 32.0;
/// Gap between the round buttons.
const BUTTON_GAP: f32 = 2.0;
/// Top and height of the progress bar, measured from the top of the card.
const BAR_Y: f32 = 68.0;
const BAR_H: f32 = 6.0;
/// Space under the row of buttons.
const BOTTOM_PAD: f32 = 10.0;

/// The vertical middle of the status line and of the buttons next to it.
fn line_center_y(card: Rect) -> f32 {
    card.bottom() - BOTTOM_PAD - BUTTON / 2.0
}

/// Where the three round buttons of a card go, left to right.
fn button_rects(card: Rect) -> [Rect; 3] {
    let right = card.right() - PAD;
    let cy = line_center_y(card);
    std::array::from_fn(|n| {
        let x = right - BUTTON * (3 - n) as f32 - BUTTON_GAP * (2 - n) as f32;
        Rect::from_center_size(egui::pos2(x + BUTTON / 2.0, cy), Vec2::splat(BUTTON))
    })
}

/// What happened in the cards this frame. The state is changed after drawing.
#[derive(Default)]
struct CardEvents {
    clicked: Option<(u64, egui::Modifiers)>,
    double: Option<u64>,
    action: Option<(u64, String)>,
}

impl App {
    pub(super) fn breakpoint(&self, ctx: &egui::Context) -> Breakpoint {
        Breakpoint::from_width(ctx.content_rect().width())
    }

    /// Four summary tiles: speed, active, queued and finished downloads.
    pub(super) fn stats_row(&self, ui: &mut egui::Ui) {
        let pal = theme::current(ui.ctx());
        let active: Vec<&Item> = self.items.iter().filter(|i| i.status().is_active()).collect();
        let speed: f64 = active.iter().map(|i| i.speed()).sum();
        // The number glides to each new speed instead of jumping, so the tile stays calm.
        let speed = anim::smooth(ui.ctx(), egui::Id::unique("stat_speed"), speed as f32, 0.35).max(0.0) as f64;
        let count = |s: Status| self.items.iter().filter(|i| i.status() == s).count();
        let (speed_text, unit) = split_speed(speed);
        let w = card_width(ui.available_width(), 4);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = CARD_GAP;
            stat_tile(ui, w, Icon::Speed, "Speed", &speed_text, &unit, pal.accent_ink);
            stat_tile(ui, w, Icon::Download, "Active", &active.len().to_string(), "downloading", pal.green_ink);
            stat_tile(ui, w, Icon::Hourglass, "Queued", &count(Status::Queued).to_string(), "waiting", pal.orange_ink);
            stat_tile(ui, w, Icon::CheckCircle, "Finished", &count(Status::Complete).to_string(), "done", pal.blue_ink);
        });
    }

    /// Shown instead of the list when there is nothing to list.
    pub(super) fn empty_view(&mut self, ui: &mut egui::Ui) {
        let (art, title, text, action) = if self.items.is_empty() {
            (Art::Flow, "No downloads yet", "Add a link, or copy one: download links on the clipboard are picked up automatically.", Some("Add download"))
        } else if !self.search.trim().is_empty() {
            (Art::Icon(Icon::Search), "No matches", "Nothing matches your search. Try fewer words.", None)
        } else {
            (Art::Icon(Icon::Folder), "Nothing here", "No downloads in this category yet.", None)
        };
        if empty_state(ui, art, title, text, action) {
            self.open_add_dialog(None);
        }
    }

    /// One card per download, in a grid that adds columns as the window gets wider.
    pub(super) fn cards_view(&mut self, ui: &mut egui::Ui) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            self.empty_view(ui);
            return;
        }
        let columns = card_columns(ui.available_width());
        let width = card_width(ui.available_width(), columns);
        let modifiers = ui.input(|i| i.modifiers);
        let queue_names: Vec<String> = self.settings.queues.iter().map(|q| q.name.clone()).collect();
        let mut ev = CardEvents::default();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(CARD_GAP);
            // Room for the shadows at the top and the bottom.
            ui.add_space(2.0);
            // Only the rows in view are drawn; the others just take their room, so a list of hundreds of downloads
            // costs no more to draw (and to animate) than the few cards that can be seen.
            let view = ui.clip_rect();
            for chunk in rows.chunks(columns) {
                let top = ui.cursor().top();
                if top + CARD_H < view.top() - CARD_H || top > view.bottom() + CARD_H {
                    ui.allocate_space(Vec2::new(ui.available_width(), CARD_H));
                    continue;
                }
                ui.horizontal(|ui| {
                    for &idx in chunk {
                        self.download_card(ui, idx, width, modifiers, &queue_names, &mut ev);
                    }
                });
            }
            ui.add_space(8.0);
        });
        if let Some((id, m)) = ev.clicked {
            self.apply_click(&rows, id, m);
        }
        if let Some(id) = ev.double {
            self.open_details(id);
        }
        if let Some((id, a)) = ev.action {
            self.row_action(ui.ctx(), id, &a);
        }
    }

    fn download_card(&self, ui: &mut egui::Ui, idx: usize, width: f32, modifiers: egui::Modifiers, queue_names: &[String], ev: &mut CardEvents) {
        let item = &self.items[idx];
        let (id, st) = (item.id, item.status());
        let ctx = ui.ctx().clone();
        let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, CARD_H), Sense::click());
        let selected = self.selected.contains(&id);
        let hover = anim::fade(&ctx, resp.id.with("hover"), resp.hovered(), 0.15);
        let radius = CornerRadius::same(d.radius_lg);
        let p = ui.painter().clone();
        // The shadow grows a little under the pointer.
        p.add(theme::shadow(&ctx, Elevation::Rest).as_shape(rect, radius));
        if hover > 0.0 {
            let mut s = theme::shadow(&ctx, Elevation::Hover);
            s.color = s.color.gamma_multiply(hover);
            p.add(s.as_shape(rect, radius));
        }
        p.rect_filled(rect, radius, pal.card);
        if selected {
            p.rect_filled(rect, radius, pal.accent_tint.gamma_multiply(0.22));
            p.rect_stroke(rect, radius, Stroke::new(2.0, pal.accent), StrokeKind::Inside);
        } else {
            p.rect_stroke(rect, radius, Stroke::new(1.0, pal.separator.lerp_to_gamma(pal.separator_strong, hover)), StrokeKind::Inside);
        }

        // File tile: the category colour with the category icon on it.
        let tile = Rect::from_min_size(rect.min + Vec2::splat(PAD), Vec2::splat(TILE));
        icons::file_tile(&p, tile, &item.category);

        let left = tile.right() + 12.0;
        let right = rect.right() - PAD;
        let (hue, status_word) = match st {
            Status::Downloading => (Hue::Accent, "Downloading"),
            Status::Connecting => (Hue::Orange, "Connecting"),
            Status::Paused => (Hue::Orange, "Paused"),
            Status::Complete => (Hue::Green, "Complete"),
            Status::Error => (Hue::Red, "Error"),
            Status::Queued => (Hue::Neutral, "Queued"),
        };
        let chip_w = paint_state_chip(&p, egui::pos2(right, rect.top() + PAD - 2.0), &pal, &d, hue, status_word);

        let name = truncated(&p, &item.name, FontId::new(15.0, theme::semibold()), pal.label, (right - left - chip_w - 10.0).max(40.0));
        elided_tooltip(ui, &name, egui::pos2(left, rect.top() + PAD), &item.name, ("card_name", id));
        p.galley(egui::pos2(left, rect.top() + PAD), name, pal.label);
        let mut meta = if item.total() > 0 { human_size(item.total()) } else { "Size unknown".to_string() };
        meta.push_str(&format!(" · {}", item.category));
        if let Some(q) = &item.queue {
            meta.push_str(&format!(" · {q}"));
        }
        let meta = truncated(&p, &meta, FontId::proportional(d.caption), pal.tertiary, right - left);
        p.galley(egui::pos2(left, rect.top() + PAD + 24.0), meta, pal.tertiary);

        // Progress, the full width of the card.
        // Waiting without an end in sight (connecting, or a size the server does not tell) slides a piece along the bar.
        let unknown_end = st == Status::Connecting || (st == Status::Downloading && item.total() == 0);
        let with_bar = unknown_end || st == Status::Downloading || (st == Status::Paused && item.total() > 0);
        if with_bar {
            let bar = Rect::from_min_size(egui::pos2(rect.left() + PAD, rect.top() + BAR_Y), Vec2::new(rect.width() - 2.0 * PAD, BAR_H));
            let color = if st == Status::Downloading { pal.accent } else { pal.orange };
            // new_child, not scope_builder: a scope would also advance the grid's layout cursor.
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar));
            if unknown_end {
                indeterminate_bar(&mut child, BAR_H);
            } else {
                capsule_bar(&mut child, egui::Id::unique(("card_bar", id)), item.progress(), color, BAR_H, None);
            }
        }

        // The line under the bar: what is happening right now.
        let (line, line_color) = match st {
            Status::Downloading => (format!("{:.0}%  ·  {}  ·  {}", item.progress() * 100.0, human_speed(item.speed()), item.time_left()), pal.secondary),
            Status::Paused if item.total() > 0 => (format!("{:.0}%  ·  Paused", item.progress() * 100.0), pal.secondary),
            Status::Paused => ("Paused".to_string(), pal.secondary),
            Status::Connecting => ("Connecting to the server…".to_string(), pal.secondary),
            Status::Complete => (if item.last_try.is_empty() { "Complete".to_string() } else { format!("Completed  ·  {}", item.last_try) }, pal.secondary),
            Status::Error => (item.live.error.lock().unwrap().clone().unwrap_or_else(|| "Something went wrong".into()), pal.red_ink),
            Status::Queued => (format!("Waiting in {}", item.queue.as_deref().unwrap_or("the list")), pal.secondary),
        };
        let line_cy = line_center_y(rect);
        let button_row = 3.0 * BUTTON + 2.0 * BUTTON_GAP;
        let line_x = rect.left() + PAD;
        let line_text = line.clone();
        let line = truncated(&p, &line, FontId::proportional(d.caption + 0.5), line_color, (right - line_x - button_row - 8.0).max(60.0));
        let line_pos = egui::pos2(line_x, line_cy - line.size().y / 2.0);
        elided_tooltip(ui, &line, line_pos, &line_text, ("card_line", id));
        p.galley(line_pos, line, line_color);

        // Actions.
        let primary = match st {
            Status::Downloading | Status::Connecting => (Icon::Pause, "Pause", "stop"),
            Status::Complete => (Icon::File, "Open file", "open"),
            _ => (Icon::Resume, "Start or resume", "resume"),
        };
        let buttons = button_rects(rect);
        for (n, (icon, tip, key)) in [primary, (Icon::Folder, "Open folder", "folder"), (Icon::Close, "Remove from the list", "remove")].into_iter().enumerate() {
            if icon_button_at(ui, buttons[n], icon, tip, egui::Id::unique(("card_btn", id, key))).clicked() {
                ev.action = Some((id, key.to_string()));
            }
        }

        if resp.clicked() {
            ev.clicked = Some((id, modifiers));
        }
        if resp.double_clicked() {
            ev.double = Some(id);
        }
        if resp.secondary_clicked() && !selected {
            ev.clicked = Some((id, egui::Modifiers::NONE));
        }
        resp.context_menu(|ui| row_menu(ui, id, st, queue_names, &mut ev.action));
    }
}

/// "4.41" and "MB/s" for a speed in bytes per second ("0" and "B/s" when idle).
fn split_speed(bps: f64) -> (String, String) {
    let text = human_speed(bps);
    match text.split_once(' ') {
        Some((n, u)) => (n.to_string(), u.to_string()),
        None => ("0".to_string(), "B/s".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_split_into_a_number_and_a_unit() {
        assert_eq!(split_speed(0.0), ("0".into(), "B/s".into()));
        assert_eq!(split_speed(4_620_000.0), ("4.41".into(), "MB/s".into()));
        assert_eq!(split_speed(900.0), ("900".into(), "B/s".into()));
    }

    /// The rectangles of the painted cards (the shapes of exactly card size and card fill).
    fn painted_cards(width: f32) -> Vec<Rect> {
        let ctx = egui::Context::default();
        let mut app = App::build(ctx.clone(), Launch { demo: true, ..Default::default() });
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(width, 900.0))), ..Default::default() };
        let mut cards = Vec::new();
        for _ in 0..3 {
            let mut out = ctx.run_ui(input.clone(), |ui| {
                crate::theme::apply(ui.ctx(), crate::theme::ThemeMode::Light, crate::theme::ColorTheme::default());
                egui::CentralPanel::default().show(ui, |ui| app.cards_view(ui));
            });
            out.textures_delta.clear();
            let fill = crate::theme::current(&ctx).card;
            cards = out
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Rect(r) if r.fill == fill && (r.rect.height() - CARD_H).abs() < 0.01 => Some(r.rect),
                    _ => None,
                })
                .collect();
        }
        cards
    }

    #[test]
    fn cards_in_a_row_are_a_gap_apart() {
        let cards = painted_cards(1000.0);
        assert!(cards.len() >= 4, "the demo has nine downloads, got {} cards", cards.len());
        let first_row: Vec<&Rect> = cards.iter().filter(|r| (r.top() - cards[0].top()).abs() < 0.5).collect();
        assert_eq!(first_row.len(), 2, "1000 points fit two columns");
        let gap = first_row[1].left() - first_row[0].right();
        assert!((gap - CARD_GAP).abs() < 0.01, "the gap between neighbouring cards is {gap}, expected {CARD_GAP}");
        // And the rows are a gap apart too.
        let below = cards.iter().find(|r| r.top() > cards[0].top() + 1.0).unwrap();
        assert!((below.top() - cards[0].bottom() - CARD_GAP).abs() < 0.01);
    }

    #[test]
    fn the_buttons_sit_inside_the_card_on_one_line_below_the_progress_bar() {
        for width in [440.0, 440.0 * 2.0, 1000.0] {
            let card = Rect::from_min_size(egui::pos2(30.0, 50.0), Vec2::new(width, CARD_H));
            let buttons = button_rects(card);
            let bar = Rect::from_min_size(egui::pos2(card.left() + PAD, card.top() + BAR_Y), Vec2::new(card.width() - 2.0 * PAD, BAR_H));
            for (n, b) in buttons.iter().enumerate() {
                assert!(card.contains_rect(*b), "button {n} {b:?} sticks out of {card:?}");
                assert!(b.top() >= bar.bottom() + 4.0, "button {n} {b:?} touches the progress bar {bar:?}");
                assert_eq!(b.center().y, buttons[0].center().y, "the buttons share a line");
                assert_eq!(b.size(), Vec2::splat(BUTTON));
            }
            for pair in buttons.windows(2) {
                assert_eq!(pair[1].left() - pair[0].right(), BUTTON_GAP, "an even gap between the buttons");
            }
            assert_eq!(card.right() - buttons[2].right(), PAD, "the last button is a card padding from the edge");
            assert!(buttons[0].left() > card.left() + PAD + TILE, "the buttons never reach over the file tile");
            assert!(card.bottom() - buttons[0].bottom() >= BOTTOM_PAD - 0.001);
        }
    }

    #[test]
    fn the_status_line_shares_its_centre_with_the_buttons() {
        let card = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(500.0, CARD_H));
        assert_eq!(line_center_y(card), button_rects(card)[1].center().y);
    }
}
