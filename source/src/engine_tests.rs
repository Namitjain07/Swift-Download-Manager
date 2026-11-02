//! Download tests against a small local web server (no internet needed).

use super::*;
use std::io::{BufRead, BufReader};
use std::collections::HashMap;
use std::net::{IpAddr, TcpListener, TcpStream};
use std::sync::atomic::AtomicUsize;

pub(crate) const SIZE: usize = 5 * 1024 * 1024 + 12345;

fn data() -> Arc<Vec<u8>> {
    let mut x: u32 = 12345;
    Arc::new(
        (0..SIZE)
            .map(|_| {
                x = x.wrapping_mul(1103515245).wrapping_add(12345);
                (x >> 16) as u8
            })
            .collect(),
    )
}

pub(crate) struct Server {
    port: u16,
    peak: Arc<AtomicUsize>,
    pub(crate) data: Arc<Vec<u8>>,
    requests: Arc<AtomicUsize>,
    net: Arc<Net>,
}

/// Which addresses connected, and how much each was sent.
#[derive(Default)]
struct Net {
    peers: Mutex<Vec<IpAddr>>,
    sent: Mutex<HashMap<IpAddr, u64>>,
}

impl Server {
    fn peers(&self) -> Vec<IpAddr> {
        self.net.peers.lock().unwrap().clone()
    }
    fn sent_to(&self, ip: &str) -> u64 {
        self.net.sent.lock().unwrap().get(&ip.parse::<IpAddr>().unwrap()).copied().unwrap_or(0)
    }
}

impl Server {
    pub(crate) fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.port, path)
    }
}

