//! Main window: header with menus and search, toolbar, sidebar, download list and status bar.

use super::layout;
use super::mcp_cmds::AiState;
use super::*;
use crate::icons::{self, Icon};
use crate::theme::ThemeMode;
use crate::widgets::{
    anim, capsule_bar, icon_button, icon_label_button, icon_segmented, menu_caption, menu_check_row, menu_icon_item, menu_item, menu_separator, menu_submenu, menu_toggle, menu_width,
    rail_row, search_field, sidebar_pill, sidebar_row, spinner, state_chip, status_chip, tool_button, tool_button_icon, toolbar_sep, truncated, ButtonKind, Hue, RowIcon, TOOL_TILE_H, TOOL_TILE_ICON_H,
};
use eframe::egui::{Align, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Sense, Stroke};
use egui_extras::{Column, TableBuilder};

impl eframe::App for App {
    /// Closing the window keeps everything: an MCP client that is still attached takes over.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
        self.close_control();
        // Let go of the download list so an MCP engine can pick it up at once.
        self.state_lock = None;
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.winui_sync(&ui.ctx().clone(), frame);
        self.draw(ui);
    }
}

impl App {
    /// One frame of the whole window. (Kept apart from `eframe::App::ui` so tests can draw it without a native frame.)
    pub(super) fn draw(&mut self, ui: &mut egui::Ui) {
        self.frame += 1;
        let ctx = ui.ctx().clone();
        theme::apply(&ctx, self.theme_override.unwrap_or(self.settings.theme), self.color_theme_override.unwrap_or(self.settings.color_theme));
        // The Mica of the WinUI 3 theme is the bottom layer of the window (nothing when another theme is in use).
        self.paint_material(&ctx);
        if !self.demo && !self.quitting && ctx.input(|i| i.viewport().close_requested()) {
            self.on_close_requested(&ctx);
        }
        self.tick();
        self.handle_screenshot(&ctx);
        self.shortcuts(&ctx);

        let bp = self.breakpoint(&ctx);
        crate::widgets::titlebar::show(ui);
        self.header(ui, bp);
        self.toolbar(ui, bp);
        self.status_bar(ui, bp);
        if self.show_categories {
            self.categories_panel(ui, bp);
        }
        let pal = theme::current(&ctx);
        let cards = self.settings.view == layout::ViewMode::Cards || bp == layout::Breakpoint::Compact;
        let margin = match bp {
            layout::Breakpoint::Compact => 12,
            layout::Breakpoint::Medium => 16,
            layout::Breakpoint::Wide => 24,
        };
        let page = Frame::NONE.fill(pal.bg).inner_margin(Margin::same(margin));
        egui::CentralPanel::default().frame(page).show(ui, |ui| {
            if bp != layout::Breakpoint::Compact && !self.items.is_empty() {
                self.stats_row(ui);
                ui.add_space(layout::CARD_GAP + 4.0);
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
        self.onboarding_window(&ctx);
        self.updates_window(&ctx);
        self.about_window(&ctx);
        self.close_window(&ctx);
        self.toast_area(&ctx);
        self.drop_files(&ctx);
        if self.menu_preview {
            self.menu_preview_areas(&ctx);
        }
        crate::widgets::titlebar::frame_and_edges(&ctx);
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
        // Not while typing, and not while a dialog is open: Delete behind a dialog must not delete downloads.
        if ctx.egui_wants_keyboard_input() || ctx.memory(|m| m.top_modal_layer().is_some()) {
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
            let screen = ctx.content_rect();
            // A dashed-look frame around the whole window and a pill in the middle.
            ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("drop_hint_frame"))).rect_stroke(
                screen.shrink(8.0),
                CornerRadius::same(theme::design(ctx).radius_xl),
                Stroke::new(3.0, pal.accent.gamma_multiply(k)),
                egui::StrokeKind::Inside,
            );
            egui::Area::new(egui::Id::unique("drop_hint")).anchor(egui::Align2::CENTER_CENTER, [0.0, (1.0 - k) * 8.0]).interactable(false).show(ctx, |ui| {
                ui.multiply_opacity(k);
                Frame::NONE.fill(pal.accent_solid).corner_radius(CornerRadius::same(theme::design(ctx).round(56.0).min(28))).shadow(theme::shadow(ctx, theme::Elevation::Popup)).inner_margin(Margin::symmetric(28, 18)).show(ui, |ui| {
                    ui.label(RichText::new("Drop to download the links inside").color(pal.on_accent).size(17.0).family(theme::semibold()));
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

    // ------------------------------------------------------------ header and menus

    /// The top bar: logo, the menus, the search box and the main action.
    fn header(&mut self, ui: &mut egui::Ui, bp: layout::Breakpoint) {
        use layout::Breakpoint::{Compact, Wide};
        let pal = theme::current(ui.ctx());
        // The name and the mark of the app are in the title bar above; the header holds the controls.
        let own_title_bar = crate::widgets::titlebar::mode() != crate::widgets::titlebar::Mode::Native;
        let frame = Frame::NONE.fill(pal.chrome).inner_margin(Margin::symmetric(if bp == Compact { 12 } else { 24 }, if own_title_bar { 6 } else { 12 }));
        egui::Panel::top("header").frame(frame).show_separator_line(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if bp == Compact && icon_button(ui, Icon::Sidebar, "Categories and queues").clicked() {
                    self.drawer_open = !self.drawer_open;
                }
                if !own_title_bar {
                    logo(ui);
                }
                if bp == Wide {
                    self.menu_bar(ui);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if bp == Compact {
                        if crate::widgets::icon_button_primary(ui, Icon::Plus, "Add a download (Ctrl+N)").clicked() {
                            self.open_add_dialog(None);
                        }
                    } else if icon_label_button(ui, ButtonKind::Primary, Icon::Plus, "Add download").on_hover_text("Add a new download (Ctrl+N)").clicked() {
                        self.open_add_dialog(None);
                    }
                    if bp != Wide {
                        let more = icon_button(ui, Icon::Menu, "Menu");
                        egui::Popup::menu(&more).frame(crate::widgets::menu::menu_surface(ui.ctx())).show(|ui| {
                            menu_width(ui);
                            self.overflow_menu(ui);
                        });
                    }
                    ui.add_space(4.0);
                    let w = if bp == Wide { (ui.available_width() - 24.0).clamp(220.0, 380.0) } else { (ui.available_width() - 4.0).clamp(80.0, 300.0) };
                    search_field(ui, &mut self.search, "Search downloads", w);
                });
            });
        });
    }

    /// The five menus, side by side (wide windows).
    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let control_h = theme::design(ui.ctx()).control_h;
        egui::MenuBar::new()
            .style(move |style: &mut egui::Style| {
                // egui's own menu look first (no borders, a clear resting fill), then ours on top.
                egui::containers::menu::menu_style(style);
                style.spacing.button_padding = egui::vec2(14.0, 8.0);
                style.spacing.interact_size.y = control_h;
                style.spacing.item_spacing.x = 2.0;
            })
            .ui(ui, |ui| {
                ui.menu_button("Tasks", |ui| {
                    menu_width(ui);
                    self.menu_tasks(ui);
                });
                ui.menu_button("File", |ui| {
                    menu_width(ui);
                    self.menu_file(ui);
                });
                ui.menu_button("Downloads", |ui| {
                    menu_width(ui);
                    self.menu_downloads(ui);
                });
                ui.menu_button("View", |ui| {
                    menu_width(ui);
                    self.menu_view(ui);
                });
                ui.menu_button("Help", |ui| {
                    menu_width(ui);
                    self.menu_help(ui);
                });
            });
    }

    /// The same five menus as submenus of one button (narrow windows).
    fn overflow_menu(&mut self, ui: &mut egui::Ui) {
        menu_submenu(ui, "Tasks", true, |ui| self.menu_tasks(ui));
        menu_submenu(ui, "File", true, |ui| self.menu_file(ui));
        menu_submenu(ui, "Downloads", true, |ui| self.menu_downloads(ui));
        menu_submenu(ui, "View", true, |ui| self.menu_view(ui));
        menu_submenu(ui, "Help", true, |ui| self.menu_help(ui));
    }

    fn menu_tasks(&mut self, ui: &mut egui::Ui) {
        if menu_item(ui, "Add new download…", "Ctrl+N", true).clicked() {
            self.open_add_dialog(None);
        }
        if menu_item(ui, "Add batch download…", "Ctrl+B", true).clicked() {
            self.open_batch();
        }
        if menu_item(ui, "Grab links from a web page…", "Ctrl+G", true).clicked() {
            self.open_grabber();
        }
        menu_separator(ui);
        if menu_item(ui, "Import links from a text file…", "", true).clicked() {
            self.import_links();
        }
        if menu_item(ui, "Export all links to a text file…", "", !self.items.is_empty()).clicked() {
            self.export_links();
        }
        menu_separator(ui);
        if menu_item(ui, "Keep running in the background", "", true).on_hover_text("Close the window and carry on downloading; open it again from the tray icon").clicked() {
            let ctx = ui.ctx().clone();
            self.go_background(&ctx);
        }
        if menu_item(ui, "Exit", "", true).clicked() {
            // Exit really quits, whatever the setting for the close button says.
            let ctx = ui.ctx().clone();
            self.quit_now(&ctx);
        }
    }

    fn menu_file(&mut self, ui: &mut egui::Ui) {
        let sel = self.selected_ids();
        let one = if sel.len() == 1 { self.items.iter().find(|i| i.id == sel[0]) } else { None };
        let one_done = one.is_some_and(|i| i.status() == Status::Complete);
        if menu_item(ui, "Open file", "", one_done).clicked()
            && let Some(i) = one
        {
            open_path(&i.path());
        }
        if menu_item(ui, "Open folder", "", one.is_some()).clicked()
            && let Some(i) = one
        {
            open_path(&i.folder);
        }
        if menu_item(ui, "Properties", "", one.is_some()).clicked() {
            self.properties = one.map(|i| i.id);
        }
        menu_separator(ui);
        if menu_item(ui, "Delete…", "Del", !sel.is_empty()).clicked() {
            self.ask_delete(sel.clone());
        }
    }

    fn menu_downloads(&mut self, ui: &mut egui::Ui) {
        let sel = self.selected_ids();
        if menu_item(ui, "Resume", "", !sel.is_empty()).clicked() {
            for id in &sel {
                self.start_item(*id);
            }
        }
        if menu_item(ui, "Pause", "", !sel.is_empty()).clicked() {
            for id in &sel {
                self.stop_item(*id);
            }
        }
        if menu_item(ui, "Pause all", "", true).clicked() {
            self.stop_all();
        }
        if menu_item(ui, "Delete all completed", "", true).clicked() {
            self.delete_completed();
        }
        menu_separator(ui);
        let names: Vec<String> = self.settings.queues.iter().map(|q| q.name.clone()).collect();
        menu_submenu(ui, "Start queue", true, |ui| {
            for q in &names {
                if menu_item(ui, q, "", !self.running.contains(q)).clicked() {
                    self.start_queue(q);
                }
            }
        });
        menu_submenu(ui, "Stop queue", true, |ui| {
            for q in &names {
                if menu_item(ui, q, "", self.running.contains(q)).clicked() {
                    self.stop_queue(q);
                }
            }
        });
        if menu_item(ui, "Scheduler…", "", true).clicked() {
            let q = self.current_queue();
            self.open_scheduler(&q);
        }
        if menu_item(ui, "Manage queues…", "", true).clicked() {
            self.queues_open = true;
        }
        menu_separator(ui);
        if menu_toggle(ui, &format!("Speed limiter ({} KB/s)", self.settings.limit_kbps), &mut self.settings.limit_on).changed() {
            self.apply_settings();
            self.dirty = true;
        }
        if menu_toggle(ui, "Watch the clipboard for links", &mut self.settings.watch_clipboard).changed() {
            self.apply_settings();
            self.dirty = true;
        }
        menu_separator(ui);
        if menu_icon_item(ui, Icon::Options, "Options…", "", true).clicked() {
            self.options = Some(self.settings.clone());
        }
    }

    fn menu_view(&mut self, ui: &mut egui::Ui) {
        menu_toggle(ui, "Show categories", &mut self.show_categories);
        menu_submenu(ui, "Show downloads as", true, |ui| {
            for (mode, name) in [(layout::ViewMode::Cards, "Cards"), (layout::ViewMode::Table, "Table")] {
                if menu_check_row(ui, name, self.settings.view == mode).clicked() {
                    self.settings.view = mode;
                    self.dirty = true;
                }
            }
        });
        menu_submenu(ui, "Appearance", true, |ui| {
            menu_caption(ui, "Mode");
            for (mode, name) in [(ThemeMode::System, "Automatic (follow system)"), (ThemeMode::Light, "Light"), (ThemeMode::Dark, "Dark")] {
                if menu_check_row(ui, name, self.settings.theme == mode).clicked() {
                    self.settings.theme = mode;
                    self.dirty = true;
                }
            }
            menu_separator(ui);
            menu_submenu(ui, "Theme", true, |ui| {
                for theme in crate::theme::ColorTheme::ALL {
                    if menu_check_row(ui, theme.name(), self.settings.color_theme == theme).clicked() {
                        self.settings.color_theme = theme;
                        self.dirty = true;
                    }
                }
            });
        });
        if menu_item(ui, "Clear sorting", "", self.sort.is_some()).clicked() {
            self.sort = None;
        }
    }

    /// `--show menus`: the rows of three menus drawn side by side on the page (screenshots only).
    fn menu_preview_areas(&mut self, ctx: &egui::Context) {
        let surface = crate::widgets::menu::menu_surface;
        egui::Area::new(egui::Id::unique("menu_preview_tasks")).fixed_pos(egui::pos2(24.0, 70.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            surface(ctx).show(ui, |ui| {
                menu_width(ui);
                self.menu_tasks(ui);
            });
        });
        egui::Area::new(egui::Id::unique("menu_preview_file")).fixed_pos(egui::pos2(330.0, 70.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            surface(ctx).show(ui, |ui| {
                menu_width(ui);
                self.menu_file(ui);
                menu_separator(ui);
                menu_caption(ui, "Mode");
                menu_check_row(ui, "Automatic (follow system)", true);
                menu_check_row(ui, "Light", false);
                menu_check_row(ui, "Dark", false);
                menu_separator(ui);
                crate::widgets::menu_danger_item(ui, "Remove", true);
            });
        });
    }

    fn menu_help(&mut self, ui: &mut egui::Ui) {
        if menu_item(ui, "Browser integration…", "", true).clicked() {
            self.browser_open = true;
        }
        if menu_item(ui, "AI assistants…", "", true).clicked() {
            self.ai_open = true;
        }
        if menu_item(ui, "Check for updates…", "", true).clicked() {
            self.updates.open = true;
            if self.updates.checked.is_none() {
                self.check_updates(true);
            }
        }
        if menu_item(ui, "About", "", true).clicked() {
            self.about_open = true;
        }
    }

    // ------------------------------------------------------------ toolbar

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
                t(ToolKey::Stop, Icon::Pause, "Pause", "Pause the selected downloads", can_stop),
                t(ToolKey::StopAll, Icon::PauseAll, "Pause all", "Pause every running download", any_active),
            ],
            vec![t(ToolKey::Delete, Icon::Delete, "Delete", "Remove the selected downloads", !sel.is_empty()), t(ToolKey::ClearDone, Icon::ClearDone, "Clear done", "Remove all completed downloads from the list", any_done)],
            vec![
                t(ToolKey::Options, Icon::Options, "Options", "Folder, connections, speed limit, proxy, browser", true),
                t(ToolKey::Scheduler, Icon::Scheduler, "Scheduler", &format!("Start and stop \"{cq}\" at set times"), true),
            ],
            vec![
                t(ToolKey::StartQueue, Icon::StartQueue, "Start queue", &format!("Start \"{cq}\" (pick another queue on the left)"), any_queued && !cq_running),
                t(ToolKey::StopQueue, Icon::StopQueue, "Stop queue", &format!("Stop \"{cq}\""), cq_running),
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
        use layout::Breakpoint::{Compact, Wide};
        let groups = self.tool_groups();
        let pal = theme::current(ui.ctx());
        let margin = if bp == Compact { Margin::symmetric(8, 4) } else { Margin::symmetric(20, 4) };
        let frame = Frame::NONE.fill(pal.chrome).inner_margin(margin);
        let mut chosen: Option<ToolKey> = None;
        let mut view = self.settings.view;
        egui::Panel::top("toolbar").frame(frame).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let toggle_w = if bp == Compact { 0.0 } else { 100.0 };
                ScrollArea::horizontal().id_salt("tools_scroll").auto_shrink([true, true]).max_width((ui.available_width() - toggle_w).max(120.0)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (n, group) in groups.iter().enumerate() {
                            if n > 0 {
                                toolbar_sep(ui, if bp == Wide { TOOL_TILE_H } else { TOOL_TILE_ICON_H });
                            }
                            for t in group {
                                let hit = if bp == Wide { tool_button(ui, t.icon, t.label, t.enabled, &t.tip) } else { tool_button_icon(ui, t.icon, t.enabled, &t.tip) };
                                if hit {
                                    chosen = Some(t.key);
                                }
                            }
                        }
                    });
                });
                if toggle_w > 0.0 {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        icon_segmented(ui, &[(layout::ViewMode::Cards, Icon::Cards, "Showing cards"), (layout::ViewMode::Table, Icon::List, "Showing the table")], &mut view);
                    });
                }
            });
        });
        if view != self.settings.view {
            self.settings.view = view;
            self.dirty = true;
        }
        if let Some(k) = chosen {
            self.run_tool(k);
        }
    }

    // ------------------------------------------------------------ status bar

    fn status_bar(&mut self, ui: &mut egui::Ui, bp: layout::Breakpoint) {
        let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
        let active = self.items.iter().filter(|i| i.status().is_active()).count();
        let speed: f64 = self.items.iter().filter(|i| i.status().is_active()).map(|i| i.speed()).sum();
        let queued = self.items.iter().filter(|i| i.status() == Status::Queued).count();
        let scheduled: Option<(String, String)> = self.settings.queues.iter().find(|q| q.schedule.start_on).map(|q| (q.name.clone(), q.schedule.start_time.clone()));
        let running: Vec<String> = self.running.iter().cloned().collect();
        let compact = bp == layout::Breakpoint::Compact;
        egui::Panel::bottom("status").frame(Frame::NONE.fill(pal.chrome).inner_margin(Margin::symmetric(if compact { 12 } else { 24 }, 8))).show(ui, |ui| {
            // Rows are as tall as a button (40) unless told otherwise: a status line of 20 point chips needs 20.
            ui.spacing_mut().interact_size.y = 20.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
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
                if self.settings.limit_on && !compact {
                    status_chip(ui, Some(pal.orange), &format!("Speed limit {} KB/s", self.settings.limit_kbps), pal.orange_ink);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let s = if speed > 0.0 { human_speed(speed) } else { "0 B/s".into() };
                    ui.label(RichText::new(format!("↓ {s}")).size(d.caption).family(theme::semibold()).color(pal.label));
                    if compact {
                        // Only the essentials fit.
                        return;
                    }
                    if let Some((txt, strong)) = self.update_hint()
                        && status_chip(ui, Some(pal.accent), &txt, if strong { pal.accent_ink } else { pal.secondary }).on_hover_text("Updates…").clicked()
                    {
                        self.updates.open = true;
                    }
                    if !self.demo {
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
                    let (txt, dot, col) = match self.ai_state() {
                        AiState::Off => ("AI: off", None, pal.secondary),
                        AiState::Unavailable => ("AI: unavailable", Some(pal.orange), pal.orange_ink),
                        AiState::Ready => ("AI: ready", Some(pal.green), pal.secondary),
                        AiState::Active => ("AI: active", Some(pal.blue), pal.blue_ink),
                    };
                    if status_chip(ui, dot, txt, col).on_hover_text("AI assistants (MCP)…").clicked() {
                        self.ai_open = true;
                    }
                    if self.settings.watch_clipboard && bp == layout::Breakpoint::Wide {
                        status_chip(ui, None, "Clipboard: watching", pal.secondary);
                    }
                });
            });
        });
    }

    // ------------------------------------------------------------ sidebar

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
        let panel = egui::Panel::left("categories").show_separator_line(false).resizable(false);
        let panel = if rail_mode { panel.exact_size(72.0) } else if bp == layout::Breakpoint::Compact { panel.exact_size(280.0) } else { panel.exact_size(d.sidebar_w) };
        let rail = panel.frame(Frame::NONE.fill(pal.sidebar).inner_margin(Margin::symmetric(if rail_mode { 10 } else { 12 }, 12))).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                sidebar_pill(ui);
                let mut new_filter = None;
                if !rail_mode {
                    section_header(ui, "Library", &pal, false);
                }
                let nav = |icon: Icon, color| RowIcon { icon, color };
                if side_row(ui, rail_mode, "All downloads", all, 0.0, nav(Icon::Download, pal.accent_ink), self.filter == Filter::All).clicked() {
                    new_filter = Some(Filter::All);
                }
                if side_row(ui, rail_mode, "Unfinished", unfinished, 0.0, nav(Icon::Hourglass, pal.orange_ink), self.filter == Filter::Unfinished).clicked() {
                    new_filter = Some(Filter::Unfinished);
                }
                if side_row(ui, rail_mode, "Finished", finished, 0.0, nav(Icon::CheckCircle, pal.green_ink), self.filter == Filter::Finished).clicked() {
                    new_filter = Some(Filter::Finished);
                }
                if rail_mode {
                    rail_gap(ui, &pal);
                } else {
                    section_header(ui, "Categories", &pal, true);
                }
                for (c, n) in &cats {
                    let r = side_row(ui, rail_mode, c, *n, 0.0, nav(icons::category_icon(c), icons::category_color(c)), self.filter == Filter::Category(c.clone()));
                    if r.clicked() {
                        new_filter = Some(Filter::Category(c.clone()));
                    }
                    r.context_menu(|ui| {
                        menu_width(ui);
                        if menu_item(ui, "Edit categories…", "", true).clicked() {
                            self.options = Some(self.settings.clone());
                        }
                    });
                }
                if rail_mode {
                    rail_gap(ui, &pal);
                } else {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), Sense::hover());
                    ui.painter().text(egui::pos2(rect.left() + 14.0, rect.bottom() - 6.0), egui::Align2::LEFT_BOTTOM, "Queues", egui::FontId::new(d.caption, theme::semibold()), pal.tertiary);
                    let add = egui::Rect::from_center_size(egui::pos2(rect.right() - 20.0, rect.bottom() - 14.0), egui::vec2(28.0, 28.0));
                    if crate::widgets::icon_button_at(ui, add, Icon::Plus, "Manage queues", egui::Id::unique("sdm_queues_add")).clicked() {
                        self.queues_open = true;
                    }
                }
                for (q, n) in &queues {
                    let on = self.running.contains(q);
                    let col = if on { pal.green_ink } else { pal.secondary };
                    let r = side_row(ui, rail_mode, q, *n, 0.0, nav(Icon::Queue, col), self.filter == Filter::Queue(q.clone()));
                    if r.clicked() {
                        new_filter = Some(Filter::Queue(q.clone()));
                    }
                    r.context_menu(|ui| {
                        menu_width(ui);
                        if menu_item(ui, "Start", "", !on).clicked() {
                            self.start_queue(q);
                        }
                        if menu_item(ui, "Stop", "", on).clicked() {
                            self.stop_queue(q);
                        }
                        if menu_item(ui, "Scheduler…", "", true).clicked() {
                            self.open_scheduler(q);
                        }
                        if menu_item(ui, "Manage queues…", "", true).clicked() {
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
        });
        // The edge of the rail is a hairline.
        let r = rail.response.rect;
        ui.painter().vline(r.right() - 0.5, r.y_range(), Stroke::new(1.0, pal.separator));
    }

    // ------------------------------------------------------------ table

    fn table(&mut self, ui: &mut egui::Ui) {
        let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
        let frame = Frame::NONE.fill(pal.card).corner_radius(CornerRadius::same(d.radius_lg)).stroke(Stroke::new(1.0, pal.separator)).inner_margin(Margin::symmetric(0, 6));
        frame.show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            self.table_inner(ui);
        });
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
        // Row marks are clipped to the table's own area (the card's rounded rectangle).
        let area = ui.max_rect();
        let painter = ui.painter().with_clip_rect(area);
        // Column dividers only show while hovered or dragged, like a native list.
        ui.visuals_mut().widgets.noninteractive.bg_stroke = Stroke::NONE;
        ui.visuals_mut().widgets.hovered.bg_stroke = Stroke::new(1.0, pal.separator_strong);
        ui.visuals_mut().widgets.active.bg_stroke = Stroke::new(1.0, pal.accent);

        // Rows sit directly on each other: their height is exactly `row_h`, so what is centred in a row is centred on the row.
        ui.spacing_mut().item_spacing.y = 0.0;
        // Seven columns need about 880 points. Below that the two least useful ones (when it was last tried, and the
        // description) are left out, so nothing is ever cut off at the edge.
        let full = ui.available_width() >= 880.0;
        let table = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .sense(Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(220.0).at_least(140.0).clip(true))
            .column(Column::initial(76.0).at_least(60.0))
            .column(Column::initial(160.0).at_least(90.0))
            .column(Column::initial(96.0).at_least(60.0));
        let table = if full {
            table.column(Column::initial(96.0).at_least(60.0)).column(Column::initial(136.0).at_least(70.0)).column(Column::remainder().at_least(70.0).clip(true))
        } else {
            table.column(Column::remainder().at_least(70.0).clip(true))
        };
        table
            .header(40.0, |mut h| {
                let cols = [
                    ("File name", SortCol::Name),
                    ("Size", SortCol::Size),
                    ("Status", SortCol::Status),
                    ("Time left", SortCol::TimeLeft),
                    ("Speed", SortCol::Speed),
                    ("Last try", SortCol::LastTry),
                    ("Description", SortCol::Description),
                ];
                for (t, c) in cols.into_iter().take(if full { 7 } else { 5 }) {
                    h.col(|ui| {
                        ui.add_space(6.0);
                        let arrow = match sort {
                            Some((sc, true)) if sc == c => " ↑",
                            Some((sc, false)) if sc == c => " ↓",
                            _ => "",
                        };
                        let active = matches!(sort, Some((sc, _)) if sc == c);
                        let r = ui.add(egui::Label::new(RichText::new(format!("{t}{arrow}")).size(d.caption).family(theme::semibold()).color(if active { pal.label } else { pal.tertiary })).sense(Sense::click()));
                        if r.on_hover_text("Click to sort").clicked() {
                            sort_click = Some(c);
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(d.row_h, rows.len(), |mut row| {
                    let item = &self.items[rows[row.index()]];
                    let st = item.status();
                    row.col(|ui| {
                        // Everything in the cell is placed from its centre line, so it lines up with the single-line cells beside it.
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), d.row_h), Sense::hover());
                        let p = ui.painter();
                        let tile = egui::Rect::from_center_size(egui::pos2(rect.left() + 6.0 + 18.0, rect.center().y), egui::Vec2::splat(36.0));
                        icons::file_tile(p, tile, &item.category);
                        let x = tile.right() + 10.0;
                        let max_w = (rect.right() - x - 8.0).max(20.0);
                        let name = truncated(p, &item.name, egui::FontId::new(d.body, theme::semibold()), pal.label, max_w);
                        let cat = truncated(p, &item.category, egui::FontId::proportional(d.caption), pal.tertiary, max_w);
                        let top = rect.center().y - (name.size().y + cat.size().y) / 2.0;
                        let below = top + name.size().y;
                        crate::widgets::elided_tooltip(ui, &name, egui::pos2(x, top), &item.name, ("row_name", item.id));
                        p.galley(egui::pos2(x, top), name, pal.label);
                        p.galley(egui::pos2(x, below), cat, pal.tertiary);
                    });
                    row.col(|ui| {
                        let t = item.total();
                        ui.label(RichText::new(if t > 0 { human_size(t) } else { String::new() }).color(pal.secondary));
                    });
                    row.col(|ui| {
                        let (hue, text_col) = match st {
                            Status::Downloading => (Hue::Accent, pal.label),
                            Status::Connecting | Status::Paused => (Hue::Orange, pal.label),
                            Status::Complete => (Hue::Green, pal.secondary),
                            Status::Error => (Hue::Red, pal.red_ink),
                            Status::Queued => (Hue::Neutral, pal.secondary),
                        };
                        let with_bar = st == Status::Downloading || (st == Status::Paused && item.total() > 0);
                        // The chip (and the bar under it) is a block placed from the centre line of the row.
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), d.row_h), Sense::hover());
                        let (chip_h, gap, bar_h) = (24.0, 6.0, 5.0);
                        let block_h = if with_bar { chip_h + gap + bar_h } else { chip_h };
                        let top = rect.center().y - block_h / 2.0;
                        let block = egui::Rect::from_min_size(egui::pos2(rect.left(), top), egui::vec2(rect.width().min(150.0), block_h));
                        ui.scope_builder(egui::UiBuilder::new().max_rect(block).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(8.0, gap);
                            // `horizontal` rows are `interact_size.y` tall (40): make them as tall as the chip, or the block outgrows its rect.
                            ui.spacing_mut().interact_size.y = chip_h;
                            if st == Status::Connecting {
                                ui.horizontal(|ui| {
                                    spinner(ui, 14.0);
                                    ui.label(RichText::new(item.status_text()).size(d.caption).color(text_col));
                                });
                            } else {
                                ui.horizontal(|ui| {
                                    let chip = state_chip(ui, hue, st.label());
                                    // A failed download's reason is in the Description column; when that column is left out, it is here.
                                    if st == Status::Error {
                                        chip.on_hover_text(item.live.error.lock().unwrap().clone().unwrap_or_else(|| "Something went wrong".into()));
                                    }
                                    if with_bar {
                                        ui.label(RichText::new(format!("{:.0}%", item.progress() * 100.0)).size(d.caption).family(theme::semibold()).color(pal.secondary));
                                    }
                                });
                            }
                            if with_bar {
                                capsule_bar(ui, egui::Id::unique(("row_bar", item.id)), item.progress(), if st == Status::Downloading { pal.accent } else { pal.orange }, bar_h, None);
                            }
                        });
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(item.time_left()).color(pal.secondary));
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(if st == Status::Downloading { human_speed(item.speed()) } else { String::new() }).color(pal.secondary));
                    });
                    if full {
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
                    }
                    let resp = row.response();
                    let span = egui::Rangef::new(resp.rect.left().max(area.left()), resp.rect.right().min(area.right()));
                    let inner = egui::Rect::from_x_y_ranges(span.shrink(6.0), (resp.rect.top() + 2.0)..=(resp.rect.bottom() - 2.0));
                    if self.selected.contains(&item.id) {
                        painter.rect_filled(inner, CornerRadius::same(d.radius_md), pal.accent_tint.gamma_multiply(0.5));
                    } else if resp.hovered() {
                        painter.rect_filled(inner, CornerRadius::same(d.radius_md), pal.hover);
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
        egui::Area::new(egui::Id::unique("toast")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -52.0 + (1.0 - k_in) * 12.0]).interactable(false).show(ctx, |ui| {
            ui.multiply_opacity(k_in.min(k_out));
            let pal = theme::current(ctx);
            // A dark pill with light text in either mode, the way phones show a message.
            let (fill, ink) = if ctx.global_style().visuals.dark_mode { (pal.label, pal.bg) } else { (pal.label, pal.card) };
            Frame::NONE.fill(fill).corner_radius(CornerRadius::same(theme::design(ctx).round(46.0).min(24))).shadow(theme::shadow(ctx, theme::Elevation::Popup)).inner_margin(Margin::symmetric(22, 12)).show(ui, |ui| {
                ui.label(RichText::new(msg).color(ink).family(theme::semibold()));
            });
        });
    }
}

