//! Flat iOS-style colours for light and dark mode.
// Palette helpers are wired into the UI in the next commits.
#![allow(dead_code)]

use eframe::egui::{self, Color32, Id, Stroke, Theme, ThemePreference};
use serde::{Deserialize, Serialize};

/// Which theme the person wants. `System` follows the operating system live.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// One flat set of colours. No gradients and no shadows anywhere in the app.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub bg: Color32,
    pub card: Color32,
    pub raised: Color32,
    pub separator: Color32,
    pub label: Color32,
    pub secondary: Color32,
    pub blue: Color32,
    pub green: Color32,
    pub orange: Color32,
    pub red: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Palette {
    pub const LIGHT: Palette = Palette {
        bg: rgb(0xF2F2F7),
        card: rgb(0xFFFFFF),
        raised: rgb(0xFFFFFF),
        separator: rgb(0xC6C6C8),
        label: rgb(0x000000),
        secondary: rgb(0x8A8A8E),
        blue: rgb(0x007AFF),
        green: rgb(0x34C759),
        orange: rgb(0xFF9500),
        red: rgb(0xFF3B30),
    };

    pub const DARK: Palette = Palette {
        bg: rgb(0x000000),
        card: rgb(0x1C1C1E),
        raised: rgb(0x2C2C2E),
        separator: rgb(0x38383A),
        label: rgb(0xFFFFFF),
        secondary: rgb(0x98989F),
        blue: rgb(0x0A84FF),
        green: rgb(0x30D158),
        orange: rgb(0xFF9F0A),
        red: rgb(0xFF453A),
    };

    /// Blend towards `other`: `t` 0 gives `self`, 1 gives `other`.
    pub fn lerp(&self, other: &Palette, t: f32) -> Palette {
        let m = |a: Color32, b: Color32| a.lerp_to_gamma(b, t.clamp(0.0, 1.0));
        Palette {
            bg: m(self.bg, other.bg),
            card: m(self.card, other.card),
            raised: m(self.raised, other.raised),
            separator: m(self.separator, other.separator),
            label: m(self.label, other.label),
            secondary: m(self.secondary, other.secondary),
            blue: m(self.blue, other.blue),
            green: m(self.green, other.green),
            orange: m(self.orange, other.orange),
            red: m(self.red, other.red),
        }
    }
}

fn luminance(c: Color32) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG contrast ratio between two colours (1 to 21).
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Seconds the colours take to cross-fade when the theme changes.
const FADE_SECS: f32 = 0.2;

fn palette_id() -> Id {
    Id::unique("sdm_palette")
}

/// The palette being shown right now (blended while the theme is fading).
pub fn current(ctx: &egui::Context) -> Palette {
    ctx.data(|d| d.get_temp::<Palette>(palette_id())).unwrap_or(Palette::LIGHT)
}

/// Pick light or dark from the mode (and the operating system for `System`).
fn wants_dark(ctx: &egui::Context, mode: ThemeMode) -> bool {
    match mode {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::System => ctx.system_theme() == Some(Theme::Dark),
    }
}

/// Call once per frame. Fades towards the wanted palette and writes it into egui's style
/// only while something changes, so an idle window costs nothing.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    let mode_id = Id::unique("sdm_theme_mode");
    if ctx.data(|d| d.get_temp::<ThemeMode>(mode_id)) != Some(mode) {
        ctx.data_mut(|d| d.insert_temp(mode_id, mode));
        // Lets the window frame follow the choice too.
        ctx.set_theme(match mode {
            ThemeMode::System => ThemePreference::System,
            ThemeMode::Light => ThemePreference::Light,
            ThemeMode::Dark => ThemePreference::Dark,
        });
    }
    let dark = wants_dark(ctx, mode);
    let t = ctx.animate_value_with_time(Id::unique("sdm_theme_t"), if dark { 1.0 } else { 0.0 }, FADE_SECS);
    let pal = Palette::LIGHT.lerp(&Palette::DARK, t);
    if ctx.data(|d| d.get_temp::<Palette>(palette_id())) == Some(pal) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(palette_id(), pal));
    let dark_mode = t > 0.5;
    ctx.all_styles_mut(|style| style_colors(style, &pal, dark_mode));
}

/// Sizes, radii and type scale shared by both themes. Flat: no shadows, no stripes.
pub fn setup(ctx: &egui::Context) {
    use egui::{epaint::Shadow, CornerRadius, FontId, TextStyle};
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(14.0, 6.0);
        style.spacing.interact_size.y = 28.0;
        style.spacing.window_margin = egui::Margin::same(20);
        style.spacing.menu_margin = egui::Margin::same(6);
        style.text_styles.insert(TextStyle::Body, FontId::proportional(13.0));
        style.text_styles.insert(TextStyle::Button, FontId::proportional(13.0));
        style.text_styles.insert(TextStyle::Small, FontId::proportional(11.0));
        style.text_styles.insert(TextStyle::Heading, FontId::proportional(17.0));
        let v = &mut style.visuals;
        v.window_shadow = Shadow::NONE;
        v.popup_shadow = Shadow::NONE;
        v.striped = false;
        v.window_corner_radius = CornerRadius::same(14);
        v.menu_corner_radius = CornerRadius::same(10);
        for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = CornerRadius::same(8);
        }
    });
}

