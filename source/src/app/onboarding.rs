//! The first start: a short welcome in six steps (what it does, the look, how it should behave, the browser,
//! updates and privacy, and a few tips). Every choice is applied at once, so the window behind shows the theme as
//! it is picked, and every step can be skipped. Shown once; `--show onboarding` shows it again for pictures.

use super::*;
use super::closing::BackgroundMode;
use crate::icons::Icon;
use crate::widgets::{anim, card, hint, plain_button, primary_button, secondary_button, segmented, segmented_width, sheet, stepper, FlowChannel, FlowState};
use eframe::egui::{Color32, ColorImage, RichText, TextureHandle, TextureOptions};
use std::collections::HashMap;

/// How many steps there are.
pub const STEPS: usize = 6;

const THEMES_PNG: &[u8] = include_bytes!("../../assets/onboarding/themes.png");
const BROWSER_PNG: &[u8] = include_bytes!("../../assets/onboarding/browser.png");
const NETWORKS_PNG: &[u8] = include_bytes!("../../assets/onboarding/networks.png");
const AI_PNG: &[u8] = include_bytes!("../../assets/onboarding/ai.png");

pub struct Onboarding {
    pub step: usize,
    /// When the step began (seconds of the window's clock), for the way it comes in.
    at: Option<f64>,
    /// The pictures, loaded when first needed and dropped with the welcome.
    textures: HashMap<&'static str, TextureHandle>,
}

impl Onboarding {
    pub fn at_step(step: usize) -> Self {
        Onboarding { step: step.min(STEPS - 1), at: None, textures: HashMap::new() }
    }
}

/// One of the pictures, decoded and uploaded the first time it is asked for.
fn picture<'a>(textures: &'a mut HashMap<&'static str, TextureHandle>, ctx: &egui::Context, name: &'static str, bytes: &[u8]) -> Option<&'a TextureHandle> {
    if !textures.contains_key(name) {
        let img = image::load_from_memory(bytes).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        textures.insert(name, ctx.load_texture(name, ColorImage::from_rgba_unmultiplied(size, img.as_raw()), TextureOptions::LINEAR));
    }
    textures.get(name)
}

/// A picture as wide as the room (never wider than it is), in a rounded frame.
fn show_picture(ui: &mut egui::Ui, tex: &TextureHandle, max_w: f32) {
    let pal = theme::current(ui.ctx());
    let [w, h] = tex.size();
    let width = ui.available_width().min(max_w).min(w as f32);
    let size = egui::vec2(width, width * h as f32 / w as f32);
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let radius = egui::CornerRadius::same(theme::design(ui.ctx()).radius_md);
        ui.painter().rect_filled(rect.expand(1.0), radius, pal.separator_strong);
        egui::Image::new((tex.id(), size)).corner_radius(radius).paint_at(ui, rect);
    });
}

/// 0 to 1 as step content comes in: `delay` seconds after the step began, over a quarter of a second.
fn arrive(ctx: &egui::Context, at: f64, delay: f32) -> f32 {
    let t = (ctx.input(|i| i.time) - at) as f32 - delay;
    let k = anim::ease_out((t / 0.28).clamp(0.0, 1.0));
    if k < 1.0 {
        ctx.request_repaint();
    }
    k
}

