//! Updates in the app: the check, the window (Help > Check for updates), the status-bar hint and the install.
//! The mechanics (versions, the list of releases, the download and its checksum) are in `crate::update`.

use super::*;
use crate::update::{self, Channel, Handover, Install, Kind, Progress, Release, Server};
use crate::widgets::{card, disclosure, dropdown, field, hint, message, plain_button, primary_button, section_title, segmented, segmented_width, sheet, small_button, spinner, ButtonKind};
use eframe::egui::RichText;
use std::sync::mpsc::{channel, Receiver, Sender};

/// How often the list is looked at on its own (when that is switched on).
const AUTO_EVERY_SECS: i64 = 20 * 60 * 60;
/// Not before this long after start-up, so that the window opens fast and downloads get the line first.
const AUTO_AFTER_START: u64 = 10;

/// What came back from a background thread.
enum Msg {
    Feed { result: Result<Vec<Release>, String>, manual: bool },
    Downloaded(Result<(PathBuf, Kind, String), String>),
}

/// What the update machinery is doing.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    Checking,
    Downloading(String),
    /// The file is here and checked: waiting for the person to restart into it.
    Ready { file: PathBuf, kind: Kind, tag: String },
}

pub struct Updates {
    pub open: bool,
    pub phase: Phase,
    pub releases: Vec<Release>,
    /// The last thing that went wrong, for the window.
    pub error: Option<String>,
    /// When the list was last read in this run (`None`: not yet).
    pub checked: Option<Instant>,
    /// A version picked in the list that is waiting for the person to confirm.
    pub confirm: Option<String>,
    /// What to say after installing opened another program or needs a command.
    pub note: Option<String>,
    progress: Arc<Progress>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    started: Instant,
}

impl Updates {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Updates { open: false, phase: Phase::Idle, releases: Vec::new(), error: None, checked: None, confirm: None, note: None, progress: Arc::new(Progress::default()), tx, rx, started: Instant::now() }
    }
}

/// Seconds since 1970.
fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Where downloaded updates are kept until they are installed.
fn updates_dir() -> PathBuf {
    default_config_dir().join("updates")
}

impl App {
    /// The newest version this computer could update to (and has not been told to skip), if the list says there is one.
    pub(super) fn available_update(&self) -> Option<&Release> {
        let r = update::newest(&self.updates.releases, self.settings.update_channel, &update::current())?;
        (r.tag != self.settings.update_skip).then_some(r)
    }

