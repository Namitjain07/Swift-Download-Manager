//! JSON-RPC 2.0 messages for MCP over stdio, and the tool list.
//!
//! `handle_line` turns one line of input into at most one line of output. It never touches the
//! downloads itself: a tool call is turned into a `Command` and handed to `run`.

use super::command::Command;
use serde_json::{json, Map, Value};

/// Protocol versions we speak, newest first. A client asking for one of them gets the same back.
pub const SUPPORTED_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Controls Swift Download Manager. Use list_downloads to see what is there, add_download to start \
a download from an http(s) link (it is saved in the folder of its category), and pause/resume/start_queue/stop_queue to \
control it. remove_download only takes an entry off the list; it never deletes files from disk.";

type Failure = (i32, String);

fn ok(id: Value, result: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string()
}

fn error(id: Value, code: i32, message: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}).to_string()
}

/// Handle one line from the client. `None` means nothing is sent back (blank line,
/// notification, or a reply to something we never asked).
pub fn handle_line(line: &str, run: &mut dyn FnMut(Command) -> Result<Value, String>) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Some(error(Value::Null, -32700, &format!("Parse error: {e}"))),
    };
    let Some(obj) = msg.as_object() else {
        return Some(error(Value::Null, -32600, "Invalid Request: expected a JSON object (batches are not supported)"));
    };
    let id = obj.get("id").cloned();
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        if obj.contains_key("result") || obj.contains_key("error") {
            return None;
        }
        return Some(error(id.unwrap_or(Value::Null), -32600, "Invalid Request: missing method"));
    };
    // Notifications are never answered, whatever they say.
    let id = id?;
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    let outcome: Result<Value, Failure> = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => tools_call(&params, run),
        _ => Err((-32601, format!("Method not found: {method}"))),
    };
    Some(match outcome {
        Ok(result) => ok(id, result),
        Err((code, message)) => error(id, code, &message),
    })
}

fn initialize(params: &Value) -> Value {
    let wanted = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
    let version = SUPPORTED_VERSIONS.iter().find(|v| **v == wanted).unwrap_or(&SUPPORTED_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "swift-download-manager", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn tools_call(params: &Value, run: &mut dyn FnMut(Command) -> Result<Value, String>) -> Result<Value, Failure> {
    let name = params.get("name").and_then(Value::as_str).ok_or((-32602, "Missing tool name".to_string()))?;
    let mut args: Map<String, Value> = match params.get("arguments") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(m)) => m.clone(),
        Some(_) => return Err((-32602, "arguments must be an object".into())),
    };
    let tag = TOOLS.iter().find(|t| t.name == name).map(|t| t.cmd).ok_or_else(|| (-32602, format!("Unknown tool: {name}")))?;
    // The tool name decides the command; an argument called "cmd" cannot pick another one.
    args.insert("cmd".into(), Value::String(tag.into()));
    let cmd: Command = serde_json::from_value(Value::Object(args)).map_err(|e| (-32602, format!("Invalid arguments for {name}: {e}")))?;
    let (text, is_error) = match run(cmd) {
        Ok(v) => (serde_json::to_string_pretty(&v).unwrap_or_default(), false),
        Err(message) => (message, true),
    };
    Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }))
}

struct Tool {
    name: &'static str,
    /// The `Command` variant it runs (its serde tag).
    cmd: &'static str,
    title: &'static str,
    description: &'static str,
    read_only: bool,
    destructive: bool,
    idempotent: bool,
    open_world: bool,
}

const TOOLS: [Tool; 10] = [
    Tool { name: "list_downloads", cmd: "list_downloads", title: "List downloads", description: "List downloads with their id, name, status, progress, size and speed. Optionally filter by status (queued, connecting, downloading, paused, complete, error) or by a search text in the name or address.", read_only: true, destructive: false, idempotent: true, open_world: false },
    Tool { name: "get_download", cmd: "get_download", title: "Get a download", description: "Everything about one download: address, folder, category, queue, size, progress, speed, time left, error and checksum result.", read_only: true, destructive: false, idempotent: true, open_world: false },
    Tool { name: "get_status", cmd: "get_status", title: "Get overall status", description: "How many downloads are active, queued or finished, the current total speed, the speed limit, and whether the app window is open or the downloads run headless.", read_only: true, destructive: false, idempotent: true, open_world: false },
    Tool { name: "list_queues", cmd: "list_queues", title: "List queues", description: "List the download queues, whether each is running, how many downloads it runs at once and how many wait in it.", read_only: true, destructive: false, idempotent: true, open_world: false },
    Tool { name: "add_download", cmd: "add_download", title: "Add a download", description: "Start downloading an http or https link. The file is saved in the download folder of its category (chosen from the file name unless you give a category); you cannot choose another folder. With start=false it is added to a queue instead of starting now.", read_only: false, destructive: false, idempotent: false, open_world: true },
    Tool { name: "pause_download", cmd: "pause", title: "Pause a download", description: "Pause a running or queued download. Progress is kept and it can be resumed later.", read_only: false, destructive: false, idempotent: true, open_world: false },
    Tool { name: "resume_download", cmd: "resume", title: "Resume a download", description: "Start or resume a paused, failed or queued download.", read_only: false, destructive: false, idempotent: true, open_world: false },
    Tool { name: "start_queue", cmd: "start_queue", title: "Start a queue", description: "Start running the downloads waiting in a queue, a few at a time.", read_only: false, destructive: false, idempotent: true, open_world: false },
    Tool { name: "stop_queue", cmd: "stop_queue", title: "Stop a queue", description: "Stop a running queue and pause its downloads.", read_only: false, destructive: false, idempotent: true, open_world: false },
    Tool { name: "remove_download", cmd: "remove", title: "Remove a download from the list", description: "Take a download off the list and stop it. Files already on disk, including partly downloaded ones, are never deleted.", read_only: false, destructive: true, idempotent: false, open_world: false },
];

