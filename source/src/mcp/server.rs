//! `swift_download_manager --mcp`: an MCP server on stdin/stdout.
//!
//! Only JSON-RPC goes to stdout; everything else goes to stderr.

use super::command::Command;
use super::control::{self, CallError};
use super::error::{ErrorKind, ToolError};
use super::lock::StateLock;
use super::policy::{Level, Policy};
use super::protocol::{self, Options};
use crate::app::{default_config_dir, App};
use serde_json::Value;
use std::io::{self, BufRead, Write};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// What the server loop reacts to.
pub enum Event {
    /// One line from the client.
    Line(String),
    /// A line longer than the limit was dropped.
    TooLong,
    /// Nothing arrived for a while: time for housekeeping.
    Idle,
}

/// Whatever actually runs the commands.
pub trait Backend {
    fn run(&mut self, cmd: Command) -> Result<Value, ToolError>;
    fn idle(&mut self);
}

/// The server loop: every line in gives at most one line out.
#[cfg(test)]
pub fn serve(events: impl Iterator<Item = Event>, out: &mut dyn Write, backend: &mut dyn Backend) -> io::Result<()> {
    serve_with(events, out, backend, &Options::default())
}

pub fn serve_with(events: impl Iterator<Item = Event>, out: &mut dyn Write, backend: &mut dyn Backend, options: &Options) -> io::Result<()> {
    for event in events {
        let answer = match event {
            Event::Idle => {
                backend.idle();
                None
            }
            Event::TooLong => Some(protocol::error(Value::Null, -32600, "Request too large: a message may be at most 1 MiB")),
            Event::Line(line) => protocol::handle_line_with(&line, options, &mut |cmd| {
                // A bug in one command must not end the whole session: the assistant gets an error and can carry on.
                match std::panic::catch_unwind(AssertUnwindSafe(|| backend.run(cmd))) {
                    Ok(result) => result,
                    Err(_) => {
                        eprintln!("swift-download-manager: a command failed unexpectedly (a bug); the session goes on");
                        Err(ToolError::new(ErrorKind::Internal, "The app failed while doing that. Nothing was changed that cannot be seen with get_status.").hint("Try again; if it keeps happening, tell the person."))
                    }
                }
            }),
        };
        if let Some(answer) = answer {
            // Compact JSON never contains a newline, so one message is one line.
            writeln!(out, "{answer}")?;
            out.flush()?;
        }
    }
    Ok(())
}

