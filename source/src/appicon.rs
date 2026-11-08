//! The app's icon: a coral rounded square with a white download arrow dropping into a tray, drawn here in code,
//! so it is sharp at every size (the window, the tray, the taskbar, the installer) and nothing has to be embedded
//! as a picture. The same drawing is used on the website and in the installers (`--write-icon`, `--write-ico`).

/// A point of the icon, in units of its side (0..1).
type P = (f32, f32);

/// The strokes of the glyph: the stem and the two sides of the arrow head, and the tray under them.
const STROKES: [(P, P); 6] = [
    ((0.500, 0.205), (0.500, 0.545)),
    ((0.500, 0.625), (0.335, 0.455)),
    ((0.500, 0.625), (0.665, 0.455)),
    ((0.250, 0.600), (0.250, 0.755)),
    ((0.250, 0.755), (0.750, 0.755)),
    ((0.750, 0.755), (0.750, 0.600)),
];

/// Corner radius of the square.
const CORNER: f32 = 0.225;

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn smooth(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Distance from `p` to the segment `a`-`b`.
fn segment_distance(p: P, a: P, b: P) -> f32 {
    let (ab, ap) = ((b.0 - a.0, b.1 - a.1), (p.0 - a.0, p.1 - a.1));
    let t = ((ap.0 * ab.0 + ap.1 * ab.1) / (ab.0 * ab.0 + ab.1 * ab.1)).clamp(0.0, 1.0);
    let q = (a.0 + ab.0 * t, a.1 + ab.1 * t);
    ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt()
}

/// Signed distance to the rounded square (negative inside).
fn square_distance(p: P) -> f32 {
    let (qx, qy) = ((p.0 - 0.5).abs() - (0.5 - CORNER), (p.1 - 0.5).abs() - (0.5 - CORNER));
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - CORNER
}

/// How far the glyph's strokes are from `p`, taken from their edge (negative inside a stroke).
fn glyph_distance(p: P, radius: f32) -> f32 {
    STROKES.iter().map(|(a, b)| segment_distance(p, *a, *b)).fold(f32::MAX, f32::min) - radius
}

/// The colour of the icon at one point, premultiplied: the background with its light, the glyph's soft shadow, the glyph.
fn shade(p: P, radius: f32) -> [f32; 4] {
    let inside = 1.0 - smooth(-0.0005, 0.0005, square_distance(p));
    if inside <= 0.0 {
        return [0.0; 4];
    }
    // Background: coral at the top left, deepening to crimson at the bottom right, with a soft light at the top.
    let t = ((p.0 + p.1) / 2.0).clamp(0.0, 1.0);
    let base = if t < 0.55 { mix([1.0, 0.353, 0.494], [0.890, 0.110, 0.373], t / 0.55) } else { mix([0.890, 0.110, 0.373], [0.690, 0.063, 0.290], (t - 0.55) / 0.45) };
    let glow = (1.0 - ((p.0 - 0.28).hypot(p.1 - 0.16) / 0.78).min(1.0)).powf(1.6) * 0.20;
    let mut c = mix(base, [1.0, 1.0, 1.0], glow);
    // A thin lighter rim just inside the edge.
    c = mix(c, [1.0, 1.0, 1.0], 0.10 * (1.0 - smooth(0.0, 0.016, -square_distance(p))));
    // The glyph's shadow (a little below it, soft), then the glyph.
    let shadow = 0.30 * (1.0 - smooth(-0.004, 0.034, glyph_distance((p.0, p.1 - 0.02), radius)));
    c = mix(c, [0.35, 0.0, 0.10], shadow);
    let glyph = 1.0 - smooth(-0.0006, 0.0006, glyph_distance(p, radius));
    c = mix(c, [1.0, 1.0, 1.0], glyph);
    [c[0] * inside, c[1] * inside, c[2] * inside, inside]
}

/// RGBA pixels of a `size` x `size` icon.
pub fn rgba(size: u32) -> Vec<u8> {
    let size = size.max(8);
    // Strokes are at least a pixel and a bit wide, so the glyph holds together at 16 pixels.
    let radius = 0.0425f32.max(1.05 / size as f32);
    let n: u32 = if size <= 64 { 4 } else if size <= 256 { 3 } else { 2 };
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0.0f32; 4];
            for sy in 0..n {
                for sx in 0..n {
                    let p = ((x as f32 + (sx as f32 + 0.5) / n as f32) / size as f32, (y as f32 + (sy as f32 + 0.5) / n as f32) / size as f32);
                    let s = shade(p, radius);
                    for k in 0..4 {
                        acc[k] += s[k];
                    }
                }
            }
            let count = (n * n) as f32;
            let a = acc[3] / count;
            if a <= 0.0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                for k in 0..3 {
                    out.push((acc[k] / count / a * 255.0 + 0.5).clamp(0.0, 255.0) as u8);
                }
                out.push((a * 255.0 + 0.5).clamp(0.0, 255.0) as u8);
            }
        }
    }
    out
}

