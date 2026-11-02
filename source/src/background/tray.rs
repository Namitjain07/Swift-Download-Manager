//! The tray icon and its menu. The same code runs on Windows, macOS and Linux (where the icon is a
//! StatusNotifierItem, shown by KDE, Xfce, Cinnamon and GNOME with the AppIndicator extension).

use std::sync::mpsc::{Receiver, Sender};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// What the person picked in the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCmd {
    Open,
    PauseAll,
    ResumeAll,
    Quit,
}

pub struct Tray {
    icon: TrayIcon,
    status: MenuItem,
    open: MenuId,
    pause: MenuId,
    resume: MenuId,
    quit: MenuId,
}

impl Tray {
    pub fn new() -> Result<Tray, String> {
        let open = MenuItem::new("Open Swift Download Manager", true, None);
        let status = MenuItem::new("Swift Download Manager: starting…", false, None);
        let pause = MenuItem::new("Pause all downloads", true, None);
        let resume = MenuItem::new("Resume downloads", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let menu = Menu::new();
        menu.append_items(&[&open, &status, &PredefinedMenuItem::separator(), &pause, &resume, &PredefinedMenuItem::separator(), &quit]).map_err(|e| e.to_string())?;
        let icon = Icon::from_rgba(crate::appicon::rgba(64), 64, 64).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new().with_menu(Box::new(menu)).with_menu_on_left_click(false).with_tooltip("Swift Download Manager").with_icon(icon).build().map_err(|e| e.to_string())?;
        Ok(Tray { icon: tray, open: open.id().clone(), pause: pause.id().clone(), resume: resume.id().clone(), quit: quit.id().clone(), status })
    }

    pub fn set_status(&self, text: &str) {
        self.status.set_text(text);
        let _ = self.icon.set_tooltip(Some(text));
    }

    /// Menu picks and clicks on the icon since the last call.
    pub fn poll(&self) -> Vec<TrayCmd> {
        let mut out = Vec::new();
        for e in MenuEvent::receiver().try_iter() {
            if e.id == self.open {
                out.push(TrayCmd::Open);
            } else if e.id == self.pause {
                out.push(TrayCmd::PauseAll);
            } else if e.id == self.resume {
                out.push(TrayCmd::ResumeAll);
            } else if e.id == self.quit {
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
pub fn run_main(cmd_tx: Sender<TrayCmd>, status_rx: Receiver<String>) {
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

    pub fn run(cmd_tx: Sender<TrayCmd>, status_rx: Receiver<String>) {
        let tray = match Tray::new() {
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
                Ok(text) => {
                    if let Some(t) = &tray {
                        t.set_status(&text);
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
        status_rx: Receiver<String>,
    }

    impl ApplicationHandler for Handler {
        fn resumed(&mut self, _: &ActiveEventLoop) {
            if self.tray.is_none() {
                match Tray::new() {
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
                    Ok(text) => {
                        if let Some(t) = &self.tray {
                            t.set_status(&text);
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

    pub fn run(cmd_tx: Sender<TrayCmd>, status_rx: Receiver<String>) {
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
