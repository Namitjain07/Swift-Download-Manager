//! One process at a time owns the download list: whoever holds `state.lock`.
//!
//! The lock is an operating-system file lock, so it is released automatically when the owner
//! exits or crashes. A window that starts while a windowless engine owns the downloads asks it
//! to hand them over (see `acquire_with_handoff`).

use super::command::Command;
use super::control::{self, CallError};
use std::fs::{OpenOptions, TryLockError};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

/// Proof that this process owns the download list. Released when dropped.
pub struct StateLock {
    _file: std::fs::File,
}

impl StateLock {
    /// `Ok(None)` when another process holds the lock.
    pub fn try_acquire(dir: &Path) -> io::Result<Option<StateLock>> {
        std::fs::create_dir_all(dir)?;
        let file = OpenOptions::new().create(true).write(true).truncate(false).open(dir.join("state.lock"))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(StateLock { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(e)) => Err(e),
        }
    }
}

/// Ask the windowless engine that owns the downloads to pause them, save and let go.
fn request_handoff(dir: &Path, timeout: Duration) -> Result<(), String> {
    match control::call_with_timeout(dir, &Command::Handoff, timeout) {
        Ok(_) => Ok(()),
        Err(CallError::NoApp) => Err("the engine is not answering".into()),
        Err(CallError::Tool(m) | CallError::Other(m)) => Err(m),
    }
}

/// Get the lock, asking a windowless engine to hand over if it holds it. Waits at most `wait`;
/// `None` means someone else (another window, or an engine that is not answering) still owns it.
pub fn acquire_with_handoff(dir: &Path, wait: Duration) -> Option<StateLock> {
    let deadline = Instant::now() + wait;
    let mut asked = false;
    loop {
        match StateLock::try_acquire(dir) {
            Ok(Some(lock)) => return Some(lock),
            Ok(None) => {}
            Err(e) => {
                eprintln!("swift-download-manager: could not use the lock file: {e}");
                return None;
            }
        }
        match control::read_info(dir) {
            Some(info) if info.mode == "headless" && !asked => {
                asked = true;
                if let Err(e) = request_handoff(dir, wait) {
                    eprintln!("swift-download-manager: the hand-over did not work: {e}");
                }
                continue;
            }
            // A window that is already open holds the lock for as long as it lives (the lock is
            // released when it exits or crashes), so a second copy has nothing to wait for.
            Some(info) if info.mode == "window" => return None,
            _ => {}
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::control::ControlInfo;
    use std::time::Instant;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sdm-lock-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn second_acquire_is_refused_until_first_drops() {
        let dir = scratch("twice");
        let first = StateLock::try_acquire(&dir).unwrap().expect("a free lock is taken");
        assert!(StateLock::try_acquire(&dir).unwrap().is_none());
        drop(first);
        assert!(StateLock::try_acquire(&dir).unwrap().is_some());
    }

    #[test]
    fn handoff_with_nothing_running_is_instant() {
        let dir = scratch("instant");
        let t = Instant::now();
        assert!(acquire_with_handoff(&dir, Duration::from_secs(5)).is_some());
        assert!(t.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn another_window_owner_is_refused_at_once_not_after_the_wait() {
        let dir = scratch("window");
        let _held = StateLock::try_acquire(&dir).unwrap().unwrap();
        std::fs::write(dir.join("control.json"), serde_json::to_string(&ControlInfo { port: 1, pid: 1, mode: "window".into() }).unwrap()).unwrap();
        let t = Instant::now();
        assert!(acquire_with_handoff(&dir, Duration::from_secs(5)).is_none());
        assert!(t.elapsed() < Duration::from_millis(500), "a second copy of the app must not wait for a live window: {:?}", t.elapsed());
    }

    #[test]
    fn an_owner_that_is_still_starting_is_waited_for_a_little_but_not_forever() {
        let dir = scratch("starting");
        let _held = StateLock::try_acquire(&dir).unwrap().unwrap();
        let t = Instant::now();
        assert!(acquire_with_handoff(&dir, Duration::from_millis(300)).is_none());
        assert!(t.elapsed() >= Duration::from_millis(250) && t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
    }
}
