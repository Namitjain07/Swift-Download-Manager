//! More windows: batch download, scheduler, download complete, browser integration, about.

use super::actions::{expand_pattern, normalize_time};
use super::*;
use crate::icons::{self, Icon};
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
        egui::Window::new("Batch download")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(560.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut b.pattern_mode, false, "A list of links");
                    ui.selectable_value(&mut b.pattern_mode, true, "Numbered links (pattern)");
                });
                ui.separator();
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
                    if ui.button("Browse…").clicked() {
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
                            ui.add(egui::Label::new(RichText::new(u).color(Color32::from_gray(100))).truncate());
                        }
                        if list.len() > 3 {
                            ui.label(RichText::new("…").color(Color32::from_gray(100)));
                        }
                    }
                    Ok(_) => {
                        ui.label(RichText::new("No links yet.").color(Color32::from_gray(120)));
                    }
                    Err(e) => {
                        ui.colored_label(Color32::from_rgb(200, 50, 40), e);
                    }
                }
                if let Some(e) = &b.error {
                    ui.colored_label(Color32::from_rgb(200, 50, 40), e);
                }
                ui.add_space(6.0);
                let ok = urls.as_ref().is_ok_and(|l| !l.is_empty());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    if ui.add_enabled(ok, egui::Button::new("Add to queue")).clicked() {
                        result = Some(false);
                    }
                    if ui.add_enabled(ok, egui::Button::new(RichText::new("Download all now").strong()).fill(Color32::from_rgb(204, 228, 247))).clicked() {
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
        egui::Window::new("Scheduler")
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
                    ui.label(RichText::new(format!("{queued} downloads waiting")).color(Color32::from_gray(110)));
                });
                ui.add_space(4.0);
                egui::Grid::new("sched").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.checkbox(&mut s.start_on, "Start the queue at");
                    ui.add(egui::TextEdit::singleline(&mut s.start_time).desired_width(60.0).hint_text("HH:MM"));
                    ui.end_row();
                    ui.checkbox(&mut s.stop_on, "Stop the queue at");
                    ui.add(egui::TextEdit::singleline(&mut s.stop_time).desired_width(60.0).hint_text("HH:MM"));
                    ui.end_row();
                    ui.label("Repeat:");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut s.daily, true, "Every day");
                        ui.radio_value(&mut s.daily, false, "Once");
                    });
                    ui.end_row();
                    ui.label("When the queue is done:");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut s.when_done, WhenDone::Nothing, "Do nothing");
                        ui.radio_value(&mut s.when_done, WhenDone::ExitApp, "Close the app");
                    });
                    ui.end_row();
                });
                let bad = (s.start_on && normalize_time(&s.start_time).is_empty()) || (s.stop_on && normalize_time(&s.stop_time).is_empty());
                if bad {
                    ui.colored_label(Color32::from_rgb(200, 50, 40), "Write times as hours and minutes, e.g. 02:30 or 23:15.");
                }
                ui.label(RichText::new(format!("Time now: {}", chrono::Local::now().format("%H:%M"))).color(Color32::from_gray(110)));
                ui.add_space(6.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        result = Some(false);
                    }
                    if ui.add_enabled(!bad, egui::Button::new(RichText::new("Save").strong())).clicked() {
                        result = Some(true);
                    }
                    if ui.button("Start queue now").clicked() {
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
        egui::Window::new("Queues")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                ui.label("Each queue downloads its files a few at a time and can have its own schedule.");
                ui.add_space(4.0);
                egui::Grid::new("queues").num_columns(4).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label(RichText::new("Name").strong());
                    ui.label(RichText::new("At once").strong());
                    ui.label("");
                    ui.label("");
                    ui.end_row();
                    for (k, q) in self.settings.queues.iter_mut().enumerate() {
                        let main = q.name == MAIN_QUEUE;
                        let mut name = q.name.clone();
                        let r = ui.add_enabled(!main, egui::TextEdit::singleline(&mut name).desired_width(170.0));
                        if r.changed() {
                            renames.push((q.name.clone(), name.clone()));
                        }
                        changed |= ui.add(egui::Slider::new(&mut q.max_parallel, 1..=8)).changed();
                        if ui.small_button("Scheduler…").clicked() {
                            sched = Some(q.name.clone());
                        }
                        if ui.add_enabled(!main, egui::Button::new("✕").small()).on_hover_text("Remove (its downloads stay in the list)").clicked() {
                            remove = Some(k);
                        }
                        ui.end_row();
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut new_name).hint_text("New queue name").desired_width(200.0));
                    if ui.add_enabled(!new_name.trim().is_empty(), egui::Button::new("+ Add queue")).clicked() {
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
        egui::Window::new("Refresh download address")
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
                    ui.colored_label(Color32::from_rgb(200, 50, 40), e);
                }
                ui.add_space(4.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    if ui.button(RichText::new("Use this link").strong()).clicked() {
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
            egui::Window::new("Download complete")
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
                    ui.add(egui::Label::new(RichText::new(&item.url).color(Color32::from_rgb(33, 100, 190))).truncate());
                    if let Some(c) = &item.check {
                        ui.label(c);
                    }
                    ui.add_space(4.0);
                    ui.checkbox(&mut dont_show, "Don't show this window again");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            act = Some("close");
                        }
                        if ui.button("Open folder").clicked() {
                            act = Some("folder");
                        }
                        if ui.button(RichText::new("Open").strong()).clicked() {
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
        egui::Window::new("Browser integration")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(480.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                match (&self.listener, self.settings.browser_listen) {
                    (Ok(()), _) => ui.colored_label(Color32::from_rgb(46, 140, 70), format!("● Ready: the app accepts downloads from the browser (port {}).", bridge::PORT)),
                    (Err(_), false) => ui.colored_label(Color32::from_gray(110), "○ Turned off in Options."),
                    (Err(e), true) => ui.colored_label(Color32::from_rgb(190, 60, 40), format!("● Not available: {e}. Another copy of the app may be open.")),
                };
                ui.add_space(6.0);
                ui.label("The browser add-on sends the downloads you start in Chrome, Edge, Brave or Firefox to this app, and adds “Download with Swift Download Manager” when you right-click a link.");
                ui.add_space(6.0);
                ui.label(RichText::new("How to install it").strong());
                ui.label("1.  Click “Save the add-on…” below and choose a folder.");
                ui.label("2.  Chrome/Edge: open the Extensions page, turn on Developer mode,\n     click “Load unpacked” and pick the saved folder.");
                ui.label("3.  Firefox: open about:debugging, “This Firefox”, “Load Temporary Add-on”\n     and pick manifest.json in the saved folder.");
                ui.add_space(6.0);
                ui.label(RichText::new("Without the add-on: copy a download link and the app will offer to download it (clipboard watching), or start the app with a link.").color(Color32::from_gray(100)));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Save the add-on…").strong()).clicked() {
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
                    if ui.checkbox(&mut self.settings.watch_clipboard, "Watch the clipboard").changed() {
                        self.apply_settings();
                        self.dirty = true;
                    }
                });
            });
        self.browser_open = open;
    }

    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let mut open = true;
        egui::Window::new("About")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), Sense::hover());
                    icons::draw(ui.painter(), r, Icon::Grabber, icons::tint(Icon::Grabber, &crate::theme::current(ui.ctx())), true);
                    ui.heading(APP_NAME);
                    ui.label("Version 1.2, written in Rust");
                    ui.label("Splits each download into several connections, re-splits as parts finish,\nretries broken connections, pauses and resumes, keeps a scheduled queue\nand works with your browser and clipboard.");
                });
            });
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
