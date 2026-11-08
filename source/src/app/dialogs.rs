//! Pop-up windows: add download, progress, options, delete, properties.

use super::closing::BackgroundMode;
use super::*;
use crate::engine;
use crate::icons::Icon;
use crate::theme::ThemeMode;
use crate::widgets::{
    button, button_width, card, check_row, disclosure, dropdown, field, field_full, form_field, hint, icon_button, kv, kv_text, message, notice, number_field, plain_button, primary_button,
    secondary_button, section_title, segment_bar, segmented, segmented_width, sheet, small_button, stepper, switch, ButtonKind, ButtonSize,
    FlowChannel, FlowState,
};
use eframe::egui::RichText;

fn browse_folder(current: &mut String) {
    if let Some(p) = rfd::FileDialog::new().set_directory(current.as_str()).pick_folder() {
        *current = p.display().to_string();
    }
}

/// A folder field with a Browse button at its right end. True when the path changed.
pub(super) fn path_field(ui: &mut egui::Ui, text: &mut String) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let bw = button_width(ui, ButtonSize::Regular, false, "Browse…");
        let w = (ui.available_width() - bw - ui.spacing().item_spacing.x).max(80.0);
        changed |= ui.add(field(text).desired_width(w)).changed();
        if button(ui, ButtonKind::Outline, "Browse…").clicked() {
            browse_folder(text);
            changed = true;
        }
    });
    changed
}

/// The names of the queues or categories as options for a drop-down.
pub(super) fn options_of(names: &[String]) -> Vec<(String, String)> {
    names.iter().map(|n| (n.clone(), n.clone())).collect()
}

