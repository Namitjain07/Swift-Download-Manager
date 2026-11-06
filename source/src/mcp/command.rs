//! What a tool call asks the app to do. The same commands are run by the open window and by
//! the windowless engine, and they travel as JSON between the `--mcp` process and the window.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    ListDownloads { status: Option<String>, query: Option<String>, limit: Option<usize> },
    GetDownload { id: u64 },
    GetStatus,
    ListQueues,
    AddDownload { url: String, name: Option<String>, category: Option<String>, queue: Option<String>, start: Option<bool> },
    Pause { id: u64 },
    Resume { id: u64 },
    StartQueue { name: String },
    StopQueue { name: String },
    Remove { id: u64 },
    /// Not a tool: where a finished file is, so the web interface can send it to a browser.
    Locate { id: u64 },
    /// Not a tool (web interface): several links at once. Each one gets its own answer.
    AddMany { urls: Vec<String>, category: Option<String>, queue: Option<String>, start: Option<bool> },
    /// Not a tool (web interface): pause everything of this person's that is running or waiting.
    PauseAll,
    /// Not a tool (web interface): carry on with everything of this person's that is paused or failed.
    ResumeAll,
    /// Not a tool (web interface): take this person's finished downloads off the list (files are never touched).
    ClearCompleted,
    /// Not a tool (web interface, administrators): the networks of this computer and how they are used.
    ListNetworks,
    /// Not a tool (web interface, administrators): choose which networks downloads use.
    SetNetworks { on: Option<bool>, auto: Option<bool>, names: Option<Vec<String>>, off: Option<Vec<String>> },
    /// Not a tool (web interface, administrators): the overall speed limit in KB/s; 0 or nothing switches it off.
    SetSpeedLimit { kbps: Option<u64> },
    /// Not a tool (web interface, administrators): what each person has downloaded.
    Usage,
    /// Not a tool: a window that is starting asks a windowless engine to let go of the downloads.
    Handoff,
}

/// Who a web request is for. On a shared server every person has their own downloads; the page on
/// your own computer has an empty name and is not tied to anybody.
#[derive(Debug, Clone, PartialEq)]
pub struct Who {
    pub name: String,
    pub admin: bool,
}
