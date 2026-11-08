//! The link between the app and Windows for the WinUI 3 theme.
//!
//! On Windows it does what a WinUI 3 app does with the system: it reads the accent colour and the other settings of the person
//! (`Windows.UI.ViewManagement.UISettings`, the source WinUI reads, so the seven shades of the accent are exactly the system's),
//! makes the title bar and the frame follow light or dark and asks for the rounded corners (`DwmSetWindowAttribute`), and
//! reads the desktop wallpaper (the Windows Imaging Component, in a thread of its own) for the Mica material, which
//! `theme::mica` blurs and tints (see there for why the app draws the material itself). It looks at the settings about once a
//! second, so a change of the accent, of the colour mode, of the transparency switch, of the wallpaper or of a high contrast
//! theme shows up in the open window. Everywhere else it says "no material", and the theme draws solid colours.

use crate::theme::mica::Wallpaper;
use crate::theme::winui::{Accent, Backdrop, SystemLook};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How often the settings of the system are read.
const LOOK_EVERY: Duration = Duration::from_millis(1000);
/// How often it is looked at whether the wallpaper is another one.
const WALLPAPER_EVERY: Duration = Duration::from_millis(4000);

static BACKDROP_OVERRIDE: Mutex<Option<Backdrop>> = Mutex::new(None);

/// `--backdrop mica|alt|acrylic|none` on the command line: the material for this run, whatever the settings say (for pictures).
pub fn set_backdrop_override(b: Option<Backdrop>) {
    *BACKDROP_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = b;
}

pub fn backdrop_override() -> Option<Backdrop> {
    *BACKDROP_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner())
}

static WALLPAPER_OVERRIDE: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

/// `--wallpaper picture.png` on the command line: the picture the material is made from for this run, instead of the desktop's
/// (for pictures that can be shared without showing somebody's own wallpaper).
pub fn set_wallpaper_override(path: Option<std::path::PathBuf>) {
    *WALLPAPER_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = path;
}

fn wallpaper_override() -> Option<std::path::PathBuf> {
    WALLPAPER_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// What the app wants of the window this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Want {
    /// Whether the app is in its dark theme.
    pub dark: bool,
    /// Whether the WinUI 3 theme is the one in use.
    pub winui: bool,
    /// The material that was chosen.
    pub backdrop: Backdrop,
    /// Follow the transparency switch of Windows.
    pub follow_effects: bool,
}

/// What came of it.
#[derive(Clone, Debug)]
pub struct Effective {
    /// The material to draw now (`Backdrop::None` when there is none: another theme, effects off, high contrast, no Windows).
    pub material: Backdrop,
    pub system: SystemLook,
    /// The blurred wallpaper and a number that changes when it does.
    pub wallpaper: Option<Arc<Wallpaper>>,
    pub wallpaper_version: u32,
}

/// Which wallpaper is loaded: where it is and when it was last written.
type WallpaperKey = (std::path::PathBuf, Option<std::time::SystemTime>);

pub struct Link {
    system: SystemLook,
    read_at: Option<Instant>,
    /// Whether the window frame was told to be dark or light.
    applied_dark: Option<bool>,
    wallpaper: Option<Arc<Wallpaper>>,
    wallpaper_key: Option<WallpaperKey>,
    wallpaper_version: u32,
    wallpaper_checked: Option<Instant>,
    loading: Option<Receiver<Option<Wallpaper>>>,
}

impl Default for Link {
    fn default() -> Self {
        Link { system: SystemLook::UNKNOWN, read_at: None, applied_dark: None, wallpaper: None, wallpaper_key: None, wallpaper_version: 0, wallpaper_checked: None, loading: None }
    }
}

/// The material that can really be shown: the one asked for, if the system allows it.
pub fn material_for(want: &Want, system: &SystemLook) -> Backdrop {
    let possible = want.winui && system.backdrops && !system.high_contrast && (!want.follow_effects || system.effects);
    if possible {
        want.backdrop
    } else {
        Backdrop::None
    }
}

