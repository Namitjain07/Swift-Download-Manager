//! Link grabber: reads a web page (and, if asked, the pages it links to on the
//! same site) and lists the files it links to, so you can pick which to download.

use super::dialogs::{options_of, path_field};
use super::*;
use crate::bridge::DOWNLOAD_EXTS;
use crate::widgets::{button_width, card, check_row, dropdown, field, field_full, form_field, message, plain_button, primary_button, secondary_button, sheet, small_button, ButtonKind, ButtonSize};
use eframe::egui::RichText;
use std::io::Read;

#[derive(Default)]
pub struct GrabState {
    pub done: bool,
    pub pages: usize,
    pub links: Vec<String>,
    pub error: Option<String>,
}

pub struct GrabDialog {
    url: String,
    exts: String,
    all_links: bool,
    follow: bool,
    state: Option<Arc<Mutex<GrabState>>>,
    found: Vec<(String, bool)>,
    folder: String,
    queue: String,
    error: Option<String>,
}

impl GrabDialog {
    pub fn new(folder: String, queue: String) -> Self {
        GrabDialog {
            url: String::new(),
            exts: DOWNLOAD_EXTS.join(" "),
            all_links: false,
            follow: false,
            state: None,
            found: Vec::new(),
            folder,
            queue,
            error: None,
        }
    }
}

/// Every link (href / src) in a page, made absolute, without duplicates.
pub fn extract_links(base: &str, html: &str) -> Vec<String> {
    let Ok(base) = url::Url::parse(base) else { return Vec::new() };
    let lower = html.to_ascii_lowercase();
    let mut out: Vec<String> = Vec::new();
    for attr in ["href=", "src=", "data-src="] {
        let mut from = 0;
        while let Some(p) = lower[from..].find(attr) {
            let start = from + p + attr.len();
            from = start;
            // Skip "data-src=" being matched again as "src=".
            if attr == "src=" && start >= 9 && lower[..start].ends_with("data-src=") {
                continue;
            }
            let rest = &html[start..];
            let value = match rest.chars().next() {
                Some(q @ ('"' | '\'')) => rest[1..].split(q).next().unwrap_or(""),
                _ => rest.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or(""),
            };
            let value = value.trim().replace("&amp;", "&");
            if value.is_empty() || value.starts_with('#') || value.starts_with("javascript:") || value.starts_with("mailto:") {
                continue;
            }
            if let Ok(mut u) = base.join(&value) {
                if matches!(u.scheme(), "http" | "https") {
                    u.set_fragment(None);
                    let s = u.to_string();
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
        }
    }
    out
}

fn ext_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.path_segments().and_then(|mut s| s.next_back().map(|x| x.to_string())))
        .and_then(|name| name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()))
        .unwrap_or_default()
}

/// Keep links whose file ending is in `exts` (space or comma separated).
pub fn filter_links(links: &[String], exts: &str) -> Vec<String> {
    let wanted: Vec<String> = exts.split(|c: char| c.is_whitespace() || c == ',').filter(|e| !e.is_empty()).map(|e| e.trim_start_matches('.').to_ascii_lowercase()).collect();
    links.iter().filter(|l| wanted.contains(&ext_of(l))).cloned().collect()
}

fn looks_like_page(url: &str) -> bool {
    matches!(ext_of(url).as_str(), "" | "html" | "htm" | "php" | "asp" | "aspx" | "jsp")
}

fn fetch_page(url: &str, proxy: Option<&str>) -> Result<String, String> {
    let proxy = match proxy.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => Some(ureq::Proxy::new(p).map_err(|e| format!("Proxy setting is not valid: {e}"))?),
        None => None,
    };
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .proxy(proxy)
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .user_agent(concat!("Mozilla/5.0 (compatible; SwiftDownloadManager/", env!("CARGO_PKG_VERSION"), ")"))
        .build()
        .into();
    let resp = agent.get(url).call().map_err(|e| format!("Could not open the page: {e}"))?;
    let mut text = String::new();
    resp.into_body().into_reader().take(8 * 1024 * 1024).read_to_string(&mut text).map_err(|e| format!("Could not read the page: {e}"))?;
    Ok(text)
}

