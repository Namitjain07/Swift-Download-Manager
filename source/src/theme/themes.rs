//! The colour themes.
//!
//! A theme is a handful of colours (the grounds, the text and one accent) and a shape (round capsules,
//! Fluent's small corners, or Carbon's square ones). Everything else, such as the greys of the lines,
//! the tinted fills and above all the *ink* colours for text, is derived from them, and each ink is
//! moved until it holds 4.5:1 against every surface it can be drawn on. So a theme can never produce
//! text that is hard to read, whatever accent it is given. `Airbnb` is the exception: it is the hand
//! tuned default (see `Palette::LIGHT` and `Palette::DARK`).

use super::palette::{contrast_ratio, over, Palette};
use eframe::egui::Color32;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The shape language of a theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Capsule buttons, 16 px cards, 24 px dialogs: the Airbnb look.
    Pill,
    /// Small 4 px controls and 8 px cards, an accent mark on the chosen sidebar row: the Files look.
    Fluent,
    /// WinUI 3: 4 px controls, 8 px cards and dialogs, 32 px tall, Segoe UI, a material behind the window.
    WinUi,
    /// Nearly square corners and a 40 px control height: the Carbon look.
    Sharp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ColorTheme {
    #[default]
    Airbnb,
    Files,
    Carbon,
    Blue,
    Green,
    Black,
    Purple,
    Pink,
    Orange,
    Teal,
    Gold,
    Nord,
    Dracula,
    Mocha,
    /// WinUI 3 / Windows 11: the design tokens of Microsoft's own controls, the system accent and a Mica or Acrylic material
    /// (see `winui.rs`). Its colours are not derived here but come from there.
    WinUi,
}

impl ColorTheme {
    pub const ALL: [ColorTheme; 15] = [
        ColorTheme::Airbnb,
        ColorTheme::WinUi,
        ColorTheme::Files,
        ColorTheme::Carbon,
        ColorTheme::Blue,
        ColorTheme::Green,
        ColorTheme::Black,
        ColorTheme::Purple,
        ColorTheme::Pink,
        ColorTheme::Orange,
        ColorTheme::Teal,
        ColorTheme::Gold,
        ColorTheme::Nord,
        ColorTheme::Dracula,
        ColorTheme::Mocha,
    ];

    /// The name shown to people.
    pub const fn name(self) -> &'static str {
        match self {
            ColorTheme::Airbnb => "Coral",
            ColorTheme::Files => "Files",
            ColorTheme::Carbon => "Carbon",
            ColorTheme::Blue => "Blue",
            ColorTheme::Green => "Green",
            ColorTheme::Black => "Black",
            ColorTheme::Purple => "Purple",
            ColorTheme::Pink => "Pink",
            ColorTheme::Orange => "Orange",
            ColorTheme::Teal => "Teal",
            ColorTheme::Gold => "Gold",
            ColorTheme::Nord => "Nord",
            ColorTheme::Dracula => "Dracula",
            ColorTheme::Mocha => "Mocha",
            ColorTheme::WinUi => "WinUI 3",
        }
    }

    /// The name stored in the settings file and accepted by `--color-theme`.
    pub const fn id(self) -> &'static str {
        match self {
            ColorTheme::Airbnb => "coral",
            ColorTheme::Files => "files",
            ColorTheme::Carbon => "carbon",
            ColorTheme::Blue => "blue",
            ColorTheme::Green => "green",
            ColorTheme::Black => "black",
            ColorTheme::Purple => "purple",
            ColorTheme::Pink => "pink",
            ColorTheme::Orange => "orange",
            ColorTheme::Teal => "teal",
            ColorTheme::Gold => "gold",
            ColorTheme::Nord => "nord",
            ColorTheme::Dracula => "dracula",
            ColorTheme::Mocha => "mocha",
            ColorTheme::WinUi => "winui",
        }
    }

    /// Finds a theme by its id or its name, ignoring case ("coral", "Blue", "files").
    pub fn from_id(s: &str) -> Option<ColorTheme> {
        let s = s.trim();
        ColorTheme::ALL.into_iter().find(|t| {
            t.id().eq_ignore_ascii_case(s)
                || t.name().eq_ignore_ascii_case(s)
                || (*t == ColorTheme::Airbnb && s.eq_ignore_ascii_case("airbnb"))
                || (*t == ColorTheme::WinUi && ["winui3", "windows", "windows 11", "windows11", "win11", "fluent2"].iter().any(|a| s.eq_ignore_ascii_case(a)))
        })
    }

    pub const fn shape(self) -> Shape {
        match self {
            ColorTheme::Files => Shape::Fluent,
            ColorTheme::WinUi => Shape::WinUi,
            ColorTheme::Carbon => Shape::Sharp,
            _ => Shape::Pill,
        }
    }

    /// The colours of this theme in light or dark.
    pub fn palette(self, dark: bool) -> Palette {
        match (self, dark) {
            (ColorTheme::Airbnb, false) => Palette::LIGHT,
            (ColorTheme::Airbnb, true) => Palette::DARK,
            (ColorTheme::WinUi, dark) => super::winui::palette(dark, &super::winui::runtime()),
            _ => build(&ground(self, dark), dark),
        }
    }
}

