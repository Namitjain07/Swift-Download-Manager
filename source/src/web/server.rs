//! The web server: a login (or an access key), a small JSON API that runs the same commands as the
//! window and the AI assistants, the page itself, and the finished files.
//!
//! Two ways to run it:
//! * **Personal**: listens on this computer only. Open the link from Options > Web interface; it carries a
//!   secret key that is swapped for a session cookie.
//! * **Shared**: a server for several people. Everybody logs in with their own account and sees only their own
//!   downloads; administrators see all. Put it behind HTTPS (a reverse proxy) when it leaves a trusted network.

use super::auth::{same_secret, Sessions, Throttle, Users};
use super::http::{self, parse_range, read_request, ReadError, Req, Span};
use crate::mcp::command::{Command, Who};
use crate::mcp::control::{self, Request};
use eframe::egui;
use serde_json::{json, Value};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PAGE: &str = include_str!("ui/index.html");
const MAX_BODY: usize = 64 * 1024;
const MAX_CONNECTIONS: usize = 128;
const COOKIE: &str = "sdm_session";

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// This computer only: one person, opened with a secret key.
    Personal,
    /// Several people with accounts.
    Shared,
}

#[derive(Clone)]
pub struct Config {
    pub bind: IpAddr,
    /// 0 picks a free port.
    pub port: u16,
    pub mode: Mode,
    /// The settings folder: `users.json` and `web-key` live here.
    pub dir: PathBuf,
    /// Mark the session cookie `Secure` (the site is served over HTTPS).
    pub secure_cookie: bool,
}

struct Shared {
    cfg: Config,
    key: String,
    sessions: Sessions,
    throttle: Throttle,
    tx: Mutex<Sender<Request>>,
    ctx: egui::Context,
    active: AtomicUsize,
    port: u16,
}

/// A running server. Dropping it stops listening.
pub struct Handle {
    pub addr: SocketAddr,
    pub key: String,
    stop: Arc<AtomicBool>,
}

impl Handle {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Like `start`, but waits (up to `tries` x 100 ms) for a port that another process is just letting go of.
pub fn start_retrying(cfg: Config, tx: Sender<Request>, ctx: egui::Context, tries: u32) -> io::Result<Handle> {
    let mut left = tries;
    loop {
        match start(cfg.clone(), tx.clone(), ctx.clone()) {
            Err(e) if e.kind() == io::ErrorKind::AddrInUse && left > 0 => {
                left -= 1;
                std::thread::sleep(Duration::from_millis(100));
            }
            other => return other,
        }
    }
}

pub fn start(cfg: Config, tx: Sender<Request>, ctx: egui::Context) -> io::Result<Handle> {
    let listener = TcpListener::bind((cfg.bind, cfg.port))?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let key = control::token_in(&cfg.dir, "web-key")?;
    let state = Arc::new(Shared {
        key: key.clone(),
        sessions: Sessions::default(),
        throttle: Throttle::default(),
        tx: Mutex::new(tx),
        ctx,
        active: AtomicUsize::new(0),
        port: addr.port(),
        cfg,
    });
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::spawn(move || {
        while !flag.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, peer)) => {
                    let state = state.clone();
                    if state.active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                        state.active.fetch_sub(1, Ordering::SeqCst);
                        let _ = reply(&stream, 503, &json!({ "error": "too many connections" }), &[]);
                        continue;
                    }
                    std::thread::spawn(move || {
                        let _ = stream.set_nonblocking(false);
                        let _ = handle(&state, stream, peer.ip());
                        state.active.fetch_sub(1, Ordering::SeqCst);
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(40)),
                Err(_) => std::thread::sleep(Duration::from_millis(200)),
            }
        }
    });
    Ok(Handle { addr, key, stop })
}

// ---------------------------------------------------------------- answers

fn security_headers() -> Vec<(&'static str, String)> {
    vec![
        ("X-Content-Type-Options", "nosniff".into()),
        ("X-Frame-Options", "DENY".into()),
        ("Referrer-Policy", "no-referrer".into()),
        ("Cache-Control", "no-store".into()),
    ]
}

