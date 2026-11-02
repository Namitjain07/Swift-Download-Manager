//! More windows: batch download, scheduler, download complete, browser integration, about.

use super::actions::{expand_pattern, normalize_time};
use super::*;
use crate::icons;
use super::mcp_cmds::AiState;
use crate::widgets::{button_row, card, card_row, card_row_h, dialog_window, group_title, plain_button, primary_button, segmented, time_wheels, tinted_button, toggle, toggle_row};
use eframe::egui::{RichText, Sense};

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
        let mut open = true;
        let mut cancel = false;
        let mut result: Option<bool> = None;
        let urls: Result<Vec<String>, String> = if b.pattern_mode {
            expand_pattern(b.pattern.trim(), b.from, b.to, b.width)
        } else {
            Ok(b.links.split_whitespace().map(|s| s.to_string()).collect())
        };
        dialog_window(ctx, "Batch download")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(560.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                segmented(ui, &[(false, "A list of links"), (true, "Numbered links (pattern)")], &mut b.pattern_mode);
                ui.add_space(8.0);
                if b.pattern_mode {
                    ui.label("Address with a * where the number goes:");
                    ui.add(egui::TextEdit::singleline(&mut b.pattern).desired_width(520.0));
                    ui.horizontal(|ui| {
                        ui.label("From");
                        ui.add(egui::DragValue::new(&mut b.from).range(0..=1_000_000));
                        ui.label("to");
                        ui.add(egui::DragValue::new(&mut b.to).range(0..=1_000_000));
                        ui.label("   Digits");
                        ui.add(egui::DragValue::new(&mut b.width).range(1..=8)).on_hover_text("2 digits makes 1 into 01");
                    });
                } else {
                    ui.label("Paste links, one per line:");
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        ui.add(egui::TextEdit::multiline(&mut b.links).desired_rows(8).desired_width(520.0).hint_text("https://example.com/file1.zip\nhttps://example.com/file2.zip"));
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Save to:");
                    ui.add(egui::TextEdit::singleline(&mut b.folder).desired_width(300.0));
                    if tinted_button(ui, "Browse…").clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_folder() {
                            b.folder = p.display().to_string();
                        }
                    }
                    ui.label("Queue:");
                    egui::ComboBox::from_id_salt("batch_q").selected_text(b.queue.clone()).show_ui(ui, |ui| {
                        for q in &queue_names {
                            ui.selectable_value(&mut b.queue, q.clone(), q);
                        }
                    });
                });
                ui.separator();
                match &urls {
                    Ok(list) if !list.is_empty() => {
                        ui.label(RichText::new(format!("{} links", list.len())).strong());
                        for u in list.iter().take(3) {
                            ui.add(egui::Label::new(RichText::new(u).color(crate::theme::current(ui.ctx()).secondary)).truncate());
                        }
                        if list.len() > 3 {
                            ui.label(RichText::new("…").color(crate::theme::current(ui.ctx()).secondary));
                        }
                    }
                    Ok(_) => {
                        ui.label(RichText::new("No links yet.").color(crate::theme::current(ui.ctx()).secondary));
                    }
                    Err(e) => {
                        ui.colored_label(crate::theme::current(ui.ctx()).red, e);
                    }
                }
                if let Some(e) = &b.error {
                    ui.colored_label(crate::theme::current(ui.ctx()).red, e);
                }
                ui.add_space(6.0);
                let ok = urls.as_ref().is_ok_and(|l| !l.is_empty());
                button_row(ui, |ui| {
                    if plain_button(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                    if ui.add_enabled(ok, egui::Button::new("Add to queue")).clicked() {
                        result = Some(false);
                    }
                    if ui.add_enabled_ui(ok, |ui| primary_button(ui, "Download all now")).inner.clicked() {
                        result = Some(true);
                    }
                });
            });
        if !open || cancel {
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
        let mut open = true;
        let mut result = None;
        let mut switch_to: Option<String> = None;
        let mut start_now = false;
        let names: Vec<String> = self.settings.queues.iter().map(|q| q.name.clone()).collect();
        let queued = self.items.iter().filter(|i| i.status() == Status::Queued && i.queue.as_deref() == Some(qname.as_str())).count();
        dialog_window(ctx, "Scheduler")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(440.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Queue:");
                    egui::ComboBox::from_id_salt("sched_q").selected_text(qname.clone()).show_ui(ui, |ui| {
                        for n in &names {
                            if ui.selectable_label(*n == qname, n).clicked() && *n != qname {
                                switch_to = Some(n.clone());
                            }
                        }
                    });
                    ui.label(RichText::new(format!("{queued} downloads waiting")).color(crate::theme::current(ui.ctx()).secondary));
                });
                ui.add_space(8.0);
                card(ui, |ui| {
                    toggle_row(ui, "Start the queue at", &mut s.start_on, false);
                    if s.start_on {
                        card_row_h(ui, false, 96.0, |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| time_wheels(ui, egui::Id::unique(("sched_start", &qname)), &mut s.start_time));
                        });
                    }
                    toggle_row(ui, "Stop the queue at", &mut s.stop_on, false);
                    if s.stop_on {
                        card_row_h(ui, false, 96.0, |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| time_wheels(ui, egui::Id::unique(("sched_stop", &qname)), &mut s.stop_time));
                        });
                    }
                    card_row(ui, false, |ui| {
                        ui.label("Repeat");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| segmented(ui, &[(true, "Every day"), (false, "Once")], &mut s.daily));
                    });
                    card_row(ui, true, |ui| {
                        ui.label("When the queue is done");
                        button_row(ui, |ui| {
                            segmented(ui, &[(WhenDone::Nothing, "Do nothing"), (WhenDone::ExitApp, "Close the app")], &mut s.when_done)
                        });
                    });
                });
                let bad = (s.start_on && normalize_time(&s.start_time).is_empty()) || (s.stop_on && normalize_time(&s.stop_time).is_empty());
                if bad {
                    ui.colored_label(crate::theme::current(ui.ctx()).red, "Write times as hours and minutes, e.g. 02:30 or 23:15.");
                }
                ui.label(RichText::new(format!("Time now: {}", chrono::Local::now().format("%H:%M"))).color(crate::theme::current(ui.ctx()).secondary));
                ui.add_space(6.0);
                button_row(ui, |ui| {
                    if plain_button(ui, "Cancel").clicked() {
                        result = Some(false);
                    }
                    if ui.add_enabled_ui(!bad, |ui| primary_button(ui, "Save")).inner.clicked() {
                        result = Some(true);
                    }
                    if tinted_button(ui, "Start queue now").clicked() {
                        start_now = true;
                    }
                });
            });
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
            None if open => self.scheduler = Some((qname, s)),
            None => {}
        }
    }

    /// Create, rename and remove queues, and set how many downloads each runs at once.
    pub(super) fn queues_window(&mut self, ctx: &egui::Context) {
        if !self.queues_open {
            return;
        }
        let mut open = true;
        let mut remove: Option<usize> = None;
        let mut renames: Vec<(String, String)> = Vec::new();
        let mut add = false;
        let mut sched: Option<String> = None;
        let mut new_name = std::mem::take(&mut self.new_queue);
        let mut changed = false;
        dialog_window(ctx, "Queues")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                ui.label("Each queue downloads its files a few at a time (the slider sets how many at once) and can have its own schedule.");
                ui.add_space(4.0);
                card(ui, |ui| {
                    let count = self.settings.queues.len();
                    for (k, q) in self.settings.queues.iter_mut().enumerate() {
                        card_row(ui, k + 1 == count, |ui| {
                            let main = q.name == MAIN_QUEUE;
                            let mut name = q.name.clone();
                            if ui.add_enabled(!main, egui::TextEdit::singleline(&mut name).desired_width(104.0)).changed() {
                                renames.push((q.name.clone(), name.clone()));
                            }
                            ui.spacing_mut().slider_width = 64.0;
                            changed |= ui.add(egui::Slider::new(&mut q.max_parallel, 1..=8)).on_hover_text("How many downloads this queue runs at once").changed();
                            button_row(ui, |ui| {
                                if ui.add_enabled(!main, egui::Button::new("✕").small()).on_hover_text("Remove (its downloads stay in the list)").clicked() {
                                    remove = Some(k);
                                }
                                if tinted_button(ui, "Scheduler…").clicked() {
                                    sched = Some(q.name.clone());
                                }
                            });
                        });
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut new_name).hint_text("New queue name").desired_width(200.0));
                    if ui.add_enabled_ui(!new_name.trim().is_empty(), |ui| tinted_button(ui, "Add queue")).inner.clicked() {
                        add = true;
                    }
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
        if let Some(k) = remove {
            let name = self.settings.queues[k].name.clone();
            self.stop_queue(&name);
            self.settings.queues.remove(k);
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
        self.queues_open = open;
        if changed {
            self.dirty = true;
        }
    }

    /// Paste a fresh link for a download whose old link stopped working.
    pub(super) fn refresh_window(&mut self, ctx: &egui::Context) {
        let Some(mut r) = self.refresh.take() else { return };
        let Some(item) = self.items.iter().find(|i| i.id == r.id) else { return };
        let name = item.name.clone();
        let mut open = true;
        let mut ok = false;
        let mut cancel = false;
        dialog_window(ctx, "Refresh download address")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                ui.label(RichText::new(&name).strong());
                ui.label("If the link stopped working (many sites give links that expire), open the download page again, copy a fresh link to the same file and paste it here. The download carries on where it stopped.");
                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut r.url).desired_width(500.0));
                if let Some(e) = &r.error {
                    ui.colored_label(crate::theme::current(ui.ctx()).red, e);
                }
                ui.add_space(4.0);
                button_row(ui, |ui| {
                    if plain_button(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                    if primary_button(ui, "Use this link").clicked() {
                        ok = true;
                    }
                });
            });
        if !open || cancel {
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
            let mut open = true;
            let mut act = None;
            let mut dont_show = !self.settings.show_complete;
            dialog_window(ctx, "Download complete")
                .id(egui::Id::unique(("complete", id)))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(440.0)
                .default_pos(egui::pos2(360.0 + (id % 4) as f32 * 20.0, 200.0 + (id % 4) as f32 * 20.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), Sense::hover());
                        icons::file_badge(ui.painter(), r, &item.category);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&item.name).strong().size(15.0));
                            ui.label(format!("{}  ·  {}", human_size(item.total()), item.folder.display()));
                        });
                    });
                    ui.add(egui::Label::new(RichText::new(&item.url).color(crate::theme::current(ui.ctx()).blue)).truncate());
                    if let Some(c) = &item.check {
                        ui.label(c);
                    }
                    ui.add_space(4.0);
                    toggle(ui, &mut dont_show, "Don't show this window again");
                    button_row(ui, |ui| {
                        if tinted_button(ui, "Close").clicked() {
                            act = Some("close");
                        }
                        if tinted_button(ui, "Open folder").clicked() {
                            act = Some("folder");
                        }
                        if primary_button(ui, "Open").clicked() {
                            act = Some("open");
                        }
                    });
                });
            if dont_show == self.settings.show_complete {
                self.settings.show_complete = !dont_show;
                self.dirty = true;
            }
            match act {
                Some("open") => open_path(&item.path()),
                Some("folder") => open_path(&item.folder),
                _ => {}
            }
            if !open || act.is_some() {
                self.completed.retain(|c| *c != id);
            }
        }
    }

    pub(super) fn browser_window(&mut self, ctx: &egui::Context) {
        if !self.browser_open {
            return;
        }
        let mut open = true;
        dialog_window(ctx, "Browser integration")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(480.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                match (&self.listener, self.settings.browser_listen) {
                    (Ok(()), _) => ui.colored_label(crate::theme::current(ui.ctx()).green, format!("● Ready: the app accepts downloads from the browser (port {}).", bridge::PORT)),
                    (Err(_), false) => ui.colored_label(crate::theme::current(ui.ctx()).secondary, "○ Turned off in Options."),
                    (Err(e), true) => ui.colored_label(crate::theme::current(ui.ctx()).red, format!("● Not available: {e}. Another copy of the app may be open.")),
                };
                ui.add_space(6.0);
                ui.label("The browser add-on sends the downloads you start in Chrome, Edge, Brave or Firefox to this app, and adds “Download with Swift Download Manager” when you right-click a link.");
                ui.add_space(6.0);
                ui.label(RichText::new("How to install it").strong());
                ui.label("1.  Click “Save the add-on…” below and choose a folder.");
                ui.label("2.  Chrome/Edge: open the Extensions page, turn on Developer mode,\n     click “Load unpacked” and pick the saved folder.");
                ui.label("3.  Firefox: open about:debugging, “This Firefox”, “Load Temporary Add-on”\n     and pick manifest.json in the saved folder.");
                ui.add_space(6.0);
                ui.label(RichText::new("Without the add-on: copy a download link and the app will offer to download it (clipboard watching), or start the app with a link.").color(crate::theme::current(ui.ctx()).secondary));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Save the add-on…").clicked() {
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            match save_addon(&dir) {
                                Ok(p) => {
                                    open_path(&p);
                                    self.toast = Some((format!("Saved to {}", p.display()), Instant::now()));
                                }
                                Err(e) => self.toast = Some((format!("Could not save: {e}"), Instant::now())),
                            }
                        }
                    }
                    if toggle(ui, &mut self.settings.watch_clipboard, "Watch the clipboard").changed() {
                        self.apply_settings();
                        self.dirty = true;
                    }
                });
            });
        self.browser_open = open;
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
        let mut open = true;
        dialog_window(ctx, "AI assistants (MCP)")
            .open(&mut open)
            .default_width(540.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                let pal = crate::theme::current(ui.ctx());
                ui.label("Let an AI assistant such as Claude look at your downloads and add, pause, resume or remove them. It cannot delete files from your disk or pick the folder a file is saved in.");
                ui.add_space(8.0);
                card(ui, |ui| {
                    toggle_row(ui, "Allow AI assistants", &mut on, false);
                    card_row(ui, true, |ui| {
                        let (text, col) = match state {
                            AiState::Off => ("Off: assistants are refused", pal.secondary),
                            AiState::Unavailable => ("Another copy of the app owns the downloads, so this window cannot be controlled", pal.orange),
                            AiState::Ready => ("Ready: connect an assistant below", pal.green),
                            AiState::Active => ("An assistant is using it right now", pal.blue),
                        };
                        ui.label(RichText::new(text).size(11.0).color(col));
                    });
                });
                for (title, text) in [("Claude Code: run this once", &code), ("Claude Desktop and other apps: add this to their MCP settings", &desktop)] {
                    group_title(ui, title);
                    ui.add(egui::Label::new(RichText::new(text.as_str()).monospace().size(11.0)).selectable(true));
                    ui.add_space(4.0);
                    if tinted_button(ui, "Copy").clicked() {
                        copied = Some(text.clone());
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new("The assistant starts this program itself and shares your open window, or runs the downloads without one while it is closed. Clients ask you before each action unless you tell them not to.").size(11.0).color(pal.secondary));
            });
        if on != self.settings.mcp_enabled {
            self.settings.mcp_enabled = on;
            self.dirty = true;
        }
        if let Some(text) = copied {
            ctx.copy_text(text);
            self.toast("Copied");
        }
        self.ai_open = open;
    }

    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let mut open = true;
        let mut about_icon = self.about_icon.take();
        dialog_window(ctx, "About")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let icon = about_icon.get_or_insert_with(|| ctx.load_texture("about-icon", egui::ColorImage::from_rgba_unmultiplied([128, 128], &crate::appicon::rgba(128)), egui::TextureOptions::LINEAR));
                    ui.add(egui::Image::new((icon.id(), egui::vec2(72.0, 72.0))));
                    ui.heading(APP_NAME);
                    ui.label(format!("Version {}, written in Rust", env!("CARGO_PKG_VERSION")));
                    ui.label("Splits each download into several connections, re-splits as parts finish,\nretries broken connections, pauses and resumes, keeps a scheduled queue\nand works with your browser and clipboard.");
                });
            });
        self.about_icon = about_icon;
        self.about_open = open;
    }
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