impl Serialize for ColorTheme {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.id())
    }
}

impl<'de> Deserialize<'de> for ColorTheme {
    /// An unknown name (a theme from a newer version, say) falls back to the default instead of
    /// making the whole settings file unreadable.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(ColorTheme::from_id(&s).unwrap_or_default())
    }
}

/// The colours a theme starts from.
struct Ground {
    bg: u32,
    card: u32,
    raised: u32,
    chrome: u32,
    sidebar: u32,
    label: u32,
    accent: u32,
}

const fn g(bg: u32, card: u32, raised: u32, chrome: u32, sidebar: u32, label: u32, accent: u32) -> Ground {
    Ground { bg, card, raised, chrome, sidebar, label, accent }
}

fn ground(theme: ColorTheme, dark: bool) -> Ground {
    use ColorTheme::*;
    match (theme, dark) {
        // Fluent / WinUI: Mica grounds, the window's own colour for the chrome and the sidebar, the Windows 11 blue.
        (Files, false) => g(0xF3F3F3, 0xFCFCFC, 0xFFFFFF, 0xF3F3F3, 0xF3F3F3, 0x1A1A1A, 0x0067C0),
        (Files, true) => g(0x202020, 0x2B2B2B, 0x2C2C2C, 0x202020, 0x202020, 0xFFFFFF, 0x4CC2FF),
        // IBM Carbon: Gray 100 and White, with Carbon's blue.
        (Carbon, false) => g(0xF4F4F4, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0x161616, 0x0F62FE),
        (Carbon, true) => g(0x161616, 0x262626, 0x2B2B2B, 0x161616, 0x161616, 0xF4F4F4, 0x4589FF),
        (Blue, false) => g(0xF2F6FD, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0x0F1B2D, 0x2563EB),
        (Blue, true) => g(0x0A111F, 0x121C30, 0x18243B, 0x0E1729, 0x0E1729, 0xEAF0FB, 0x3B82F6),
        (Green, false) => g(0xF1F7F3, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0x0E1F16, 0x16A34A),
        (Green, true) => g(0x08120C, 0x0F1D15, 0x14261B, 0x0B1810, 0x0B1810, 0xE8F4EC, 0x22C55E),
        // Black: monochrome. OLED black with a white accent, and white with a black one.
        (Black, false) => g(0xF5F5F5, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0x0A0A0A, 0x111111),
        (Black, true) => g(0x000000, 0x0C0C0C, 0x141414, 0x000000, 0x000000, 0xFFFFFF, 0xFFFFFF),
        (Purple, false) => g(0xF6F3FD, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0x1B1230, 0x7C3AED),
        (Purple, true) => g(0x0E0A19, 0x171126, 0x1E1733, 0x120D20, 0x120D20, 0xEFEAFB, 0x8B5CF6),
        (Pink, false) => g(0xFDF2F7, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0x2A1220, 0xEC4899),
        (Pink, true) => g(0x140A10, 0x21111B, 0x291722, 0x1A0D15, 0x1A0D15, 0xFBEAF2, 0xF472B6),
        (Orange, false) => g(0xFEF5EE, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0x2B1708, 0xF97316),
        (Orange, true) => g(0x140D07, 0x211710, 0x2A1E15, 0x1A110A, 0x1A110A, 0xFBEFE4, 0xFB923C),
        (Teal, false) => g(0xEFF8F7, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0x0B2220, 0x0D9488),
        (Teal, true) => g(0x071413, 0x0E1F1D, 0x132926, 0x0A1A18, 0x0A1A18, 0xE4F5F3, 0x2DD4BF),
        (Gold, false) => g(0xFBF7EC, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0x2A2208, 0xD97706),
        (Gold, true) => g(0x120F05, 0x1E1A0C, 0x272213, 0x171307, 0x171307, 0xF8F2DE, 0xFBBF24),
        // Nord: Polar Night / Snow Storm with Frost blue.
        (Nord, false) => g(0xE5E9F0, 0xF2F4F8, 0xFFFFFF, 0xECEFF4, 0xECEFF4, 0x2E3440, 0x5E81AC),
        (Nord, true) => g(0x2E3440, 0x3B4252, 0x434C5E, 0x2A303B, 0x2A303B, 0xECEFF4, 0x88C0D0),
        // Dracula and its light sibling.
        (Dracula, false) => g(0xFBFAF4, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0x282A36, 0x6B46C1),
        (Dracula, true) => g(0x282A36, 0x343746, 0x3B3E50, 0x21222C, 0x21222C, 0xF8F8F2, 0xBD93F9),
        // Catppuccin Latte and Mocha.
        (Mocha, false) => g(0xEFF1F5, 0xFFFFFF, 0xFFFFFF, 0xE6E9EF, 0xE6E9EF, 0x4C4F69, 0x8839EF),
        (Mocha, true) => g(0x1E1E2E, 0x313244, 0x363850, 0x181825, 0x181825, 0xCDD6F4, 0xCBA6F7),
        // Airbnb never gets here (it has its own palette); this keeps the match complete.
        (Airbnb | WinUi, _) => g(0xF7F7F7, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0x222222, 0xFF385C),
    }
}

