use super::*;
use crate::web::auth::Users;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;

type Log = Arc<Mutex<Vec<(Command, Option<Who>)>>>;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sdm-web-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A stand-in for the app: records who asked what, and answers from `answer`.
fn fake_app(answer: impl Fn(&Command) -> Result<Value, String> + Send + 'static) -> (Sender<Request>, Log) {
    let (tx, rx) = mpsc::channel::<Request>();
    let log: Log = Arc::default();
    let seen = log.clone();
    std::thread::spawn(move || {
        for req in rx {
            seen.lock().unwrap().push((req.cmd.clone(), req.who.clone()));
            let _ = req.reply.send(answer(&req.cmd));
        }
    });
    (tx, log)
}

fn ok_everything(_: &Command) -> Result<Value, String> {
    Ok(json!({ "fine": true }))
}

struct Site {
    handle: Handle,
    log: Log,
    dir: PathBuf,
}

fn site_with(name: &str, mode: Mode, users: &[(&str, &str, bool)], answer: impl Fn(&Command) -> Result<Value, String> + Send + 'static) -> Site {
    let dir = scratch(name);
    let mut store = Users::load(&dir);
    for (n, p, a) in users {
        store.set(n, p, *a).unwrap();
    }
    let (tx, log) = fake_app(answer);
    let cfg = Config { bind: "127.0.0.1".parse().unwrap(), port: 0, mode, dir: dir.clone(), secure_cookie: false };
    let handle = start(cfg, tx, egui::Context::default()).unwrap();
    Site { handle, log, dir }
}

fn site(name: &str, mode: Mode, users: &[(&str, &str, bool)]) -> Site {
    site_with(name, mode, users, ok_everything)
}

struct Answer {
    code: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
    fn header(&self, name: &str) -> &str {
        self.headers.get(name).map(String::as_str).unwrap_or("")
    }
    fn cookie(&self) -> Option<String> {
        self.header("set-cookie").split(';').next().filter(|c| c.contains('=') && !c.ends_with('=')).map(str::to_string)
    }
}

fn send(site: &Site, method: &str, path: &str, extra: &[(&str, &str)], body: &str) -> Answer {
    let port = site.handle.addr.port();
    let mut text = format!("{method} {path} HTTP/1.1\r\n");
    if !extra.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        text += &format!("Host: 127.0.0.1:{port}\r\n");
    }
    for (k, v) in extra {
        text += &format!("{k}: {v}\r\n");
    }
    text += &format!("Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    let mut s = TcpStream::connect(site.handle.addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(text.as_bytes()).unwrap();
    let mut raw = Vec::new();
    let _ = s.read_to_end(&mut raw);
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").expect("a whole answer");
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let mut lines = head.lines();
    let code = lines.next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
    let headers = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string())).collect();
    Answer { code, headers, body: raw[split + 4..].to_vec() }
}

fn get(site: &Site, path: &str, cookie: &str) -> Answer {
    send(site, "GET", path, &[("Cookie", cookie)], "")
}

fn post(site: &Site, path: &str, cookie: &str, body: &str) -> Answer {
    send(site, "POST", path, &[("Cookie", cookie), ("X-SDM-Request", "1"), ("Content-Type", "application/json")], body)
}

/// Open the personal link and return the session cookie.
fn personal_session(site: &Site) -> String {
    let a = send(site, "GET", &format!("/?key={}", site.handle.key), &[], "");
    assert_eq!(a.code, 302);
    a.cookie().expect("a session cookie")
}

fn login(site: &Site, user: &str, password: &str) -> Answer {
    post(site, "/api/login", "", &json!({ "user": user, "password": password }).to_string())
}

// ---------------------------------------------------------------- personal

