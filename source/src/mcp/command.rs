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