/// The coral logo tile in the header.
fn logo(ui: &mut egui::Ui) {
    let pal = theme::current(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(38.0, 38.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(theme::design(ui.ctx()).radius_md), pal.accent_solid);
    icons::draw_weight(ui.painter(), egui::Rect::from_center_size(rect.center(), egui::vec2(24.0, 24.0)), Icon::Download, pal.on_accent, true, false);
}

pub(super) fn row_menu(ui: &mut egui::Ui, id: u64, st: Status, queues: &[String], action: &mut Option<(u64, String)>) {
    menu_width(ui);
    let done = st == Status::Complete;
    let mut item = |ui: &mut egui::Ui, enabled: bool, label: &str, key: &str| {
        if menu_item(ui, label, "", enabled).clicked() {
            *action = Some((id, key.to_string()));
        }
    };
    item(ui, done, "Open", "open");
    item(ui, true, "Open folder", "folder");
    menu_separator(ui);
    item(ui, !done && !st.is_active(), "Resume", "resume");
    item(ui, st.is_active() || st == Status::Queued, "Pause", "stop");
    item(ui, !st.is_active(), "Redownload", "redownload");
    item(ui, !done, "Refresh download address…", "refresh");
    menu_submenu(ui, "Move to queue", !done && !st.is_active(), |ui| {
        for q in queues {
            if menu_item(ui, q, "", true).clicked() {
                *action = Some((id, format!("queue:{q}")));
            }
        }
        menu_separator(ui);
        if menu_item(ui, "Take out of queues", "", true).clicked() {
            *action = Some((id, "unqueue".into()));
        }
    });
    let mut item = |ui: &mut egui::Ui, enabled: bool, label: &str, key: &str| {
        if menu_item(ui, label, "", enabled).clicked() {
            *action = Some((id, key.to_string()));
        }
    };
    item(ui, true, "Move up", "up");
    item(ui, true, "Move down", "down");
    menu_separator(ui);
    item(ui, true, "Show progress / speed limit", "progress");
    item(ui, done, "Check SHA-256", "verify");
    item(ui, true, "Copy address", "copy");
    item(ui, true, "Properties", "props");
    menu_separator(ui);
    if crate::widgets::menu_danger_item(ui, "Remove", true).clicked() {
        *action = Some((id, "remove".into()));
    }
}

/// One sidebar row: a full row with its label and count, or just the icon in the narrow rail.
fn side_row(ui: &mut egui::Ui, rail: bool, label: &str, count: usize, indent: f32, icon: RowIcon, selected: bool) -> egui::Response {
    if rail { rail_row(ui, label, count, icon, selected) } else { sidebar_row(ui, label, count, indent, icon, selected) }
}

/// Small grey title above a group of sidebar rows.
fn section_header(ui: &mut egui::Ui, text: &str, pal: &crate::theme::Palette, gap: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), if gap { 38.0 } else { 26.0 }), Sense::hover());
    ui.painter().text(egui::pos2(rect.left() + 14.0, rect.bottom() - 6.0), egui::Align2::LEFT_BOTTOM, text, egui::FontId::new(theme::design(ui.ctx()).caption, theme::semibold()), pal.tertiary);
}