pub(crate) fn server() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let data = data();
    let requests = Arc::new(AtomicUsize::new(0));
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let net = Arc::new(Net::default());
    let (d, r, pk, nt) = (data.clone(), requests.clone(), peak.clone(), net.clone());
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (d, r, a, pk, nt) = (d.clone(), r.clone(), active.clone(), pk.clone(), nt.clone());
            thread::spawn(move || {
                let now = a.fetch_add(1, Ordering::SeqCst) + 1;
                pk.fetch_max(now, Ordering::SeqCst);
                handle(stream, &d, &r, now, &nt);
                a.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    Server { port, peak, data, requests, net }
}

fn handle(mut s: TcpStream, data: &[u8], requests: &AtomicUsize, active_now: usize, net: &Net) {
    let n_req = requests.fetch_add(1, Ordering::SeqCst);
    let peer = s.peer_addr().unwrap().ip();
    net.peers.lock().unwrap().push(peer);
    let mut reader = BufReader::new(s.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let mut range: Option<(u64, Option<u64>)> = None;
    let mut auth = String::new();
    let mut cookie = String::new();
    let mut agent = String::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
            break;
        }
        let lower = h.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("range: bytes=") {
            let v = v.trim();
            let (a, b) = v.split_once('-').unwrap();
            range = Some((a.parse().unwrap(), b.parse().ok()));
        }
        if lower.starts_with("cookie:") {
            cookie = h["cookie:".len()..].trim().to_string();
        }
        if lower.starts_with("user-agent:") {
            agent = h["user-agent:".len()..].trim().to_string();
        }
        if lower.starts_with("authorization:") {
            auth = h["authorization:".len()..].trim().to_string();
        }
    }
    let total = data.len() as u64;
    let send = |s: &mut TcpStream, head: String, body: &[u8], slow: bool, cut: Option<usize>| {
        let _ = s.write_all(head.as_bytes());
        let body = match cut {
            Some(c) => &body[..c.min(body.len())],
            None => body,
        };
        for chunk in body.chunks(32 * 1024) {
            if s.write_all(chunk).is_err() {
                return;
            }
            *net.sent.lock().unwrap().entry(peer).or_default() += chunk.len() as u64;
            if slow {
                thread::sleep(Duration::from_millis(15));
            }
        }
    };
    if path == "/missing" {
        send(&mut s, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(), &[], false, None);
        return;
    }
    // Limited: like many real servers, at most 3 connections at once.
    if path.starts_with("/limited") && active_now > 3 {
        send(&mut s, "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(), &[], false, None);
        return;
    }
    if path.starts_with("/members") && (cookie != "session=abc123; lang=en" || !agent.starts_with("Mozilla/5.0")) {
        send(&mut s, "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(), &[], false, None);
        return;
    }
    if path.starts_with("/auth") && auth != "Basic dXNlcjpwYXNz" {
        send(&mut s, "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(), &[], false, None);
        return;
    }
    // "/pernet": the network that comes from 127.0.0.2 is slow, the others are fast.
    let slow = path.starts_with("/slow") || path.starts_with("/limited") || (path.starts_with("/pernet") && peer == "127.0.0.2".parse::<IpAddr>().unwrap());
    // Flaky: the first 6 requests break after 100 KB.
    let cut = if path.starts_with("/flaky") && n_req < 6 { Some(100 * 1024) } else { None };
    let extra = if path.starts_with("/named") {
        "Content-Disposition: attachment; filename=\"report final.pdf\"\r\n"
    } else {
        ""
    };
    match range {
        Some((a, b)) if !path.starts_with("/norange") => {
            let b = b.unwrap_or(total - 1).min(total - 1);
            let body = &data[a as usize..=b as usize];
            let head = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {a}-{b}/{total}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
                body.len()
            );
            send(&mut s, head, body, slow, cut);
        }
        _ => {
            let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\n{extra}Connection: close\r\n\r\n");
            send(&mut s, head, data, slow, cut);
        }
    }
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sdm-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn job(url: String, target: PathBuf) -> Job {
    Job { url, target, connections: 8, retries: 3, ..Default::default() }
}

fn go(job: Job, live: &Arc<Live>, limiter: &Arc<Limiter>) {
    start(job, live.clone(), limiter.clone(), eframe::egui::Context::default());
}

fn wait(live: &Live, secs: u64) -> Status {
    let t = Instant::now();
    thread::sleep(Duration::from_millis(30));
    while live.status().is_active() && t.elapsed().as_secs() < secs {
        thread::sleep(Duration::from_millis(20));
    }
    live.status()
}

fn fresh() -> Arc<Live> {
    Live::new(0, 0, Status::Paused, Vec::new(), false)
}

#[test]
fn split_download_is_byte_exact() {
    let srv = server();
    let out = dir("split").join("file.bin");
    let live = fresh();
    go(job(srv.url("/range/file.bin"), out.clone()), &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Complete, "{:?}", live.error.lock().unwrap());
    assert!(live.resumable.load(Ordering::SeqCst));
    assert!(live.segments.lock().unwrap().len() >= 8);
    assert_eq!(std::fs::read(&out).unwrap(), *srv.data);
    assert!(!temp_path(&out).exists());
}

#[test]
fn pause_and_resume_is_byte_exact() {
    let srv = server();
    let out = dir("pause").join("file.bin");
    let live = fresh();
    let limiter = Arc::new(Limiter::new());
    let j = job(srv.url("/slow/file.bin"), out.clone());
    go(j.clone(), &live, &limiter);
    let t = Instant::now();
    while live.downloaded.load(Ordering::SeqCst) < 1024 * 1024 && t.elapsed().as_secs() < 20 {
        thread::sleep(Duration::from_millis(10));
    }
    live.stop.store(true, Ordering::SeqCst);
    assert_eq!(wait(&live, 20), Status::Paused);
    let at_pause = live.downloaded.load(Ordering::SeqCst);
    assert!(at_pause > 0 && at_pause < SIZE as u64);
    assert!(temp_path(&out).exists());
    // Resume: only the missing bytes are fetched.
    go(j, &live, &limiter);
    assert_eq!(wait(&live, 60), Status::Complete, "{:?}", live.error.lock().unwrap());
    assert_eq!(live.downloaded.load(Ordering::SeqCst), SIZE as u64);
    assert_eq!(std::fs::read(&out).unwrap(), *srv.data);
}

#[test]
fn server_without_resume_uses_one_connection() {
    let srv = server();
    let out = dir("norange").join("file.bin");
    let live = fresh();
    go(job(srv.url("/norange/file.bin"), out.clone()), &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Complete);
    assert!(!live.resumable.load(Ordering::SeqCst));
    assert!(live.segments.lock().unwrap().is_empty());
    assert_eq!(std::fs::read(&out).unwrap(), *srv.data);
}

#[test]
fn missing_file_reports_error() {
    let srv = server();
    let out = dir("missing").join("x.bin");
    let live = fresh();
    go(job(srv.url("/missing"), out.clone()), &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Error);
    assert!(live.error.lock().unwrap().as_deref().unwrap().contains("404"));
    assert!(!out.exists());
}

#[test]
fn broken_connections_are_retried() {
    let srv = server();
    let out = dir("flaky").join("file.bin");
    let live = fresh();
    go(job(srv.url("/flaky/file.bin"), out.clone()), &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 60), Status::Complete, "{:?}", live.error.lock().unwrap());
    assert_eq!(std::fs::read(&out).unwrap(), *srv.data);
    assert!(srv.requests.load(Ordering::SeqCst) > 8);
}

#[test]
fn server_suggested_name_is_used() {
    let srv = server();
    let d = dir("named");
    let live = fresh();
    let mut j = job(srv.url("/named/download?id=7"), d.join("download_id=7"));
    j.accept_server_name = true;
    go(j, &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Complete);
    assert_eq!(live.file_name.lock().unwrap().as_deref(), Some("report final.pdf"));
    assert_eq!(std::fs::read(d.join("report final.pdf")).unwrap(), *srv.data);
}

#[test]
fn login_is_sent() {
    let srv = server();
    let d = dir("auth");
    let live = fresh();
    go(job(srv.url("/auth/file.bin"), d.join("no.bin")), &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Error);
    let live = fresh();
    let mut j = job(srv.url("/auth/file.bin"), d.join("yes.bin"));
    j.user = "user".into();
    j.pass = "pass".into();
    go(j, &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Complete, "{:?}", live.error.lock().unwrap());
    assert_eq!(std::fs::read(d.join("yes.bin")).unwrap(), *srv.data);
}

#[test]
fn speed_limit_slows_download() {
    let srv = server();
    let out = dir("limit").join("file.bin");
    let live = fresh();
    let limiter = Arc::new(Limiter::new());
    limiter.bytes_per_sec.store(2 * 1024 * 1024, Ordering::SeqCst);
    let t = Instant::now();
    go(job(srv.url("/range/file.bin"), out.clone()), &live, &limiter);
    assert_eq!(wait(&live, 60), Status::Complete);
    // 5 MB at 2 MB/s takes at least about 2 seconds.
    assert!(t.elapsed().as_secs_f64() > 1.8, "too fast: {:?}", t.elapsed());
    assert_eq!(std::fs::read(&out).unwrap(), *srv.data);
}

#[test]
fn free_connections_split_the_biggest_part() {
    let live = Live::new(0, 0, Status::Paused, vec![Segment { start: 0, end: 4_000_000 - 1, done: 1_000_000 }], true);
    let idx = split_largest(&live).unwrap();
    let segs = live.segments.lock().unwrap();
    assert_eq!(idx, 1);
    assert_eq!(segs[0].end + 1, segs[1].start);
    assert_eq!(segs[1].end, 4_000_000 - 1);
    assert_eq!(segs[0].remaining() + segs[1].remaining(), 3_000_000);
}

#[test]
fn helpers() {
    assert_eq!(base64_encode(b"user:pass"), "dXNlcjpwYXNz");
    assert_eq!(base64_encode(b"ab"), "YWI=");
    assert_eq!(name_from_disposition("attachment; filename=\"a b.zip\""), Some("a b.zip".into()));
    assert_eq!(name_from_disposition("attachment; filename*=UTF-8''r%C3%A9sum%C3%A9.pdf"), Some("résumé.pdf".into()));
    assert_eq!(file_name_from_url("https://x.org/dl/My%20File.iso?x=1"), "My File.iso");
    assert_eq!(sanitize("a/b:c?.txt"), "a_b_c_.txt");
    assert_eq!(numbered("a.zip", 2), "a (2).zip");
}

#[test]
fn checksum() {
    let d = dir("sha");
    let p = d.join("abc.txt");
    std::fs::write(&p, b"abc").unwrap();
    assert_eq!(sha256_file(&p).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}

#[test]
fn busy_server_gets_fewer_connections() {
    let srv = server();
    let out = dir("limited").join("file.bin");
    let live = fresh();
    go(job(srv.url("/limited/file.bin"), out.clone()), &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 90), Status::Complete, "{:?}", live.error.lock().unwrap());
    assert_eq!(std::fs::read(&out).unwrap(), *srv.data);
    assert!(srv.peak.load(Ordering::SeqCst) >= 4, "the app did try more connections first");
}

#[test]
fn browser_cookies_and_identity_are_sent() {
    let srv = server();
    let d = dir("cookies");
    let live = fresh();
    go(job(srv.url("/members/file.bin"), d.join("no.bin")), &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Error);
    let live = fresh();
    let mut j = job(srv.url("/members/file.bin"), d.join("yes.bin"));
    j.cookies = "session=abc123; lang=en".into();
    j.user_agent = "Mozilla/5.0 (Test Browser)".into();
    go(j, &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Complete, "{:?}", live.error.lock().unwrap());
    assert_eq!(std::fs::read(d.join("yes.bin")).unwrap(), *srv.data);
}

#[test]
fn per_download_limit_works_with_overall_limit() {
    let srv = server();
    let out = dir("itemlimit").join("file.bin");
    let overall = Arc::new(Limiter::new());
    let own = Arc::new(Limiter::chained(overall.clone()));
    own.bytes_per_sec.store(2 * 1024 * 1024, Ordering::SeqCst);
    let live = fresh();
    let t = Instant::now();
    go(job(srv.url("/range/file.bin"), out.clone()), &live, &own);
    assert_eq!(wait(&live, 60), Status::Complete);
    assert!(t.elapsed().as_secs_f64() > 1.8, "too fast: {:?}", t.elapsed());
    assert_eq!(std::fs::read(&out).unwrap(), *srv.data);
}

// --- several networks at once ---------------------------------------------------------------------

use crate::netroute::Route;

fn loopback(ip: &str) -> Route {
    Route { name: "lo".into(), addr: ip.parse().unwrap(), index: None }
}

/// Whether this computer lets us use other 127.x.x.x addresses as if they were separate networks.
fn aliases_work() -> bool {
    TcpListener::bind("127.0.0.2:0").is_ok() && TcpListener::bind("127.0.0.3:0").is_ok()
}

#[test]
fn download_over_two_networks_is_byte_exact_and_uses_both() {
    if !aliases_work() {
        eprintln!("skipped: no 127.0.0.2 / 127.0.0.3 here");
        return;
    }
    let srv = server();
    let out = dir("two-nets").join("file.bin");
    let live = fresh();
    let j = Job { routes: vec![loopback("127.0.0.2"), loopback("127.0.0.3")], ..job(srv.url("/range/file.bin"), out.clone()) };
    go(j, &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Complete, "{:?}", live.error.lock().unwrap());
    assert_eq!(std::fs::read(&out).unwrap(), *srv.data);
    let peers = srv.peers();
    assert!(peers.contains(&"127.0.0.2".parse().unwrap()), "{peers:?}");
    assert!(peers.contains(&"127.0.0.3".parse().unwrap()), "{peers:?}");
    assert!(!peers.contains(&"127.0.0.1".parse().unwrap()), "nothing may go out the default way: {peers:?}");
}

#[test]
fn a_faster_network_carries_more_of_the_file() {
    if !aliases_work() {
        eprintln!("skipped: no 127.0.0.2 / 127.0.0.3 here");
        return;
    }
    let srv = server();
    let out = dir("fast-slow").join("file.bin");
    let live = fresh();
    // Two big parts (the small test file would otherwise be cut into parts too small to share out again).
    let j = Job { routes: vec![loopback("127.0.0.2"), loopback("127.0.0.3")], connections: 2, ..job(srv.url("/pernet/file.bin"), out.clone()) };
    go(j, &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 60), Status::Complete, "{:?}", live.error.lock().unwrap());
    assert_eq!(std::fs::read(&out).unwrap(), *srv.data);
    let (slow, fast) = (srv.sent_to("127.0.0.2"), srv.sent_to("127.0.0.3"));
    assert!(slow > 0, "the slow network still helps");
    assert!(fast > slow * 2, "the fast network does most of the work: slow {slow}, fast {fast}");
}

#[test]
fn a_network_that_does_not_work_is_left_out_and_the_download_still_finishes() {
    if !aliases_work() {
        eprintln!("skipped: no 127.0.0.2 here");
        return;
    }
    let srv = server();
    let out = dir("one-bad").join("file.bin");
    let live = fresh();
    // 203.0.113.7 is reserved for documentation: no network card on this computer has it.
    let gone = Route { name: "unplugged".into(), addr: "203.0.113.7".parse().unwrap(), index: None };
    let j = Job { routes: vec![gone, loopback("127.0.0.2")], ..job(srv.url("/range/file.bin"), out.clone()) };
    go(j, &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Complete, "{:?}", live.error.lock().unwrap());
    assert_eq!(std::fs::read(&out).unwrap(), *srv.data);
    assert!(srv.peers().iter().all(|p| *p == "127.0.0.2".parse::<IpAddr>().unwrap()));
}

#[test]
fn with_no_working_network_the_download_says_so() {
    let srv = server();
    let out = dir("all-bad").join("file.bin");
    let live = fresh();
    let gone = Route { name: "unplugged".into(), addr: "203.0.113.7".parse().unwrap(), index: None };
    let j = Job { routes: vec![gone], retries: 1, ..job(srv.url("/range/file.bin"), out.clone()) };
    go(j, &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Error);
    assert!(srv.peers().is_empty(), "nothing may fall back to the normal connection");
    assert!(!out.exists());
}

#[test]
fn a_server_can_refuse_to_fetch_from_private_addresses() {
    let srv = server();
    let out = dir("private").join("file.bin");
    let live = fresh();
    // The test server is on 127.0.0.1, which is exactly what a shared server must not be made to reach.
    // With the usual retries: a refusal like this will not get better, so it must not wait and try again.
    go(Job { block_private: true, ..job(srv.url("/range/file.bin"), out.clone()) }, &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 5), Status::Error, "gave up straight away");
    assert!(live.error.lock().unwrap().clone().unwrap_or_default().contains("private"), "{:?}", live.error.lock().unwrap());
    assert!(srv.peers().is_empty(), "no connection may be made");
    assert!(!out.exists());
}

#[test]
fn private_addresses_are_fine_when_not_refused() {
    let srv = server();
    let out = dir("not-private").join("file.bin");
    let live = fresh();
    go(job(srv.url("/range/file.bin"), out.clone()), &live, &Arc::new(Limiter::new()));
    assert_eq!(wait(&live, 30), Status::Complete);
}
