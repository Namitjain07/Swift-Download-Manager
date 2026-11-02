//! What happens when the window is closed: quit, or keep downloading in the background.

use super::*;
use crate::widgets::{button_row, dialog_window, plain_button, primary_button, tinted_button, toggle};
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
        dialog_window(ctx, "Keep downloading?")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .default_width(crate::widgets::fit_width(ctx, 440.0))
            .show(ctx, |ui| {
                ui.label("Downloads are still running. Swift Download Manager can keep going in the background, with an icon in the system tray, and open again from there.");
                ui.add_space(8.0);
                toggle(ui, &mut remember, "Remember my choice");
                ui.add_space(8.0);
                button_row(ui, |ui| {
                    if primary_button(ui, "Keep in background").clicked() {
                        choice = Some(CloseAction::Background);
                    }
                    if tinted_button(ui, "Quit").clicked() {
                        choice = Some(CloseAction::Quit);
                    }
                    if plain_button(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        self.remember_close = remember;
        if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
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

    /// (downloads running, downloads waiting, total speed in bytes per second)
    pub fn background_summary(&self) -> (usize, usize, f64) {
        let active: Vec<&Item> = self.items.iter().filter(|i| i.status().is_active()).collect();
        (active.len(), self.items.iter().filter(|i| i.status() == Status::Queued).count(), active.iter().map(|i| i.speed()).sum())
    }

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

    /// Accept links from the browser add-on without a window: they start straight away.
    pub fn enable_browser_bridge(&mut self) {
        if self.settings.browser_listen && self.listener.is_err() {
            self.listener = bridge::start_listener(bridge::PORT, self.inbox.clone(), self.ctx.clone());
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
