//! More windows: batch download, scheduler, queues, download complete, browser integration, AI assistants, about.

use super::actions::{expand_pattern, normalize_time};
use super::dialogs::{options_of, path_field};
use super::mcp_cmds::AiState;
use super::*;
use crate::icons::{self, Icon};
use crate::widgets::{
    button, card, check_row, dropdown, field, field_full, field_multiline, form_field, icon_button, message, number_field, plain_button, primary_button, secondary_button, segmented, segmented_width, sheet,
    small_button, stepper, time_wheels, ButtonKind, truncated,
};
use eframe::egui::{FontId, RichText, Sense};

const ADDON_FILES: [(&str, &[u8]); 9] = [
    ("manifest.json", include_bytes!("../../browser-extension/manifest.json")),
    ("background.js", include_bytes!("../../browser-extension/background.js")),
    ("popup.html", include_bytes!("../../browser-extension/popup.html")),
    ("popup.js", include_bytes!("../../browser-extension/popup.js")),
    ("README.txt", include_bytes!("../../browser-extension/README.txt")),
    ("icons/icon-16.png", include_bytes!("../../browser-extension/icons/icon-16.png")),
    ("icons/icon-32.png", include_bytes!("../../browser-extension/icons/icon-32.png")),
    ("icons/icon-48.png", include_bytes!("../../browser-extension/icons/icon-48.png")),
    ("icons/icon-128.png", include_bytes!("../../browser-extension/icons/icon-128.png")),
];

/// Write the browser add-on into `dir/swift-download-manager-addon`.
pub fn save_addon(dir: &std::path::Path) -> std::io::Result<PathBuf> {
    let out = dir.join("swift-download-manager-addon");
    std::fs::create_dir_all(out.join("icons"))?;
    for (name, text) in ADDON_FILES {
        std::fs::write(out.join(name), text)?;
    }
    Ok(out)
}

