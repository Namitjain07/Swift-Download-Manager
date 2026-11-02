//! Pop-up windows: add download, progress, options, delete, properties.

use super::*;
use crate::engine;
use crate::widgets::segment_bar;
use crate::widgets::{button_row, card, dialog_window, group_title, plain_button, primary_button, spinner, tinted_button, toggle, toggle_row};
use eframe::egui::RichText;

fn browse_folder(current: &mut String) {
    if let Some(p) = rfd::FileDialog::new().set_directory(current.as_str()).pick_folder() {
        *current = p.display().to_string();
    }
}

impl App {
    pub(super) fn add_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.add.take() else { return };
        let cat_names: Vec<String> = self.settings.categories.iter().map(|c| c.name.clone()).collect();
        let queue_names: Vec<String> = self.settings.queues.iter().map(|q| q.name.clone()).collect();
        let mut open = true;
        let mut result: Option<bool> = None;
        let mut cancel = false;
        dialog_window(ctx, "Enter new address to download")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(540.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -40.0])
            .show(ctx, |ui| {
                if let Some(src) = &d.from_outside {
                    ui.colored_label(crate::theme::current(ui.ctx()).blue, format!("ℹ {src}"));
                    ui.add_space(4.0);
                }
                egui::Grid::new("add_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label("Address:");
                    let r = ui.add(egui::TextEdit::singleline(&mut d.url).hint_text("https://…").desired_width(410.0));
                    if !d.focused {
                        r.request_focus();
                        d.focused = true;
                    }
                    if r.changed() && !d.name_edited {
                        d.name = engine::file_name_from_url(d.url.trim());
                        d.category = self.cat_of(&d.name);
                        d.folder = self.folder_for(&d.category).display().to_string();
                    }
                    ui.end_row();
                    ui.label("Save as:");
                    if ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(410.0)).changed() {
                        d.name_edited = true;
                    }
                    ui.end_row();
                    ui.label("Category:");
                    let before = d.category.clone();
                    egui::ComboBox::from_id_salt("cat").selected_text(d.category.clone()).show_ui(ui, |ui| {
                        for c in &cat_names {
                            ui.selectable_value(&mut d.category, c.clone(), c);
                        }
                    });
                    if before != d.category {
                        d.folder = self.folder_for(&d.category).display().to_string();
                    }
                    ui.end_row();
                    ui.label("Save to:");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut d.folder).desired_width(330.0));
                        if tinted_button(ui, "Browse…").clicked() {
                            browse_folder(&mut d.folder);
                        }
                    });
                    ui.end_row();
                    ui.label("Description:");
                    ui.add(egui::TextEdit::singleline(&mut d.description).desired_width(410.0));
                    ui.end_row();
                });
                egui::CollapsingHeader::new("More options (login, referrer, checksum)").show(ui, |ui| {
                    egui::Grid::new("add_adv").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label("Referrer page:");
                        ui.add(egui::TextEdit::singleline(&mut d.referer).hint_text("Page the link came from (some sites need it)").desired_width(360.0));
                        ui.end_row();
                        ui.label("User name:");
                        ui.add(egui::TextEdit::singleline(&mut d.user).desired_width(200.0));
                        ui.end_row();
                        ui.label("Password:");
                        ui.add(egui::TextEdit::singleline(&mut d.pass).password(true).desired_width(200.0));
                        ui.end_row();
                        ui.label("SHA-256:");
                        ui.add(egui::TextEdit::singleline(&mut d.expected_sha).hint_text("Optional: checked when the download finishes").desired_width(360.0));
                        ui.end_row();
                    });
                });
                if let Some(e) = &d.error {
                    ui.colored_label(crate::theme::current(ui.ctx()).red, e);
                }
                ui.add_space(6.0);
                button_row(ui, |ui| {
                    if plain_button(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                    egui::ComboBox::from_id_salt("add_q").selected_text(d.queue.clone()).width(110.0).show_ui(ui, |ui| {
                        for q in &queue_names {
                            ui.selectable_value(&mut d.queue, q.clone(), q);
                        }
                    });
                    if tinted_button(ui, "Download Later").on_hover_text("Add to the queue picked next to this button").clicked() {
                        result = Some(false);
                    }
                    let start = primary_button(ui, "Start Download");
                    if start.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        result = Some(true);
                    }
                });
            });
        if !open || cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return;
        }
        if let Some(now) = result {
            d.error = None;
            match self.add_download(&d, now) {
                Ok(_) => return,
                Err(e) => d.error = Some(e),
            }
        }
        self.add = Some(d);
    }

    pub(super) fn progress_windows(&mut self, ctx: &egui::Context) {
        let ids: Vec<u64> = self.progress_open.iter().copied().collect();
        for id in ids {
            let Some(idx) = self.items.iter().position(|i| i.id == id) else {
                self.progress_open.remove(&id);
                continue;
            };
            let mut open = true;
            let mut act: Option<&str> = None;
            let mut details = self.details_open.contains(&id);
            let mut lim_on = self.items[idx].limit_kbps > 0;
            let mut lim_val = if lim_on { self.items[idx].limit_kbps } else { 256 };
            {
                let item = &self.items[idx];
                let st = item.status();
                let title = format!("{:.0}% {}", item.progress() * 100.0, item.name);
                dialog_window(ctx, title)
                    .id(egui::Id::unique(("progress", id)))
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .default_width(470.0)
                    .default_pos(egui::pos2(280.0 + (id % 5) as f32 * 24.0, 150.0 + (id % 5) as f32 * 24.0))
                    .show(ctx, |ui| {
                        egui::Grid::new(("pg", id)).num_columns(2).spacing([14.0, 4.0]).show(ui, |ui| {
                            ui.label("Address");
                            ui.add(egui::Label::new(RichText::new(&item.url).color(crate::theme::current(ui.ctx()).blue)).truncate());
                            ui.end_row();
                            ui.label("Status");
                            let s = match st {
                                Status::Error => item.live.error.lock().unwrap().clone().unwrap_or_else(|| "Error".into()),
                                s => item.live.note.lock().unwrap().clone().unwrap_or_else(|| s.label().to_string()),
                            };
                            ui.label(RichText::new(s).strong());
                            ui.end_row();
                            ui.label("File size");
                            ui.label(if item.total() > 0 { human_size(item.total()) } else { "Unknown".into() });
                            ui.end_row();
                            ui.label("Downloaded");
                            ui.label(if item.total() > 0 {
                                format!("{} ({:.2} %)", human_size(item.done()), item.progress() * 100.0)
                            } else {
                                human_size(item.done())
                            });
                            ui.end_row();
                            ui.label("Transfer rate");
                            ui.label(human_speed(item.speed()));
                            ui.end_row();
                            ui.label("Time left");
                            ui.label(item.time_left());
                            ui.end_row();
                            ui.label("Connections");
                            ui.label(format!("{}", item.live.connections.load(Ordering::Relaxed)));
                            ui.end_row();
                            ui.label("Resume capability");
                            ui.label(if item.live.resumable.load(Ordering::Relaxed) {
                                "Yes"
                            } else if item.total() == 0 && st != Status::Complete {
                                "Unknown"
                            } else {
                                "No"
                            });
                            ui.end_row();
                        });
                        ui.add_space(6.0);
                        let segs = item.live.segments.lock().unwrap().clone();
                        segment_bar(ui, item.progress(), item.total(), &segs);
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            toggle(ui, &mut lim_on, "Limit this download to");
                            ui.add_enabled(lim_on, egui::DragValue::new(&mut lim_val).range(1..=1_000_000).suffix(" KB/s"));
                        });
                        ui.horizontal(|ui| {
                            if tinted_button(ui, if details { "<< Hide details" } else { "Show details >>" }).clicked() {
                                details = !details;
                            }
                            button_row(ui, |ui| {
                                if plain_button(ui, "Cancel").on_hover_text("Stop and close").clicked() {
                                    act = Some("cancel");
                                }
                                if st.is_active() {
                                    if tinted_button(ui, "Pause").clicked() {
                                        act = Some("pause");
                                    }
                                } else if st == Status::Complete {
                                    if tinted_button(ui, "Open").clicked() {
                                        act = Some("open");
                                    }
                                    if tinted_button(ui, "Open folder").clicked() {
                                        act = Some("folder");
                                    }
                                } else if tinted_button(ui, "Resume").clicked() {
                                    act = Some("resume");
                                }
                            });
                        });
                        if details {
                            ui.separator();
                            if segs.is_empty() {
                                ui.label(RichText::new("Single connection (the server does not allow splitting).").color(crate::theme::current(ui.ctx()).secondary));
                            } else {
                                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                                    egui::Grid::new(("segs", id)).striped(true).num_columns(3).spacing([18.0, 2.0]).show(ui, |ui| {
                                        ui.label(RichText::new("N.").strong());
                                        ui.label(RichText::new("Downloaded").strong());
                                        ui.label(RichText::new("Info").strong());
                                        ui.end_row();
                                        for (k, s) in segs.iter().enumerate() {
                                            ui.label(format!("{}", k + 1));
                                            ui.label(format!("{} of {}", human_size(s.done.min(s.len())), human_size(s.len())));
                                            ui.label(if s.finished() {
                                                "Done".to_string()
                                            } else if st == Status::Downloading {
                                                "Receiving data…".to_string()
                                            } else {
                                                format!("{:.0}%", s.done as f64 / s.len().max(1) as f64 * 100.0)
                                            });
                                            ui.end_row();
                                        }
                                    });
                                });
                            }
                        }
                    });
            }
            let new_limit = if lim_on { lim_val.max(1) } else { 0 };
            if new_limit != self.items[idx].limit_kbps {
                self.items[idx].set_limit(new_limit);
                self.dirty = true;
            }
            if details {
                self.details_open.insert(id);
            } else {
                self.details_open.remove(&id);
            }
            match act {
                Some("pause") => self.stop_item(id),
                Some("resume") => self.start_item(id),
                Some("cancel") => {
                    self.stop_item(id);
                    open = false;
                }
                Some("open") => open_path(&self.items[idx].path()),
                Some("folder") => open_path(&self.items[idx].folder),
                _ => {}
            }
            if !open {
                self.progress_open.remove(&id);
            }
        }
    }

    pub(super) fn options_window(&mut self, ctx: &egui::Context) {
        let Some(mut s) = self.options.take() else { return };
        let mut open = true;
        let mut result = None;
        dialog_window(ctx, "Options")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(540.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -10.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(ctx.content_rect().height() - 140.0).show(ui, |ui| options_body(ui, &mut s, &mut result));
            });
        match result {
            Some(true) => {
                // Queues and schedules have their own windows; keep their current state.
                s.queues = self.settings.queues.clone();
                s.schedule = self.settings.schedule.clone();
                s.site_logins.retain(|l| !l.site.trim().is_empty());
                let mut seen = Vec::new();
                for c in &mut s.categories {
                    c.name = c.name.trim().to_string();
                    while seen.contains(&c.name) {
                        c.name.push_str(" 2");
                    }
                    seen.push(c.name.clone());
                }
                self.settings = s;
                self.fix_settings();
                self.apply_settings();
                self.dirty = true;
            }
            Some(false) => {}
            None if open => self.options = Some(s),
            None => {}
        }
    }

    pub(super) fn delete_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.delete.take() else { return };
        let mut result = None;
        let any_done = d.ids.iter().any(|id| self.items.iter().any(|i| i.id == *id && i.status() == Status::Complete));
        dialog_window(ctx, "Confirm deletion")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(if d.ids.len() == 1 { "Remove this download from the list?".to_string() } else { format!("Remove {} downloads from the list?", d.ids.len()) });
                if any_done {
                    toggle(ui, &mut d.delete_files, "Also delete the downloaded files from disk");
                }
                ui.add_space(6.0);
                button_row(ui, |ui| {
                    if plain_button(ui, "Cancel").clicked() {
                        result = Some(false);
                    }
                    if primary_button(ui, "Remove").clicked() {
                        result = Some(true);
                    }
                });
            });
        match result {
            Some(true) => {
                let ids = d.ids.clone();
                self.remove_items(&ids, d.delete_files);
            }
            Some(false) => {}
            None => self.delete = Some(d),
        }
    }

    pub(super) fn properties_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.properties else { return };
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else {
            self.properties = None;
            return;
        };
        let mut open = true;
        let mut close = false;
        let mut verify = false;
        dialog_window(ctx, "File properties")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(500.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .show(ctx, |ui| {
                egui::Grid::new("props").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("File name");
                    ui.label(RichText::new(&item.name).strong());
                    ui.end_row();
                    ui.label("Category");
                    ui.label(&item.category);
                    ui.end_row();
                    ui.label("Saved to");
                    ui.label(item.folder.display().to_string());
                    ui.end_row();
                    ui.label("Address");
                    ui.add(egui::Label::new(&item.url).truncate());
                    ui.end_row();
                    if !item.referer.is_empty() {
                        ui.label("Referrer");
                        ui.add(egui::Label::new(&item.referer).truncate());
                        ui.end_row();
                    }
                    ui.label("Size");
                    ui.label(human_size(item.total()));
                    ui.end_row();
                    ui.label("Status");
                    ui.label(item.status().label());
                    ui.end_row();
                    ui.label("Added");
                    ui.label(&item.added);
                    ui.end_row();
                    ui.label("Description");
                    ui.text_edit_singleline(&mut item.description);
                    ui.end_row();
                    ui.label("Expected SHA-256");
                    ui.add(egui::TextEdit::singleline(&mut item.expected_sha).hint_text("Paste the checksum from the website").desired_width(330.0));
                    ui.end_row();
                    ui.label("Checksum");
                    ui.horizontal(|ui| {
                        let done = item.status() == Status::Complete;
                        if ui.add_enabled(done && !item.is_checking, egui::Button::new("Check SHA-256")).clicked() {
                            verify = true;
                        }
                        if item.is_checking {
                            spinner(ui, 14.0);
                            ui.label(RichText::new("Checking…").color(crate::theme::current(ui.ctx()).secondary));
                        }
                        if let Some(c) = &item.check {
                            let col = if c.contains("NOT") { crate::theme::current(ui.ctx()).red } else if c.contains("matches") { crate::theme::current(ui.ctx()).green } else { crate::theme::current(ui.ctx()).secondary };
                            let r = ui.add(egui::Label::new(RichText::new(c).color(col)).truncate().sense(egui::Sense::click()));
                            if r.on_hover_text("Click to copy").clicked() {
                                ui.ctx().copy_text(c.trim_start_matches("SHA-256: ").to_string());
                            }
                        }
                    });
                    ui.end_row();
                });
                ui.add_space(6.0);
                button_row(ui, |ui| {
                    if tinted_button(ui, "Close").clicked() {
                        close = true;
                    }
                    if ui.add_enabled(item.status() == Status::Complete, egui::Button::new("Open")).clicked() {
                        open_path(&item.path());
                    }
                    if tinted_button(ui, "Open folder").clicked() {
                        open_path(&item.folder);
                    }
                });
            });
        item.expected_sha = item.expected_sha.trim().to_ascii_lowercase();
        if verify {
            self.verify(id);
        }
        if !open || close {
            self.properties = None;
            self.dirty = true;
        }
    }
}