/// A JSON answer.
fn reply(mut s: &TcpStream, code: u16, body: &Value, extra: &[(&str, String)]) -> io::Result<()> {
    let text = body.to_string();
    let mut headers = security_headers();
    headers.push(("Content-Type", "application/json".into()));
    headers.push(("Content-Length", text.len().to_string()));
    headers.extend(extra.iter().map(|(k, v)| (*k, v.clone())));
    http::write_head(&mut s, code, &headers)?;
    s.write_all(text.as_bytes())
}

fn page(mut s: &TcpStream, head_only: bool) -> io::Result<()> {
    let mut headers = security_headers();
    headers.push(("Content-Type", "text/html; charset=utf-8".into()));
    headers.push(("Content-Length", PAGE.len().to_string()));
    // The page is one file with its own style and script, and loads nothing from anywhere else.
    headers.push(("Content-Security-Policy", "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'".into()));
    http::write_head(&mut s, 200, &headers)?;
    if head_only {
        return Ok(());
    }
    s.write_all(PAGE.as_bytes())
}

// ---------------------------------------------------------------- one request

fn handle(state: &Shared, stream: TcpStream, peer: IpAddr) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(60)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let req = match read_request(&mut reader, MAX_BODY) {
        Ok(r) => r,
        Err(ReadError::Closed) => return Ok(()),
        Err(ReadError::TooLarge) => return reply(&stream, 413, &json!({ "error": "request too large" }), &[]),
        Err(ReadError::Malformed) => return reply(&stream, 400, &json!({ "error": "bad request" }), &[]),
    };
    // A page from another site (or another name for this computer) must never get an answer.
    if state.cfg.mode == Mode::Personal {
        let host = req.header("host");
        let ok = [format!("127.0.0.1:{}", state.port), format!("localhost:{}", state.port), format!("[::1]:{}", state.port)].iter().any(|h| h == host);
        if !ok {
            return reply(&stream, 403, &json!({ "error": "unexpected Host header" }), &[]);
        }
    }
    let origin = req.header("origin");
    if !origin.is_empty() && origin.split_once("://").map(|(_, h)| h) != Some(req.header("host")) {
        return reply(&stream, 403, &json!({ "error": "web pages from other sites may not use this" }), &[]);
    }
    let head_only = req.method == "HEAD";
    let method = if head_only { "GET" } else { req.method.as_str() };
    // Changing anything needs a header that a page on another site cannot add.
    if matches!(method, "POST" | "DELETE") && req.header("x-sdm-request") != "1" && req.header("authorization").is_empty() {
        return reply(&stream, 403, &json!({ "error": "missing X-SDM-Request header" }), &[]);
    }

    match (method, req.path.as_str()) {
        ("GET", "/") => {
            // The access key arrives in the link; swap it for a session and take it out of the address.
            if let Some(given) = req.query.get("key") {
                if state.cfg.mode == Mode::Personal && same_secret(given, &state.key) {
                    let token = state.sessions.create("", true);
                    return reply(&stream, 302, &json!({}), &[("Location", "/".into()), ("Set-Cookie", cookie(state, &token, &req))]);
                }
                return reply(&stream, 403, &json!({ "error": "wrong key" }), &[]);
            }
            page(&stream, head_only)
        }
        ("GET", "/favicon.ico") => reply(&stream, 204, &json!({}), &[]),
        ("POST", "/api/login") => login(state, &stream, &req, peer),
        ("POST", "/api/logout") => {
            if let Some(t) = req.cookie(COOKIE) {
                state.sessions.destroy(&t);
            }
            reply(&stream, 200, &json!({ "ok": true }), &[("Set-Cookie", format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"))])
        }
        _ => {
            let Some(who) = identify(state, &req) else {
                let shared = state.cfg.mode == Mode::Shared;
                return reply(&stream, 401, &json!({ "error": "sign in first", "shared": shared }), &[]);
            };
            authenticated(state, &stream, &req, method, head_only, &who)
        }
    }
}

fn cookie(state: &Shared, token: &str, req: &Req) -> String {
    let secure = state.cfg.secure_cookie || req.header("x-forwarded-proto") == "https";
    format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=604800{}", if secure { "; Secure" } else { "" })
}

/// Who is asking, from the session cookie or (for scripts) the access key.
fn identify(state: &Shared, req: &Req) -> Option<Who> {
    if let Some(s) = req.cookie(COOKIE).and_then(|t| state.sessions.lookup(&t)) {
        // The account may have been removed since.
        // The accounts are read fresh each time, so people added or removed with `--add-user` / `--remove-user`
        // while the server runs take effect at once.
        if state.cfg.mode == Mode::Shared {
            let users = Users::load(&state.cfg.dir);
            let admin = users.names().into_iter().find(|(n, _)| *n == s.user).map(|(_, a)| a)?;
            return Some(Who { name: s.user, admin });
        }
        return Some(Who { name: s.user, admin: s.admin });
    }
    let bearer = req.header("authorization").strip_prefix("Bearer ").unwrap_or("");
    if state.cfg.mode == Mode::Personal && !bearer.is_empty() && same_secret(bearer, &state.key) {
        return Some(Who { name: String::new(), admin: true });
    }
    None
}

fn login(state: &Shared, stream: &TcpStream, req: &Req, peer: IpAddr) -> io::Result<()> {
    if state.cfg.mode != Mode::Shared {
        return reply(stream, 404, &json!({ "error": "this server has no accounts" }), &[]);
    }
    if state.throttle.blocked(peer) {
        return reply(stream, 429, &json!({ "error": "too many wrong passwords; try again in a few minutes" }), &[("Retry-After", "300".into())]);
    }
    let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
    let (user, password) = (body["user"].as_str().unwrap_or(""), body["password"].as_str().unwrap_or(""));
    let found = Users::load(&state.cfg.dir).verify(user, password).cloned();
    match found {
        Some(u) => {
            state.throttle.succeeded(peer);
            let token = state.sessions.create(&u.name, u.admin);
            reply(stream, 200, &json!({ "user": u.name, "admin": u.admin }), &[("Set-Cookie", cookie(state, &token, req))])
        }
        None => {
            state.throttle.failed(peer);
            reply(stream, 401, &json!({ "error": "wrong user name or password" }), &[])
        }
    }
}

/// Run a command in the app for this person.
fn ask(state: &Shared, who: &Who, cmd: Command) -> Result<Value, (u16, String)> {
    let (answer, wait) = mpsc::channel();
    let sent = state.tx.lock().unwrap().send(Request { cmd, reply: answer, who: Some(who.clone()) });
    if sent.is_err() {
        return Err((503, "the app is shutting down".into()));
    }
    state.ctx.request_repaint();
    match wait.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(m)) if m.contains("No download with id") => Err((404, m)),
        Ok(Err(m)) => Err((400, m)),
        Err(_) => Err((504, "the app did not answer in time".into())),
    }
}

