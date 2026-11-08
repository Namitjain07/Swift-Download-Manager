//! Ways for links to reach the app from outside the window:
//! - a tiny local web listener (127.0.0.1 only) used by the browser add-on
//!   and by starting the program with a link while it is already open;
//! - watching the clipboard for copied download links.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

pub const PORT: u16 = 9614;

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Browser,
    Clipboard,
    CommandLine,
}

#[derive(Clone, Debug)]
pub struct Incoming {
    pub url: String,
    pub name: String,
    pub referer: String,
    /// Browser cookies for the site, so downloads behind a login work.
    pub cookies: String,
    pub user_agent: String,
    pub source: Source,
}

pub type Inbox = Arc<Mutex<Vec<Incoming>>>;

/// File endings the clipboard watcher treats as downloads.
pub const DOWNLOAD_EXTS: &[&str] = &[
    "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "tgz", "zst", "iso", "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "epub",
    "mp3", "wav", "flac", "ogg", "m4a", "aac", "exe", "msi", "deb", "rpm", "appimage", "dmg", "apk", "pkg", "bin", "mp4", "mkv",
    "avi", "mov", "webm", "wmv", "flv", "m4v", "mpg", "mpeg", "img", "torrent",
];

pub fn looks_like_download(text: &str) -> bool {
    let t = text.trim();
    if t.contains(char::is_whitespace) || !(t.starts_with("http://") || t.starts_with("https://")) {
        return false;
    }
    let Ok(u) = url::Url::parse(t) else { return false };
    let path = u.path().to_ascii_lowercase();
    let ext = path.rsplit('/').next().unwrap_or("").rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    DOWNLOAD_EXTS.contains(&ext)
}

fn query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| crate::engine::percent_decode(&v.replace('+', " ")))
    })
}

pub fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// What the listener shares with the app besides the inbox: a snapshot of the downloads for the pop-up of the add-on,
/// and the commands the add-on sends (pause, resume, open the window).
#[derive(Clone, Default)]
pub struct Link {
    /// The status as JSON text, rebuilt by the app a few times a second while something runs.
    pub status: Arc<Mutex<String>>,
    pub commands: Arc<Mutex<Vec<Command>>>,
}

/// What the pop-up of the add-on can ask the app to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Pause(u64),
    Resume(u64),
    PauseAll,
    ResumeAll,
    /// Bring the window to the front (or start one).
    Open,
}

/// Whether a request may be answered. A page on the web must never be able to reach this listener (it could make the
/// program fetch links, or read what is downloading), so a request is only answered when it comes from the add-on
/// (its address is an extension's), or from a program that sends none of the headers a browser adds (this program
/// itself, curl). A browser tells where a request comes from in `Origin` and `Sec-Fetch-Site`; `Host` must be this
/// computer, which also shuts out DNS rebinding (a web page whose name was pointed at 127.0.0.1).
pub fn allowed(host: Option<&str>, origin: Option<&str>, fetch_site: Option<&str>) -> bool {
    if let Some(h) = host {
        let name = h.rsplit_once(':').filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit())).map_or(h, |(n, _)| n);
        if !matches!(name, "127.0.0.1" | "localhost" | "[::1]") {
            return false;
        }
    }
    match origin {
        Some(o) => ["chrome-extension://", "moz-extension://", "safari-web-extension://"].iter().any(|p| o.starts_with(p)),
        None => fetch_site.is_none_or(|s| s == "none"),
    }
}


/// What the add-on's door is doing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListenerStatus {
    /// Not started: switched off, or this process has no door to keep.
    Off,
    /// Started, but something else has the port for now (the window that is closing, or the background process that is
    /// handing over). It keeps trying and takes the port by itself as soon as it is free. Holds what the system said.
    Waiting(String),
    Listening,
}

struct Shared {
    stop: AtomicBool,
    status: Mutex<ListenerStatus>,
    /// The thread that accepts has let go of the port.
    closed: AtomicBool,
    /// Requests being answered right now.
    open: AtomicUsize,
}

impl Shared {
    fn status(&self) -> MutexGuard<'_, ListenerStatus> {
        self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }
}

