//! The WinUI 3 theme (Windows 11, Fluent 2).
//!
//! This is not a recolouring of another theme: the colours are the design tokens of WinUI 3 itself, copied from Microsoft's own
//! XAML resources (`microsoft-ui-xaml`, `controls/dev/CommonStyles/Common_themeresources_any.xaml`), in light and dark, with the
//! same translucency they have there (`#AARRGGBB`). What WinUI draws on is a *material*: the window is Mica (or Mica Alt, or
//! Acrylic) and the layers on it are translucent, so that the colour of the wallpaper shows through the window the way it does
//! in the Settings app. Without a material (other systems, Windows 10, a window that was not made transparent, or the
//! "transparency effects" switch of Windows turned off) the same layers are mixed into solid colours (`Material::Solid`).
//!
//! The accent comes from the system on Windows (`UISettings`, the source WinUI itself reads) and is a set of seven shades,
//! of which light themes use the dark ones and dark themes the light ones, exactly as the XAML says (`AccentFillColorDefault`
//! is `SystemAccentColorDark1` in light and `SystemAccentColorLight2` in dark).

use super::palette::Palette;
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

/// A colour as XAML writes it: `#AARRGGBB`.
const fn argb(v: u32) -> Color32 {
    Color32::from_rgba_unmultiplied_const((v >> 16) as u8, (v >> 8) as u8, v as u8, (v >> 24) as u8)
}

/// An opaque colour: `#RRGGBB`.
const fn solid(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// What is under the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backdrop {
    /// Solid colours: no material.
    None,
    /// Mica: the wallpaper, blurred and tinted, behind the whole window. The default of Windows 11 apps.
    #[default]
    Mica,
    /// Mica Alt: a stronger tint, for apps with a tabbed title bar.
    MicaAlt,
    /// Acrylic: the blurred desktop behind the window, as flyouts have it.
    Acrylic,
}

impl Backdrop {
    #[cfg(test)]
    pub const ALL: [Backdrop; 4] = [Backdrop::Mica, Backdrop::MicaAlt, Backdrop::Acrylic, Backdrop::None];

    pub const fn name(self) -> &'static str {
        match self {
            Backdrop::None => "None",
            Backdrop::Mica => "Mica",
            Backdrop::MicaAlt => "Mica Alt",
            Backdrop::Acrylic => "Acrylic",
        }
    }
}

// ---------------------------------------------------------------- the tokens

/// The brushes of WinUI 3 that this app draws with, for one of the two themes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    // Text
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_tertiary: Color32,
    pub text_disabled: Color32,
    pub text_on_accent: Color32,
    pub text_on_accent_secondary: Color32,
    pub text_on_accent_disabled: Color32,
    // Control fills
    pub control_default: Color32,
    pub control_secondary: Color32,
    pub control_tertiary: Color32,
    pub control_disabled: Color32,
    pub control_input_active: Color32,
    pub control_solid: Color32,
    pub control_alt_secondary: Color32,
    pub control_alt_quarternary: Color32,
    pub subtle_secondary: Color32,
    pub subtle_tertiary: Color32,
    pub accent_disabled: Color32,
    // Strokes
    pub stroke_control: Color32,
    pub stroke_control_secondary: Color32,
    pub stroke_strong: Color32,
    pub stroke_strong_disabled: Color32,
    pub stroke_card: Color32,
    pub stroke_surface: Color32,
    pub stroke_flyout: Color32,
    pub stroke_on_accent: Color32,
    pub stroke_on_accent_secondary: Color32,
    pub divider: Color32,
    pub focus_outer: Color32,
    pub focus_inner: Color32,
    // Surfaces
    pub card_default: Color32,
    pub card_secondary: Color32,
    pub layer_default: Color32,
    pub layer_on_mica_alt: Color32,
    pub layer_on_acrylic: Color32,
    pub solid_base: Color32,
    pub solid_base_alt: Color32,
    pub solid_secondary: Color32,
    pub solid_tertiary: Color32,
    pub solid_quarternary: Color32,
    /// The dim layer behind a dialog.
    pub smoke: Color32,
    // The states
    pub critical: Color32,
    pub critical_background: Color32,
    pub success: Color32,
    pub success_background: Color32,
    pub caution: Color32,
    pub caution_background: Color32,
}

