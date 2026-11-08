//! The material behind the window of the WinUI 3 theme: Mica, Mica Alt and Acrylic, drawn by the app.
//!
//! Mica is the desktop wallpaper, blurred very strongly and tinted with the colour of the theme, shown behind the window, so
//! that a window sitting on a green wallpaper is faintly green and one moved across to a blue part of it turns faintly blue.
//! WinUI asks Windows to draw it. This app draws with OpenGL, whose surface Windows composes over a plain white sheet instead of
//! over the material, so the app makes the same picture itself: it takes a small copy of the wallpaper (`winui_link` reads it with
//! the Windows Imaging Component), blurs it here, mixes in the tint of the material and paints the part of it that lies behind
//! the window as the bottom layer of the window. The window stays opaque, which also means it works the same on every graphics
//! card, and the accent, the light and dark mode and the transparency switch of Windows are still the system's own.
//!
//! The tint colours are those of `MicaController` (Base: `#F3F3F3` in light and `#202020` in dark; Alt: `#DADADA` and `#0A0A0A`).

use super::winui::Backdrop;
use eframe::egui::{Color32, ColorImage, Pos2, Rect, Vec2};

/// A small copy of the wallpaper, already blurred.
#[derive(Clone, Debug, PartialEq)]
pub struct Wallpaper {
    pub width: usize,
    pub height: usize,
    /// Red, green and blue of every pixel, 0 to 1, row by row.
    pub rgb: Vec<[f32; 3]>,
}

/// Blurs by `radius` pixels with three passes of a box filter in each direction (which comes close to a Gaussian).
fn blur(rgb: &mut [[f32; 3]], width: usize, height: usize, radius: usize) {
    if radius == 0 || width == 0 || height == 0 {
        return;
    }
    let mut tmp = vec![[0.0f32; 3]; rgb.len()];
    for _ in 0..3 {
        // Horizontal, then vertical, with the edge pixels repeated.
        for y in 0..height {
            box_line(&rgb[y * width..(y + 1) * width], &mut tmp[y * width..(y + 1) * width], radius);
        }
        let mut column = vec![[0.0f32; 3]; height];
        let mut out = vec![[0.0f32; 3]; height];
        for x in 0..width {
            for y in 0..height {
                column[y] = tmp[y * width + x];
            }
            box_line(&column, &mut out, radius);
            for y in 0..height {
                rgb[y * width + x] = out[y];
            }
        }
    }
}

/// One line blurred with a box of `2 * radius + 1` pixels.
fn box_line(src: &[[f32; 3]], dst: &mut [[f32; 3]], radius: usize) {
    let n = src.len();
    let at = |i: isize| src[i.clamp(0, n as isize - 1) as usize];
    let size = (2 * radius + 1) as f32;
    let r = radius as isize;
    let mut sum = [0.0f32; 3];
    for i in -r..=r {
        let p = at(i);
        (0..3).for_each(|c| sum[c] += p[c]);
    }
    for i in 0..n as isize {
        dst[i as usize] = [sum[0] / size, sum[1] / size, sum[2] / size];
        let (leaving, entering) = (at(i - r), at(i + r + 1));
        (0..3).for_each(|c| sum[c] += entering[c] - leaving[c]);
    }
}

