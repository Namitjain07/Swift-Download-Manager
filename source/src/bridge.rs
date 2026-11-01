//! Ways for links to reach the app from outside the window:
//! - a tiny local web listener (127.0.0.1 only) used by the browser add-on
//!   and by starting the program with a link while it is already open;
//! - watching the clipboard for copied download links.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Start the local listener. Fails when another copy of the app already listens.
pub fn start_listener(port: u16, inbox: Inbox, ctx: eframe::egui::Context) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = handle(stream, &inbox, &ctx);
        }
    });
    Ok(())
}

fn handle(mut s: TcpStream, inbox: &Inbox, ctx: &eframe::egui::Context) -> std::io::Result<()> {
    s.set_read_timeout(Some(Duration::from_secs(3)))?;
    let mut reader = BufReader::new(s.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let mut len = 0usize;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" {
            break;
        }
        if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0).min(64 * 1024);
        }
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    let (path, mut query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.clone(), String::new()),
    };
    if method == "POST" && !body.is_empty() {
        query = String::from_utf8_lossy(&body).into_owned();
    }
    let (code, text) = match (method.as_str(), path.as_str()) {
        ("OPTIONS", _) => ("204 No Content", String::new()),
        ("GET", "/ping") => ("200 OK", "swift-download-manager".to_string()),
        ("GET" | "POST", "/add") => match query_value(&query, "url") {
            Some(u) if u.starts_with("http://") || u.starts_with("https://") => {
                let src = match query_value(&query, "from").as_deref() {
                    Some("cli") => Source::CommandLine,
                    _ => Source::Browser,
                };
                inbox.lock().unwrap().push(Incoming {
                    url: u,
                    name: query_value(&query, "name").unwrap_or_default(),
                    referer: query_value(&query, "referer").unwrap_or_default(),
                    cookies: query_value(&query, "cookies").unwrap_or_default(),
                    user_agent: query_value(&query, "ua").unwrap_or_default(),
                    source: src,
                });
                ctx.request_repaint();
                ("200 OK", "added".to_string())
            }
            _ => ("400 Bad Request", "missing url".to_string()),
        },
        _ => ("404 Not Found", "unknown".to_string()),
    };
    let resp = format!(
        "HTTP/1.1 {code}\r\nContent-Type: text/plain\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
    s.write_all(resp.as_bytes())
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
        start_listener(port, inbox.clone(), eframe::egui::Context::default()).unwrap();
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