/// Read the page (and same-site pages it links to, if `follow`), collecting links.
fn grab(url: String, follow: bool, proxy: Option<String>, state: Arc<Mutex<GrabState>>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let html = fetch_page(&url, proxy.as_deref())?;
            let links = extract_links(&url, &html);
            {
                let mut s = state.lock().unwrap();
                s.pages = 1;
                s.links = links.clone();
            }
            ctx.request_repaint();
            if follow {
                let host = url::Url::parse(&url).ok().and_then(|u| u.host_str().map(|h| h.to_string()));
                let pages: Vec<String> = links
                    .iter()
                    .filter(|l| looks_like_page(l) && url::Url::parse(l).ok().and_then(|u| u.host_str().map(|h| h.to_string())) == host && **l != url)
                    .take(30)
                    .cloned()
                    .collect();
                for p in pages {
                    if let Ok(html) = fetch_page(&p, proxy.as_deref()) {
                        let mut s = state.lock().unwrap();
                        s.pages += 1;
                        for l in extract_links(&p, &html) {
                            if !s.links.contains(&l) {
                                s.links.push(l);
                            }
                        }
                    }
                    ctx.request_repaint();
                }
            }
            Ok(())
        })();
        let mut s = state.lock().unwrap();
        s.error = result.err();
        s.done = true;
        ctx.request_repaint();
    });
}

impl App {
    pub(super) fn open_grabber(&mut self) {
        self.grabber = Some(GrabDialog::new(self.settings.folder.display().to_string(), self.current_queue()));
    }

