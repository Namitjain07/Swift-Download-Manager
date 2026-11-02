//! Main window: menu, toolbar, categories, download list and status bar.

use super::*;
use super::mcp_cmds::AiState;
use crate::icons::{self, Icon};
use crate::theme::ThemeStyle;
use super::layout;
use crate::widgets::{anim, capsule_bar, icon_button, icon_segmented, rail_row, sidebar_pill, sidebar_row, spinner, state_chip, status_chip, tool_button, tool_button_icon, toolbar_sep, Hue, RowIcon};
use eframe::egui::{RichText, Sense, Stroke};
use egui_extras::{Column, TableBuilder};

impl eframe::App for App {
    /// Closing the window keeps everything: an MCP client that is still attached takes over.
    fn on_exit(&mut self) {
        self.save();
        self.close_control();
        // Let go of the download list so an MCP engine can pick it up at once.
        self.state_lock = None;
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame += 1;
        let ctx = ui.ctx().clone();
        theme::apply(&ctx, self.theme_override.unwrap_or(self.settings.theme), self.style_override.unwrap_or(self.settings.theme_style));
        if !self.demo && !self.quitting && ctx.input(|i| i.viewport().close_requested()) {
            self.on_close_requested(&ctx);
        }
        self.tick();
        self.handle_screenshot(&ctx);
        self.shortcuts(&ctx);

        let bp = self.breakpoint(&ctx);
        self.menu_bar(ui);
        self.toolbar(ui, bp);
        self.status_bar(ui, bp);
        if self.show_categories {
            self.categories_panel(ui, bp);
        }
        let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
        let bp = self.breakpoint(&ctx);
        let cards = self.settings.view == layout::ViewMode::Cards || bp == layout::Breakpoint::Compact;
        // The list floats on a rounded surface above the page ground (Drishti), or fills the window (iOS table).
        let floating = d.floating_list || cards;
        let margin = if bp == layout::Breakpoint::Compact { 10 } else { 16 };
        let page = if floating { egui::Frame::NONE.fill(pal.bg).inner_margin(egui::Margin::same(margin)) } else { egui::Frame::NONE.fill(pal.card) };
        egui::CentralPanel::default().frame(page).show(ui, |ui| {
            if bp != layout::Breakpoint::Compact && !self.items.is_empty() {
                self.stats_row(ui);
                ui.add_space(layout::CARD_GAP);
            }
            if cards {
                self.cards_view(ui);
            } else {
                self.table(ui);
            }
        });

        self.add_window(&ctx);
        self.batch_window(&ctx);
        self.grabber_window(&ctx);
        self.refresh_window(&ctx);
        self.queues_window(&ctx);
        self.progress_windows(&ctx);
        self.complete_windows(&ctx);
        self.options_window(&ctx);
        self.scheduler_window(&ctx);
        self.delete_window(&ctx);
        self.properties_window(&ctx);
        self.browser_window(&ctx);
        self.ai_window(&ctx);
        self.about_window(&ctx);
        self.close_window(&ctx);
        self.toast_area(&ctx);
        self.drop_files(&ctx);
    }
}

