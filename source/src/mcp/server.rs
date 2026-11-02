//! `swift_download_manager --mcp`: an MCP server on stdin/stdout.
//!
//! Only JSON-RPC goes to stdout; everything else goes to stderr.

use super::command::Command;
use super::control::{self, CallError};
use super::lock::StateLock;
use super::protocol;
use crate::app::{default_config_dir, App};
use serde_json::Value;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// What the server loop reacts to.
pub enum Event {
    /// One line from the client.
    Line(String),
    /// Nothing arrived for a while: time for housekeeping.
    Idle,
}

/// Whatever actually runs the commands.
pub trait Backend {
    fn run(&mut self, cmd: Command) -> Result<Value, String>;
    fn idle(&mut self);
}

/// The server loop: every line in gives at most one line out.
pub fn serve(events: impl Iterator<Item = Event>, out: &mut dyn Write, backend: &mut dyn Backend) -> io::Result<()> {
    for event in events {
        match event {
            Event::Idle => backend.idle(),
            Event::Line(line) => {
                if let Some(answer) = protocol::handle_line(&line, &mut |cmd| backend.run(cmd)) {
                    // Compact JSON never contains a newline, so one message is one line.
                    writeln!(out, "{answer}")?;
                    out.flush()?;
                }
            }
        }
    }
    Ok(())
}

/// Lines of input as text. Bytes that are not UTF-8 are replaced, so one bad line cannot end the session.
pub fn lines_lossy(reader: impl BufRead) -> impl Iterator<Item = String> {
    reader.split(b'\n').map_while(Result::ok).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// How long a tool call waits for another process to finish starting or handing over.
const BUSY_WAIT: Duration = Duration::from_secs(5);
/// After handing the downloads to a window, how long this engine leaves it alone to open.
const YIELD_GRACE: Duration = Duration::from_secs(8);

/// The windowless engine: owns the downloads while no window does.
struct Local {
    app: App,
    rx: mpsc::Receiver<control::Request>,
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
}

impl Engine {
    pub fn new(dir: PathBuf) -> Self {
        Engine { dir, local: None, busy_wait: BUSY_WAIT, yield_grace: YIELD_GRACE, yielded: None }
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
        control::start(&self.dir, tx, eframe::egui::Context::default(), "headless").map_err(|e| format!("could not open the control door: {e}"))?;
        self.local = Some(Local { app, rx, _lock: lock });
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
            if job.cmd == Command::Handoff {
                local.app.prepare_handoff();
                handoff = Some(job.reply);
                break;
            }
            let answer = local.app.run_command(job.cmd);
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

impl Backend for Engine {
    fn run(&mut self, cmd: Command) -> Result<Value, String> {
        self.serve_requests();
        let started = Instant::now();
        while self.local.is_none() {
            match control::call(&self.dir, &cmd) {
                Ok(v) => return Ok(v),
                Err(CallError::Tool(m) | CallError::Other(m)) => return Err(m),
                Err(CallError::NoApp) => {
                    // Just handed the downloads to a window that is still opening: leave the lock
                    // free for it instead of racing it for the downloads.
                    if self.yielded.is_some_and(|t| t.elapsed() < self.yield_grace) {
                        if started.elapsed() >= self.busy_wait {
                            return Err("The app window is still opening. Try again in a moment.".into());
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
                Ok(None) => return Err("The download list is in use by another Swift Download Manager that is not answering. Try again in a moment.".into()),
                Err(e) => return Err(format!("Could not use the download list: {e}")),
            }
        }
        let Some(local) = self.local.as_mut() else { return Err("the download engine could not start".into()) };
        local.app.pump();
        let answer = local.app.run_command(cmd);
        local.app.pump();
        answer
    }

    fn idle(&mut self) {
        self.serve_requests();
        if let Some(local) = &mut self.local {
            local.app.pump();
        }
    }
}

/// Run the MCP server until the client closes stdin. Returns the process exit code.
pub fn run_stdio() -> i32 {
    let (tx, rx) = mpsc::channel::<Option<String>>();
    std::thread::spawn(move || {
        for line in lines_lossy(io::stdin().lock()) {
            if tx.send(Some(line)).is_err() {
                return;
            }
        }
        let _ = tx.send(None);
    });
    let events = std::iter::from_fn(move || match rx.recv_timeout(Duration::from_millis(200)) {
        Ok(Some(line)) => Some(Event::Line(line)),
        Ok(None) | Err(RecvTimeoutError::Disconnected) => None,
        Err(RecvTimeoutError::Timeout) => Some(Event::Idle),
    });
    let mut engine = Engine::new(default_config_dir());
    let stdout = io::stdout();
    let result = serve(events, &mut stdout.lock(), &mut engine);
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
        fn run(&mut self, cmd: Command) -> Result<Value, String> {
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
}
