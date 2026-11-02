//! Flat iOS-style colours for light and dark mode.
// Palette helpers are wired into the UI in the next commits.
#![allow(dead_code)]

use eframe::egui::Color32;
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
}