impl App {
    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        ctx.request_repaint();
        if shot.saved.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.frame >= 120 && !shot.requested {
            shot.requested = true;
            let (path, saved, c2) = (shot.path.clone(), shot.saved.clone(), ctx.clone());
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let mut bytes = Vec::with_capacity(w * h * 4);
                for px in &img.pixels {
                    let [r, g, b, _] = px.to_array();
                    bytes.extend_from_slice(&[r, g, b, 255]);
                }
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                match image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                    Some(im) => {
                        if let Err(e) = im.save(&path) {
                            eprintln!("could not save screenshot: {e}");
                        }
                    }
                    None => eprintln!("screenshot had an unexpected size"),
                }
                saved.store(true, Ordering::SeqCst);
                c2.request_repaint();
            });
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::G)) {
            self.open_grabber();
        }
        let (new, del, all, batch) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::N),
                i.key_pressed(egui::Key::Delete),
                i.modifiers.command && i.key_pressed(egui::Key::A),
                i.modifiers.command && i.key_pressed(egui::Key::B),
            )
        });
        if new {
            self.open_add_dialog(None);
        }
        if batch {
            self.open_batch();
        }
        if del {
            self.ask_delete(self.selected_ids());
        }
        if all {
            let ids: Vec<u64> = self.visible_rows().into_iter().map(|r| self.items[r].id).collect();
            self.selected.extend(ids);
        }
    }

    /// Link lists, shortcuts or saved web pages dropped onto the window.
    fn drop_files(&mut self, ctx: &egui::Context) {
        let (hovering, dropped) = ctx.input(|i| (!i.raw.hovered_files.is_empty(), i.raw.dropped_files.clone()));
        let k = anim::fade(ctx, egui::Id::unique("drop_hint_fade"), hovering, 0.15);
        if k > 0.0 {
            let pal = theme::current(ctx);
            egui::Area::new(egui::Id::unique("drop_hint")).anchor(egui::Align2::CENTER_CENTER, [0.0, (1.0 - k) * 8.0]).interactable(false).show(ctx, |ui| {
                ui.multiply_opacity(k);
                egui::Frame::NONE.fill(pal.blue).corner_radius(16).inner_margin(egui::Margin::symmetric(28, 20)).show(ui, |ui| {
                    ui.label(RichText::new("Drop to download the links inside").color(Color32::WHITE).size(17.0));
                });
            });
        }
        let paths: Vec<PathBuf> = dropped.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect();
        if !paths.is_empty() {
            self.files_dropped(paths);
        }
    }

    pub(super) fn open_scheduler(&mut self, queue: &str) {
        if let Some(q) = self.settings.queues.iter().find(|q| q.name == queue) {
            self.scheduler = Some((q.name.clone(), q.schedule.clone()));
        }
    }

    pub(super) fn open_batch(&mut self) {
        self.batch = Some(BatchDialog {
            queue: self.current_queue(),
            folder: self.settings.folder.display().to_string(),
            from: 1,
            to: 10,
            width: 1,
            pattern: "https://example.com/photos/img*.jpg".into(),
            ..Default::default()
        });
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Tasks", |ui| {
                    if ui.button("Add new download…  (Ctrl+N)").clicked() {
                        self.open_add_dialog(None);
                    }
                    if ui.button("Add batch download…  (Ctrl+B)").clicked() {
                        self.open_batch();
                    }
                    if ui.button("Grab links from a web page…  (Ctrl+G)").clicked() {
                        self.open_grabber();
                    }
                    ui.separator();
                    if ui.button("Import links from a text file…").clicked() {
                        self.import_links();
                    }
                    if ui.add_enabled(!self.items.is_empty(), egui::Button::new("Export all links to a text file…")).clicked() {
                        self.export_links();
                    }
                    ui.separator();
                    if ui.button("Keep running in the background").on_hover_text("Close the window and carry on downloading; open it again from the tray icon").clicked() {
                        let ctx = ui.ctx().clone();
                        self.go_background(&ctx);
                    }
                    if ui.button("Exit").clicked() {
                        // Exit really quits, whatever the setting for the close button says.
                        let ctx = ui.ctx().clone();
                        self.quit_now(&ctx);
                    }
                });
                ui.menu_button("File", |ui| {
                    let sel = self.selected_ids();
                    let one = if sel.len() == 1 { self.items.iter().find(|i| i.id == sel[0]) } else { None };
                    let one_done = one.is_some_and(|i| i.status() == Status::Complete);
                    if ui.add_enabled(one_done, egui::Button::new("Open file")).clicked() {
                        if let Some(i) = one {
                            open_path(&i.path());
                        }
                    }
                    if ui.add_enabled(one.is_some(), egui::Button::new("Open folder")).clicked() {
                        if let Some(i) = one {
                            open_path(&i.folder);
                        }
                    }
                    if ui.add_enabled(one.is_some(), egui::Button::new("Properties")).clicked() {
                        self.properties = one.map(|i| i.id);
                    }
                    ui.separator();
                    if ui.add_enabled(!sel.is_empty(), egui::Button::new("Delete…")).clicked() {
                        self.ask_delete(sel.clone());
                    }
                });
                ui.menu_button("Downloads", |ui| {
                    if ui.button("Resume").clicked() {
                        for id in self.selected_ids() {
                            self.start_item(id);
                        }
                    }
                    if ui.button("Stop").clicked() {
                        for id in self.selected_ids() {
                            self.stop_item(id);
                        }
                    }
                    if ui.button("Stop all").clicked() {
                        self.stop_all();
                    }
                    if ui.button("Delete all completed").clicked() {
                        self.delete_completed();
                    }
                    ui.separator();
                    let names: Vec<String> = self.settings.queues.iter().map(|q| q.name.clone()).collect();
                    ui.menu_button("Start queue", |ui| {
                        for q in &names {
                            if ui.add_enabled(!self.running.contains(q), egui::Button::new(q)).clicked() {
                                self.start_queue(q);
                            }
                        }
                    });
                    ui.menu_button("Stop queue", |ui| {
                        for q in &names {
                            if ui.add_enabled(self.running.contains(q), egui::Button::new(q)).clicked() {
                                self.stop_queue(q);
                            }
                        }
                    });
                    if ui.button("Scheduler…").clicked() {
                        let q = self.current_queue();
                        self.open_scheduler(&q);
                    }
                    if ui.button("Manage queues…").clicked() {
                        self.queues_open = true;
                    }
                    ui.separator();
                    if ui.checkbox(&mut self.settings.limit_on, format!("Speed limiter ({} KB/s)", self.settings.limit_kbps)).changed() {
                        self.apply_settings();
                        self.dirty = true;
                    }
                    if ui.checkbox(&mut self.settings.watch_clipboard, "Watch the clipboard for links").changed() {
                        self.apply_settings();
                        self.dirty = true;
                    }
                    ui.separator();
                    if ui.button("Options…").clicked() {
                        self.options = Some(self.settings.clone());
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_categories, "Show categories");
                    ui.menu_button("Show downloads as", |ui| {
                        for (mode, name) in [(layout::ViewMode::Cards, "Cards"), (layout::ViewMode::Table, "Table")] {
                            if ui.selectable_value(&mut self.settings.view, mode, name).clicked() {
                                self.dirty = true;
                            }
                        }
                    });
                    ui.menu_button("Appearance", |ui| {
                        ui.label(RichText::new("DESIGN").size(11.0).color(theme::current(ui.ctx()).tertiary));
                        for (style, name) in [(ThemeStyle::Drishti, "Drishti (default)"), (ThemeStyle::Ios, "iOS")] {
                            if ui.selectable_value(&mut self.settings.theme_style, style, name).clicked() {
                                self.dirty = true;
                            }
                        }
                        ui.separator();
                        ui.label(RichText::new("MODE").size(11.0).color(theme::current(ui.ctx()).tertiary));
                        for (mode, name) in [(ThemeMode::System, "Automatic (follow system)"), (ThemeMode::Light, "Light"), (ThemeMode::Dark, "Dark")] {
                            if ui.selectable_value(&mut self.settings.theme, mode, name).clicked() {
                                self.dirty = true;
                            }
                        }
                    });
                    if ui.button("Clear sorting").clicked() {
                        self.sort = None;
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Browser integration…").clicked() {
                        self.browser_open = true;
                    }
                    if ui.button("AI assistants…").clicked() {
                        self.ai_open = true;
                    }
                    if ui.button("About").clicked() {
                        self.about_open = true;
                    }
                });
            });
        });
    }

    /// The tools, in groups. Whether each one is available depends on the selection and the queues.
    fn tool_groups(&self) -> Vec<Vec<Tool>> {
        let sel = self.selected_ids();
        let sel_has = |f: &dyn Fn(Status) -> bool| sel.iter().any(|id| self.items.iter().any(|i| i.id == *id && f(i.status())));
        let can_resume = sel_has(&|s| matches!(s, Status::Paused | Status::Error | Status::Queued));
        let can_stop = sel_has(&|s| s.is_active() || s == Status::Queued);
        let any_active = self.items.iter().any(|i| i.status().is_active());
        let any_done = self.items.iter().any(|i| i.status() == Status::Complete);
        let cq = self.current_queue();
        let any_queued = self.items.iter().any(|i| i.status() == Status::Queued && i.queue.as_deref() == Some(cq.as_str()));
        let cq_running = self.running.contains(&cq);
        let t = |key, icon, label, tip: &str, enabled| Tool { key, icon, label, tip: tip.to_string(), enabled };
        vec![
            vec![t(ToolKey::Add, Icon::AddUrl, "Add URL", "Add a new download (Ctrl+N)", true), t(ToolKey::Batch, Icon::Batch, "Batch", "Add many links at once (Ctrl+B)", true)],
            vec![
                t(ToolKey::Resume, Icon::Resume, "Resume", "Start or resume the selected downloads", can_resume),
                t(ToolKey::Stop, Icon::Stop, "Stop", "Pause the selected downloads", can_stop),
                t(ToolKey::StopAll, Icon::StopAll, "Stop All", "Pause every running download", any_active),
            ],
            vec![t(ToolKey::Delete, Icon::Delete, "Delete", "Remove the selected downloads", !sel.is_empty()), t(ToolKey::ClearDone, Icon::DeleteDone, "Clear Done", "Remove all completed downloads from the list", any_done)],
            vec![
                t(ToolKey::Options, Icon::Options, "Options", "Folder, connections, speed limit, proxy, browser", true),
                t(ToolKey::Scheduler, Icon::Scheduler, "Scheduler", &format!("Start and stop \"{cq}\" at set times"), true),
            ],
            vec![
                t(ToolKey::StartQueue, Icon::StartQueue, "Start Queue", &format!("Start \"{cq}\" (pick another queue on the left)"), any_queued && !cq_running),
                t(ToolKey::StopQueue, Icon::StopQueue, "Stop Queue", &format!("Stop \"{cq}\""), cq_running),
            ],
        ]
    }

    fn run_tool(&mut self, key: ToolKey) {
        let sel = self.selected_ids();
        let cq = self.current_queue();
        match key {
            ToolKey::Add => self.open_add_dialog(None),
            ToolKey::Batch => self.open_batch(),
            ToolKey::Resume => sel.iter().for_each(|id| self.start_item(*id)),
            ToolKey::Stop => sel.iter().for_each(|id| self.stop_item(*id)),
            ToolKey::StopAll => self.stop_all(),
            ToolKey::Delete => self.ask_delete(sel),
            ToolKey::ClearDone => self.delete_completed(),
            ToolKey::Options => self.options = Some(self.settings.clone()),
            ToolKey::Scheduler => self.open_scheduler(&cq),
            ToolKey::StartQueue => self.start_queue(&cq),
            ToolKey::StopQueue => self.stop_queue(&cq),
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, bp: layout::Breakpoint) {
        use layout::Breakpoint::{Compact, Medium, Wide};
        let groups = self.tool_groups();
        let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
        let margin = if bp == Compact { egui::Margin::symmetric(10, 6) } else if d.floating_list { egui::Margin::symmetric(16, 8) } else { egui::Margin::symmetric(10, 6) };
        let frame = if d.floating_list { egui::Frame::NONE.fill(pal.bg).inner_margin(margin) } else { egui::Frame::NONE.fill(pal.card).inner_margin(margin) };
        let mut chosen: Option<ToolKey> = None;
        let mut drawer = false;
        let mut view = self.settings.view;
        egui::Panel::top("toolbar").frame(frame).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                match bp {
                    Wide | Medium => {
                        for (n, group) in groups.iter().enumerate() {
                            if n > 0 {
                                toolbar_sep(ui);
                            }
                            for t in group {
                                let hit = if bp == Wide { tool_button(ui, t.icon, t.label, t.enabled, &t.tip) } else { tool_button_icon(ui, t.icon, t.enabled, &t.tip) };
                                if hit {
                                    chosen = Some(t.key);
                                }
                            }
                        }
                    }
                    Compact => {
                        if icon_button(ui, Icon::Menu, "Categories and queues").clicked() {
                            drawer = true;
                        }
                        let all: Vec<&Tool> = groups.iter().flatten().collect();
                        for t in all.iter().filter(|t| matches!(t.key, ToolKey::Add | ToolKey::Resume | ToolKey::Stop)) {
                            if tool_button_icon(ui, t.icon, t.enabled, &t.tip) {
                                chosen = Some(t.key);
                            }
                        }
                        let more = icon_button(ui, Icon::More, "More actions");
                        egui::Popup::menu(&more).show(|ui| {
                            for t in all.iter().filter(|t| !matches!(t.key, ToolKey::Add | ToolKey::Resume | ToolKey::Stop)) {
                                if ui.add_enabled(t.enabled, egui::Button::new(t.label)).on_hover_text(&t.tip).clicked() {
                                    chosen = Some(t.key);
                                    ui.close();
                                }
                            }
                        });
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if bp != Compact {
                        icon_segmented(ui, &[(layout::ViewMode::Cards, Icon::Cards, "Showing cards"), (layout::ViewMode::Table, Icon::List, "Showing the table")], &mut view);
                        ui.add_space(6.0);
                    }
                    if !self.search.is_empty() && icon_button(ui, Icon::Close, "Clear search").clicked() {
                        self.search.clear();
                    }
                    let w = if bp == Wide { 170.0 } else { (ui.available_width() - 8.0).clamp(90.0, 170.0) };
                    ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search downloads").desired_width(w));
                });
            });
        });
        if drawer {
            self.drawer_open = !self.drawer_open;
        }
        if view != self.settings.view {
            self.settings.view = view;
            self.dirty = true;
        }
        if let Some(k) = chosen {
            self.run_tool(k);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, bp: layout::Breakpoint) {
        let pal = theme::current(ui.ctx());
        let active = self.items.iter().filter(|i| i.status().is_active()).count();
        let speed: f64 = self.items.iter().filter(|i| i.status().is_active()).map(|i| i.speed()).sum();
        let queued = self.items.iter().filter(|i| i.status() == Status::Queued).count();
        let scheduled: Option<(String, String)> = self.settings.queues.iter().find(|q| q.schedule.start_on).map(|q| (q.name.clone(), q.schedule.start_time.clone()));
        let running: Vec<String> = self.running.iter().cloned().collect();
        egui::Panel::bottom("status")
            .frame(egui::Frame::NONE.fill(pal.bg).inner_margin(egui::Margin::symmetric(14, 5)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    let compact = bp == layout::Breakpoint::Compact;
                    if !compact {
                        status_chip(ui, None, &format!("{} downloads", self.items.len()), pal.secondary);
                    }
                    status_chip(ui, (active > 0).then_some(pal.green), &format!("{active} active"), pal.secondary);
                    if !compact {
                        status_chip(ui, None, &format!("{queued} queued"), pal.secondary);
                    }
                    if bp == layout::Breakpoint::Wide && !running.is_empty() {
                        status_chip(ui, Some(pal.green), &format!("Running: {}", running.join(", ")), pal.green_ink);
                    }
                    if let (Some((q, t)), true) = (&scheduled, bp == layout::Breakpoint::Wide) {
                        status_chip(ui, Some(pal.blue), &format!("{q} starts at {t}"), pal.blue_ink);
                    }
                    if self.settings.limit_on && bp != layout::Breakpoint::Compact {
                        status_chip(ui, Some(pal.orange), &format!("Speed limit {} KB/s", self.settings.limit_kbps), pal.orange_ink);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let s = if speed > 0.0 { human_speed(speed) } else { "0 B/s".into() };
                        ui.label(RichText::new(format!("↓ {s}")).size(11.0).color(pal.label));
                        if bp == layout::Breakpoint::Compact {
                            // Only the essentials fit.
                        } else if !self.demo {
                            let (txt, dot, col) = match &self.listener {
                                Ok(()) => ("Browser: connected", Some(pal.green), pal.secondary),
                                Err(_) if !self.settings.browser_listen => ("Browser: off", None, pal.secondary),
                                Err(_) => ("Browser: unavailable", Some(pal.red), pal.red_ink),
                            };
                            if status_chip(ui, dot, txt, col).on_hover_text("Browser integration…").clicked() {
                                self.browser_open = true;
                            }
                        } else {
                            status_chip(ui, Some(pal.green), "Browser: connected", pal.secondary);
                        }
                        if bp != layout::Breakpoint::Compact {
                            let (txt, dot, col) = match self.ai_state() {
                            AiState::Off => ("AI: off", None, pal.secondary),
                            AiState::Unavailable => ("AI: unavailable", Some(pal.orange), pal.orange_ink),
                            AiState::Ready => ("AI: ready", Some(pal.green), pal.secondary),
                            AiState::Active => ("AI: active", Some(pal.blue), pal.blue_ink),
                        };
                            if status_chip(ui, dot, txt, col).on_hover_text("AI assistants (MCP)…").clicked() {
                                self.ai_open = true;
                            }
                        }
                        if self.settings.watch_clipboard && bp == layout::Breakpoint::Wide {
                            status_chip(ui, None, "Clipboard: watching", pal.secondary);
                        }
                    });
                });
            });
    }

    fn categories_panel(&mut self, ui: &mut egui::Ui, bp: layout::Breakpoint) {
        let rail_mode = bp == layout::Breakpoint::Medium;
        if bp == layout::Breakpoint::Compact && !self.drawer_open {
            return;
        }
        let count = |f: &dyn Fn(&Item) -> bool| self.items.iter().filter(|i| f(i)).count();
        let all = self.items.len();
        let cats: Vec<(String, usize)> = self.settings.categories.iter().map(|c| (c.name.clone(), count(&|i: &Item| i.category == c.name))).collect();
        let unfinished = count(&|i: &Item| i.status() != Status::Complete);
        let finished = count(&|i: &Item| i.status() == Status::Complete);
        let queues: Vec<(String, usize)> = self
            .settings
            .queues
            .iter()
            .map(|q| (q.name.clone(), count(&|i: &Item| i.queue.as_deref() == Some(q.name.as_str()) && i.status() != Status::Complete)))
            .collect();

        let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
        let panel = egui::Panel::left("categories").show_separator_line(!d.floating_list);
        let panel = if rail_mode { panel.resizable(false).exact_size(60.0) } else { panel.resizable(true).default_size(if bp == layout::Breakpoint::Compact { 230.0 } else { d.sidebar_w }) };
        let rail = panel
            .frame(egui::Frame::NONE.fill(if d.floating_list { pal.sidebar } else { pal.bg }).inner_margin(egui::Margin::symmetric(if rail_mode { 8 } else { 8 }, 10)))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                sidebar_pill(ui);
                if !rail_mode {
                    section_header(ui, "Categories", &pal);
                }
                let mut new_filter = None;
                if side_row(ui, rail_mode, "All Downloads", all, 0.0, RowIcon::Folder, pal.blue_ink, self.filter == Filter::All).clicked() {
                    new_filter = Some(Filter::All);
                }
                for (c, n) in &cats {
                    let r = side_row(ui, rail_mode, c, *n, 14.0, RowIcon::Folder, icons::category_color(c), self.filter == Filter::Category(c.clone()));
                    if r.clicked() {
                        new_filter = Some(Filter::Category(c.clone()));
                    }
                    r.context_menu(|ui| {
                        if ui.button("Edit categories…").clicked() {
                            self.options = Some(self.settings.clone());
                        }
                    });
                }
                ui.add_space(4.0);
                if side_row(ui, rail_mode, "Unfinished", unfinished, 0.0, RowIcon::Folder, pal.orange_ink, self.filter == Filter::Unfinished).clicked() {
                    new_filter = Some(Filter::Unfinished);
                }
                if side_row(ui, rail_mode, "Finished", finished, 0.0, RowIcon::Folder, pal.green_ink, self.filter == Filter::Finished).clicked() {
                    new_filter = Some(Filter::Finished);
                }
                ui.add_space(10.0);
                if rail_mode {
                    ui.separator();
                } else {
                    ui.horizontal(|ui| {
                        section_header(ui, "Queues", &pal);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("+").on_hover_text("Manage queues").clicked() {
                                self.queues_open = true;
                            }
                        });
                    });
                }
                for (q, n) in &queues {
                    let on = self.running.contains(q);
                    let col = if on { pal.green } else { pal.secondary };
                    let r = side_row(ui, rail_mode, q, *n, 0.0, RowIcon::Dot, col, self.filter == Filter::Queue(q.clone()));
                    if r.clicked() {
                        new_filter = Some(Filter::Queue(q.clone()));
                    }
                    r.context_menu(|ui| {
                        if ui.add_enabled(!on, egui::Button::new("Start")).clicked() {
                            self.start_queue(q);
                        }
                        if ui.add_enabled(on, egui::Button::new("Stop")).clicked() {
                            self.stop_queue(q);
                        }
                        if ui.button("Scheduler…").clicked() {
                            self.open_scheduler(q);
                        }
                        if ui.button("Manage queues…").clicked() {
                            self.queues_open = true;
                        }
                    });
                }
                if let Some(f) = new_filter {
                    self.filter = f;
                    if bp == layout::Breakpoint::Compact {
                        self.drawer_open = false;
                    }
                }
            });
        if d.floating_list {
            // The rail's edge has to read as an edge: separator-strong, 1 px.
            let r = rail.response.rect;
            ui.painter().vline(r.right() - 0.5, r.y_range(), Stroke::new(1.0, pal.separator_strong));
        }
    }

    fn table(&mut self, ui: &mut egui::Ui) {
        let d = theme::design(ui.ctx());
        if d.floating_list {
            let pal = theme::current(ui.ctx());
            egui::Frame::NONE.fill(pal.card).corner_radius(egui::CornerRadius::same(d.radius_lg)).inner_margin(egui::Margin::symmetric(0, 6)).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                self.table_inner(ui);
            });
        } else {
            self.table_inner(ui);
        }
    }

    fn table_inner(&mut self, ui: &mut egui::Ui) {
        let d = theme::design(ui.ctx());
        let rows = self.visible_rows();
        if rows.is_empty() {
            self.empty_view(ui);
            return;
        }
        let mut clicked: Option<(u64, egui::Modifiers)> = None;
        let mut double: Option<u64> = None;
        let mut action: Option<(u64, String)> = None;
        let queue_names: Vec<String> = self.settings.queues.iter().map(|q| q.name.clone()).collect();
        let mut sort_click: Option<SortCol> = None;
        let modifiers = ui.input(|i| i.modifiers);
        let sort = self.sort;
        let pal = theme::current(ui.ctx());
        let hair = Stroke::new(1.0, theme::hairline(ui.ctx()));
        // Row marks are clipped to the table's own area (the card's rounded rectangle in Drishti).
        let area = ui.max_rect();
        let painter = ui.painter().with_clip_rect(area);
        // Column dividers only show while hovered or dragged, like a native list.
        ui.visuals_mut().widgets.noninteractive.bg_stroke = Stroke::NONE;
        ui.visuals_mut().widgets.hovered.bg_stroke = Stroke::new(1.0, pal.separator);
        ui.visuals_mut().widgets.active.bg_stroke = Stroke::new(1.0, pal.blue);

        TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(if d.floating_list { 220.0 } else { 300.0 }).at_least(120.0).clip(true))
            .column(Column::initial(80.0).at_least(50.0))
            .column(Column::initial(if d.state_chips { 160.0 } else { 120.0 }).at_least(60.0))
            .column(Column::initial(95.0).at_least(50.0))
            .column(Column::initial(100.0).at_least(50.0))
            .column(Column::initial(if d.floating_list { 150.0 } else { 125.0 }).at_least(60.0))
            .column(Column::remainder().at_least(60.0).clip(true))
            .header(d.control_h.max(28.0), |mut h| {
                let cols = [
                    ("File Name", SortCol::Name),
                    ("Size", SortCol::Size),
                    ("Status", SortCol::Status),
                    ("Time left", SortCol::TimeLeft),
                    ("Transfer rate", SortCol::Speed),
                    ("Last Try Date", SortCol::LastTry),
                    ("Description", SortCol::Description),
                ];
                for (t, c) in cols {
                    h.col(|ui| {
                        let arrow = match sort {
                            Some((sc, true)) if sc == c => " ▲",
                            Some((sc, false)) if sc == c => " ▼",
                            _ => "",
                        };
                        let r = ui.add(egui::Label::new(RichText::new(format!("{}{arrow}", if d.style == ThemeStyle::Drishti { t.to_uppercase() } else { t.to_string() })).size(d.overline).color(pal.tertiary)).sense(Sense::click()));
                        if r.on_hover_text("Click to sort").clicked() {
                            sort_click = Some(c);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(d.row_h, rows.len(), |mut row| {
                    let item = &self.items[rows[row.index()]];
                    row.set_selected(!d.floating_list && self.selected.contains(&item.id));
                    let st = item.status();
                    row.col(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::hover());
                        icons::file_badge(ui.painter(), r, &item.category);
                        ui.label(&item.name);
                    });
                    row.col(|ui| {
                        let t = item.total();
                        ui.label(if t > 0 { human_size(t) } else { String::new() });
                    });
                    row.col(|ui| {
                        let (hue, dot, bar, text_col) = match st {
                            Status::Downloading => (Hue::Green, pal.green, pal.green_ink, pal.label),
                            Status::Connecting | Status::Paused => (Hue::Orange, pal.orange, pal.orange_ink, pal.label),
                            Status::Complete => (Hue::Blue, pal.blue, pal.blue_ink, pal.secondary),
                            Status::Error => (Hue::Red, pal.red, pal.red_ink, pal.red_ink),
                            Status::Queued => (Hue::Neutral, pal.tertiary, pal.secondary, pal.secondary),
                        };
                        let with_bar = st == Status::Downloading || (st == Status::Paused && item.total() > 0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            ui.set_max_width(if d.state_chips { 150.0 } else { 112.0 });
                            if st == Status::Connecting {
                                ui.horizontal(|ui| {
                                    spinner(ui, 13.0);
                                    ui.label(RichText::new(item.status_text()).size(d.caption).color(text_col));
                                });
                            } else if d.state_chips {
                                ui.horizontal(|ui| {
                                    state_chip(ui, hue, st.label());
                                    if with_bar {
                                        ui.label(RichText::new(format!("{:.0}%", item.progress() * 100.0)).size(d.caption).color(pal.secondary));
                                    }
                                });
                            } else {
                                status_chip(ui, Some(dot), &item.status_text(), text_col);
                            }
                            if with_bar {
                                capsule_bar(ui, egui::Id::unique(("row_bar", item.id)), item.progress(), bar, 5.0, None);
                            }
                        });
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(item.time_left()).color(pal.secondary));
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(if st == Status::Downloading { human_speed(item.speed()) } else { String::new() }).color(pal.secondary));
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(&item.last_try).color(pal.tertiary));
                    });
                    row.col(|ui| {
                        let desc = if st == Status::Error {
                            item.live.error.lock().unwrap().clone().unwrap_or_default()
                        } else if let Some(c) = &item.check {
                            c.clone()
                        } else {
                            item.description.clone()
                        };
                        ui.label(RichText::new(desc).color(pal.tertiary));
                    });
                    let resp = row.response();
                    let span = egui::Rangef::new(resp.rect.left().max(area.left()), resp.rect.right().min(area.right()));
                    if d.floating_list {
                        let inner = egui::Rect::from_x_y_ranges(span.shrink(6.0), (resp.rect.top() + 2.0)..=(resp.rect.bottom() - 2.0));
                        if self.selected.contains(&item.id) {
                            painter.rect_filled(inner, egui::CornerRadius::same(d.radius_md), pal.blue_tint);
                        } else if resp.hovered() {
                            painter.rect_filled(inner, egui::CornerRadius::same(d.radius_md), pal.hover);
                        }
                    }
                    painter.hline(span.shrink(d.inset), resp.rect.bottom(), hair);
                    let id = item.id;
                    if resp.clicked() {
                        clicked = Some((id, modifiers));
                    }
                    if resp.double_clicked() {
                        double = Some(id);
                    }
                    if resp.secondary_clicked() && !self.selected.contains(&id) {
                        clicked = Some((id, egui::Modifiers::NONE));
                    }
                    resp.context_menu(|ui| row_menu(ui, id, st, &queue_names, &mut action));
                });
            });

        if let Some(c) = sort_click {
            self.sort = match self.sort {
                Some((sc, true)) if sc == c => Some((c, false)),
                Some((sc, false)) if sc == c => None,
                _ => Some((c, true)),
            };
        }
        if let Some((id, m)) = clicked {
            self.apply_click(&rows, id, m);
        }
        if let Some(id) = double {
            self.open_details(id);
        }
        if let Some((id, a)) = action {
            self.row_action(ui.ctx(), id, &a);
        }
    }

    /// A click on a row or card: plain click selects it, Ctrl toggles, Shift selects a range.
    pub(super) fn apply_click(&mut self, rows: &[usize], id: u64, m: egui::Modifiers) {
        if m.command {
            if !self.selected.remove(&id) {
                self.selected.insert(id);
            }
        } else if m.shift && !self.selected.is_empty() {
            let pos = |x: u64| rows.iter().position(|&r| self.items[r].id == x);
            let anchor = self.selected.iter().filter_map(|&s| pos(s)).min().unwrap_or(0);
            if let Some(p) = pos(id) {
                let (a, b) = if anchor <= p { (anchor, p) } else { (p, anchor) };
                self.selected = rows[a..=b].iter().map(|&r| self.items[r].id).collect();
            }
        } else {
            self.selected.clear();
            self.selected.insert(id);
        }
    }

    /// Double click: finished downloads show their properties, the others their progress.
    pub(super) fn open_details(&mut self, id: u64) {
        if self.items.iter().any(|i| i.id == id && i.status() == Status::Complete) {
            self.properties = Some(id);
        } else {
            self.progress_open.insert(id);
        }
    }

    pub(super) fn row_action(&mut self, ctx: &egui::Context, id: u64, a: &str) {
        let Some(item) = self.items.iter().find(|i| i.id == id) else { return };
        let (path, folder, url) = (item.path(), item.folder.clone(), item.url.clone());
        match a {
            "open" => open_path(&path),
            "folder" => open_path(&folder),
            "resume" => self.start_item(id),
            "stop" => self.stop_item(id),
            "redownload" => self.redownload(id),
            "refresh" => self.refresh = Some(RefreshDialog { id, url: url.clone(), error: None }),
            q if q.starts_with("queue:") => self.move_to_queue(id, &q["queue:".len()..]),
            "unqueue" => {
                if let Some(i) = self.items.iter_mut().find(|i| i.id == id) {
                    i.queue = None;
                    if i.status() == Status::Queued {
                        *i.live.status.lock().unwrap() = Status::Paused;
                    }
                    self.dirty = true;
                }
            }
            "up" => self.move_item(id, -1),
            "down" => self.move_item(id, 1),
            "progress" => {
                self.progress_open.insert(id);
            }
            "verify" => {
                self.properties = Some(id);
                self.verify(id);
            }
            "copy" => {
                ctx.copy_text(url);
                self.toast("Address copied");
            }
            "props" => self.properties = Some(id),
            "remove" => self.ask_delete(vec![id]),
            _ => {}
        }
    }

    fn toast_area(&mut self, ctx: &egui::Context) {
        let Some((msg, since)) = &self.toast else { return };
        // Slides up and fades in over 0.2 s, fades out over the last 0.25 s of its 4 s.
        let age = since.elapsed().as_secs_f32();
        let k_in = anim::ease_out(age / 0.2);
        let k_out = ((4.0 - age) / 0.25).clamp(0.0, 1.0);
        if age < 0.25 || age > 3.7 {
            ctx.request_repaint();
        }
        egui::Area::new(egui::Id::unique("toast"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -40.0 + (1.0 - k_in) * 12.0])
            .interactable(false)
            .show(ctx, |ui| {
                ui.multiply_opacity(k_in.min(k_out));
                let (pal, d) = (theme::current(ctx), theme::design(ctx));
                let frame = if d.style == ThemeStyle::Drishti {
                    // A floating layer: popover fill, a strong edge and the sheet shadow.
                    egui::Frame::NONE.fill(pal.raised).stroke(Stroke::new(1.0, pal.separator_strong)).shadow(ctx.global_style().visuals.popup_shadow).corner_radius(d.radius_lg).inner_margin(egui::Margin::symmetric(16, 10))
                } else {
                    egui::Frame::NONE.fill(Color32::from_rgb(44, 44, 46)).corner_radius(18).inner_margin(egui::Margin::symmetric(18, 10))
                };
                let ink = if d.style == ThemeStyle::Drishti { pal.label } else { Color32::WHITE };
                frame.show(ui, |ui| ui.label(RichText::new(msg).color(ink)));
            });
    }
}

