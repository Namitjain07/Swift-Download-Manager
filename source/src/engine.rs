//! The download engine: probes a link, splits the file into parts, fetches
//! each part on its own connection, re-splits the biggest part whenever a
//! connection becomes free, retries broken connections and supports pause / resume.

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::netroute::{Lease, Route, Router};

/// Parts smaller than twice this are not split again.
const MIN_SPLIT: u64 = 512 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Queued,
    Connecting,
    Downloading,
    Paused,
    Complete,
    Error,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Queued => "Queued",
            Status::Connecting => "Connecting…",
            Status::Downloading => "Downloading",
            Status::Paused => "Paused",
            Status::Complete => "Complete",
            Status::Error => "Error",
        }
    }
    pub fn is_active(self) -> bool {
        matches!(self, Status::Connecting | Status::Downloading)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Segment {
    pub start: u64,
    /// Inclusive end byte.
    pub end: u64,
    pub done: u64,
}

impl Segment {
    pub fn len(&self) -> u64 {
        self.end + 1 - self.start
    }
    pub fn remaining(&self) -> u64 {
        self.len().saturating_sub(self.done)
    }
    pub fn finished(&self) -> bool {
        self.done >= self.len()
    }
}

/// Speed limiter (0 = unlimited). A download's own limiter can be chained to
/// the overall one, so both limits apply.
pub struct Limiter {
    pub bytes_per_sec: AtomicU64,
    bucket: Mutex<(Instant, f64)>,
    next: Option<Arc<Limiter>>,
}

impl Limiter {
    pub fn new() -> Self {
        Self { bytes_per_sec: AtomicU64::new(0), bucket: Mutex::new((Instant::now(), 0.0)), next: None }
    }
    pub fn chained(next: Arc<Limiter>) -> Self {
        Self { next: Some(next), ..Self::new() }
    }
    pub fn take(&self, n: usize) {
        let limit = self.bytes_per_sec.load(Ordering::Relaxed);
        if limit > 0 {
            let wait = {
                let mut b = self.bucket.lock().unwrap();
                let now = Instant::now();
                let elapsed = now.duration_since(b.0).as_secs_f64();
                b.0 = now;
                b.1 = (b.1 + elapsed * limit as f64).min(limit as f64 / 4.0);
                b.1 -= n as f64;
                if b.1 < 0.0 { -b.1 / limit as f64 } else { 0.0 }
            };
            if wait > 0.0 {
                thread::sleep(Duration::from_secs_f64(wait.min(2.0)));
            }
        }
        if let Some(next) = &self.next {
            next.take(n);
        }
    }
}

/// Live state of one download, shared between the UI and worker threads.
pub struct Live {
    pub status: Mutex<Status>,
    pub total: AtomicU64,
    pub downloaded: AtomicU64,
    pub stop: AtomicBool,
    pub resumable: AtomicBool,
    pub segments: Mutex<Vec<Segment>>,
    pub error: Mutex<Option<String>>,
    /// Set when the server suggested a different file name.
    pub file_name: Mutex<Option<String>>,
    /// Number of connections receiving data right now.
    pub connections: AtomicU64,
    /// Short note about retries, shown in the progress window.
    pub note: Mutex<Option<String>>,
    /// The networks this download is spread over, while it runs with more than one chosen.
    pub router: Mutex<Option<Arc<Router>>>,
}

impl Live {
    pub fn new(total: u64, downloaded: u64, status: Status, segments: Vec<Segment>, resumable: bool) -> Arc<Self> {
        Arc::new(Self {
            status: Mutex::new(status),
            total: AtomicU64::new(total),
            downloaded: AtomicU64::new(downloaded),
            stop: AtomicBool::new(false),
            resumable: AtomicBool::new(resumable),
            segments: Mutex::new(segments),
            error: Mutex::new(None),
            file_name: Mutex::new(None),
            connections: AtomicU64::new(0),
            note: Mutex::new(None),
            router: Mutex::new(None),
        })
    }
    pub fn status(&self) -> Status {
        *self.status.lock().unwrap()
    }
    fn set_status(&self, s: Status) {
        *self.status.lock().unwrap() = s;
    }
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Default)]
pub struct Job {
    pub url: String,
    pub target: PathBuf,
    pub connections: usize,
    /// Use the name the server suggests (only for a fresh download with an automatic name).
    pub accept_server_name: bool,
    pub referer: String,
    pub user: String,
    pub pass: String,
    /// e.g. "http://127.0.0.1:8080"
    pub proxy: Option<String>,
    pub retries: u32,
    /// Browser cookies for the site ("a=1; b=2"), so downloads behind a login work.
    pub cookies: String,
    /// Browser identity to send instead of the app's own.
    pub user_agent: String,
    /// Networks to download over at the same time (Wi-Fi, Ethernet, a phone ...). Empty: the normal way out.
    pub routes: Vec<Route>,
    /// Refuse links that lead to private or local addresses (for a server that other people use).
    pub block_private: bool,
}

