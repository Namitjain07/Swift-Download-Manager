//! Runs MCP commands against the app's own state. The open window and the windowless engine
//! both go through `App::run_command`, so an assistant sees the same behaviour in either.

use super::*;
use crate::engine;
use crate::mcp::command::Command;
use serde_json::{json, Value};

/// What the status bar says about AI assistants.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AiState {
    /// Switched off in the settings.
    Off,
    /// On, but this window could not open its control door (another copy owns the downloads).
    Unavailable,
    /// On and waiting for an assistant.
    Ready,
    /// An assistant used it in the last few seconds.
    Active,
}

const OFF: &str = "MCP is turned off in Swift Download Manager (Options > AI assistants).";

fn status_key(s: Status) -> &'static str {
    match s {
        Status::Queued => "queued",
        Status::Connecting => "connecting",
        Status::Downloading => "downloading",
        Status::Paused => "paused",
        Status::Complete => "complete",
        Status::Error => "error",
    }
}

/// The address without any user name or password in it.
fn redact(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) => {
            let _ = u.set_username("");
            let _ = u.set_password(None);
            u.to_string()
        }
        Err(_) => url.to_string(),
    }
}

fn summary(item: &Item) -> Value {
    json!({
        "id": item.id,
        "name": item.name,
        "status": status_key(item.status()),
        "progress": (item.progress() as f64 * 10_000.0).round() / 10_000.0,
        "size_bytes": item.total(),
        "downloaded_bytes": item.done(),
        "speed_bytes_per_sec": item.speed().round(),
        "category": item.category,
        "queue": item.queue,
        "error": item.live.error.lock().unwrap().clone(),
    })
}

/// The summary plus the details. Passwords and cookies are never included.
fn detail(item: &Item) -> Value {
    let mut v = summary(item);
    v["url"] = json!(redact(&item.url));
    v["folder"] = json!(item.folder.display().to_string());
    v["path"] = json!(item.path().display().to_string());
    v["time_left_secs"] = json!(item.secs_left().round());
    v["checksum"] = json!(item.check);
    v["added"] = json!(item.added);
    v["last_try"] = json!(item.last_try);
    v["description"] = json!(item.description);
    v
}

impl App {
    /// Run one MCP command. Both the open window and the windowless engine call this.
    pub fn run_command(&mut self, cmd: Command) -> Result<Value, String> {
        if !self.settings.mcp_enabled {
            return Err(OFF.into());
        }
        self.last_ai_call = Some(std::time::Instant::now());
        self.execute(cmd, None)
    }

    /// Run a command for a person using the web interface: they only see and change their own downloads
    /// (an administrator sees everybody's), and what they add goes into their own folder.
    pub fn run_as(&mut self, cmd: Command, who: &crate::mcp::command::Who) -> Result<Value, String> {
        self.execute(cmd, Some(who))
    }

    /// A request from either kind of caller.
    pub fn run_request(&mut self, cmd: Command, who: Option<crate::mcp::command::Who>) -> Result<Value, String> {
        match who {
            Some(w) => self.run_as(cmd, &w),
            None => self.run_command(cmd),
        }
    }

    fn execute(&mut self, cmd: Command, who: Option<&crate::mcp::command::Who>) -> Result<Value, String> {
        // `mine`: only this person's downloads. `owner`: who new downloads belong to.
        let mine: Option<&str> = who.filter(|w| !w.name.is_empty() && !w.admin).map(|w| w.name.as_str());
        let owner: &str = who.map(|w| w.name.as_str()).unwrap_or("");
        let shared_server = who.is_some_and(|w| !w.name.is_empty());
        let queue_rights = || if shared_server && mine.is_some() { Err("Only an administrator can start or stop queues.".to_string()) } else { Ok(()) };
        // Settings that reach beyond one person's downloads: administrators of the web interface only (an AI assistant cannot).
        let admin_only = || match who {
            Some(w) if w.admin => Ok(()),
            Some(_) => Err("Only an administrator can do that.".to_string()),
            None => Err("That can only be done in the web interface, by an administrator.".to_string()),
        };
        match cmd {
            Command::ListDownloads { status, query, limit } => self.cmd_list(status, query, limit, mine),
            Command::GetDownload { id } => Ok(detail(self.find_owned(id, mine)?)),
            Command::GetStatus => Ok(self.cmd_status(mine)),
            Command::ListQueues => Ok(self.cmd_queues()),
            Command::AddDownload { url, name, category, queue, start } => self.cmd_add(url, name, category, queue, start, owner, who.is_none()),
            Command::AddMany { urls, category, queue, start } => self.cmd_add_many(urls, category, queue, start, owner, who.is_none()),
            Command::PauseAll => {
                let ids = self.ids_where(mine, |s| s.is_active() || s == Status::Queued);
                ids.iter().for_each(|id| self.stop_item(*id));
                Ok(json!({ "paused": ids.len() }))
            }
            Command::ResumeAll => {
                // Through the queues, so each queue's own limit on parallel downloads still holds.
                let ids = self.ids_where(mine, |s| matches!(s, Status::Paused | Status::Error | Status::Queued));
                for id in &ids {
                    let queue = self.items.iter().find(|i| i.id == *id).and_then(|i| i.queue.clone()).unwrap_or_else(|| MAIN_QUEUE.to_string());
                    self.move_to_queue(*id, &queue);
                    self.start_queue(&queue);
                }
                Ok(json!({ "resumed": ids.len() }))
            }
            Command::ClearCompleted => {
                let ids = self.ids_where(mine, |s| s == Status::Complete);
                self.forget_items(&ids);
                Ok(json!({ "removed": ids.len(), "note": "Taken off the list. No files were deleted." }))
            }
            Command::ListNetworks => {
                admin_only()?;
                Ok(self.cmd_networks())
            }
            Command::SetNetworks { on, auto, names, off } => {
                admin_only()?;
                self.cmd_set_networks(on, auto, names, off)
            }
            Command::SetSpeedLimit { kbps } => {
                admin_only()?;
                let kbps = kbps.unwrap_or(0);
                self.settings.limit_on = kbps > 0;
                if kbps > 0 {
                    self.settings.limit_kbps = kbps.clamp(1, 1_000_000);
                }
                self.dirty = true;
                self.apply_settings();
                Ok(json!({ "speed_limit_kbps": self.settings.limit_on.then_some(self.settings.limit_kbps) }))
            }
            Command::Usage => {
                admin_only()?;
                Ok(self.cmd_usage())
            }
            Command::Pause { id } => {
                self.find_owned(id, mine)?;
                self.cmd_pause(id)
            }
            Command::Resume { id } => {
                self.find_owned(id, mine)?;
                self.cmd_resume(id)
            }
            Command::StartQueue { name } => {
                queue_rights()?;
                let name = self.queue_named(&name)?;
                self.start_queue(&name);
                Ok(json!({ "queue": name, "running": true, "waiting": self.waiting_in(&name) }))
            }
            Command::StopQueue { name } => {
                queue_rights()?;
                let name = self.queue_named(&name)?;
                self.stop_queue(&name);
                Ok(json!({ "queue": name, "running": false, "waiting": self.waiting_in(&name) }))
            }
            Command::Handoff => Err("Only a windowless engine hands the downloads over; this app is running them.".into()),
            Command::Remove { id } => {
                let name = self.find_owned(id, mine)?.name.clone();
                self.forget_items(&[id]);
                if who.is_none() {
                    self.toast(format!("An AI assistant removed {name} from the list"));
                }
                Ok(json!({ "removed": id, "name": name, "note": "Taken off the list. No files were deleted." }))
            }
            Command::Locate { id } => {
                let item = self.find_owned(id, mine)?;
                if item.status() != Status::Complete {
                    return Err(format!("Download {id} is not finished yet."));
                }
                Ok(json!({ "path": item.path().display().to_string(), "name": item.name, "size": item.total() }))
            }
        }
    }

