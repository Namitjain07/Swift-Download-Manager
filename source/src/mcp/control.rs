//! The private door between a `--mcp` process and the open window.
//!
//! This is separate from the browser add-on's listener (port 9614), which answers any web page.
//! Here: loopback only, a random port written to `control.json`, every request needs the secret
//! token from `mcp-token`, and anything that looks like it came from a web page is refused.

use super::command::Command;
use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

const MAX_BODY: usize = 64 * 1024;
const MAX_LINE: usize = 8 * 1024;
const MAX_HEADERS: usize = 64;

/// A command on its way to the app, with a place to send the answer.
pub struct Request {
    pub cmd: Command,
    pub reply: Sender<Result<Value, String>>,
    /// Set for the web interface; `None` for an AI assistant (MCP).
    pub who: Option<super::command::Who>,
}

/// Where the listener can be found. Written to `control.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ControlInfo {
    pub port: u16,
    pub pid: u32,
    /// "window" or "headless".
    pub mode: String,
}

#[derive(Debug, PartialEq)]
pub enum CallError {
    /// Nothing is listening: no window (or engine) is open.
    NoApp,
    /// The app answered, and the command itself failed (unknown id, MCP turned off, ...).
    Tool(String),
    /// The app could not be reached properly.
    Other(String),
}

// ---------------------------------------------------------------- token and control file

fn looks_like_token(t: &str) -> bool {
    t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit())
}

/// The shared secret, created on first use: 32 random bytes as hex, readable only by you.
pub fn token(dir: &Path) -> io::Result<String> {
    token_in(dir, "mcp-token")
}

/// A secret kept in the file `file` of the settings folder (see `token`).
pub fn token_in(dir: &Path, file: &str) -> io::Result<String> {
    let path = dir.join(file);
    if let Ok(t) = std::fs::read_to_string(&path) {
        if looks_like_token(t.trim()) {
            return Ok(t.trim().to_string());
        }
        let _ = std::fs::remove_file(&path);
    }
    std::fs::create_dir_all(dir)?;
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    let new: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    match opts.open(&path) {
        Ok(mut f) => {
            f.write_all(new.as_bytes())?;
            Ok(new)
        }
        // Another process won the race to create it: use theirs.
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            for _ in 0..50 {
                if let Ok(t) = std::fs::read_to_string(&path) {
                    if looks_like_token(t.trim()) {
                        return Ok(t.trim().to_string());
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(io::Error::other("the MCP token file is unreadable"))
        }
        Err(e) => Err(e),
    }
}

pub fn read_info(dir: &Path) -> Option<ControlInfo> {
    serde_json::from_str(&std::fs::read_to_string(dir.join("control.json")).ok()?).ok()
}

fn write_info(dir: &Path, info: &ControlInfo) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join("control.json.tmp");
    std::fs::write(&tmp, serde_json::to_string(info).unwrap_or_default())?;
    std::fs::rename(tmp, dir.join("control.json"))
}

/// Forget the listener when this process stops (only if the file is still ours).
pub fn clear_info(dir: &Path) {
    if read_info(dir).is_some_and(|i| i.pid == std::process::id()) {
        let _ = std::fs::remove_file(dir.join("control.json"));
    }
}

// ---------------------------------------------------------------- the listener

/// Start listening on a random loopback port. Commands arrive on `tx`; `ctx` is woken so a
/// window picks them up at once.
pub fn start(dir: &Path, tx: Sender<Request>, ctx: egui::Context, mode: &str) -> io::Result<ControlInfo> {
    let token = token(dir)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let info = ControlInfo { port, pid: std::process::id(), mode: mode.to_string() };
    write_info(dir, &info)?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (tx, ctx, token) = (tx.clone(), ctx.clone(), token.clone());
            std::thread::spawn(move || {
                let _ = handle(stream, port, &token, &tx, &ctx);
            });
        }
    });
    Ok(info)
}