impl App {
    pub(super) fn add_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.add.take() else { return };
        let cat_names: Vec<String> = self.settings.categories.iter().map(|c| c.name.clone()).collect();
        let queue_names: Vec<String> = self.settings.queues.iter().map(|q| q.name.clone()).collect();
        let (cats, queues) = (options_of(&cat_names), options_of(&queue_names));
        let pal = theme::current(ctx);
        let mut result: Option<bool> = None;
        let mut cancel = false;
        let out = sheet("add", "New download").width(580.0).show(ctx, |s| {
            s.body(|ui| {
                if let Some(src) = &d.from_outside {
                    notice(ui, Icon::Info, src, pal.blue_tint, pal.blue_ink);
                    ui.add_space(10.0);
                }
                form_field(ui, "Web address", |ui| {
                    let r = field_full(ui, &mut d.url, "https://…");
                    if !d.focused {
                        r.request_focus();
                        d.focused = true;
                    }
                    if r.changed() && !d.name_edited {
                        d.name = engine::file_name_from_url(d.url.trim());
                        d.category = self.cat_of(&d.name);
                        d.folder = self.folder_for(&d.category).display().to_string();
                    }
                });
                form_field(ui, "Save as", |ui| {
                    if field_full(ui, &mut d.name, "File name").changed() {
                        d.name_edited = true;
                    }
                });
                ui.columns(2, |cols| {
                    form_field(&mut cols[0], "Category", |ui| {
                        let before = d.category.clone();
                        let w = ui.available_width();
                        dropdown(ui, "add_cat", w, &cats, &mut d.category);
                        if before != d.category {
                            d.folder = self.folder_for(&d.category).display().to_string();
                        }
                    });
                    form_field(&mut cols[1], "Add to queue", |ui| {
                        let w = ui.available_width();
                        dropdown(ui, "add_queue", w, &queues, &mut d.queue).on_hover_text("Where \"Download later\" puts it");
                    });
                });
                form_field(ui, "Save to", |ui| {
                    path_field(ui, &mut d.folder);
                });
                form_field(ui, "Description", |ui| {
                    field_full(ui, &mut d.description, "Optional note");
                });
                disclosure(ui, "add_more", "More options (login, referrer, checksum)", |ui| {
                    ui.add_space(4.0);
                    form_field(ui, "Referrer page", |ui| {
                        field_full(ui, &mut d.referer, "Page the link came from (some sites need it)");
                    });
                    ui.columns(2, |cols| {
                        form_field(&mut cols[0], "User name", |ui| {
                            field_full(ui, &mut d.user, "");
                        });
                        form_field(&mut cols[1], "Password", |ui| {
                            let w = ui.available_width();
                            ui.add(field(&mut d.pass).password(true).desired_width(w));
                        });
                    });
                    form_field(ui, "SHA-256 checksum", |ui| {
                        field_full(ui, &mut d.expected_sha, "Optional: checked when the download finishes");
                    });
                });
                if let Some(e) = &d.error {
                    ui.add_space(4.0);
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
                    let start = primary_button(ui, "Start download");
                    if start.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        result = Some(true);
                    }
                    if secondary_button(ui, "Download later").on_hover_text("Add to the queue picked above").clicked() {
                        result = Some(false);
                    }
                },
            );
        });
        if out.close || cancel {
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
            let mut act: Option<&str> = None;
            let mut details = self.details_open.contains(&id);
            let mut lim_on = self.items[idx].limit_kbps > 0;
            let mut lim_val: u64 = if lim_on { self.items[idx].limit_kbps } else { 256 };
            let mut close = {
                let item = &self.items[idx];
                let st = item.status();
                let pal = theme::current(ctx);
                let title = format!("{:.0}% {}", item.progress() * 100.0, item.name);
                let pos = egui::pos2(280.0 + (id % 5) as f32 * 24.0, 120.0 + (id % 5) as f32 * 24.0);
                // The parts of the file as channels that run into it, with how fast each one goes.
                let segs = item.live.segments.lock().unwrap().clone();
                let channels = {
                    let key = egui::Id::unique(("flow_rates", id));
                    let mut rates = ctx.data(|d| d.get_temp::<crate::widgets::flow::Rates>(key)).unwrap_or_default();
                    let parts: Vec<(u64, bool)> = segs.iter().map(|s| (s.done.min(s.len()), s.finished())).collect();
                    let list = if parts.is_empty() {
                        // One connection (or not known yet): a single channel.
                        vec![FlowChannel { rate: 1.0, done: st == Status::Complete }]
                    } else {
                        rates.update(ctx.input(|i| i.time), &parts)
                    };
                    ctx.data_mut(|d| d.insert_temp(key, rates));
                    list
                };
                let flow_state = match st {
                    Status::Connecting | Status::Downloading => FlowState::Running,
                    Status::Complete => FlowState::Complete,
                    Status::Error => FlowState::Failed,
                    Status::Queued | Status::Paused => FlowState::Waiting,
                };
                let out = sheet(("progress", id), title).width(500.0).floating(pos).show(ctx, |s| {
                    s.body(|ui| {
                        crate::widgets::flow(ui, ("progress", id), &channels, item.progress(), flow_state, 112.0);
                        ui.add_space(12.0);
                        kv(ui, "Address", |ui| {
                            ui.add(egui::Label::new(RichText::new(&item.url).color(pal.blue_ink)).truncate());
                        });
                        kv(ui, "Status", |ui| {
                            let text = match st {
                                Status::Error => item.live.error.lock().unwrap().clone().unwrap_or_else(|| "Error".into()),
                                s => item.live.note().unwrap_or_else(|| s.label().to_string()),
                            };
                            let col = if st == Status::Error { pal.red_ink } else { pal.label };
                            ui.add(egui::Label::new(RichText::new(text).family(theme::semibold()).color(col)).truncate());
                        });
                        kv_text(ui, "File size", &if item.total() > 0 { human_size(item.total()) } else { "Unknown".into() });
                        kv_text(
                            ui,
                            "Downloaded",
                            &if item.total() > 0 { format!("{} ({:.2} %)", human_size(item.done()), item.progress() * 100.0) } else { human_size(item.done()) },
                        );
                        kv_text(ui, "Transfer rate", &human_speed(item.speed()));
                        kv_text(ui, "Time left", &item.time_left());
                        kv_text(ui, "Connections", &item.live.connections.load(Ordering::Relaxed).to_string());
                        // With several networks chosen: what each one is carrying.
                        let nets = item.live.router.lock().unwrap().as_ref().map(|r| r.status());
                        for n in nets.into_iter().flatten() {
                            let text = if n.gone { "not connected".to_string() } else if n.resting { "not working, resting".to_string() } else { format!("{} · {} connections", human_speed(n.bytes_per_sec), n.connections) };
                            kv_text(ui, &format!("Network {}", n.name), &text);
                        }
                        kv_text(
                            ui,
                            "Resume capability",
                            if item.live.resumable.load(Ordering::Relaxed) {
                                "Yes"
                            } else if item.total() == 0 && st != Status::Complete {
                                "Unknown"
                            } else {
                                "No"
                            },
                        );
                        ui.add_space(10.0);
                        segment_bar(ui, item.progress(), item.total(), &segs);
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label("Limit this download to");
                            ui.add_space(4.0);
                            switch(ui, &mut lim_on);
                            ui.add_space(4.0);
                            ui.add_enabled_ui(lim_on, |ui| number_field(ui, &mut lim_val, 1..=1_000_000, "KB/s", 130.0));
                        });
                        if details {
                            ui.add_space(6.0);
                            if segs.is_empty() {
                                message(ui, "Single connection (the server does not allow splitting).", false);
                            } else {
                                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                                    egui::Grid::new(("segs", id)).num_columns(3).spacing([28.0, 6.0]).show(ui, |ui| {
                                        for h in ["Part", "Downloaded", "Info"] {
                                            ui.label(RichText::new(h).size(12.0).family(theme::semibold()).color(pal.tertiary));
                                        }
                                        ui.end_row();
                                        for (k, s) in segs.iter().enumerate() {
                                            ui.label((k + 1).to_string());
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
                    s.footer_split(
                        |ui| {
                            if small_button(ui, ButtonKind::Secondary, if details { "Hide details" } else { "Show details" }).clicked() {
                                details = !details;
                            }
                        },
                        |ui| {
                            if st.is_active() {
                                if primary_button(ui, "Pause").clicked() {
                                    act = Some("pause");
                                }
                            } else if st == Status::Complete {
                                if primary_button(ui, "Open").clicked() {
                                    act = Some("open");
                                }
                                if secondary_button(ui, "Open folder").clicked() {
                                    act = Some("folder");
                                }
                            } else if primary_button(ui, "Resume").clicked() {
                                act = Some("resume");
                            }
                            if plain_button(ui, "Cancel").on_hover_text("Stop and close").clicked() {
                                act = Some("cancel");
                            }
                        },
                    );
                });
                out.close
            };
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
                    close = true;
                }
                Some("open") => open_path(&self.items[idx].path()),
                Some("folder") => open_path(&self.items[idx].folder),
                _ => {}
            }
            if close {
                self.progress_open.remove(&id);
            }
        }
    }

    pub(super) fn options_window(&mut self, ctx: &egui::Context) {
        let Some(mut s) = self.options.take() else { return };
        // Both footer closures report here, so it is a Cell rather than two mutable borrows.
        let chosen = std::cell::Cell::new(None::<bool>);
        let link = self.web_link();
        let out = sheet("options", "Options").width(620.0).show(ctx, |sh| {
            sh.body(|ui| options_body(ui, &mut s, link.as_deref()));
            sh.footer_split(
                |ui| {
                    if plain_button(ui, "Cancel").clicked() {
                        chosen.set(Some(false));
                    }
                },
                |ui| {
                    if primary_button(ui, "Save").clicked() {
                        chosen.set(Some(true));
                    }
                },
            );
        });
        let mut result = chosen.get();
        if out.close && result.is_none() {
            result = Some(false);
        }
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
                if s.launch_at_login != self.settings.launch_at_login {
                    let done = if s.launch_at_login { crate::autostart::enable() } else { crate::autostart::disable() };
                    if let Err(e) = done {
                        self.toast(format!("Could not change starting with the system: {e}"));
                        s.launch_at_login = self.settings.launch_at_login;
                    }
                }
                self.settings = s;
                self.fix_settings();
                self.apply_settings();
                self.dirty = true;
            }
            Some(false) => {}
            None => self.options = Some(s),
        }
    }