impl Tokens {
    pub const LIGHT: Tokens = Tokens {
        text_primary: argb(0xE4000000),
        text_secondary: argb(0x9E000000),
        text_tertiary: argb(0x72000000),
        text_disabled: argb(0x5C000000),
        text_on_accent: argb(0xFFFFFFFF),
        text_on_accent_secondary: argb(0xB3FFFFFF),
        text_on_accent_disabled: argb(0xFFFFFFFF),
        control_default: argb(0xB3FFFFFF),
        control_secondary: argb(0x80F9F9F9),
        control_tertiary: argb(0x4DF9F9F9),
        control_disabled: argb(0x4DF9F9F9),
        control_input_active: argb(0xFFFFFFFF),
        control_solid: argb(0xFFFFFFFF),
        control_alt_secondary: argb(0x06000000),
        control_alt_quarternary: argb(0x18000000),
        subtle_secondary: argb(0x09000000),
        subtle_tertiary: argb(0x06000000),
        accent_disabled: argb(0x37000000),
        stroke_control: argb(0x0F000000),
        stroke_control_secondary: argb(0x29000000),
        stroke_strong: argb(0x72000000),
        stroke_strong_disabled: argb(0x37000000),
        stroke_card: argb(0x0F000000),
        stroke_surface: argb(0x66757575),
        stroke_flyout: argb(0x0F000000),
        stroke_on_accent: argb(0x14FFFFFF),
        stroke_on_accent_secondary: argb(0x66000000),
        divider: argb(0x0F000000),
        focus_outer: argb(0xE4000000),
        focus_inner: argb(0xB3FFFFFF),
        card_default: argb(0xB3FFFFFF),
        card_secondary: argb(0x80F6F6F6),
        layer_default: argb(0x80FFFFFF),
        layer_on_mica_alt: argb(0xB3FFFFFF),
        layer_on_acrylic: argb(0x40FFFFFF),
        solid_base: solid(0xF3F3F3),
        solid_base_alt: solid(0xDADADA),
        solid_secondary: solid(0xEEEEEE),
        solid_tertiary: solid(0xF9F9F9),
        solid_quarternary: solid(0xFFFFFF),
        smoke: argb(0x4D000000),
        critical: solid(0xC42B1C),
        critical_background: solid(0xFDE7E9),
        success: solid(0x0F7B0F),
        success_background: solid(0xDFF6DD),
        caution: solid(0x9D5D00),
        caution_background: solid(0xFFF4CE),
    };

    pub const DARK: Tokens = Tokens {
        text_primary: argb(0xFFFFFFFF),
        text_secondary: argb(0xC5FFFFFF),
        text_tertiary: argb(0x87FFFFFF),
        text_disabled: argb(0x5DFFFFFF),
        text_on_accent: argb(0xFF000000),
        text_on_accent_secondary: argb(0x80000000),
        text_on_accent_disabled: argb(0x87FFFFFF),
        control_default: argb(0x0FFFFFFF),
        control_secondary: argb(0x15FFFFFF),
        control_tertiary: argb(0x08FFFFFF),
        control_disabled: argb(0x0BFFFFFF),
        control_input_active: argb(0xB31E1E1E),
        control_solid: argb(0xFF454545),
        control_alt_secondary: argb(0x19000000),
        control_alt_quarternary: argb(0x12FFFFFF),
        subtle_secondary: argb(0x0FFFFFFF),
        subtle_tertiary: argb(0x0AFFFFFF),
        accent_disabled: argb(0x28FFFFFF),
        stroke_control: argb(0x12FFFFFF),
        stroke_control_secondary: argb(0x18FFFFFF),
        stroke_strong: argb(0x8BFFFFFF),
        stroke_strong_disabled: argb(0x28FFFFFF),
        stroke_card: argb(0x19000000),
        stroke_surface: argb(0x66757575),
        stroke_flyout: argb(0x33000000),
        stroke_on_accent: argb(0x14FFFFFF),
        stroke_on_accent_secondary: argb(0x23000000),
        divider: argb(0x15FFFFFF),
        focus_outer: argb(0xFFFFFFFF),
        focus_inner: argb(0xB3000000),
        card_default: argb(0x0DFFFFFF),
        card_secondary: argb(0x08FFFFFF),
        layer_default: argb(0x4C3A3A3A),
        layer_on_mica_alt: argb(0x733A3A3A),
        layer_on_acrylic: argb(0x09FFFFFF),
        solid_base: solid(0x202020),
        solid_base_alt: solid(0x0A0A0A),
        solid_secondary: solid(0x1C1C1C),
        solid_tertiary: solid(0x282828),
        solid_quarternary: solid(0x2C2C2C),
        smoke: argb(0x4D000000),
        critical: solid(0xFF99A4),
        critical_background: solid(0x442726),
        success: solid(0x6CCB5F),
        success_background: solid(0x393D1B),
        caution: solid(0xFCE100),
        caution_background: solid(0x433519),
    };