pub fn temp_path(target: &Path) -> PathBuf {
    let mut s = target.as_os_str().to_owned();
    s.push(".sdpart");
    PathBuf::from(s)
}

fn agent(job: &Job, route: Option<&Route>) -> Result<ureq::Agent, String> {
    let proxy = match job.proxy.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => Some(ureq::Proxy::new(p).map_err(|e| format!("Proxy setting is not valid: {e}"))?),
        None => None,
    };
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .proxy(proxy)
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(Duration::from_secs(60)))
        .user_agent("SwiftDownloadManager/1.0")
        .build();
    Ok(if route.is_some() || job.block_private { crate::netroute::agent(config, route, job.block_private) } else { config.into() })
}

/// Why a request failed.
struct ReqErr {
    msg: String,
    /// The network card is to blame (gone, unreachable, silent), not the server.
    network: bool,
}

impl From<String> for ReqErr {
    fn from(msg: String) -> Self {
        ReqErr { msg, network: false }
    }
}

impl From<ReqErr> for String {
    fn from(e: ReqErr) -> Self {
        e.msg
    }
}

/// Whether a failed connection says the chosen network is not working.
fn network_problem(e: &ureq::Error) -> bool {
    use std::io::ErrorKind::*;
    match e {
        ureq::Error::Io(e) => matches!(e.kind(), AddrNotAvailable | NetworkUnreachable | HostUnreachable | NetworkDown | PermissionDenied),
        ureq::Error::Timeout(t) => matches!(t, ureq::Timeout::Connect | ureq::Timeout::Resolve),
        _ => false,
    }
}

fn request(job: &Job, range: &str, lease: Option<&Lease>) -> Result<ureq::http::Response<ureq::Body>, ReqErr> {
    let agent = agent(job, lease.map(|l| l.route()))?;
    let mut req = agent.get(&job.url).header("Range", range);
    if !job.referer.trim().is_empty() {
        req = req.header("Referer", job.referer.trim());
    }
    if !job.cookies.trim().is_empty() {
        req = req.header("Cookie", job.cookies.trim());
    }
    if !job.user_agent.trim().is_empty() {
        req = req.header("User-Agent", job.user_agent.trim());
    }
    if !job.user.is_empty() {
        let token = base64_encode(format!("{}:{}", job.user, job.pass).as_bytes());
        req = req.header("Authorization", &format!("Basic {token}"));
    }
    req.call().map_err(|e| ReqErr { network: lease.is_some() && network_problem(&e), msg: format!("Could not connect: {e}") })
}

/// A network for one connection. `None` when no networks were chosen: use the normal way out.
fn take_route(router: &Arc<Router>) -> Result<Option<Lease>, String> {
    if router.len() == 0 {
        return Ok(None);
    }
    router.acquire().map(Some).ok_or_else(|| "None of the chosen networks is working right now".to_string())
}