/// Colours of every egui widget from the palette. Sizes and shapes are set in `setup`.
fn style_colors(style: &mut egui::Style, p: &Palette, dark: bool) {
    let v = &mut style.visuals;
    v.dark_mode = dark;
    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.window_stroke = Stroke::new(1.0, p.separator);
    v.extreme_bg_color = p.bg;
    v.faint_bg_color = p.bg;
    v.code_bg_color = p.bg;
    v.hyperlink_color = p.blue;
    v.warn_fg_color = p.orange;
    v.error_fg_color = p.red;
    v.weak_text_color = Some(p.secondary);
    v.selection.bg_fill = p.blue.gamma_multiply(0.28);
    v.selection.stroke = Stroke::new(1.0, p.blue);
    let hairline = Stroke::new(1.0, p.separator);
    v.widgets.noninteractive.bg_fill = p.card;
    v.widgets.noninteractive.weak_bg_fill = p.card;
    v.widgets.noninteractive.bg_stroke = hairline;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.label);
    let steps = [(&mut v.widgets.inactive, 0.35), (&mut v.widgets.hovered, 0.55), (&mut v.widgets.active, 0.75), (&mut v.widgets.open, 0.55)];
    for (w, k) in steps {
        w.bg_fill = p.separator.gamma_multiply(k);
        w.weak_bg_fill = p.separator.gamma_multiply(k);
        w.bg_stroke = Stroke::NONE;
        w.fg_stroke = Stroke::new(1.0, p.label);
        w.expansion = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_on_card_has_4_5_contrast_in_both_themes() {
        assert!(contrast_ratio(Palette::LIGHT.label, Palette::LIGHT.card) >= 4.5);
        assert!(contrast_ratio(Palette::DARK.label, Palette::DARK.card) >= 4.5);
    }

    #[test]
    fn secondary_label_on_card_is_at_least_3() {
        // #8A8A8E on white is about 3.4:1, so the 4.5 target cannot hold for the specified secondary colour.
        assert!(contrast_ratio(Palette::LIGHT.secondary, Palette::LIGHT.card) >= 3.0);
        assert!(contrast_ratio(Palette::DARK.secondary, Palette::DARK.card) >= 3.0);
    }

    #[test]
    fn lerp_endpoints_match() {
        let (a, b) = (Palette::LIGHT, Palette::DARK);
        assert_eq!(a.lerp(&b, 0.0), a);
        assert_eq!(a.lerp(&b, 1.0), b);
    }

    #[test]
    fn palette_matches_spec_hex() {
        assert_eq!(Palette::LIGHT.bg, Color32::from_rgb(0xF2, 0xF2, 0xF7));
        assert_eq!(Palette::DARK.card, Color32::from_rgb(0x1C, 0x1C, 0x1E));
        assert_eq!(Palette::DARK.blue, Color32::from_rgb(0x0A, 0x84, 0xFF));
    }

    fn frame(ctx: &eframe::egui::Context, mode: ThemeMode) {
        let mut out = ctx.run_ui(Default::default(), |ui| apply(ui.ctx(), mode));
        out.textures_delta.clear();
    }

    #[test]
    fn forced_modes_apply_their_palette_on_the_first_frame() {
        for (mode, want) in [(ThemeMode::Dark, Palette::DARK), (ThemeMode::Light, Palette::LIGHT)] {
            let ctx = eframe::egui::Context::default();
            frame(&ctx, mode);
            assert_eq!(current(&ctx), want);
        }
    }

    #[test]
    fn switching_mode_blends_instead_of_jumping() {
        let ctx = eframe::egui::Context::default();
        frame(&ctx, ThemeMode::Light);
        frame(&ctx, ThemeMode::Dark); // egui returns the old value on the frame an animation starts
        frame(&ctx, ThemeMode::Dark);
        let mid = current(&ctx);
        assert_ne!(mid, Palette::LIGHT);
        assert_ne!(mid, Palette::DARK);
    }

    #[test]
    fn setup_makes_a_flat_style() {
        use eframe::egui::{epaint::Shadow, CornerRadius, TextStyle};
        let ctx = eframe::egui::Context::default();
        setup(&ctx);
        let st = ctx.global_style();
        assert_eq!(st.visuals.window_shadow, Shadow::NONE);
        assert_eq!(st.visuals.popup_shadow, Shadow::NONE);
        assert!(!st.visuals.striped);
        assert_eq!(st.visuals.window_corner_radius, CornerRadius::same(14));
        assert_eq!(st.visuals.menu_corner_radius, CornerRadius::same(10));
        assert_eq!(st.text_styles[&TextStyle::Body].size, 13.0);
        assert_eq!(st.text_styles[&TextStyle::Small].size, 11.0);
        assert_eq!(st.text_styles[&TextStyle::Heading].size, 17.0);
    }
}