    pub(super) fn delete_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.delete.take() else { return };
        let chosen = std::cell::Cell::new(None::<bool>);
        let any_done = d.ids.iter().any(|id| self.items.iter().any(|i| i.id == *id && i.status() == Status::Complete));
        let out = sheet("delete", "Remove from the list?").width(440.0).show(ctx, |s| {
            s.body(|ui| {
                ui.label(if d.ids.len() == 1 { "This download will be removed from the list.".to_string() } else { format!("These {} downloads will be removed from the list.", d.ids.len()) });
                if any_done {
                    ui.add_space(8.0);
                    check_row(ui, &mut d.delete_files, "Also delete the downloaded files from disk");
                }
            });
            s.footer_split(
                |ui| {
                    if plain_button(ui, "Cancel").clicked() {
                        chosen.set(Some(false));
                    }
                },
                |ui| {
                    if button(ui, ButtonKind::Danger, "Remove").clicked() {
                        chosen.set(Some(true));
                    }
                },
            );
        });
        let mut result = chosen.get();
        if out.close && result.is_none() {
            result = Some(false);
        }
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
        let pal = theme::current(ctx);
        let mut close = false;
        let mut verify = false;
        let out = sheet("properties", "File properties").width(580.0).show(ctx, |s| {
            s.body(|ui| {
                kv_text(ui, "File name", &item.name);
                kv_text(ui, "Category", &item.category);
                kv_text(ui, "Saved to", &item.folder.display().to_string());
                kv_text(ui, "Address", &item.url);
                if !item.referer.is_empty() {
                    kv_text(ui, "Referrer", &item.referer);
                }
                kv_text(ui, "Size", &human_size(item.total()));
                kv_text(ui, "Status", item.status().label());
                kv_text(ui, "Added", &item.added);
                ui.add_space(8.0);
                form_field(ui, "Description", |ui| {
                    field_full(ui, &mut item.description, "Optional note");
                });
                form_field(ui, "Expected SHA-256", |ui| {
                    field_full(ui, &mut item.expected_sha, "Paste the checksum from the website");
                });
                let done = item.status() == Status::Complete;
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(done && !item.is_checking, |ui| {
                        if button(ui, ButtonKind::Outline, "Check SHA-256").clicked() {
                            verify = true;
                        }
                    });
                    if item.is_checking {
                        crate::widgets::arc_loader(ui, 18.0);
                        ui.label(RichText::new("Checking…").color(pal.secondary));
                    }
                    if let Some(c) = &item.check {
                        let col = if c.contains("NOT") {
                            pal.red_ink
                        } else if c.contains("matches") {
                            pal.green_ink
                        } else {
                            pal.secondary
                        };
                        let r = ui.add(egui::Label::new(RichText::new(c).color(col)).truncate().sense(egui::Sense::click()));
                        if r.on_hover_text("Click to copy").clicked() {
                            ui.ctx().copy_text(c.trim_start_matches("SHA-256: ").to_string());
                        }
                    }
                });
            });
            s.footer_split(
                |ui| {
                    if plain_button(ui, "Close").clicked() {
                        close = true;
                    }
                },
                |ui| {
                    ui.add_enabled_ui(item.status() == Status::Complete, |ui| {
                        if primary_button(ui, "Open").clicked() {
                            open_path(&item.path());
                        }
                    });
                    if secondary_button(ui, "Open folder").clicked() {
                        open_path(&item.folder);
                    }
                },
            );
        });
        item.expected_sha = item.expected_sha.trim().to_ascii_lowercase();
        if verify {
            self.verify(id);
        }
        if out.close || close {
            self.properties = None;
            self.dirty = true;
        }
    }
}

