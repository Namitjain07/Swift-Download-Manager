//! App state, settings and saving. Actions are in `app/actions.rs`, the main
//! window in `app/view.rs` and pop-up windows in `app/dialogs.rs` / `app/tools.rs`.

mod actions;
mod dialogs;
mod cards;
pub(crate) mod closing;
mod grabber;
pub(crate) mod layout;
pub(crate) mod mcp_cmds;
mod tools;
mod view;

use crate::bridge::{self, Inbox, Incoming};
use crate::engine::{Limiter, Live, Segment, Status};
use crate::theme::{self, ColorTheme, ThemeMode};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub const APP_NAME: &str = "Swift Download Manager";
pub const MAIN_QUEUE: &str = "Main Queue";

// ---------------------------------------------------------------- helpers

/// Which category a file name belongs to, from the categories' file types.
pub fn category_for(cats: &[CategoryDef], name: &str) -> String {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    if !ext.is_empty() {
        for c in cats {
            if c.exts.split(|ch: char| ch.is_whitespace() || ch == ',').any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(&ext)) {
                return c.name.clone();
            }
        }
    }
    cats.iter().find(|c| c.name == "General").or(cats.last()).map(|c| c.name.clone()).unwrap_or_else(|| "General".into())
}

pub fn human_size(b: u64) -> String {
    let b = b as f64;
    if b < 1024.0 {
        format!("{} B", b as u64)
    } else if b < 1024.0 * 1024.0 {
        format!("{:.1} KB", b / 1024.0)
    } else if b < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} MB", b / 1024.0 / 1024.0)
    } else {
        format!("{:.2} GB", b / 1024.0 / 1024.0 / 1024.0)
    }
}

fn human_speed(bps: f64) -> String {
    if bps < 1.0 { String::new() } else { format!("{}/s", human_size(bps as u64)) }
}

