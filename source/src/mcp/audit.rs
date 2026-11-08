//! A record of what AI assistants did: when, which tool, on what, and how it ended.
//!
//! The newest entries are kept in memory for the AI assistants window and for the `get_audit_log` tool. They are also
//! appended to `mcp-audit.log` in the settings folder (one JSON object per line; the file is rotated at 256 KB, so
//! it cannot grow without end). Links are reduced to host and path before they get here (see `Command::audit_summary`),
//! so the log never holds a password or an access key.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::path::Path;

/// How many entries stay in memory.
pub const KEPT: usize = 200;
const FILE: &str = "mcp-audit.log";
const MAX_FILE_BYTES: u64 = 256 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub at: String,
    pub tool: String,
    pub detail: String,
    pub ok: bool,
    /// The kind of error, when it did not work (`forbidden`, `not_found`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Log {
    entries: VecDeque<Entry>,
}

impl Log {
    pub fn push(&mut self, entry: Entry) {
        self.entries.push_back(entry);
        while self.entries.len() > KEPT {
            self.entries.pop_front();
        }
    }

    /// The newest `n` entries, newest first.
    pub fn recent(&self, n: usize) -> Vec<&Entry> {
        self.entries.iter().rev().take(n).collect()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Start with what an earlier run wrote down.
    pub fn load(dir: &Path) -> Log {
        let mut log = Log::default();
        for e in tail(dir, KEPT) {
            log.push(e);
        }
        log
    }
}

/// Add one entry to the file. The file is moved aside (to `mcp-audit.log.1`, replacing an older one) once it is big.
pub fn append(dir: &Path, entry: &Entry) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(FILE);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
        let _ = std::fs::rename(&path, dir.join(format!("{FILE}.1")));
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "{}", serde_json::to_string(entry).map_err(io::Error::other)?)
}

/// The last `n` entries of the file, oldest first. Lines that cannot be read (a half-written one) are skipped.
pub fn tail(dir: &Path, n: usize) -> Vec<Entry> {
    let Ok(text) = std::fs::read_to_string(dir.join(FILE)) else { return Vec::new() };
    let mut entries: Vec<Entry> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let skip = entries.len().saturating_sub(n);
    entries.drain(..skip);
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(n: usize) -> Entry {
        Entry { at: format!("2026-11-08 10:{n:02}:00"), tool: "add_download".into(), detail: format!("https://example.com/{n}"), ok: n & 1 == 0, error: (n & 1 == 1).then(|| "forbidden".to_string()) }
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sdm-audit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn memory_keeps_the_newest_and_shows_them_newest_first() {
        let mut log = Log::default();
        for n in 0..(KEPT + 5) {
            log.push(Entry { detail: n.to_string(), ..entry(0) });
        }
        assert_eq!(log.len(), KEPT);
        let recent = log.recent(3);
        assert_eq!(recent.iter().map(|e| e.detail.as_str()).collect::<Vec<_>>(), vec!["204", "203", "202"]);
        assert!(Log::default().is_empty());
    }

    #[test]
    fn file_round_trips_and_ignores_damaged_lines() {
        let dir = scratch("roundtrip");
        for n in 0..5 {
            append(&dir, &entry(n)).unwrap();
        }
        std::fs::OpenOptions::new().append(true).open(dir.join(FILE)).unwrap().write_all(b"{half a line").unwrap();
        let back = tail(&dir, 3);
        assert_eq!(back, vec![entry(2), entry(3), entry(4)]);
        assert_eq!(Log::load(&dir).len(), 5);
        assert!(tail(&scratch("none"), 3).is_empty());
    }

    #[test]
    fn file_is_rotated_when_big() {
        let dir = scratch("rotate");
        let big = Entry { detail: "x".repeat(1000), ..entry(0) };
        for _ in 0..300 {
            append(&dir, &big).unwrap();
        }
        assert!(dir.join(format!("{FILE}.1")).exists(), "the old file was moved aside");
        assert!(std::fs::metadata(dir.join(FILE)).unwrap().len() < MAX_FILE_BYTES + 2000);
    }
}
