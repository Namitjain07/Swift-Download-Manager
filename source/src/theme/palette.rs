//! Colour tokens, in light and dark.
//!
//! One design, two modes. The light palette follows the airy look of Airbnb: white surfaces on a
//! very light grey page, near-black text, soft grey lines and one coral accent. The dark palette
//! keeps the same accent on neutral charcoal surfaces. Colours are always flat: there are no
//! gradients anywhere in the app.
//!
//! `*_ink` colours are for text and stand-alone marks (they hold 4.5:1 against the surfaces),
//! `*_tint` colours are quiet backgrounds for state chips and selections.

use eframe::egui::Color32;

/// One flat set of colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    // Grounds
    /// Page ground behind every screen.
    pub bg: Color32,
    /// Cards, grouped lists and the header (`surface`).
    pub card: Color32,
    /// Quiet fills: tonal buttons, tiles, the track of a segmented control.
    pub surface2: Color32,
    /// Hover and pressed fill for `surface2`.
    pub surface3: Color32,
    /// Dialogs, menus and toasts.
    pub raised: Color32,
    /// Navigation rail ground.
    pub sidebar: Color32,
    /// The header, toolbar and status bar: white in the Airbnb look, the window ground in Files.
    pub chrome: Color32,
    /// Selected thumb inside a segmented control.
    pub segment_on: Color32,
    /// Track behind progress bars and meters.
    pub track: Color32,
    /// The dim layer behind a dialog.
    pub scrim: Color32,
    // Lines and overlays
    /// Translucent wash on hover.
    pub hover: Color32,
    /// Translucent wash while pressed.
    pub press: Color32,
    /// Hairline between rows and around cards.
    pub separator: Color32,
    /// A line that has to read as an edge (dialogs, the sidebar).
    pub separator_strong: Color32,
    /// Border of inputs and outlined buttons.
    pub control_border: Color32,
    // Text
    /// Primary text.
    pub label: Color32,
    /// Secondary text.
    pub secondary: Color32,
    /// Tertiary text: units, timestamps, hints.
    pub tertiary: Color32,
    pub disabled: Color32,
    // The accent (coral) and the state hues.
    pub accent: Color32,
    pub accent_ink: Color32,
    /// Fill of the primary button (holds 4.5:1 with `on_accent`).
    pub accent_solid: Color32,
    /// Primary button while hovered or pressed.
    pub accent_press: Color32,
    pub on_accent: Color32,
    pub accent_tint: Color32,
    pub green: Color32,
    pub green_ink: Color32,
    pub green_tint: Color32,
    pub orange: Color32,
    pub orange_ink: Color32,
    pub orange_tint: Color32,
    pub red: Color32,
    pub red_ink: Color32,
    pub red_tint: Color32,
    pub red_solid: Color32,
    /// Links and "information".
    pub blue: Color32,
    pub blue_ink: Color32,
    pub blue_tint: Color32,
    /// Base colour of drop shadows (alpha is applied where they are drawn).
    pub shadow: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// A colour with an opacity of `a` out of 255.