/// The contents of the Options window.
fn options_body(ui: &mut egui::Ui, s: &mut Settings, result: &mut Option<bool>) {
    group_title(ui, "Downloads");
        egui::Grid::new("opts").num_columns(2).spacing([12.0, 7.0]).show(ui, |ui| {
            ui.label("Download folder:");
            ui.horizontal(|ui| {
                let mut f = s.folder.display().to_string();
                let mut changed = ui.add(egui::TextEdit::singleline(&mut f).desired_width(250.0)).changed();
                if tinted_button(ui, "Browse…").clicked() {
                    browse_folder(&mut f);
                    changed = true;
                }
                if changed {
                    s.folder = PathBuf::from(f);
                }
            });
            ui.end_row();
            ui.label("");
            toggle(ui, &mut s.per_category_folders, "Use a sub-folder for each category");
            ui.end_row();
            ui.label("Connections per download:");
            ui.add(egui::Slider::new(&mut s.connections, 1..=32));
            ui.end_row();
            ui.label("Retries on errors:");
            ui.add(egui::Slider::new(&mut s.retries, 0..=20));
            ui.end_row();
            ui.label("Speed limiter:");
            ui.horizontal(|ui| {
                toggle(ui, &mut s.limit_on, "Limit to");
                ui.add_enabled(s.limit_on, egui::DragValue::new(&mut s.limit_kbps).range(1..=1_000_000).suffix(" KB/s"));
            });
            ui.end_row();
            ui.label("Proxy:");
            ui.horizontal(|ui| {
                toggle(ui, &mut s.proxy_on, "Use");
                ui.add_enabled(s.proxy_on, egui::TextEdit::singleline(&mut s.proxy).hint_text("http://host:port").desired_width(220.0));
            });
            ui.end_row();
        });
        ui.separator();
        group_title(ui, "Windows");
        card(ui, |ui| {
            toggle_row(ui, "Show the progress window when a download starts", &mut s.show_progress, false);
            toggle_row(ui, "Show a window when a download completes", &mut s.show_complete, true);
        });
        group_title(ui, "Catching links");
        card(ui, |ui| {
            toggle_row(ui, "Watch the clipboard and offer to download copied file links", &mut s.watch_clipboard, false);
            toggle_row(ui, "Accept downloads from the browser add-on (restart to apply)", &mut s.browser_listen, false);
            ui.add_enabled_ui(s.browser_listen, |ui| toggle_row(ui, "Start browser downloads straight away without asking", &mut s.browser_start_now, true));
        });
        group_title(ui, "AI assistants");
        card(ui, |ui| {
            toggle_row(ui, "Let AI assistants add, pause and remove downloads (MCP)", &mut s.mcp_enabled, true);
        });
        ui.label(RichText::new("They cannot delete files or choose folders. Setup: Help > AI assistants…").size(11.0).color(crate::theme::current(ui.ctx()).secondary));
        ui.add_space(8.0);
        egui::CollapsingHeader::new(RichText::new("Categories (file types and folders)").strong()).show(ui, |ui| {
            let mut remove = None;
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                egui::Grid::new("cats").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
                    ui.label(RichText::new("Name").strong());
                    ui.label(RichText::new("File endings").strong());
                    ui.label(RichText::new("Own folder (optional)").strong());
                    ui.label("");
                    ui.end_row();
                    for (k, c) in s.categories.iter_mut().enumerate() {
                        let general = c.name == "General";
                        ui.add_enabled(!general, egui::TextEdit::singleline(&mut c.name).desired_width(90.0));
                        ui.add(egui::TextEdit::singleline(&mut c.exts).desired_width(190.0).hint_text(if general { "everything else" } else { "zip rar 7z" }));
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut c.folder).desired_width(120.0));
                            if ui.small_button("…").on_hover_text("Choose folder").clicked() {
                                browse_folder(&mut c.folder);
                            }
                        });
                        if ui.add_enabled(!general, egui::Button::new("✕").small()).on_hover_text("Remove").clicked() {
                            remove = Some(k);
                        }
                        ui.end_row();
                    }
                });
            });
            if let Some(k) = remove {
                s.categories.remove(k);
            }
            if tinted_button(ui, "+ Add category").clicked() {
                let pos = s.categories.iter().position(|c| c.name == "General").unwrap_or(s.categories.len());
                s.categories.insert(pos, CategoryDef { name: "New category".into(), exts: String::new(), folder: String::new() });
            }
        });
        egui::CollapsingHeader::new(RichText::new("Site logins").strong()).show(ui, |ui| {
            ui.label(RichText::new("Used automatically for downloads from these sites.").color(crate::theme::current(ui.ctx()).secondary));
            let mut remove = None;
            egui::Grid::new("logins").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
                ui.label(RichText::new("Site").strong());
                ui.label(RichText::new("User name").strong());
                ui.label(RichText::new("Password").strong());
                ui.label("");
                ui.end_row();
                for (k, l) in s.site_logins.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(&mut l.site).hint_text("example.com").desired_width(150.0));
                    ui.add(egui::TextEdit::singleline(&mut l.user).desired_width(110.0));
                    ui.add(egui::TextEdit::singleline(&mut l.pass).password(true).desired_width(110.0));
                    if ui.small_button("✕").clicked() {
                        remove = Some(k);
                    }
                    ui.end_row();
                }
            });
            if let Some(k) = remove {
                s.site_logins.remove(k);
            }
            if tinted_button(ui, "+ Add login").clicked() {
                s.site_logins.push(SiteLogin::default());
            }
        });
        ui.add_space(8.0);
        button_row(ui, |ui| {
            if plain_button(ui, "Cancel").clicked() {
                *result = Some(false);
            }
            if primary_button(ui, "OK").clicked() {
                *result = Some(true);
            }
        });
}