/// The network cards that are connected, looked up at most every 3 seconds.
fn connected_networks(ui: &egui::Ui) -> Vec<crate::netroute::Iface> {
    // The sample window (--screenshot) shows made-up networks instead of this computer's.
    if ui.data(|d| d.get_temp::<bool>(egui::Id::unique("sdm_demo_networks"))).unwrap_or(false) {
        use crate::netroute::{Iface, Kind};
        let sample = |name: &str, last: u8, kind| Iface { name: name.into(), addr: std::net::IpAddr::from([192, 168, 1, last]), index: None, kind };
        return vec![sample("Wi-Fi", 20, Kind::Wifi), sample("Ethernet", 21, Kind::Ethernet), sample("Phone hotspot", 22, Kind::Phone)];
    }
    let id = egui::Id::unique("sdm_connected_networks");
    let now = ui.input(|i| i.time);
    if let Some((at, list)) = ui.data(|d| d.get_temp::<(f64, Vec<crate::netroute::Iface>)>(id))
        && now - at < 3.0
    {
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(3));
        return list;
    }
    let list = crate::netroute::interfaces();
    ui.data_mut(|d| d.insert_temp(id, (now, list.clone())));
    ui.ctx().request_repaint_after(std::time::Duration::from_secs(3));
    list
}