impl App {
    pub(super) fn batch_window(&mut self, ctx: &egui::Context) {
        let Some(mut b) = self.batch.take() else { return };
        let queue_names: Vec<String> = self.settings.queues.iter().map(|q| q.name.clone()).collect();
        if !queue_names.contains(&b.queue) {
            b.queue = MAIN_QUEUE.to_string();
        }
        let queues = options_of(&queue_names);
        let pal = theme::current(ctx);
        let mut cancel = false;
        let mut result: Option<bool> = None;
        let urls: Result<Vec<String>, String> = if b.pattern_mode {
            expand_pattern(b.pattern.trim(), b.from, b.to, b.width)
        } else {
            Ok(b.links.split_whitespace().map(|s| s.to_string()).collect())
        };
        let ok = urls.as_ref().is_ok_and(|l| !l.is_empty());
        let out = sheet("batch", "Batch download").width(600.0).show(ctx, |s| {
            s.body(|ui| {
                let w = segmented_width(ui, &["A list of links", "Numbered links (pattern)"]);
                ui.horizontal(|ui| {
                    ui.set_width(w);
                    segmented(ui, &[(false, "A list of links"), (true, "Numbered links (pattern)")], &mut b.pattern_mode);
                });
                ui.add_space(12.0);
                if b.pattern_mode {
                    form_field(ui, "Address with a * where the number goes", |ui| {
                        field_full(ui, &mut b.pattern, "https://example.com/photos/img*.jpg");
                    });
                    ui.columns(3, |cols| {
                        form_field(&mut cols[0], "From", |ui| {
                            let w = ui.available_width();
                            number_field(ui, &mut b.from, 0..=1_000_000, "", w);
                        });
                        form_field(&mut cols[1], "To", |ui| {
                            let w = ui.available_width();
                            number_field(ui, &mut b.to, 0..=1_000_000, "", w);
                        });
                        form_field(&mut cols[2], "Digits", |ui| {
                            let w = ui.available_width();
                            number_field(ui, &mut b.width, 1..=8, "", w).on_hover_text("2 digits makes 1 into 01");
                        });
                    });
                } else {
                    form_field(ui, "Paste links, one per line", |ui| {
                        let w = ui.available_width();
                        ui.add(field_multiline(&mut b.links, 8).desired_width(w).hint_text("https://example.com/file1.zip\nhttps://example.com/file2.zip"));
                    });
                }
                form_field(ui, "Save to", |ui| {
                    path_field(ui, &mut b.folder);
                });
                form_field(ui, "Add to queue", |ui| {
                    let w = ui.available_width();
                    dropdown(ui, "batch_queue", w, &queues, &mut b.queue);
                });
                ui.add_space(4.0);
                match &urls {
                    Ok(list) if !list.is_empty() => {
                        ui.label(RichText::new(format!("{} links", list.len())).family(theme::semibold()));
                        for u in list.iter().take(3) {
                            ui.add(egui::Label::new(RichText::new(u).size(12.5).color(pal.secondary)).truncate());
                        }
                        if list.len() > 3 {
                            ui.label(RichText::new("…").color(pal.secondary));
                        }
                    }
                    Ok(_) => message(ui, "No links yet.", false),
                    Err(e) => message(ui, e, true),
                }
                if let Some(e) = &b.error {
                    message(ui, e, true);
                }
            });
            s.footer_split(
                |ui| {
                    if plain_button(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                },
                |ui| {
                    ui.add_enabled_ui(ok, |ui| {
                        if primary_button(ui, "Download all now").clicked() {
                            result = Some(true);
                        }
                        if secondary_button(ui, "Add to queue").clicked() {
                            result = Some(false);
                        }
                    });
                },
            );
        });
        if out.close || cancel {
            return;
        }
        if let (Some(start), Ok(list)) = (result, &urls) {
            let folder = (!b.folder.trim().is_empty()).then(|| PathBuf::from(b.folder.trim()));
            let q = b.queue.clone();
            match self.add_many(list, folder, start, &q, "") {
                Ok(n) => {
                    self.toast(format!("Added {n} downloads"));
                    return;
                }
                Err(e) => b.error = Some(e),
            }
        }
        self.batch = Some(b);
    }

    pub(super) fn scheduler_window(&mut self, ctx: &egui::Context) {
        let Some((qname, mut s)) = self.scheduler.take() else { return };
        let mut result = None;
        let mut switch_to: Option<String> = None;
        let mut start_now = false;
        let names: Vec<String> = self.settings.queues.iter().map(|q| q.name.clone()).collect();
        let options = options_of(&names);
        let queued = self.items.iter().filter(|i| i.status() == Status::Queued && i.queue.as_deref() == Some(qname.as_str())).count();
        let pal = theme::current(ctx);
        let bad = (s.start_on && normalize_time(&s.start_time).is_empty()) || (s.stop_on && normalize_time(&s.stop_time).is_empty());
        let out = sheet("scheduler", "Scheduler").width(540.0).show(ctx, |sh| {
            sh.body(|ui| {
                ui.horizontal(|ui| {
                    let mut chosen = qname.clone();
                    dropdown(ui, "sched_q", 200.0, &options, &mut chosen);
                    if chosen != qname {
                        switch_to = Some(chosen);
                    }
                    ui.label(RichText::new(format!("{queued} downloads waiting")).color(pal.secondary));
                });
                ui.add_space(10.0);
                let repeat_w = segmented_width(ui, &["Every day", "Once"]);
                let done_w = segmented_width(ui, &["Do nothing", "Close the app"]);
                card(ui, |g| {
                    g.toggle("Start the queue at", None, &mut s.start_on);
                    if s.start_on {
                        g.custom(104.0, |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| time_wheels(ui, egui::Id::unique(("sched_start", &qname)), &mut s.start_time));
                        });
                    }
                    g.toggle("Stop the queue at", None, &mut s.stop_on);
                    if s.stop_on {
                        g.custom(104.0, |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| time_wheels(ui, egui::Id::unique(("sched_stop", &qname)), &mut s.stop_time));
                        });
                    }
                    g.row("Repeat", None, repeat_w, |ui| segmented(ui, &[(true, "Every day"), (false, "Once")], &mut s.daily));
                    g.row("When the queue is done", None, done_w, |ui| segmented(ui, &[(WhenDone::Nothing, "Do nothing"), (WhenDone::ExitApp, "Close the app")], &mut s.when_done));
                });
                ui.add_space(8.0);
                if bad {
                    message(ui, "Write times as hours and minutes, e.g. 02:30 or 23:15.", true);
                }
                message(ui, &format!("Time now: {}", chrono::Local::now().format("%H:%M")), false);
            });
            sh.footer_split(
                |ui| {
                    if small_button(ui, ButtonKind::Secondary, "Start queue now").clicked() {
                        start_now = true;
                    }
                },
                |ui| {
                    ui.add_enabled_ui(!bad, |ui| {
                        if primary_button(ui, "Save").clicked() {
                            result = Some(true);
                        }
                    });
                    if plain_button(ui, "Cancel").clicked() {
                        result = Some(false);
                    }
                },
            );
        });
        if out.close && result.is_none() {
            result = Some(false);
        }
        if start_now {
            self.start_queue(&qname);
        }
        if let Some(n) = switch_to {
            self.open_scheduler(&n);
            return;
        }
        match result {
            Some(true) => {
                s.start_time = normalize_time(&s.start_time).max("00:00".into());
                s.stop_time = normalize_time(&s.stop_time).max("00:00".into());
                // Allow a time set for later today to fire today.
                s.last_start.clear();
                s.last_stop.clear();
                if let Some(q) = self.settings.queues.iter_mut().find(|q| q.name == qname) {
                    q.schedule = s;
                }
                self.dirty = true;
                self.toast(format!("Schedule for {qname} saved"));
            }
            Some(false) => {}
            None => self.scheduler = Some((qname, s)),
        }
    }

    /// Create, rename and remove queues, and set how many downloads each runs at once.
    pub(super) fn queues_window(&mut self, ctx: &egui::Context) {
        if !self.queues_open {
            return;
        }
        let mut remove: Option<String> = None;
        let mut renames: Vec<(String, String)> = Vec::new();
        let mut add = false;
        let mut sched: Option<String> = None;
        let mut new_name = std::mem::take(&mut self.new_queue);
        let mut changed = false;
        let pal = theme::current(ctx);
        let out = sheet("queues", "Queues").width(560.0).show(ctx, |s| {
            s.body(|ui| {
                ui.label(RichText::new("Each queue downloads its files a few at a time and can have its own schedule.").color(pal.secondary));
                ui.add_space(10.0);
                let sched_w = crate::widgets::button_width(ui, crate::widgets::ButtonSize::Small, false, "Scheduler");
                card(ui, |g| {
                    for q in self.settings.queues.iter_mut() {
                        g.custom(64.0, |ui| {
                            let main = q.name == MAIN_QUEUE;
                            let total = ui.available_width();
                            let stepper_w = 128.0;
                            let name_w = (total - stepper_w - sched_w - 36.0 - 3.0 * 8.0).max(80.0);
                            let mut name = q.name.clone();
                            if ui.add_enabled(!main, field(&mut name).desired_width(name_w)).changed() {
                                renames.push((q.name.clone(), name.clone()));
                            }
                            changed |= stepper(ui, &mut q.max_parallel, 1..=8).on_hover_text("How many downloads this queue runs at once").changed();
                            if small_button(ui, ButtonKind::Secondary, "Scheduler").clicked() {
                                sched = Some(q.name.clone());
                            }
                            ui.add_enabled_ui(!main, |ui| {
                                if icon_button(ui, Icon::Delete, "Remove (its downloads stay in the list)").clicked() {
                                    remove = Some(q.name.clone());
                                }
                            });
                        });
                    }
                });
                ui.add_space(14.0);
                form_field(ui, "New queue", |ui| {
                    ui.horizontal(|ui| {
                        let bw = crate::widgets::button_width(ui, crate::widgets::ButtonSize::Regular, false, "Add queue");
                        let w = (ui.available_width() - bw - ui.spacing().item_spacing.x).max(80.0);
                        ui.add(field(&mut new_name).hint_text("Queue name").desired_width(w));
                        ui.add_enabled_ui(!new_name.trim().is_empty(), |ui| {
                            if button(ui, ButtonKind::Secondary, "Add queue").clicked() {
                                add = true;
                            }
                        });
                    });
                });
            });
        });
        for (old, new) in renames {
            let new = new.trim().to_string();
            if new.is_empty() || self.settings.queues.iter().any(|q| q.name == new) {
                continue;
            }
            if let Some(q) = self.settings.queues.iter_mut().find(|q| q.name == old) {
                q.name = new.clone();
            }
            for i in &mut self.items {
                if i.queue.as_deref() == Some(old.as_str()) {
                    i.queue = Some(new.clone());
                }
            }
            if self.running.remove(&old) {
                self.running.insert(new.clone());
            }
            if self.filter == Filter::Queue(old.clone()) {
                self.filter = Filter::Queue(new);
            }
            self.dirty = true;
        }
        if let Some(name) = remove {
            self.stop_queue(&name);
            self.settings.queues.retain(|q| q.name != name);
            for i in &mut self.items {
                if i.queue.as_deref() == Some(name.as_str()) {
                    i.queue = None;
                    if i.status() == Status::Queued {
                        *i.live.status.lock().unwrap() = Status::Paused;
                    }
                }
            }
            if self.filter == Filter::Queue(name) {
                self.filter = Filter::All;
            }
            self.dirty = true;
        }
        if add {
            let n = new_name.trim().to_string();
            if self.settings.queues.iter().any(|q| q.name == n) {
                self.toast("There is already a queue with that name");
            } else {
                self.settings.queues.push(QueueDef { name: n, ..Default::default() });
                new_name.clear();
                self.dirty = true;
            }
        }
        self.new_queue = new_name;
        if let Some(q) = sched {
            self.open_scheduler(&q);
        }
        if out.close {
            self.queues_open = false;
        }
        if changed {
            self.dirty = true;
        }
    }

    /// Paste a fresh link for a download whose old link stopped working.
    pub(super) fn refresh_window(&mut self, ctx: &egui::Context) {
        let Some(mut r) = self.refresh.take() else { return };
        let Some(item) = self.items.iter().find(|i| i.id == r.id) else { return };
        let name = item.name.clone();
        let mut ok = false;
        let mut cancel = false;
        let pal = theme::current(ctx);
        let out = sheet("refresh", "Refresh download address").width(560.0).show(ctx, |s| {
            s.body(|ui| {
                ui.label(RichText::new(&name).family(theme::semibold()));
                ui.add_space(4.0);
                ui.add(egui::Label::new(RichText::new("If the link stopped working (many sites give links that expire), open the download page again, copy a fresh link to the same file and paste it here. The download carries on where it stopped.").color(pal.secondary)).wrap());
                ui.add_space(10.0);
                form_field(ui, "New address", |ui| {
                    field_full(ui, &mut r.url, "https://…");
                });
                if let Some(e) = &r.error {
                    message(ui, e, true);
                }
            });
            s.footer_split(
                |ui| {
                    if plain_button(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                },
                |ui| {
                    if primary_button(ui, "Use this link").clicked() {
                        ok = true;
                    }
                },
            );
        });
        if out.close || cancel {
            return;
        }
        if ok {
            let url = r.url.clone();
            match self.refresh_address(r.id, &url) {
                Ok(()) => {
                    self.toast("Address updated");
                    return;
                }
                Err(e) => r.error = Some(e),
            }
        }
        self.refresh = Some(r);
    }

    pub(super) fn complete_windows(&mut self, ctx: &egui::Context) {
        let ids = self.completed.clone();
        for id in ids {
            let Some(item) = self.items.iter().find(|i| i.id == id) else {
                self.completed.retain(|c| *c != id);
                continue;
            };
            let act = std::cell::Cell::new(None::<&str>);
            let mut dont_show = !self.settings.show_complete;
            let pal = theme::current(ctx);
            let pos = egui::pos2(360.0 + (id % 4) as f32 * 20.0, 160.0 + (id % 4) as f32 * 20.0);
            let out = sheet(("complete", id), "Download complete").width(480.0).floating(pos).show(ctx, |s| {
                s.body(|ui| {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(52.0, 52.0), Sense::hover());
                        icons::file_tile(ui.painter(), r, &item.category);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            let w = ui.available_width();
                            let g = truncated(ui.painter(), &item.name, FontId::new(16.0, theme::semibold()), pal.label, w);
                            let (rect, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                            ui.painter().galley(rect.min, g, pal.label);
                            ui.add(egui::Label::new(RichText::new(format!("{}  ·  {}", human_size(item.total()), item.folder.display())).size(12.5).color(pal.tertiary)).truncate());
                        });
                    });
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(RichText::new(&item.url).color(pal.blue_ink)).truncate());
                    if let Some(c) = &item.check {
                        let col = if c.contains("NOT") { pal.red_ink } else if c.contains("matches") { pal.green_ink } else { pal.secondary };
                        ui.label(RichText::new(c).color(col));
                    }
                    ui.add_space(6.0);
                    check_row(ui, &mut dont_show, "Don't show this window again");
                });
                s.footer_split(
                    |ui| {
                        if plain_button(ui, "Close").clicked() {
                            act.set(Some("close"));
                        }
                    },
                    |ui| {
                        if primary_button(ui, "Open").clicked() {
                            act.set(Some("open"));
                        }
                        if secondary_button(ui, "Open folder").clicked() {
                            act.set(Some("folder"));
                        }
                    },
                );
            });
            if dont_show == self.settings.show_complete {
                self.settings.show_complete = !dont_show;
                self.dirty = true;
            }
            match act.get() {
                Some("open") => open_path(&item.path()),
                Some("folder") => open_path(&item.folder),
                _ => {}
            }
            if out.close || act.get().is_some() {
                self.completed.retain(|c| *c != id);
            }
        }
    }

    pub(super) fn browser_window(&mut self, ctx: &egui::Context) {
        if !self.browser_open {
            return;
        }
        let pal = theme::current(ctx);
        let mut saved: Option<std::path::PathBuf> = None;
        let mut save_error: Option<String> = None;
        let mut close = false;
        let out = sheet("browser", "Browser integration").width(560.0).backdrop_closes().show(ctx, |s| {
            s.body(|ui| {
                let (icon, tint, ink, text) = match (&self.listener, self.settings.browser_listen) {
                    (Ok(()), _) => (Icon::CheckCircle, pal.green_tint, pal.green_ink, format!("Ready: the app accepts downloads from the browser (port {}).", bridge::PORT)),
                    (Err(_), false) => (Icon::Info, pal.surface2, pal.secondary, "Turned off in Options.".to_string()),
                    (Err(e), true) => (Icon::WarningCircle, pal.red_tint, pal.red_ink, format!("Not available: {e}. Another copy of the app may be open.")),
                };
                crate::widgets::notice(ui, icon, &text, tint, ink);
                ui.add_space(12.0);
                ui.add(egui::Label::new(RichText::new("The browser add-on sends the downloads you start in Chrome, Edge, Brave or Firefox to this app, and adds “Download with Swift Download Manager” when you right-click a link.").color(pal.secondary)).wrap());
                ui.add_space(12.0);
                ui.label(RichText::new("How to install it").size(15.0).family(theme::semibold()));
                ui.add_space(4.0);
                step(ui, 1, "Click “Save the add-on…” below and choose a folder.");
                step(ui, 2, "Chrome or Edge: open the Extensions page, turn on Developer mode, click “Load unpacked” and pick the saved folder.");
                step(ui, 3, "Firefox: open about:debugging, “This Firefox”, “Load Temporary Add-on” and pick manifest.json in the saved folder.");
                ui.add_space(8.0);
                ui.add(egui::Label::new(RichText::new("Without the add-on: copy a download link and the app will offer to download it (clipboard watching), or start the app with a link.").size(12.5).color(pal.tertiary)).wrap());
                ui.add_space(6.0);
                if check_row(ui, &mut self.settings.watch_clipboard, "Watch the clipboard for links").changed() {
                    self.apply_settings();
                    self.dirty = true;
                }
            });
            s.footer_split(
                |ui| {
                    if plain_button(ui, "Close").clicked() {
                        close = true;
                    }
                },
                |ui| {
                    if primary_button(ui, "Save the add-on…").clicked()
                        && let Some(dir) = rfd::FileDialog::new().pick_folder()
                    {
                        match save_addon(&dir) {
                            Ok(p) => saved = Some(p),
                            Err(e) => save_error = Some(format!("Could not save: {e}")),
                        }
                    }
                },
            );
        });
        if let Some(p) = saved {
            open_path(&p);
            self.toast(format!("Saved to {}", p.display()));
        }
        if let Some(e) = save_error {
            self.toast(e);
        }
        if out.close || close {
            self.browser_open = false;
        }
    }

    /// Help > AI assistants: the switch, the state, and setup text to copy.
    pub(super) fn ai_window(&mut self, ctx: &egui::Context) {
        if !self.ai_open {
            return;
        }
        // The sample window (--demo, screenshots) shows a placeholder instead of this machine's path.
        let exe = if self.demo { "/path/to/swift_download_manager".to_string() } else { std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "swift_download_manager".into()) };
        let (state, mut on) = (self.ai_state(), self.settings.mcp_enabled);
        let (code, desktop) = (crate::mcp::setup::claude_code_command(&exe), crate::mcp::setup::desktop_config(&exe));
        let mut copied = None;
        let mut close = false;
        let pal = theme::current(ctx);
        let out = sheet("ai", "AI assistants (MCP)").width(600.0).backdrop_closes().show(ctx, |s| {
            s.body(|ui| {
                ui.add(egui::Label::new(RichText::new("Let an AI assistant such as Claude look at your downloads and add, pause, resume or remove them. It cannot delete files from your disk or pick the folder a file is saved in.").color(pal.secondary)).wrap());
                ui.add_space(12.0);
                let (icon, tint, ink, text) = match state {
                    AiState::Off => (Icon::Info, pal.surface2, pal.secondary, "Off: assistants are refused"),
                    AiState::Unavailable => (Icon::WarningCircle, pal.orange_tint, pal.orange_ink, "Another copy of the app owns the downloads, so this window cannot be controlled"),
                    AiState::Ready => (Icon::CheckCircle, pal.green_tint, pal.green_ink, "Ready: connect an assistant below"),
                    AiState::Active => (Icon::Robot, pal.blue_tint, pal.blue_ink, "An assistant is using it right now"),
                };
                card(ui, |g| {
                    g.toggle("Allow AI assistants", None, &mut on);
                });
                ui.add_space(8.0);
                crate::widgets::notice(ui, icon, text, tint, ink);
                for (title, snippet) in [("Claude Code: run this once", &code), ("Claude Desktop and other apps: add this to their MCP settings", &desktop)] {
                    crate::widgets::section_title(ui, title);
                    code_box(ui, snippet);
                    ui.add_space(6.0);
                    if crate::widgets::small_icon_label_button(ui, ButtonKind::Secondary, Icon::Copy, "Copy").clicked() {
                        copied = Some(snippet.clone());
                    }
                }
                crate::widgets::hint(ui, "The assistant starts this program itself and shares your open window, or runs the downloads without one while it is closed. Clients ask you before each action unless you tell them not to.");
            });
            s.footer(|ui| {
                if primary_button(ui, "Done").clicked() {
                    close = true;
                }
            });
        });
        if on != self.settings.mcp_enabled {
            self.settings.mcp_enabled = on;
            self.dirty = true;
        }
        if let Some(text) = copied {
            ctx.copy_text(text);
            self.toast("Copied");
        }
        if out.close || close {
            self.ai_open = false;
        }
    }

    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let pal = theme::current(ctx);
        let mut about_icon = self.about_icon.take();
        let out = sheet("about", "About").width(420.0).backdrop_closes().show(ctx, |s| {
            s.body(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    let icon = about_icon.get_or_insert_with(|| ctx.load_texture("about-icon", egui::ColorImage::from_rgba_unmultiplied([128, 128], &crate::appicon::rgba(128)), egui::TextureOptions::LINEAR));
                    ui.add(egui::Image::new((icon.id(), egui::vec2(80.0, 80.0))));
                    ui.add_space(8.0);
                    ui.label(RichText::new(APP_NAME).size(theme::design(ctx).heading - 2.0).family(theme::semibold()).color(pal.label));
                    ui.label(RichText::new(format!("Version {}, written in Rust", env!("CARGO_PKG_VERSION"))).color(pal.tertiary));
                    ui.add_space(10.0);
                    ui.add(egui::Label::new(RichText::new("Splits each download into several connections, re-splits as parts finish, retries broken connections, pauses and resumes, keeps a scheduled queue and works with your browser and clipboard.").color(pal.secondary)).wrap());
                    ui.add_space(6.0);
                });
            });
        });
        self.about_icon = about_icon;
        if out.close {
            self.about_open = false;
        }
    }
}

