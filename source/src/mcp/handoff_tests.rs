//! A window taking over from a windowless engine while a download is running.

use super::command::Command;
use super::control;
use super::lock::acquire_with_handoff;
use super::server::{Backend, Engine};
use crate::app::{App, Launch};
use crate::engine::tests::{server, SIZE};
use eframe::egui;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sdm-handoff-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Settings that keep the download slow enough to hand over half way.
fn write_settings(dir: &Path, folder: &Path) {
    // The test server is on this computer, which assistants may not reach unless allowed.
    let state = json!({ "settings": { "folder": folder, "limit_on": true, "limit_kbps": 1500, "mcp_block_private": false }, "next_id": 1, "items": [] });
    std::fs::write(dir.join("downloads.json"), state.to_string()).unwrap();
}

#[test]
fn handoff_keeps_progress_and_resumes_byte_exact() {
    let dir = scratch("midway");
    let folder = dir.join("dl");
    std::fs::create_dir_all(&folder).unwrap();
    write_settings(&dir, &folder);
    let srv = server();

    // A windowless engine owns the downloads and is part way through a file.
    let mut engine = Engine::new(dir.clone());
    let added = engine
        .run(Command::AddDownload { url: srv.url("/slow/file.bin"), name: Some("file.bin".into()), category: None, queue: None, start: Some(true) })
        .unwrap();
    let id = added["id"].as_u64().unwrap();
    let started = Instant::now();
    loop {
        engine.idle();
        let d = engine.run(Command::GetDownload { id }).unwrap();
        if d["downloaded_bytes"].as_u64().unwrap() >= 1_000_000 {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(30), "the download never got going: {d}");
        std::thread::sleep(Duration::from_millis(50));
    }

    // The window starts and asks for the downloads; the engine keeps answering while it lets go.
    let window_dir = dir.clone();
    let window = std::thread::spawn(move || acquire_with_handoff(&window_dir, Duration::from_secs(15)));
    let asked = Instant::now();
    while !window.is_finished() {
        engine.idle();
        assert!(asked.elapsed() < Duration::from_secs(20), "the hand-over never finished");
        std::thread::sleep(Duration::from_millis(20));
    }
    let lock = window.join().unwrap().expect("the window gets the lock");

    // What was saved: progress kept, paused, flagged to resume.
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("downloads.json")).unwrap()).unwrap();
    let item = &saved["items"][0];
    assert_eq!(item["status"], "Paused");
    let at = item["downloaded"].as_u64().unwrap();
    assert!(at >= 1_000_000 && at < SIZE as u64, "progress was kept: {at}");
    assert_eq!(item["handoff_resume"], true);
    assert!(control::read_info(&dir).is_none(), "the engine stopped advertising itself");

    // The window loads the same list, resumes the download and finishes it byte for byte.
    let mut window = App::build(egui::Context::default(), Launch { headless: true, config_dir: Some(dir.clone()), ..Default::default() });
    let waited = Instant::now();
    loop {
        window.pump();
        if window.run_command(Command::GetDownload { id }).unwrap()["status"] == "complete" {
            break;
        }
        assert!(waited.elapsed() < Duration::from_secs(60), "the resumed download never finished");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(std::fs::read(folder.join("file.bin")).unwrap(), *srv.data);
    drop(lock);
}

#[test]
fn engine_does_not_grab_the_downloads_back_while_a_window_is_opening() {
    let dir = scratch("yield");
    let mut engine = Engine::new(dir.clone());
    engine.busy_wait = Duration::from_millis(400);
    engine.yield_grace = Duration::from_secs(10);
    engine.run(Command::GetStatus).expect("becomes the owner");

    // A window asks for the downloads but has not taken the lock yet.
    let asker_dir = dir.clone();
    let asker = std::thread::spawn(move || control::call_with_timeout(&asker_dir, &Command::Handoff, Duration::from_secs(10)));
    let started = Instant::now();
    while !asker.is_finished() {
        engine.idle();
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(10));
    }
    asker.join().unwrap().expect("the engine hands over");

    // The lock is free for a moment. A tool call now must wait for the window, not take over.
    let answer = engine.run(Command::GetStatus);
    assert!(answer.is_err(), "the engine must not start running the downloads again: {answer:?}");
    assert!(super::lock::StateLock::try_acquire(&dir).unwrap().is_some(), "the lock stayed free for the window");
}