    pub(super) fn ai_state(&self) -> AiState {
        if !self.settings.mcp_enabled {
            AiState::Off
        } else if self.control_rx.is_none() && !self.demo {
            AiState::Unavailable
        } else if self.last_ai_call.is_some_and(|t| t.elapsed().as_secs() < 5) {
            AiState::Active
        } else {
            AiState::Ready
        }
    }

    /// Per-frame work for a windowless engine: speeds, queues, schedules, saving.
    pub fn pump(&mut self) {
        self.tick();
    }

    /// Open the private control door so `--mcp` processes can reach this app.
    pub(super) fn start_control(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        match crate::mcp::control::start(&self.config_dir, tx, self.ctx.clone(), "window") {
            Ok(_) => self.control_rx = Some(rx),
            Err(e) => eprintln!("swift-download-manager: the MCP control listener could not start: {e}"),
        }
    }

    /// Start, stop or move the web interface to match the settings.
    pub(super) fn sync_web(&mut self) {
        let want = self.settings.web_enabled && !self.demo && !self.headless && self.state_lock.is_some();
        if !want {
            self.web = None;
            self.web_rx = None;
            return;
        }
        let port = self.settings.web_port;
        if self.web.as_ref().is_some_and(|h| h.addr.port() == port) {
            return;
        }
        self.web = None;
        let (tx, rx) = std::sync::mpsc::channel();
        let cfg = crate::web::server::Config { bind: [127, 0, 0, 1].into(), port, mode: crate::web::server::Mode::Personal, dir: self.config_dir.clone(), secure_cookie: false };
        match crate::web::server::start_retrying(cfg, tx, self.ctx.clone(), 20) {
            Ok(h) => {
                self.web = Some(h);
                self.web_rx = Some(rx);
            }
            Err(e) => self.toast(format!("The web interface could not start on port {port}: {e}")),
        }
    }

    /// Set up a download server (`--server`): where files go, and whether private addresses are refused.
    pub fn configure_server(&mut self, downloads: PathBuf, block_private: bool) {
        self.settings.folder = downloads;
        self.settings.per_category_folders = false;
        self.settings.block_private = block_private;
        self.dirty = true;
        self.save();
    }

    /// The port of the web interface, when it is switched on in the settings.
    pub fn web_port_if_enabled(&self) -> Option<u16> {
        self.settings.web_enabled.then_some(self.settings.web_port)
    }

    /// The link that opens the web interface in a browser, while it is on.
    pub(super) fn web_link(&self) -> Option<String> {
        self.web.as_ref().map(|h| format!("http://127.0.0.1:{}/?key={}", h.addr.port(), h.key))
    }

    /// Answer commands that arrived from `--mcp` processes and browsers. Called every frame.
    pub(super) fn serve_control(&mut self) {
        let mut jobs: Vec<_> = self.control_rx.iter().flat_map(|rx| rx.try_iter()).collect();
        jobs.extend(self.web_rx.iter().flat_map(|rx| rx.try_iter()));
        for job in jobs {
            let answer = self.run_request(job.cmd, job.who);
            let _ = job.reply.send(answer);
        }
    }

    /// The window is closing: save, and stop advertising the control door.
    pub(super) fn close_control(&mut self) {
        // Stop listening for browsers too, so the next process can take the port.
        self.web = None;
        self.web_rx = None;
        if self.control_rx.take().is_some() {
            crate::mcp::control::clear_info(&self.config_dir);
        }
    }

    /// Take the right to run the downloads, asking a windowless engine to hand over if needed.
    pub(super) fn take_state_lock(&mut self) {
        self.state_lock = crate::mcp::lock::acquire_with_handoff(&self.config_dir, std::time::Duration::from_secs(5));
    }

