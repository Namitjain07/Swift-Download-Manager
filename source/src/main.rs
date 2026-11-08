// No console window behind the app on Windows (release builds).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod appicon;
mod autostart;
mod background;
mod bridge;
mod engine;
mod icons;
mod mcp;
mod netroute;
mod serve;
mod theme;
mod winui_link;
mod web;
mod widgets;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    // Keep downloading with no window, with an icon in the tray (started by "keep running in the background").
    if args.iter().skip(1).any(|a| a == "--background") {
        std::process::exit(background::run());
    }
    // A download server for a team: no window, several people sign in through the web interface.
    if args.iter().skip(1).any(|a| matches!(a.as_str(), "--server" | "--add-user" | "--remove-user" | "--list-users" | "--healthcheck")) {
        std::process::exit(serve::run(&args[1..]));
    }
    // MCP server for AI assistants: talks over stdin/stdout, no window.
    if args.iter().skip(1).any(|a| a == "--mcp") {
        std::process::exit(mcp::server::run_stdio());
    }
    let mut screenshot: Option<PathBuf> = None;
    let mut demo = false;
    let mut theme: Option<theme::ThemeMode> = None;
    let mut color_theme: Option<theme::ColorTheme> = None;
    let mut show: Option<String> = None;
    let mut size = [1180.0f32, 680.0];
    let mut start_url: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" if i + 1 < args.len() => {
                screenshot = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--demo" => demo = true,
            // Lists the networks this computer could download over (to check that a USB phone or a second card is seen).
            "--networks" => {
                print!("{}", netroute::describe(&netroute::interfaces()));
                return Ok(());
            }
            // Keep the title bar of the system instead of the app's own (for a desktop where that works better).
            "--native-title-bar" => widgets::titlebar::use_native(true),
            // Starting size of the window, for screenshots of the narrow layouts: --size 900x600
            "--size" if i + 1 < args.len() => {
                if let Some((w, h)) = args[i + 1].split_once('x') {
                    if let (Ok(w), Ok(h)) = (w.parse::<f32>(), h.parse::<f32>()) {
                        size = [w.clamp(300.0, 4000.0), h.clamp(300.0, 3000.0)];
                    }
                }
                i += 1;
            }
            // Opens one window at start-up, for screenshots: --show options|scheduler|add|batch|...
            "--show" if i + 1 < args.len() => {
                show = Some(args[i + 1].clone());
                i += 1;
            }
            "--theme" if i + 1 < args.len() => {
                theme = match args[i + 1].as_str() {
                    "light" => Some(theme::ThemeMode::Light),
                    "dark" => Some(theme::ThemeMode::Dark),
                    _ => None,
                };
                i += 1;
            }
            // Forces a colour theme for this run, for screenshots: --color-theme files|carbon|blue|green|black|purple|pink|...
            "--color-theme" if i + 1 < args.len() => {
                color_theme = theme::ColorTheme::from_id(&args[i + 1]);
                if color_theme.is_none() {
                    eprintln!("unknown colour theme {:?}; try one of: {}", args[i + 1], theme::ColorTheme::ALL.map(|t| t.id()).join(", "));
                }
                i += 1;
            }
            // The material behind the window of the WinUI 3 theme for this run: --backdrop mica|alt|acrylic|none
            "--backdrop" if i + 1 < args.len() => {
                let b = match args[i + 1].to_ascii_lowercase().as_str() {
                    "mica" => Some(theme::winui::Backdrop::Mica),
                    "alt" | "mica-alt" | "mica_alt" | "micaalt" => Some(theme::winui::Backdrop::MicaAlt),
                    "acrylic" => Some(theme::winui::Backdrop::Acrylic),
                    "none" | "solid" => Some(theme::winui::Backdrop::None),
                    other => {
                        eprintln!("unknown backdrop {other:?}; try one of: mica, alt, acrylic, none");
                        None
                    }
                };
                winui_link::set_backdrop_override(b);
                i += 1;
            }
            // The picture the WinUI 3 material is made from for this run, instead of the desktop's: --wallpaper picture.png
            "--wallpaper" if i + 1 < args.len() => {
                winui_link::set_wallpaper_override(Some(PathBuf::from(&args[i + 1])));
                i += 1;
            }
            "--write-icon" if i + 1 < args.len() => {
                // Used when making the installers: --write-icon icon.png [size]
                let size = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(256);
                if let Err(e) = appicon::save_png(std::path::Path::new(&args[i + 1]), size) {
                    eprintln!("could not write icon: {e}");
                }
                return Ok(());
            }
            "--fetch" if i + 2 < args.len() => {
                fetch(&args[i + 1], &args[i + 2], args.get(i + 3).map(|s| s.as_str()) == Some("--pause-once"));
                return Ok(());
            }
            a if a.starts_with("http://") || a.starts_with("https://") => start_url = Some(a.to_string()),
            _ => {}
        }
        i += 1;
    }
    // The picture is taken with sample rows so the whole layout is visible.
    if screenshot.is_some() {
        demo = true;
    }
    // Started with a link while the app is already open: hand it over and quit.
    if let Some(url) = &start_url {
        if !demo && bridge::send_to_running(bridge::PORT, url) {
            return Ok(());
        }
    }

    // OpenGL first (it needs about a tenth of the memory and does not wake a discrete graphics card); if the window
    // cannot be made that way (a remote desktop, a virtual machine without graphics drivers), the other renderer.
    let order = if cfg!(target_os = "macos") { [eframe::Renderer::Wgpu, eframe::Renderer::Glow] } else { [eframe::Renderer::Glow, eframe::Renderer::Wgpu] };
    let mut result = Ok(());
    for renderer in order {
        let options = eframe::NativeOptions {
            viewport: widgets::titlebar::viewport(
                eframe::egui::ViewportBuilder::default()
                    .with_title(app::APP_NAME)
                    .with_inner_size(size)
                    .with_min_inner_size([420.0, 420.0])
                    .with_app_id("swift-download-manager")
                    .with_icon(appicon::egui_icon()),
                widgets::titlebar::mode(),
            ),
            renderer,
            ..Default::default()
        };
        let (screenshot, start_url, show) = (screenshot.clone(), start_url.clone(), show.clone());
        result = eframe::run_native(
            app::APP_NAME,
            options,
            Box::new(move |cc| Ok(Box::new(app::App::new(cc, app::Launch { screenshot, demo, start_url, theme, color_theme, show, ..Default::default() })))),
        );
        match &result {
            // The window never opened with this renderer: try the next one. (An error after the app has run is not retried.)
            Err(eframe::Error::OpenGL(_)) | Err(eframe::Error::Glutin(_)) | Err(eframe::Error::NoGlutinConfigs(..)) | Err(eframe::Error::Wgpu(_)) => {
                eprintln!("swift-download-manager: {renderer:?} could not start ({}); trying another way to draw", result.as_ref().unwrap_err());
            }
            _ => break,
        }
    }
    result
}