/// Options > Networks: spread each download over several network cards at once.
fn networks_card(ui: &mut egui::Ui, s: &mut Settings) {
    // Screenshots (--show networks) start with this card in view.
    if ui.data(|d| d.get_temp::<bool>(egui::Id::unique("sdm_scroll_networks"))).unwrap_or(false) {
        ui.scroll_to_cursor(Some(egui::Align::TOP));
    }
    section_title(ui, "Networks");
    let nets = connected_networks(ui);
    let switch_w = crate::widgets::switch::switch_size(ui.ctx()).x;
    card(ui, |g| {
        g.toggle("Download over several networks at once", Some("Combine Wi-Fi, Ethernet, a phone's hotspot and more into one faster download."), &mut s.multi_network);
        if !s.multi_network {
            return;
        }
        g.toggle("Use every connected network automatically", Some("Also a network you plug in later, such as a phone with USB tethering. Stands back while a VPN is connected."), &mut s.networks_auto);
        if nets.is_empty() {
            g.custom(48.0, |ui| {
                ui.label("No connected network found.");
            });
        }
        for n in &nets {
            // By hand the switch means "use it"; in automatic mode it means "do not leave it out".
            let mut on = if s.networks_auto { !s.networks_off.contains(&n.name) } else { s.networks.contains(&n.name) };
            let sub = format!("{} · {}", n.kind.label(), n.addr);
            if g.row(&n.name, Some(&sub), switch_w, |ui| switch(ui, &mut on)).changed() {
                let list = if s.networks_auto { &mut s.networks_off } else { &mut s.networks };
                list.retain(|x| x != &n.name);
                if on != s.networks_auto {
                    list.push(n.name.clone());
                }
            }
        }
    });
    if s.multi_network {
        let text = if s.networks_auto {
            "Every download uses all the connected networks at once and their speeds add up. Switch a network off to leave it out (for a phone on a metered plan). A network that is plugged in or drops out while a download runs is picked up or skipped within a few seconds."
        } else {
            "Pick two or more. Each download then uses every picked network at once and their speeds add up. A picked network that is plugged in or drops out while a download runs is picked up or skipped within a few seconds."
        };
        hint(ui, text);
    }
}