pub fn egui_icon() -> eframe::egui::IconData {
    eframe::egui::IconData { rgba: rgba(128), width: 128, height: 128 }
}

/// Save the icon as a PNG (used when making the installers and the website).
pub fn save_png(path: &std::path::Path, size: u32) -> Result<(), String> {
    image::RgbaImage::from_raw(size, size, rgba(size)).ok_or("bad size")?.save(path).map_err(|e| e.to_string())
}

/// The sizes an `.ico` file carries: Windows picks the nearest for the title bar, the taskbar, the Start menu and Explorer.
const ICO_SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

/// An `.ico` file with the icon at every size (each one a PNG inside, which Windows has read since Vista).
pub fn ico_bytes() -> Result<Vec<u8>, String> {
    use image::codecs::png::PngEncoder;
    use image::{ExtendedColorType, ImageEncoder};
    let images: Vec<(u32, Vec<u8>)> = ICO_SIZES
        .iter()
        .map(|&s| {
            let mut png = Vec::new();
            PngEncoder::new(&mut png).write_image(&rgba(s), s, s, ExtendedColorType::Rgba8).map_err(|e| e.to_string())?;
            Ok((s, png))
        })
        .collect::<Result<_, String>>()?;
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (s, png) in &images {
        let dim = if *s >= 256 { 0 } else { *s as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0, 1, 0, 32, 0]);
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        out.extend_from_slice(&png);
    }
    Ok(out)
}

/// Save the `.ico` (used when making the Windows program and installer).
pub fn save_ico(path: &std::path::Path) -> Result<(), String> {
    std::fs::write(path, ico_bytes()?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(data: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * size + x) * 4) as usize;
        [data[i], data[i + 1], data[i + 2], data[i + 3]]
    }

    #[test]
    fn the_middle_is_the_white_arrow_on_a_coral_square() {
        let d = rgba(256);
        // The stem of the arrow, in the middle, is white.
        let [r, g, b, a] = px(&d, 256, 128, 90);
        assert!(a == 255 && r > 240 && g > 240 && b > 240, "the stem is not white: {r},{g},{b},{a}");
        // Beside it, the square is coral to crimson: strong red, little green.
        let [r, g, b, a] = px(&d, 256, 40, 128);
        assert!(a == 255 && r > 190 && g < 110 && b > 50 && b < 150, "the background is not coral: {r},{g},{b},{a}");
    }

    #[test]
    fn the_icon_has_rounded_transparent_corners_and_a_solid_middle_at_every_size() {
        for size in [16, 24, 32, 48, 64, 128, 256, 512] {
            let d = rgba(size);
            assert_eq!(d.len(), (size * size * 4) as usize);
            assert_eq!(px(&d, size, 0, 0)[3], 0, "corner must be see-through at {size}px");
            assert_eq!(px(&d, size, size - 1, size - 1)[3], 0);
            assert!(px(&d, size, size / 2, size / 2)[3] > 250, "middle must be solid at {size}px");
            assert!(px(&d, size, size / 2, 2)[3] > 250, "the top edge is filled at {size}px");
        }
    }

    #[test]
    fn the_glyph_is_still_there_at_sixteen_pixels() {
        // Count the nearly white pixels: the arrow and the tray must not vanish in a favicon-sized icon.
        let d = rgba(16);
        let white = d.chunks(4).filter(|p| p[0] > 225 && p[1] > 200 && p[2] > 200 && p[3] > 240).count();
        assert!(white >= 20, "only {white} white pixels at 16 px");
    }

    #[test]
    fn the_drawing_is_left_right_symmetrical() {
        let d = rgba(128);
        for y in (4..124).step_by(7) {
            for x in 4..60 {
                let (a, b) = (px(&d, 128, x, y), px(&d, 128, 127 - x, y));
                // The shading is lit from the left, so only compare where the glyph (white) is.
                if a[0] > 245 && a[1] > 245 {
                    assert!(b[0] > 235 && b[1] > 235, "({x},{y}) is white but its mirror is {b:?}");
                }
            }
        }
    }

    #[test]
    fn save_png_writes_the_requested_size() {
        let path = std::env::temp_dir().join(format!("sdm-icon-{}.png", std::process::id()));
        save_png(&path, 64).unwrap();
        let img = image::open(&path).unwrap();
        assert_eq!((img.width(), img.height()), (64, 64));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn the_ico_file_has_every_size_and_each_one_is_a_png_of_that_size() {
        let ico = ico_bytes().unwrap();
        assert_eq!(&ico[0..4], &[0, 0, 1, 0], "an icon file");
        let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
        assert_eq!(count, ICO_SIZES.len());
        for (i, size) in ICO_SIZES.iter().enumerate() {
            let e = &ico[6 + 16 * i..6 + 16 * (i + 1)];
            assert_eq!(e[0] as u32, if *size >= 256 { 0 } else { *size });
            let (len, off) = (u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize, u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize);
            let png = image::load_from_memory(&ico[off..off + len]).unwrap();
            assert_eq!((png.width(), png.height()), (*size, *size));
        }
    }
}
