//! The app's icon, drawn in code (no image files needed): a blue rounded
//! square with a white download arrow. Used for the window and the installers.

/// RGBA pixels of a `size` x `size` icon.
pub fn rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut out = vec![0u8; (size * size * 4) as usize];
    const SUB: u32 = 4;
    for y in 0..size {
        for x in 0..size {
            let (mut bg, mut fg) = (0.0f32, 0.0f32);
            for sy in 0..SUB {
                for sx in 0..SUB {
                    let px = (x as f32 + (sx as f32 + 0.5) / SUB as f32) / s;
                    let py = (y as f32 + (sy as f32 + 0.5) / SUB as f32) / s;
                    if in_rounded_square(px, py) {
                        bg += 1.0;
                        if in_arrow(px, py) {
                            fg += 1.0;
                        }
                    }
                }
            }
            let n = (SUB * SUB) as f32;
            let (bg, fg) = (bg / n, fg / n);
            // Vertical gradient for the background.
            let t = y as f32 / s;
            let (r0, g0, b0) = (52.0 + 0.0 * t, 140.0 - 40.0 * t, 236.0 - 36.0 * t);
            let a = bg;
            let mix = if bg > 0.0 { fg / bg } else { 0.0 };
            let r = r0 * (1.0 - mix) + 255.0 * mix;
            let g = g0 * (1.0 - mix) + 255.0 * mix;
            let b = b0 * (1.0 - mix) + 255.0 * mix;
            let i = ((y * size + x) * 4) as usize;
            out[i] = r as u8;
            out[i + 1] = g as u8;
            out[i + 2] = b as u8;
            out[i + 3] = (a * 255.0) as u8;
        }
    }
    out
}

fn in_rounded_square(x: f32, y: f32) -> bool {
    let (m, r) = (0.06, 0.2);
    let cx = x.clamp(m + r, 1.0 - m - r);
    let cy = y.clamp(m + r, 1.0 - m - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r && x >= m && x <= 1.0 - m && y >= m && y <= 1.0 - m
}

fn in_arrow(x: f32, y: f32) -> bool {
    // Shaft
    let shaft = (x - 0.5).abs() <= 0.085 && (0.2..=0.55).contains(&y);
    // Head: triangle pointing down
    let head = y >= 0.47 && y <= 0.72 && (x - 0.5).abs() <= (0.72 - y) * 1.0;
    // Base line
    let base = (0.78..=0.85).contains(&y) && (0.24..=0.76).contains(&x);
    shaft || head || base
}

pub fn egui_icon() -> eframe::egui::IconData {
    eframe::egui::IconData { rgba: rgba(128), width: 128, height: 128 }
}

/// Save the icon as a PNG (used when making the installers).
pub fn save_png(path: &std::path::Path, size: u32) -> Result<(), String> {
    image::RgbaImage::from_raw(size, size, rgba(size)).ok_or("bad size")?.save(path).map_err(|e| e.to_string())
}
