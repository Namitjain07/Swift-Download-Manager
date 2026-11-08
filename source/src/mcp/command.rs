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
    /// Move a download to another queue.
    MoveDownload { id: u64, queue: String },
    /// Check a link (size, type, whether it can be resumed) without downloading it. The `--mcp` process does the looking itself,
    /// so that a slow server never holds up the window; the app only checks the rules, notes it in the log and hands them back.
    ProbeLink { url: String },
    /// Start checking a finished file against a SHA-256 (the one given, or the one already expected for it).
    VerifyDownload { id: u64, sha256: Option<String> },
    /// The settings an assistant may know (no passwords, proxies or logins).
    GetSettings,
    /// What assistants did recently.
    AuditLog { limit: Option<usize> },
    /// Not a tool: where a finished file is, so the web interface can send it to a browser.
    Locate { id: u64 },
    /// Several links at once. Each one gets its own answer.
    AddMany { urls: Vec<String>, category: Option<String>, queue: Option<String>, start: Option<bool> },
    /// Pause everything that is running or waiting (for a person of the web interface: only theirs).
    PauseAll,
    /// Carry on with everything that is paused or failed (for a person of the web interface: only theirs).
    ResumeAll,
    /// Take finished downloads off the list (files are never touched).
    ClearCompleted,
    /// The networks of this computer and how they are used (web interface administrators, and assistants at the full level).
    ListNetworks,
    /// Not a tool (web interface, administrators): choose which networks downloads use.
    SetNetworks { on: Option<bool>, auto: Option<bool>, names: Option<Vec<String>>, off: Option<Vec<String>> },
    /// The overall speed limit in KB/s; 0 or nothing switches it off (web interface administrators, and assistants at the full level).
    SetSpeedLimit { kbps: Option<u64> },
    /// Not a tool (web interface, administrators): what each person has downloaded.
    Usage,
    /// Not a tool: a window that is starting asks a windowless engine to let go of the downloads.
    Handoff,
}

/// What kind of call this is, for the access level: looking, changing downloads, or changing the whole app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    Read,
    Change,
    Wide,
}

impl Command {
    /// The tool that runs this command (the name an assistant knows it by).
    pub fn tool_name(&self) -> &'static str {
        match self {
            Command::ListDownloads { .. } => "list_downloads",
            Command::GetDownload { .. } => "get_download",
            Command::GetStatus => "get_status",
            Command::ListQueues => "list_queues",
            Command::AddDownload { .. } => "add_download",
            Command::AddMany { .. } => "add_downloads",
            Command::Pause { .. } => "pause_download",
            Command::Resume { .. } => "resume_download",
            Command::PauseAll => "pause_all",
            Command::ResumeAll => "resume_all",
            Command::StartQueue { .. } => "start_queue",
            Command::StopQueue { .. } => "stop_queue",
            Command::Remove { .. } => "remove_download",
            Command::MoveDownload { .. } => "move_download",
            Command::ProbeLink { .. } => "probe_link",
            Command::VerifyDownload { .. } => "verify_download",
            Command::GetSettings => "get_settings",
            Command::AuditLog { .. } => "get_audit_log",
            Command::ClearCompleted => "clear_completed",
            Command::ListNetworks => "list_networks",
            Command::SetSpeedLimit { .. } => "set_speed_limit",
            Command::SetNetworks { .. } => "set_networks",
            Command::Usage => "usage",
            Command::Locate { .. } => "locate",
            Command::Handoff => "handoff",
        }
    }

    /// How far this reaches, for the access level.
    pub fn reach(&self) -> Reach {
        match self {
            Command::ListDownloads { .. }
            | Command::GetDownload { .. }
            | Command::GetStatus
            | Command::ListQueues
            | Command::ProbeLink { .. }
            | Command::GetSettings
            | Command::AuditLog { .. }
            | Command::Locate { .. } => Reach::Read,
            // Starts reading a file and records the result on the download: a change, but a small one.
            Command::VerifyDownload { .. } => Reach::Change,
            Command::AddDownload { .. }
            | Command::AddMany { .. }
            | Command::Pause { .. }
            | Command::Resume { .. }
            | Command::PauseAll
            | Command::ResumeAll
            | Command::StartQueue { .. }
            | Command::StopQueue { .. }
            | Command::Remove { .. }
            | Command::MoveDownload { .. } => Reach::Change,
            Command::ClearCompleted | Command::ListNetworks | Command::SetSpeedLimit { .. } | Command::SetNetworks { .. } | Command::Usage | Command::Handoff => Reach::Wide,
        }
    }

    /// Whether this changes anything (for the limit on how fast an assistant may change things).
    pub fn changes_things(&self) -> bool {
        self.reach() != Reach::Read
    }

    /// A short description for the log: what was asked, without anything secret (a link is reduced to its host and path:
    /// the password, the query and the fragment of a link can carry keys).
    pub fn audit_summary(&self) -> String {
        match self {
            Command::AddDownload { url, queue, .. } => format!("{}{}", plain_link(url), queue.as_ref().map(|q| format!(" -> {}", super::policy::untrusted(q, 40))).unwrap_or_default()),
            Command::ProbeLink { url } => plain_link(url),
            Command::AddMany { urls, .. } => format!("{} links", urls.len()),
            Command::GetDownload { id } | Command::Pause { id } | Command::Resume { id } | Command::Remove { id } | Command::VerifyDownload { id, .. } | Command::Locate { id } => format!("download {id}"),
            Command::MoveDownload { id, queue } => format!("download {id} -> {}", super::policy::untrusted(queue, 40)),
            Command::StartQueue { name } | Command::StopQueue { name } => format!("queue {}", super::policy::untrusted(name, 40)),
            Command::ListDownloads { status, query, .. } => {
                let mut parts = Vec::new();
                if let Some(s) = status {
                    parts.push(format!("status {}", super::policy::untrusted(s, 20)));
                }
                if let Some(q) = query {
                    parts.push(format!("search \"{}\"", super::policy::untrusted(q, 40)));
                }
                parts.join(", ")
            }
            Command::SetSpeedLimit { kbps } => match kbps {
                Some(k) if *k > 0 => format!("{k} KB/s"),
                _ => "no limit".to_string(),
            },
            _ => String::new(),
        }
    }
}