/// Options > Web interface: use the downloads from a browser on this computer.
fn web_card(ui: &mut egui::Ui, s: &mut Settings, link: Option<&str>) {
    section_title(ui, "Web interface");
    // The room the two buttons of the Link row need (measured before the card borrows the ui).
    let buttons_w = button_width(ui, ButtonSize::Regular, false, "Open") + button_width(ui, ButtonSize::Regular, false, "Copy") + ui.spacing().item_spacing.x;
    card(ui, |g| {
        g.toggle("Use Swift Download Manager from a web browser", Some("Only this computer can reach it."), &mut s.web_enabled);
        if !s.web_enabled {
            return;
        }
        g.row("Port", None, 130.0, |ui| number_field(ui, &mut s.web_port, 1024..=65535, "", 130.0));
        match link {
            Some(link) => {
                g.row("Link", Some("It holds a secret key: treat it like a password."), buttons_w, |ui| {
                    if primary_button(ui, "Open").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(link));
                    }
                    if secondary_button(ui, "Copy").clicked() {
                        ui.ctx().copy_text(link.to_string());
                    }
                });
            }
            None => {
                g.custom(52.0, |ui| {
                    ui.label(RichText::new("Press Save to start it; the link appears here.").color(theme::current(ui.ctx()).secondary));
                });
            }
        }
    });
    if s.web_enabled {
        hint(ui, "It needs this program (or its tray icon) to be running.");
    }
}

/// Options for the WinUI 3 theme: the material behind the window, the accent and the transparency switch of Windows.
fn winui_card(ui: &mut egui::Ui, s: &mut Settings) {
    use crate::theme::winui::{runtime, Accent, Backdrop};
    /// The accent colours Windows itself offers (a selection of them).
    const PRESETS: [u32; 16] = [0xFFB900, 0xF7630C, 0xDA3B01, 0xE81123, 0xEA005E, 0xC239B3, 0x8764B8, 0x0078D4, 0x0063B1, 0x00B7C3, 0x038387, 0x10893E, 0x107C10, 0x7A7574, 0x68768A, 0x767676];
    section_title(ui, "WinUI 3");
    let material_w = segmented_width(ui, &["Mica", "Mica Alt", "Acrylic", "None"]);
    let now = runtime();
    card(ui, |g| {
        g.row(
            "Material behind the window",
            Some("What the window is made of: the wallpaper, blurred and tinted (Mica), a stronger tint (Mica Alt), or a thinner, more colourful one (Acrylic)."),
            material_w,
            |ui| segmented(ui, &[(Backdrop::Mica, "Mica"), (Backdrop::MicaAlt, "Mica Alt"), (Backdrop::Acrylic, "Acrylic"), (Backdrop::None, "None")], &mut s.winui_backdrop),
        );
        let mut system = s.winui_accent.is_none();
        g.toggle("Use the accent colour of Windows", Some("Follows Settings > Personalization > Colors, also while the app is open."), &mut system);
        if system != s.winui_accent.is_none() {
            s.winui_accent = if system { None } else { Some(u32::from_be_bytes([0, now.accent.base.r(), now.accent.base.g(), now.accent.base.b()])) };
        }
        if let Some(chosen) = s.winui_accent {
            g.free(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    for preset in PRESETS {
                        let colour = egui::Color32::from_rgb((preset >> 16) as u8, (preset >> 8) as u8, preset as u8);
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
                        let r = egui::CornerRadius::same(theme::design(ui.ctx()).radius_sm);
                        ui.painter().rect_filled(rect, r, colour);
                        if chosen == preset {
                            ui.painter().rect_stroke(rect.expand(2.0), r, egui::Stroke::new(2.0, theme::current(ui.ctx()).label), egui::StrokeKind::Outside);
                        }
                        if resp.on_hover_text(format!("#{preset:06X}")).clicked() {
                            s.winui_accent = Some(preset);
                        }
                    }
                });
                ui.add_space(8.0);
                // Any other colour, as #RRGGBB.
                let id = egui::Id::unique("winui_accent_hex");
                let mut text = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| format!("#{chosen:06X}"));
                let shown = format!("#{chosen:06X}");
                if !ui.memory(|m| m.has_focus(id.with("edit"))) && !text.eq_ignore_ascii_case(&shown) && Accent::parse_hex(&text).is_none_or(|v| v != chosen) {
                    text = shown;
                }
                ui.horizontal(|ui| {
                    ui.label("Other colour");
                    let edited = ui.add(field(&mut text).id(id.with("edit")).desired_width(110.0).hint_text("#0078D4")).changed();
                    if edited && let Some(v) = Accent::parse_hex(&text) {
                        s.winui_accent = Some(v);
                    }
                });
                ui.data_mut(|d| d.insert_temp(id, text));
            });
        }
        g.toggle("Follow the transparency effects setting of Windows", Some("When Windows turns transparency effects off, the window is solid."), &mut s.winui_follow_effects);
    });
    let in_use = match now.material {
        Backdrop::None if s.winui_backdrop != Backdrop::None => "No material is showing now: Windows has transparency effects off, a high contrast theme is on, or this is not Windows. The colours are solid.".to_string(),
        Backdrop::None => "The window is solid.".to_string(),
        m => format!("{} is showing now.", m.name()),
    };
    hint(ui, &in_use);
}