impl App {
    /// The welcome window, while it is open. Returns nothing: it changes the settings as it goes.
    pub(super) fn onboarding_window(&mut self, ctx: &egui::Context) {
        let Some(mut ob) = self.onboarding.take() else { return };
        let (pal, d) = (theme::current(ctx), theme::design(ctx));
        let now = ctx.input(|i| i.time);
        let at = *ob.at.get_or_insert(now);
        let step = ob.step;
        let go = std::cell::Cell::new(None::<isize>);
        let finish = std::cell::Cell::new(false);
        let mut open_browser = false;
        let was_autostart = self.settings.launch_at_login;

        let title = ["Welcome", "Make it yours", "How it should behave", "Your browser", "Updates and privacy", "A few tips"][step];
        let out = sheet("welcome", title).width(720.0).show(ctx, |s| {
            s.body(|ui| {
                ui.set_min_height(360.0);
                let mode_w = segmented_width(ui, &["Automatic", "Light", "Dark"]);
                let close_w = segmented_width(ui, &["Ask", "Keep running", "Quit"]);
                let k = arrive(ctx, at, 0.0);
                ui.set_opacity(k);
                ui.add_space((1.0 - k) * 12.0);
                match step {
                    0 => {
                        // Eight channels running into one file: the program's picture.
                        let t = now as f32;
                        let channels: Vec<FlowChannel> = (0..8).map(|i| FlowChannel { rate: (0.55 + 0.45 * (t * 0.9 + i as f32 * 1.3).sin()).clamp(0.1, 1.0), done: false }).collect();
                        crate::widgets::flow(ui, "welcome_flow", &channels, (t * 0.12).fract(), FlowState::Running, 124.0);
                        ui.add_space(10.0);
                        ui.label(RichText::new("Downloads, accelerated.").size(d.heading).family(theme::semibold()).color(pal.label));
                        ui.add_space(4.0);
                        ui.add(egui::Label::new(RichText::new("Swift Download Manager splits every file into several connections, keeps going when one breaks, and picks up where it left off after a restart. It takes a minute to set up, and you can change everything later in Options.").size(d.body).color(pal.secondary)).wrap());
                        ui.add_space(12.0);
                        for (icon, text) in [
                            (Icon::Speed, "Up to 32 connections per file, and Wi-Fi, Ethernet and a phone's tethering at the same time"),
                            (Icon::Queue, "Queues and a scheduler, pause and resume, links from your browser, AI assistants and a web page"),
                            (Icon::CheckCircle, "Free and open source. It talks only to the servers you download from."),
                        ] {
                            ui.horizontal_top(|ui| {
                                icon_here(ui, icon, 22.0, pal.accent_ink);
                                ui.add(egui::Label::new(RichText::new(text).color(pal.label)).wrap());
                            });
                            ui.add_space(4.0);
                        }
                    }
                    1 => {
                        card(ui, |g| {
                            g.row("Mode", Some("Automatic follows your system."), mode_w, |ui| segmented(ui, &[(ThemeMode::System, "Automatic"), (ThemeMode::Light, "Light"), (ThemeMode::Dark, "Dark")], &mut self.settings.theme));
                            g.free(|ui| {
                                ui.label(RichText::new("Theme").size(d.body).color(pal.label));
                                ui.add_space(10.0);
                                crate::widgets::theme_picker(ui, &mut self.settings.color_theme);
                            });
                        });
                        ui.add_space(6.0);
                        hint(ui, "The whole window changes as you choose. Fifteen themes, each in light and dark; WinUI 3 follows the accent, the material and the light or dark setting of Windows 11.");
                        if let Some(tex) = picture(&mut ob.textures, ctx, "themes", THEMES_PNG) {
                            ui.add_space(8.0);
                            show_picture(ui, tex, 560.0);
                        }
                    }
                    2 => {
                        card(ui, |g| {
                            g.free(|ui| {
                                ui.label(RichText::new("Where downloads are saved").color(pal.label));
                                ui.add_space(6.0);
                                let mut f = self.settings.folder.display().to_string();
                                if super::dialogs::path_field(ui, &mut f) {
                                    self.settings.folder = PathBuf::from(f);
                                }
                            });
                            g.row("Connections per download", Some("Each file is split into this many parts. 8 is a good start."), 130.0, |ui| stepper(ui, &mut self.settings.connections, 1..=32));
                            g.row("When the window is closed", Some("Keep running: downloads go on, with an icon in the system tray."), close_w, |ui| {
                                segmented(ui, &[(BackgroundMode::Ask, "Ask"), (BackgroundMode::Keep, "Keep running"), (BackgroundMode::Quit, "Quit")], &mut self.settings.background)
                            });
                            g.toggle("Start with the system, in the background", Some("Like a download manager should: ready when you log in."), &mut self.settings.launch_at_login);
                            g.toggle("Watch the clipboard", Some("Offers to download file links that you copy."), &mut self.settings.watch_clipboard);
                        });
                    }
                    3 => {
                        ui.add(egui::Label::new(RichText::new("The add-on for Chrome, Edge, Brave, Opera and Firefox catches the downloads you start in the browser, passes your login along, and finds the videos and sounds on a page.").color(pal.secondary)).wrap());
                        ui.add_space(8.0);
                        if let Some(tex) = picture(&mut ob.textures, ctx, "browser", BROWSER_PNG) {
                            show_picture(ui, tex, 640.0);
                        }
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if secondary_button(ui, "How to install the add-on").clicked() {
                                open_browser = true;
                            }
                            ui.label(RichText::new("You can do this later: Help > Browser integration.").size(d.caption).color(pal.tertiary));
                        });
                    }
                    4 => {
                        card(ui, |g| {
                            g.toggle("Look for updates by itself", Some("Once a day. Only the list of versions is fetched; nothing about you is sent."), &mut self.settings.update_check);
                            g.toggle("Download them in the background", Some("It then only waits for you to restart into it."), &mut self.settings.update_download);
                            g.toggle("Let AI assistants add, pause and remove downloads", Some("They cannot delete files or choose folders, and you set what they may do (Help > AI assistants)."), &mut self.settings.mcp_enabled);
                        });
                        ui.add_space(8.0);
                        if let Some(tex) = picture(&mut ob.textures, ctx, "ai", AI_PNG) {
                            show_picture(ui, tex, 300.0);
                            ui.add_space(6.0);
                        }
                        hint(ui, "No ads, no account, no usage data. Help > Check for updates also lists every version, so you can go back to an older one.");
                    }
                    _ => {
                        let tips = [
                            (Icon::AddUrl, "Drop links on the window", "Drag a text file or a saved web page with links onto it."),
                            (Icon::Copy, "Copy a link", "A download link on the clipboard is picked up and offered."),
                            (Icon::Options, "Right-click a download", "Move it to a queue, refresh an expired link, check its SHA-256."),
                            (Icon::Search, "Point at a long name", "A shortened file name shows in full when you hover it."),
                            (Icon::Minus, "Shrink a progress window", "The minus button makes it a small bar in the corner."),
                            (Icon::Queue, "Close it to the tray", "Downloads keep going; the tray icon lists them and has the controls."),
                        ];
                        let col_w = ((ui.available_width() - 12.0) / 2.0).floor();
                        for (row, pair) in tips.chunks(2).enumerate() {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 12.0;
                                for (col, (icon, head, text)) in pair.iter().enumerate() {
                                    let k = arrive(ctx, at, 0.1 + (row * 2 + col) as f32 * 0.09);
                                    ui.allocate_ui(egui::vec2(col_w, 0.0), |ui| {
                                        ui.set_opacity(k);
                                        ui.add_space((1.0 - k) * 10.0);
                                        egui::Frame::NONE.fill(pal.surface2).corner_radius(egui::CornerRadius::same(d.radius_md)).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                                            ui.set_width(col_w - 24.0);
                                            ui.set_min_height(54.0);
                                            ui.horizontal_top(|ui| {
                                                icon_here(ui, *icon, 24.0, pal.accent_ink);
                                                ui.vertical(|ui| {
                                                    ui.label(RichText::new(*head).family(theme::semibold()).color(pal.label));
                                                    ui.add(egui::Label::new(RichText::new(*text).size(d.caption + 0.5).color(pal.secondary)).wrap());
                                                });
                                            });
                                        });
                                    });
                                }
                            });
                            ui.add_space(12.0);
                        }
                        ui.add_space(10.0);
                        if let Some(tex) = picture(&mut ob.textures, ctx, "networks", NETWORKS_PNG) {
                            ui.label(RichText::new("One more: Options > Networks lets one download use Wi-Fi, Ethernet and a phone at once.").size(d.caption + 0.5).color(pal.secondary));
                            ui.add_space(6.0);
                            show_picture(ui, tex, 440.0);
                        }
                    }
                }
            });
            s.footer_split(
                |ui| {
                    // Where we are: one dot per step, the current one wider and in the accent colour.
                    for n in 0..STEPS {
                        let (w, col) = if n == step { (22.0, pal.accent) } else if n < step { (8.0, pal.accent.gamma_multiply(0.45)) } else { (8.0, pal.separator_strong) };
                        let wk = anim::smooth(ctx, egui::Id::unique(("welcome_dot", n)), w, 0.12);
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(wk, 8.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, egui::CornerRadius::same(4), col);
                    }
                    ui.add_space(8.0);
                    if step + 1 < STEPS && plain_button(ui, "Skip").clicked() {
                        finish.set(true);
                    }
                },
                |ui| {
                    if step + 1 == STEPS {
                        if primary_button(ui, "Start downloading").clicked() {
                            finish.set(true);
                        }
                    } else if primary_button(ui, "Next").clicked() {
                        go.set(Some(1));
                    }
                    if step > 0 && secondary_button(ui, "Back").clicked() {
                        go.set(Some(-1));
                    }
                },
            );
        });
        if out.close {
            finish.set(true);
        }
        if open_browser {
            self.browser_open = true;
        }
        if finish.get() {
            self.finish_onboarding(was_autostart);
            return;
        }
        if let Some(delta) = go.get() {
            ob.step = (step as isize + delta).clamp(0, STEPS as isize - 1) as usize;
            ob.at = None;
        }
        self.onboarding = Some(ob);
    }

    /// The welcome is over: keep what was chosen, and do what the choices ask of the system.
    pub(super) fn finish_onboarding(&mut self, was_autostart: bool) {
        self.onboarding = None;
        if self.settings.launch_at_login != was_autostart && !self.demo {
            let done = if self.settings.launch_at_login { crate::autostart::enable() } else { crate::autostart::disable() };
            if let Err(e) = done {
                self.settings.launch_at_login = was_autostart;
                self.toast(format!("Could not change starting with the system: {e}"));
            }
        }
        // An odd folder or count from the steps is put right like any other settings change.
        self.fix_settings();
        self.apply_settings();
        if !self.demo {
            self.settings.onboarded = true;
            self.dirty = true;
        }
    }
}

