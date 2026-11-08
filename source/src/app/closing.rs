//! What happens when the window is closed: quit, or keep downloading in the background.

use super::*;
use crate::bridge::Source;
use crate::widgets::{check_row, plain_button, primary_button, secondary_button, sheet};
use serde::{Deserialize, Serialize};

/// What the setting says to do when the window is closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BackgroundMode {
    /// Ask when something is running; quit when nothing is.
    #[default]
    Ask,
    /// Always keep running in the background (tray icon), even when idle.
    Keep,
    /// Always quit.
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseAction {
    Quit,
    Background,
    Ask,
}

/// The decision table. `busy` means downloads are running or waiting for a queue or a schedule.
pub fn close_action(mode: BackgroundMode, busy: bool) -> CloseAction {
    match (mode, busy) {
        (BackgroundMode::Keep, _) => CloseAction::Background,
        (BackgroundMode::Quit, _) => CloseAction::Quit,
        (BackgroundMode::Ask, true) => CloseAction::Ask,
        (BackgroundMode::Ask, false) => CloseAction::Quit,
    }
}

impl App {
    /// True when closing the window would interrupt something: running downloads, a running queue,
    /// or a queue that is scheduled to start by itself.
    pub(super) fn needs_background(&self) -> bool {
        self.items.iter().any(|i| i.status().is_active()) || !self.running.is_empty() || self.settings.queues.iter().any(|q| q.schedule.start_on)
    }

    /// Called when the person closes the window.
    pub(super) fn on_close_requested(&mut self, ctx: &egui::Context) {
        match close_action(self.settings.background, self.needs_background()) {
            CloseAction::Quit => self.quitting = true,
            CloseAction::Background => {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.go_background(ctx);
            }
            CloseAction::Ask => {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.ask_close = true;
            }
        }
    }