/// Sleep for the retry delay, waking early when the download is paused.
fn retry_pause(live: &Live, attempt: u32) {
    *live.note.lock().unwrap() = Some(format!("Connection problem, retry {attempt}…"));
    for _ in 0..20 {
        if live.stopped() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Wait before trying again, except when another network can take over right away.
fn retry_after_failure(live: &Live, attempt: u32, router: &Router) {
    if router.usable() > 0 && router.len() > 1 {
        *live.note.lock().unwrap() = Some("A network is not working, using the others…".into());
        return;
    }
    retry_pause(live, attempt);
}

/// A part's first connection can be the one that probed the server.
type Reuse = (Box<dyn Read + Send>, Option<Lease>);

/// Start (or resume) a download on a background thread.
pub fn start(job: Job, live: Arc<Live>, limiter: Arc<Limiter>, ctx: eframe::egui::Context) {
    live.stop.store(false, Ordering::SeqCst);
    *live.error.lock().unwrap() = None;
    *live.note.lock().unwrap() = None;
    live.set_status(Status::Connecting);
    thread::spawn(move || {
        let job = Arc::new(job);
        let result = run(&job, &live, &limiter);
        live.connections.store(0, Ordering::SeqCst);
        *live.router.lock().unwrap() = None;
        *live.note.lock().unwrap() = None;
        match result {
            Ok(true) => live.set_status(Status::Complete),
            Ok(false) => live.set_status(Status::Paused),
            Err(_) if live.stopped() => live.set_status(Status::Paused),
            Err(e) => {
                *live.error.lock().unwrap() = Some(e);
                live.set_status(Status::Error);
            }
        }
        ctx.request_repaint();
    });
}

fn parse_total_from_content_range(v: &str) -> Option<u64> {
    v.rsplit('/').next()?.trim().parse().ok()
}

pub fn name_from_disposition(v: &str) -> Option<String> {
    for part in v.split(';') {
        if let Some(rest) = part.trim().strip_prefix("filename*=") {
            let rest = rest.trim_matches('"');
            let name = rest.splitn(3, '\'').last().unwrap_or(rest);
            return Some(percent_decode(name));
        }
    }
    for part in v.split(';') {
        if let Some(rest) = part.trim().strip_prefix("filename=") {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz");
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(T[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn header(resp: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    resp.headers().get(name).and_then(|v| v.to_str().ok()).map(|s| s.to_string())
}

/// Returns Ok(true) when finished, Ok(false) when paused.
fn run(job: &Arc<Job>, live: &Arc<Live>, limiter: &Arc<Limiter>) -> Result<bool, String> {
    let have_segments = !live.segments.lock().unwrap().is_empty();
    let router = Router::new(job.routes.clone());
    if router.len() > 0 {
        *live.router.lock().unwrap() = Some(router.clone());
    }

    // A previously split download whose part file still exists: just resume the parts.
    if have_segments && temp_path(&job.target).exists() && live.resumable.load(Ordering::SeqCst) {
        live.set_status(Status::Downloading);
        return run_segments(job, live, limiter, &router, None);
    }

    // Probe the server (with retries). The probe goes out through one of the chosen networks.
    let mut attempt = 0;
    let (resp, mut lease) = loop {
        let r = take_route(&router).map_err(ReqErr::from).and_then(|lease| match request(job, "bytes=0-", lease.as_ref()) {
            Ok(r) if r.status().as_u16() >= 400 => Err(ReqErr::from(format!("Server answered {}", r.status().as_u16()))),
            Ok(r) => Ok((r, lease)),
            Err(e) => {
                if e.network {
                    lease.iter().for_each(|l| l.fail());
                }
                Err(e)
            }
        });
        match r {
            Ok(r) => break r,
            Err(e) => {
                let e = e.msg;
                // 4xx answers will not get better by retrying.
                let permanent = (e.starts_with("Server answered 4") && !e.contains("429")) || e.contains("may not download from private");
                if live.stopped() || permanent || attempt >= job.retries {
                    return Err(e);
                }
                attempt += 1;
                retry_after_failure(live, attempt, &router);
            }
        }
    };
    *live.note.lock().unwrap() = None;
    let mut job = (**job).clone();
    if job.accept_server_name {
        if let Some(n) = header(&resp, "content-disposition").and_then(|v| name_from_disposition(&v)) {
            let n = sanitize(&n);
            if let Some(dir) = job.target.parent() {
                let mut candidate = dir.join(&n);
                let mut k = 1;
                while candidate.exists() {
                    candidate = dir.join(numbered(&n, k));
                    k += 1;
                }
                *live.file_name.lock().unwrap() = candidate.file_name().map(|s| s.to_string_lossy().into_owned());
                job.target = candidate;
            }
        }
    }
    let job = Arc::new(job);
    let tmp = temp_path(&job.target);
    let code = resp.status().as_u16();
    let resumable = code == 206;
    let total = if resumable {
        header(&resp, "content-range").and_then(|v| parse_total_from_content_range(&v))
    } else {
        header(&resp, "content-length").and_then(|v| v.parse().ok())
    }
    .unwrap_or(0);
    live.resumable.store(resumable && total > 0, Ordering::SeqCst);
    live.total.store(total, Ordering::SeqCst);

    if let Some(dir) = job.target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create folder: {e}"))?;
    }

    if resumable && total > 0 {
        // Split into parts. The first part re-uses the probe connection.
        let n = job.connections.max(router.len()).max(1).min(((total / MIN_SPLIT).max(1)) as usize) as u64;
        let size = total / n;
        let segs = (0..n)
            .map(|i| Segment { start: i * size, end: if i == n - 1 { total - 1 } else { (i + 1) * size - 1 }, done: 0 })
            .collect();
        *live.segments.lock().unwrap() = segs;
        live.downloaded.store(0, Ordering::SeqCst);
        let f = File::create(&tmp).map_err(|e| format!("Cannot create file: {e}"))?;
        f.set_len(total).map_err(|e| format!("Cannot reserve space: {e}"))?;
        drop(f);
        live.set_status(Status::Downloading);
        let first: Box<dyn Read + Send> = Box::new(resp.into_body().into_reader());
        return run_segments(&job, live, limiter, &router, Some((first, lease)));
    }

    // Plain single stream (server cannot resume): on a broken connection start over.
    live.segments.lock().unwrap().clear();
    let mut reader: Box<dyn Read + Send> = Box::new(resp.into_body().into_reader());
    let mut attempt = 0;
    loop {
        live.downloaded.store(0, Ordering::SeqCst);
        live.set_status(Status::Downloading);
        live.connections.store(1, Ordering::SeqCst);
        match single_stream(&mut reader, &tmp, live, limiter, lease.as_ref()) {
            Ok(true) => break,
            Ok(false) => return Ok(false),
            Err(e) => {
                if live.stopped() || attempt >= job.retries {
                    return Err(e);
                }
                attempt += 1;
                retry_after_failure(live, attempt, &router);
                drop(lease.take()); // hand the network back first, so the router sees the real load
                lease = take_route(&router)?;
                let r = request(&job, "bytes=0-", lease.as_ref())?;
                reader = Box::new(r.into_body().into_reader());
            }
        }
    }
    let got = live.downloaded.load(Ordering::SeqCst);
    let expected = live.total.load(Ordering::SeqCst);
    if expected > 0 && got < expected {
        return Err(format!("Connection closed early ({got} of {expected} bytes)"));
    }
    live.total.store(got, Ordering::SeqCst);
    finish(&tmp, &job.target)?;
    Ok(true)
}

fn single_stream(reader: &mut Box<dyn Read + Send>, tmp: &Path, live: &Live, limiter: &Limiter, lease: Option<&Lease>) -> Result<bool, String> {
    let mut f = File::create(tmp).map_err(|e| format!("Cannot create file: {e}"))?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if live.stopped() {
            return Ok(false);
        }
        let n = reader.read(&mut buf).map_err(|e| format!("Connection lost: {e}"))?;
        if n == 0 {
            return Ok(true);
        }
        limiter.take(n);
        f.write_all(&buf[..n]).map_err(|e| format!("Write failed: {e}"))?;
        live.downloaded.fetch_add(n as u64, Ordering::SeqCst);
        if let Some(l) = lease {
            l.add_bytes(n as u64);
        }
    }
}

/// Split the part with the most bytes left in two; returns the new part's index.
fn split_largest(live: &Live) -> Option<usize> {
    let mut segs = live.segments.lock().unwrap();
    let (idx, rem) = segs.iter().enumerate().map(|(i, s)| (i, s.remaining())).max_by_key(|(_, r)| *r)?;
    if rem < MIN_SPLIT * 2 {
        return None;
    }
    let s = &mut segs[idx];
    let new_start = s.start + s.done + rem / 2;
    let new = Segment { start: new_start, end: s.end, done: 0 };
    s.end = new_start - 1;
    segs.push(new);
    Some(segs.len() - 1)
}

/// How a connection ended.
enum End {
    Done,
    /// The server is busy (too many connections): the part goes back to the pool.
    Busy,
    Failed(String),
}

fn run_segments(job: &Arc<Job>, live: &Arc<Live>, limiter: &Arc<Limiter>, router: &Arc<Router>, mut first: Option<Reuse>) -> Result<bool, String> {
    let tmp = temp_path(&job.target);
    let (tx, rx) = mpsc::channel::<(usize, End)>();
    // Lowered when the server says it has too many connections from us.
    let mut max = job.connections.max(router.len()).max(1);
    let mut running: Vec<usize> = Vec::new();

    let spawn = |idx: usize, reuse: Option<Reuse>| {
        let (job, live, limiter, router, tmp, tx) = (job.clone(), live.clone(), limiter.clone(), router.clone(), tmp.clone(), tx.clone());
        live.connections.fetch_add(1, Ordering::SeqCst);
        thread::spawn(move || {
            let r = segment_worker(idx, &job, &tmp, &live, &limiter, &router, reuse);
            live.connections.fetch_sub(1, Ordering::SeqCst);
            let _ = tx.send((idx, r));
        });
    };
    let next_unstarted = |running: &Vec<usize>| {
        let segs = live.segments.lock().unwrap();
        (0..segs.len()).find(|i| !segs[*i].finished() && !running.contains(i))
    };

    while running.len() < max {
        let Some(idx) = next_unstarted(&running) else { break };
        let seg_fresh = live.segments.lock().unwrap()[idx].done == 0;
        let reuse = if idx == 0 && seg_fresh { first.take() } else { None };
        spawn(idx, reuse);
        running.push(idx);
    }
    drop(first);

    let mut err: Option<String> = None;
    while !running.is_empty() {
        let (idx, res) = rx.recv().map_err(|_| "Internal error".to_string())?;
        running.retain(|i| *i != idx);
        match res {
            End::Failed(e) => {
                if err.is_none() {
                    err = Some(e);
                    // Stop the other parts so the download can be resumed cleanly.
                    live.stop.store(true, Ordering::SeqCst);
                }
            }
            End::Busy => {
                if live.stopped() || err.is_some() {
                    continue;
                }
                max = running.len().max(1);
                if running.is_empty() {
                    // Everyone else finished meanwhile: keep trying with this one.
                    spawn(idx, None);
                    running.push(idx);
                }
            }
            End::Done => {
                if live.stopped() || err.is_some() {
                    continue;
                }
                // Give free connections more work: waiting parts, or half of the biggest one.
                while running.len() < max {
                    let Some(n) = next_unstarted(&running).or_else(|| split_largest(live)) else { break };
                    spawn(n, None);
                    running.push(n);
                }
            }
        }
    }
    if let Some(e) = err {
        live.stop.store(false, Ordering::SeqCst);
        return Err(e);
    }
    let all_done = live.segments.lock().unwrap().iter().all(|s| s.finished());
    if !all_done {
        return Ok(false);
    }
    finish(&tmp, &job.target)?;
    Ok(true)
}

/// Wait while the server is busy, waking early when the download is paused.
fn busy_pause(live: &Live, secs: u64) {
    *live.note.lock().unwrap() = Some(format!("Server is busy, trying again in {secs} s…"));
    for _ in 0..secs * 10 {
        if live.stopped() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn segment_worker(idx: usize, job: &Job, tmp: &Path, live: &Arc<Live>, limiter: &Arc<Limiter>, router: &Arc<Router>, mut reuse: Option<Reuse>) -> End {
    let mut f = match OpenOptions::new().write(true).open(tmp) {
        Ok(f) => f,
        Err(e) => return End::Failed(format!("Cannot open file: {e}")),
    };
    let mut attempt = 0;
    loop {
        if live.stopped() {
            return End::Done;
        }
        let (pos, end) = {
            let s = &live.segments.lock().unwrap()[idx];
            if s.finished() {
                return End::Done;
            }
            (s.start + s.done, s.end)
        };
        // The network this connection uses (dropped, and so handed back, at the end of each round).
        let mut lease: Option<Lease> = None;
        // Err((server busy?, message, seconds the server asked us to wait))
        let reader: Result<Box<dyn Read + Send>, (bool, String, Option<u64>)> = match reuse.take() {
            Some((r, l)) => {
                lease = l;
                Ok(r)
            }
            None => match take_route(router) {
                Err(e) => Err((false, e, None)),
                Ok(l) => {
                    lease = l;
                    match request(job, &format!("bytes={pos}-{end}"), lease.as_ref()) {
                        Ok(resp) => {
                            let code = resp.status().as_u16();
                            if code == 206 {
                                Ok(Box::new(resp.into_body().into_reader()))
                            } else if code == 429 || code == 503 {
                                let wait = header(&resp, "retry-after").and_then(|v| v.trim().parse().ok());
                                Err((true, format!("Server is busy (answer {code})"), wait))
                            } else {
                                Err((false, format!("Server refused to resume (answer {code})"), None))
                            }
                        }
                        // The chosen network is down: rest it, the others carry on.
                        Err(e) if e.network => {
                            lease.iter().for_each(|l| l.fail());
                            Err((false, e.msg, None))
                        }
                        // Could not connect at all: many servers do this when they limit connections.
                        Err(e) => Err((true, e.msg, None)),
                    }
                }
            },
        };
        let (busy, e, wait) = match reader {
            Ok(mut r) => match pump(idx, &mut r, &mut f, pos, live, limiter, &mut attempt, lease.as_ref()) {
                Ok(()) => return End::Done,
                Err(e) => (false, e, None),
            },
            Err(x) => x,
        };
        drop(lease);
        if live.stopped() {
            return End::Done;
        }
        if busy && live.connections.load(Ordering::SeqCst) > 1 {
            // Other connections are still working: hand this part back and use fewer connections.
            return End::Busy;
        }
        let limit = if busy { job.retries * 3 + 3 } else { job.retries };
        if attempt >= limit {
            return End::Failed(format!("Connection {}: {e}", idx + 1));
        }
        attempt += 1;
        if busy {
            busy_pause(live, wait.unwrap_or(2 * attempt as u64).clamp(1, 30));
        } else {
            retry_after_failure(live, attempt, router);
        }
    }
}

/// Copy bytes for one part until it is complete. Progress resets the retry counter.
#[allow(clippy::too_many_arguments)]
fn pump(idx: usize, r: &mut Box<dyn Read + Send>, f: &mut File, pos: u64, live: &Live, limiter: &Limiter, attempt: &mut u32, lease: Option<&Lease>) -> Result<(), String> {
    f.seek(SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if live.stopped() {
            return Ok(());
        }
        let remaining = live.segments.lock().unwrap()[idx].remaining();
        if remaining == 0 {
            return Ok(());
        }
        let want = (buf.len() as u64).min(remaining) as usize;
        let n = r.read(&mut buf[..want]).map_err(|e| format!("connection lost: {e}"))?;
        if n == 0 {
            return Err("connection closed early".into());
        }
        limiter.take(n);
        f.write_all(&buf[..n]).map_err(|e| format!("write failed: {e}"))?;
        live.segments.lock().unwrap()[idx].done += n as u64;
        live.downloaded.fetch_add(n as u64, Ordering::SeqCst);
        if let Some(l) = lease {
            l.add_bytes(n as u64);
        }
        *attempt = 0;
    }
}

fn finish(tmp: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        let _ = std::fs::remove_file(target);
    }
    std::fs::rename(tmp, target).map_err(|e| format!("Could not finish file: {e}"))
}

pub fn numbered(name: &str, n: usize) -> String {
    match name.rfind('.') {
        Some(p) if p > 0 => format!("{} ({n}){}", &name[..p], &name[p..]),
        _ => format!("{name} ({n})"),
    }
}

pub fn file_name_from_url(url: &str) -> String {
    let name = url::Url::parse(url)
        .ok()
        .and_then(|u| u.path_segments().and_then(|mut s| s.next_back().map(|x| x.to_string())))
        .filter(|s| !s.is_empty())
        .map(|s| percent_decode(&s))
        .unwrap_or_else(|| "download".to_string());
    sanitize(&name)
}

pub fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '_' } else { c })
        .collect();
    let t = cleaned.trim().trim_matches('.').to_string();
    if t.is_empty() { "download".into() } else { t }
}

/// SHA-256 of a file as lowercase hex.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
#[path = "engine_tests.rs"]
pub(crate) mod tests;