fn rgb(h: u32) -> Color32 {
    Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
}

fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// `from` moved towards `to` as far as is needed (and no further) for it to have `min` contrast
/// against every colour in `against`. Gives `to` itself if even that is not enough.
fn readable(from: Color32, to: Color32, against: &[Color32], min: f32) -> Color32 {
    for step in 0..=48 {
        let candidate = from.lerp_to_gamma(to, step as f32 / 48.0);
        if against.iter().all(|bg| contrast_ratio(candidate, *bg) >= min) {
            return candidate;
        }
    }
    to
}

/// A fill that text of colour `on` can sit on at 4.5:1: `fill` moved away from `on` until it is.
fn fill_for(on: Color32, fill: Color32) -> Color32 {
    let away = if on == Color32::WHITE { Color32::BLACK } else { Color32::WHITE };
    for step in 0..=48 {
        let candidate = fill.lerp_to_gamma(away, step as f32 / 48.0);
        if contrast_ratio(on, candidate) >= 4.5 {
            return candidate;
        }
    }
    away
}

/// One of the state hues: (the plain colour, its ink for text, its tint for backgrounds).
fn hue(base: Color32, label: Color32, surfaces: &[Color32], tint_alpha: u8) -> (Color32, Color32, Color32) {
    let tint = alpha(base, tint_alpha);
    let mut against: Vec<Color32> = surfaces.to_vec();
    against.extend(surfaces.iter().map(|s| over(tint, *s)));
    (base, readable(base, label, &against, 4.5), tint)
}