fn respond(mut s: &TcpStream, code: &str, body: &Value) -> io::Result<()> {
    let text = body.to_string();
    // No CORS headers on purpose: a web page must never be able to read an answer.
    write!(s, "HTTP/1.1 {code}\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len())
}

fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn read_limited_line(reader: &mut impl BufRead) -> io::Result<String> {
    let mut line = String::new();
    reader.take(MAX_LINE as u64).read_line(&mut line)?;
    Ok(line)
}

fn handle(stream: TcpStream, port: u16, token: &str, tx: &Sender<Request>, ctx: &egui::Context) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let request_line = read_limited_line(&mut reader)?;
    let mut parts = request_line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    let mut headers: HashMap<String, String> = HashMap::new();
    for _ in 0..MAX_HEADERS {
        let h = read_limited_line(&mut reader)?;
        if h.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    // A request meant for another site (DNS rebinding) or sent by a web page is refused first.
    let host = headers.get("host").map(String::as_str).unwrap_or("");
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        return respond(&stream, "403 Forbidden", &json!({ "error": "unexpected Host header" }));
    }
    if headers.contains_key("origin") {
        return respond(&stream, "403 Forbidden", &json!({ "error": "web pages may not use this" }));
    }
    let given = headers.get("authorization").and_then(|a| a.strip_prefix("Bearer ")).unwrap_or("");
    if !same_secret(given, token) {
        return respond(&stream, "401 Unauthorized", &json!({ "error": "missing or wrong token" }));
    }
    let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    if len > MAX_BODY {
        return respond(&stream, "413 Payload Too Large", &json!({ "error": "body too large" }));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;

    match (method.as_str(), path.as_str()) {
        ("GET", "/info") => respond(&stream, "200 OK", &json!({ "app": "swift-download-manager", "pid": std::process::id() })),
        ("POST", "/rpc") => {
            let cmd = match serde_json::from_slice::<Value>(&body).ok().and_then(|v| serde_json::from_value::<Command>(v.get("command")?.clone()).ok()) {
                Some(c) => c,
                None => return respond(&stream, "400 Bad Request", &json!({ "error": "expected {\"command\": {...}}" })),
            };
            let (reply, answer) = mpsc::channel();
            if tx.send(Request { cmd, reply, who: None }).is_err() {
                return respond(&stream, "503 Service Unavailable", &json!({ "error": "the app is shutting down" }));
            }
            ctx.request_repaint();
            match answer.recv_timeout(Duration::from_secs(10)) {
                Ok(Ok(v)) => respond(&stream, "200 OK", &json!({ "ok": v })),
                Ok(Err(m)) => respond(&stream, "200 OK", &json!({ "error": m })),
                Err(_) => respond(&stream, "504 Gateway Timeout", &json!({ "error": "the app did not answer in time (is its window hidden?)" })),
            }
        }
        _ => respond(&stream, "404 Not Found", &json!({ "error": "unknown route" })),
    }
}

// ---------------------------------------------------------------- the client

/// Run a command in the app that is listening, if there is one.
pub fn call(dir: &Path, cmd: &Command) -> Result<Value, CallError> {
    call_with_timeout(dir, cmd, Duration::from_secs(15))
}

/// Like `call`, giving up on the answer after `timeout`.
pub fn call_with_timeout(dir: &Path, cmd: &Command, timeout: Duration) -> Result<Value, CallError> {
    let info = read_info(dir).ok_or(CallError::NoApp)?;
    let token = std::fs::read_to_string(dir.join("mcp-token")).map(|t| t.trim().to_string()).map_err(|_| CallError::NoApp)?;
    let mut s = TcpStream::connect_timeout(&([127, 0, 0, 1], info.port).into(), Duration::from_millis(500)).map_err(|_| CallError::NoApp)?;
    let io = |e: io::Error| CallError::Other(e.to_string());
    s.set_read_timeout(Some(timeout)).map_err(io)?;
    let body = json!({ "command": cmd }).to_string();
    write!(
        s,
        "POST /rpc HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        info.port,
        body.len()
    )
    .map_err(io)?;
    let mut text = String::new();
    s.read_to_string(&mut text).map_err(io)?;
    let code = text.split_whitespace().nth(1).unwrap_or("");
    let payload: Value = text.split_once("\r\n\r\n").and_then(|(_, b)| serde_json::from_str(b).ok()).unwrap_or(Value::Null);
    match code {
        "200" => match (payload.get("ok"), payload.get("error").and_then(Value::as_str)) {
            (Some(v), _) => Ok(v.clone()),
            (None, Some(m)) => Err(CallError::Tool(m.to_string())),
            _ => Err(CallError::Other("the app sent an answer that could not be understood".into())),
        },
        "401" | "403" => Err(CallError::Other("the app refused the request (token or address mismatch)".into())),
        _ => Err(CallError::Other(payload.get("error").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("the app answered HTTP {code}")))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::mpsc;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sdm-control-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A listener whose requests are answered by `answer`.
    fn serve(name: &str, answer: fn(&Command) -> Result<Value, String>) -> (std::path::PathBuf, ControlInfo) {
        let dir = scratch(name);
        let (tx, rx) = mpsc::channel::<Request>();
        let info = start(&dir, tx, eframe::egui::Context::default(), "window").unwrap();
        std::thread::spawn(move || {
            for req in rx {
                let r = answer(&req.cmd);
                let _ = req.reply.send(r);
            }
        });
        (dir, info)
    }

    fn echo(cmd: &Command) -> Result<Value, String> {
        Ok(json!({ "got": format!("{cmd:?}") }))
    }

    fn raw(port: u16, request: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        s.write_all(request.as_bytes()).unwrap();
        let mut out = String::new();
        let _ = s.read_to_string(&mut out);
        out
    }

    fn post(port: u16, host: &str, extra: &str, body: &str) -> String {
        raw(port, &format!("POST /rpc HTTP/1.1\r\nHost: {host}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()))
    }

    const GET_STATUS: &str = r#"{"command":{"cmd":"get_status"}}"#;

    #[test]
    fn rejects_missing_or_wrong_token() {
        let (dir, info) = serve("auth", echo);
        let host = format!("127.0.0.1:{}", info.port);
        assert!(post(info.port, &host, "", GET_STATUS).starts_with("HTTP/1.1 401"));
        assert!(post(info.port, &host, "Authorization: Bearer nope\r\n", GET_STATUS).starts_with("HTTP/1.1 401"));
        let good = format!("Authorization: Bearer {}\r\n", token(&dir).unwrap());
        assert!(post(info.port, &host, &good, GET_STATUS).starts_with("HTTP/1.1 200"));
    }

    #[test]
    fn rejects_browser_origin_even_with_the_token() {
        let (dir, info) = serve("origin", echo);
        let host = format!("127.0.0.1:{}", info.port);
        let h = format!("Authorization: Bearer {}\r\nOrigin: https://evil.example\r\n", token(&dir).unwrap());
        assert!(post(info.port, &host, &h, GET_STATUS).starts_with("HTTP/1.1 403"));
    }

    #[test]
    fn rejects_foreign_host_header() {
        let (dir, info) = serve("host", echo);
        let h = format!("Authorization: Bearer {}\r\n", token(&dir).unwrap());
        assert!(post(info.port, "evil.example", &h, GET_STATUS).starts_with("HTTP/1.1 403"), "DNS rebinding");
        assert!(post(info.port, "127.0.0.1:1", &h, GET_STATUS).starts_with("HTTP/1.1 403"), "wrong port");
        assert!(post(info.port, &format!("localhost:{}", info.port), &h, GET_STATUS).starts_with("HTTP/1.1 200"));
    }

    #[test]
    fn rejects_oversized_body_without_reading_it() {
        let (dir, info) = serve("big", echo);
        let host = format!("127.0.0.1:{}", info.port);
        let req = format!("POST /rpc HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {}\r\nContent-Length: 100000\r\nConnection: close\r\n\r\n", token(&dir).unwrap());
        assert!(raw(info.port, &req).starts_with("HTTP/1.1 413"));
    }

    #[test]
    fn answers_have_no_cors_headers_and_unknown_routes_404() {
        let (dir, info) = serve("cors", echo);
        let host = format!("127.0.0.1:{}", info.port);
        let h = format!("Authorization: Bearer {}\r\n", token(&dir).unwrap());
        let ok = post(info.port, &host, &h, GET_STATUS);
        assert!(!ok.to_ascii_lowercase().contains("access-control"));
        let preflight = raw(info.port, &format!("OPTIONS /rpc HTTP/1.1\r\nHost: {host}\r\nOrigin: https://evil.example\r\nConnection: close\r\n\r\n"));
        assert!(!preflight.starts_with("HTTP/1.1 200") && !preflight.starts_with("HTTP/1.1 204"));
        let nothing = raw(info.port, &format!("GET /nothing HTTP/1.1\r\nHost: {host}\r\n{h}Connection: close\r\n\r\n"));
        assert!(nothing.starts_with("HTTP/1.1 404"));
    }

    #[test]
    fn call_reaches_the_app_and_returns_its_answer() {
        let (dir, _info) = serve("call", echo);
        let v = call(&dir, &Command::GetDownload { id: 7 }).unwrap();
        assert_eq!(v["got"], "GetDownload { id: 7 }");
    }

    #[test]
    fn tool_errors_come_back_as_tool_errors() {
        let (dir, _info) = serve("toolerr", |_| Err("nope".into()));
        assert_eq!(call(&dir, &Command::GetStatus), Err(CallError::Tool("nope".into())));
    }

    #[test]
    fn call_without_a_listener_says_no_app() {
        let dir = scratch("none");
        assert_eq!(call(&dir, &Command::GetStatus), Err(CallError::NoApp));
        // A stale control file (nobody listening) is the same as no app.
        std::fs::write(dir.join("control.json"), r#"{"port":1,"pid":1,"mode":"window"}"#).unwrap();
        assert_eq!(call(&dir, &Command::GetStatus), Err(CallError::NoApp));
    }

    #[test]
    fn token_is_stable_hex_and_private() {
        let dir = scratch("token");
        let a = token(&dir).unwrap();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(token(&dir).unwrap(), a);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dir.join("mcp-token")).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }
}