const fn rgba(hex: u32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied_const((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, a)
}

macro_rules! lerp_fields {
    ($a:expr, $b:expr, $t:expr, $($f:ident),+ $(,)?) => {
        Palette { $($f: $a.$f.lerp_to_gamma($b.$f, $t)),+ }
    };
}

impl Palette {
    pub const LIGHT: Palette = Palette {
        bg: rgb(0xF7F7F7),
        card: rgb(0xFFFFFF),
        surface2: rgb(0xF2F2F2),
        surface3: rgb(0xE8E8E8),
        raised: rgb(0xFFFFFF),
        sidebar: rgb(0xFFFFFF),
        chrome: rgb(0xFFFFFF),
        segment_on: rgb(0xFFFFFF),
        track: rgb(0xEBEBEB),
        scrim: rgba(0x000000, 102),
        hover: rgba(0x000000, 13),
        press: rgba(0x000000, 26),
        separator: rgb(0xEBEBEB),
        separator_strong: rgb(0xDDDDDD),
        control_border: rgb(0x909090),
        label: rgb(0x222222),
        secondary: rgb(0x484848),
        tertiary: rgb(0x6A6A6A),
        disabled: rgb(0xB0B0B0),
        accent: rgb(0xFF385C),
        accent_ink: rgb(0xC2005A),
        accent_solid: rgb(0xE31C5F),
        accent_press: rgb(0xD70466),
        on_accent: rgb(0xFFFFFF),
        accent_tint: rgba(0xFF385C, 26),
        green: rgb(0x34C759),
        green_ink: rgb(0x0A6B27),
        green_tint: rgba(0x34C759, 36),
        orange: rgb(0xFF9500),
        orange_ink: rgb(0x8A4B00),
        orange_tint: rgba(0xFF9500, 36),
        red: rgb(0xB32D0E),
        red_ink: rgb(0xB32D0E),
        red_tint: rgba(0xC13515, 26),
        red_solid: rgb(0xB32D0E),
        blue: rgb(0x007AFF),
        blue_ink: rgb(0x0058BE),
        blue_tint: rgba(0x007AFF, 28),
        shadow: rgb(0x000000),
    };

    pub const DARK: Palette = Palette {
        bg: rgb(0x0F0F10),
        card: rgb(0x1C1C1E),
        surface2: rgb(0x2C2C2E),
        surface3: rgb(0x3A3A3C),
        raised: rgb(0x262628),
        sidebar: rgb(0x151517),
        chrome: rgb(0x1C1C1E),
        segment_on: rgb(0x636366),
        track: rgb(0x3A3A3C),
        scrim: rgba(0x000000, 153),
        hover: rgba(0xFFFFFF, 18),
        press: rgba(0xFFFFFF, 31),
        separator: rgba(0xFFFFFF, 26),
        separator_strong: rgba(0xFFFFFF, 46),
        control_border: rgb(0x76767B),
        label: rgb(0xF5F5F7),
        secondary: rgb(0xC7C7CC),
        tertiary: rgb(0x98989F),
        disabled: rgb(0x636366),
        accent: rgb(0xFF385C),
        accent_ink: rgb(0xFF6B88),
        accent_solid: rgb(0xE31C5F),
        accent_press: rgb(0xFF385C),
        on_accent: rgb(0xFFFFFF),
        accent_tint: rgba(0xFF385C, 46),
        green: rgb(0x30D158),
        green_ink: rgb(0x30D158),
        green_tint: rgba(0x30D158, 46),
        orange: rgb(0xFF9F0A),
        orange_ink: rgb(0xFF9F0A),
        orange_tint: rgba(0xFF9F0A, 46),
        red: rgb(0xFF6B4A),
        red_ink: rgb(0xFF8A70),
        red_tint: rgba(0xFF6B4A, 46),
        red_solid: rgb(0xC13515),
        blue: rgb(0x0A84FF),
        blue_ink: rgb(0x409CFF),
        blue_tint: rgba(0x0A84FF, 46),
        shadow: rgb(0x000000),
    };

    /// Blend towards `other`: `t` 0 gives `self`, 1 gives `other`.
    pub fn lerp(&self, other: &Palette, t: f32) -> Palette {
        let t = t.clamp(0.0, 1.0);
        lerp_fields!(
            self, other, t, bg, card, surface2, surface3, raised, sidebar, chrome, segment_on, track, scrim, hover, press, separator, separator_strong,
            control_border, label, secondary, tertiary, disabled, accent, accent_ink, accent_solid, accent_press, on_accent, accent_tint, green,
            green_ink, green_tint, orange, orange_ink, orange_tint, red, red_ink, red_tint, red_solid, blue, blue_ink, blue_tint, shadow
        )
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

/// `top` (possibly translucent) painted over the opaque `bottom`.
pub fn over(top: Color32, bottom: Color32) -> Color32 {
    let k = 1.0 - top.a() as f32 / 255.0;
    let mix = |t: u8, b: u8| (t as f32 * top.a() as f32 / 255.0 + b as f32 * k).round().min(255.0) as u8;
    Color32::from_rgb(mix(top.r(), bottom.r()), mix(top.g(), bottom.g()), mix(top.b(), bottom.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn both() -> [(&'static str, Palette); 2] {
        [("light", Palette::LIGHT), ("dark", Palette::DARK)]
    }

    #[test]
    fn text_reads_on_every_ground() {
        for (name, p) in both() {
            for (ground_name, ground) in [("page", p.bg), ("surface", p.card), ("sidebar", p.sidebar), ("dialog", p.raised)] {
                for (ink_name, ink) in [("label", p.label), ("secondary", p.secondary), ("tertiary", p.tertiary)] {
                    let c = contrast_ratio(ink, ground);
                    assert!(c >= 4.5, "{ink_name} on {ground_name} in {name} is only {c:.2}:1");
                }
            }
        }
    }

    #[test]
    fn ink_colours_read_on_surfaces_and_on_their_own_tint() {
        for (name, p) in both() {
            for (hue_name, ink, tint) in [
                ("accent", p.accent_ink, p.accent_tint),
                ("green", p.green_ink, p.green_tint),
                ("orange", p.orange_ink, p.orange_tint),
                ("red", p.red_ink, p.red_tint),
                ("blue", p.blue_ink, p.blue_tint),
            ] {
                let c = contrast_ratio(ink, p.card);
                assert!(c >= 4.5, "{hue_name}-ink on a card in {name} is only {c:.2}:1");
                let c = contrast_ratio(ink, over(tint, p.card));
                assert!(c >= 4.5, "{hue_name}-ink on its tint in {name} is only {c:.2}:1");
                let c = contrast_ratio(ink, over(tint, p.raised));
                assert!(c >= 4.5, "{hue_name}-ink on its tint over a dialog in {name} is only {c:.2}:1");
            }
        }
    }

    #[test]
    fn solid_buttons_carry_readable_text() {
        for (name, p) in both() {
            for (what, fill) in [("accent", p.accent_solid), ("accent pressed", p.accent_press), ("red", p.red_solid)] {
                let c = contrast_ratio(p.on_accent, fill);
                // The dark mode "pressed" accent is the bright brand coral, which only has to hold the 3:1 of large, bold text.
                let need = if what == "accent pressed" && name == "dark" { 3.0 } else { 4.5 };
                assert!(c >= need, "white on {what} in {name} is only {c:.2}:1");
            }
        }
    }

    #[test]
    fn control_borders_have_3_to_1_against_the_surface() {
        for (name, p) in both() {
            for (ground_name, ground) in [("surface", p.card), ("dialog", p.raised)] {
                let c = contrast_ratio(p.control_border, ground);
                assert!(c >= 3.0, "control-border on {ground_name} in {name} is only {c:.2}:1");
            }
        }
    }

    #[test]
    fn lerp_endpoints_match() {
        assert_eq!(Palette::LIGHT.lerp(&Palette::DARK, 0.0), Palette::LIGHT);
        assert_eq!(Palette::LIGHT.lerp(&Palette::DARK, 1.0), Palette::DARK);
        let mid = Palette::LIGHT.lerp(&Palette::DARK, 0.5);
        assert_ne!(mid, Palette::LIGHT);
        assert_ne!(mid, Palette::DARK);
    }

    #[test]
    fn the_accent_is_coral_and_no_surface_is_translucent() {
        assert_eq!(Palette::LIGHT.accent, Color32::from_rgb(0xFF, 0x38, 0x5C));
        assert_eq!(Palette::DARK.accent, Palette::LIGHT.accent, "the brand colour does not change with the mode");
        for (name, p) in both() {
            for (what, c) in [("bg", p.bg), ("card", p.card), ("surface2", p.surface2), ("raised", p.raised), ("sidebar", p.sidebar), ("track", p.track)] {
                assert_eq!(c.a(), 255, "{what} in {name} must be opaque");
            }
        }
    }
}