/// Lines of input as text. Bytes that are not UTF-8 are replaced, so one bad line cannot end the session.
#[cfg(test)]
pub fn lines_lossy(reader: impl BufRead) -> impl Iterator<Item = String> {
    reader.split(b'\n').map_while(Result::ok).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// Lines of input as events, like `lines_lossy`, but a line of more than `max` bytes is skipped as it arrives
/// (and reported as `Event::TooLong`) instead of being held in memory: a client cannot make this process grow without end.
pub fn events_from(mut reader: impl BufRead, max: usize) -> impl Iterator<Item = Event> {
    std::iter::from_fn(move || {
        let mut line: Vec<u8> = Vec::new();
        let mut too_long = false;
        loop {
            let (used, finished, at_end) = {
                let buf = match reader.fill_buf() {
                    Ok(b) => b,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => return None,
                };
                if buf.is_empty() {
                    (0, false, true)
                } else {
                    let (take, finished) = match buf.iter().position(|b| *b == b'\n') {
                        Some(i) => (i, true),
                        None => (buf.len(), false),
                    };
                    if !too_long {
                        if line.len() + take > max {
                            too_long = true;
                            line = Vec::new();
                        } else {
                            line.extend_from_slice(&buf[..take]);
                        }
                    }
                    (take + usize::from(finished), finished, false)
                }
            };
            reader.consume(used);
            if at_end && line.is_empty() && !too_long {
                return None;
            }
            if finished || at_end {
                return Some(if too_long { Event::TooLong } else { Event::Line(String::from_utf8_lossy(&line).into_owned()) });
            }
        }
    })
}

/// How long a tool call waits for another process to finish starting or handing over.
const BUSY_WAIT: Duration = Duration::from_secs(5);
/// After handing the downloads to a window, how long this engine leaves it alone to open.
const YIELD_GRACE: Duration = Duration::from_secs(8);

/// The windowless engine: owns the downloads while no window does.
struct Local {
    app: App,
    rx: mpsc::Receiver<control::Request>,
    /// The web interface, while this engine owns the downloads.
    _web: Option<crate::web::server::Handle>,
    /// Released when this is dropped, after the app has saved.
    _lock: StateLock,
}

/// Uses the open window when there is one; otherwise runs the downloads itself, without a window.
/// When a window opens later it asks this engine to hand the downloads over, and from then on
/// every command goes to the window.
pub struct Engine {
    dir: PathBuf,
    local: Option<Local>,
    pub(crate) busy_wait: Duration,
    pub(crate) yield_grace: Duration,
    /// When the downloads were last handed to a window.
    yielded: Option<Instant>,
    /// A shared server: the web interface is configured here, and no window may take the downloads.
    server: Option<crate::web::server::Config>,
}

impl Engine {
    pub fn new(dir: PathBuf) -> Self {
        Engine { dir, local: None, busy_wait: BUSY_WAIT, yield_grace: YIELD_GRACE, yielded: None, server: None }
    }

    /// An engine for a shared server (`--server`): it serves the web interface to several people and never
    /// hands its downloads to a window.
    pub fn for_server(dir: PathBuf, web: crate::web::server::Config) -> Self {
        Engine { server: Some(web), ..Engine::new(dir) }
    }

    /// Become the owner of the downloads, waiting up to `wait` for a window that is closing to let
    /// go of them. An error means somebody else owns them (a window or another engine is running).
    pub fn ensure_local(&mut self, wait: Duration) -> Result<(), String> {
        let started = Instant::now();
        while self.local.is_none() {
            match StateLock::try_acquire(&self.dir) {
                Ok(Some(lock)) => return self.start_local(lock),
                Ok(None) => {
                    if control::read_info(&self.dir).is_some() {
                        return Err("another copy of Swift Download Manager is already running the downloads".into());
                    }
                    if started.elapsed() >= wait {
                        return Err("the download list stayed busy".into());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => return Err(format!("could not use the download list: {e}")),
            }
        }
        Ok(())
    }

    /// The app that runs the downloads, while this engine owns them.
    pub fn app_mut(&mut self) -> Option<&mut App> {
        self.local.as_mut().map(|l| &mut l.app)
    }

    /// Pause everything and save, so the next owner finds the downloads where they were.
    pub fn shutdown(&mut self) {
        if let Some(mut local) = self.local.take() {
            local.app.shutdown();
            control::clear_info(&self.dir);
        }
    }

    fn start_local(&mut self, lock: StateLock) -> Result<(), String> {
        eprintln!("swift-download-manager: no window is open, running the downloads without one");
        let app = App::headless_in(self.dir.clone());
        let (tx, rx) = mpsc::channel();
        let ctx = eframe::egui::Context::default();
        control::start(&self.dir, tx.clone(), ctx.clone(), "headless").map_err(|e| format!("could not open the control door: {e}"))?;
        // The web interface: configured for a server, or switched on in the settings of a personal computer.
        let cfg = match &self.server {
            Some(c) => Some(c.clone()),
            None if app.web_port_if_enabled().is_some() => Some(crate::web::server::Config {
                bind: [127, 0, 0, 1].into(),
                port: app.web_port_if_enabled().unwrap_or(9615),
                mode: crate::web::server::Mode::Personal,
                dir: self.dir.clone(),
                secure_cookie: false,
            }),
            None => None,
        };
        let web = match cfg {
            Some(c) => match crate::web::server::start_retrying(c, tx, ctx, 50) {
                Ok(h) => Some(h),
                Err(e) if self.server.is_some() => return Err(format!("the web interface could not start: {e}")),
                Err(e) => {
                    eprintln!("swift-download-manager: the web interface could not start: {e}");
                    None
                }
            },
            None => None,
        };
        self.local = Some(Local { app, rx, _web: web, _lock: lock });
        self.yielded = None;
        Ok(())
    }

    /// Answer what other processes asked of this engine, handing the downloads over when a
    /// window asks for them.
    fn serve_requests(&mut self) {
        let Some(local) = &mut self.local else { return };
        let jobs: Vec<control::Request> = local.rx.try_iter().collect();
        let mut handoff = None;
        for job in jobs {
            if job.cmd == Command::Handoff && self.server.is_some() {
                let _ = job.reply.send(Err("This is a download server; it keeps its downloads and does not hand them to a window.".into()));
                continue;
            }
            if job.cmd == Command::Handoff {
                local.app.prepare_handoff();
                handoff = Some(job.reply);
                break;
            }
            let answer = local.app.run_request(job.cmd, job.who);
            let _ = job.reply.send(answer);
        }
        if let Some(reply) = handoff {
            eprintln!("swift-download-manager: a window opened and took over the downloads");
            control::clear_info(&self.dir);
            // Dropping `local` releases the lock; only then is the window told it may go ahead.
            self.local = None;
            self.yielded = Some(Instant::now());
            let _ = reply.send(Ok(serde_json::json!({ "handed_over": true })));
        }
    }
}

impl Engine {
    /// Hand a command to whoever owns the downloads (the open window, or this process without one).
    fn dispatch(&mut self, cmd: Command) -> Result<Value, ToolError> {
        self.serve_requests();
        let started = Instant::now();
        while self.local.is_none() {
            match control::call(&self.dir, &cmd) {
                Ok(v) => return Ok(v),
                Err(CallError::Tool(e)) => return Err(e),
                Err(CallError::Other(m)) => return Err(m.into()),
                Err(CallError::NoApp) => {
                    // Just handed the downloads to a window that is still opening: leave the lock
                    // free for it instead of racing it for the downloads.
                    if self.yielded.is_some_and(|t| t.elapsed() < self.yield_grace) {
                        if started.elapsed() >= self.busy_wait {
                            return Err(ToolError::new(ErrorKind::Busy, "The app window is still opening. Try again in a moment."));
                        }
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                }
            }
            match StateLock::try_acquire(&self.dir) {
                Ok(Some(lock)) => self.start_local(lock)?,
                // Someone else owns the downloads but is not answering yet (starting up).
                Ok(None) if started.elapsed() < self.busy_wait => std::thread::sleep(Duration::from_millis(100)),
                Ok(None) => return Err(ToolError::new(ErrorKind::Busy, "The download list is in use by another Swift Download Manager that is not answering. Try again in a moment.")),
                Err(e) => return Err(ToolError::new(ErrorKind::Internal, format!("Could not use the download list: {e}"))),
            }
        }
        let Some(local) = self.local.as_mut() else { return Err(ToolError::new(ErrorKind::Internal, "the download engine could not start")) };
        local.app.pump();
        let answer = local.app.run_command(cmd);
        local.app.pump();
        answer
    }
}

impl Backend for Engine {
    fn run(&mut self, cmd: Command) -> Result<Value, ToolError> {
        // Looking at a link takes as long as the other server takes, so it is done here and not in the window. The app is
        // asked first: it checks the rules (and whether assistants are allowed at all), notes it in the log, and hands the
        // rules back for the look to use.
        if let Command::ProbeLink { url } = &cmd {
            let answer = self.dispatch(cmd.clone())?;
            let policy: Policy = serde_json::from_value(answer.get("policy").cloned().unwrap_or(Value::Null)).map_err(|_| ToolError::new(ErrorKind::Internal, "The app did not say what the rules are."))?;
            return super::probe::probe(url, &policy);
        }
        self.dispatch(cmd)
    }

    fn idle(&mut self) {
        self.serve_requests();
        if let Some(local) = &mut self.local {
            local.app.pump();
        }
    }
}

/// What the command line of the `--mcp` process asks for: `--level read|standard|full` (or `--read-only`) limits what
/// this connection may do, whatever the settings of the app allow. The app's own settings are applied as well; the
/// lower of the two wins.
pub fn options_from_args(args: &[String]) -> Result<Options, String> {
    let mut level = Level::Full;
    let mut i = 0;
    while i < args.len() {
        let value = match args[i].as_str() {
            "--read-only" => Some("read".to_string()),
            "--level" => {
                i += 1;
                Some(args.get(i).cloned().ok_or("--level needs a value: read, standard or full")?)
            }
            other => other.strip_prefix("--level=").map(str::to_string),
        };
        if let Some(v) = value {
            let wanted = Level::parse(&v).ok_or_else(|| format!("\"{v}\" is not an access level; use read, standard or full"))?;
            level = level.min(wanted);
        }
        i += 1;
    }
    Ok(Options { level })
}

/// Run the MCP server until the client closes stdin. Returns the process exit code.
pub fn run_stdio() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match options_from_args(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("swift-download-manager: {e}");
            return 2;
        }
    };
    let (tx, rx) = mpsc::channel::<Option<Event>>();
    std::thread::spawn(move || {
        for event in events_from(io::stdin().lock(), protocol::MAX_LINE) {
            if tx.send(Some(event)).is_err() {
                return;
            }
        }
        let _ = tx.send(None);
    });
    let events = std::iter::from_fn(move || match rx.recv_timeout(Duration::from_millis(200)) {
        Ok(Some(event)) => Some(event),
        Ok(None) | Err(RecvTimeoutError::Disconnected) => None,
        Err(RecvTimeoutError::Timeout) => Some(Event::Idle),
    });
    let mut engine = Engine::new(default_config_dir());
    let stdout = io::stdout();
    let result = serve_with(events, &mut stdout.lock(), &mut engine, &options);
    engine.shutdown();
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("swift-download-manager: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::io::Cursor;

    #[derive(Default)]
    struct Fake {
        commands: Vec<Command>,
        idles: usize,
    }

    impl Backend for Fake {
        fn run(&mut self, cmd: Command) -> Result<Value, ToolError> {
            self.commands.push(cmd);
            Ok(json!({ "mode": "fake" }))
        }
        fn idle(&mut self) {
            self.idles += 1;
        }
    }

    fn session(input: &[u8]) -> (Vec<String>, Fake) {
        let mut out = Vec::new();
        let mut fake = Fake::default();
        let events = lines_lossy(Cursor::new(input.to_vec())).map(Event::Line).chain([Event::Idle, Event::Idle]);
        serve(events, &mut out, &mut fake).unwrap();
        (String::from_utf8(out).unwrap().lines().map(str::to_string).collect(), fake)
    }

    #[test]
    fn stdio_session_round_trip() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#, "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#, "\n",
            "{garbage\n",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_status","arguments":{}}}"#, "\n",
        );
        let (lines, fake) = session(input.as_bytes());
        assert_eq!(lines.len(), 4, "initialize, tools/list, the parse error and the call; got {lines:?}");
        let ids: Vec<Value> = lines.iter().map(|l| serde_json::from_str::<Value>(l).unwrap()["id"].clone()).collect();
        assert_eq!(ids, vec![json!(1), json!(2), Value::Null, json!(3)]);
        assert_eq!(fake.commands, vec![Command::GetStatus]);
        assert_eq!(fake.idles, 2);
    }

    #[test]
    fn stdout_only_has_json_one_message_per_line() {
        let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\nnot json\n\n[1]\n";
        let (lines, _) = session(input.as_bytes());
        assert_eq!(lines.len(), 3);
        for l in &lines {
            let v: Value = serde_json::from_str(l).expect("every line is JSON");
            assert_eq!(v["jsonrpc"], "2.0");
        }
    }

    #[test]
    fn invalid_utf8_and_blank_lines_do_not_end_the_session() {
        let mut input = vec![0xff, 0xfe, b'\n', b'\n'];
        input.extend_from_slice(br#"{"jsonrpc":"2.0","id":9,"method":"ping"}"#);
        input.push(b'\n');
        let (lines, _) = session(&input);
        assert_eq!(lines.len(), 2, "a parse error, then the ping answer: {lines:?}");
        assert_eq!(serde_json::from_str::<Value>(&lines[0]).unwrap()["error"]["code"], -32700);
        assert_eq!(serde_json::from_str::<Value>(&lines[1]).unwrap()["id"], 9);
    }

    #[test]
    fn idle_events_write_nothing() {
        let (lines, fake) = session(b"");
        assert!(lines.is_empty());
        assert_eq!(fake.idles, 2);
    }

    fn events_of(input: &[u8], max: usize) -> Vec<String> {
        events_from(Cursor::new(input.to_vec()), max)
            .map(|e| match e {
                Event::Line(l) => format!("line:{l}"),
                Event::TooLong => "too long".to_string(),
                Event::Idle => "idle".to_string(),
            })
            .collect()
    }

    #[test]
    fn long_lines_are_dropped_as_they_arrive_and_the_session_goes_on() {
        assert_eq!(events_of(b"ab\ncd\n", 10), vec!["line:ab", "line:cd"]);
        assert_eq!(events_of(b"ab\ncd", 10), vec!["line:ab", "line:cd"], "the last line needs no newline");
        assert_eq!(events_of(b"", 10), Vec::<String>::new());
        assert_eq!(events_of(b"\n\n", 10), vec!["line:", "line:"]);
        // Exactly the limit passes; one more does not; the line after a long one is read normally.
        assert_eq!(events_of(b"1234567890\n", 10), vec!["line:1234567890"]);
        assert_eq!(events_of(b"12345678901\nok\n", 10), vec!["too long", "line:ok"]);
        assert_eq!(events_of(&[b'x'; 5000], 100), vec!["too long"], "also when the line never ends");
        // Bytes that are not UTF-8 are replaced.
        assert_eq!(events_of(&[0xff, b'a', b'\n'], 10), vec!["line:\u{fffd}a"]);
        // A big input is skipped without building a big line: the cap works across the reader's own chunks.
        let mut big = vec![b'y'; 3 * 1024 * 1024];
        big.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n");
        let events = events_of(&big, protocol::MAX_LINE);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], "too long");
    }

    #[test]
    fn a_too_long_line_gets_an_error_and_later_lines_still_work() {
        let mut input = vec![b'z'; protocol::MAX_LINE + 1];
        input.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"ping\"}\n");
        let mut out = Vec::new();
        let mut fake = Fake::default();
        serve(events_from(Cursor::new(input), protocol::MAX_LINE), &mut out, &mut fake).unwrap();
        let lines: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["error"]["code"], -32600);
        assert!(lines[0]["error"]["message"].as_str().unwrap().contains("too large"));
        assert_eq!(lines[1]["id"], 4);
    }

    struct Panics;

    impl Backend for Panics {
        fn run(&mut self, _cmd: Command) -> Result<Value, ToolError> {
            panic!("a bug in a command");
        }
        fn idle(&mut self) {}
    }

    #[test]
    fn a_bug_in_one_command_does_not_end_the_session() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_status","arguments":{}}}"#, "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#, "\n",
        );
        let mut out = Vec::new();
        serve(lines_lossy(Cursor::new(input.as_bytes().to_vec())).map(Event::Line), &mut out, &mut Panics).unwrap();
        let lines: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 2, "the failing call and the ping after it both got an answer");
        assert_eq!(lines[0]["result"]["isError"], true);
        assert_eq!(lines[0]["result"]["structuredContent"]["error"]["kind"], "internal");
        assert_eq!(lines[1]["id"], 2);
    }

    #[test]
    fn the_command_line_can_only_lower_the_level() {
        let args = |v: &[&str]| options_from_args(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(args(&["--mcp"]).unwrap().level, Level::Full);
        assert_eq!(args(&["--mcp", "--read-only"]).unwrap().level, Level::ReadOnly);
        assert_eq!(args(&["--mcp", "--level", "standard"]).unwrap().level, Level::Standard);
        assert_eq!(args(&["--mcp", "--level=read"]).unwrap().level, Level::ReadOnly);
        // Asking twice never raises it again.
        assert_eq!(args(&["--read-only", "--level", "full"]).unwrap().level, Level::ReadOnly);
        assert!(args(&["--level"]).unwrap_err().contains("needs a value"));
        assert!(args(&["--level", "root"]).unwrap_err().contains("not an access level"));
    }

    #[test]
    fn read_only_connections_never_reach_the_backend_for_changes() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"pause_all","arguments":{}}}"#, "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_status","arguments":{}}}"#, "\n",
        );
        let mut out = Vec::new();
        let mut fake = Fake::default();
        serve_with(lines_lossy(Cursor::new(input.as_bytes().to_vec())).map(Event::Line), &mut out, &mut fake, &Options { level: Level::ReadOnly }).unwrap();
        assert_eq!(fake.commands, vec![Command::GetStatus]);
    }
}