impl Link {
    /// Call once a frame. Reads the system now and then, tells the window frame whether it is dark, keeps the wallpaper up to date
    /// while a material is wanted, and says what the theme can rely on.
    pub fn tick(&mut self, frame: &eframe::Frame, want: &Want) -> Effective {
        let now = Instant::now();
        if self.read_at.is_none_or(|t| t.elapsed() >= LOOK_EVERY) {
            self.system = sys::read_system();
            self.read_at = Some(now);
        }
        if self.applied_dark != Some(want.dark) && sys::apply_to_window(frame, want.dark) {
            self.applied_dark = Some(want.dark);
        }
        let material = material_for(want, &self.system);
        if material != Backdrop::None {
            self.keep_wallpaper_current(now);
        }
        Effective { material, system: self.system, wallpaper: self.wallpaper.clone(), wallpaper_version: self.wallpaper_version }
    }

    fn keep_wallpaper_current(&mut self, now: Instant) {
        if let Some(rx) = &self.loading {
            match rx.try_recv() {
                Ok(found) => {
                    self.loading = None;
                    self.wallpaper = found.map(Arc::new);
                    self.wallpaper_version = self.wallpaper_version.wrapping_add(1);
                }
                Err(mpsc::TryRecvError::Disconnected) => self.loading = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.loading.is_some() || self.wallpaper_checked.is_some_and(|t| now.duration_since(t) < WALLPAPER_EVERY) {
            return;
        }
        self.wallpaper_checked = Some(now);
        let Some(path) = wallpaper_override().or_else(sys::wallpaper_path) else {
            // A desktop with a colour only: one flat colour for the material.
            if self.wallpaper_key.is_some() || self.wallpaper.is_none() {
                self.wallpaper_key = None;
                self.wallpaper = Some(Arc::new(Wallpaper::flat(sys::desktop_colour())));
                self.wallpaper_version = self.wallpaper_version.wrapping_add(1);
            }
            return;
        };
        let key = (path.clone(), std::fs::metadata(&path).and_then(|m| m.modified()).ok());
        if self.wallpaper_key.as_ref() == Some(&key) {
            return;
        }
        self.wallpaper_key = Some(key);
        let (tx, rx) = mpsc::channel();
        self.loading = Some(rx);
        // Decoding a big picture takes a moment: not in the frame.
        std::thread::spawn(move || {
            let _ = tx.send(sys::load_wallpaper(&path));
        });
    }
}

#[cfg(windows)]
mod sys {
    use super::*;
    use eframe::egui::Color32;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use windows::core::PCWSTR;
    use windows::UI::ViewManagement::{UIColorType, UISettings};
    use windows::Win32::Foundation::{GENERIC_READ, HWND};
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWM_WINDOW_CORNER_PREFERENCE};
    use windows::Win32::Graphics::Gdi::{GetSysColor, COLOR_DESKTOP};
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_WICPixelFormat32bppRGBA, IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_SINGLETHREADED};
    use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETDESKWALLPAPER, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS};

    /// How wide the copy of the wallpaper is that the material is made from: Mica's blur is far too wide to need more.
    const COPY_WIDTH: u32 = 208;

    fn color(ui: &UISettings, kind: UIColorType) -> Option<Color32> {
        ui.GetColorValue(kind).ok().map(|c| Color32::from_rgb(c.R, c.G, c.B))
    }

    fn high_contrast() -> bool {
        let mut hc = HIGHCONTRASTW { cbSize: size_of::<HIGHCONTRASTW>() as u32, ..Default::default() };
        // SAFETY: `hc` is a valid HIGHCONTRASTW and `cbSize` says how big it is.
        let ok = unsafe { SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, Some(&mut hc as *mut HIGHCONTRASTW as *mut c_void), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)) };
        ok.is_ok() && (hc.dwFlags.0 & HCF_HIGHCONTRASTON.0) != 0
    }

    /// What Windows says. Anything it does not answer keeps its default.
    pub fn read_system() -> SystemLook {
        // WinRT needs the thread set up; winit has usually done it already, and then this says so and changes nothing.
        // SAFETY: plain COM initialisation of this thread.
        let _ = unsafe { RoInitialize(RO_INIT_SINGLETHREADED) };
        let mut look = SystemLook { backdrops: true, ..SystemLook::UNKNOWN };
        look.high_contrast = high_contrast();
        if let Ok(ui) = UISettings::new() {
            if let (Some(base), Some(d1), Some(d2), Some(d3), Some(l1), Some(l2), Some(l3)) = (
                color(&ui, UIColorType::Accent),
                color(&ui, UIColorType::AccentDark1),
                color(&ui, UIColorType::AccentDark2),
                color(&ui, UIColorType::AccentDark3),
                color(&ui, UIColorType::AccentLight1),
                color(&ui, UIColorType::AccentLight2),
                color(&ui, UIColorType::AccentLight3),
            ) {
                look.accent = Some(Accent { base, dark1: d1, dark2: d2, dark3: d3, light1: l1, light2: l2, light3: l3 });
            }
            // The colour Windows puts behind apps: black in the dark mode and white in the light one.
            look.dark = color(&ui, UIColorType::Background).map(|c| (c.r() as u32 + c.g() as u32 + c.b() as u32) < 384);
            look.effects = ui.AdvancedEffectsEnabled().unwrap_or(true);
            look.text_scale = ui.TextScaleFactor().map(|f| f as f32).unwrap_or(1.0).clamp(1.0, 2.25);
            look.animations = ui.AnimationsEnabled().unwrap_or(true);
        }
        look
    }

    fn hwnd_of(frame: &eframe::Frame) -> Option<HWND> {
        match frame.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut c_void)),
            _ => None,
        }
    }

    /// Dark title bar and frame (the system's buttons, menus and the snap layouts follow it) and rounded corners. `false`: the window
    /// is not there yet.
    pub fn apply_to_window(frame: &eframe::Frame, dark: bool) -> bool {
        let Some(hwnd) = hwnd_of(frame) else { return false };
        let dark_flag: i32 = dark.into();
        let corner = DWMWCP_ROUND;
        // SAFETY: `hwnd` is the window of this process, and each value is passed with its own size.
        unsafe {
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark_flag as *const i32 as *const c_void, size_of::<i32>() as u32);
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner as *const DWM_WINDOW_CORNER_PREFERENCE as *const c_void, size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32);
        }
        true
    }

    /// The picture on the desktop: the one Windows says, or the copy it keeps of a slide show's current picture.
    pub fn wallpaper_path() -> Option<PathBuf> {
        let mut buf = [0u16; 520];
        // SAFETY: `buf` has room for the 260 characters Windows may write, and its size is passed in characters.
        let ok = unsafe { SystemParametersInfoW(SPI_GETDESKWALLPAPER, buf.len() as u32 / 2, Some(buf.as_mut_ptr() as *mut c_void), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)) };
        let len = buf.iter().position(|c| *c == 0).unwrap_or(0);
        if ok.is_ok() && len > 0 {
            let path = PathBuf::from(String::from_utf16_lossy(&buf[..len]));
            if path.is_file() {
                return Some(path);
            }
        }
        let transcoded = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Themes\TranscodedWallpaper"))?;
        transcoded.is_file().then_some(transcoded)
    }

    /// The colour of a desktop that has no picture.
    pub fn desktop_colour() -> Color32 {
        // SAFETY: a plain lookup of a system colour.
        let c = unsafe { GetSysColor(COLOR_DESKTOP) };
        Color32::from_rgb((c & 0xFF) as u8, ((c >> 8) & 0xFF) as u8, ((c >> 16) & 0xFF) as u8)
    }

    /// Reads a picture of any kind Windows can (JPEG, PNG, BMP, ...) and makes it small, with the Windows Imaging Component.
    pub fn load_wallpaper(path: &Path) -> Option<Wallpaper> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: standard WIC calls on interfaces this function creates and owns; every buffer is as big as it is said to be.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
            let decoder = factory.CreateDecoderFromFilename(PCWSTR(wide.as_ptr()), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand).ok()?;
            let frame = decoder.GetFrame(0).ok()?;
            let (mut w, mut h) = (0u32, 0u32);
            frame.GetSize(&mut w, &mut h).ok()?;
            if w < 2 || h < 2 {
                return None;
            }
            let (tw, th) = (COPY_WIDTH, ((h as f32 / w as f32) * COPY_WIDTH as f32).round().max(2.0) as u32);
            let scaler = factory.CreateBitmapScaler().ok()?;
            scaler.Initialize(&frame, tw, th, WICBitmapInterpolationModeFant).ok()?;
            let converter = factory.CreateFormatConverter().ok()?;
            converter.Initialize(&scaler, &GUID_WICPixelFormat32bppRGBA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom).ok()?;
            let mut pixels = vec![0u8; (tw * th * 4) as usize];
            converter.CopyPixels(std::ptr::null(), tw * 4, &mut pixels).ok()?;
            Wallpaper::from_rgba(tw as usize, th as usize, &pixels)
        }
    }
}