    /// Another process is about to take over: pause what runs, remember it, save and let go.
    pub fn prepare_handoff(&mut self) {
        self.handoff_queues = self.running.iter().cloned().collect();
        for item in &mut self.items {
            if item.status().is_active() {
                item.handoff_resume = true;
            }
        }
        self.stop_all();
        for _ in 0..300 {
            if !self.items.iter().any(|i| i.status().is_active()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        self.save();
    }

    /// After loading: carry on with what was running when the downloads were handed over.
    pub(super) fn resume_handed_over(&mut self) {
        for q in std::mem::take(&mut self.handoff_queues) {
            self.running.insert(q);
            self.dirty = true;
        }
        let ids: Vec<u64> = self.items.iter().filter(|i| i.handoff_resume).map(|i| i.id).collect();
        for id in ids {
            if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
                item.handoff_resume = false;
            }
            self.start_item(id);
            self.dirty = true;
        }
    }

    /// A windowless engine is stopping: pause everything and save.
    pub fn shutdown(&mut self) {
        self.stop_all();
        for _ in 0..150 {
            if !self.items.iter().any(|i| i.status().is_active()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        self.save();
    }

    fn find_item(&self, id: u64) -> Result<&Item, String> {
        self.items.iter().find(|i| i.id == id).ok_or_else(|| format!("No download with id {id}. Use list_downloads to see the ids."))
    }

    /// Like `find_item`, but somebody else's download looks exactly like one that does not exist.
    fn find_owned(&self, id: u64, mine: Option<&str>) -> Result<&Item, String> {
        self.items.iter().find(|i| i.id == id && mine.is_none_or(|m| i.owner == m)).ok_or_else(|| format!("No download with id {id}. Use list_downloads to see the ids."))
    }

    fn queue_named(&self, name: &str) -> Result<String, String> {
        let want = name.trim();
        self.settings
            .queues
            .iter()
            .find(|q| q.name.eq_ignore_ascii_case(want))
            .map(|q| q.name.clone())
            .ok_or_else(|| format!("Unknown queue \"{want}\". Available: {}", self.settings.queues.iter().map(|q| q.name.as_str()).collect::<Vec<_>>().join(", ")))
    }

    fn waiting_in(&self, queue: &str) -> usize {
        self.items.iter().filter(|i| i.queue.as_deref() == Some(queue) && i.status() == Status::Queued).count()
    }

    fn cmd_list(&self, status: Option<String>, query: Option<String>, limit: Option<usize>, mine: Option<&str>) -> Result<Value, String> {
        let want = match status.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            None => None,
            Some(s) => Some(
                [Status::Queued, Status::Connecting, Status::Downloading, Status::Paused, Status::Complete, Status::Error]
                    .into_iter()
                    .find(|st| status_key(*st) == s.to_ascii_lowercase())
                    .ok_or_else(|| format!("Unknown status \"{s}\". Use one of: queued, connecting, downloading, paused, complete, error."))?,
            ),
        };
        let q = query.unwrap_or_default().trim().to_lowercase();
        let hits: Vec<&Item> = self
            .items
            .iter()
            .filter(|i| mine.is_none_or(|m| i.owner == m))
            .filter(|i| want.is_none_or(|w| i.status() == w))
            .filter(|i| q.is_empty() || i.name.to_lowercase().contains(&q) || i.url.to_lowercase().contains(&q))
            .collect();
        let limit = limit.unwrap_or(100).clamp(1, 500);
        Ok(json!({
            "total": hits.len(),
            "count": hits.len().min(limit),
            "downloads": hits.iter().take(limit).map(|i| summary(i)).collect::<Vec<_>>(),
        }))
    }

    fn cmd_status(&self, mine: Option<&str>) -> Value {
        let own = || self.items.iter().filter(|i| mine.is_none_or(|m| i.owner == m));
        let n = |f: &dyn Fn(Status) -> bool| own().filter(|i| f(i.status())).count();
        json!({
            "mode": if self.headless { "headless" } else { "window" },
            "downloads": own().count(),
            "active": n(&|s| s.is_active()),
            "queued": n(&|s| s == Status::Queued),
            "paused": n(&|s| s == Status::Paused),
            "complete": n(&|s| s == Status::Complete),
            "failed": n(&|s| s == Status::Error),
            "speed_bytes_per_sec": own().filter(|i| i.status().is_active()).map(|i| i.speed()).sum::<f64>().round(),
            "speed_limit_kbps": self.settings.limit_on.then_some(self.settings.limit_kbps),
            "queues_running": self.running.iter().collect::<Vec<_>>(),
        })
    }

    fn cmd_queues(&self) -> Value {
        let list: Vec<Value> = self
            .settings
            .queues
            .iter()
            .map(|q| {
                json!({
                    "name": q.name,
                    "running": self.running.contains(&q.name),
                    "max_parallel": q.max_parallel,
                    "waiting": self.waiting_in(&q.name),
                    "schedule": {
                        "start_at": q.schedule.start_on.then(|| q.schedule.start_time.clone()),
                        "stop_at": q.schedule.stop_on.then(|| q.schedule.stop_time.clone()),
                        "daily": q.schedule.daily,
                    },
                })
            })
            .collect();
        json!({ "queues": list, "categories": self.settings.categories.iter().map(|c| c.name.clone()).collect::<Vec<_>>() })
    }

    #[allow(clippy::too_many_arguments)]
    fn cmd_add(&mut self, url: String, name: Option<String>, category: Option<String>, queue: Option<String>, start: Option<bool>, owner: &str, announce: bool) -> Result<Value, String> {
        let url = url.trim().to_string();
        Self::check_url(&url)?;
        let given = name.as_deref().map(str::trim).filter(|n| !n.is_empty());
        let name = engine::sanitize(given.map(str::to_string).unwrap_or_else(|| engine::file_name_from_url(&url)).as_str());
        let category = match category.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
            Some(c) => self
                .settings
                .categories
                .iter()
                .find(|d| d.name.eq_ignore_ascii_case(c))
                .map(|d| d.name.clone())
                .ok_or_else(|| format!("Unknown category \"{c}\". Available: {}", self.settings.categories.iter().map(|d| d.name.as_str()).collect::<Vec<_>>().join(", ")))?,
            None => self.cat_of(&name),
        };
        let queue = match queue.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            Some(q) => self.queue_named(q)?,
            None => MAIN_QUEUE.to_string(),
        };
        // The destination always comes from the category; there is no way to pick a folder.
        let mut folder = self.folder_for(&category);
        // On a shared server everybody downloads into a folder of their own.
        if !owner.is_empty() {
            folder = folder.join(owner);
        }
        let d = AddDialog { url, name_edited: given.is_some(), folder: folder.display().to_string(), name, category, queue, ..Default::default() };
        // Adding selects the new row; keep whatever the person had selected.
        let selection = std::mem::take(&mut self.selected);
        let added = self.add_download(&d, start.unwrap_or(true));
        self.selected = selection;
        let id = added?;
        self.progress_open.remove(&id);
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.owner = owner.to_string();
        }
        let item = self.find_item(id)?;
        let (shown, status) = (item.name.clone(), status_key(item.status()));
        let out = json!({ "id": id, "name": shown, "folder": item.folder.display().to_string(), "status": status, "queue": item.queue });
        if announce {
            self.toast(format!("An AI assistant added {shown}"));
        }
        Ok(out)
    }

    /// The ids of this person's downloads (everybody's when `mine` is None) whose state `f` accepts.
    fn ids_where(&self, mine: Option<&str>, f: impl Fn(Status) -> bool) -> Vec<u64> {
        self.items.iter().filter(|i| mine.is_none_or(|m| i.owner == m)).filter(|i| f(i.status())).map(|i| i.id).collect()
    }

    /// Several links at once; each gets its own answer, so one bad link does not spoil the others.
    fn cmd_add_many(&mut self, urls: Vec<String>, category: Option<String>, queue: Option<String>, start: Option<bool>, owner: &str, announce: bool) -> Result<Value, String> {
        const MOST: usize = 200;
        let urls: Vec<String> = urls.into_iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).collect();
        if urls.is_empty() {
            return Err("Give at least one link (urls).".into());
        }
        if urls.len() > MOST {
            return Err(format!("At most {MOST} links at a time."));
        }
        let (mut added, mut failed) = (0, 0);
        let mut results = Vec::new();
        for url in urls {
            let shown = redact(&url);
            match self.cmd_add(url, None, category.clone(), queue.clone(), start, owner, false) {
                Ok(v) => {
                    added += 1;
                    results.push(json!({ "url": shown, "ok": true, "id": v["id"], "name": v["name"] }));
                }
                Err(e) => {
                    failed += 1;
                    results.push(json!({ "url": shown, "ok": false, "error": e }));
                }
            }
        }
        if announce && added > 0 {
            self.toast(format!("An AI assistant added {added} downloads"));
        }
        Ok(json!({ "added": added, "failed": failed, "results": results }))
    }

    /// The networks of this computer, which ones downloads use, and what each is carrying right now.
    fn cmd_networks(&self) -> Value {
        use crate::netroute::{interfaces, Choice};
        let all = interfaces();
        let choice: Choice = self.net_choice();
        let used: Vec<String> = Choice { on: true, ..choice }.routes(&all).into_iter().map(|r| r.name).collect();
        // What every running download has on each network, added up.
        let mut live: std::collections::BTreeMap<String, (usize, f64, &'static str)> = std::collections::BTreeMap::new();
        for item in &self.items {
            if let Some(router) = item.live.router.lock().unwrap().as_ref() {
                for n in router.status() {
                    let e = live.entry(n.name).or_insert((0, 0.0, "working"));
                    e.0 += n.connections;
                    e.1 += n.bytes_per_sec;
                    if n.gone {
                        e.2 = "gone";
                    } else if n.resting && e.2 != "gone" {
                        e.2 = "resting";
                    }
                }
            }
        }
        json!({
            "enabled": self.settings.multi_network,
            "automatic": self.settings.networks_auto,
            "chosen": self.settings.networks,
            "off": self.settings.networks_off,
            "available": all.iter().map(|i| json!({ "name": i.name, "kind": i.kind.label(), "address": i.addr.to_string(), "used": used.contains(&i.name) })).collect::<Vec<_>>(),
            "live": live.into_iter().map(|(name, (connections, speed, state))| json!({ "name": name, "connections": connections, "speed_bytes_per_sec": speed.round(), "state": state })).collect::<Vec<_>>(),
        })
    }

    fn cmd_set_networks(&mut self, on: Option<bool>, auto: Option<bool>, names: Option<Vec<String>>, off: Option<Vec<String>>) -> Result<Value, String> {
        let clean = |list: Vec<String>| -> Result<Vec<String>, String> {
            let list: Vec<String> = list.into_iter().map(|n| n.trim().to_string()).collect();
            if list.len() > 32 || list.iter().any(|n| n.is_empty() || n.chars().count() > 128) {
                return Err("Network names have 1 to 128 characters, and at most 32 can be listed.".into());
            }
            let mut seen = Vec::new();
            for n in list {
                if !seen.contains(&n) {
                    seen.push(n);
                }
            }
            Ok(seen)
        };
        // Check everything first, then change: a bad request changes nothing.
        let names = names.map(clean).transpose()?;
        let off = off.map(clean).transpose()?;
        if let Some(v) = on {
            self.settings.multi_network = v;
        }
        if let Some(v) = auto {
            self.settings.networks_auto = v;
        }
        if let Some(v) = names {
            self.settings.networks = v;
        }
        if let Some(v) = off {
            self.settings.networks_off = v;
        }
        self.dirty = true;
        self.apply_settings();
        Ok(self.cmd_networks())
    }

    /// What each person has: how many downloads, how many are running or done, how much was fetched.
    fn cmd_usage(&self) -> Value {
        let mut per: std::collections::BTreeMap<&str, (usize, usize, usize, u64, f64)> = std::collections::BTreeMap::new();
        for i in &self.items {
            let e = per.entry(i.owner.as_str()).or_default();
            e.0 += 1;
            if i.status().is_active() {
                e.1 += 1;
                e.4 += i.speed();
            }
            if i.status() == Status::Complete {
                e.2 += 1;
            }
            e.3 += i.done();
        }
        json!({
            "users": per
                .into_iter()
                .map(|(name, (downloads, active, complete, bytes, speed))| json!({
                    "name": name, "downloads": downloads, "active": active, "complete": complete, "downloaded_bytes": bytes, "speed_bytes_per_sec": speed.round(),
                }))
                .collect::<Vec<_>>(),
        })
    }

    fn cmd_pause(&mut self, id: u64) -> Result<Value, String> {
        self.find_item(id)?;
        self.stop_item(id);
        // The connections need a moment to let go; report the settled state.
        for _ in 0..30 {
            if !self.find_item(id)?.status().is_active() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(summary(self.find_item(id)?))
    }

    fn cmd_resume(&mut self, id: u64) -> Result<Value, String> {
        if self.find_item(id)?.status() == Status::Complete {
            return Err(format!("Download {id} is already complete."));
        }
        self.start_item(id);
        Ok(summary(self.find_item(id)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::command::Command;
    use serde_json::json;
    use std::path::Path;

    fn test_app(dir: &Path) -> App {
        App::build(egui::Context::default(), Launch { headless: true, config_dir: Some(dir.to_path_buf()), ..Default::default() })
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sdm-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn add(url: &str, name: Option<&str>) -> Command {
        Command::AddDownload { url: url.into(), name: name.map(Into::into), category: None, queue: None, start: Some(false) }
    }

    #[test]
    fn add_download_rejects_non_http_links() {
        let dir = scratch("urls");
        let mut app = test_app(&dir);
        for url in ["file:///etc/passwd", "javascript:alert(1)", "ftp://example.com/a.zip", "not a link", ""] {
            assert!(app.run_command(add(url, None)).is_err(), "{url} should be refused");
        }
        assert!(app.items.is_empty());
    }

    #[test]
    fn add_download_cannot_escape_the_download_folder() {
        let dir = scratch("escape");
        let mut app = test_app(&dir);
        for name in ["../../x.bin", "/etc/passwd", "..\\..\\evil.exe", "a/b/c.zip"] {
            let out = app.run_command(add("https://example.com/f.zip", Some(name))).unwrap();
            let id = out["id"].as_u64().unwrap();
            let item = app.items.iter().find(|i| i.id == id).unwrap();
            assert!(!item.name.contains('/') && !item.name.contains('\\'), "{name} became {}", item.name);
            assert_ne!(item.name, "..");
            assert_eq!(item.folder, app.folder_for(&item.category), "folder must come from the category");
            assert_eq!(item.path().parent().unwrap(), app.folder_for(&item.category));
        }
    }

    #[test]
    fn add_download_has_no_folder_argument() {
        // The tool schema offers no folder, and an unknown argument is not read.
        let from_json = serde_json::from_value::<Command>(json!({"cmd": "add_download", "url": "https://e.com/a.zip", "folder": "/etc"}));
        assert!(matches!(from_json, Ok(Command::AddDownload { .. })));
        let dir = scratch("nofolder");
        let mut app = test_app(&dir);
        app.run_command(from_json.unwrap()).unwrap();
        assert!(app.items.iter().all(|i| i.folder != Path::new("/etc")));
    }

    #[test]
    fn add_download_rejects_unknown_category_and_queue() {
        let dir = scratch("unknown");
        let mut app = test_app(&dir);
        let e = app.run_command(Command::AddDownload { url: "https://e.com/a.zip".into(), name: None, category: Some("Nope".into()), queue: None, start: Some(false) }).unwrap_err();
        assert!(e.contains("Video") && e.contains("Documents"), "lists the valid categories: {e}");
        let e = app.run_command(Command::AddDownload { url: "https://e.com/a.zip".into(), name: None, category: None, queue: Some("Nope".into()), start: Some(false) }).unwrap_err();
        assert!(e.contains(MAIN_QUEUE), "lists the valid queues: {e}");
        assert!(app.items.is_empty());
    }

    #[test]
    fn remove_never_touches_files() {
        let dir = scratch("remove");
        let mut app = test_app(&dir);
        // A finished download with its file, and an unfinished one with a partial file.
        let done = dir.join("done.bin");
        std::fs::write(&done, b"finished").unwrap();
        let live = Live::new(8, 8, Status::Complete, Vec::new(), true);
        app.items.push(Item::new(100, "https://e.com/done.bin".into(), "done.bin".into(), dir.clone(), "General".into(), live, &app.limiter));
        let id = app.run_command(add("https://e.com/part.bin", Some("part.bin"))).unwrap()["id"].as_u64().unwrap();
        let part_path = app.items.iter().find(|i| i.id == id).unwrap().path();
        let part = engine::temp_path(&part_path);
        std::fs::create_dir_all(part.parent().unwrap()).unwrap();
        std::fs::write(&part, b"half").unwrap();

        app.run_command(Command::Remove { id: 100 }).unwrap();
        app.run_command(Command::Remove { id }).unwrap();
        assert!(app.items.is_empty());
        assert!(done.exists(), "the finished file must stay");
        assert!(part.exists(), "the partial file must stay");
        let _ = std::fs::remove_file(&part);
    }

    #[test]
    fn disabled_mcp_refuses_everything() {
        let dir = scratch("off");
        let mut app = test_app(&dir);
        app.settings.mcp_enabled = false;
        for cmd in [Command::GetStatus, Command::ListQueues, add("https://e.com/a.zip", None), Command::Remove { id: 1 }] {
            let e = app.run_command(cmd).unwrap_err();
            assert!(e.contains("turned off"), "{e}");
        }
        assert!(app.items.is_empty());
    }

    #[test]
    fn list_downloads_filters_by_status_and_query() {
        let dir = scratch("list");
        let mut app = test_app(&dir);
        app.run_command(add("https://e.com/alpha.zip", None)).unwrap();
        app.run_command(add("https://e.com/beta.pdf", None)).unwrap();
        let all = app.run_command(Command::ListDownloads { status: None, query: None, limit: None }).unwrap();
        assert_eq!(all["count"], 2);
        let q = app.run_command(Command::ListDownloads { status: None, query: Some("BETA".into()), limit: None }).unwrap();
        assert_eq!(q["downloads"][0]["name"], "beta.pdf");
        assert_eq!(q["count"], 1);
        let queued = app.run_command(Command::ListDownloads { status: Some("queued".into()), query: None, limit: Some(1) }).unwrap();
        assert_eq!(queued["count"], 1);
        assert_eq!(queued["total"], 2);
        let none = app.run_command(Command::ListDownloads { status: Some("complete".into()), query: None, limit: None }).unwrap();
        assert_eq!(none["count"], 0);
        assert!(app.run_command(Command::ListDownloads { status: Some("flying".into()), query: None, limit: None }).is_err());
    }

    #[test]
    fn unknown_ids_and_queues_give_clear_errors() {
        let dir = scratch("ids");
        let mut app = test_app(&dir);
        for cmd in [Command::GetDownload { id: 99 }, Command::Pause { id: 99 }, Command::Resume { id: 99 }, Command::Remove { id: 99 }] {
            let e = app.run_command(cmd).unwrap_err();
            assert!(e.contains("99") && e.contains("list_downloads"), "{e}");
        }
        assert!(app.run_command(Command::StartQueue { name: "Nope".into() }).unwrap_err().contains(MAIN_QUEUE));
    }

    #[test]
    fn pause_and_queue_commands_change_state() {
        let dir = scratch("state");
        let mut app = test_app(&dir);
        let id = app.run_command(add("https://e.com/a.zip", None)).unwrap()["id"].as_u64().unwrap();
        let out = app.run_command(Command::Pause { id }).unwrap();
        assert_eq!(out["status"], "paused");
        let out = app.run_command(Command::StartQueue { name: MAIN_QUEUE.into() }).unwrap();
        assert_eq!(out["running"], true);
        assert!(app.running.contains(MAIN_QUEUE));
        app.run_command(Command::StopQueue { name: MAIN_QUEUE.into() }).unwrap();
        assert!(!app.running.contains(MAIN_QUEUE));
        let status = app.run_command(Command::GetStatus).unwrap();
        assert_eq!(status["mode"], "headless");
        assert_eq!(status["downloads"], 1);
    }

    #[test]
    fn headless_app_saves_to_its_own_config_dir_and_reloads() {
        let dir = scratch("persist");
        let mut app = test_app(&dir);
        app.run_command(add("https://e.com/keep.zip", None)).unwrap();
        app.save();
        assert!(dir.join("downloads.json").exists());
        let again = test_app(&dir);
        assert_eq!(again.items.len(), 1);
        assert_eq!(again.items[0].name, "keep.zip");
    }

    #[test]
    fn old_settings_default_mcp_on() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(s.mcp_enabled);
    }

    #[test]
    fn control_requests_are_answered_on_the_next_tick() {
        let dir = scratch("serve");
        let mut app = test_app(&dir);
        let (tx, rx) = std::sync::mpsc::channel();
        app.control_rx = Some(rx);
        let (reply, answer) = std::sync::mpsc::channel();
        tx.send(crate::mcp::control::Request { cmd: Command::GetStatus, reply, who: None }).unwrap();
        assert!(answer.try_recv().is_err(), "nothing happens before the app ticks");
        app.pump();
        let status = answer.try_recv().expect("answered during the tick").unwrap();
        assert_eq!(status["mode"], "headless");
    }

    #[test]
    fn running_queues_survive_a_handover() {
        let dir = scratch("queues");
        let mut app = test_app(&dir);
        app.run_command(Command::StartQueue { name: MAIN_QUEUE.into() }).unwrap();
        app.prepare_handoff();
        assert!(!app.running.contains(MAIN_QUEUE), "the old owner stops running it");
        let next = test_app(&dir);
        assert!(next.running.contains(MAIN_QUEUE), "the new owner carries on");
    }

    #[test]
    fn handoff_is_refused_by_a_running_app() {
        let dir = scratch("refuse");
        let mut app = test_app(&dir);
        assert!(app.run_command(Command::Handoff).is_err());
    }

    #[test]
    fn control_requests_get_the_off_message_when_disabled() {
        let dir = scratch("off-control");
        let mut app = test_app(&dir);
        let (tx, rx) = std::sync::mpsc::channel();
        app.control_rx = Some(rx);
        app.settings.mcp_enabled = false;
        let (reply, answer) = std::sync::mpsc::channel();
        tx.send(crate::mcp::control::Request { cmd: Command::GetStatus, reply, who: None }).unwrap();
        app.pump();
        assert!(answer.try_recv().unwrap().unwrap_err().contains("turned off"));
        app.settings.mcp_enabled = true;
        let (reply, answer) = std::sync::mpsc::channel();
        tx.send(crate::mcp::control::Request { cmd: Command::GetStatus, reply, who: None }).unwrap();
        app.pump();
        assert!(answer.try_recv().unwrap().is_ok(), "switching it back on works without a restart");
    }

    #[test]
    fn ai_state_reports_what_the_status_bar_shows() {
        let dir = scratch("ai-state");
        let mut app = test_app(&dir);
        app.settings.mcp_enabled = false;
        assert_eq!(app.ai_state(), AiState::Off);
        app.settings.mcp_enabled = true;
        assert_eq!(app.ai_state(), AiState::Unavailable, "no control door in a test app");
        let (_tx, rx) = std::sync::mpsc::channel();
        app.control_rx = Some(rx);
        assert_eq!(app.ai_state(), AiState::Ready);
        app.run_command(Command::GetStatus).unwrap();
        assert_eq!(app.ai_state(), AiState::Active, "a command was just handled");
    }

    // --- a shared server: every person has their own downloads --------------------------------

    use crate::mcp::command::Who;

    fn person(name: &str, admin: bool) -> Who {
        Who { name: name.into(), admin }
    }

    fn add_as(app: &mut App, who: &Who, url: &str, name: &str) -> u64 {
        app.run_as(add(url, Some(name)), who).unwrap()["id"].as_u64().unwrap()
    }

    #[test]
    fn people_see_and_change_only_their_own_downloads() {
        let dir = scratch("owners");
        let mut app = test_app(&dir);
        let (anna, ben) = (person("anna", false), person("ben", false));
        let a = add_as(&mut app, &anna, "https://e.com/a.zip", "a.zip");
        let b = add_as(&mut app, &ben, "https://e.com/b.zip", "b.zip");

        let list = |app: &mut App, who: &Who| app.run_as(Command::ListDownloads { status: None, query: None, limit: None }, who).unwrap();
        assert_eq!(list(&mut app, &anna)["total"], 1);
        assert_eq!(list(&mut app, &anna)["downloads"][0]["id"], a);
        assert_eq!(list(&mut app, &ben)["downloads"][0]["id"], b);

        // Somebody else's id looks exactly like an id that does not exist.
        for cmd in [Command::GetDownload { id: b }, Command::Pause { id: b }, Command::Resume { id: b }, Command::Remove { id: b }, Command::Locate { id: b }] {
            let e = app.run_as(cmd, &anna).unwrap_err();
            assert!(e.contains("No download with id"), "{e}");
        }
        assert_eq!(app.items.len(), 2, "nothing was removed");
        assert_eq!(app.run_as(Command::GetStatus, &anna).unwrap()["downloads"], 1);
        app.run_as(Command::Remove { id: a }, &anna).unwrap();
        assert_eq!(app.items.len(), 1);
    }

    #[test]
    fn an_admin_sees_everybody_and_may_manage_queues() {
        let dir = scratch("admin");
        let mut app = test_app(&dir);
        let (anna, admin) = (person("anna", false), person("root", true));
        add_as(&mut app, &anna, "https://e.com/a.zip", "a.zip");
        add_as(&mut app, &admin, "https://e.com/r.zip", "r.zip");
        assert_eq!(app.run_as(Command::ListDownloads { status: None, query: None, limit: None }, &admin).unwrap()["total"], 2);
        assert_eq!(app.run_as(Command::GetStatus, &admin).unwrap()["downloads"], 2);
        assert!(app.run_as(Command::StartQueue { name: MAIN_QUEUE.into() }, &admin).is_ok());
        let e = app.run_as(Command::StartQueue { name: MAIN_QUEUE.into() }, &anna).unwrap_err();
        assert!(e.contains("administrator"), "{e}");
        assert!(app.run_as(Command::StopQueue { name: MAIN_QUEUE.into() }, &anna).is_err());
        assert!(app.run_as(Command::ListQueues, &anna).is_ok());
    }

    #[test]
    fn each_person_downloads_into_their_own_folder() {
        let dir = scratch("folders");
        let mut app = test_app(&dir);
        let id = add_as(&mut app, &person("anna", false), "https://e.com/a.zip", "a.zip");
        let item = app.items.iter().find(|i| i.id == id).unwrap();
        assert_eq!(item.folder, app.folder_for(&item.category).join("anna"));
        assert_eq!(item.owner, "anna");
        // The local page and MCP are not tied to anybody.
        let local = add_as(&mut app, &person("", true), "https://e.com/l.zip", "l.zip");
        let item = app.items.iter().find(|i| i.id == local).unwrap();
        assert_eq!(item.folder, app.folder_for(&item.category));
        assert_eq!(item.owner, "");
        let mcp = app.run_command(add("https://e.com/m.zip", Some("m.zip"))).unwrap()["id"].as_u64().unwrap();
        assert_eq!(app.items.iter().find(|i| i.id == mcp).unwrap().owner, "");
    }

    #[test]
    fn the_local_page_sees_everything() {
        let dir = scratch("local");
        let mut app = test_app(&dir);
        app.run_command(add("https://e.com/m.zip", Some("m.zip"))).unwrap();
        let local = person("", true);
        assert_eq!(app.run_as(Command::ListDownloads { status: None, query: None, limit: None }, &local).unwrap()["total"], 1);
        assert!(app.run_as(Command::StartQueue { name: MAIN_QUEUE.into() }, &local).is_ok());
    }

    fn many(urls: &[&str]) -> Command {
        Command::AddMany { urls: urls.iter().map(|u| u.to_string()).collect(), category: None, queue: None, start: Some(false) }
    }

    #[test]
    fn several_links_are_added_at_once_and_one_bad_link_does_not_spoil_the_rest() {
        let dir = scratch("many");
        let mut app = test_app(&dir);
        let anna = person("anna", false);
        let out = app.run_as(many(&["https://e.com/a.zip", "not a link", "https://user:secret@e.com/b.zip", "   ", "ftp://e.com/c.zip"]), &anna).unwrap();
        assert_eq!((out["added"].as_u64(), out["failed"].as_u64()), (Some(2), Some(2)), "blank lines are skipped: {out}");
        let results = out["results"].as_array().unwrap();
        assert_eq!(results.len(), 4);
        assert_eq!((results[0]["ok"].clone(), results[1]["ok"].clone(), results[2]["ok"].clone(), results[3]["ok"].clone()), (json!(true), json!(false), json!(true), json!(false)));
        assert!(results[1]["error"].as_str().unwrap().len() > 5, "a bad link says why: {out}");
        assert!(!out.to_string().contains("secret"), "the answer never repeats a password: {out}");
        assert_eq!(app.items.len(), 2);
        assert!(app.items.iter().all(|i| i.owner == "anna"), "they are the caller's");
        // Nothing, or far too much, is refused as a whole.
        assert!(app.run_as(many(&[]), &anna).is_err());
        assert!(app.run_as(many(&["", "  "]), &anna).is_err());
        let lots: Vec<String> = (0..201).map(|i| format!("https://e.com/{i}.zip")).collect();
        let e = app.run_as(Command::AddMany { urls: lots, category: None, queue: None, start: Some(false) }, &anna).unwrap_err();
        assert!(e.contains("200"), "{e}");
        assert_eq!(app.items.len(), 2, "a refused request adds nothing");
        // A wrong category is told for every link, and nothing is added.
        let out = app.run_as(Command::AddMany { urls: vec!["https://e.com/z.zip".into()], category: Some("Nope".into()), queue: None, start: Some(false) }, &anna).unwrap();
        assert_eq!((out["added"].as_u64(), out["failed"].as_u64()), (Some(0), Some(1)));
        assert_eq!(app.items.len(), 2);
    }

    #[test]
    fn a_person_on_the_web_is_not_announced_as_an_ai_assistant() {
        let dir = scratch("announce");
        let mut app = test_app(&dir);
        app.run_as(add("https://e.com/a.zip", Some("a.zip")), &person("", true)).unwrap();
        app.run_as(many(&["https://e.com/b.zip", "https://e.com/c.zip"]), &person("anna", false)).unwrap();
        assert!(app.toast.is_none(), "{:?}", app.toast);
        app.run_command(add("https://e.com/m.zip", Some("m.zip"))).unwrap();
        assert!(app.toast.as_ref().unwrap().0.contains("AI assistant"), "an assistant still is");
    }

    fn status_of(app: &App, id: u64) -> Status {
        app.items.iter().find(|i| i.id == id).unwrap().status()
    }

    #[test]
    fn pause_all_resume_all_and_clear_finished_touch_only_the_callers_own_downloads() {
        let dir = scratch("bulk");
        let mut app = test_app(&dir);
        let (anna, ben) = (person("anna", false), person("ben", false));
        let a1 = add_as(&mut app, &anna, "https://e.com/a1.zip", "a1.zip");
        let a2 = add_as(&mut app, &anna, "https://e.com/a2.zip", "a2.zip");
        let b1 = add_as(&mut app, &ben, "https://e.com/b1.zip", "b1.zip");
        assert!([a1, a2, b1].iter().all(|id| status_of(&app, *id) == Status::Queued), "added without starting, they wait in the queue");
        assert!(!app.running.contains(MAIN_QUEUE));
        // Ben has paused his.
        app.stop_item(b1);
        assert_eq!(status_of(&app, b1), Status::Paused);

        // Anna carries on with everything of hers: through the queue, so the queue's own limit on parallel downloads still holds.
        assert_eq!(app.run_as(Command::ResumeAll, &anna).unwrap()["resumed"], 2);
        assert_eq!((status_of(&app, a1), status_of(&app, a2)), (Status::Queued, Status::Queued));
        assert_eq!(status_of(&app, b1), Status::Paused, "Ben's paused download is not hers to start");
        assert!(app.running.contains(MAIN_QUEUE), "the queue runs, so they begin");

        // ... and pauses all of it again.
        assert_eq!(app.run_as(Command::PauseAll, &anna).unwrap()["paused"], 2);
        assert_eq!((status_of(&app, a1), status_of(&app, a2)), (Status::Paused, Status::Paused));

        // Finished downloads: each person clears their own, and no file is touched.
        for id in [a1, b1] {
            *app.items.iter().find(|i| i.id == id).unwrap().live.status.lock().unwrap() = Status::Complete;
        }
        let (fa, fb) = (app.items.iter().find(|i| i.id == a1).unwrap().path(), app.items.iter().find(|i| i.id == b1).unwrap().path());
        for f in [&fa, &fb] {
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, b"data").unwrap();
        }
        let out = app.run_as(Command::ClearCompleted, &anna).unwrap();
        assert_eq!(out["removed"], 1);
        assert!(app.items.iter().all(|i| i.id != a1) && app.items.iter().any(|i| i.id == b1), "only Anna's finished one went");
        assert!(fa.exists() && fb.exists(), "files are never deleted");
        // Nothing to do is not an error.
        assert_eq!(app.run_as(Command::ClearCompleted, &anna).unwrap()["removed"], 0);
        assert_eq!(app.run_as(Command::PauseAll, &anna).unwrap()["paused"], 0);
    }

    #[test]
    fn network_speed_and_usage_commands_are_for_administrators_of_the_web_interface() {
        let dir = scratch("admin-only");
        let mut app = test_app(&dir);
        let (anna, root) = (person("anna", false), person("root", true));
        let all = || vec![Command::ListNetworks, Command::SetNetworks { on: Some(true), auto: Some(true), names: None, off: None }, Command::SetSpeedLimit { kbps: Some(100) }, Command::Usage];
        for cmd in all() {
            let e = app.run_as(cmd.clone(), &anna).unwrap_err();
            assert!(e.contains("administrator"), "a normal user: {e}");
            let e = app.run_command(cmd).unwrap_err();
            assert!(e.contains("web interface"), "an AI assistant: {e}");
        }
        assert!(!app.settings.multi_network && !app.settings.networks_auto && !app.settings.limit_on, "nothing changed");
        for cmd in all() {
            assert!(app.run_as(cmd, &root).is_ok());
        }
    }

    #[test]
    fn choosing_networks_is_checked_cleaned_and_saved() {
        let dir = scratch("set-networks");
        let mut app = test_app(&dir);
        let root = person("root", true);
        let out = app.run_as(Command::SetNetworks { on: Some(true), auto: Some(false), names: Some(vec!["  Wi-Fi ".into(), "Ethernet".into(), "Wi-Fi".into()]), off: Some(vec!["Phone".into()]) }, &root).unwrap();
        assert_eq!((out["enabled"].clone(), out["automatic"].clone()), (json!(true), json!(false)));
        assert_eq!(out["chosen"], json!(["Wi-Fi", "Ethernet"]), "trimmed and each name once");
        assert_eq!(out["off"], json!(["Phone"]));
        assert!(app.settings.multi_network && app.settings.networks == ["Wi-Fi", "Ethernet"]);
        assert!(app.dirty, "the choice is saved");
        // A part left out keeps what it was.
        let out = app.run_as(Command::SetNetworks { on: None, auto: Some(true), names: None, off: None }, &root).unwrap();
        assert_eq!((out["enabled"].clone(), out["automatic"].clone(), out["chosen"].clone()), (json!(true), json!(true), json!(["Wi-Fi", "Ethernet"])));
        // A bad request changes nothing at all, not even the parts that were fine.
        let before = (app.settings.multi_network, app.settings.networks_auto, app.settings.networks.clone());
        for bad in [vec!["".to_string()], vec!["x".repeat(129)], (0..33).map(|i| format!("n{i}")).collect()] {
            let e = app.run_as(Command::SetNetworks { on: Some(false), auto: Some(false), names: Some(bad), off: None }, &root).unwrap_err();
            assert!(e.contains("Network names"), "{e}");
            assert_eq!(before, (app.settings.multi_network, app.settings.networks_auto, app.settings.networks.clone()));
        }
    }

    #[test]
    fn the_network_list_says_which_cards_are_used_and_what_they_carry() {
        let dir = scratch("list-networks");
        let mut app = test_app(&dir);
        let root = person("root", true);
        let out = app.run_as(Command::ListNetworks, &root).unwrap();
        for key in ["enabled", "automatic", "chosen", "off", "available", "live"] {
            assert!(out.get(key).is_some(), "{key} is missing: {out}");
        }
        for card in out["available"].as_array().unwrap() {
            assert!(card["name"].is_string() && card["kind"].is_string() && card["address"].is_string() && card["used"].is_boolean(), "{card}");
            assert_eq!(card["used"], false, "nothing is used while the feature is off");
        }
        // What a running download has on each network is added up.
        let id = add_as(&mut app, &root, "https://e.com/a.zip", "a.zip");
        let router = crate::netroute::Router::new(vec![crate::netroute::Route { name: "Wi-Fi".into(), addr: "10.0.0.2".parse().unwrap(), index: None }, crate::netroute::Route { name: "USB".into(), addr: "10.0.1.2".parse().unwrap(), index: None }]);
        let _held = (router.acquire(), router.acquire());
        router.refresh(&[crate::netroute::Route { name: "Wi-Fi".into(), addr: "10.0.0.2".parse().unwrap(), index: None }]);
        *app.items.iter().find(|i| i.id == id).unwrap().live.router.lock().unwrap() = Some(router);
        let out = app.run_as(Command::ListNetworks, &root).unwrap();
        let live = out["live"].as_array().unwrap();
        let state = |name: &str| live.iter().find(|n| n["name"] == name).map(|n| (n["state"].clone(), n["connections"].clone()));
        assert_eq!(state("Wi-Fi"), Some((json!("working"), json!(1))), "{out}");
        assert_eq!(state("USB"), Some((json!("gone"), json!(1))), "unplugged shows as gone: {out}");
    }

    #[test]
    fn the_speed_limit_is_set_cleared_and_kept_in_range() {
        let dir = scratch("speed-limit");
        let mut app = test_app(&dir);
        let root = person("root", true);
        let cap = |app: &App| app.limiter.bytes_per_sec.load(Ordering::Relaxed);
        assert_eq!(app.run_as(Command::SetSpeedLimit { kbps: Some(2048) }, &root).unwrap()["speed_limit_kbps"], 2048);
        assert_eq!(cap(&app), 2048 * 1024, "the running limiter follows at once");
        assert_eq!(app.run_as(Command::GetStatus, &root).unwrap()["speed_limit_kbps"], 2048);
        assert_eq!(app.run_as(Command::SetSpeedLimit { kbps: Some(u64::MAX) }, &root).unwrap()["speed_limit_kbps"], 1_000_000);
        assert!(app.run_as(Command::SetSpeedLimit { kbps: Some(0) }, &root).unwrap()["speed_limit_kbps"].is_null());
        assert_eq!(cap(&app), 0);
        app.run_as(Command::SetSpeedLimit { kbps: Some(5) }, &root).unwrap();
        assert!(app.run_as(Command::SetSpeedLimit { kbps: None }, &root).unwrap()["speed_limit_kbps"].is_null());
        assert_eq!(cap(&app), 0, "nothing means off");
    }

    #[test]
    fn usage_is_counted_for_each_person() {
        let dir = scratch("usage");
        let mut app = test_app(&dir);
        let (anna, ben, root) = (person("anna", false), person("ben", false), person("root", true));
        let a1 = add_as(&mut app, &anna, "https://e.com/a1.zip", "a1.zip");
        add_as(&mut app, &anna, "https://e.com/a2.zip", "a2.zip");
        add_as(&mut app, &ben, "https://e.com/b1.zip", "b1.zip");
        let item = app.items.iter().find(|i| i.id == a1).unwrap();
        item.live.downloaded.store(1500, Ordering::SeqCst);
        *item.live.status.lock().unwrap() = Status::Complete;
        let out = app.run_as(Command::Usage, &root).unwrap();
        let users = out["users"].as_array().unwrap();
        let of = |n: &str| users.iter().find(|u| u["name"] == n).unwrap().clone();
        assert_eq!((of("anna")["downloads"].clone(), of("anna")["complete"].clone(), of("anna")["downloaded_bytes"].clone()), (json!(2), json!(1), json!(1500)));
        assert_eq!((of("ben")["downloads"].clone(), of("ben")["complete"].clone()), (json!(1), json!(0)));
    }

    #[test]
    fn the_queue_list_names_the_categories_for_the_web_page() {
        let dir = scratch("categories");
        let mut app = test_app(&dir);
        let out = app.run_command(Command::ListQueues).unwrap();
        let names: Vec<&str> = out["categories"].as_array().unwrap().iter().filter_map(|c| c.as_str()).collect();
        assert!(names.contains(&"General") && names.contains(&"Video"), "{names:?}");
    }

    #[test]
    fn a_file_can_be_fetched_only_when_it_is_finished() {
        let dir = scratch("locate");
        let mut app = test_app(&dir);
        // Keep the test's files out of the real Downloads folder.
        app.settings.folder = dir.join("downloads");
        let anna = person("anna", false);
        let id = add_as(&mut app, &anna, "https://e.com/a.bin", "a.bin");
        assert!(app.run_as(Command::Locate { id }, &anna).unwrap_err().contains("not finished"));
        let path = {
            let item = app.items.iter().find(|i| i.id == id).unwrap();
            std::fs::create_dir_all(&item.folder).unwrap();
            std::fs::write(item.path(), b"12345").unwrap();
            *item.live.status.lock().unwrap() = Status::Complete;
            item.live.total.store(5, Ordering::Relaxed);
            item.path()
        };
        let out = app.run_as(Command::Locate { id }, &anna).unwrap();
        assert_eq!(out["path"], path.display().to_string());
        assert_eq!(out["name"], "a.bin");
        assert_eq!(out["size"], 5);
    }

    #[test]
    fn owners_are_remembered() {
        let dir = scratch("remember");
        let mut app = test_app(&dir);
        add_as(&mut app, &person("anna", false), "https://e.com/a.zip", "a.zip");
        app.save();
        let again = test_app(&dir);
        assert_eq!(again.items[0].owner, "anna");
    }
}