    /// Look at the list of versions in the background. `manual` is a press of the button: it says "up to date" afterwards.
    pub(super) fn check_updates(&mut self, manual: bool) {
        if self.demo || self.updates.phase == Phase::Checking {
            return;
        }
        self.updates.phase = Phase::Checking;
        self.updates.error = None;
        let url = self.settings.update_server.feed_url(&self.settings.update_url);
        let (tx, ctx) = (self.updates.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Feed { result: update::fetch_feed(&url), manual });
            ctx.request_repaint();
        });
    }

    /// Download the file of `tag` (any version in the list: a newer one, or an older one to go back to).
    pub(super) fn download_update(&mut self, tag: &str) {
        let Some(release) = self.updates.releases.iter().find(|r| r.tag == tag).cloned() else { return };
        let how = Install::detect();
        let Some((asset, kind)) = update::pick_asset(&release, how) else {
            // A copy that cannot be updated from here (built from source, macOS ...): the page of the release.
            open_path(std::path::Path::new(&release.url));
            self.updates.note = Some("This copy cannot update itself, so the page of that version was opened.".into());
            return;
        };
        let asset = asset.clone();
        self.updates.phase = Phase::Downloading(release.tag.clone());
        self.updates.error = None;
        self.updates.progress = Arc::new(Progress::default());
        let (tx, ctx, progress, tag) = (self.updates.tx.clone(), self.ctx.clone(), self.updates.progress.clone(), release.tag.clone());
        std::thread::spawn(move || {
            let result = update::download(&asset, &updates_dir().join(&tag), &progress).map(|f| (f, kind, tag));
            let _ = tx.send(Msg::Downloaded(result));
            ctx.request_repaint();
        });
    }

    /// Hand the downloaded file over to be installed. Windows: the program quits and the new one starts.
    pub(super) fn install_update(&mut self) {
        let Phase::Ready { file, kind, .. } = self.updates.phase.clone() else { return };
        match update::install(&file, kind) {
            Ok(Handover::Quit) => {
                self.save();
                self.quitting = true;
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Ok(Handover::Opened(p)) => {
                self.updates.note = Some(format!("The installer of your system was opened with {}.", p.display()));
                self.updates.phase = Phase::Idle;
            }
            Ok(Handover::Command(c)) => {
                self.updates.note = Some(format!("Run this to install it: {c}"));
                self.updates.phase = Phase::Idle;
            }
            Err(e) => {
                self.updates.error = Some(e);
                self.updates.phase = Phase::Idle;
            }
        }
    }

    /// Per-frame: results of the background work, and the automatic check.
    pub(super) fn tick_updates(&mut self) {
        while let Ok(msg) = self.updates.rx.try_recv() {
            match msg {
                Msg::Feed { result, manual } => {
                    self.updates.phase = Phase::Idle;
                    self.updates.checked = Some(Instant::now());
                    match result {
                        Ok(list) => {
                            self.settings.update_last = now_secs();
                            self.dirty = true;
                            self.updates.releases = list;
                            if let Some(r) = self.available_update().cloned() {
                                if !manual {
                                    self.toast(format!("Version {} is available (Help > Check for updates)", r.version));
                                }
                                if self.settings.update_download && !manual {
                                    self.download_update(&r.tag);
                                }
                            }
                        }
                        Err(e) => {
                            // A quiet failure for the automatic check (no network is normal); the window shows it when asked.
                            if manual || self.updates.open {
                                self.updates.error = Some(e);
                            }
                        }
                    }
                }
                Msg::Downloaded(Ok((file, kind, tag))) => {
                    self.toast(format!("Version {} is ready to install", tag.trim_start_matches('v')));
                    self.updates.phase = Phase::Ready { file, kind, tag };
                }
                Msg::Downloaded(Err(e)) => {
                    self.updates.phase = Phase::Idle;
                    if e != "Cancelled" {
                        self.updates.error = Some(e);
                    }
                }
            }
        }
        // The automatic check: once a day, a little after start-up, and never in the sample window or a headless engine.
        let due = now_secs() - self.settings.update_last >= AUTO_EVERY_SECS;
        if self.settings.update_check && !self.demo && !self.headless && self.updates.phase == Phase::Idle && self.updates.checked.is_none() && due && self.updates.started.elapsed().as_secs() >= AUTO_AFTER_START {
            self.check_updates(false);
        }
        if matches!(self.updates.phase, Phase::Checking | Phase::Downloading(_)) {
            crate::widgets::anim::repaint_soon(&self.ctx);
        }
    }

    /// The window: where the program stands, what is new, the other versions, and the settings.
    pub(super) fn updates_window(&mut self, ctx: &egui::Context) {
        if !self.updates.open {
            return;
        }
        let (pal, d) = (theme::current(ctx), theme::design(ctx));
        let current = update::current();
        
        let mut act: Option<UpdateAct> = None;
        let mut settings = self.settings.clone();
        let out = sheet("updates", "Updates").width(580.0).show(ctx, |s| {
            s.body(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{APP_NAME} {current}")).size(d.body + 1.0).family(theme::semibold()).color(pal.label));
                    if current.is_prerelease() {
                        ui.label(RichText::new("pre-release").color(pal.orange_ink));
                    }
                });
                ui.add_space(6.0);
                // ---- where things stand
                match self.updates.phase.clone() {
                    Phase::Checking => {
                        ui.horizontal(|ui| {
                            spinner(ui, 18.0);
                            ui.label("Looking for a newer version…");
                        });
                    }
                    Phase::Downloading(tag) => {
                        let (done, total) = (self.updates.progress.done.load(Ordering::Relaxed), self.updates.progress.total.load(Ordering::Relaxed));
                        ui.label(RichText::new(format!("Downloading {}", tag.trim_start_matches('v'))).family(theme::semibold()).color(pal.label));
                        let frac = if total > 0 { done as f32 / total as f32 } else { 0.0 };
                        crate::widgets::capsule_bar(ui, egui::Id::unique("update_bar"), frac, pal.accent, 8.0, None);
                        ui.label(RichText::new(if total > 0 { format!("{} of {}", human_size(done), human_size(total)) } else { human_size(done) }).size(d.caption).color(pal.tertiary));
                        if small_button(ui, ButtonKind::Secondary, "Cancel").clicked() {
                            act = Some(UpdateAct::CancelDownload);
                        }
                    }
                    Phase::Ready { tag, kind, .. } => {
                        ui.label(RichText::new(format!("Version {} is ready", tag.trim_start_matches('v'))).family(theme::semibold()).color(pal.green_ink));
                        ui.label(RichText::new(match kind {
                            Kind::WindowsInstaller => "Installing closes this window, keeps your downloads and settings, and starts the new version.",
                            Kind::WindowsPortable => "The program is replaced by the new one and starts again. Your downloads and settings are kept.",
                            Kind::Deb => "The installer of your system opens with the package.",
                            Kind::ArchPackage => "You get the command to install the package.",
                        }).color(pal.secondary));
                        ui.add_space(6.0);
                        let label = if matches!(kind, Kind::WindowsInstaller | Kind::WindowsPortable) { "Install and restart" } else { "Install" };
                        if primary_button(ui, label).clicked() {
                            act = Some(UpdateAct::Install);
                        }
                    }
                    Phase::Idle => {
                        if let Some(r) = self.available_update().cloned() {
                            ui.label(RichText::new(format!("Version {} is available", r.version)).size(d.body + 1.0).family(theme::semibold()).color(pal.accent_ink));
                            ui.label(RichText::new(format!("Published {}{}", r.day(), if r.prerelease { " · pre-release" } else { "" })).size(d.caption).color(pal.tertiary));
                            if !r.notes.trim().is_empty() {
                                ui.add_space(6.0);
                                egui::ScrollArea::vertical().max_height(150.0).id_salt("update_notes").show(ui, |ui| {
                                    for line in r.notes.lines().map(str::trim).filter(|l| !l.is_empty()).take(40) {
                                        let heading = line.starts_with('#');
                                        let text = line.trim_start_matches(['#', ' ']).replace("**", "").replace('`', "");
                                        let rich = if heading { RichText::new(text).size(d.caption + 1.0).family(theme::semibold()).color(pal.label) } else { RichText::new(text).size(d.caption + 0.5).color(pal.secondary) };
                                        ui.add(egui::Label::new(rich).wrap());
                                    }
                                });
                            }
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if primary_button(ui, "Update now").clicked() {
                                    act = Some(UpdateAct::Download(r.tag.clone()));
                                }
                                if plain_button(ui, "Skip this version").clicked() {
                                    act = Some(UpdateAct::Skip(r.tag.clone()));
                                }
                            });
                        } else if self.updates.checked.is_some() || self.settings.update_last > 0 {
                            ui.label(RichText::new("You have the newest version.").size(d.body + 1.0).family(theme::semibold()).color(pal.green_ink));
                            ui.label(RichText::new(if self.updates.checked.is_some() { "Checked just now.".to_string() } else { format!("Last checked {}.", day_text(self.settings.update_last)) }).size(d.caption).color(pal.tertiary));
                        } else {
                            ui.label("Not checked yet.");
                        }
                        ui.add_space(8.0);
                        if small_button(ui, ButtonKind::Secondary, "Check now").clicked() {
                            act = Some(UpdateAct::Check);
                        }
                    }
                }
                if let Some(e) = &self.updates.error {
                    ui.add_space(8.0);
                    message(ui, e, true);
                }
                if let Some(n) = &self.updates.note {
                    ui.add_space(8.0);
                    message(ui, n, false);
                }

                // ---- every version
                ui.add_space(14.0);
                disclosure(ui, "updates_versions", "Other versions (also to go back to an older one)", |ui| {
                    ui.add_space(4.0);
                    if self.updates.releases.is_empty() {
                        hint(ui, "The list is read when you press Check now.");
                    }
                    let busy = !matches!(self.updates.phase, Phase::Idle);
                    for r in self.updates.releases.iter().filter(|r| settings.update_channel == Channel::Beta || !r.prerelease || r.version == current).take(30) {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(r.version.to_string()).family(theme::semibold()).color(pal.label));
                            ui.label(RichText::new(r.day()).size(d.caption).color(pal.tertiary));
                            if r.prerelease {
                                ui.label(RichText::new("pre-release").size(d.caption).color(pal.orange_ink));
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if r.version == current {
                                    ui.label(RichText::new("Installed").size(d.caption).color(pal.green_ink));
                                } else if self.updates.confirm.as_deref() == Some(r.tag.as_str()) {
                                    if small_button(ui, ButtonKind::Primary, "Install").clicked() {
                                        act = Some(UpdateAct::Download(r.tag.clone()));
                                    }
                                    if small_button(ui, ButtonKind::Secondary, "No").clicked() {
                                        act = Some(UpdateAct::Confirm(None));
                                    }
                                    ui.label(RichText::new(if r.version < current { "Go back? Downloads and settings stay." } else { "Install it?" }).size(d.caption).color(pal.secondary));
                                } else {
                                    ui.add_enabled_ui(!busy, |ui| {
                                        if small_button(ui, ButtonKind::Secondary, if r.version < current { "Go back to this" } else { "Install" }).clicked() {
                                            act = Some(UpdateAct::Confirm(Some(r.tag.clone())));
                                        }
                                    });
                                }
                            });
                        });
                    }
                });

                // ---- settings
                ui.add_space(4.0);
                disclosure(ui, "updates_settings", "Settings", |ui| {
                    ui.add_space(4.0);
                    let channel_w = segmented_width(ui, &["Stable", "With betas"]);
                    card(ui, |g| {
                        g.toggle("Look for updates by itself", Some("Once a day, at most. Only the list of versions is fetched; nothing about you is sent."), &mut settings.update_check);
                        g.toggle("Download them in the background", Some("Then it only waits for you to restart into it."), &mut settings.update_download);
                        g.row("Versions", Some("Betas are tried out before they are finished."), channel_w, |ui| segmented(ui, &[(Channel::Stable, "Stable"), (Channel::Beta, "With betas")], &mut settings.update_channel));
                        g.row("Get them from", None, 220.0, |ui| {
                            let options: Vec<(Server, String)> = [Server::GitHub, Server::Website, Server::Custom].into_iter().map(|s| (s, s.label().to_string())).collect();
                            dropdown(ui, "update_server", 220.0, &options, &mut settings.update_server)
                        });
                        if settings.update_server == Server::Custom {
                            g.free(|ui| {
                                ui.add(field(&mut settings.update_url).hint_text("https://updates.example.org/releases.json").desired_width(ui.available_width()));
                            });
                        }
                    });
                    if settings.update_server == Server::Custom {
                        hint(ui, "A web address (https) that serves the list of releases in the same shape as GitHub's API: tag_name, prerelease, published_at, body and assets with name, size, browser_download_url and digest (sha256:…). Files are only installed when their checksum matches.");
                    }
                    section_title(ui, "Safety");
                    hint(ui, "A downloaded file is only installed when its SHA-256 matches the one in the list, and files come only from the addresses in the list.");
                });
            });
            s.footer(|ui| {
                if primary_button(ui, "Close").clicked() {
                    act = Some(UpdateAct::Close);
                }
            });
        });
        // Settings the person changed here take effect (and are saved) at once.
        if settings.update_check != self.settings.update_check
            || settings.update_download != self.settings.update_download
            || settings.update_channel != self.settings.update_channel
            || settings.update_server != self.settings.update_server
            || settings.update_url != self.settings.update_url
        {
            self.settings = settings;
            self.dirty = true;
        }
        if out.close {
            act = Some(UpdateAct::Close);
        }
        match act {
            Some(UpdateAct::Close) => self.updates.open = false,
            Some(UpdateAct::Check) => self.check_updates(true),
            Some(UpdateAct::Download(tag)) => {
                self.updates.confirm = None;
                self.updates.note = None;
                self.download_update(&tag);
            }
            Some(UpdateAct::Confirm(tag)) => self.updates.confirm = tag,
            Some(UpdateAct::Skip(tag)) => {
                self.settings.update_skip = tag;
                self.dirty = true;
            }
            Some(UpdateAct::CancelDownload) => self.updates.progress.cancel.store(true, Ordering::SeqCst),
            Some(UpdateAct::Install) => self.install_update(),
            None => {}
        }
    }

    /// The little hint in the status bar: an update is available, downloading or waiting for a restart.
    pub(super) fn update_hint(&self) -> Option<(String, bool)> {
        match &self.updates.phase {
            Phase::Ready { tag, .. } => Some((format!("Restart to update to {}", tag.trim_start_matches('v')), true)),
            Phase::Downloading(tag) => Some((format!("Downloading {}…", tag.trim_start_matches('v')), false)),
            _ => self.available_update().map(|r| (format!("Update available: {}", r.version), true)),
        }
    }
}