impl Wallpaper {
    /// From pixels as the imaging library hands them out (`RGBA`, 8 bits each), blurred for the material.
    pub fn from_rgba(width: usize, height: usize, rgba: &[u8]) -> Option<Wallpaper> {
        if width < 2 || height < 2 || rgba.len() < width * height * 4 {
            return None;
        }
        let (pixels, _) = rgba.as_chunks::<4>();
        let mut rgb: Vec<[f32; 3]> = pixels.iter().take(width * height).map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0]).collect();
        // The copy is small (about 200 pixels wide), so this blurs over a tenth of the screen: Mica's blur is that wide.
        blur(&mut rgb, width, height, (width / 22).max(2));
        Some(Wallpaper { width, height, rgb })
    }

    /// One colour for a desktop that has no picture, only a colour (or whose picture could not be read).
    pub fn flat(colour: Color32) -> Wallpaper {
        let [r, g, b, _] = colour.to_srgba_unmultiplied();
        Wallpaper { width: 2, height: 2, rgb: vec![[r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]; 4] }
    }

    /// The picture with the tint of `material` laid over it, as an image to paint (all of it opaque).
    pub fn tinted(&self, material: Backdrop, dark: bool) -> ColorImage {
        let (tint, cover) = tint_of(material, dark);
        let [tr, tg, tb, _] = tint.to_srgba_unmultiplied();
        let tint = [tr as f32 / 255.0, tg as f32 / 255.0, tb as f32 / 255.0];
        let pixels = self
            .rgb
            .iter()
            .map(|p| {
                let mix = |c: usize| ((p[c] * (1.0 - cover) + tint[c] * cover) * 255.0).round().clamp(0.0, 255.0) as u8;
                Color32::from_rgb(mix(0), mix(1), mix(2))
            })
            .collect();
        ColorImage { size: [self.width, self.height], source_size: Vec2::new(self.width as f32, self.height as f32), pixels }
    }

    /// The part of the picture a screen of `monitor` points shows: the desktop fills the screen with the picture, whatever
    /// the shapes of the two, and shows the middle of it when it has to cut something off.
    pub fn fill_uv(&self, monitor: Vec2) -> Rect {
        let (picture, screen) = (self.width as f32 / self.height as f32, monitor.x / monitor.y.max(1.0));
        if !screen.is_finite() || screen <= 0.0 {
            return Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        }
        if picture > screen {
            let shown = screen / picture;
            Rect::from_min_max(Pos2::new((1.0 - shown) / 2.0, 0.0), Pos2::new((1.0 + shown) / 2.0, 1.0))
        } else {
            let shown = picture / screen;
            Rect::from_min_max(Pos2::new(0.0, (1.0 - shown) / 2.0), Pos2::new(1.0, (1.0 + shown) / 2.0))
        }
    }
}

/// How far a material covers the wallpaper with its own colour: the colour, and the share of it, 0 to 1.
pub fn tint_of(material: Backdrop, dark: bool) -> (Color32, f32) {
    let rgb = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
    match (material, dark) {
        (Backdrop::MicaAlt, false) => (rgb(0xDADADA), 0.78),
        (Backdrop::MicaAlt, true) => (rgb(0x0A0A0A), 0.87),
        (Backdrop::Acrylic, false) => (rgb(0xF3F3F3), 0.58),
        (Backdrop::Acrylic, true) => (rgb(0x202020), 0.68),
        (_, false) => (rgb(0xF3F3F3), 0.76),
        (_, true) => (rgb(0x202020), 0.84),
    }
}

