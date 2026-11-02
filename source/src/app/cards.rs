//! The card view of the downloads, the summary tiles above it, and the empty state.

use super::layout::{card_columns, card_width, Breakpoint, CARD_GAP};
use super::view::row_menu;
use super::*;
use crate::icons::{self, Icon};
use crate::widgets::{anim, capsule_bar, empty_state, icon_button_at, paint_state_chip, spinner, stat_tile, Hue};
use eframe::egui::{Align2, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};

/// Height of one download card.
const CARD_H: f32 = 112.0;
const PAD: f32 = 16.0;
const TILE: f32 = 44.0;

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
        let count = |s: Status| self.items.iter().filter(|i| i.status() == s).count();
        let (speed_text, unit) = split_speed(speed);
        let w = card_width(ui.available_width(), 4);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = CARD_GAP;
            stat_tile(ui, w, "Speed", &speed_text, &unit, pal.blue_ink);
            stat_tile(ui, w, "Active", &active.len().to_string(), "downloads", pal.green_ink);
            stat_tile(ui, w, "Queued", &count(Status::Queued).to_string(), "waiting", pal.label);
            stat_tile(ui, w, "Finished", &count(Status::Complete).to_string(), "done", pal.label);
        });
    }

    /// Shown instead of the list when there is nothing to list.
    pub(super) fn empty_view(&mut self, ui: &mut egui::Ui) {
        let (icon, title, text, action) = if self.items.is_empty() {
            (Icon::AddUrl, "No downloads yet", "Add a link, or copy one: download links on the clipboard are picked up automatically.", Some("Add download"))
        } else if !self.search.trim().is_empty() {
            (Icon::Search, "No matches", "Nothing matches your search. Try fewer words.", None)
        } else {
            (Icon::Folder, "Nothing here", "No downloads in this category yet.", None)
        };
        if empty_state(ui, icon, title, text, action) {
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
            for chunk in rows.chunks(columns) {
                ui.horizontal(|ui| {
                    for &idx in chunk {
                        self.download_card(ui, idx, width, modifiers, &queue_names, &mut ev);
                    }
                });
            }
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
        p.rect_filled(rect, radius, pal.card);
        if d.card_border && !selected {
            p.rect_stroke(rect, radius, Stroke::new(1.0, theme::hairline(&ctx)), StrokeKind::Inside);
        }
        p.rect_filled(rect, radius, pal.hover.gamma_multiply(hover));
        if selected {
            p.rect_filled(rect, radius, pal.blue_tint);
            p.rect_stroke(rect, radius, Stroke::new(1.5, pal.blue), StrokeKind::Inside);
        }

        // File tile: the category colour with the file ending on it.
        let tile = Rect::from_min_size(rect.min + Vec2::splat(PAD), Vec2::splat(TILE));
        let cat = icons::category_color(&item.category);
        p.rect_filled(tile, CornerRadius::same(d.radius_md), cat.gamma_multiply(0.18));
        let ext = item.name.rsplit_once('.').map(|(_, e)| e.to_uppercase()).filter(|e| !e.is_empty() && e.len() <= 4).unwrap_or_else(|| "FILE".into());
        p.text(tile.center(), Align2::CENTER_CENTER, ext, FontId::proportional(d.caption), cat);

        let left = tile.right() + 12.0;
        let right = rect.right() - PAD;
        let (hue, status_word) = match st {
            Status::Downloading => (Hue::Green, "Downloading"),
            Status::Connecting => (Hue::Orange, "Connecting"),
            Status::Paused => (Hue::Orange, "Paused"),
            Status::Complete => (Hue::Blue, "Complete"),
            Status::Error => (Hue::Red, "Error"),
            Status::Queued => (Hue::Neutral, "Queued"),
        };
        let chip_w = paint_state_chip(&p, egui::pos2(right, rect.top() + PAD - 1.0), &pal, &d, hue, status_word);

        let name = truncated(&p, &item.name, FontId::proportional(d.body), pal.label, (right - left - chip_w - 10.0).max(40.0));
        p.galley(egui::pos2(left, rect.top() + PAD), name, pal.label);
        let mut meta = if item.total() > 0 { human_size(item.total()) } else { "Size unknown".to_string() };
        meta.push_str(&format!(" · {}", item.category));
        if let Some(q) = &item.queue {
            meta.push_str(&format!(" · {q}"));
        }
        let meta = truncated(&p, &meta, FontId::proportional(d.caption), pal.tertiary, right - left);
        p.galley(egui::pos2(left, rect.top() + PAD + 24.0), meta, pal.tertiary);

        // Progress.
        let with_bar = st == Status::Downloading || (st == Status::Paused && item.total() > 0);
        if with_bar {
            let bar = Rect::from_min_size(egui::pos2(left, rect.top() + PAD + 50.0), Vec2::new(right - left, 6.0));
            let color = if st == Status::Downloading { pal.green_ink } else { pal.orange_ink };
            // new_child, not scope_builder: a scope would also advance the grid's layout cursor.
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar));
            capsule_bar(&mut child, egui::Id::unique(("card_bar", id)), item.progress(), color, 6.0, None);
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
        let line_y = rect.top() + PAD + if with_bar || st == Status::Connecting { 66.0 } else { 52.0 };
        let button_row = 3.0 * 32.0 + 2.0 * 4.0;
        let line = truncated(&p, &line, FontId::proportional(d.caption), line_color, (right - left - button_row - 8.0).max(60.0));
        p.galley(egui::pos2(left, line_y), line, line_color);
        if st == Status::Connecting {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(egui::pos2(left, rect.top() + PAD + 48.0), Vec2::splat(16.0))));
            spinner(&mut child, 14.0);
        }

        // Actions.
        let primary = match st {
            Status::Downloading | Status::Connecting => (Icon::Pause, "Pause", "stop"),
            Status::Complete => (Icon::File, "Open file", "open"),
            _ => (Icon::Play, "Start or resume", "resume"),
        };
        for (n, (icon, tip, key)) in [primary, (Icon::Folder, "Open folder", "folder"), (Icon::Close, "Remove from the list", "remove")].into_iter().enumerate() {
            let x = right - 32.0 * (3 - n) as f32 - 4.0 * (2 - n) as f32;
            let button = Rect::from_min_size(egui::pos2(x, rect.bottom() - PAD - 28.0), Vec2::splat(32.0));
            if icon_button_at(ui, button, icon, tip, egui::Id::unique(("card_btn", id, key))).clicked() {
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

/// One line of text cut off with an ellipsis when it does not fit `max_width`.
fn truncated(p: &egui::Painter, text: &str, font: FontId, color: egui::Color32, max_width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap.max_width = max_width;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    p.layout_job(job)
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
        let (mode, style) = (crate::theme::ThemeMode::Dark, crate::theme::ThemeStyle::Drishti);
        let mut app = App::build(ctx.clone(), Launch { demo: true, ..Default::default() });
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(width, 900.0))), ..Default::default() };
        let mut cards = Vec::new();
        for _ in 0..3 {
            let mut out = ctx.run_ui(input.clone(), |ui| {
                crate::theme::apply(ui.ctx(), mode, style);
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
}
