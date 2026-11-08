//! The WinUI 3 theme, from the side of the app: what is behind the window, which accent is in use, and what the options window
//! shows about it. The colours themselves are in `theme::winui`, the material in `theme::mica`; the link to Windows is in
//! `winui_link`.

use super::*;
use crate::theme::mica::{behind_window, Wallpaper};
use crate::theme::winui::Backdrop;
use crate::theme::ColorTheme;
use crate::winui_link::{self, Want};
use std::sync::Arc;
use std::time::Duration;

/// The picture of the material that is on screen, and what it was made from.
pub(super) struct MaterialTexture {
    texture: egui::TextureHandle,
    /// What it was made for: the wallpaper, the colour mode and the material.
    key: (u32, bool, Backdrop),
    wallpaper: Arc<Wallpaper>,
}

impl App {
    /// Once a frame, before the theme is applied: reads what Windows says, tells the window frame whether it is dark, and tells the
    /// theme which accent and material to draw with. Does nothing visible for the other themes.
    pub(super) fn winui_sync(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let theme = self.color_theme_override.unwrap_or(self.settings.color_theme);
        let dark = theme::is_dark(ctx, self.theme_override.unwrap_or(self.settings.theme));
        let want = Want { dark, winui: theme == ColorTheme::WinUi, backdrop: winui_link::backdrop_override().unwrap_or(self.settings.winui_backdrop), follow_effects: self.settings.winui_follow_effects };
        let found = self.winui_link.tick(frame, &want);
        self.winui_system = found.system;
        self.winui_material = found.material;
        self.winui_dark = dark;
        self.winui_wallpaper = found.wallpaper;
        self.winui_wallpaper_version = found.wallpaper_version;
        theme::winui::set_runtime(winui_link::accent_for(self.settings.winui_accent, &found.system), found.material);
        if want.winui {
            // So that a new accent, a switched transparency effect or a high contrast theme shows up in a window that is otherwise idle.
            ctx.request_repaint_after(Duration::from_millis(1000));
        }
    }

    /// Paints the material as the bottom layer of the window: the wallpaper, blurred and tinted, the part of it that lies behind the
    /// window, so that the window picks up the colours of what it is on and they move with it.
    pub(super) fn paint_material(&mut self, ctx: &egui::Context) {
        if self.winui_material == Backdrop::None {
            self.winui_texture = None;
            return;
        }
        // Until the wallpaper is read (a moment after the start) the material is a flat grey.
        let wallpaper = self.winui_wallpaper.clone().unwrap_or_else(|| Arc::new(Wallpaper::flat(egui::Color32::from_gray(128))));
        let key = (self.winui_wallpaper_version, self.winui_dark, self.winui_material);
        if self.winui_texture.as_ref().is_none_or(|t| t.key != key || !Arc::ptr_eq(&t.wallpaper, &wallpaper)) {
            let image = wallpaper.tinted(self.winui_material, self.winui_dark);
            self.winui_texture = Some(MaterialTexture { texture: ctx.load_texture("winui-material", image, egui::TextureOptions::LINEAR), key, wallpaper: wallpaper.clone() });
        }
        let Some(tex) = &self.winui_texture else { return };
        let (window, monitor) = ctx.input(|i| (i.viewport().inner_rect, i.viewport().monitor_size));
        let whole = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
        let uv = match (window, monitor) {
            (Some(w), Some(m)) if m.x > 0.0 && m.y > 0.0 => behind_window(wallpaper.fill_uv(m), w, m),
            _ => whole,
        };
        ctx.layer_painter(egui::LayerId::background()).image(tex.texture.id(), ctx.content_rect(), uv, egui::Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_winui_settings_have_defaults_and_are_read_from_older_files() {
        let s = Settings::default();
        assert_eq!(s.winui_backdrop, Backdrop::Mica);
        assert_eq!(s.winui_accent, None, "the accent of Windows, unless one is chosen");
        assert!(s.winui_follow_effects);
        // A settings file from before the theme existed has none of the three: the defaults apply.
        let old: Settings = serde_json::from_str(r#"{"connections": 4}"#).unwrap();
        assert_eq!((old.winui_backdrop, old.winui_accent, old.winui_follow_effects), (Backdrop::Mica, None, true));
        // And what is chosen is kept.
        let chosen = Settings { winui_backdrop: Backdrop::MicaAlt, winui_accent: Some(0x107C10), winui_follow_effects: false, color_theme: ColorTheme::WinUi, ..Settings::default() };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&chosen).unwrap()).unwrap();
        assert_eq!((back.winui_backdrop, back.winui_accent, back.winui_follow_effects, back.color_theme), (Backdrop::MicaAlt, Some(0x107C10), false, ColorTheme::WinUi));
    }
}