    pub const fn of(dark: bool) -> &'static Tokens {
        if dark {
            &Tokens::DARK
        } else {
            &Tokens::LIGHT
        }
    }
}

// ---------------------------------------------------------------- the accent

/// The seven shades of an accent colour, as Windows hands them out (`UIColorType::Accent`, `AccentDark1` to `AccentLight3`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Accent {
    pub base: Color32,
    pub dark1: Color32,
    pub dark2: Color32,
    pub dark3: Color32,
    pub light1: Color32,
    pub light2: Color32,
    pub light3: Color32,
}

impl Accent {
    /// The default accent of Windows 11 (the blue of its logo) and the shades Windows makes from it.
    pub const WINDOWS_BLUE: Accent = Accent {
        base: solid(0x0078D4),
        dark1: solid(0x005FB8),
        dark2: solid(0x003D92),
        dark3: solid(0x001968),
        light1: solid(0x0091F8),
        light2: solid(0x4CC2FF),
        light3: solid(0x99EBFF),
    };

    /// The shades of any colour, made the way Windows makes them: lighter shades keep the hue's saturation less and less and
    /// lean towards cyan; darker ones lose brightness and lean towards blue. (The factors reproduce `WINDOWS_BLUE` from
    /// its base colour to within a few units per channel, see the tests.)
    pub fn from_base(base: Color32) -> Accent {
        let [r, g, b, _] = base.to_srgba_unmultiplied();
        let (h, s, v) = rgb_to_hsv(r, g, b);
        let shade = |hue: f32, sat: f32, val: f32| {
            let (r, g, b) = hsv_to_rgb(hue.rem_euclid(360.0), sat.clamp(0.0, 1.0), val.clamp(0.0, 1.0));
            Color32::from_rgb(r, g, b)
        };
        Accent {
            base: Color32::from_rgb(r, g, b),
            light1: shade(h - 0.1, s, v * 1.17),
            light2: shade(h - 6.0, s * 0.70, 1.0),
            light3: shade(h - 16.0, s * 0.40, 1.0),
            dark1: shade(h + 2.0, s, v * 0.869),
            dark2: shade(h + 9.0, s, v * 0.69),
            dark3: shade(h + 18.0, s, v * 0.49),
        }
    }

    /// `#RRGGBB` (with or without the `#`) as a number, for the settings file.
    pub fn parse_hex(text: &str) -> Option<u32> {
        let t = text.trim().trim_start_matches('#');
        (t.len() == 6).then(|| u32::from_str_radix(t, 16).ok()).flatten()
    }
}

fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let byte = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (byte(r), byte(g), byte(b))
}

// ---------------------------------------------------------------- what the system says

/// What the operating system says about the look, as far as it can be asked (Windows: `UISettings` and DWM).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SystemLook {
    /// The accent colour of the person's settings.
    pub accent: Option<Accent>,
    /// Whether apps are set to the dark theme.
    pub dark: Option<bool>,
    /// The "transparency effects" switch of Windows.
    pub effects: bool,
    /// Whether the system can draw a Mica or Acrylic backdrop behind a window (Windows 11 22H2 or newer).
    pub backdrops: bool,
    /// The text size slider of Windows, 1.0 to 2.25.
    pub text_scale: f32,
    /// The "animation effects" switch of Windows.
    pub animations: bool,
    /// A high contrast theme is on.
    pub high_contrast: bool,
}