/// The local door of the add-on (127.0.0.1 only). One process owns it at a time; the window and the background process
/// pass it to each other, so a door that finds the port taken waits for it instead of giving up for good, and a door
/// that is dropped gives the port back at once (the next owner starts a moment later, and the add-on tries again for a
/// few seconds, see `browser-extension/background.js`).
pub struct Listener {
    port: u16,
    shared: Arc<Shared>,
}

/// A request that is being answered: counted while it lives, even when its thread panics.
struct Slot(Arc<Shared>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.open.fetch_sub(1, Ordering::AcqRel);
    }
}

const MAX_CONNECTIONS: usize = 32;

impl Listener {
    fn with_status(port: u16, status: ListenerStatus, closed: bool) -> Self {
        Listener { port, shared: Arc::new(Shared { stop: AtomicBool::new(false), status: Mutex::new(status), closed: AtomicBool::new(closed), open: AtomicUsize::new(0) }) }
    }

    /// No door (switched off, or not started yet).
    pub fn off() -> Self {
        Self::with_status(0, ListenerStatus::Off, true)
    }

    /// Says it is listening without doing so (the sample window of the screenshots).
    pub fn pretend() -> Self {
        Self::with_status(0, ListenerStatus::Listening, true)
    }

    /// Open the door. Returns at once: when the port is taken, the status says so and a thread tries again.
    pub fn start(port: u16, inbox: Inbox, link: Link, ctx: eframe::egui::Context) -> Self {
        // The first try is made here, so a free port is reported as listening from the very first frame.
        let first = TcpListener::bind(("127.0.0.1", port));
        let status = match &first {
            Ok(_) => ListenerStatus::Listening,
            Err(e) => ListenerStatus::Waiting(e.to_string()),
        };
        let this = Self::with_status(port, status, false);
        let shared = this.shared.clone();
        let spawned = thread::Builder::new().name("sdm-bridge".into()).spawn({
            let shared = shared.clone();
            move || {
                if let Some(listener) = first.ok().or_else(|| wait_for_port(port, &shared)) {
                    if !shared.stopping() {
                        *shared.status() = ListenerStatus::Listening;
                        accept_loop(&listener, &shared, &inbox, &link, &ctx);
                    }
                    drop(listener);
                }
                shared.closed.store(true, Ordering::Release);
            }
        });
        if let Err(e) = spawned {
            *shared.status() = ListenerStatus::Waiting(format!("could not start a thread: {e}"));
            shared.closed.store(true, Ordering::Release);
        }
        this
    }

    pub fn status(&self) -> ListenerStatus {
        self.shared.status().clone()
    }

    /// Started (listening, or waiting for the port).
    pub fn is_on(&self) -> bool {
        *self.shared.status() != ListenerStatus::Off
    }

    pub fn is_listening(&self) -> bool {
        *self.shared.status() == ListenerStatus::Listening
    }

    /// Close the door and give the port back: it can be bound again as soon as this returns. Requests that were already
    /// being answered are given a moment to finish, so a link that was accepted is in the inbox when this returns.
    pub fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        let until = Instant::now() + Duration::from_secs(1);
        // `accept` blocks, so the thread is woken with a connection of our own (repeated, in case it was not
        // blocking yet the first time).
        while !self.shared.closed.load(Ordering::Acquire) && Instant::now() < until {
            if self.is_listening() {
                let _ = TcpStream::connect_timeout(&([127, 0, 0, 1], self.port).into(), Duration::from_millis(100));
            }
            thread::sleep(Duration::from_millis(5));
        }
        let handlers_until = Instant::now() + Duration::from_millis(300);
        while self.shared.open.load(Ordering::Acquire) > 0 && Instant::now() < handlers_until {
            thread::sleep(Duration::from_millis(5));
        }
        *self.shared.status() = ListenerStatus::Off;
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Sleep, but wake up as soon as the listener is told to stop.
fn sleep_unless_stopped(shared: &Shared, total: Duration) {
    let until = Instant::now() + total;
    while !shared.stopping() && Instant::now() < until {
        thread::sleep(Duration::from_millis(20));
    }
}

/// The port is taken: try again, quickly at first (a closing window lets go within a moment), then once a second for as
/// long as this process runs, so the door opens by itself whenever the other copy is gone. None when told to stop.
fn wait_for_port(port: u16, shared: &Shared) -> Option<TcpListener> {
    let started = Instant::now();
    loop {
        sleep_unless_stopped(shared, if started.elapsed() < Duration::from_secs(10) { Duration::from_millis(100) } else { Duration::from_secs(1) });
        if shared.stopping() {
            return None;
        }
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(l) => return Some(l),
            Err(e) => *shared.status() = ListenerStatus::Waiting(e.to_string()),
        }
    }
}

