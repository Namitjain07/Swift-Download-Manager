//! The app's icon: the neon globe artwork in `assets/app-icon-512.png`, embedded in the program.
//! Used for the window and the installers (`--write-icon`).

use image::imageops::{resize, FilterType};
use image::RgbaImage;
use std::sync::OnceLock;

fn master() -> &'static RgbaImage {
    static MASTER: OnceLock<RgbaImage> = OnceLock::new();
    MASTER.get_or_init(|| image::load_from_memory(include_bytes!("../assets/app-icon-512.png")).expect("the embedded icon is a valid PNG").to_rgba8())
}

/// RGBA pixels of a `size` x `size` icon.
pub fn rgba(size: u32) -> Vec<u8> {
    let m = master();
    if size == m.width() { m.as_raw().clone() } else { resize(m, size, size, FilterType::Lanczos3).into_raw() }
}

pub fn egui_icon() -> eframe::egui::IconData {
    eframe::egui::IconData { rgba: rgba(128), width: 128, height: 128 }
}

/// Save the icon as a PNG (used when making the installers).
pub fn save_png(path: &std::path::Path, size: u32) -> Result<(), String> {
    image::RgbaImage::from_raw(size, size, rgba(size)).ok_or("bad size")?.save(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(data: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * size + x) * 4) as usize;
        [data[i], data[i + 1], data[i + 2], data[i + 3]]
    }

    #[test]
    fn icon_is_the_neon_globe_artwork_not_a_drawing() {
        // The middle of the artwork is the glowing green arrow.
        let d = rgba(512);
        let [r, g, b, a] = px(&d, 512, 256, 256);
        assert!(a == 255 && g > 180 && r < 170 && b < 130, "centre is not the green arrow: {r},{g},{b},{a}");
    }

    #[test]
    fn icon_has_rounded_transparent_corners_at_every_size() {
        for size in [16, 32, 48, 128, 256, 512] {
            let d = rgba(size);
            assert_eq!(d.len(), (size * size * 4) as usize);
            assert_eq!(px(&d, size, 0, 0)[3], 0, "corner must be see-through at {size}px");
            assert_eq!(px(&d, size, size - 1, size - 1)[3], 0);
            assert!(px(&d, size, size / 2, size / 2)[3] > 250, "middle must be solid at {size}px");
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
}