/// A numbered step of instructions: a coral circle with the number, and the text beside it.
fn step(ui: &mut egui::Ui, n: usize, text: &str) {
    let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 13.0, pal.accent_tint);
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, n.to_string(), FontId::new(d.caption + 1.0, theme::semibold()), pal.accent_ink);
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(text).color(pal.label)).wrap());
    });
    ui.add_space(4.0);
}

/// A rounded box with selectable monospace text, for commands and settings to copy.
fn code_box(ui: &mut egui::Ui, text: &str) {
    let (pal, d) = (theme::current(ui.ctx()), theme::design(ui.ctx()));
    egui::Frame::NONE.fill(pal.surface2).corner_radius(egui::CornerRadius::same(d.radius_md)).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(text).monospace().size(12.0).color(pal.label)).selectable(true));
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn addon_is_complete() {
        let dir = std::env::temp_dir().join(format!("sdm-addon-{}", std::process::id()));
        let out = super::save_addon(&dir).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["manifest_version"], 3);
        assert_eq!(manifest["host_permissions"][0], "<all_urls>");
        let perms = manifest["permissions"].to_string();
        assert!(perms.contains("cookies") && perms.contains("webRequest") && perms.contains("downloads"));
        assert!(std::fs::read_to_string(out.join("background.js")).unwrap().contains("127.0.0.1:9614"));
        assert!(out.join("background.js").exists() && out.join("popup.js").exists());
        assert!(out.join("icons/icon-128.png").exists());
        assert!(out.join(manifest["icons"]["128"].as_str().unwrap()).exists());
    }
}
