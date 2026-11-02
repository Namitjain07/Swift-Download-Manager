//! What the buttons do, plus per-frame bookkeeping (speeds, queues, scheduler, saving).

use super::*;
use crate::bridge::Source;
use crate::engine::{self, Job};

impl App {
    fn unique_name(&self, folder: &PathBuf, name: &str) -> String {
        let mut final_name = name.to_string();
        let mut n = 1;
        while folder.join(&final_name).exists() || self.items.iter().any(|i| &i.folder == folder && i.name == final_name) {
            final_name = engine::numbered(name, n);
            n += 1;
        }
        final_name
    }

    pub(super) fn check_url(url: &str) -> Result<(), String> {
        let parsed = url::Url::parse(url).map_err(|_| format!("\"{url}\" doesn't look like a web address."))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err("Only http:// and https:// links are supported.".into());
        }
        Ok(())
    }

    pub(super) fn add_download(&mut self, d: &AddDialog, start_now: bool) -> Result<u64, String> {
        let url = d.url.trim().to_string();
        Self::check_url(&url)?;
        let sha = d.expected_sha.trim().to_ascii_lowercase();
        if !sha.is_empty() && (sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit())) {
            return Err("A SHA-256 checksum has 64 letters and digits (0-9, a-f).".into());
        }
        let name = engine::sanitize(if d.name.trim().is_empty() { "download" } else { d.name.trim() });
        let folder = PathBuf::from(d.folder.trim());
        let final_name = self.unique_name(&folder, &name);
        let id = self.next_id;
        self.next_id += 1;
        let status = if start_now { Status::Paused } else { Status::Queued };
        let mut item = Item::new(id, url, final_name, folder, d.category.clone(), Live::new(0, 0, status, Vec::new(), false), &self.limiter);
        item.description = d.description.clone();
        item.auto_name = !d.name_edited;
        item.referer = d.referer.trim().to_string();
        item.user = d.user.clone();
        item.pass = d.pass.clone();
        item.expected_sha = sha;
        item.cookies = d.cookies.clone();
        item.user_agent = d.user_agent.clone();
        if !start_now {
            let q = if self.settings.queues.iter().any(|q| q.name == d.queue) { d.queue.clone() } else { MAIN_QUEUE.to_string() };
            item.queue = Some(q);
        }
        self.items.push(item);
        if start_now {
            self.start_item(id);
            if self.settings.show_progress {
                self.progress_open.insert(id);
            }
        }
        self.selected.clear();
        self.selected.insert(id);
        self.dirty = true;
        Ok(id)
    }

    /// Add several links at once (batch / import / grabber). Returns how many were added.
    pub(super) fn add_many(&mut self, urls: &[String], folder: Option<PathBuf>, start: bool, queue: &str, referer: &str) -> Result<usize, String> {
        for u in urls {
            Self::check_url(u)?;
        }
        for u in urls {
            let name = engine::file_name_from_url(u);
            let category = self.cat_of(&name);
            let d = AddDialog {
                url: u.clone(),
                folder: folder.clone().unwrap_or_else(|| self.folder_for(&category)).display().to_string(),
                name,
                category,
                queue: queue.to_string(),
                referer: referer.to_string(),
                ..Default::default()
            };
            self.add_download(&d, false)?;
        }
        if start {
            self.running.insert(queue.to_string());
        }
        Ok(urls.len())
    }

    /// The networks to download over: the cards chosen in Options > Networks that are connected right now.
    fn chosen_routes(&self) -> Vec<crate::netroute::Route> {
        if !self.settings.multi_network || self.demo {
            return Vec::new();
        }
        crate::netroute::resolve(&self.settings.networks, &crate::netroute::interfaces())
    }

    pub(super) fn start_item(&mut self, id: u64) {
        let logins = self.settings.site_logins.clone();
        let routes = self.chosen_routes();
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else { return };
        if item.status().is_active() || item.status() == Status::Complete {
            return;
        }
        item.last_try = now_stamp();
        item.check = None;
        if self.demo {
            *item.live.status.lock().unwrap() = Status::Downloading;
            item.demo_speed = Some(1_500_000.0);
            return;
        }
        let fresh = item.live.segments.lock().unwrap().is_empty() && item.done() == 0;
        // A saved site login is used when the download has none of its own.
        let (user, pass) = match logins.iter().find(|l| item.user.is_empty() && l.matches(&item.url)) {
            Some(l) => (l.user.clone(), l.pass.clone()),
            None => (item.user.clone(), item.pass.clone()),
        };
        let job = Job {
            url: item.url.clone(),
            target: item.path(),
            connections: self.settings.connections,
            accept_server_name: item.auto_name && fresh,
            referer: item.referer.clone(),
            user,
            pass,
            proxy: self.settings.proxy_on.then(|| self.settings.proxy.clone()),
            retries: self.settings.retries,
            cookies: item.cookies.clone(),
            user_agent: item.user_agent.clone(),
            routes,
        };
        engine::start(job, item.live.clone(), item.limiter.clone(), self.ctx.clone());
        self.dirty = true;
    }

    pub(super) fn stop_item(&mut self, id: u64) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            let st = item.status();
            if st.is_active() {
                item.live.stop.store(true, Ordering::SeqCst);
                if self.demo {
                    *item.live.status.lock().unwrap() = Status::Paused;
                    item.demo_speed = None;
                }
            } else if st == Status::Queued {
                *item.live.status.lock().unwrap() = Status::Paused;
            }
            self.dirty = true;
        }
    }

    pub(super) fn redownload(&mut self, id: u64) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            if item.status().is_active() {
                return;
            }
            if !self.demo {
                let _ = std::fs::remove_file(engine::temp_path(&item.path()));
            }
            item.live.segments.lock().unwrap().clear();
            item.live.downloaded.store(0, Ordering::SeqCst);
            item.live.total.store(0, Ordering::SeqCst);
            *item.live.status.lock().unwrap() = Status::Paused;
        }
        self.start_item(id);
    }

    /// Give a download a fresh link (when the old one expired) and carry on where it stopped.
    pub(super) fn refresh_address(&mut self, id: u64, url: &str) -> Result<(), String> {
        let url = url.trim();
        Self::check_url(url)?;
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else { return Ok(()) };
        let was_running = item.status().is_active() || item.status() == Status::Error;
        if item.status().is_active() {
            item.live.stop.store(true, Ordering::SeqCst);
            // Wait briefly for the connections to let go of the file.
            for _ in 0..50 {
                if !item.live.status().is_active() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        item.url = url.to_string();
        if item.status() == Status::Error {
            *item.live.status.lock().unwrap() = Status::Paused;
        }
        self.dirty = true;
        if was_running {
            self.start_item(id);
        }
        Ok(())
    }

    pub(super) fn remove_items(&mut self, ids: &[u64], delete_files: bool) {
        self.drop_items(ids, true, delete_files);
    }

    /// Take downloads off the list and stop them, leaving every file on disk (even partial ones).
    pub(super) fn forget_items(&mut self, ids: &[u64]) {
        self.drop_items(ids, false, false);
    }

    fn drop_items(&mut self, ids: &[u64], delete_partial: bool, delete_files: bool) {
        for id in ids {
            if let Some(item) = self.items.iter().find(|i| i.id == *id) {
                item.live.stop.store(true, Ordering::SeqCst);
                if !self.demo {
                    if delete_partial {
                        let _ = std::fs::remove_file(engine::temp_path(&item.path()));
                    }
                    if delete_files && item.status() == Status::Complete {
                        let _ = std::fs::remove_file(item.path());
                    }
                }
            }
            self.progress_open.remove(id);
            self.selected.remove(id);
            self.completed.retain(|c| c != id);
        }
        self.items.retain(|i| !ids.contains(&i.id));
        self.dirty = true;
    }

    pub(super) fn stop_all(&mut self) {
        self.running.clear();
        let ids: Vec<u64> = self.items.iter().filter(|i| i.status().is_active()).map(|i| i.id).collect();
        for id in ids {
            self.stop_item(id);
        }
    }

    /// The queue the toolbar's queue buttons act on: the one picked in the sidebar, else the main queue.
    pub(super) fn current_queue(&self) -> String {
        match &self.filter {
            Filter::Queue(q) => q.clone(),
            _ => MAIN_QUEUE.to_string(),
        }
    }

    pub(super) fn start_queue(&mut self, q: &str) {
        self.running.insert(q.to_string());
    }

    /// Stop a queue: its running downloads go back to waiting in it.
    pub(super) fn stop_queue(&mut self, q: &str) {
        self.running.remove(q);
        let ids: Vec<u64> = self.items.iter().filter(|i| i.queue.as_deref() == Some(q) && i.status().is_active()).map(|i| i.id).collect();
        for id in ids {
            self.stop_item(id);
        }
        for item in &self.items {
            if item.queue.as_deref() == Some(q) && matches!(item.status(), Status::Paused | Status::Connecting | Status::Downloading) {
                *item.live.status.lock().unwrap() = Status::Queued;
            }
        }
        self.dirty = true;
    }

    pub(super) fn move_to_queue(&mut self, id: u64, q: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.queue = Some(q.to_string());
            if matches!(item.status(), Status::Paused | Status::Error | Status::Queued) {
                *item.live.status.lock().unwrap() = Status::Queued;
            }
            self.dirty = true;
        }
    }

    pub(super) fn delete_completed(&mut self) {
        let ids: Vec<u64> = self.items.iter().filter(|i| i.status() == Status::Complete).map(|i| i.id).collect();
        self.remove_items(&ids, false);
    }

    pub(super) fn selected_ids(&self) -> Vec<u64> {
        self.items.iter().filter(|i| self.selected.contains(&i.id)).map(|i| i.id).collect()
    }

    pub(super) fn open_add_dialog(&mut self, incoming: Option<Incoming>) {
        let mut d = AddDialog { folder: self.settings.folder.display().to_string(), category: "General".into(), queue: self.current_queue(), ..Default::default() };
        if let Some(inc) = incoming {
            d.from_outside = Some(match inc.source {
                Source::Browser if !inc.cookies.is_empty() => "Sent from your web browser (uses your browser login for this site)".into(),
                Source::Browser => "Sent from your web browser".into(),
                Source::Clipboard => "Found a download link on the clipboard".into(),
                Source::CommandLine => "Opened with a link".into(),
            });
            d.url = inc.url;
            d.referer = inc.referer;
            d.cookies = inc.cookies;
            d.user_agent = inc.user_agent;
            d.name = if inc.name.trim().is_empty() { engine::file_name_from_url(&d.url) } else { engine::sanitize(&inc.name) };
            d.category = self.cat_of(&d.name);
            d.folder = self.folder_for(&d.category).display().to_string();
            d.focused = true;
        }
        self.add = Some(d);
    }

    pub(super) fn ask_delete(&mut self, ids: Vec<u64>) {
        if !ids.is_empty() {
            self.delete = Some(DeleteDialog { ids, delete_files: false });
        }
    }

    pub(super) fn matches(&self, item: &Item) -> bool {
        let by_filter = match &self.filter {
            Filter::All => true,
            Filter::Category(c) => &item.category == c,
            Filter::Unfinished => item.status() != Status::Complete,
            Filter::Finished => item.status() == Status::Complete,
            Filter::Queue(q) => item.queue.as_deref() == Some(q.as_str()) && item.status() != Status::Complete,
        };
        let q = self.search.trim().to_lowercase();
        by_filter && (q.is_empty() || item.name.to_lowercase().contains(&q) || item.url.to_lowercase().contains(&q) || item.description.to_lowercase().contains(&q))
    }

    /// Indices of visible rows, in display order.
    pub(super) fn visible_rows(&self) -> Vec<usize> {
        let mut rows: Vec<usize> = (0..self.items.len()).filter(|&i| self.matches(&self.items[i])).collect();
        if let Some((col, asc)) = self.sort {
            let it = &self.items;
            rows.sort_by(|&a, &b| {
                let (a, b) = (&it[a], &it[b]);
                let o = match col {
                    SortCol::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    SortCol::Size => a.total().cmp(&b.total()),
                    SortCol::Status => a.status().label().cmp(b.status().label()).then(a.progress().total_cmp(&b.progress())),
                    SortCol::TimeLeft => a.secs_left().total_cmp(&b.secs_left()),
                    SortCol::Speed => a.speed().total_cmp(&b.speed()),
                    SortCol::LastTry => a.last_try.cmp(&b.last_try),
                    SortCol::Description => a.description.cmp(&b.description),
                };
                if asc { o } else { o.reverse() }
            });
        }
        rows
    }

    /// Move a download earlier (-1) or later (+1) in the queue order.
    pub(super) fn move_item(&mut self, id: u64, dir: i32) {
        let Some(pos) = self.items.iter().position(|i| i.id == id) else { return };
        let target = pos as i32 + dir;
        if target >= 0 && (target as usize) < self.items.len() {
            self.items.swap(pos, target as usize);
            self.sort = None;
            self.dirty = true;
        }
    }

    pub(super) fn verify(&mut self, id: u64) {
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else { return };
        if item.is_checking || item.status() != Status::Complete {
            return;
        }
        item.is_checking = true;
        item.check = Some("Checking…".into());
        let (path, slot, ctx) = (item.path(), item.checking.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let r = engine::sha256_file(&path).map_err(|e| format!("Could not read the file: {e}"));
            *slot.lock().unwrap() = Some(r);
            ctx.request_repaint();
        });
    }

    pub(super) fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    pub(super) fn export_links(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_file_name("download-links.txt").save_file() else { return };
        let text: String = self.items.iter().map(|i| format!("{}\n", i.url)).collect();
        match std::fs::write(&path, text) {
            Ok(()) => self.toast(format!("Saved {} links", self.items.len())),
            Err(e) => self.toast(format!("Could not save: {e}")),
        }
    }

    /// Open the batch window with these links (import, dropped files).
    pub(super) fn batch_with(&mut self, links: Vec<String>) {
        self.batch = Some(BatchDialog {
            links: links.join("\n"),
            folder: self.settings.folder.display().to_string(),
            from: 1,
            to: 10,
            width: 1,
            queue: self.current_queue(),
            ..Default::default()
        });
    }

    pub(super) fn import_links(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Link lists", &["txt", "lst", "csv", "url", "html", "htm"]).pick_file() else { return };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let links = links_in_text(&text);
                if links.is_empty() {
                    self.toast("No web links found in that file");
                } else {
                    self.batch_with(links);
                }
            }
            Err(e) => self.toast(format!("Could not open: {e}")),
        }
    }

    /// Files dropped on the window: link lists, shortcut files or web pages.
    pub(super) fn files_dropped(&mut self, paths: Vec<PathBuf>) {
        let mut links = Vec::new();
        for p in paths {
            if let Ok(text) = std::fs::read_to_string(&p) {
                for l in links_in_text(&text) {
                    if !links.contains(&l) {
                        links.push(l);
                    }
                }
            }
        }
        match links.len() {
            0 => self.toast("No web links found in what you dropped"),
            1 => {
                let url = links.remove(0);
                self.open_add_dialog(Some(Incoming { url, name: String::new(), referer: String::new(), cookies: String::new(), user_agent: String::new(), source: Source::CommandLine }));
                if let Some(d) = &mut self.add {
                    d.from_outside = Some("From the file you dropped".into());
                }
            }
            _ => self.batch_with(links),
        }
    }

    /// Per-frame bookkeeping.
    pub(super) fn tick(&mut self) {
        self.serve_control();
        // Speeds.
        let dt = self.last_sample.elapsed().as_secs_f64();
        if dt >= 0.5 {
            self.last_sample = Instant::now();
            for item in &mut self.items {
                let now = item.done();
                let inst = now.saturating_sub(item.last_bytes) as f64 / dt;
                item.speed = if item.speed == 0.0 { inst } else { item.speed * 0.6 + inst * 0.4 };
                if item.status() != Status::Downloading {
                    item.speed = 0.0;
                }
                item.last_bytes = now;
            }
        }
        // Status changes, server-suggested names and checksum results.
        let mut finished = Vec::new();
        let cats = self.settings.categories.clone();
        for item in &mut self.items {
            let st = item.status();
            if st != item.last_status {
                if st == Status::Complete && item.last_status.is_active() {
                    finished.push(item.id);
                }
                item.last_status = st;
                self.dirty = true;
            }
            if let Some(n) = item.live.file_name.lock().unwrap().take() {
                item.name = n;
                item.category = category_for(&cats, &item.name);
                self.dirty = true;
            }
            if let Some(r) = item.checking.lock().unwrap().take() {
                item.is_checking = false;
                item.check = Some(match r {
                    Ok(h) if item.expected_sha.is_empty() => format!("SHA-256: {h}"),
                    Ok(h) if h == item.expected_sha => "SHA-256 matches".to_string(),
                    Ok(h) => format!("SHA-256 does NOT match (got {}…)", &h[..16]),
                    Err(e) => e,
                });
                self.dirty = true;
            }
        }
        for id in finished {
            let name = self.items.iter().find(|i| i.id == id).map(|i| i.name.clone()).unwrap_or_default();
            if self.items.iter().any(|i| i.id == id && !i.expected_sha.is_empty()) {
                self.verify(id);
            }
            if self.settings.show_complete {
                self.progress_open.remove(&id);
                self.completed.push(id);
            } else {
                self.toast(format!("Finished: {name}"));
            }
        }
        self.take_incoming();
        self.run_schedule();
        self.run_queues();
        // Saving.
        let any_active = self.items.iter().any(|i| i.status().is_active());
        if any_active && self.last_save.elapsed().as_secs() >= 3 {
            self.dirty = true;
        }
        if self.dirty && self.last_save.elapsed().as_millis() > 400 {
            self.save();
        }
        if any_active || !self.running.is_empty() || self.toast.is_some() || self.items.iter().any(|i| i.is_checking) || self.last_ai_call.is_some_and(|t| t.elapsed().as_secs() < 6) {
            self.ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        // The scheduler needs a look at the clock now and then even when idle.
        self.ctx.request_repaint_after(std::time::Duration::from_secs(15));
        if self.toast.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs() >= 4) {
            self.toast = None;
        }
    }

    /// Links from the browser, the clipboard or a second copy of the app.
    fn take_incoming(&mut self) {
        if self.headless {
            // No window to ask in: links from the browser add-on start straight away.
            let all: Vec<Incoming> = std::mem::take(&mut *self.inbox.lock().unwrap());
            for inc in all.into_iter().filter(|i| i.source != Source::Clipboard) {
                self.add_incoming_now(inc);
            }
            return;
        }
        if self.add.is_some() {
            return;
        }
        let next = {
            let mut inbox = self.inbox.lock().unwrap();
            if inbox.is_empty() { None } else { Some(inbox.remove(0)) }
        };
        let Some(inc) = next else { return };
        if inc.source == Source::Browser && self.settings.browser_start_now {
            self.add_incoming_now(inc);
        } else {
            self.open_add_dialog(Some(inc));
            self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    /// Add a link that arrived from outside and start it at once.
    fn add_incoming_now(&mut self, inc: Incoming) {
        let mut d = AddDialog::default();
        d.name = if inc.name.is_empty() { engine::file_name_from_url(&inc.url) } else { engine::sanitize(&inc.name) };
        d.category = self.cat_of(&d.name);
        d.folder = self.folder_for(&d.category).display().to_string();
        d.url = inc.url;
        d.referer = inc.referer;
        d.cookies = inc.cookies;
        d.user_agent = inc.user_agent;
        match self.add_download(&d, true) {
            Ok(id) => {
                if self.headless {
                    self.progress_open.remove(&id);
                }
            }
            Err(e) => self.toast(e),
        }
    }

    fn run_queues(&mut self) {
        let running: Vec<String> = self.running.iter().cloned().collect();
        for q in running {
            let Some(def) = self.settings.queues.iter().find(|d| d.name == q).cloned() else {
                self.running.remove(&q);
                continue;
            };
            let in_q = |i: &Item| i.queue.as_deref() == Some(q.as_str());
            let active = self.items.iter().filter(|i| in_q(i) && i.status().is_active()).count();
            let waiting: Vec<u64> = self.items.iter().filter(|i| in_q(i) && i.status() == Status::Queued).map(|i| i.id).collect();
            if waiting.is_empty() && active == 0 {
                self.running.remove(&q);
                self.toast(format!("{q} is finished"));
                if def.schedule.when_done == WhenDone::ExitApp && !self.demo {
                    self.save();
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                continue;
            }
            for id in waiting.into_iter().take(def.max_parallel.max(1).saturating_sub(active)) {
                self.start_item(id);
            }
        }
    }

    fn run_schedule(&mut self) {
        if self.demo || self.last_schedule_check.elapsed().as_secs() < 1 {
            return;
        }
        self.last_schedule_check = Instant::now();
        let now = chrono::Local::now();
        let (hm, today) = (now.format("%H:%M").to_string(), now.format("%Y-%m-%d").to_string());
        let mut to_start = Vec::new();
        let mut to_stop = Vec::new();
        for q in &mut self.settings.queues {
            let s = &mut q.schedule;
            if s.start_on && hm == normalize_time(&s.start_time) && s.last_start != today {
                s.last_start = today.clone();
                if !s.daily {
                    s.start_on = false;
                }
                to_start.push(q.name.clone());
            }
            if s.stop_on && hm == normalize_time(&s.stop_time) && s.last_stop != today {
                s.last_stop = today.clone();
                if !s.daily {
                    s.stop_on = false;
                }
                to_stop.push(q.name.clone());
            }
        }
        for q in to_start {
            self.start_queue(&q);
            self.toast(format!("Scheduler started {q}"));
            self.dirty = true;
        }
        for q in to_stop {
            if self.running.contains(&q) {
                self.stop_queue(&q);
                self.toast(format!("Scheduler stopped {q}"));
            }
            self.dirty = true;
        }
    }
}

/// Web links in a piece of text: link lists, ".url" shortcut files and web pages.
pub fn links_in_text(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '<' || c == '>') {
        let w = raw.trim_start_matches("URL=").trim_end_matches([',', ';', ')']);
        if (w.starts_with("http://") || w.starts_with("https://")) && url::Url::parse(w).is_ok() && !out.iter().any(|x| x == w) {
            out.push(w.to_string());
        }
    }
    out
}

/// "7:5" -> "07:05". Returns an empty string for times that don't make sense.
pub fn normalize_time(t: &str) -> String {
    let Some((h, m)) = t.trim().split_once(':') else { return String::new() };
    match (h.trim().parse::<u32>(), m.trim().parse::<u32>()) {
        (Ok(h), Ok(m)) if h < 24 && m < 60 => format!("{h:02}:{m:02}"),
        _ => String::new(),
    }
}

/// Expand "https://x.org/img*.jpg" from..to with zero padding.
pub fn expand_pattern(pattern: &str, from: u32, to: u32, width: usize) -> Result<Vec<String>, String> {
    if !pattern.contains('*') {
        return Err("Put a * in the address where the number goes.".into());
    }
    if to < from {
        return Err("\"To\" must be at least \"From\".".into());
    }
    if to - from >= 10_000 {
        return Err("That's more than 10 000 links.".into());
    }
    Ok((from..=to).map(|n| pattern.replacen('*', &format!("{n:0width$}"), 1)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_and_time() {
        assert_eq!(expand_pattern("https://x.org/p*.jpg", 8, 10, 2).unwrap(), vec!["https://x.org/p08.jpg", "https://x.org/p09.jpg", "https://x.org/p10.jpg"]);
        assert!(expand_pattern("https://x.org/p.jpg", 1, 2, 1).is_err());
        assert_eq!(normalize_time("7:5"), "07:05");
        assert_eq!(normalize_time("25:00"), "");
    }

    #[test]
    fn categories_logins_and_link_lists() {
        let mut cats = default_categories();
        assert_eq!(category_for(&cats, "a.ZIP"), "Compressed");
        assert_eq!(category_for(&cats, "song.flac"), "Music");
        assert_eq!(category_for(&cats, "notes"), "General");
        cats.insert(0, CategoryDef { name: "Photos".into(), exts: "jpg, png .heic".into(), folder: String::new() });
        assert_eq!(category_for(&cats, "me.heic"), "Photos");
        assert_eq!(category_for(&cats, "me.png"), "Photos");

        let l = SiteLogin { site: "https://files.example.com/".into(), user: "u".into(), pass: "p".into() };
        assert!(l.matches("https://files.example.com/a.zip"));
        assert!(l.matches("https://eu.files.example.com/a.zip"));
        assert!(!l.matches("https://example.com/a.zip"));
        assert!(!l.matches("https://badfiles.example.com.evil.org/a.zip"));

        let text = "[InternetShortcut]\nURL=https://x.org/a.zip\n<a href=\"https://x.org/b.pdf\">b</a> https://x.org/a.zip, ftp://no";
        assert_eq!(links_in_text(text), vec!["https://x.org/a.zip", "https://x.org/b.pdf"]);
    }
}