#[test]
fn the_personal_page_needs_its_key_and_swaps_it_for_a_cookie() {
    let s = site("personal-key", Mode::Personal, &[]);
    assert_eq!(get(&s, "/api/status", "").code, 401);
    assert_eq!(send(&s, "GET", "/?key=wrong", &[], "").code, 403);
    assert_eq!(send(&s, "GET", &format!("/?key={}", "0".repeat(64)), &[], "").code, 403);

    let a = send(&s, "GET", &format!("/?key={}", s.handle.key), &[], "");
    assert_eq!((a.code, a.header("location")), (302, "/"), "the key is taken out of the address");
    let cookie = a.cookie().unwrap();
    assert!(a.header("set-cookie").contains("HttpOnly") && a.header("set-cookie").contains("SameSite=Strict"));

    let status = get(&s, "/api/status", &cookie);
    assert_eq!((status.code, status.json()["fine"].clone()), (200, json!(true)));
    let log = s.log.lock().unwrap();
    assert_eq!(log[0].1, Some(Who { name: String::new(), admin: true }), "the personal page is not tied to a person");
}

#[test]
fn scripts_can_use_the_key_as_a_bearer_token() {
    let s = site("personal-bearer", Mode::Personal, &[]);
    let a = send(&s, "GET", "/api/status", &[("Authorization", &format!("Bearer {}", s.handle.key))], "");
    assert_eq!(a.code, 200);
    assert_eq!(send(&s, "GET", "/api/status", &[("Authorization", "Bearer nope")], "").code, 401);
}

#[test]
fn the_personal_page_refuses_other_names_and_other_sites() {
    let s = site("personal-host", Mode::Personal, &[]);
    let cookie = personal_session(&s);
    let evil = send(&s, "GET", "/api/status", &[("Host", "evil.example:80"), ("Cookie", &cookie)], "");
    assert_eq!(evil.code, 403, "DNS rebinding");
    let port = s.handle.addr.port();
    let cross = send(&s, "GET", "/api/status", &[("Origin", "https://evil.example"), ("Cookie", &cookie)], "");
    assert_eq!(cross.code, 403, "a page from another site");
    let same = send(&s, "GET", "/api/status", &[("Origin", &format!("http://127.0.0.1:{port}")), ("Cookie", &cookie)], "");
    assert_eq!(same.code, 200);
    let name = send(&s, "GET", "/api/status", &[("Host", &format!("localhost:{port}")), ("Cookie", &cookie)], "");
    assert_eq!(name.code, 200);
}