/// Test helper: download one link without the window, printing progress.
/// With `--pause-once` it pauses half way and resumes, to test resuming.
fn fetch(url: &str, out: &str, pause_once: bool) {
    use std::sync::atomic::Ordering;
    let live = engine::Live::new(0, 0, engine::Status::Paused, Vec::new(), false);
    let limiter = std::sync::Arc::new(engine::Limiter::new());
    let ctx = eframe::egui::Context::default();
    let job = || engine::Job { url: url.to_string(), target: PathBuf::from(out), connections: 8, accept_server_name: false, retries: 5, ..Default::default() };
    engine::start(job(), live.clone(), limiter.clone(), ctx.clone());
    let mut paused = !pause_once;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let st = live.status();
        let done = live.downloaded.load(Ordering::SeqCst);
        let total = live.total.load(Ordering::SeqCst);
        println!("{:?} {done}/{total} parts={} resumable={}", st, live.segments.lock().unwrap().len(), live.resumable.load(Ordering::SeqCst));
        if !paused && total > 0 && done * 2 > total {
            paused = true;
            live.stop.store(true, Ordering::SeqCst);
            while live.status().is_active() {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            println!("paused at {}", live.downloaded.load(Ordering::SeqCst));
            engine::start(job(), live.clone(), limiter.clone(), ctx.clone());
        }
        match st {
            engine::Status::Complete => break,
            engine::Status::Error => {
                println!("error: {:?}", live.error.lock().unwrap());
                break;
            }
            _ => {}
        }
    }
}