enum UpdateAct {
    Close,
    Check,
    Download(String),
    Confirm(Option<String>),
    Skip(String),
    CancelDownload,
    Install,
}

/// "3 days ago", "today", from seconds since 1970.
fn day_text(secs: i64) -> String {
    let days = (now_secs() - secs).max(0) / 86_400;
    match days {
        0 => "today".to_string(),
        1 => "yesterday".to_string(),
        n => format!("{n} days ago"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_day_reads_as_a_few_words() {
        assert_eq!(day_text(now_secs()), "today");
        assert_eq!(day_text(now_secs() - 90_000), "yesterday");
        assert_eq!(day_text(now_secs() - 5 * 86_400 - 100), "5 days ago");
        assert_eq!(day_text(now_secs() + 1000), "today", "a clock set back is not a crash");
    }
}

impl App {
    /// Sample versions for the pictures of the window (`--show updates`): one newer than this one, the ones before it.
    pub(super) fn show_sample_updates(&mut self) {
        let release = |tag: &str, day: &str, pre: bool, notes: &str| Release {
            version: update::Version::parse(tag).expect("a sample tag"),
            tag: tag.to_string(),
            prerelease: pre,
            published: format!("{day}T12:00:00Z"),
            notes: notes.to_string(),
            url: String::new(),
            assets: Vec::new(),
        };
        self.updates.releases = vec![
            release("v1.10.0", "2026-12-02", false, "### Added\n- Onboarding with tips\n- Updates from inside the app, and going back to an older version\n### Fixed\n- Connection problems no longer linger after the download recovered"),
            release("v1.9.0", "2026-11-08", false, "- WinUI 3 theme\n- A stronger MCP server"),
            release("v1.8.0", "2026-11-06", false, "- Fourteen themes"),
            release("v1.7.0", "2026-11-06", false, "- One design for the whole app"),
            release("v1.6.0-beta.1", "2026-11-02", true, "- Web interface"),
        ];
        self.updates.checked = Some(Instant::now());
        self.updates.open = true;
    }
}