/// Answer every request on a thread of its own, so a client that connects and says nothing (a browser warming up a
/// connection) cannot keep the add-on waiting.
fn accept_loop(listener: &TcpListener, shared: &Arc<Shared>, inbox: &Inbox, link: &Link, ctx: &eframe::egui::Context) {
    for stream in listener.incoming() {
        if shared.stopping() {
            break;
        }
        let Ok(stream) = stream else {
            thread::sleep(Duration::from_millis(50));
            continue;
        };
        if shared.open.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            shared.open.fetch_sub(1, Ordering::AcqRel);
            continue;
        }
        let slot = Slot(shared.clone());
        let (inbox, link, ctx) = (inbox.clone(), link.clone(), ctx.clone());
        // When no thread can be made the closure is dropped with its slot, which gives the count back.
        let _ = thread::Builder::new().name("sdm-bridge-request".into()).spawn(move || {
            let _slot = slot;
            let _ = handle(stream, &inbox, &link, &ctx);
        });
    }
}

fn push(inbox: &Inbox, query: &str, url: String, src: Source) {
    inbox.lock().unwrap().push(Incoming {
        url,
        name: query_value(query, "name").unwrap_or_default(),
        referer: query_value(query, "referer").unwrap_or_default(),
        cookies: query_value(query, "cookies").unwrap_or_default(),
        user_agent: query_value(query, "ua").unwrap_or_default(),
        source: src,
    });
}

fn is_web_link(u: &str) -> bool {
    u.starts_with("http://") || u.starts_with("https://")
}