fn answer(stream: &TcpStream, r: Result<Value, (u16, String)>) -> io::Result<()> {
    match r {
        Ok(v) => reply(stream, 200, &v, &[]),
        Err((code, m)) => reply(stream, code, &json!({ "error": m }), &[]),
    }
}

fn authenticated(state: &Shared, stream: &TcpStream, req: &Req, method: &str, head_only: bool, who: &Who) -> io::Result<()> {
    let parts: Vec<&str> = req.path.trim_matches('/').split('/').collect();
    let id = |s: &str| s.parse::<u64>().ok();
    let json_body = || serde_json::from_slice::<Value>(&req.body).unwrap_or(Value::Null);
    match (method, parts.as_slice()) {
        ("GET", ["api", "me"]) => reply(stream, 200, &json!({ "user": who.name, "admin": who.admin, "shared": state.cfg.mode == Mode::Shared }), &[]),
        ("GET", ["api", "status"]) => answer(stream, ask(state, who, Command::GetStatus)),
        ("GET", ["api", "queues"]) => answer(stream, ask(state, who, Command::ListQueues)),
        ("GET", ["api", "downloads"]) => {
            let limit = req.query.get("limit").and_then(|v| v.parse().ok());
            let cmd = Command::ListDownloads { status: req.query.get("status").cloned(), query: req.query.get("q").cloned(), limit };
            answer(stream, ask(state, who, cmd))
        }
        ("POST", ["api", "downloads"]) => {
            let b = json_body();
            let text = |k: &str| b[k].as_str().map(str::to_string);
            let Some(url) = text("url") else { return reply(stream, 400, &json!({ "error": "a link (url) is needed" }), &[]) };
            let cmd = Command::AddDownload { url, name: text("name"), category: text("category"), queue: text("queue"), start: b["start"].as_bool() };
            answer(stream, ask(state, who, cmd))
        }
        ("GET", ["api", "downloads", n]) => match id(n) {
            Some(id) => answer(stream, ask(state, who, Command::GetDownload { id })),
            None => reply(stream, 404, &json!({ "error": "no such download" }), &[]),
        },
        ("POST", ["api", "downloads", n, "pause"]) => match id(n) {
            Some(id) => answer(stream, ask(state, who, Command::Pause { id })),
            None => reply(stream, 404, &json!({ "error": "no such download" }), &[]),
        },
        ("POST", ["api", "downloads", n, "resume"]) => match id(n) {
            Some(id) => answer(stream, ask(state, who, Command::Resume { id })),
            None => reply(stream, 404, &json!({ "error": "no such download" }), &[]),
        },
        ("DELETE", ["api", "downloads", n]) => match id(n) {
            Some(id) => answer(stream, ask(state, who, Command::Remove { id })),
            None => reply(stream, 404, &json!({ "error": "no such download" }), &[]),
        },
        ("POST", ["api", "queues", name, "start"]) => answer(stream, ask(state, who, Command::StartQueue { name: http::percent_decode(name) })),
        ("POST", ["api", "queues", name, "stop"]) => answer(stream, ask(state, who, Command::StopQueue { name: http::percent_decode(name) })),
        ("GET", ["files", n]) => match id(n) {
            Some(id) => send_file(state, stream, req, head_only, who, id),
            None => reply(stream, 404, &json!({ "error": "no such download" }), &[]),
        },
        ("GET" | "POST" | "DELETE", _) => reply(stream, 404, &json!({ "error": "unknown address" }), &[]),
        _ => reply(stream, 405, &json!({ "error": "method not allowed" }), &[]),
    }
}