fn schema(tool: &str) -> Value {
    let id = json!({ "type": "integer", "minimum": 0, "description": "The download's id, from list_downloads." });
    let name = json!({ "type": "string", "description": "The queue's name, from list_queues." });
    match tool {
        "list_downloads" => json!({ "type": "object", "properties": {
            "status": { "type": "string", "enum": ["queued", "connecting", "downloading", "paused", "complete", "error"] },
            "query": { "type": "string", "description": "Only downloads whose name or address contains this text." },
            "limit": { "type": "integer", "minimum": 1, "maximum": 500, "description": "At most this many (default 100)." } } }),
        "get_download" | "pause_download" | "resume_download" | "remove_download" => json!({ "type": "object", "properties": { "id": id }, "required": ["id"] }),
        "add_download" => json!({ "type": "object", "properties": {
            "url": { "type": "string", "description": "An http:// or https:// link." },
            "name": { "type": "string", "description": "File name to save as (default: taken from the link)." },
            "category": { "type": "string", "description": "A category from the app, such as Video or Documents." },
            "queue": { "type": "string", "description": "Queue to use when start is false (default: Main Queue)." },
            "start": { "type": "boolean", "description": "Start now (default true) or only add to the queue." } }, "required": ["url"] }),
        "start_queue" | "stop_queue" => json!({ "type": "object", "properties": { "name": name }, "required": ["name"] }),
        _ => json!({ "type": "object", "properties": {} }),
    }
}

fn tools() -> Vec<Value> {
    TOOLS
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "title": t.title,
                "description": t.description,
                "inputSchema": schema(t.name),
                "annotations": {
                    "title": t.title,
                    "readOnlyHint": t.read_only,
                    "destructiveHint": t.destructive,
                    "idempotentHint": t.idempotent,
                    "openWorldHint": t.open_world,
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::command::Command;
    use serde_json::{json, Value};

    fn ask(line: &str) -> Option<Value> {
        handle_line(line, &mut |_| Ok(json!({"ok": true}))).map(|s| serde_json::from_str(&s).expect("a response is valid JSON"))
    }

    #[test]
    fn initialize_negotiates_version() {
        let r = ask(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#).unwrap();
        assert_eq!(r["id"], 1);
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(r["result"]["serverInfo"]["name"], "swift-download-manager");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        let r = ask(r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
    }

    #[test]
    fn tools_list_has_ten_tools_with_hints() {
        let r = ask(r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#).unwrap();
        let tools = r["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 10);
        let by = |n: &str| tools.iter().find(|t| t["name"] == n).unwrap_or_else(|| panic!("tool {n} missing"));
        assert_eq!(by("list_downloads")["annotations"]["readOnlyHint"], true);
        assert_eq!(by("remove_download")["annotations"]["destructiveHint"], true);
        assert_eq!(by("add_download")["annotations"]["openWorldHint"], true);
        assert_eq!(by("add_download")["annotations"]["destructiveHint"], false);
        assert_eq!(by("pause_download")["annotations"]["idempotentHint"], true);
        for t in tools {
            assert_eq!(t["inputSchema"]["type"], "object", "{} needs an object schema", t["name"]);
        }
        assert_eq!(by("add_download")["inputSchema"]["required"], json!(["url"]));
    }

    #[test]
    fn notification_gets_no_reply() {
        assert!(ask(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        // Even an unknown method, when it has no id, must not be answered.
        assert!(ask(r#"{"jsonrpc":"2.0","method":"nope"}"#).is_none());
    }

    #[test]
    fn garbage_lines_get_json_rpc_errors() {
        assert_eq!(ask("{nope").unwrap()["error"]["code"], -32700);
        assert_eq!(ask(r#"{"jsonrpc":"2.0","id":1,"method":"nope"}"#).unwrap()["error"]["code"], -32601);
        assert_eq!(ask(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_download","arguments":{"id":"x"}}}"#).unwrap()["error"]["code"], -32602);
        assert_eq!(ask(r#"[1,2]"#).unwrap()["error"]["code"], -32600);
        assert_eq!(ask("42").unwrap()["error"]["code"], -32600);
        assert!(ask("").is_none(), "blank lines are ignored");
    }

    #[test]
    fn unknown_tool_is_invalid_params() {
        let r = ask(r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"format_disk","arguments":{}}}"#).unwrap();
        assert_eq!(r["error"]["code"], -32602);
    }

    #[test]
    fn tool_error_is_not_a_protocol_error() {
        let line = r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"get_status"}}"#;
        let out = handle_line(line, &mut |_| Err("boom".to_string())).unwrap();
        let r: Value = serde_json::from_str(&out).unwrap();
        assert!(r.get("error").is_none());
        assert_eq!(r["result"]["isError"], true);
        assert_eq!(r["result"]["content"][0]["text"], "boom");
    }

    #[test]
    fn tool_call_maps_arguments_to_the_command() {
        let mut seen = Vec::new();
        let line = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"add_download","arguments":{"url":"https://example.com/a.zip","start":false}}}"#;
        let out = handle_line(line, &mut |c| {
            seen.push(c);
            Ok(json!({"id": 1}))
        })
        .unwrap();
        assert_eq!(seen, vec![Command::AddDownload { url: "https://example.com/a.zip".into(), name: None, category: None, queue: None, start: Some(false) }]);
        let r: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(r["result"]["isError"], false);
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        assert_eq!(serde_json::from_str::<Value>(text).unwrap(), json!({"id": 1}));
    }
}