/// A hairline between the groups of the narrow icon rail.
fn rail_gap(ui: &mut egui::Ui, pal: &crate::theme::Palette) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 17.0), Sense::hover());
    ui.painter().hline((rect.left() + 10.0)..=(rect.right() - 10.0), rect.center().y, Stroke::new(1.0, pal.separator));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::input;
    use eframe::egui::{vec2, Shape};

    /// Every piece of text painted in a frame.
    fn texts(shape: &Shape, out: &mut Vec<String>) {
        match shape {
            Shape::Text(t) => out.push(t.galley.text().to_string()),
            Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
            _ => {}
        }
    }

    /// Draws `frames` frames of the whole window and returns all the text that was painted.
    fn draw(app: &mut App, ctx: &egui::Context, size: egui::Vec2, frames: usize) -> Vec<String> {
        let mut all = Vec::new();
        for _ in 0..frames {
            let mut out = ctx.run_ui(input(size, vec![]), |ui| app.draw(ui));
            out.textures_delta.clear();
            out.shapes.iter().for_each(|s| texts(&s.shape, &mut all));
        }
        all
    }

    const WINDOWS: [&str; 20] = [
        "", "add", "batch", "options", "networks", "appearance", "scheduler", "queues", "delete", "properties", "progress", "about", "browser", "ai", "drawer", "close", "table", "grabber", "menus", "empty",
    ];
    const SIZES: [(f32, f32); 6] = [(420.0, 420.0), (600.0, 760.0), (719.0, 640.0), (900.0, 640.0), (1180.0, 680.0), (1700.0, 1000.0)];

    #[test]
    fn every_window_draws_at_every_size_in_both_themes_without_clashing_ids_or_panics() {
        for name in WINDOWS {
            for (w, h) in SIZES {
                for mode in [ThemeMode::Light, ThemeMode::Dark] {
                    let ctx = egui::Context::default();
                    let launch = Launch { demo: true, theme: Some(mode), show: (!name.is_empty()).then(|| name.to_string()), ..Default::default() };
                    let mut app = App::build(ctx.clone(), launch);
                    let text = draw(&mut app, &ctx, vec2(w, h), 4);
                    assert!(!text.is_empty(), "{name:?} at {w}x{h} painted no text at all");
                    let clash = text.iter().find(|t| t.contains('🔥'));
                    assert!(clash.is_none(), "{name:?} at {w}x{h} in {mode:?} has a widget id clash: {clash:?}");
                }
            }
        }
    }

    #[test]
    fn every_colour_theme_draws_the_main_window_and_the_busiest_dialogs() {
        for theme in crate::theme::ColorTheme::ALL {
            for mode in [ThemeMode::Light, ThemeMode::Dark] {
                for name in ["", "table", "options", "appearance", "add", "progress", "menus"] {
                    for (w, h) in [(600.0, 760.0), (1180.0, 800.0)] {
                        let ctx = egui::Context::default();
                        let launch = Launch { demo: true, theme: Some(mode), color_theme: Some(theme), show: (!name.is_empty()).then(|| name.to_string()), ..Default::default() };
                        let mut app = App::build(ctx.clone(), launch);
                        let text = draw(&mut app, &ctx, vec2(w, h), 3);
                        assert!(!text.is_empty(), "{name:?} in {} {mode:?} painted nothing", theme.name());
                        assert!(!text.iter().any(|t| t.contains('🔥')), "{name:?} at {w}x{h} in {} {mode:?} has a widget id clash", theme.name());
                    }
                }
            }
        }
    }

    #[test]
    fn the_options_dialog_offers_every_colour_theme_by_name() {
        let ctx = egui::Context::default();
        let mut app = App::build(ctx.clone(), Launch { demo: true, theme: Some(ThemeMode::Light), show: Some("appearance".into()), ..Default::default() });
        let text = draw(&mut app, &ctx, vec2(1180.0, 900.0), 6);
        for theme in crate::theme::ColorTheme::ALL {
            assert!(text.iter().any(|t| t == theme.name()), "the picker does not show {}: {text:?}", theme.name());
        }
    }

    #[test]
    fn the_clash_detector_used_above_really_sees_a_clash() {
        // Two different widgets sharing one id: egui paints a fire emoji warning, and the tests rely on seeing it.
        let ctx = egui::Context::default();
        let mut all = Vec::new();
        for _ in 0..2 {
            let mut out = ctx.run_ui(input(vec2(300.0, 300.0), vec![]), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let id = egui::Id::unique("the-same-id");
                    ui.interact(egui::Rect::from_min_size(egui::pos2(10.0, 10.0), vec2(20.0, 20.0)), id, egui::Sense::click());
                    ui.interact(egui::Rect::from_min_size(egui::pos2(100.0, 100.0), vec2(20.0, 20.0)), id, egui::Sense::click());
                });
            });
            out.textures_delta.clear();
            out.shapes.iter().for_each(|s| texts(&s.shape, &mut all));
        }
        assert!(all.iter().any(|t| t.contains('🔥')), "no warning was painted: the clash test above would never fail");
    }

    #[test]
    fn the_download_complete_window_and_the_empty_window_draw_too() {
        let ctx = egui::Context::default();
        let mut app = App::build(ctx.clone(), Launch { demo: true, theme: Some(ThemeMode::Light), ..Default::default() });
        app.completed.push(2);
        let text = draw(&mut app, &ctx, vec2(1180.0, 680.0), 4);
        assert!(text.iter().any(|t| t == "Download complete"), "the complete window is shown");
        assert!(!text.iter().any(|t| t.contains('🔥')));

        // No downloads at all: the empty state with its button.
        let mut empty = App::build(ctx.clone(), Launch { demo: true, theme: Some(ThemeMode::Light), ..Default::default() });
        empty.items.clear();
        empty.selected.clear();
        let text = draw(&mut empty, &ctx, vec2(1180.0, 680.0), 4);
        assert!(text.iter().any(|t| t == "No downloads yet"), "{text:?}");
        assert!(text.iter().any(|t| t == "Add download"));
    }

    #[test]
    fn the_main_window_names_every_part_of_itself() {
        let ctx = egui::Context::default();
        let mut app = App::build(ctx.clone(), Launch { demo: true, theme: Some(ThemeMode::Light), ..Default::default() });
        let text = draw(&mut app, &ctx, vec2(1180.0, 680.0), 4);
        for want in [
            "Tasks", "File", "Downloads", "View", "Help", "Add URL", "Batch", "Resume", "Pause", "Pause all", "Delete", "Clear done", "Options", "Scheduler", "Start queue", "Stop queue", "All downloads",
            "Unfinished", "Finished", "Compressed", "Documents", "Main Queue", "Speed", "Active", "Queued", "9 downloads", "ubuntu-24.04-desktop-amd64.iso",
        ] {
            assert!(text.iter().any(|t| t == want), "the main window does not show {want:?}");
        }
    }

    #[test]
    fn narrow_windows_fold_the_menus_into_one_button() {
        let ctx = egui::Context::default();
        let mut app = App::build(ctx.clone(), Launch { demo: true, theme: Some(ThemeMode::Light), ..Default::default() });
        let text = draw(&mut app, &ctx, vec2(600.0, 760.0), 4);
        assert!(!text.iter().any(|t| t == "Tasks"), "no menu bar when the window is narrow");
        assert!(!text.iter().any(|t| t == "Add URL"), "and only icons in the toolbar");
    }

    #[test]
    fn shortcuts_are_ignored_while_a_dialog_is_open() {
        let ctx = egui::Context::default();
        let mut app = App::build(ctx.clone(), Launch { demo: true, theme: Some(ThemeMode::Light), show: Some("about".into()), ..Default::default() });
        draw(&mut app, &ctx, vec2(1180.0, 680.0), 3);
        assert!(app.about_open);
        let press = |pressed| egui::Event::Key { key: egui::Key::Delete, physical_key: Some(egui::Key::Delete), pressed, repeat: false, modifiers: egui::Modifiers::NONE };
        for pressed in [true, false] {
            let mut out = ctx.run_ui(input(vec2(1180.0, 680.0), vec![press(pressed)]), |ui| app.draw(ui));
            out.textures_delta.clear();
        }
        assert!(app.delete.is_none(), "Delete behind the About dialog must not open the delete dialog");

        // With nothing open, the same key does ask.
        let mut app = App::build(ctx.clone(), Launch { demo: true, theme: Some(ThemeMode::Light), ..Default::default() });
        draw(&mut app, &ctx, vec2(1180.0, 680.0), 3);
        for pressed in [true, false] {
            let mut out = ctx.run_ui(input(vec2(1180.0, 680.0), vec![press(pressed)]), |ui| app.draw(ui));
            out.textures_delta.clear();
        }
        assert!(app.delete.is_some(), "Delete asks to remove the selected download");
    }
}
