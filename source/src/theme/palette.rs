//! Colour tokens for both designs, in light and dark.
//!
//! `DRISHTI_*` follows the Drishti design system (tokens.json): dark by default, one blue accent,
//! `-ink` colours for text and stand-alone marks, tinted fills for state. `IOS_*` is the flat iOS look.
//! Both fill every token, so widgets never need to know which design is active.

use eframe::egui::Color32;

/// One flat set of colours. No gradients anywhere in the app.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    // Grounds
    /// Page ground behind every screen.
    pub bg: Color32,
    /// Cards, grouped lists and sheets (`surface`).
    pub card: Color32,
    /// Controls and nested fills (`surface-2`).
    pub surface2: Color32,
    /// Hover and pressed fill for controls (`surface-3`).
    pub surface3: Color32,
    /// Dialogs, menus and toasts (`popover`).
    pub raised: Color32,
    /// Navigation rail ground.
    pub sidebar: Color32,
    /// Selected thumb inside a segmented control.
    pub segment_on: Color32,
    /// Track behind progress bars and meters.
    pub track: Color32,
    // Lines and overlays
    pub hover: Color32,
    pub separator: Color32,
    pub separator_strong: Color32,
    pub control_border: Color32,
    // Text
    /// Primary text (`ink`).
    pub label: Color32,
    /// Secondary text (`ink-2`).
    pub secondary: Color32,
    /// Tertiary text: overline labels, units, timestamps (`ink-3`).
    pub tertiary: Color32,
    pub disabled: Color32,
    // Accent and state hues. `*_ink` is for text and stand-alone marks, `*_tint` for quiet backgrounds.
    pub blue: Color32,
    pub blue_ink: Color32,
    pub blue_solid: Color32,
    pub on_accent: Color32,
    pub blue_tint: Color32,
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
    pub const IOS_LIGHT: Palette = Palette {
        bg: rgb(0xF2F2F7),
        card: rgb(0xFFFFFF),
        surface2: rgb(0xE5E5EA),
        surface3: rgb(0xD1D1D6),
        raised: rgb(0xFFFFFF),
        sidebar: rgb(0xF2F2F7),
        segment_on: rgb(0xFFFFFF),
        track: rgb(0xE5E5EA),
        hover: rgba(0xC6C6C8, 77),
        separator: rgb(0xC6C6C8),
        separator_strong: rgb(0xC6C6C8),
        control_border: rgb(0xC6C6C8),
        label: rgb(0x000000),
        secondary: rgb(0x8A8A8E),
        tertiary: rgb(0x8A8A8E),
        disabled: rgb(0xC7C7CC),
        blue: rgb(0x007AFF),
        blue_ink: rgb(0x007AFF),
        blue_solid: rgb(0x007AFF),
        on_accent: rgb(0xFFFFFF),
        blue_tint: rgba(0x007AFF, 41),
        green: rgb(0x34C759),
        green_ink: rgb(0x34C759),
        green_tint: rgba(0x34C759, 41),
        orange: rgb(0xFF9500),
        orange_ink: rgb(0xFF9500),
        orange_tint: rgba(0xFF9500, 41),
        red: rgb(0xFF3B30),
        red_ink: rgb(0xFF3B30),
        red_tint: rgba(0xFF3B30, 41),
        red_solid: rgb(0xFF3B30),
    };

    pub const IOS_DARK: Palette = Palette {
        bg: rgb(0x000000),
        card: rgb(0x1C1C1E),
        surface2: rgb(0x2C2C2E),
        surface3: rgb(0x3A3A3C),
        raised: rgb(0x2C2C2E),
        sidebar: rgb(0x000000),
        segment_on: rgb(0x636366),
        track: rgb(0x2C2C2E),
        hover: rgba(0x38383A, 77),
        separator: rgb(0x38383A),
        separator_strong: rgb(0x38383A),
        control_border: rgb(0x38383A),
        label: rgb(0xFFFFFF),
        secondary: rgb(0x98989F),
        tertiary: rgb(0x98989F),
        disabled: rgb(0x48484A),
        blue: rgb(0x0A84FF),
        blue_ink: rgb(0x0A84FF),
        blue_solid: rgb(0x0A84FF),
        on_accent: rgb(0xFFFFFF),
        blue_tint: rgba(0x0A84FF, 41),
        green: rgb(0x30D158),
        green_ink: rgb(0x30D158),
        green_tint: rgba(0x30D158, 41),
        orange: rgb(0xFF9F0A),
        orange_ink: rgb(0xFF9F0A),
        orange_tint: rgba(0xFF9F0A, 41),
        red: rgb(0xFF453A),
        red_ink: rgb(0xFF453A),
        red_tint: rgba(0xFF453A, 41),
        red_solid: rgb(0xFF453A),
    };

    pub const DRISHTI_LIGHT: Palette = Palette {
        bg: rgb(0xF2F2F7),
        card: rgb(0xFFFFFF),
        surface2: rgb(0xE5E5EA),
        surface3: rgb(0xD1D1D6),
        raised: rgb(0xFFFFFF),
        sidebar: rgb(0xFFFFFF),
        segment_on: rgb(0xFFFFFF),
        track: rgb(0xD1D1D6),
        hover: rgba(0x000000, 13),
        separator: rgba(0x3C3C43, 36),
        separator_strong: rgba(0x3C3C43, 74),
        control_border: rgb(0x8E8E93),
        label: rgb(0x000000),
        secondary: rgb(0x48484A),
        tertiary: rgb(0x6E6E73),
        disabled: rgb(0xAEAEB2),
        blue: rgb(0x007AFF),
        blue_ink: rgb(0x005ECB),
        blue_solid: rgb(0x0071E3),
        on_accent: rgb(0xFFFFFF),
        blue_tint: rgba(0x007AFF, 31),
        green: rgb(0x34C759),
        green_ink: rgb(0x1A7332),
        green_tint: rgba(0x34C759, 31),
        orange: rgb(0xFF9500),
        orange_ink: rgb(0x9C4A00),
        orange_tint: rgba(0xFF9500, 31),
        red: rgb(0xFF3B30),
        red_ink: rgb(0xC4001A),
        red_tint: rgba(0xFF3B30, 31),
        red_solid: rgb(0xD70015),
    };

    pub const DRISHTI_DARK: Palette = Palette {
        bg: rgb(0x000000),
        card: rgb(0x1C1C1E),
        surface2: rgb(0x2C2C2E),
        surface3: rgb(0x3A3A3C),
        raised: rgb(0x2C2C2E),
        sidebar: rgb(0x111111),
        segment_on: rgb(0x636366),
        track: rgb(0x3A3A3C),
        hover: rgba(0xFFFFFF, 15),
        separator: rgba(0xFFFFFF, 20),
        separator_strong: rgba(0x545458, 166),
        control_border: rgb(0x6C6C70),
        label: rgb(0xFFFFFF),
        secondary: rgb(0xAEAEB2),
        tertiary: rgb(0x8E8E93),
        disabled: rgb(0x636366),
        blue: rgb(0x0A84FF),
        blue_ink: rgb(0x409CFF),
        blue_solid: rgb(0x0A6CFF),
        on_accent: rgb(0xFFFFFF),
        blue_tint: rgba(0x0A84FF, 41),
        green: rgb(0x30D158),
        green_ink: rgb(0x30D158),
        green_tint: rgba(0x30D158, 41),
        orange: rgb(0xFF9F0A),
        orange_ink: rgb(0xFF9F0A),
        orange_tint: rgba(0xFF9F0A, 41),
        red: rgb(0xFF453A),
        red_ink: rgb(0xFF6961),
        red_tint: rgba(0xFF453A, 41),
        red_solid: rgb(0xD70015),
    };

    /// Blend towards `other`: `t` 0 gives `self`, 1 gives `other`.
    pub fn lerp(&self, other: &Palette, t: f32) -> Palette {
        let t = t.clamp(0.0, 1.0);
        lerp_fields!(
            self, other, t, bg, card, surface2, surface3, raised, sidebar, segment_on, track, hover, separator, separator_strong,
            control_border, label, secondary, tertiary, disabled, blue, blue_ink, blue_solid, on_accent, blue_tint, green, green_ink,
            green_tint, orange, orange_ink, orange_tint, red, red_ink, red_tint, red_solid
        )
    }
}