/// `https://host/path` of a link, or a placeholder when it is not one.
fn plain_link(link: &str) -> String {
    match url::Url::parse(link.trim()) {
        Ok(u) => format!("{}://{}{}", u.scheme(), u.host_str().unwrap_or("?"), super::policy::untrusted(u.path(), 80)),
        Err(_) => "(not a link)".to_string(),
    }
}

/// Who a web request is for. On a shared server every person has their own downloads; the page on
/// your own computer has an empty name and is not tied to anybody.
#[derive(Debug, Clone, PartialEq)]
pub struct Who {
    pub name: String,
    pub admin: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_serialises_with_its_tag() {
        let c = Command::MoveDownload { id: 3, queue: "Night".into() };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["cmd"], "move_download");
        assert_eq!(serde_json::from_value::<Command>(v).unwrap(), c);
    }

    #[test]
    fn reach_decides_what_each_level_may_run() {
        assert_eq!(Command::GetStatus.reach(), Reach::Read);
        assert_eq!(Command::ProbeLink { url: String::new() }.reach(), Reach::Read);
        assert_eq!(Command::Pause { id: 1 }.reach(), Reach::Change);
        assert_eq!(Command::Remove { id: 1 }.reach(), Reach::Change);
        assert_eq!(Command::SetSpeedLimit { kbps: None }.reach(), Reach::Wide);
        assert_eq!(Command::ClearCompleted.reach(), Reach::Wide);
        assert!(Command::PauseAll.changes_things());
        assert!(!Command::GetSettings.changes_things());
    }

    #[test]
    fn audit_text_has_no_secrets() {
        let c = Command::AddDownload { url: "https://user:pass@example.com/a/b.zip?token=SECRET#frag".into(), name: None, category: None, queue: Some("Night".into()), start: None };
        let s = c.audit_summary();
        assert_eq!(s, "https://example.com/a/b.zip -> Night");
        assert!(!s.contains("SECRET") && !s.contains("pass"));
        assert_eq!(Command::ProbeLink { url: "nonsense".into() }.audit_summary(), "(not a link)");
        assert_eq!(Command::AddMany { urls: vec!["a".into(), "b".into()], category: None, queue: None, start: None }.audit_summary(), "2 links");
        assert_eq!(Command::SetSpeedLimit { kbps: Some(500) }.audit_summary(), "500 KB/s");
    }

    #[test]
    fn tool_names_match_the_protocol_names() {
        assert_eq!(Command::Pause { id: 1 }.tool_name(), "pause_download");
        assert_eq!(Command::AddMany { urls: vec![], category: None, queue: None, start: None }.tool_name(), "add_downloads");
    }
}