#[test]
fn changes_need_the_custom_header() {
    let s = site("csrf", Mode::Personal, &[]);
    let cookie = personal_session(&s);
    let plain = send(&s, "POST", "/api/downloads", &[("Cookie", &cookie)], r#"{"url":"https://e.com/a.zip"}"#);
    assert_eq!(plain.code, 403);
    let del = send(&s, "DELETE", "/api/downloads/3", &[("Cookie", &cookie)], "");
    assert_eq!(del.code, 403);
    assert!(s.log.lock().unwrap().is_empty(), "nothing reached the app");
    assert_eq!(post(&s, "/api/downloads", &cookie, r#"{"url":"https://e.com/a.zip"}"#).code, 200);
}

#[test]
fn the_api_turns_into_commands() {
    let s = site("api", Mode::Personal, &[]);
    let c = personal_session(&s);
    get(&s, "/api/downloads?status=paused&q=iso%20x&limit=7", &c);
    post(&s, "/api/downloads", &c, r#"{"url":"https://e.com/a.zip","name":"a.zip","category":"Programs","queue":"Main Queue","start":false}"#);
    get(&s, "/api/downloads/12", &c);
    post(&s, "/api/downloads/12/pause", &c, "");
    post(&s, "/api/downloads/12/resume", &c, "");
    send(&s, "DELETE", "/api/downloads/12", &[("Cookie", &c), ("X-SDM-Request", "1")], "");
    post(&s, "/api/queues/Night%20Queue/start", &c, "");
    post(&s, "/api/queues/Night%20Queue/stop", &c, "");
    get(&s, "/api/queues", &c);
    let cmds: Vec<Command> = s.log.lock().unwrap().iter().map(|(c, _)| c.clone()).collect();
    assert_eq!(
        cmds,
        vec![
            Command::ListDownloads { status: Some("paused".into()), query: Some("iso x".into()), limit: Some(7) },
            Command::AddDownload { url: "https://e.com/a.zip".into(), name: Some("a.zip".into()), category: Some("Programs".into()), queue: Some("Main Queue".into()), start: Some(false) },
            Command::GetDownload { id: 12 },
            Command::Pause { id: 12 },
            Command::Resume { id: 12 },
            Command::Remove { id: 12 },
            Command::StartQueue { name: "Night Queue".into() },
            Command::StopQueue { name: "Night Queue".into() },
            Command::ListQueues,
        ]
    );
}

#[test]
fn bad_input_is_answered_politely() {
    let s = site_with("errors", Mode::Personal, &[], |c| match c {
        Command::GetDownload { .. } => Err("No download with id 9. Use list_downloads to see the ids.".into()),
        _ => Err("nope".into()),
    });
    let c = personal_session(&s);
    assert_eq!(get(&s, "/api/downloads/9", &c).code, 404);
    assert_eq!(get(&s, "/api/downloads/abc", &c).code, 404);
    assert_eq!(get(&s, "/api/status", &c).code, 400);
    assert_eq!(post(&s, "/api/downloads", &c, "{}").code, 400);
    assert_eq!(post(&s, "/api/downloads", &c, "not json").code, 400);
    assert_eq!(get(&s, "/nothing/here", &c).code, 404);
    assert_eq!(send(&s, "PUT", "/api/status", &[("Cookie", &c)], "").code, 405);
}

#[test]
fn the_page_is_served_with_strict_headers() {
    let s = site("page", Mode::Personal, &[]);
    let a = get(&s, "/", "");
    assert_eq!(a.code, 200);
    assert!(a.header("content-type").starts_with("text/html"));
    assert!(a.header("content-security-policy").contains("default-src 'none'"));
    assert_eq!(a.header("x-frame-options"), "DENY");
    assert_eq!(a.header("x-content-type-options"), "nosniff");
    assert!(!a.body.is_empty());
}

// ---------------------------------------------------------------- shared

#[test]
fn people_log_in_and_the_app_hears_who_they_are() {
    let s = site("shared", Mode::Shared, &[("anna", "correct horse", false), ("root", "battery staple", true)]);
    assert_eq!(get(&s, "/api/status", "").json()["shared"], true, "the page is told to show a login");
    assert_eq!(login(&s, "anna", "wrong").code, 401);
    assert_eq!(login(&s, "nobody", "correct horse").code, 401);

    let a = login(&s, "anna", "correct horse");
    assert_eq!(a.code, 200);
    let cookie = a.cookie().unwrap();
    assert_eq!(get(&s, "/api/me", &cookie).json(), json!({ "user": "anna", "admin": false, "shared": true }));

    // A person cannot claim to be somebody else.
    post(&s, "/api/downloads", &cookie, r#"{"url":"https://e.com/a.zip","owner":"root","who":"root","user":"root"}"#);
    let log = s.log.lock().unwrap();
    let (cmd, who) = log.last().unwrap();
    assert!(matches!(cmd, Command::AddDownload { .. }));
    assert_eq!(*who, Some(Who { name: "anna".into(), admin: false }));
    drop(log);

    let root = login(&s, "root", "battery staple").cookie().unwrap();
    assert_eq!(get(&s, "/api/me", &root).json()["admin"], true);
}

#[test]
fn logging_out_ends_the_session() {
    let s = site("logout", Mode::Shared, &[("anna", "correct horse", false)]);
    let cookie = login(&s, "anna", "correct horse").cookie().unwrap();
    assert_eq!(get(&s, "/api/status", &cookie).code, 200);
    assert_eq!(post(&s, "/api/logout", &cookie, "").code, 200);
    assert_eq!(get(&s, "/api/status", &cookie).code, 401);
}

#[test]
fn a_shared_server_has_no_key_door() {
    let s = site("shared-key", Mode::Shared, &[("anna", "correct horse", false)]);
    assert_eq!(send(&s, "GET", &format!("/?key={}", s.handle.key), &[], "").code, 403);
    let bearer = send(&s, "GET", "/api/status", &[("Authorization", &format!("Bearer {}", s.handle.key))], "");
    assert_eq!(bearer.code, 401);
}

#[test]
fn too_many_wrong_passwords_are_slowed_down() {
    let s = site("throttle", Mode::Shared, &[("anna", "correct horse", false)]);
    for _ in 0..5 {
        assert_eq!(login(&s, "anna", "guess").code, 401);
    }
    let blocked = login(&s, "anna", "correct horse");
    assert_eq!(blocked.code, 429, "even the right password waits");
    assert!(!blocked.header("retry-after").is_empty());
}

#[test]
fn a_personal_page_has_no_login() {
    let s = site("no-login", Mode::Personal, &[]);
    assert_eq!(login(&s, "anna", "correct horse").code, 404);
}

// ---------------------------------------------------------------- files

fn file_answer(path: &Path, name: &'static str) -> impl Fn(&Command) -> Result<Value, String> + Send + 'static {
    let path = path.display().to_string();
    move |c| match c {
        Command::Locate { id: 1 } => Ok(json!({ "path": path, "name": name, "size": 26 })),
        Command::Locate { .. } => Err("No download with id 2. Use list_downloads to see the ids.".into()),
        _ => Ok(json!({})),
    }
}

#[test]
fn a_finished_file_is_sent_whole_or_in_pieces() {
    let dir = scratch("files-src");
    let path = dir.join("alphabet.txt");
    std::fs::write(&path, b"abcdefghijklmnopqrstuvwxyz").unwrap();
    let s = site_with("files", Mode::Personal, &[], file_answer(&path, "my \"file\".txt"));
    let c = personal_session(&s);

    let whole = get(&s, "/files/1", &c);
    assert_eq!((whole.code, whole.body.as_slice()), (200, &b"abcdefghijklmnopqrstuvwxyz"[..]));
    assert_eq!(whole.header("content-type"), "application/octet-stream", "never shown inline");
    assert!(whole.header("content-disposition").starts_with("attachment;"));
    assert_eq!(whole.header("accept-ranges"), "bytes");

    let part = send(&s, "GET", "/files/1", &[("Cookie", &c), ("Range", "bytes=5-9")], "");
    assert_eq!((part.code, part.body.as_slice()), (206, &b"fghij"[..]));
    assert_eq!(part.header("content-range"), "bytes 5-9/26");

    let tail = send(&s, "GET", "/files/1", &[("Cookie", &c), ("Range", "bytes=-3")], "");
    assert_eq!(tail.body, b"xyz");
    let beyond = send(&s, "GET", "/files/1", &[("Cookie", &c), ("Range", "bytes=40-")], "");
    assert_eq!(beyond.code, 416);
    assert_eq!(beyond.header("content-range"), "bytes */26");

    let head = send(&s, "HEAD", "/files/1", &[("Cookie", &c)], "");
    assert_eq!((head.code, head.header("content-length"), head.body.len()), (200, "26", 0));
}

#[test]
fn files_need_a_login_and_ownership_is_up_to_the_app() {
    let dir = scratch("files-auth");
    let path = dir.join("a.bin");
    std::fs::write(&path, b"secret").unwrap();
    let s = site_with("files-auth-site", Mode::Shared, &[("anna", "correct horse", false)], file_answer(&path, "a.bin"));
    assert_eq!(get(&s, "/files/1", "").code, 401, "nobody gets a file without signing in");
    let c = login(&s, "anna", "correct horse").cookie().unwrap();
    assert_eq!(get(&s, "/files/1", &c).code, 200);
    assert_eq!(get(&s, "/files/2", &c).code, 404, "somebody else's download looks like a missing one");
    assert_eq!(get(&s, "/files/abc", &c).code, 404);
    assert_eq!(get(&s, "/files/../../etc/passwd", &c).code, 404);
    assert_eq!(get(&s, "/files/1/../..", &c).code, 404);
}

#[test]
fn the_path_of_a_file_comes_only_from_the_app() {
    let dir = scratch("files-path");
    let s = site_with("files-path", Mode::Personal, &[], move |_| Ok(json!({ "path": dir.join("gone.bin").display().to_string(), "name": "gone.bin", "size": 1 })));
    let c = personal_session(&s);
    assert_eq!(get(&s, "/files/1?path=/etc/passwd", &c).code, 404, "the file the app named is missing");
}

#[test]
fn dropping_the_handle_closes_the_door() {
    let s = site("stop", Mode::Personal, &[]);
    let addr = s.handle.addr;
    assert!(TcpStream::connect(addr).is_ok());
    drop(s.handle);
    std::thread::sleep(Duration::from_millis(300));
    assert!(TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_err());
    let _ = std::fs::remove_dir_all(s.dir);
}