fn build(s: &Ground, dark: bool) -> Palette {
    let (bg, card, raised, chrome, sidebar, label, accent) = (rgb(s.bg), rgb(s.card), rgb(s.raised), rgb(s.chrome), rgb(s.sidebar), rgb(s.label), rgb(s.accent));
    let grounds = [bg, card, raised, chrome, sidebar];

    let surface2 = raised.lerp_to_gamma(label, if dark { 0.10 } else { 0.05 });
    let surface3 = raised.lerp_to_gamma(label, if dark { 0.17 } else { 0.10 });
    let mut for_secondary = grounds.to_vec();
    for_secondary.push(surface2);
    let secondary = readable(label.lerp_to_gamma(bg, 0.20), label, &for_secondary, 4.5);
    let tertiary = readable(label.lerp_to_gamma(bg, 0.42), label, &grounds, 4.5);
    let control_border = readable(label.lerp_to_gamma(raised, 0.62), label, &[card, raised], 3.0);

    // The accent: the plain colour for marks and progress bars, a solid fill for buttons (white or near-black text on
    // it, whichever reads better) and an ink for coloured text.
    let near_black = Color32::from_rgb(0x11, 0x11, 0x11);
    let on_accent = if contrast_ratio(Color32::WHITE, accent) >= contrast_ratio(near_black, accent) { Color32::WHITE } else { near_black };
    let accent_solid = fill_for(on_accent, accent);
    let away = if on_accent == Color32::WHITE { Color32::BLACK } else { Color32::WHITE };
    let accent_press = accent_solid.lerp_to_gamma(away, 0.14);
    let accent_tint = alpha(accent, if dark { 46 } else { 26 });
    let mut for_ink = grounds.to_vec();
    for_ink.extend([over(accent_tint, card), over(accent_tint, raised), over(accent_tint, bg)]);
    let accent_ink = readable(accent, label, &for_ink, 4.5);

    let tint_a = if dark { 46 } else { 36 };
    let (green, green_ink, green_tint) = hue(rgb(if dark { 0x30D158 } else { 0x34C759 }), label, &[card, raised, bg], tint_a);
    let (orange, orange_ink, orange_tint) = hue(rgb(if dark { 0xFF9F0A } else { 0xFF9500 }), label, &[card, raised, bg], tint_a);
    let (red, red_ink, red_tint) = hue(rgb(if dark { 0xFF6B4A } else { 0xD13B1F }), label, &[card, raised, bg], if dark { 46 } else { 26 });
    let (blue, blue_ink, blue_tint) = hue(rgb(if dark { 0x0A84FF } else { 0x007AFF }), label, &[card, raised, bg], if dark { 46 } else { 28 });

    Palette {
        bg,
        card,
        surface2,
        surface3,
        raised,
        sidebar,
        chrome,
        segment_on: if dark { raised.lerp_to_gamma(label, 0.26) } else { card },
        track: raised.lerp_to_gamma(label, if dark { 0.20 } else { 0.08 }),
        scrim: alpha(Color32::BLACK, if dark { 153 } else { 102 }),
        hover: alpha(label, if dark { 18 } else { 13 }),
        press: alpha(label, if dark { 31 } else { 26 }),
        separator: alpha(label, if dark { 26 } else { 20 }),
        separator_strong: alpha(label, if dark { 46 } else { 36 }),
        control_border,
        label,
        secondary,
        tertiary,
        disabled: label.lerp_to_gamma(bg, 0.70),
        accent,
        accent_ink,
        accent_solid,
        accent_press,
        on_accent,
        accent_tint,
        green,
        green_ink,
        green_tint,
        orange,
        orange_ink,
        orange_tint,
        red,
        red_ink,
        red_tint,
        red_solid: fill_for(Color32::WHITE, red),
        blue,
        blue_ink,
        blue_tint,
        ctl: surface2,
        ctl_hover: surface3,
        ctl_press: surface3,
        ctl_stroke: alpha(label, if dark { 46 } else { 36 }),
        ctl_stroke_bottom: alpha(label, if dark { 46 } else { 36 }),
        ctl_strong: control_border,
        accent_hover: accent_press,
        focus: label,
        shadow: Color32::BLACK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every() -> impl Iterator<Item = (String, ColorTheme, bool, Palette)> {
        ColorTheme::ALL.into_iter().flat_map(|t| [false, true].map(move |dark| (format!("{} {}", t.name(), if dark { "dark" } else { "light" }), t, dark, t.palette(dark))))
    }

    #[test]
    fn there_are_fifteen_themes_with_their_own_names_and_ids() {
        assert_eq!(ColorTheme::ALL.len(), 15);
        let names: std::collections::BTreeSet<_> = ColorTheme::ALL.iter().map(|t| t.name()).collect();
        let ids: std::collections::BTreeSet<_> = ColorTheme::ALL.iter().map(|t| t.id()).collect();
        assert_eq!((names.len(), ids.len()), (15, 15));
        assert_eq!(ColorTheme::default(), ColorTheme::Airbnb);
    }

    #[test]
    fn a_theme_is_found_by_id_or_by_name_in_any_case() {
        for t in ColorTheme::ALL {
            assert_eq!(ColorTheme::from_id(t.id()), Some(t));
            assert_eq!(ColorTheme::from_id(&t.name().to_uppercase()), Some(t));
        }
        assert_eq!(ColorTheme::from_id("airbnb"), Some(ColorTheme::Airbnb));
        assert_eq!(ColorTheme::from_id("  BLUE "), Some(ColorTheme::Blue));
        assert_eq!(ColorTheme::from_id("neon"), None);
    }

    #[test]
    fn a_saved_theme_is_read_back_and_an_unknown_one_falls_back_to_the_default() {
        for t in ColorTheme::ALL {
            let text = serde_json::to_string(&t).unwrap();
            assert_eq!(serde_json::from_str::<ColorTheme>(&text).unwrap(), t, "{text}");
        }
        assert_eq!(serde_json::from_str::<ColorTheme>("\"from-the-future\"").unwrap(), ColorTheme::Airbnb);
        // ... and one odd value does not spoil the rest of a settings file.
        #[derive(Deserialize)]
        struct S {
            theme: ColorTheme,
            n: u8,
        }
        let s: S = serde_json::from_str(r#"{"theme":"nope","n":3}"#).unwrap();
        assert_eq!((s.theme, s.n), (ColorTheme::Airbnb, 3));
    }

    #[test]
    fn text_reads_on_every_ground_in_every_theme() {
        for (name, _, _, p) in every() {
            for (ground_name, ground) in [("page", p.bg), ("card", p.card), ("chrome", p.chrome), ("sidebar", p.sidebar), ("dialog", p.raised)] {
                for (ink_name, ink) in [("label", p.label), ("secondary", p.secondary), ("tertiary", p.tertiary)] {
                    let c = contrast_ratio(ink, ground);
                    assert!(c >= 4.5, "{ink_name} on {ground_name} in {name} is only {c:.2}:1");
                }
            }
        }
    }

    #[test]
    fn every_ink_reads_on_its_own_tint_and_on_the_surfaces_in_every_theme() {
        for (name, _, _, p) in every() {
            for (hue_name, ink, tint) in [
                ("accent", p.accent_ink, p.accent_tint),
                ("green", p.green_ink, p.green_tint),
                ("orange", p.orange_ink, p.orange_tint),
                ("red", p.red_ink, p.red_tint),
                ("blue", p.blue_ink, p.blue_tint),
            ] {
                for (surface_name, surface) in [("card", p.card), ("dialog", p.raised), ("page", p.bg)] {
                    let c = contrast_ratio(ink, surface);
                    assert!(c >= 4.5, "{hue_name}-ink on {surface_name} in {name} is only {c:.2}:1");
                    let c = contrast_ratio(ink, over(tint, surface));
                    assert!(c >= 4.5, "{hue_name}-ink on its tint over {surface_name} in {name} is only {c:.2}:1");
                }
            }
            let c = contrast_ratio(p.secondary, over(p.surface2, p.card));
            assert!(c >= 4.5, "secondary on a grey chip in {name} is only {c:.2}:1");
        }
    }

    #[test]
    fn solid_buttons_carry_readable_text_in_every_theme() {
        for (name, theme, dark, p) in every() {
            for (what, fill) in [("accent", p.accent_solid), ("accent pressed", p.accent_press), ("red", p.red_solid)] {
                let on = if what == "red" { Color32::WHITE } else { p.on_accent };
                let c = contrast_ratio(on, fill);
                // The hand-made Coral dark "pressed" fill is the bright brand coral: it holds the 3:1 of
                // large bold text for the moment a button is held down (see the palette tests).
                // WinUI itself draws a pressed accent button at 80 % opacity, which is below 4.5:1 for white text.
                let need = if (theme == ColorTheme::Airbnb && dark && what == "accent pressed") || (theme == ColorTheme::WinUi && what == "accent pressed") { 3.0 } else { 4.5 };
                assert!(c >= need, "text on the {what} button in {name} is only {c:.2}:1");
            }
        }
    }

    #[test]
    fn control_borders_are_visible_in_every_theme() {
        for (name, _, _, p) in every() {
            for (ground_name, ground) in [("card", p.card), ("dialog", p.raised)] {
                let c = contrast_ratio(p.control_border, ground);
                assert!(c >= 3.0, "the border on {ground_name} in {name} is only {c:.2}:1");
            }
        }
    }

    #[test]
    fn surfaces_are_opaque_and_dark_themes_are_darker_than_light_ones() {
        for t in ColorTheme::ALL {
            let (l, d) = (t.palette(false), t.palette(true));
            for (what, c) in [("bg", l.bg), ("card", l.card), ("raised", l.raised), ("chrome", l.chrome), ("sidebar", l.sidebar), ("bg", d.bg), ("card", d.card), ("raised", d.raised)] {
                assert_eq!(c.a(), 255, "{} {what}", t.name());
            }
            assert!(super::super::palette::contrast_ratio(l.bg, Color32::BLACK) > contrast_ratio(d.bg, Color32::BLACK), "{} light is lighter than dark", t.name());
        }
    }

    #[test]
    fn the_themes_really_differ_from_each_other() {
        // Same mode, different accent: no two themes may share an accent (and Black's is white or near-black).
        let mut seen = std::collections::BTreeSet::new();
        for t in ColorTheme::ALL {
            assert!(seen.insert(t.palette(false).accent.to_array()), "{} repeats an accent", t.name());
        }
        let blue = ColorTheme::Blue.palette(true);
        let green = ColorTheme::Green.palette(true);
        assert_ne!(blue.bg, green.bg);
        assert_ne!(blue.accent_solid, green.accent_solid);
    }

    #[test]
    fn files_blends_its_chrome_into_the_window_and_uses_small_corners() {
        for dark in [false, true] {
            let p = ColorTheme::Files.palette(dark);
            assert_eq!(p.chrome, p.bg, "the header, toolbar and sidebar share the window's colour");
            assert_eq!(p.sidebar, p.bg);
        }
        assert_eq!(ColorTheme::Files.shape(), Shape::Fluent);
        assert_eq!(ColorTheme::Carbon.shape(), Shape::Sharp);
        assert_eq!(ColorTheme::Airbnb.shape(), Shape::Pill);
        // The Windows 11 blue, with dark text on the light dark-mode accent.
        assert_eq!(ColorTheme::Files.palette(false).accent, Color32::from_rgb(0x00, 0x67, 0xC0));
        assert_eq!(ColorTheme::Files.palette(true).on_accent, Color32::from_rgb(0x11, 0x11, 0x11));
    }

    #[test]
    fn the_black_theme_is_monochrome_and_pure_black_in_the_dark() {
        let d = ColorTheme::Black.palette(true);
        assert_eq!(d.bg, Color32::BLACK);
        assert_eq!(d.accent, Color32::WHITE);
        assert_eq!(d.on_accent, Color32::from_rgb(0x11, 0x11, 0x11), "dark text on the white button");
        let l = ColorTheme::Black.palette(false);
        assert_eq!(l.on_accent, Color32::WHITE, "white text on the near-black button");
    }
}