impl SystemLook {
    /// What to assume when nothing can be asked (other systems, or the first frame).
    pub const UNKNOWN: SystemLook = SystemLook { accent: None, dark: None, effects: true, backdrops: false, text_scale: 1.0, animations: true, high_contrast: false };
}

// ---------------------------------------------------------------- the material and the accent in use

/// What the palette is built from, besides light or dark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Runtime {
    pub accent: Accent,
    /// What is really behind the window: `Backdrop::None` when the choice cannot be honoured.
    pub material: Backdrop,
    /// Counts the changes, so that the theme animates from one to the next.
    pub version: u32,
}

static RUNTIME: RwLock<Runtime> = RwLock::new(Runtime { accent: Accent::WINDOWS_BLUE, material: Backdrop::None, version: 0 });

/// The accent and the material in use now.
pub fn runtime() -> Runtime {
    *RUNTIME.read().unwrap_or_else(|e| e.into_inner())
}

impl Runtime {
    /// Takes a new accent and material; says whether anything changed (and counts the change if so).
    fn updated(&mut self, accent: Accent, material: Backdrop) -> bool {
        if self.accent == accent && self.material == material {
            return false;
        }
        *self = Runtime { accent, material, version: self.version.wrapping_add(1) };
        true
    }
}

/// Tells the theme which accent and material to use. Does nothing (and does not restart the animation of the theme)
/// when nothing changed. Returns whether something did.
pub fn set_runtime(accent: Accent, material: Backdrop) -> bool {
    RUNTIME.write().unwrap_or_else(|e| e.into_inner()).updated(accent, material)
}

/// `over` for colours that may be translucent: `top` laid over the opaque `bottom`, in sRGB, as WinUI mixes them.
pub fn flat(top: Color32, bottom: Color32) -> Color32 {
    let [tr, tg, tb, ta] = top.to_srgba_unmultiplied();
    let [br, bg, bb, _] = bottom.to_srgba_unmultiplied();
    let a = ta as f32 / 255.0;
    let mix = |t: u8, b: u8| (t as f32 * a + b as f32 * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(mix(tr, br), mix(tg, bg), mix(tb, bb))
}

/// `c` with an opacity of `opacity` (0 to 1) of what it had.
fn faded(c: Color32, opacity: f32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (a as f32 * opacity).round() as u8)
}

/// The accent fill of the theme: `AccentFillColorDefault`.
pub fn accent_fill(accent: &Accent, dark: bool) -> Color32 {
    if dark {
        accent.light2
    } else {
        accent.dark1
    }
}

/// The accent for text: `AccentTextFillColorPrimary`.
pub fn accent_text(accent: &Accent, dark: bool) -> Color32 {
    if dark {
        accent.light3
    } else {
        accent.dark2
    }
}

