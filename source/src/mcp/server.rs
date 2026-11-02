//! `swift_download_manager --mcp`: an MCP server on stdin/stdout.
//!
//! Only JSON-RPC goes to stdout; everything else goes to stderr.

use super::command::Command;
use super::control::{self, CallError};
use super::protocol;
use crate::app::{default_config_dir, App};
use serde_json::Value;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

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

/// Uses the open window when there is one; otherwise runs the downloads itself, without a window.
pub struct Engine {
    dir: PathBuf,
    local: Option<App>,
}

impl Engine {
    pub fn new(dir: PathBuf) -> Self {
        Engine { dir, local: None }
    }

    /// Pause everything and save, so the next owner finds the downloads where they were.
    pub fn shutdown(&mut self) {
        if let Some(app) = &mut self.local {
            app.shutdown();
        }
    }
}

impl Backend for Engine {
    fn run(&mut self, cmd: Command) -> Result<Value, String> {
        if self.local.is_none() {
            match control::call(&self.dir, &cmd) {
                Ok(v) => return Ok(v),
                Err(CallError::Tool(m) | CallError::Other(m)) => return Err(m),
                Err(CallError::NoApp) => {
                    eprintln!("swift-download-manager: no window is open, running the downloads without one");
                    self.local = Some(App::headless());
                }
            }
        }
        let Some(app) = self.local.as_mut() else { return Err("the download engine could not start".into()) };
        app.pump();
        let answer = app.run_command(cmd);
        app.pump();
        answer
    }

    fn idle(&mut self) {
        if let Some(app) = &mut self.local {
            app.pump();
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