#[cfg(test)]
fn luminance(c: Color32) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG contrast ratio between two colours (1 to 21). Used by the tests.
#[cfg(test)]
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `top` (possibly translucent) painted over the opaque `bottom`. Used by the tests.
#[cfg(test)]
pub fn over(top: Color32, bottom: Color32) -> Color32 {
    let k = 1.0 - top.a() as f32 / 255.0;
    let mix = |t: u8, b: u8| (t as f32 + b as f32 * k).round().min(255.0) as u8;
    Color32::from_rgb(mix(top.r(), bottom.r()), mix(top.g(), bottom.g()), mix(top.b(), bottom.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> [(&'static str, Palette); 4] {
        [("ios light", Palette::IOS_LIGHT), ("ios dark", Palette::IOS_DARK), ("drishti light", Palette::DRISHTI_LIGHT), ("drishti dark", Palette::DRISHTI_DARK)]
    }

    #[test]
    fn label_on_card_has_4_5_contrast_in_every_palette() {
        for (name, p) in all() {
            assert!(contrast_ratio(p.label, p.card) >= 4.5, "{name}");
        }
    }

    #[test]
    fn ios_secondary_label_on_card_is_at_least_3() {
        // #8A8A8E on white is about 3.4:1, so 4.5 is not reachable with the specified iOS colour.
        assert!(contrast_ratio(Palette::IOS_LIGHT.secondary, Palette::IOS_LIGHT.card) >= 3.0);
        assert!(contrast_ratio(Palette::IOS_DARK.secondary, Palette::IOS_DARK.card) >= 3.0);
    }

    #[test]
    fn lerp_endpoints_match_in_both_designs() {
        for (a, b) in [(Palette::IOS_LIGHT, Palette::IOS_DARK), (Palette::DRISHTI_LIGHT, Palette::DRISHTI_DARK)] {
            assert_eq!(a.lerp(&b, 0.0), a);
            assert_eq!(a.lerp(&b, 1.0), b);
        }
    }

    #[test]
    fn ios_palette_matches_spec_hex() {
        assert_eq!(Palette::IOS_LIGHT.bg, Color32::from_rgb(0xF2, 0xF2, 0xF7));
        assert_eq!(Palette::IOS_DARK.card, Color32::from_rgb(0x1C, 0x1C, 0x1E));
        assert_eq!(Palette::IOS_DARK.blue, Color32::from_rgb(0x0A, 0x84, 0xFF));
    }

    #[test]
    fn drishti_palette_matches_the_design_system_tokens() {
        let (l, d) = (Palette::DRISHTI_LIGHT, Palette::DRISHTI_DARK);
        assert_eq!(d.bg, Color32::from_rgb(0x00, 0x00, 0x00));
        assert_eq!(d.card, Color32::from_rgb(0x1C, 0x1C, 0x1E));
        assert_eq!(d.surface2, Color32::from_rgb(0x2C, 0x2C, 0x2E));
        assert_eq!(d.surface3, Color32::from_rgb(0x3A, 0x3A, 0x3C));
        assert_eq!(d.sidebar, Color32::from_rgb(0x11, 0x11, 0x11));
        assert_eq!(d.blue_solid, Color32::from_rgb(0x0A, 0x6C, 0xFF));
        assert_eq!(l.blue_ink, Color32::from_rgb(0x00, 0x5E, 0xCB));
        assert_eq!(l.green_ink, Color32::from_rgb(0x1A, 0x73, 0x32));
        assert_eq!(d.red_solid, Color32::from_rgb(0xD7, 0x00, 0x15));
        // Translucent tokens keep their opacity (rgba(255,255,255,0.06) is 15/255).
        assert_eq!(d.hover, Color32::from_rgba_unmultiplied(255, 255, 255, 15));
        assert_eq!(l.separator, Color32::from_rgba_unmultiplied(0x3C, 0x3C, 0x43, 36));
        assert_eq!(d.blue_tint, Color32::from_rgba_unmultiplied(10, 132, 255, 41));
    }

    #[test]
    fn drishti_ink_colours_read_on_every_ground_as_the_design_system_promises() {
        for (name, p) in [("light", Palette::DRISHTI_LIGHT), ("dark", Palette::DRISHTI_DARK)] {
            for (ground_name, ground) in [("bg", p.bg), ("surface", p.card), ("sidebar", p.sidebar)] {
                for (ink_name, ink) in [("ink", p.label), ("ink-2", p.secondary), ("ink-3", p.tertiary)] {
                    let c = contrast_ratio(ink, ground);
                    assert!(c >= 4.5, "{ink_name} on {ground_name} in {name} is only {c:.2}:1");
                }
            }
            for (hue_name, ink) in [("blue-ink", p.blue_ink), ("green-ink", p.green_ink), ("orange-ink", p.orange_ink), ("red-ink", p.red_ink)] {
                let c = contrast_ratio(ink, p.card);
                assert!(c >= 4.5, "{hue_name} on surface in {name} is only {c:.2}:1");
            }
            let c = contrast_ratio(p.blue_ink, over(p.blue_tint, p.card));
            assert!(c >= 4.5, "blue-ink on blue-tint in {name} is only {c:.2}:1");
            let c = contrast_ratio(p.on_accent, p.blue_solid);
            assert!(c >= 4.5, "on-accent on blue-solid in {name} is only {c:.2}:1");
            let c = contrast_ratio(p.on_accent, p.red_solid);
            assert!(c >= 4.5, "on-accent on red-solid in {name} is only {c:.2}:1");
        }
    }

    #[test]
    fn drishti_control_border_has_3_to_1_against_the_surface() {
        for (name, p) in [("light", Palette::DRISHTI_LIGHT), ("dark", Palette::DRISHTI_DARK)] {
            let c = contrast_ratio(p.control_border, p.card);
            assert!(c >= 3.0, "control-border on surface in {name} is only {c:.2}:1");
        }
    }
}