pub(super) fn row_menu(ui: &mut egui::Ui, id: u64, st: Status, queues: &[String], action: &mut Option<(u64, String)>) {
    let done = st == Status::Complete;
    let mut item = |ui: &mut egui::Ui, enabled: bool, label: &str, key: &str| {
        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
            *action = Some((id, key.to_string()));
        }
    };
    item(ui, done, "Open", "open");
    item(ui, true, "Open folder", "folder");
    ui.separator();
    item(ui, !done && !st.is_active(), "Resume", "resume");
    item(ui, st.is_active() || st == Status::Queued, "Stop", "stop");
    item(ui, !st.is_active(), "Redownload", "redownload");
    item(ui, !done, "Refresh download address…", "refresh");
    ui.add_enabled_ui(!done && !st.is_active(), |ui| {
        ui.menu_button("Move to queue", |ui| {
            for q in queues {
                if ui.button(q).clicked() {
                    *action = Some((id, format!("queue:{q}")));
                }
            }
            ui.separator();
            if ui.button("Take out of queues").clicked() {
                *action = Some((id, "unqueue".into()));
            }
        });
    });
    let mut item = |ui: &mut egui::Ui, enabled: bool, label: &str, key: &str| {
        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
            *action = Some((id, key.to_string()));
        }
    };
    item(ui, true, "Move up", "up");
    item(ui, true, "Move down", "down");
    ui.separator();
    item(ui, true, "Show progress / speed limit", "progress");
    item(ui, done, "Check SHA-256", "verify");
    item(ui, true, "Copy address", "copy");
    item(ui, true, "Properties", "props");
    ui.separator();
    item(ui, true, "Remove", "remove");
}

/// Small grey section title above a group of sidebar rows.
/// One sidebar row: a full row with its label and count, or just the glyph in the narrow rail.
fn side_row(ui: &mut egui::Ui, rail: bool, label: &str, count: usize, indent: f32, icon: RowIcon, col: Color32, selected: bool) -> egui::Response {
    if rail { rail_row(ui, label, count, icon, col, selected) } else { sidebar_row(ui, label, count, indent, icon, col, selected) }
}

fn section_header(ui: &mut egui::Ui, text: &str, pal: &crate::theme::Palette) {
    ui.add_space(2.0);
    ui.label(RichText::new(text.to_uppercase()).size(theme::design(ui.ctx()).overline).color(pal.tertiary));
    ui.add_space(2.0);
}

/// What a toolbar button does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolKey {
    Add,
    Batch,
    Resume,
    Stop,
    StopAll,
    Delete,
    ClearDone,
    Options,
    Scheduler,
    StartQueue,
    StopQueue,
}

struct Tool {
    key: ToolKey,
    icon: Icon,
    label: &'static str,
    tip: String,
    enabled: bool,
}