/// An icon of `size` points in the layout, drawn in `colour`.
fn icon_here(ui: &mut egui::Ui, icon: Icon, size: f32, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    crate::icons::draw_weight(ui.painter(), rect, icon, colour, true, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut App, ctx: &egui::Context, time: f64) {
        let mut raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 800.0))), ..Default::default() };
        raw.time = Some(time);
        let mut out = ctx.run_ui(raw, |ui| app.draw(ui));
        out.textures_delta.clear();
    }

    #[test]
    fn every_step_draws_in_both_modes_and_the_welcome_can_be_walked_to_the_end() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let ctx = egui::Context::default();
            let mut app = App::build(ctx.clone(), Launch { demo: true, theme: Some(mode), ..Default::default() });
            for step in 0..STEPS {
                app.onboarding = Some(Onboarding::at_step(step));
                for t in 0..3 {
                    frame(&mut app, &ctx, 1.0 + t as f64 * 0.2);
                }
                assert_eq!(app.onboarding.as_ref().map(|o| o.step), Some(step), "step {step} stays until a button is pressed");
            }
        }
        assert_eq!(Onboarding::at_step(99).step, STEPS - 1, "a step past the end is the last one");
    }

    #[test]
    fn finishing_keeps_the_choices_and_remembers_that_the_welcome_was_shown() {
        let dir = std::env::temp_dir().join(format!("sdm-welcome-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::build(egui::Context::default(), Launch { headless: true, config_dir: Some(dir.clone()), ..Default::default() });
        assert!(!app.settings.onboarded, "a fresh install has not seen it");
        app.onboarding = Some(Onboarding::at_step(2));
        app.settings.connections = 12;
        app.settings.update_check = true;
        app.finish_onboarding(app.settings.launch_at_login);
        assert!(app.onboarding.is_none());
        assert!(app.settings.onboarded && app.settings.connections == 12 && app.settings.update_check);
        let _ = std::fs::remove_dir_all(dir);
    }
}
