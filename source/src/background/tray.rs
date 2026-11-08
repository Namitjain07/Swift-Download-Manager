//! The tray icon and its menu. The same code runs on Windows, macOS and Linux (where the icon is a
//! StatusNotifierItem, shown by KDE, Xfce, Cinnamon and GNOME with the AppIndicator extension).
//!
//! The menu shows what is going on (how many downloads, how fast, and each running download with its percentage),
//! and has the few things that are wanted without opening the window: pause and resume everything, add the link on
//! the clipboard, open the download folder, look for updates, start with the system, quit.

use std::sync::mpsc::{Receiver, Sender};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// What the person picked in the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCmd {
    Open,
    PauseAll,
    ResumeAll,
    /// Add the download link that is on the clipboard.
    AddClipboard,
    OpenFolder,
    CheckUpdates,
    /// Start with the system: on or off.
    Autostart(bool),
    Quit,
}

/// What the tray shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrayState {
    /// The status line: "2 downloading, 4.41 MB/s".
    pub status: String,
    /// One line per running download ("41%  ubuntu.iso"), the first few.
    pub rows: Vec<String>,
    /// Whether the program starts with the system.
    pub autostart: bool,
    /// A new version, when there is one ("Update to 1.10.0").
    pub update: Option<String>,
}

/// The longest a download's line is, so the menu does not run across the screen.
const ROW_CHARS: usize = 44;
/// How many running downloads are listed.
pub const MAX_ROWS: usize = 6;

/// A file name cut to fit a menu line, with the percentage first.
pub fn row_text(percent: u32, name: &str) -> String {
    let name: String = if name.chars().count() > ROW_CHARS { name.chars().take(ROW_CHARS - 1).chain(std::iter::once('…')).collect() } else { name.to_string() };
    format!("{percent}%  {name}")
}


/// Events handed over by the handlers of `forward_events` (the window's process): the menu picks and the clicks that
/// arrived since the last `poll`.
static FORWARDED: std::sync::Mutex<Vec<Forwarded>> = std::sync::Mutex::new(Vec::new());

enum Forwarded {
    Menu(MenuId),
    Click,
}

/// Wake the window the moment the tray is used, even while it is minimized or has nothing else to do, instead of
/// looking at the tray a few times a second. (Setting a handler takes the events away from the channels, so `poll`
/// reads these instead.)
pub fn forward_events(ctx: eframe::egui::Context) {
    let menu_ctx = ctx.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        FORWARDED.lock().unwrap_or_else(|p| p.into_inner()).push(Forwarded::Menu(e.id));
        menu_ctx.request_repaint();
    }));
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if matches!(e, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. }) {
            FORWARDED.lock().unwrap_or_else(|p| p.into_inner()).push(Forwarded::Click);
            ctx.request_repaint();
        }
    }));
}

struct Items {
    open: MenuItem,
    status: MenuItem,
    rows: Vec<MenuItem>,
    pause: MenuItem,
    resume: MenuItem,
    clipboard: MenuItem,
    folder: MenuItem,
    updates: MenuItem,
    autostart: CheckMenuItem,
    quit: MenuItem,
}

pub struct Tray {
    icon: TrayIcon,
    items: Items,
    shown: TrayState,
}