fn handle(mut s: TcpStream, inbox: &Inbox, link: &Link, ctx: &eframe::egui::Context) -> std::io::Result<()> {
    s.set_read_timeout(Some(Duration::from_secs(3)))?;
    let mut reader = BufReader::new(s.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let (mut len, mut host, mut origin, mut fetch_site) = (0usize, None, None, None);
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" {
            break;
        }
        let lower = h.to_ascii_lowercase();
        let value = |name: &str| lower.starts_with(name).then(|| h[name.len()..].trim().to_string());
        if let Some(v) = lower.strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0).min(256 * 1024);
        } else if let Some(v) = value("host:") {
            host = Some(v);
        } else if let Some(v) = value("origin:") {
            origin = Some(v);
        } else if let Some(v) = value("sec-fetch-site:") {
            fetch_site = Some(v.to_ascii_lowercase());
        }
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    let respond = |s: &mut TcpStream, code: &str, kind: &str, text: &str, cors: Option<&str>| {
        let cors = cors.map(|o| format!("Access-Control-Allow-Origin: {o}\r\nVary: Origin\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n")).unwrap_or_default();
        let resp = format!("HTTP/1.1 {code}\r\nContent-Type: {kind}\r\n{cors}Cache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
        s.write_all(resp.as_bytes())
    };
    if !allowed(host.as_deref(), origin.as_deref(), fetch_site.as_deref()) {
        return respond(&mut s, "403 Forbidden", "text/plain", "forbidden", None);
    }
    let cors = origin.as_deref();
    let (path, mut query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.clone(), String::new()),
    };
    if method == "POST" && !body.is_empty() {
        query = String::from_utf8_lossy(&body).into_owned();
    }
    let (code, kind, text) = match (method.as_str(), path.as_str()) {
        ("OPTIONS", _) => ("204 No Content", "text/plain", String::new()),
        ("GET", "/ping") => ("200 OK", "text/plain", "swift-download-manager".to_string()),
        // What the pop-up of the add-on shows: the running downloads.
        ("GET", "/status") => {
            // An idle window repaints only every 15 seconds, and the snapshot is made while it draws: asking wakes it, so the
            // next answer (the pop-up asks once a second) is fresh.
            ctx.request_repaint();
            let json = link.status.lock().unwrap().clone();
            ("200 OK", "application/json", if json.is_empty() { "{}".to_string() } else { json })
        }
        ("GET" | "POST", "/add") => match query_value(&query, "url") {
            Some(u) if is_web_link(&u) => {
                let src = if query_value(&query, "from").as_deref() == Some("cli") { Source::CommandLine } else { Source::Browser };
                push(inbox, &query, u, src);
                ctx.request_repaint();
                ("200 OK", "text/plain", "added".to_string())
            }
            _ => ("400 Bad Request", "text/plain", "missing url".to_string()),
        },
        // Several links at once ("urls": one per line), with the cookies and referrer of the page they were found on.
        ("POST", "/add-many") => {
            let urls: Vec<String> = query_value(&query, "urls").unwrap_or_default().lines().map(str::trim).filter(|u| is_web_link(u)).take(500).map(str::to_string).collect();
            for u in &urls {
                push(inbox, &query, u.clone(), Source::Browser);
            }
            ctx.request_repaint();
            if urls.is_empty() { ("400 Bad Request", "text/plain", "no links".to_string()) } else { ("200 OK", "text/plain", format!("added {}", urls.len())) }
        }
        ("POST", "/control") => {
            let id = query_value(&query, "id").and_then(|v| v.parse::<u64>().ok());
            let cmd = match (query_value(&query, "cmd").as_deref(), id) {
                (Some("pause"), Some(id)) => Some(Command::Pause(id)),
                (Some("resume"), Some(id)) => Some(Command::Resume(id)),
                (Some("pause-all"), _) => Some(Command::PauseAll),
                (Some("resume-all"), _) => Some(Command::ResumeAll),
                (Some("open"), _) => Some(Command::Open),
                _ => None,
            };
            match cmd {
                Some(c) => {
                    link.commands.lock().unwrap().push(c);
                    ctx.request_repaint();
                    ("200 OK", "text/plain", "done".to_string())
                }
                None => ("400 Bad Request", "text/plain", "unknown command".to_string()),
            }
        }
        _ => ("404 Not Found", "text/plain", "unknown".to_string()),
    };
    respond(&mut s, code, kind, &text, cors)
}

/// Hand a link to an already running copy of the app. True when it accepted it.
pub fn send_to_running(port: u16, url: &str) -> bool {
    let Ok(mut s) = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(400)) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let req = format!("GET /add?from=cli&url={} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n", url_encode(url));
    if s.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut resp = String::new();
    let _ = s.read_to_string(&mut resp);
    resp.starts_with("HTTP/1.1 200")
}