/// The palette of the WinUI 3 theme.
pub fn palette(dark: bool, rt: &Runtime) -> Palette {
    let t = Tokens::of(dark);
    let fill = accent_fill(&rt.accent, dark);
    let ink = accent_text(&rt.accent, dark);

    // The layers mixed into solid colours: what the window looks like without a material, and the colour the translucent
    // fills are measured against.
    let ground = t.solid_base;
    let page_solid = flat(t.layer_default, ground);
    let card_solid = flat(t.card_default, page_solid);
    let (chrome, sidebar, bg, card) = match rt.material {
        Backdrop::None => (ground, ground, page_solid, card_solid),
        Backdrop::Mica => (Color32::TRANSPARENT, Color32::TRANSPARENT, t.layer_default, t.card_default),
        Backdrop::MicaAlt => (Color32::TRANSPARENT, Color32::TRANSPARENT, t.layer_on_mica_alt, t.card_default),
        Backdrop::Acrylic => (Color32::TRANSPARENT, Color32::TRANSPARENT, t.layer_on_acrylic, t.card_default),
    };

    let status = |c: Color32, background: Color32| (c, c, background);
    let (green, green_ink, green_tint) = status(t.success, t.success_background);
    let (orange, orange_ink, orange_tint) = status(t.caution, t.caution_background);
    let (red, red_ink, red_tint) = status(t.critical, t.critical_background);
    let blue_tint = faded(fill, if dark { 0.16 } else { 0.10 });

    Palette {
        bg,
        card,
        surface2: t.control_default,
        surface3: t.control_secondary,
        raised: t.solid_quarternary,
        sidebar,
        chrome,
        segment_on: t.control_solid,
        track: t.control_alt_quarternary,
        scrim: t.smoke,
        hover: t.subtle_secondary,
        press: t.subtle_tertiary,
        separator: t.divider,
        separator_strong: t.stroke_surface,
        control_border: t.stroke_strong,
        label: t.text_primary,
        secondary: t.text_secondary,
        // WinUI sets hints, captions and placeholders in its secondary text colour; the tertiary one is for what is switched off.
        tertiary: t.text_secondary,
        disabled: t.text_disabled,
        accent: fill,
        accent_ink: ink,
        accent_solid: fill,
        // AccentFillColorSecondary and Tertiary: the accent at 90 % and 80 %, mixed here into the card they lie on.
        accent_press: flat(faded(fill, 0.8), card_solid),
        on_accent: t.text_on_accent,
        accent_tint: faded(fill, if dark { 0.16 } else { 0.10 }),
        green,
        green_ink,
        green_tint,
        orange,
        orange_ink,
        orange_tint,
        red,
        red_ink,
        red_tint,
        red_solid: solid(0xC42B1C),
        blue: fill,
        blue_ink: ink,
        blue_tint,
        ctl: t.control_default,
        ctl_hover: t.control_secondary,
        ctl_press: t.control_tertiary,
        ctl_stroke: t.stroke_control,
        ctl_stroke_bottom: t.stroke_control_secondary,
        ctl_strong: t.stroke_strong,
        accent_hover: flat(faded(fill, 0.9), card_solid),
        focus: t.focus_outer,
        shadow: Color32::BLACK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::palette::contrast_ratio;

    fn rt(material: Backdrop) -> Runtime {
        Runtime { accent: Accent::WINDOWS_BLUE, material, version: 0 }
    }

    /// A colour as it looks on `ground`.
    fn seen(c: Color32, ground: Color32) -> Color32 {
        flat(c, ground)
    }

    #[test]
    fn the_tokens_are_the_ones_in_the_xaml_of_winui() {
        // A few of them, checked against Common_themeresources_any.xaml, in the order #AARRGGBB. A translucent colour is stored
        // premultiplied, which can cost a unit in a channel when it is read back.
        let is = |c: Color32, want: [u8; 4]| {
            let got = c.to_srgba_unmultiplied();
            assert!((0..3).all(|i| (got[i] as i32 - want[i] as i32).abs() <= 2) && got[3] == want[3], "{got:?} is not {want:?}");
        };
        is(Tokens::LIGHT.text_primary, [0, 0, 0, 0xE4]);
        is(Tokens::DARK.text_primary, [255, 255, 255, 255]);
        assert_eq!(Tokens::LIGHT.solid_base, Color32::from_rgb(0xF3, 0xF3, 0xF3));
        assert_eq!(Tokens::DARK.solid_base, Color32::from_rgb(0x20, 0x20, 0x20));
        is(Tokens::LIGHT.control_default, [255, 255, 255, 0xB3]);
        is(Tokens::DARK.control_default, [255, 255, 255, 0x0F]);
        is(Tokens::LIGHT.layer_default, [255, 255, 255, 0x80]);
        is(Tokens::DARK.layer_default, [0x3A, 0x3A, 0x3A, 0x4C]);
        is(Tokens::DARK.layer_on_mica_alt, [0x3A, 0x3A, 0x3A, 0x73]);
        is(Tokens::LIGHT.stroke_surface, [0x75, 0x75, 0x75, 0x66]);
        assert_eq!(Tokens::LIGHT.critical, Color32::from_rgb(0xC4, 0x2B, 0x1C));
        assert_eq!(Tokens::DARK.success, Color32::from_rgb(0x6C, 0xCB, 0x5F));
        is(Tokens::DARK.smoke, [0, 0, 0, 0x4D]);
    }

    #[test]
    fn the_accent_follows_the_rules_of_the_xaml() {
        // Light: AccentFillColorDefault = Dark1, text = Dark2. Dark: Light2 and Light3. Text on the accent: white / black.
        let (l, d) = (palette(false, &rt(Backdrop::None)), palette(true, &rt(Backdrop::None)));
        assert_eq!(l.accent_solid, Accent::WINDOWS_BLUE.dark1);
        assert_eq!(l.accent_ink, Accent::WINDOWS_BLUE.dark2);
        assert_eq!(d.accent_solid, Accent::WINDOWS_BLUE.light2);
        assert_eq!(d.accent_ink, Accent::WINDOWS_BLUE.light3);
        assert_eq!(l.on_accent, Color32::WHITE);
        assert_eq!(d.on_accent, Color32::BLACK);
    }

    #[test]
    fn the_shades_made_from_a_base_colour_match_windows_for_its_default_blue() {
        let made = Accent::from_base(Accent::WINDOWS_BLUE.base);
        let near = |a: Color32, b: Color32, tolerance: i32| {
            let (a, b) = (a.to_srgba_unmultiplied(), b.to_srgba_unmultiplied());
            (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() <= tolerance)
        };
        for (name, got, want) in [
            ("light1", made.light1, Accent::WINDOWS_BLUE.light1),
            ("light2", made.light2, Accent::WINDOWS_BLUE.light2),
            ("light3", made.light3, Accent::WINDOWS_BLUE.light3),
            ("dark1", made.dark1, Accent::WINDOWS_BLUE.dark1),
            ("dark2", made.dark2, Accent::WINDOWS_BLUE.dark2),
            ("dark3", made.dark3, Accent::WINDOWS_BLUE.dark3),
        ] {
            assert!(near(got, want, 14), "{name}: made {got:?}, Windows has {want:?}");
        }
        assert_eq!(made.base, Accent::WINDOWS_BLUE.base);
        // Any other colour gets seven shades that go from light to dark.
        let red = Accent::from_base(Color32::from_rgb(0xE8, 0x11, 0x23));
        let lum = |c: Color32| contrast_ratio(c, Color32::BLACK);
        assert!(lum(red.light3) > lum(red.light2) && lum(red.light2) > lum(red.light1) && lum(red.light1) > lum(red.base) && lum(red.base) > lum(red.dark1) && lum(red.dark1) > lum(red.dark2) && lum(red.dark2) > lum(red.dark3));
    }

    #[test]
    fn hex_accents_are_read_from_the_settings() {
        assert_eq!(Accent::parse_hex("#0078D4"), Some(0x0078D4));
        assert_eq!(Accent::parse_hex(" ff0000 "), Some(0xFF0000));
        assert_eq!(Accent::parse_hex("#0078D"), None);
        assert_eq!(Accent::parse_hex("zzzzzz"), None);
    }

    #[test]
    fn without_a_material_every_ground_is_a_solid_colour_and_text_reads_on_all_of_them() {
        for dark in [false, true] {
            let p = palette(dark, &rt(Backdrop::None));
            for (name, c) in [("bg", p.bg), ("card", p.card), ("raised", p.raised), ("chrome", p.chrome), ("sidebar", p.sidebar)] {
                assert_eq!(c.a(), 255, "{name} dark={dark}");
            }
            for (ground_name, ground) in [("window", p.chrome), ("page", p.bg), ("card", p.card), ("dialog", p.raised)] {
                // The text tokens are translucent: look at them on the ground, as the eye does.
                for (ink_name, ink) in [("primary", p.label), ("secondary", p.secondary)] {
                    let c = contrast_ratio(seen(ink, ground), ground);
                    assert!(c >= 4.5, "{ink_name} text on the {ground_name} in dark={dark} is only {c:.2}:1");
                }
                // Tertiary text is for hints and units: WinUI makes it a little softer.
                assert!(contrast_ratio(seen(p.tertiary, ground), ground) >= 3.0, "tertiary on {ground_name}");
            }
        }
    }

    #[test]
    fn the_accent_button_carries_readable_text_in_every_state() {
        for dark in [false, true] {
            let p = palette(dark, &rt(Backdrop::None));
            for (state, fill, need) in [("rest", p.accent_solid, 4.5), ("hover", p.accent_hover, 4.5), ("pressed", p.accent_press, 3.0)] {
                let c = contrast_ratio(p.on_accent, fill);
                // WinUI itself draws the pressed accent button at 80 % opacity (and its text at 70 %): that is below 4.5:1.
                assert!(c >= need, "text on the {state} accent button in dark={dark} is only {c:.2}:1");
            }
            let c = contrast_ratio(p.on_accent, p.red_solid);
            assert!(c >= 4.5 || dark, "white on the danger button: {c:.2}");
        }
    }

    #[test]
    fn the_state_colours_read_on_their_own_backgrounds() {
        for dark in [false, true] {
            let p = palette(dark, &rt(Backdrop::None));
            for (name, ink, tint) in [("success", p.green_ink, p.green_tint), ("caution", p.orange_ink, p.orange_tint), ("critical", p.red_ink, p.red_tint)] {
                let c = contrast_ratio(ink, tint);
                assert!(c >= 4.5, "{name} on its background in dark={dark} is only {c:.2}:1");
            }
            let tinted = seen(p.blue_tint, p.card);
            assert!(contrast_ratio(p.blue_ink, tinted) >= 4.5, "accent text on its tint in dark={dark}");
        }
    }

    #[test]
    fn a_material_lets_it_show_through_the_window_and_the_sidebar() {
        for material in [Backdrop::Mica, Backdrop::MicaAlt, Backdrop::Acrylic] {
            for dark in [false, true] {
                let p = palette(dark, &rt(material));
                assert_eq!(p.chrome.a(), 0, "{material:?}: the title bar and toolbar are the material itself");
                assert_eq!(p.sidebar.a(), 0, "{material:?}: and so is the navigation pane");
                assert!(p.bg.a() > 0 && p.bg.a() < 255, "{material:?}: the page is a translucent layer on it");
                assert!(p.card.a() < 255, "{material:?}: cards are translucent too");
                assert_eq!(p.raised.a(), 255, "dialogs and menus are solid");
            }
        }
        // Mica Alt has a stronger layer than Mica (LayerOnMicaBaseAlt against LayerFillColorDefault).
        assert!(palette(false, &rt(Backdrop::MicaAlt)).bg.a() > palette(false, &rt(Backdrop::Mica)).bg.a());
        assert!(palette(false, &rt(Backdrop::Acrylic)).bg.a() < palette(false, &rt(Backdrop::Mica)).bg.a());
    }

    #[test]
    fn the_control_brushes_are_the_ones_of_a_winui_button() {
        let (l, d) = (palette(false, &rt(Backdrop::None)), palette(true, &rt(Backdrop::None)));
        assert_eq!(l.ctl, Tokens::LIGHT.control_default);
        assert_eq!(l.ctl_hover, Tokens::LIGHT.control_secondary);
        assert_eq!(l.ctl_press, Tokens::LIGHT.control_tertiary);
        assert_eq!(d.ctl_stroke_bottom, Tokens::DARK.stroke_control_secondary);
        assert_eq!(l.hover, Tokens::LIGHT.subtle_secondary, "list items and icon buttons use the subtle fills");
        assert_eq!(l.separator, Tokens::LIGHT.divider);
        assert_eq!(d.focus, Color32::WHITE);
    }

    #[test]
    fn the_runtime_counts_changes_and_ignores_repeats() {
        // On a copy: the real one is shared by every test that builds a palette.
        let mut r = rt(Backdrop::None);
        let green = Accent::from_base(Color32::from_rgb(0x10, 0x7C, 0x10));
        assert!(r.updated(green, Backdrop::MicaAlt));
        assert!(!r.updated(green, Backdrop::MicaAlt), "the same again changes nothing");
        assert_eq!((r.version, r.material, r.accent), (1, Backdrop::MicaAlt, green));
        assert!(r.updated(green, Backdrop::None), "another material is a change");
        assert!(r.updated(Accent::WINDOWS_BLUE, Backdrop::None), "so is another accent");
        assert_eq!(r.version, 3);
    }

    #[test]
    fn backdrops_are_saved_by_name() {
        for b in Backdrop::ALL {
            let text = serde_json::to_string(&b).unwrap();
            assert_eq!(serde_json::from_str::<Backdrop>(&text).unwrap(), b);
        }
        assert_eq!(serde_json::to_string(&Backdrop::MicaAlt).unwrap(), "\"mica_alt\"");
        assert_eq!(Backdrop::default(), Backdrop::Mica);
    }
}
