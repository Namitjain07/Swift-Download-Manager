//! Runs MCP commands against the app's own state. The open window and the windowless engine
//! both go through `App::run_command`, so an assistant sees the same behaviour in either.

use super::*;
use crate::engine;
use crate::mcp::command::Command;
use serde_json::{json, Value};

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
    })
}

/// The summary plus the details. Passwords and cookies are never included.
fn detail(item: &Item) -> Value {
    let mut v = summary(item);
    v["url"] = json!(redact(&item.url));
    v["folder"] = json!(item.folder.display().to_string());
    v["path"] = json!(item.path().display().to_string());
    v["time_left_secs"] = json!(item.secs_left().round());
    v["error"] = json!(item.live.error.lock().unwrap().clone());
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
        match cmd {
            Command::ListDownloads { status, query, limit } => self.cmd_list(status, query, limit),
            Command::GetDownload { id } => Ok(detail(self.find_item(id)?)),
            Command::GetStatus => Ok(self.cmd_status()),
            Command::ListQueues => Ok(self.cmd_queues()),
            Command::AddDownload { url, name, category, queue, start } => self.cmd_add(url, name, category, queue, start),
            Command::Pause { id } => self.cmd_pause(id),
            Command::Resume { id } => self.cmd_resume(id),
            Command::StartQueue { name } => {
                let name = self.queue_named(&name)?;
                self.start_queue(&name);
                Ok(json!({ "queue": name, "running": true, "waiting": self.waiting_in(&name) }))
            }
            Command::StopQueue { name } => {
                let name = self.queue_named(&name)?;
                self.stop_queue(&name);
                Ok(json!({ "queue": name, "running": false, "waiting": self.waiting_in(&name) }))
            }
            Command::Remove { id } => {
                let name = self.find_item(id)?.name.clone();
                self.forget_items(&[id]);
                self.toast(format!("An AI assistant removed {name} from the list"));
                Ok(json!({ "removed": id, "name": name, "note": "Taken off the list. No files were deleted." }))
            }
        }
    }

    /// Per-frame work for a windowless engine: speeds, queues, schedules, saving.
    #[allow(dead_code)] // used by the --mcp server
    pub fn pump(&mut self) {
        self.tick();
    }

    /// Save now (used when handing the downloads over or shutting down).
    #[allow(dead_code)] // used by the --mcp server
    pub fn save_now(&mut self) {
        self.save();
    }

    fn find_item(&self, id: u64) -> Result<&Item, String> {
        self.items.iter().find(|i| i.id == id).ok_or_else(|| format!("No download with id {id}. Use list_downloads to see the ids."))
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

    fn cmd_list(&self, status: Option<String>, query: Option<String>, limit: Option<usize>) -> Result<Value, String> {
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

    fn cmd_status(&self) -> Value {
        let n = |f: &dyn Fn(Status) -> bool| self.items.iter().filter(|i| f(i.status())).count();
        json!({
            "mode": if self.headless { "headless" } else { "window" },
            "downloads": self.items.len(),
            "active": n(&|s| s.is_active()),
            "queued": n(&|s| s == Status::Queued),
            "paused": n(&|s| s == Status::Paused),
            "complete": n(&|s| s == Status::Complete),
            "failed": n(&|s| s == Status::Error),
            "speed_bytes_per_sec": self.items.iter().filter(|i| i.status().is_active()).map(|i| i.speed()).sum::<f64>().round(),
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
        json!({ "queues": list })
    }

    fn cmd_add(&mut self, url: String, name: Option<String>, category: Option<String>, queue: Option<String>, start: Option<bool>) -> Result<Value, String> {
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
        let folder = self.folder_for(&category);
        let d = AddDialog { url, name_edited: given.is_some(), folder: folder.display().to_string(), name, category, queue, ..Default::default() };
        // Adding selects the new row; keep whatever the person had selected.
        let selection = std::mem::take(&mut self.selected);
        let added = self.add_download(&d, start.unwrap_or(true));
        self.selected = selection;
        let id = added?;
        self.progress_open.remove(&id);
        let item = self.find_item(id)?;
        let (shown, status) = (item.name.clone(), status_key(item.status()));
        let out = json!({ "id": id, "name": shown, "folder": item.folder.display().to_string(), "status": status, "queue": item.queue });
        self.toast(format!("An AI assistant added {shown}"));
        Ok(out)
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
}