#[cfg(not(windows))]
mod sys {
    use super::*;
    use eframe::egui::Color32;
    use std::path::{Path, PathBuf};

    pub fn read_system() -> SystemLook {
        SystemLook::UNKNOWN
    }

    pub fn apply_to_window(_frame: &eframe::Frame, _dark: bool) -> bool {
        true
    }

    pub fn wallpaper_path() -> Option<PathBuf> {
        None
    }

    pub fn desktop_colour() -> Color32 {
        Color32::GRAY
    }

    pub fn load_wallpaper(_path: &Path) -> Option<Wallpaper> {
        None
    }
}

/// The accent to use: the person's own choice (`#RRGGBB` as a number), else the system's, else the blue of Windows.
pub fn accent_for(own: Option<u32>, system: &SystemLook) -> Accent {
    match own {
        Some(rgb) => Accent::from_base(eframe::egui::Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)),
        None => system.accent.unwrap_or(Accent::WINDOWS_BLUE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn want(backdrop: Backdrop) -> Want {
        Want { dark: false, winui: true, backdrop, follow_effects: true }
    }

    fn system() -> SystemLook {
        SystemLook { backdrops: true, ..SystemLook::UNKNOWN }
    }

    #[test]
    fn a_material_shows_only_when_everything_allows_it() {
        let w = want(Backdrop::Mica);
        assert_eq!(material_for(&w, &system()), Backdrop::Mica);
        assert_eq!(material_for(&want(Backdrop::Acrylic), &system()), Backdrop::Acrylic);
        // Each thing that can say no does.
        assert_eq!(material_for(&Want { winui: false, ..w }, &system()), Backdrop::None, "another theme is in use");
        assert_eq!(material_for(&w, &SystemLook { backdrops: false, ..system() }), Backdrop::None, "this system cannot show one");
        assert_eq!(material_for(&w, &SystemLook { high_contrast: true, ..system() }), Backdrop::None, "a high contrast theme is on");
        // The transparency switch of Windows is followed, unless the person says not to.
        let off = SystemLook { effects: false, ..system() };
        assert_eq!(material_for(&w, &off), Backdrop::None);
        assert_eq!(material_for(&Want { follow_effects: false, ..w }, &off), Backdrop::Mica);
        assert_eq!(material_for(&want(Backdrop::None), &system()), Backdrop::None);
    }

    #[test]
    fn the_accent_is_the_persons_own_or_the_systems_or_the_blue_of_windows() {
        let green = Accent::from_base(eframe::egui::Color32::from_rgb(0x10, 0x7C, 0x10));
        let with_accent = SystemLook { accent: Some(green), ..system() };
        assert_eq!(accent_for(None, &with_accent), green);
        assert_eq!(accent_for(None, &system()), Accent::WINDOWS_BLUE);
        let own = accent_for(Some(0xE81123), &with_accent);
        assert_eq!(own.base, eframe::egui::Color32::from_rgb(0xE8, 0x11, 0x23));
        assert_ne!(own, green, "the choice of the person wins over the system");
    }

    #[test]
    fn the_command_line_can_choose_the_material() {
        set_backdrop_override(Some(Backdrop::Acrylic));
        assert_eq!(backdrop_override(), Some(Backdrop::Acrylic));
        set_backdrop_override(None);
        assert_eq!(backdrop_override(), None);
    }

    #[cfg(windows)]
    #[test]
    fn windows_answers_with_sensible_settings() {
        let look = sys::read_system();
        assert!(look.text_scale >= 1.0 && look.text_scale <= 2.25, "{look:?}");
        assert!(look.backdrops);
        // On any Windows 10 or 11 desktop session the accent is available, and its shades run from light to dark.
        if let Some(a) = look.accent {
            let lum = |c: eframe::egui::Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
            assert!(lum(a.light3) >= lum(a.light2) && lum(a.light1) >= lum(a.base) && lum(a.base) >= lum(a.dark1) && lum(a.dark1) >= lum(a.dark3), "{a:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_wallpaper_picture_is_read_and_made_small() {
        // The desktop of a build machine may have no picture; then there is nothing to read, which is fine.
        let Some(path) = wallpaper_override().or_else(sys::wallpaper_path) else { return };
        if let Some(w) = sys::load_wallpaper(&path) {
            assert!(w.width <= 208 && w.width >= 2 && w.height >= 2, "{}x{}", w.width, w.height);
            assert_eq!(w.rgb.len(), w.width * w.height);
            assert!(w.rgb.iter().all(|p| p.iter().all(|c| (0.0..=1.0).contains(c))));
        }
    }
}