fn build(state: &TrayState) -> Result<(Menu, Items), String> {
    let open = MenuItem::new("Open Swift Download Manager", true, None);
    let status = MenuItem::new(&state.status, false, None);
    let rows: Vec<MenuItem> = state.rows.iter().take(MAX_ROWS).map(|r| MenuItem::new(r, true, None)).collect();
    let pause = MenuItem::new("Pause all downloads", true, None);
    let resume = MenuItem::new("Resume downloads", true, None);
    let clipboard = MenuItem::new("Add the link on the clipboard", true, None);
    let folder = MenuItem::new("Open the download folder", true, None);
    let updates = MenuItem::new(state.update.as_deref().unwrap_or("Check for updates…"), true, None);
    let autostart = CheckMenuItem::new("Start with the system", true, state.autostart, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    let sep = PredefinedMenuItem::separator;
    menu.append_items(&[&open, &status]).map_err(|e| e.to_string())?;
    if !rows.is_empty() {
        menu.append(&sep()).map_err(|e| e.to_string())?;
        for r in &rows {
            menu.append(r).map_err(|e| e.to_string())?;
        }
    }
    menu.append_items(&[&sep(), &pause, &resume, &clipboard, &folder, &sep(), &updates, &autostart, &sep(), &quit]).map_err(|e| e.to_string())?;
    Ok((menu, Items { open, status, rows, pause, resume, clipboard, folder, updates, autostart, quit }))
}

impl Tray {
    pub fn new(state: &TrayState) -> Result<Tray, String> {
        let (menu, items) = build(state)?;
        let icon = Icon::from_rgba(crate::appicon::rgba(64), 64, 64).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new().with_menu(Box::new(menu)).with_menu_on_left_click(false).with_tooltip("Swift Download Manager").with_icon(icon).build().map_err(|e| e.to_string())?;
        Ok(Tray { icon: tray, items, shown: state.clone() })
    }

    /// Show `state`. The text of lines that stay is changed in place; when lines come or go the menu is made again
    /// (a menu that is open is left alone as long as only the text changes).
    pub fn update(&mut self, state: &TrayState) {
        if *state == self.shown {
            return;
        }
        let same_shape = state.rows.len().min(MAX_ROWS) == self.items.rows.len() && state.autostart == self.shown.autostart && state.update.is_some() == self.shown.update.is_some();
        if same_shape {
            self.items.status.set_text(&state.status);
            for (item, text) in self.items.rows.iter().zip(&state.rows) {
                item.set_text(text);
            }
            if let Some(u) = &state.update {
                self.items.updates.set_text(u);
            }
        } else if let Ok((menu, items)) = build(state) {
            self.icon.set_menu(Some(Box::new(menu)));
            self.items = items;
        }
        let _ = self.icon.set_tooltip(Some(format!("Swift Download Manager: {}", state.status)));
        self.shown = state.clone();
    }

    /// Menu picks and clicks on the icon since the last call.
    pub fn poll(&self) -> Vec<TrayCmd> {
        let it = &self.items;
        let is = |id: &MenuId, other: &MenuId| id == other;
        let mut out = Vec::new();
        let forwarded: Vec<Forwarded> = std::mem::take(&mut *FORWARDED.lock().unwrap_or_else(|p| p.into_inner()));
        let clicks = forwarded.iter().filter(|f| matches!(f, Forwarded::Click)).count();
        let mut picked: Vec<MenuId> = forwarded.into_iter().filter_map(|f| if let Forwarded::Menu(id) = f { Some(id) } else { None }).collect();
        picked.extend(MenuEvent::receiver().try_iter().map(|e| e.id));
        out.extend(std::iter::repeat_n(TrayCmd::Open, clicks));
        for id in picked {
            if is(&id, it.open.id()) || it.rows.iter().any(|r| is(&id, r.id())) {
                out.push(TrayCmd::Open);
            } else if is(&id, it.pause.id()) {
                out.push(TrayCmd::PauseAll);
            } else if is(&id, it.resume.id()) {
                out.push(TrayCmd::ResumeAll);
            } else if is(&id, it.clipboard.id()) {
                out.push(TrayCmd::AddClipboard);
            } else if is(&id, it.folder.id()) {
                out.push(TrayCmd::OpenFolder);
            } else if is(&id, it.updates.id()) {
                out.push(TrayCmd::CheckUpdates);
            } else if is(&id, it.autostart.id()) {
                // The tick box has already flipped itself: its new state is what was asked for.
                out.push(TrayCmd::Autostart(it.autostart.is_checked()));
            } else if is(&id, it.quit.id()) {
                out.push(TrayCmd::Quit);
            }
        }
        for e in TrayIconEvent::receiver().try_iter() {
            if matches!(e, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. }) {
                out.push(TrayCmd::Open);
            }
        }
        out
    }
}

/// Show the tray icon and forward what is picked until the engine thread is done (its status
/// channel closes). Linux needs no event loop; Windows and macOS do, and it has to be this thread.
pub fn run_main(cmd_tx: Sender<TrayCmd>, status_rx: Receiver<TrayState>) {
    #[cfg(any(windows, target_os = "macos"))]
    desktop::run(cmd_tx, status_rx);
    #[cfg(not(any(windows, target_os = "macos")))]
    simple::run(cmd_tx, status_rx);
}

#[cfg(not(any(windows, target_os = "macos")))]
mod simple {
    use super::*;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;

    pub fn run(cmd_tx: Sender<TrayCmd>, status_rx: Receiver<TrayState>) {
        let mut tray = match Tray::new(&TrayState { status: "starting…".into(), ..Default::default() }) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("swift-download-manager: no tray icon: {e}");
                None
            }
        };
        loop {
            if let Some(t) = &tray {
                for cmd in t.poll() {
                    let _ = cmd_tx.send(cmd);
                }
            }
            match status_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(state) => {
                    if let Some(t) = &mut tray {
                        t.update(&state);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

#[cfg(any(windows, target_os = "macos"))]
mod desktop {
    use super::*;
    use std::sync::mpsc::TryRecvError;
    use std::time::{Duration, Instant};
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::window::WindowId;

    struct Handler {
        tray: Option<Tray>,
        cmd_tx: Sender<TrayCmd>,
        status_rx: Receiver<TrayState>,
    }

    impl ApplicationHandler for Handler {
        fn resumed(&mut self, _: &ActiveEventLoop) {
            if self.tray.is_none() {
                match Tray::new(&TrayState { status: "starting…".into(), ..Default::default() }) {
                    Ok(t) => self.tray = Some(t),
                    Err(e) => eprintln!("swift-download-manager: no tray icon: {e}"),
                }
            }
        }

        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            if let Some(t) = &self.tray {
                for cmd in t.poll() {
                    let _ = self.cmd_tx.send(cmd);
                }
            }
            loop {
                match self.status_rx.try_recv() {
                    Ok(state) => {
                        if let Some(t) = &mut self.tray {
                            t.update(&state);
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        event_loop.exit();
                        return;
                    }
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100)));
        }
    }

    pub fn run(cmd_tx: Sender<TrayCmd>, status_rx: Receiver<TrayState>) {
        #[cfg(target_os = "macos")]
        let event_loop = {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            // No Dock icon and no menu bar of its own: only the tray icon.
            EventLoop::builder().with_activation_policy(ActivationPolicy::Accessory).build()
        };
        #[cfg(not(target_os = "macos"))]
        let event_loop = EventLoop::new();
        match event_loop {
            Ok(event_loop) => {
                let mut handler = Handler { tray: None, cmd_tx, status_rx };
                if let Err(e) = event_loop.run_app(&mut handler) {
                    eprintln!("swift-download-manager: the tray's event loop stopped: {e}");
                }
            }
            Err(e) => eprintln!("swift-download-manager: could not start the tray's event loop: {e}"),
        }
    }
}