fn human_time(secs: f64) -> String {
    if !secs.is_finite() || secs <= 0.0 {
        return String::new();
    }
    let s = secs as u64;
    if s >= 3600 {
        format!("{} h {} min", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{} min {} sec", s / 60, s % 60)
    } else {
        format!("{s} sec")
    }
}

/// Local date and time, e.g. "Nov 02 2026 21:06".
fn now_stamp() -> String {
    chrono::Local::now().format("%b %d %Y %H:%M").to_string()
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// Where settings and the download list live (`downloads.json` and the MCP files).
pub fn default_config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .unwrap_or_else(|| home_dir().join(".config"));
    base.join("swift-download-manager")
}

fn open_path(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(path).spawn();
    let _ = r;
}

// ---------------------------------------------------------------- settings

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum WhenDone {
    #[default]
    Nothing,
    ExitApp,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct CategoryDef {
    pub name: String,
    /// File endings, separated by spaces.
    pub exts: String,
    /// Own folder; empty = the default download folder.
    pub folder: String,
}

pub fn default_categories() -> Vec<CategoryDef> {
    let c = |n: &str, e: &str| CategoryDef { name: n.into(), exts: e.into(), folder: String::new() };
    vec![
        c("Compressed", "zip rar 7z tar gz bz2 xz tgz zst iso"),
        c("Documents", "pdf doc docx txt xls xlsx ppt pptx odt rtf epub csv"),
        c("Music", "mp3 wav flac ogg m4a aac wma opus"),
        c("Programs", "exe msi deb rpm appimage dmg apk sh bin pkg"),
        c("Video", "mp4 mkv avi mov webm wmv flv m4v mpg mpeg"),
        c("General", ""),
    ]
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct QueueDef {
    pub name: String,
    pub max_parallel: usize,
    pub schedule: Schedule,
}

impl Default for QueueDef {
    fn default() -> Self {
        Self { name: MAIN_QUEUE.into(), max_parallel: 2, schedule: Schedule::default() }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct SiteLogin {
    /// e.g. "example.com" (also matches its sub-sites)
    pub site: String,
    pub user: String,
    pub pass: String,
}

impl SiteLogin {
    pub fn matches(&self, url: &str) -> bool {
        let site = self.site.trim().trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_ascii_lowercase();
        let Some(host) = url::Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase())) else { return false };
        !site.is_empty() && (host == site || host.ends_with(&format!(".{site}")))
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    pub start_on: bool,
    pub start_time: String,
    pub stop_on: bool,
    pub stop_time: String,
    pub daily: bool,
    pub when_done: WhenDone,
    pub last_start: String,
    pub last_stop: String,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            start_on: false,
            start_time: "02:00".into(),
            stop_on: false,
            stop_time: "07:00".into(),
            daily: true,
            when_done: WhenDone::Nothing,
            last_start: String::new(),
            last_stop: String::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub folder: PathBuf,
    pub connections: usize,
    pub limit_on: bool,
    pub limit_kbps: u64,
    pub per_category_folders: bool,
    pub show_progress: bool,
    pub show_complete: bool,
    pub max_queue: usize,
    pub retries: u32,
    pub proxy_on: bool,
    pub proxy: String,
    pub watch_clipboard: bool,
    pub browser_listen: bool,
    pub browser_start_now: bool,
    /// Older versions kept one schedule here; now each queue has its own.
    pub schedule: Schedule,
    pub queues: Vec<QueueDef>,
    pub categories: Vec<CategoryDef>,
    pub site_logins: Vec<SiteLogin>,
    /// Automatic, Light or Dark (View > Appearance).
    pub theme: ThemeMode,
    /// The colour theme: Coral (the default), Files, Carbon, Ocean blue, ... (View > Appearance, Options).
    pub color_theme: ColorTheme,
    /// Cards or the classic table (View > Show downloads as).
    pub view: layout::ViewMode,
    /// What closing the window does (Options > Background).
    pub background: closing::BackgroundMode,
    /// Start with the system, in the background (Options > Background).
    pub launch_at_login: bool,
    /// Let AI assistants look at and control downloads through MCP (Options > AI assistants).
    pub mcp_enabled: bool,
    /// Download over several networks at once (Options > Networks).
    pub multi_network: bool,
    /// Names of the network cards to use (e.g. "wlan0", "Ethernet"); their addresses are looked up at each start.
    pub networks: Vec<String>,
    /// Let a browser on this computer use the downloads (Options > Web interface).
    pub web_enabled: bool,
    pub web_port: u16,
    /// Refuse links that lead to private or local addresses (set by `--server`).
    pub block_private: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            folder: home_dir().join("Downloads"),
            connections: 8,
            limit_on: false,
            limit_kbps: 512,
            per_category_folders: false,
            show_progress: true,
            show_complete: true,
            max_queue: 2,
            retries: 5,
            proxy_on: false,
            proxy: "http://127.0.0.1:8080".into(),
            watch_clipboard: true,
            browser_listen: true,
            browser_start_now: false,
            schedule: Schedule::default(),
            queues: vec![QueueDef::default()],
            categories: default_categories(),
            site_logins: Vec::new(),
            theme: ThemeMode::System,
            color_theme: ColorTheme::default(),
            view: layout::ViewMode::Cards,
            background: closing::BackgroundMode::Ask,
            launch_at_login: false,
            mcp_enabled: true,
            multi_network: false,
            networks: Vec::new(),
            web_enabled: false,
            web_port: 9615,
            block_private: false,
        }
    }
}

// ---------------------------------------------------------------- downloads

#[derive(Serialize, Deserialize)]
struct SavedItem {
    id: u64,
    url: String,
    name: String,
    folder: PathBuf,
    category: String,
    added: String,
    last_try: String,
    description: String,
    status: Status,
    total: u64,
    downloaded: u64,
    resumable: bool,
    segments: Vec<Segment>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    auto_name: bool,
    #[serde(default)]
    referer: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    pass: String,
    #[serde(default)]
    expected_sha: String,
    #[serde(default)]
    check: Option<String>,
    #[serde(default)]
    queue: Option<String>,
    #[serde(default)]
    cookies: String,
    #[serde(default)]
    user_agent: String,
    #[serde(default)]
    limit_kbps: u64,
    /// Was running when a window took the downloads over from the MCP engine: carry on.
    #[serde(default)]
    handoff_resume: bool,
    #[serde(default)]
    owner: String,
}

#[derive(Serialize, Deserialize, Default)]
struct SavedState {
    settings: Settings,
    next_id: u64,
    items: Vec<SavedItem>,
    /// Queues that were running at a hand-over.
    #[serde(default)]
    handoff_queues: Vec<String>,
}

pub struct Item {
    id: u64,
    url: String,
    name: String,
    folder: PathBuf,
    category: String,
    added: String,
    last_try: String,
    description: String,
    live: Arc<Live>,
    /// The name came from the link (the server may suggest a better one).
    auto_name: bool,
    referer: String,
    user: String,
    pass: String,
    /// SHA-256 the person expects; checked automatically when the download completes.
    expected_sha: String,
    /// Result of the last checksum check.
    check: Option<String>,
    checking: Arc<Mutex<Option<Result<String, String>>>>,
    is_checking: bool,
    /// Queue it waits in (None = not in a queue).
    queue: Option<String>,
    cookies: String,
    user_agent: String,
    /// This download's own speed limit (0 = none), chained to the overall limit.
    limit_kbps: u64,
    limiter: Arc<Limiter>,
    handoff_resume: bool,
    /// On a shared server: whose download this is (empty on a personal computer).
    owner: String,
    speed: f64,
    last_bytes: u64,
    last_status: Status,
    demo_speed: Option<f64>,
}

impl Item {
    fn new(id: u64, url: String, name: String, folder: PathBuf, category: String, live: Arc<Live>, overall: &Arc<Limiter>) -> Self {
        let last_status = live.status();
        let last_bytes = live.downloaded.load(Ordering::Relaxed);
        Item {
            id,
            url,
            name,
            folder,
            category,
            added: now_stamp(),
            last_try: String::new(),
            description: String::new(),
            live,
            auto_name: false,
            referer: String::new(),
            user: String::new(),
            pass: String::new(),
            expected_sha: String::new(),
            check: None,
            checking: Arc::new(Mutex::new(None)),
            is_checking: false,
            queue: None,
            cookies: String::new(),
            user_agent: String::new(),
            limit_kbps: 0,
            limiter: Arc::new(Limiter::chained(overall.clone())),
            handoff_resume: false,
            owner: String::new(),
            speed: 0.0,
            last_bytes,
            last_status,
            demo_speed: None,
        }
    }
    fn set_limit(&mut self, kbps: u64) {
        self.limit_kbps = kbps;
        self.limiter.bytes_per_sec.store(kbps * 1024, Ordering::Relaxed);
    }
    fn path(&self) -> PathBuf {
        self.folder.join(&self.name)
    }
    fn total(&self) -> u64 {
        self.live.total.load(Ordering::Relaxed)
    }
    fn done(&self) -> u64 {
        self.live.downloaded.load(Ordering::Relaxed)
    }
    fn status(&self) -> Status {
        self.live.status()
    }
    fn speed(&self) -> f64 {
        self.demo_speed.unwrap_or(self.speed)
    }
    fn progress(&self) -> f32 {
        let t = self.total();
        if t == 0 { 0.0 } else { (self.done() as f64 / t as f64) as f32 }
    }
    fn status_text(&self) -> String {
        match self.status() {
            Status::Downloading | Status::Paused if self.total() > 0 => format!("{:.2}%", self.progress() * 100.0),
            Status::Downloading => human_size(self.done()),
            s => s.label().into(),
        }
    }
    fn secs_left(&self) -> f64 {
        if self.status() != Status::Downloading || self.total() == 0 {
            return 0.0;
        }
        self.total().saturating_sub(self.done()) as f64 / self.speed().max(1.0)
    }
    fn time_left(&self) -> String {
        human_time(self.secs_left())
    }
}

// ---------------------------------------------------------------- dialogs' state

#[derive(Clone, PartialEq)]
enum Filter {
    All,
    Category(String),
    Unfinished,
    Finished,
    Queue(String),
}

#[derive(Clone, Copy, PartialEq)]
enum SortCol {
    Name,
    Size,
    Status,
    TimeLeft,
    Speed,
    LastTry,
    Description,
}

#[derive(Default)]
struct AddDialog {
    url: String,
    name: String,
    name_edited: bool,
    folder: String,
    category: String,
    description: String,
    referer: String,
    user: String,
    pass: String,
    expected_sha: String,
    cookies: String,
    user_agent: String,
    queue: String,
    error: Option<String>,
    focused: bool,
    from_outside: Option<String>,
}

#[derive(Default)]
struct BatchDialog {
    pattern_mode: bool,
    links: String,
    pattern: String,
    from: u32,
    to: u32,
    width: usize,
    folder: String,
    queue: String,
    error: Option<String>,
}

struct RefreshDialog {
    id: u64,
    url: String,
    error: Option<String>,
}

struct DeleteDialog {
    ids: Vec<u64>,
    delete_files: bool,
}

struct Shot {
    path: PathBuf,
    requested: bool,
    saved: Arc<AtomicBool>,
}

/// How the app was started (command line).
#[derive(Default)]
pub struct Launch {
    pub screenshot: Option<PathBuf>,
    pub demo: bool,
    pub start_url: Option<String>,
    /// Forces a theme for this run only.
    pub theme: Option<ThemeMode>,
    /// Forces a colour theme for this run only (`--color-theme`).
    pub color_theme: Option<ColorTheme>,
    /// Opens one window at start-up (screenshots).
    pub show: Option<String>,
    /// No window: the engine runs for an MCP client. No browser listener or clipboard watching.
    pub headless: bool,
    /// Where to keep settings and the download list (default: the user's config folder).
    pub config_dir: Option<PathBuf>,
}

pub struct App {
    items: Vec<Item>,
    next_id: u64,
    settings: Settings,
    limiter: Arc<Limiter>,
    filter: Filter,
    search: String,
    sort: Option<(SortCol, bool)>,
    selected: BTreeSet<u64>,
    add: Option<AddDialog>,
    batch: Option<BatchDialog>,
    progress_open: BTreeSet<u64>,
    details_open: BTreeSet<u64>,
    options: Option<Settings>,
    /// Scheduler window: which queue, and its schedule being edited.
    scheduler: Option<(String, Schedule)>,
    queues_open: bool,
    new_queue: String,
    refresh: Option<RefreshDialog>,
    grabber: Option<grabber::GrabDialog>,
    delete: Option<DeleteDialog>,
    properties: Option<u64>,
    completed: Vec<u64>,
    about_open: bool,
    about_icon: Option<egui::TextureHandle>,
    /// Narrow windows show the categories in a drawer that this opens.
    drawer_open: bool,
    browser_open: bool,
    show_categories: bool,
    /// Queues that are running now.
    running: BTreeSet<String>,
    demo: bool,
    shot: Option<Shot>,
    frame: u64,
    dirty: bool,
    last_save: Instant,
    last_sample: Instant,
    last_schedule_check: Instant,
    inbox: Inbox,
    clipboard_on: Arc<AtomicBool>,
    listener: Result<(), String>,
    toast: Option<(String, Instant)>,
    /// Forces a theme for this run only (`--theme`), without touching the saved choice.
    theme_override: Option<ThemeMode>,
    color_theme_override: Option<ColorTheme>,
    config_dir: PathBuf,
    /// Running without a window (MCP engine).
    headless: bool,
    /// Commands from `--mcp` processes (see `mcp::control`).
    control_rx: Option<std::sync::mpsc::Receiver<crate::mcp::control::Request>>,
    /// The web interface on this computer (Options > Web interface) and its requests.
    web: Option<crate::web::server::Handle>,
    web_rx: Option<std::sync::mpsc::Receiver<crate::mcp::control::Request>>,
    /// Owns the download list while this process runs (see `mcp::lock`).
    state_lock: Option<crate::mcp::lock::StateLock>,
    /// Queues that were running when the downloads were handed over; restarted on load.
    handoff_queues: Vec<String>,
    /// When an AI assistant last used this app (for the status bar).
    last_ai_call: Option<Instant>,
    ai_open: bool,
    /// The window is really closing (not going to the background or asking).
    quitting: bool,
    /// The "keep downloading?" question is showing.
    ask_close: bool,
    /// `--show menus`: draw the rows of some menus on the page, for screenshots (menus are pop-ups, which a screenshot cannot open).
    menu_preview: bool,
    remember_close: bool,
    ctx: egui::Context,
}

// ---------------------------------------------------------------- setup & storage

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self {
        // The bundled fonts are ready before the first frame, so nothing is ever drawn in a stand-in font.
        theme::install_fonts(&cc.egui_ctx);
        Self::build(cc.egui_ctx.clone(), launch)
    }

    /// An app with no window, for `--mcp` when nothing else owns the downloads.
    pub fn headless_in(config_dir: PathBuf) -> Self {
        Self::build(egui::Context::default(), Launch { headless: true, config_dir: Some(config_dir), ..Default::default() })
    }

    pub(crate) fn build(ctx: egui::Context, launch: Launch) -> Self {
        let Launch { screenshot, demo, start_url, theme: theme_override, color_theme: color_theme_override, show, headless, config_dir } = launch;
        let mut app = App {
            items: Vec::new(),
            next_id: 1,
            settings: Settings::default(),
            limiter: Arc::new(Limiter::new()),
            filter: Filter::All,
            search: String::new(),
            sort: None,
            selected: BTreeSet::new(),
            add: None,
            batch: None,
            progress_open: BTreeSet::new(),
            details_open: BTreeSet::new(),
            options: None,
            scheduler: None,
            queues_open: false,
            new_queue: String::new(),
            refresh: None,
            grabber: None,
            delete: None,
            properties: None,
            completed: Vec::new(),
            about_open: false,
            about_icon: None,
            drawer_open: false,
            quitting: false,
            ask_close: false,
            menu_preview: false,
            remember_close: false,
            browser_open: false,
            show_categories: true,
            running: BTreeSet::new(),
            demo,
            shot: screenshot.map(|path| Shot { path, requested: false, saved: Arc::new(AtomicBool::new(false)) }),
            frame: 0,
            dirty: false,
            last_save: Instant::now(),
            last_sample: Instant::now(),
            last_schedule_check: Instant::now(),
            inbox: Arc::new(Mutex::new(Vec::new())),
            clipboard_on: Arc::new(AtomicBool::new(false)),
            // The sample window (--demo, screenshots) shows the add-on as connected.
            listener: if demo { Ok(()) } else { Err("off".into()) },
            toast: None,
            theme_override,
            color_theme_override,
            config_dir: config_dir.unwrap_or_else(default_config_dir),
            headless,
            control_rx: None,
            web: None,
            web_rx: None,
            state_lock: None,
            handoff_queues: Vec::new(),
            last_ai_call: None,
            ai_open: false,
            ctx,
        };
        if demo {
            app.load_demo();
        } else {
            // The lock comes first: a windowless engine may have to save before we read.
            if !headless {
                app.take_state_lock();
            }
            app.load();
            if !headless {
                app.settings.launch_at_login = crate::autostart::is_enabled();
            }
            app.resume_handed_over();
            if !headless {
                if app.state_lock.is_some() {
                    app.start_control();
                    app.sync_web();
                } else {
                    app.toast("AI assistants cannot control the downloads: another copy of the app owns them");
                }
                if app.settings.browser_listen {
                    app.listener = bridge::start_listener(bridge::PORT, app.inbox.clone(), app.ctx.clone());
                }
                bridge::start_clipboard_watch(app.clipboard_on.clone(), app.inbox.clone(), app.ctx.clone());
            }
        }
        if let Some(url) = start_url {
            app.inbox.lock().unwrap().push(Incoming {
                url,
                name: String::new(),
                referer: String::new(),
                cookies: String::new(),
                user_agent: String::new(),
                source: bridge::Source::CommandLine,
            });
        }
        app.apply_settings();
        if let Some(name) = show {
            app.show_window(&name);
        }
        app
    }

    /// Open one window by name, with sample data (used by `--show`).
    fn show_window(&mut self, name: &str) {
        match name {
            "add" => self.open_add_dialog(None),
            "batch" => self.open_batch(),
            "options" | "networks" | "appearance" => {
                if name == "networks" {
                    self.ctx.data_mut(|d| d.insert_temp(egui::Id::unique("sdm_scroll_networks"), true));
                }
                if name == "appearance" {
                    self.ctx.data_mut(|d| d.insert_temp(egui::Id::unique("sdm_scroll_appearance"), true));
                }
                let mut o = self.settings.clone();
                if self.demo {
                    // The picture shows the Networks card filled in.
                    o.multi_network = true;
                    o.networks = vec!["Wi-Fi".into(), "Ethernet".into(), "Phone hotspot".into()];
                    self.ctx.data_mut(|d| d.insert_temp(egui::Id::unique("sdm_demo_networks"), true));
                }
                self.options = Some(o);
            }
            "scheduler" => self.open_scheduler(MAIN_QUEUE),
            "queues" => self.queues_open = true,
            "delete" => self.delete = Some(DeleteDialog { ids: vec![2], delete_files: false }),
            "properties" => self.properties = Some(2),
            "progress" => {
                self.progress_open.insert(1);
            }
            "about" => self.about_open = true,
            "browser" => self.browser_open = true,
            "ai" => self.ai_open = true,
            "drawer" => self.drawer_open = true,
            "close" => self.ask_close = true,
            "menus" => {
                self.menu_preview = true;
                self.toast("Address copied");
            }
            "table" => self.settings.view = layout::ViewMode::Table,
            "grabber" => self.open_grabber(),
            // The window with no downloads at all, for the picture of the empty state.
            "empty" => {
                self.items.clear();
                self.selected.clear();
            }
            other => eprintln!("unknown window: {other}"),
        }
    }

    fn state_file(&self) -> PathBuf {
        self.config_dir.join("downloads.json")
    }

    fn load(&mut self) {
        let Ok(text) = std::fs::read_to_string(self.state_file()) else { return };
        let Ok(state) = serde_json::from_str::<SavedState>(&text) else { return };
        self.settings = state.settings;
        self.handoff_queues = state.handoff_queues;
        self.next_id = state.next_id.max(1);
        for s in state.items {
            // Anything that was running when the app closed is now paused.
            let status = if s.status.is_active() { Status::Paused } else { s.status };
            let live = Live::new(s.total, s.downloaded, status, s.segments, s.resumable);
            *live.error.lock().unwrap() = s.error;
            let mut item = Item::new(s.id, s.url, s.name, s.folder, s.category, live, &self.limiter);
            item.added = s.added;
            item.last_try = s.last_try;
            item.description = s.description;
            item.auto_name = s.auto_name;
            item.referer = s.referer;
            item.user = s.user;
            item.pass = s.pass;
            item.expected_sha = s.expected_sha;
            item.check = s.check;
            item.queue = s.queue.or_else(|| (status == Status::Queued).then(|| MAIN_QUEUE.to_string()));
            item.cookies = s.cookies;
            item.user_agent = s.user_agent;
            item.set_limit(s.limit_kbps);
            item.handoff_resume = s.handoff_resume;
            item.owner = s.owner;
            self.items.push(item);
        }
        self.fix_settings();
    }

    /// Make sure there is a main queue and a "General" category, and carry over older settings.
    fn fix_settings(&mut self) {
        let st = &mut self.settings;
        if !st.queues.iter().any(|q| q.name == MAIN_QUEUE) {
            st.queues.insert(0, QueueDef { name: MAIN_QUEUE.into(), max_parallel: st.max_queue.max(1), schedule: st.schedule.clone() });
        }
        if st.schedule.start_on || st.schedule.stop_on {
            if let Some(main) = st.queues.iter_mut().find(|q| q.name == MAIN_QUEUE) {
                main.schedule = st.schedule.clone();
            }
            st.schedule = Schedule::default();
        }
        st.categories.retain(|c| !c.name.trim().is_empty());
        if st.categories.is_empty() {
            st.categories = default_categories();
        }
        if !st.categories.iter().any(|c| c.name == "General") {
            st.categories.push(CategoryDef { name: "General".into(), exts: String::new(), folder: String::new() });
        }
    }

    fn cat_of(&self, name: &str) -> String {
        category_for(&self.settings.categories, name)
    }

    fn save(&mut self) {
        self.dirty = false;
        self.last_save = Instant::now();
        if self.demo {
            return;
        }
        let items = self
            .items
            .iter()
            .map(|i| SavedItem {
                id: i.id,
                url: i.url.clone(),
                name: i.name.clone(),
                folder: i.folder.clone(),
                category: i.category.clone(),
                added: i.added.clone(),
                last_try: i.last_try.clone(),
                description: i.description.clone(),
                status: i.status(),
                total: i.total(),
                downloaded: i.done(),
                resumable: i.live.resumable.load(Ordering::Relaxed),
                segments: i.live.segments.lock().unwrap().clone(),
                error: i.live.error.lock().unwrap().clone(),
                auto_name: i.auto_name,
                referer: i.referer.clone(),
                user: i.user.clone(),
                pass: i.pass.clone(),
                expected_sha: i.expected_sha.clone(),
                check: i.check.clone(),
                queue: i.queue.clone(),
                cookies: i.cookies.clone(),
                user_agent: i.user_agent.clone(),
                limit_kbps: i.limit_kbps,
                handoff_resume: i.handoff_resume,
                owner: i.owner.clone(),
            })
            .collect();
        let state = SavedState { settings: self.settings.clone(), next_id: self.next_id, items, handoff_queues: self.handoff_queues.clone() };
        let path = self.state_file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(&state) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(tmp, path);
            }
        }
    }

    /// Sample rows for `--demo` / `--screenshot`, so the window shows its whole layout.
    /// Nothing is downloaded or saved in this mode.
    fn load_demo(&mut self) {
        let rows: [(&str, u64, f64, Status, Option<f64>, &str); 9] = [
            ("ubuntu-24.04-desktop-amd64.iso", 6_114_656_256, 0.4123, Status::Downloading, Some(4_620_000.0), "Sample: OS image"),
            ("project-report-final.pdf", 2_457_600, 1.0, Status::Complete, None, "SHA-256 matches"),
            ("holiday-video-4k.mp4", 1_288_490_188, 0.7342, Status::Downloading, Some(2_150_000.0), ""),
            ("podcast-episode-112.mp3", 58_720_256, 1.0, Status::Complete, None, ""),
            ("setup-tools-2.3.1.exe", 94_371_840, 0.2611, Status::Paused, None, ""),
            ("photos-backup.zip", 734_003_200, 0.0, Status::Queued, None, "Queued for tonight"),
            ("dataset-sample.csv", 18_874_368, 1.0, Status::Complete, None, ""),
            ("library-docs.tar.gz", 41_943_040, 0.0, Status::Queued, None, ""),
            ("conference-talk.webm", 312_475_648, 0.0, Status::Error, None, ""),
        ];
        for (i, (name, total, p, st, speed, desc)) in rows.iter().enumerate() {
            let done = (*total as f64 * p) as u64;
            let segs = if matches!(st, Status::Downloading | Status::Paused) {
                let n = 8u64;
                let size = total / n;
                (0..n)
                    .map(|k| {
                        let start = k * size;
                        let end = if k == n - 1 { total - 1 } else { (k + 1) * size - 1 };
                        let frac = (p + (k as f64 * 0.37).sin() * 0.25).clamp(0.05, 1.0);
                        Segment { start, end, done: ((end - start) as f64 * frac) as u64 }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let live = Live::new(*total, done, *st, segs, true);
            if *st == Status::Error {
                *live.error.lock().unwrap() = Some("Server answered 404".into());
            }
            let cat = self.cat_of(name);
            let mut item = Item::new(i as u64 + 1, format!("https://example.com/files/{name}"), name.to_string(), self.settings.folder.clone(), cat, live, &self.limiter);
            if *st == Status::Queued {
                item.queue = Some(MAIN_QUEUE.into());
            }
            item.added = "Nov 02 2026 09:14".into();
            item.last_try = if *st == Status::Queued { String::new() } else { "Nov 02 2026 21:06".into() };
            item.description = desc.to_string();
            item.demo_speed = *speed;
            self.items.push(item);
        }
        self.next_id = self.items.len() as u64 + 1;
        self.selected.insert(1);
        self.settings.queues[0].schedule.start_on = true;
        self.settings.queues.push(QueueDef { name: "Night".into(), ..Default::default() });
    }

    fn apply_settings(&mut self) {
        let v = if self.settings.limit_on { self.settings.limit_kbps.max(1) * 1024 } else { 0 };
        self.limiter.bytes_per_sec.store(v, Ordering::Relaxed);
        self.clipboard_on.store(self.settings.watch_clipboard && !self.demo, Ordering::Relaxed);
        self.sync_web();
    }

    fn folder_for(&self, category: &str) -> PathBuf {
        if let Some(c) = self.settings.categories.iter().find(|c| c.name == category && !c.folder.trim().is_empty()) {
            return PathBuf::from(c.folder.trim());
        }
        if self.settings.per_category_folders { self.settings.folder.join(category) } else { self.settings.folder.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_without_theme_still_load_and_follow_the_system() {
        let s: Settings = serde_json::from_str(r#"{"connections":4}"#).unwrap();
        assert_eq!(s.theme, crate::theme::ThemeMode::System, "a new install follows the system's light or dark");
        assert_eq!(s.connections, 4);
    }

    #[test]
    fn a_saved_mode_is_kept() {
        let s: Settings = serde_json::from_str(r#"{"theme":"Light"}"#).unwrap();
        assert_eq!(s.theme, crate::theme::ThemeMode::Light);
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.theme, s.theme);
    }

    #[test]
    fn settings_saved_by_the_two_design_versions_still_load() {
        // Versions 1.3 to 1.6 stored a design name ("Drishti" or "Ios"). It is ignored now, and the
        // rest of the file (above all the download list) must still be read.
        for old in [r#"{"theme":"Dark","theme_style":"Drishti","connections":6}"#, r#"{"theme":"Dark","theme_style":"Ios","connections":6}"#] {
            let s: Settings = serde_json::from_str(old).unwrap();
            assert_eq!((s.theme, s.connections), (crate::theme::ThemeMode::Dark, 6), "{old}");
        }
        let state: SavedState = serde_json::from_str(r#"{"settings":{"theme_style":"Ios"},"next_id":5,"items":[]}"#).unwrap();
        assert_eq!(state.next_id, 5);
    }

    #[test]
    fn old_settings_get_the_card_view_and_a_saved_table_view_is_kept() {
        let s: Settings = serde_json::from_str(r#"{"connections":4}"#).unwrap();
        assert_eq!(s.view, layout::ViewMode::Cards);
        let s: Settings = serde_json::from_str(r#"{"view":"Table"}"#).unwrap();
        assert_eq!(s.view, layout::ViewMode::Table);
    }
}