fn options_body(ui: &mut egui::Ui, s: &mut Settings, web_link: Option<&str>) {
    let pal = theme::current(ui.ctx());
    section_title(ui, "Downloads");
    form_field(ui, "Download folder", |ui| {
        let mut f = s.folder.display().to_string();
        if path_field(ui, &mut f) {
            s.folder = PathBuf::from(f);
        }
    });
    card(ui, |g| {
        g.toggle("Use a sub-folder for each category", None, &mut s.per_category_folders);
        g.row("Connections per download", Some("Each download is split into this many parts."), 130.0, |ui| stepper(ui, &mut s.connections, 1..=32));
        g.row("Retries on errors", None, 130.0, |ui| stepper(ui, &mut s.retries, 0..=20));
        g.row("Speed limit", None, 51.0 + 8.0 + 150.0, |ui| {
            ui.add_enabled_ui(s.limit_on, |ui| number_field(ui, &mut s.limit_kbps, 1..=1_000_000, "KB/s", 150.0));
            switch(ui, &mut s.limit_on)
        });
        g.row("Proxy", None, 51.0 + 8.0 + 210.0, |ui| {
            let w = 210.0;
            ui.add_enabled(s.proxy_on, field(&mut s.proxy).hint_text("http://host:port").desired_width(w));
            switch(ui, &mut s.proxy_on)
        });
    });
    section_title(ui, "Windows");
    card(ui, |g| {
        g.toggle("Show the progress window when a download starts", None, &mut s.show_progress);
        g.toggle("Show a window when a download completes", None, &mut s.show_complete);
    });
    section_title(ui, "Catching links");
    card(ui, |g| {
        g.toggle("Watch the clipboard", Some("Offers to download file links that you copy."), &mut s.watch_clipboard);
        g.toggle("Accept downloads from the browser add-on", Some("Restart the app to apply."), &mut s.browser_listen);
        g.enable(s.browser_listen);
        g.toggle("Start browser downloads straight away", Some("Without asking first."), &mut s.browser_start_now);
    });
    // Screenshots (--show appearance) start with this card in view.
    if ui.data(|d| d.get_temp::<bool>(egui::Id::unique("sdm_scroll_appearance"))).unwrap_or(false) {
        ui.scroll_to_cursor(Some(egui::Align::TOP));
    }
    section_title(ui, "Appearance");
    let mode_w = segmented_width(ui, &["Automatic", "Light", "Dark"]);
    card(ui, |g| {
        g.row("Mode", None, mode_w, |ui| segmented(ui, &[(ThemeMode::System, "Automatic"), (ThemeMode::Light, "Light"), (ThemeMode::Dark, "Dark")], &mut s.theme));
        g.free(|ui| {
            ui.label(egui::RichText::new("Theme").size(theme::design(ui.ctx()).body).color(theme::current(ui.ctx()).label));
            ui.add_space(10.0);
            crate::widgets::theme_picker(ui, &mut s.color_theme);
        });
    });
    if s.color_theme == crate::theme::ColorTheme::WinUi {
        // Screenshots (--show winui) start with this card in view.
        if ui.data(|d| d.get_temp::<bool>(egui::Id::unique("sdm_scroll_winui"))).unwrap_or(false) {
            ui.scroll_to_cursor(Some(egui::Align::TOP));
        }
        winui_card(ui, s);
    }
    networks_card(ui, s);
    web_card(ui, s, web_link);
    section_title(ui, "Background");
    let close_w = segmented_width(ui, &["Ask", "Keep running", "Quit"]);
    card(ui, |g| {
        g.row("When the window is closed", None, close_w, |ui| segmented(ui, &[(BackgroundMode::Ask, "Ask"), (BackgroundMode::Keep, "Keep running"), (BackgroundMode::Quit, "Quit")], &mut s.background));
        g.toggle("Start with the system, in the background", None, &mut s.launch_at_login);
    });
    hint(ui, "Running in the background keeps downloads, queues and the browser add-on going, with an icon in the tray.");
    section_title(ui, "AI assistants");
    card(ui, |g| {
        g.toggle("Let AI assistants add, pause and remove downloads (MCP)", Some("They cannot delete files or choose folders. What they may do, limits and setup: Help > AI assistants…"), &mut s.mcp_enabled);
    });
    ui.add_space(10.0);
    disclosure(ui, "opt_categories", "Categories (file types and folders)", |ui| {
        ui.add_space(4.0);
        let mut remove = None;
        for (k, c) in s.categories.iter_mut().enumerate() {
            let general = c.name == "General";
            card(ui, |g| {
                g.free(|ui| {
                    ui.horizontal(|ui| {
                        let total = ui.available_width();
                        ui.add_enabled(!general, field(&mut c.name).desired_width(150.0).hint_text("Name"));
                        ui.add(field(&mut c.exts).desired_width((total - 150.0 - 36.0 - 16.0).max(60.0)).hint_text(if general { "everything else" } else { "zip rar 7z" }));
                        ui.add_enabled_ui(!general, |ui| {
                            if icon_button(ui, Icon::Close, "Remove this category").clicked() {
                                remove = Some(k);
                            }
                        });
                    });
                    ui.add_space(2.0);
                    path_field(ui, &mut c.folder);
                });
            });
            ui.add_space(8.0);
        }
        if let Some(k) = remove {
            s.categories.remove(k);
        }
        if crate::widgets::small_icon_label_button(ui, ButtonKind::Secondary, Icon::Plus, "Add category").clicked() {
            let pos = s.categories.iter().position(|c| c.name == "General").unwrap_or(s.categories.len());
            s.categories.insert(pos, CategoryDef { name: "New category".into(), exts: String::new(), folder: String::new() });
        }
    });
    disclosure(ui, "opt_logins", "Site logins", |ui| {
        ui.add_space(4.0);
        ui.label(RichText::new("Used automatically for downloads from these sites.").color(pal.secondary));
        ui.add_space(6.0);
        let mut remove = None;
        for (k, l) in s.site_logins.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let total = ui.available_width();
                let each = ((total - 36.0 - 3.0 * 8.0) / 3.0).max(60.0);
                ui.add(field(&mut l.site).hint_text("example.com").desired_width(each));
                ui.add(field(&mut l.user).hint_text("User name").desired_width(each));
                ui.add(field(&mut l.pass).hint_text("Password").password(true).desired_width(each));
                if icon_button(ui, Icon::Close, "Remove this login").clicked() {
                    remove = Some(k);
                }
            });
        }
        if let Some(k) = remove {
            s.site_logins.remove(k);
        }
        if crate::widgets::small_icon_label_button(ui, ButtonKind::Secondary, Icon::Plus, "Add login").clicked() {
            s.site_logins.push(SiteLogin::default());
        }
    });
    ui.add_space(8.0);
}