/// The part of the picture (in 0 to 1 of the picture) that lies behind a window: `fill` is the part the screen shows, `window` is
/// where the window is on the screen, `monitor` how big the screen is (both in the same unit).
pub fn behind_window(fill: Rect, window: Rect, monitor: Vec2) -> Rect {
    let at = |p: Pos2| Pos2::new(fill.min.x + (p.x / monitor.x.max(1.0)).clamp(0.0, 1.0) * fill.width(), fill.min.y + (p.y / monitor.y.max(1.0)).clamp(0.0, 1.0) * fill.height());
    Rect::from_min_max(at(window.min), at(window.max))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_picture(w: usize, h: usize, rgb: [u8; 3]) -> Vec<u8> {
        (0..w * h).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect()
    }

    #[test]
    fn a_blur_spreads_a_bright_spot_and_keeps_a_flat_picture_flat() {
        let flat = Wallpaper::from_rgba(40, 24, &solid_picture(40, 24, [200, 100, 50])).unwrap();
        for p in &flat.rgb {
            assert!((p[0] - 200.0 / 255.0).abs() < 0.01 && (p[1] - 100.0 / 255.0).abs() < 0.01, "{p:?}");
        }
        // One white pixel in a black picture: it becomes a faint glow, and nothing is lost or gained.
        let mut rgba = solid_picture(40, 24, [0, 0, 0]);
        let centre = (12 * 40 + 20) * 4;
        rgba[centre..centre + 3].copy_from_slice(&[255, 255, 255]);
        let spot = Wallpaper::from_rgba(40, 24, &rgba).unwrap();
        let peak = spot.rgb.iter().map(|p| p[0]).fold(0.0f32, f32::max);
        assert!(peak < 0.2 && peak > 0.0, "the spot is spread out: {peak}");
        let total: f32 = spot.rgb.iter().map(|p| p[0]).sum();
        assert!((total - 1.0).abs() < 0.1, "the light is only moved around: {total}");
    }

    #[test]
    fn bad_pictures_are_refused_instead_of_panicking() {
        assert!(Wallpaper::from_rgba(0, 0, &[]).is_none());
        assert!(Wallpaper::from_rgba(10, 10, &[0; 8]).is_none());
        assert!(Wallpaper::from_rgba(1, 50, &[0; 200]).is_none());
    }

    #[test]
    fn the_tint_makes_light_materials_light_and_dark_ones_dark_and_keeps_a_trace_of_the_wallpaper() {
        let red = Wallpaper::from_rgba(32, 18, &solid_picture(32, 18, [255, 0, 0])).unwrap();
        let lum = |c: Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
        let (light, dark) = (red.tinted(Backdrop::Mica, false).pixels[100], red.tinted(Backdrop::Mica, true).pixels[100]);
        assert!(lum(light) > 600 && lum(dark) < 160, "{light:?} / {dark:?}");
        // The wallpaper's red is still there, as a tinge.
        assert!(light.r() > light.b() && dark.r() > dark.b(), "{light:?} / {dark:?}");
        // Mica Alt is darker than Mica in both modes; acrylic lets more of the wallpaper through.
        assert!(lum(red.tinted(Backdrop::MicaAlt, false).pixels[100]) < lum(light));
        assert!(lum(red.tinted(Backdrop::MicaAlt, true).pixels[100]) < lum(dark));
        let (acrylic, mica) = (red.tinted(Backdrop::Acrylic, false).pixels[100], light);
        assert!((acrylic.r() as i32 - acrylic.b() as i32) > (mica.r() as i32 - mica.b() as i32), "more colour comes through acrylic");
        assert!(red.tinted(Backdrop::Mica, false).pixels.iter().all(|c| c.a() == 255), "opaque");
    }

    #[test]
    fn a_flat_desktop_colour_gives_a_flat_material() {
        let w = Wallpaper::flat(Color32::from_rgb(0, 120, 212));
        let image = w.tinted(Backdrop::Mica, true);
        assert!(image.pixels.windows(2).all(|p| p[0] == p[1]));
        assert_eq!(image.size, [2, 2]);
    }

    #[test]
    fn the_desktop_fills_the_screen_with_the_picture_and_shows_the_middle() {
        let wide = Wallpaper { width: 32, height: 9, rgb: vec![[0.0; 3]; 32 * 9] };
        let screen_16_9 = wide.fill_uv(Vec2::new(1920.0, 1080.0));
        assert!((screen_16_9.width() - 0.5).abs() < 1e-5, "a 32:9 picture on a 16:9 screen shows half of it, cut at the sides: {screen_16_9:?}");
        assert!((screen_16_9.center().x - 0.5).abs() < 1e-5 && screen_16_9.height() == 1.0);
        let tall_screen = Wallpaper { width: 16, height: 9, rgb: vec![[0.0; 3]; 144] }.fill_uv(Vec2::new(1080.0, 1920.0));
        assert!(tall_screen.width() < 0.4 && tall_screen.height() == 1.0, "{tall_screen:?}");
        let same = Wallpaper { width: 16, height: 9, rgb: vec![[0.0; 3]; 144] }.fill_uv(Vec2::new(1920.0, 1080.0));
        assert_eq!(same, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)));
        // A screen of no size gives the whole picture.
        assert_eq!(wide.fill_uv(Vec2::ZERO), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)));
    }

    #[test]
    fn the_part_behind_a_window_moves_with_the_window() {
        let fill = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let monitor = Vec2::new(1000.0, 500.0);
        let left = behind_window(fill, Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(500.0, 250.0)), monitor);
        let right = behind_window(fill, Rect::from_min_size(Pos2::new(500.0, 250.0), Vec2::new(500.0, 250.0)), monitor);
        assert_eq!(left, Rect::from_min_max(Pos2::ZERO, Pos2::new(0.5, 0.5)));
        assert_eq!(right, Rect::from_min_max(Pos2::new(0.5, 0.5), Pos2::new(1.0, 1.0)));
        // A window half off the screen is clamped, never outside the picture.
        let off = behind_window(fill, Rect::from_min_size(Pos2::new(800.0, -100.0), Vec2::new(500.0, 250.0)), monitor);
        assert!(off.min.x >= 0.0 && off.min.y >= 0.0 && off.max.x <= 1.0 && off.max.y <= 1.0, "{off:?}");
        // With a picture that is cut at the sides, the window sees only the part that is shown.
        let cut = Rect::from_min_max(Pos2::new(0.25, 0.0), Pos2::new(0.75, 1.0));
        let whole = behind_window(cut, Rect::from_min_size(Pos2::ZERO, monitor), monitor);
        assert_eq!(whole, cut);
    }
}