/// Watch the clipboard once a second while `enabled` is on.
pub fn start_clipboard_watch(enabled: Arc<AtomicBool>, inbox: Inbox, ctx: eframe::egui::Context) {
    thread::spawn(move || {
        let mut board: Option<arboard::Clipboard> = None;
        let mut last = String::new();
        let mut first = true;
        loop {
            thread::sleep(Duration::from_millis(1000));
            if !enabled.load(Ordering::Relaxed) {
                first = true;
                continue;
            }
            if board.is_none() {
                board = arboard::Clipboard::new().ok();
            }
            let Some(b) = board.as_mut() else { continue };
            let text = b.get_text().unwrap_or_default();
            if text == last {
                continue;
            }
            last = text.clone();
            // Ignore whatever was already copied when watching started.
            if first {
                first = false;
                continue;
            }
            if looks_like_download(&text) {
                inbox.lock().unwrap().push(Incoming {
                    url: text.trim().to_string(),
                    name: String::new(),
                    referer: String::new(),
                    cookies: String::new(),
                    user_agent: String::new(),
                    source: Source::Clipboard,
                });
                ctx.request_repaint();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_detection() {
        assert!(looks_like_download("https://example.com/files/setup.exe"));
        assert!(looks_like_download(" https://example.com/a/b/movie.MKV?token=1 "));
        assert!(!looks_like_download("https://example.com/page.html"));
        assert!(!looks_like_download("hello https://example.com/a.zip"));
        assert!(!looks_like_download("ftp://example.com/a.zip"));
    }

    #[test]
    fn listener_accepts_links_from_browser_and_second_copy() {
        let inbox: Inbox = Arc::new(Mutex::new(Vec::new()));
        let port = 39614;
        let door = Listener::start(port, inbox.clone(), Link::default(), eframe::egui::Context::default());
        assert_eq!(door.status(), ListenerStatus::Listening);
        // A second copy of the app hands its link over.
        assert!(send_to_running(port, "https://example.com/a file.zip"));
        // The browser add-on posts a form.
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let body = "url=https%3A%2F%2Fexample.com%2Fv.mp4&referer=https%3A%2F%2Fexample.com%2F&name=v.mp4&cookies=sid%3D1%3B+a%3Db&ua=Mozilla%2F5.0";
        write!(s, "POST /add HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).unwrap();
        assert!(resp.contains("200 OK"));
        let got = inbox.lock().unwrap().clone();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].url, "https://example.com/a file.zip");
        assert_eq!(got[0].source, Source::CommandLine);
        assert_eq!(got[1].url, "https://example.com/v.mp4");
        assert_eq!(got[1].referer, "https://example.com/");
        assert_eq!(got[1].source, Source::Browser);
        assert_eq!(got[1].cookies, "sid=1; a=b");
        assert_eq!(got[1].user_agent, "Mozilla/5.0");
        // Nothing listens on another port.
        assert!(!send_to_running(39615, "https://example.com/x.zip"));
    }
}

#[cfg(test)]
mod access_tests {
    use super::*;

    /// One request with the headers a browser (or not) would send; returns the whole answer.
    fn ask(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let mut req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
        for (k, v) in headers {
            req.push_str(&format!("{k}: {v}\r\n"));
        }
        req.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).unwrap();
        resp
    }

    #[test]
    fn only_the_add_on_and_plain_programs_get_an_answer() {
        // A program that sends no browser headers (this one, curl).
        assert!(allowed(Some("127.0.0.1:9614"), None, None));
        assert!(allowed(Some("localhost:9614"), None, Some("none")));
        assert!(allowed(None, None, None));
        // The add-on.
        assert!(allowed(Some("127.0.0.1:9614"), Some("chrome-extension://abcdefghijklmnop"), Some("none")));
        assert!(allowed(Some("127.0.0.1:9614"), Some("moz-extension://1234-5678"), Some("cross-site")));
        // A web page: by its origin, or by what the browser says about the request (an image tag has no origin).
        assert!(!allowed(Some("127.0.0.1:9614"), Some("https://evil.example"), Some("cross-site")));
        assert!(!allowed(Some("127.0.0.1:9614"), Some("null"), None));
        assert!(!allowed(Some("127.0.0.1:9614"), None, Some("cross-site")), "<img src=\"http://127.0.0.1:9614/add?url=...\">");
        assert!(!allowed(Some("127.0.0.1:9614"), None, Some("same-site")));
        // A name that was pointed at this computer (DNS rebinding) is not this computer.
        assert!(!allowed(Some("evil.example:9614"), None, None));
        assert!(!allowed(Some("127.0.0.1.evil.example"), Some("chrome-extension://x"), None));
    }

    #[test]
    fn the_add_on_gets_the_status_and_can_send_many_links_and_commands() {
        let inbox: Inbox = Arc::new(Mutex::new(Vec::new()));
        let link = Link::default();
        *link.status.lock().unwrap() = r#"{"active":2}"#.to_string();
        let port = 39616;
        let _door = Listener::start(port, inbox.clone(), link.clone(), eframe::egui::Context::default());
        let ext = [("Origin", "chrome-extension://abc"), ("Sec-Fetch-Site", "none")];

        // The pop-up asks what is going on: it is told, and the answer carries the extension's origin back.
        let r = ask(port, "GET", "/status", &ext, "");
        assert!(r.contains("200 OK") && r.contains(r#"{"active":2}"#) && r.contains("Access-Control-Allow-Origin: chrome-extension://abc"), "{r}");
        // A web page is refused, and told nothing about CORS, so it cannot read anything.
        let r = ask(port, "GET", "/status", &[("Origin", "https://evil.example"), ("Sec-Fetch-Site", "cross-site")], "");
        assert!(r.contains("403 Forbidden") && !r.contains("Access-Control-Allow-Origin") && !r.contains("active"), "{r}");
        let r = ask(port, "GET", "/add?url=https%3A%2F%2Fevil.example%2Fa.zip", &[("Sec-Fetch-Site", "cross-site")], "");
        assert!(r.contains("403"), "{r}");
        assert!(inbox.lock().unwrap().is_empty(), "nothing got in");

        // Several links with the page's cookies; lines that are not web links are dropped.
        let body = "urls=https%3A%2F%2Fa.example%2F1.zip%0Aftp%3A%2F%2Fno%0Ahttps%3A%2F%2Fa.example%2F2.zip&cookies=s%3D1&referer=https%3A%2F%2Fa.example%2F";
        let r = ask(port, "POST", "/add-many", &ext, body);
        assert!(r.contains("added 2"), "{r}");
        let got = inbox.lock().unwrap().clone();
        assert_eq!(got.iter().map(|i| i.url.as_str()).collect::<Vec<_>>(), ["https://a.example/1.zip", "https://a.example/2.zip"]);
        assert!(got.iter().all(|i| i.cookies == "s=1" && i.referer == "https://a.example/" && i.source == Source::Browser));
        assert!(ask(port, "POST", "/add-many", &ext, "urls=").contains("400"));

        // Commands for the window.
        assert!(ask(port, "POST", "/control", &ext, "cmd=pause&id=7").contains("200 OK"));
        assert!(ask(port, "POST", "/control", &ext, "cmd=resume-all").contains("200 OK"));
        assert!(ask(port, "POST", "/control", &ext, "cmd=open").contains("200 OK"));
        assert!(ask(port, "POST", "/control", &ext, "cmd=pause").contains("400"), "a pause without a download");
        assert!(ask(port, "POST", "/control", &ext, "cmd=format-disk").contains("400"));
        assert_eq!(*link.commands.lock().unwrap(), vec![Command::Pause(7), Command::ResumeAll, Command::Open]);
    }
}

#[cfg(test)]
mod door_tests {
    use super::*;

    fn free_port() -> u16 {
        TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port()
    }

    fn new_inbox() -> Inbox {
        Arc::new(Mutex::new(Vec::new()))
    }

    fn open(port: u16, inbox: &Inbox) -> Listener {
        Listener::start(port, inbox.clone(), Link::default(), eframe::egui::Context::default())
    }

    fn wait_for(what: &str, ready: impl Fn() -> bool) {
        for _ in 0..400 {
            if ready() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("gave up waiting for: {what}");
    }

    #[test]
    fn a_door_that_finds_the_port_taken_waits_and_opens_by_itself() {
        let port = free_port();
        let other_copy = TcpListener::bind(("127.0.0.1", port)).unwrap();
        let inbox = new_inbox();
        let door = open(port, &inbox);
        assert!(matches!(door.status(), ListenerStatus::Waiting(_)), "the port is taken: {:?}", door.status());
        assert!(door.is_on() && !door.is_listening());
        // The other copy goes away (a window that has finished closing).
        drop(other_copy);
        wait_for("the door to open once the port is free", || door.is_listening());
        assert!(send_to_running(port, "https://example.com/late.zip"), "answers once it listens");
        assert_eq!(inbox.lock().unwrap().len(), 1);
    }

    #[test]
    fn dropping_the_door_gives_the_port_back_at_once() {
        let port = free_port();
        let door = open(port, &new_inbox());
        assert_eq!(door.status(), ListenerStatus::Listening);
        assert!(send_to_running(port, "https://example.com/a.zip"));
        drop(door);
        // Nothing to wait for: the next owner (the background process) can bind right now.
        let next = TcpListener::bind(("127.0.0.1", port)).expect("the port is free as soon as the door is dropped");
        drop(next);
        assert!(!send_to_running(port, "https://example.com/b.zip"), "nobody listens any more");
    }

    #[test]
    fn a_door_that_is_waiting_can_be_dropped_too() {
        let port = free_port();
        let other_copy = TcpListener::bind(("127.0.0.1", port)).unwrap();
        let door = open(port, &new_inbox());
        let started = Instant::now();
        drop(door);
        assert!(started.elapsed() < Duration::from_millis(500), "dropping a waiting door does not wait for the port: {:?}", started.elapsed());
        // It really stopped: it does not grab the port when it frees up.
        drop(other_copy);
        thread::sleep(Duration::from_millis(300));
        TcpListener::bind(("127.0.0.1", port)).expect("a dropped door must not take the port later");
    }

    #[test]
    fn the_doors_of_a_hand_over_follow_each_other() {
        // The window has the port, the background process starts and waits, the window lets go, the background process
        // answers: the add-on talks to whoever has the door, with only a short gap.
        let port = free_port();
        let (window_inbox, background_inbox) = (new_inbox(), new_inbox());
        let window = open(port, &window_inbox);
        let background = open(port, &background_inbox);
        assert_eq!(window.status(), ListenerStatus::Listening);
        assert!(matches!(background.status(), ListenerStatus::Waiting(_)));
        assert!(send_to_running(port, "https://example.com/one.zip"));
        assert_eq!((window_inbox.lock().unwrap().len(), background_inbox.lock().unwrap().len()), (1, 0));

        drop(window);
        wait_for("the background process to take the port", || background.is_listening());
        assert!(send_to_running(port, "https://example.com/two.zip"));
        assert_eq!(background_inbox.lock().unwrap().iter().map(|i| i.url.as_str()).collect::<Vec<_>>(), ["https://example.com/two.zip"]);
    }

    #[test]
    fn a_client_that_says_nothing_does_not_keep_the_others_waiting() {
        let port = free_port();
        let inbox = new_inbox();
        let _door = open(port, &inbox);
        // A browser warming up a connection: it connects and sends nothing (the door waits 3 s for it to speak).
        let _silent: Vec<TcpStream> = (0..3).map(|_| TcpStream::connect(("127.0.0.1", port)).unwrap()).collect();
        let started = Instant::now();
        assert!(send_to_running(port, "https://example.com/quick.zip"));
        assert!(started.elapsed() < Duration::from_secs(1), "answered in {:?}", started.elapsed());
        assert_eq!(inbox.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_link_that_was_accepted_is_in_the_inbox_when_the_door_has_closed() {
        let port = free_port();
        let inbox = new_inbox();
        let mut door = open(port, &inbox);
        // Many links at once, then close at once: whatever was answered "added" must be there.
        let senders: Vec<_> = (0..8).map(|n| thread::spawn(move || send_to_running(port, &format!("https://example.com/{n}.zip")))).collect();
        door.stop();
        let accepted = senders.into_iter().map(|t| t.join().unwrap()).filter(|ok| *ok).count();
        assert_eq!(inbox.lock().unwrap().len(), accepted, "every link the add-on was told about is kept");
        assert_eq!(door.status(), ListenerStatus::Off);
    }

    #[test]
    fn a_door_that_is_off_or_pretending_does_nothing() {
        let mut off = Listener::off();
        assert_eq!(off.status(), ListenerStatus::Off);
        assert!(!off.is_on());
        off.stop();
        let mut shown = Listener::pretend();
        assert!(shown.is_listening() && shown.is_on());
        shown.stop();
        assert_eq!(shown.status(), ListenerStatus::Off);
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    #[test]
    fn asking_for_the_status_wakes_an_idle_window_so_the_next_answer_is_fresh() {
        let port = TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port();
        let ctx = eframe::egui::Context::default();
        let _door = Listener::start(port, Arc::new(Mutex::new(Vec::new())), Link::default(), ctx.clone());
        // A frame has been drawn and nothing else is wanted: the window sleeps.
        for _ in 0..5 {
            let mut frame = ctx.run_ui(Default::default(), |_| {});
            frame.textures_delta.clear();
            if !ctx.has_requested_repaint() {
                break;
            }
        }
        assert!(!ctx.has_requested_repaint(), "a window with nothing to do sleeps");
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET /status HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).unwrap();
        assert!(resp.contains("200 OK"), "{resp}");
        assert!(ctx.has_requested_repaint(), "asking for the status wakes the window, which then refreshes the snapshot");
    }
}