/// Send a finished file to the browser, in pieces if it asks (so a broken transfer can carry on).
fn send_file(state: &Shared, stream: &TcpStream, req: &Req, head_only: bool, who: &Who, id: u64) -> io::Result<()> {
    let found = match ask(state, who, Command::Locate { id }) {
        Ok(v) => v,
        Err((code, m)) => return reply(stream, code, &json!({ "error": m }), &[]),
    };
    // The path comes from the app's own list, never from the request.
    let (Some(path), Some(name)) = (found["path"].as_str(), found["name"].as_str()) else {
        return reply(stream, 500, &json!({ "error": "the app gave no file" }), &[]);
    };
    let Ok(mut file) = std::fs::File::open(path) else { return reply(stream, 404, &json!({ "error": "the file is gone from the server" }), &[]) };
    let len = file.metadata()?.len();
    let mut headers = security_headers();
    headers.push(("Content-Type", "application/octet-stream".into()));
    headers.push(("Content-Disposition", http::disposition(name)));
    headers.push(("Accept-Ranges", "bytes".into()));
    let (code, start, count) = match parse_range(req.header("range"), len) {
        Span::Whole => (200, 0, len),
        Span::Part(a, b) => {
            headers.push(("Content-Range", format!("bytes {a}-{b}/{len}")));
            (206, a, b - a + 1)
        }
        Span::Unsatisfiable => {
            headers.push(("Content-Range", format!("bytes */{len}")));
            headers.push(("Content-Length", "0".into()));
            return http::write_head(&mut &*stream, 416, &headers);
        }
    };
    headers.push(("Content-Length", count.to_string()));
    let mut out = stream;
    http::write_head(&mut out, code, &headers)?;
    if head_only {
        return Ok(());
    }
    file.seek(SeekFrom::Start(start))?;
    let mut left = file.take(count);
    io::copy(&mut left, &mut out).map(|_| ())
}

#[cfg(test)]
mod tests;