    pub(super) fn grabber_window(&mut self, ctx: &egui::Context) {
        let Some(mut g) = self.grabber.take() else { return };
        let mut add: Option<bool> = None;
        let mut cancel = false;
        // Collect results from the background reader.
        let mut busy = false;
        let mut pages = 0;
        if let Some(st) = &g.state {
            let s = st.lock().unwrap();
            busy = !s.done;
            pages = s.pages;
            let shown = if g.all_links { s.links.clone() } else { filter_links(&s.links, &g.exts) };
            for l in shown {
                if !g.found.iter().any(|(x, _)| *x == l) {
                    g.found.push((l, true));
                }
            }
            if s.done {
                g.error = s.error.clone();
            }
        }
        let queues = options_of(&self.settings.queues.iter().map(|q| q.name.clone()).collect::<Vec<_>>());
        let proxy = self.settings.proxy_on.then(|| self.settings.proxy.clone());
        let pal = theme::current(ctx);
        let picked = g.found.iter().filter(|(_, on)| *on).count();
        let out = sheet("grabber", "Grab links from a web page").width(680.0).show(ctx, |s| {
            s.body(|ui| {
                ui.label(RichText::new("Reads a web page and lists the files it links to.").color(pal.secondary));
                ui.add_space(8.0);
                form_field(ui, "Page address", |ui| {
                    ui.horizontal(|ui| {
                        let ok = g.url.trim().starts_with("http") && !busy;
                        let bw = button_width(ui, ButtonSize::Regular, false, "Find links");
                        let w = (ui.available_width() - bw - ui.spacing().item_spacing.x).max(80.0);
                        ui.add(field(&mut g.url).hint_text("https://example.com/downloads/").desired_width(w));
                        ui.add_enabled_ui(ok, |ui| {
                            if primary_button(ui, "Find links").clicked() {
                                let st = Arc::new(Mutex::new(GrabState::default()));
                                g.found.clear();
                                g.error = None;
                                grab(g.url.trim().to_string(), g.follow, proxy.clone(), st.clone(), ctx.clone());
                                g.state = Some(st);
                            }
                        });
                    });
                });
                check_row(ui, &mut g.follow, "Also look in pages it links to on the same site (up to 30)");
                check_row(ui, &mut g.all_links, "Show all links, not just downloadable files");
                if !g.all_links {
                    form_field(ui, "File types", |ui| {
                        if field_full(ui, &mut g.exts, "zip pdf mp4").changed() {
                            g.found.clear();
                        }
                    });
                }
                ui.add_space(8.0);
                if busy {
                    ui.horizontal(|ui| {
                        crate::widgets::arc_loader(ui, 18.0);
                        ui.label(format!("Reading… {pages} page(s) so far"));
                    });
                }
                if let Some(e) = &g.error {
                    message(ui, e, true);
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} files found, {picked} picked", g.found.len())).family(theme::semibold()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if small_button(ui, ButtonKind::Plain, "Pick none").clicked() {
                            g.found.iter_mut().for_each(|f| f.1 = false);
                        }
                        if small_button(ui, ButtonKind::Plain, "Pick all").clicked() {
                            g.found.iter_mut().for_each(|f| f.1 = true);
                        }
                    });
                });
                card(ui, |grp| {
                    grp.free(|ui| {
                        egui::ScrollArea::vertical().max_height(220.0).auto_shrink([false, true]).show(ui, |ui| {
                            for (l, on) in &mut g.found {
                                check_row(ui, on, l);
                            }
                            if g.found.is_empty() {
                                let text = if !busy && g.state.is_some() && g.error.is_none() {
                                    "No files of those types on this page. Try “Show all links” or more file types."
                                } else {
                                    "Nothing yet. Enter a page address and press “Find links”."
                                };
                                ui.label(RichText::new(text).color(pal.tertiary));
                            }
                        });
                    });
                });
                ui.add_space(10.0);
                form_field(ui, "Save to", |ui| {
                    path_field(ui, &mut g.folder);
                });
                form_field(ui, "Add to queue", |ui| {
                    let w = ui.available_width();
                    dropdown(ui, "grab_queue", w, &queues, &mut g.queue);
                });
            });
            s.footer_split(
                |ui| {
                    if plain_button(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                },
                |ui| {
                    ui.add_enabled_ui(picked > 0, |ui| {
                        if primary_button(ui, "Download picked now").clicked() {
                            add = Some(true);
                        }
                        if secondary_button(ui, "Add to queue").clicked() {
                            add = Some(false);
                        }
                    });
                },
            );
        });
        if out.close || cancel {
            return;
        }
        if let Some(start) = add {
            let urls: Vec<String> = g.found.iter().filter(|(_, on)| *on).map(|(l, _)| l.clone()).collect();
            let folder = (!g.folder.trim().is_empty()).then(|| PathBuf::from(g.folder.trim()));
            let referer = g.url.trim().to_string();
            match self.add_many(&urls, folder, start, &g.queue.clone(), &referer) {
                Ok(n) => {
                    self.toast(format!("Added {n} downloads"));
                    return;
                }
                Err(e) => g.error = Some(e),
            }
        }
        self.grabber = Some(g);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_filters_links() {
        let html = r##"<a href="files/a.zip">A</a> <A HREF='/b.PDF#top'>B</A> <img data-src="https://cdn.x.org/c.jpg">
            <a href=d.mp4>D</a> <a href="#x">skip</a> <a href="mailto:me@x.org">m</a> <a href="page2.html">next</a>
            <a href="files/a.zip">again</a> <a href="get?id=1&amp;f=2">q</a>"##;
        let links = extract_links("https://x.org/dl/index.html", html);
        assert_eq!(
            links,
            vec![
                "https://x.org/dl/files/a.zip",
                "https://x.org/b.PDF",
                "https://x.org/dl/d.mp4",
                "https://x.org/dl/page2.html",
                "https://x.org/dl/get?id=1&f=2",
                "https://cdn.x.org/c.jpg",
            ]
        );
        assert_eq!(filter_links(&links, "zip pdf, .mp4"), vec!["https://x.org/dl/files/a.zip", "https://x.org/b.PDF", "https://x.org/dl/d.mp4"]);
        assert!(looks_like_page("https://x.org/dl/page2.html") && !looks_like_page("https://x.org/a.zip"));
    }
}
