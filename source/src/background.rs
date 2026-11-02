//! Running in the background: the program keeps downloading with no window, and an icon in the
//! system tray (the notification area on Windows, the menu bar on macOS) opens it again.
//!
//! Closing the window with "keep running" starts `swift_download_manager --background` and exits.
//! The background process owns the downloads through the same lock and hand-over the MCP engine
//! uses, so opening the window from the tray (or starting the program again) simply takes them over.

use crate::app::default_config_dir;
use crate::mcp::server::Engine;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

mod tray;

pub use tray::TrayCmd;

/// How long the background process waits for a closing window to let go of the downloads.
const START_WAIT: Duration = Duration::from_secs(30);

/// Text for the tray tooltip and the status line of its menu.
pub fn status_text(active: usize, queued: usize, speed_bps: f64) -> String {
    if active > 0 {
        let speed = crate::app::human_size(speed_bps.max(0.0) as u64);
        format!("Swift Download Manager: {active} downloading, {speed}/s")
    } else if queued > 0 {
        format!("Swift Download Manager: {queued} waiting")
    } else {
        "Swift Download Manager: idle".to_string()
    }
}

/// Start the background process and return at once. It outlives this process.
pub fn spawn_background() -> std::io::Result<()> {
    spawn_detached(Some("--background"))
}

/// Start another copy of the program (a window) and return at once.
pub fn spawn_window() -> std::io::Result<()> {
    spawn_detached(None)
}

fn spawn_detached(arg: Option<&str>) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(std::env::current_exe()?);
    if let Some(a) = arg {
        cmd.arg(a);
    }
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own process group, so closing a terminal or pressing Ctrl+C here does not stop it.
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
        cmd.creation_flags(0x0000_0008 | 0x0000_0200);
    }
    cmd.spawn().map(|_| ())
}

/// `swift_download_manager --background`. Returns the process exit code.
pub fn run() -> i32 {
    let (cmd_tx, cmd_rx) = mpsc::channel::<TrayCmd>();
    let (status_tx, status_rx) = mpsc::channel::<String>();
    let engine = std::thread::spawn(move || engine_loop(cmd_rx, status_tx));
    // The tray lives on this (the main) thread, which Windows and macOS require. It returns when
    // the engine thread has finished and dropped its end of the status channel.
    tray::run_main(cmd_tx, status_rx);
    let _ = engine.join();
    0
}

/// Owns the downloads and reacts to the tray until it is told to quit or a window takes over.
fn engine_loop(cmd_rx: Receiver<TrayCmd>, status_tx: Sender<String>) {
    let mut engine = Engine::new(default_config_dir());
    if let Err(e) = engine.ensure_local(START_WAIT) {
        eprintln!("swift-download-manager: not running in the background: {e}");
        return;
    }
    if let Some(app) = engine.app_mut() {
        app.enable_browser_bridge();
    }
    let mut last_status = Instant::now() - Duration::from_secs(10);
    loop {
        match cmd_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(TrayCmd::Open) => {
                if let Err(e) = spawn_window() {
                    eprintln!("swift-download-manager: could not open the window: {e}");
                }
            }
            Ok(TrayCmd::PauseAll) => {
                if let Some(app) = engine.app_mut() {
                    app.pause_all();
                }
            }
            Ok(TrayCmd::ResumeAll) => {
                if let Some(app) = engine.app_mut() {
                    app.resume_all();
                }
            }
            Ok(TrayCmd::Quit) | Err(RecvTimeoutError::Disconnected) => {
                engine.shutdown();
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        use crate::mcp::server::Backend;
        engine.idle();
        let Some(app) = engine.app_mut() else {
            // A window opened and took the downloads over: this process has nothing left to do.
            return;
        };
        if last_status.elapsed() >= Duration::from_secs(1) {
            last_status = Instant::now();
            let (active, queued, speed) = app.background_summary();
            if status_tx.send(status_text(active, queued, speed)).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tray_text_says_what_is_going_on() {
        assert_eq!(status_text(0, 0, 0.0), "Swift Download Manager: idle");
        assert_eq!(status_text(0, 3, 0.0), "Swift Download Manager: 3 waiting");
        assert_eq!(status_text(2, 5, 4_620_000.0), "Swift Download Manager: 2 downloading, 4.41 MB/s");
        assert_eq!(status_text(1, 0, -5.0), "Swift Download Manager: 1 downloading, 0 B/s");
    }
}