    pub(super) fn quit_now(&mut self, ctx: &egui::Context) {
        self.quitting = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// Hand the downloads to a background process (with a tray icon) and close the window.
    pub(super) fn go_background(&mut self, ctx: &egui::Context) {
        if self.demo || self.headless {
            return;
        }
        // Pause what runs (it is flagged to resume), save, and let go of the download list.
        self.prepare_handoff();
        self.close_control();
        self.state_lock = None;
        match crate::background::spawn_background() {
            Ok(()) => self.quit_now(ctx),
            Err(e) => {
                // Could not start it: take everything back and stay open.
                self.take_state_lock();
                self.resume_handed_over();
                self.start_control();
                self.toast(format!("Could not keep running in the background: {e}"));
            }
        }
    }

    /// The question asked when closing with downloads running.
    pub(super) fn close_window(&mut self, ctx: &egui::Context) {
        if !self.ask_close {
            return;
        }
        let mut remember = self.remember_close;
        let mut choice: Option<CloseAction> = None;
        let mut cancel = false;
        let pal = theme::current(ctx);
        let out = sheet("close", "Keep downloading?").width(480.0).show(ctx, |s| {
            s.body(|ui| {
                ui.add(egui::Label::new(egui::RichText::new("Downloads are still running. Swift Download Manager can keep going in the background, with an icon in the system tray, and open again from there.").color(pal.secondary)).wrap());
                ui.add_space(10.0);
                check_row(ui, &mut remember, "Remember my choice");
            });
            s.footer_split(
                |ui| {
                    if plain_button(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                },
                |ui| {
                    if primary_button(ui, "Keep in background").clicked() {
                        choice = Some(CloseAction::Background);
                    }
                    if secondary_button(ui, "Quit").clicked() {
                        choice = Some(CloseAction::Quit);
                    }
                },
            );
        });
        self.remember_close = remember;
        if cancel || out.close {
            self.ask_close = false;
        }
        if let Some(c) = choice {
            self.ask_close = false;
            if self.remember_close {
                self.settings.background = if c == CloseAction::Background { BackgroundMode::Keep } else { BackgroundMode::Quit };
                self.dirty = true;
            }
            if c == CloseAction::Background { self.go_background(ctx) } else { self.quit_now(ctx) }
        }
    }

    // ---- used by the background process (no window)


    /// Pause everything that is running.
    pub fn pause_all(&mut self) {
        self.stop_all();
    }

    /// Resume paused downloads and run the queues that have something waiting.
    pub fn resume_all(&mut self) {
        let paused: Vec<u64> = self.items.iter().filter(|i| i.status() == Status::Paused).map(|i| i.id).collect();
        for id in paused {
            self.start_item(id);
        }
        let queues: Vec<String> = self.settings.queues.iter().filter(|q| self.items.iter().any(|i| i.queue.as_deref() == Some(q.name.as_str()) && i.status() == Status::Queued)).map(|q| q.name.clone()).collect();
        for q in queues {
            self.start_queue(&q);
        }
    }

    // ---- the tray icon (in the window's process and in the background one)

    /// What the tray shows now: the status line, the running downloads with their percentages, and the switches.
    pub fn tray_state(&self) -> crate::background::TrayState {
        use crate::background::{row_text, TrayState, MAX_ROWS};
        let active: Vec<&Item> = self.items.iter().filter(|i| i.status().is_active()).collect();
        let queued = self.items.iter().filter(|i| i.status() == Status::Queued).count();
        TrayState {
            status: crate::background::status_text(active.len(), queued, active.iter().map(|i| i.speed()).sum()),
            rows: active.iter().take(MAX_ROWS).map(|i| row_text((i.progress() * 100.0) as u32, &i.name)).collect(),
            autostart: self.settings.launch_at_login,
            update: self.update_hint().filter(|(_, strong)| *strong).map(|(t, _)| t),
        }
    }

    /// Add the link on the clipboard (it starts at once in the background; the window asks first).
    pub fn add_clipboard_link(&mut self) {
        let text = arboard::Clipboard::new().and_then(|mut b| b.get_text()).unwrap_or_default();
        let link = text.trim();
        if (link.starts_with("http://") || link.starts_with("https://")) && !link.contains(char::is_whitespace) && url::Url::parse(link).is_ok() {
            self.inbox.lock().unwrap().push(Incoming { url: link.to_string(), name: String::new(), referer: String::new(), cookies: String::new(), user_agent: String::new(), source: Source::CommandLine });
            self.ctx.request_repaint();
        } else {
            self.toast("There is no download link on the clipboard");
        }
    }

    pub fn open_download_folder(&self) {
        open_path(&self.settings.folder);
    }

    /// Start with the system: on or off.
    pub fn set_autostart(&mut self, on: bool) {
        let done = if on { crate::autostart::enable() } else { crate::autostart::disable() };
        match done {
            Ok(()) => {
                self.settings.launch_at_login = on;
                self.dirty = true;
            }
            Err(e) => self.toast(format!("Could not change starting with the system: {e}")),
        }
    }

    /// The tray icon of the window: made on the first frame (it must be made by the thread that runs the window), kept
    /// up to date once a second, and listened to.
    pub(super) fn tray_tick(&mut self, ctx: &egui::Context) {
        if self.demo || self.headless || self.shot.is_some() {
            return;
        }
        if self.tray.is_none() && !self.tray_tried {
            self.tray_tried = true;
            match crate::background::Tray::new(&self.tray_state()) {
                Ok(t) => self.tray = Some(t),
                Err(e) => eprintln!("swift-download-manager: no tray icon: {e}"),
            }
        }
        let cmds = self.tray.as_ref().map(|t| t.poll()).unwrap_or_default();
        for cmd in cmds {
            use crate::background::TrayCmd;
            match cmd {
                TrayCmd::Open => self.raise_window(ctx),
                TrayCmd::PauseAll => self.pause_all(),
                TrayCmd::ResumeAll => self.resume_all(),
                TrayCmd::AddClipboard => {
                    self.add_clipboard_link();
                    self.raise_window(ctx);
                }
                TrayCmd::OpenFolder => self.open_download_folder(),
                TrayCmd::CheckUpdates => {
                    self.raise_window(ctx);
                    self.updates.open = true;
                    self.check_updates(true);
                }
                TrayCmd::Autostart(on) => self.set_autostart(on),
                TrayCmd::Quit => self.quit_now(ctx),
            }
        }
        if self.tray_shown.elapsed().as_millis() >= 1000 {
            self.tray_shown = Instant::now();
            let state = self.tray_state();
            if let Some(t) = &mut self.tray {
                t.update(&state);
            }
        }
        // The tray's clicks arrive while the window may be hidden: look a few times a second even then.
        ctx.request_repaint_after(std::time::Duration::from_millis(if self.tray.is_some() { 400 } else { 15_000 }));
    }

    /// Bring the window back, whatever state it is in (hidden, minimized, behind others).
    pub(super) fn raise_window(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    // ---- the browser add-on's window into the app (see `bridge`)

    /// The status the pop-up of the add-on shows, as JSON: totals and the newest unfinished downloads.
    fn bridge_snapshot(&self) -> String {
        let name = |s: Status| match s {
            Status::Queued => "queued",
            Status::Connecting => "connecting",
            Status::Downloading => "downloading",
            Status::Paused => "paused",
            Status::Complete => "complete",
            Status::Error => "error",
        };
        let rows: Vec<serde_json::Value> = self
            .items
            .iter()
            .rev()
            .filter(|i| i.status() != Status::Complete)
            .take(8)
            .map(|i| serde_json::json!({ "id": i.id, "name": i.name, "status": name(i.status()), "progress": i.progress(), "speed": i.speed() as u64, "size": i.total() }))
            .collect();
        let count = |f: &dyn Fn(Status) -> bool| self.items.iter().filter(|i| f(i.status())).count();
        serde_json::json!({
            "app": "swift-download-manager",
            "version": env!("CARGO_PKG_VERSION"),
            "active": count(&|s| s.is_active()),
            "queued": count(&|s| s == Status::Queued),
            "paused": count(&|s| s == Status::Paused),
            "complete": count(&|s| s == Status::Complete),
            "speed": self.items.iter().filter(|i| i.status().is_active()).map(|i| i.speed()).sum::<f64>() as u64,
            "update": self.update_hint().filter(|(_, strong)| *strong).map(|(t, _)| t),
            "downloads": rows,
        })
        .to_string()
    }

    /// Per frame: keep the snapshot fresh (half a second old at most) and do what the add-on asked.
    pub(super) fn bridge_tick(&mut self, ctx: &egui::Context) {
        use crate::bridge::Command;
        if self.listener.is_err() {
            return;
        }
        if self.bridge_shown.elapsed().as_millis() >= 500 {
            self.bridge_shown = Instant::now();
            *self.bridge_link.status.lock().unwrap() = self.bridge_snapshot();
        }
        let cmds = std::mem::take(&mut *self.bridge_link.commands.lock().unwrap());
        for c in cmds {
            match c {
                Command::Pause(id) => self.stop_item(id),
                Command::Resume(id) => self.start_item(id),
                Command::PauseAll => self.stop_all(),
                Command::ResumeAll => self.resume_all(),
                Command::Open if self.headless => {
                    let _ = crate::background::spawn_window();
                }
                Command::Open => self.raise_window(ctx),
            }
        }
    }

    /// Accept links from the browser add-on without a window: they start straight away.
    pub fn enable_browser_bridge(&mut self) {
        if self.settings.browser_listen && self.listener.is_err() {
            self.listener = bridge::start_listener_with(bridge::PORT, self.inbox.clone(), self.bridge_link.clone(), self.ctx.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_close_decision_table() {
        use BackgroundMode as M;
        use CloseAction as A;
        assert_eq!(close_action(M::Ask, true), A::Ask);
        assert_eq!(close_action(M::Ask, false), A::Quit);
        assert_eq!(close_action(M::Keep, true), A::Background);
        assert_eq!(close_action(M::Keep, false), A::Background, "Keep means always");
        assert_eq!(close_action(M::Quit, true), A::Quit);
        assert_eq!(close_action(M::Quit, false), A::Quit);
    }

    #[test]
    fn old_settings_ask_and_a_saved_choice_is_kept() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.background, BackgroundMode::Ask);
        assert!(!s.launch_at_login);
        let s: Settings = serde_json::from_str(r#"{"background":"Keep","launch_at_login":true}"#).unwrap();
        assert_eq!((s.background, s.launch_at_login), (BackgroundMode::Keep, true));
    }
}
