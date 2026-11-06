//! Icons: the Phosphor icon font (bundled, MIT licensed), in its line and solid weights.
//!
//! Every icon is one character of the font, drawn as text, so icons are crisp at every size and
//! follow the theme through their tint. The line weight is the default; play, pause and stop are
//! drawn solid, the way iOS draws its media controls.

use crate::theme::{self, Palette};
use eframe::egui::{Align2, Color32, CornerRadius, FontId, Painter, Rect, Vec2};

/// Every icon the app uses, with the Phosphor character it is drawn from.
macro_rules! icons {
    ($($name:ident = $cp:expr),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Icon {
            $($name),+
        }

        impl Icon {
            /// The character of this icon in the Phosphor font.
            pub const fn glyph(self) -> char {
                let cp: u32 = match self {
                    $(Icon::$name => $cp),+
                };
                match char::from_u32(cp) {
                    Some(c) => c,
                    None => '?',
                }
            }

            /// Every icon, for tests.
            #[cfg(test)]
            pub const ALL: &'static [Icon] = &[$(Icon::$name),+];
        }
    };
}

icons! {
    AddUrl = 0xe2e6,
    Plus = 0xe3d4,
    Batch = 0xe2f8,
    Resume = 0xe3d0,
    Pause = 0xe39e,
    PauseAll = 0xe3a0,
    Delete = 0xe4a6,
    ClearDone = 0xec54,
    Options = 0xe270,
    Scheduler = 0xe19a,
    StartQueue = 0xe3d2,
    StopQueue = 0xe46e,
    Folder = 0xe25a,
    File = 0xe230,
    FileText = 0xe23a,
    FileZip = 0xe958,
    Music = 0xe340,
    Program = 0xe5da,
    Video = 0xe792,
    Close = 0xe4f6,
    Menu = 0xe2f0,
    Cards = 0xe464,
    List = 0xe5a2,
    Search = 0xe30c,
    Check = 0xe182,
    CheckCircle = 0xe184,
    WarningCircle = 0xe4e2,
    Info = 0xe2ce,
    CaretDown = 0xe136,
    CaretRight = 0xe13a,
    CaretUp = 0xe13c,
    Copy = 0xe1ca,
    Robot = 0xe762,
    Download = 0xe20c,
    Hourglass = 0xe2b2,
    Queue = 0xe6ac,
    Minus = 0xe32a,
    Speed = 0xee74,
    Sidebar = 0xec24,
}

impl Icon {
    /// Icons that read better solid, like the media controls on an iPhone.
    pub const fn solid(self) -> bool {
        matches!(self, Icon::Resume | Icon::Pause | Icon::PauseAll | Icon::StartQueue | Icon::StopQueue | Icon::CheckCircle | Icon::WarningCircle)
    }
}

/// The colour a toolbar button gives each icon: the accent for adding, red for removing, ink for the rest.
pub fn tint(icon: Icon, pal: &Palette) -> Color32 {
    match icon {
        Icon::AddUrl | Icon::Plus => pal.accent_ink,
        Icon::Delete | Icon::ClearDone => pal.red_ink,
        _ => pal.label,
    }
}

/// Disabled icons are drawn at about a third of their strength.
fn dim_for(tint: Color32, enabled: bool) -> Color32 {
    if enabled { tint } else { tint.gamma_multiply(0.35) }
}

/// Draws `icon` centred in `rect`. The icon is as big as the smaller side of `rect`.
pub fn draw(p: &Painter, rect: Rect, icon: Icon, tint: Color32, enabled: bool) {
    draw_weight(p, rect, icon, tint, enabled, icon.solid());
}

/// The same, choosing the solid or the line weight.
pub fn draw_weight(p: &Painter, rect: Rect, icon: Icon, tint: Color32, enabled: bool, solid: bool) {
    let size = rect.width().min(rect.height());
    let family = if solid { theme::icon_fill_font() } else { theme::icon_font() };
    p.text(rect.center(), Align2::CENTER_CENTER, icon.glyph(), FontId::new(size, family), dim_for(tint, enabled));
}

/// Colour used for a category / file type.
pub fn category_color(cat: &str) -> Color32 {
    match cat {
        "Compressed" => Color32::from_rgb(214, 130, 20),
        "Documents" => Color32::from_rgb(52, 120, 210),
        "Music" => Color32::from_rgb(170, 80, 190),
        "Programs" => Color32::from_rgb(40, 160, 100),
        "Video" => Color32::from_rgb(226, 60, 80),
        "General" => Color32::from_rgb(120, 130, 145),
        other => {
            // Your own categories get a steady colour from their name.
            const P: [(u8, u8, u8); 6] = [(0, 150, 160), (220, 110, 40), (120, 90, 200), (190, 60, 120), (90, 140, 40), (60, 100, 170)];
            let h = other.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
            let (r, g, b) = P[(h % 6) as usize];
            Color32::from_rgb(r, g, b)
        }
    }
}

/// The icon of a category / file type.
pub fn category_icon(cat: &str) -> Icon {
    match cat {
        "Compressed" => Icon::FileZip,
        "Documents" => Icon::FileText,
        "Music" => Icon::Music,
        "Programs" => Icon::Program,
        "Video" => Icon::Video,
        "General" => Icon::File,
        _ => Icon::Folder,
    }
}

/// A rounded square in the category's colour with the category's icon on it: the picture of a file.
pub fn file_tile(p: &Painter, rect: Rect, cat: &str) {
    let col = category_color(cat);
    let radius = (rect.width().min(rect.height()) * 0.28).round().clamp(6.0, 14.0) as u8;
    p.rect_filled(rect, CornerRadius::same(radius), col.gamma_multiply(0.16));
    let side = rect.width().min(rect.height()) * 0.5;
    draw(p, Rect::from_center_size(rect.center(), Vec2::splat(side)), category_icon(cat), col, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::themed;
    use crate::theme::ThemeMode;
    use eframe::egui::{self, pos2, vec2};

    #[test]
    fn every_icon_is_in_the_font_in_both_weights() {
        let ctx = themed(ThemeMode::Light);
        for &icon in Icon::ALL {
            for family in [theme::icon_font(), theme::icon_fill_font()] {
                let ok = ctx.fonts_mut(|f| f.has_glyph(&FontId::new(16.0, family.clone()), icon.glyph()));
                assert!(ok, "{icon:?} (U+{:04X}) is not in {family:?}", icon.glyph() as u32);
            }
        }
    }

    #[test]
    fn no_two_icons_share_a_glyph() {
        let mut seen = std::collections::BTreeMap::new();
        for &icon in Icon::ALL {
            seen.entry(icon.glyph()).or_insert_with(Vec::new).push(icon);
        }
        for (glyph, icons) in seen {
            assert_eq!(icons.len(), 1, "U+{:04X} is shared by {icons:?}", glyph as u32);
        }
    }

    #[test]
    fn every_icon_paints_in_both_states_and_stays_in_its_box() {
        let ctx = themed(ThemeMode::Light);
        for &icon in Icon::ALL {
            for enabled in [true, false] {
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    let rect = Rect::from_center_size(pos2(40.0, 40.0), vec2(24.0, 24.0));
                    draw(ui.painter(), rect, icon, tint(icon, &Palette::LIGHT), enabled);
                });
                out.textures_delta.clear();
                assert!(!out.shapes.is_empty(), "{icon:?} painted nothing (enabled={enabled})");
                for s in &out.shapes {
                    let r = s.shape.visual_bounding_rect();
                    assert!(r.min.x.is_finite() && r.max.y.is_finite(), "non-finite geometry");
                    assert!(r.width() <= 40.0 && r.height() <= 40.0, "{icon:?} spills out of its box: {r:?}");
                }
            }
        }
    }

    #[test]
    fn a_disabled_icon_is_dimmer_than_an_enabled_one() {
        let on = tint(Icon::Delete, &Palette::LIGHT);
        assert!(dim_for(on, false).a() < dim_for(on, true).a());
    }

    #[test]
    fn media_controls_are_solid_and_the_rest_are_line_icons() {
        assert!(Icon::Pause.solid() && Icon::Resume.solid() && Icon::StopQueue.solid());
        assert!(!Icon::Folder.solid() && !Icon::Search.solid() && !Icon::Options.solid());
    }

    #[test]
    fn categories_have_a_colour_and_an_icon_of_their_own() {
        let names = ["Compressed", "Documents", "Music", "Programs", "Video", "General"];
        let icons: std::collections::BTreeSet<u32> = names.iter().map(|n| category_icon(n).glyph() as u32).collect();
        assert_eq!(icons.len(), names.len(), "each built-in category has its own icon");
        assert_eq!(category_color("Mine"), category_color("Mine"), "a custom category keeps its colour");
        assert_eq!(category_icon("Mine"), Icon::Folder);
    }

    #[test]
    fn a_file_tile_paints_a_tint_and_an_icon() {
        let ctx = themed(ThemeMode::Light);
        let mut out = ctx.run_ui(Default::default(), |ui| {
            file_tile(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, vec2(44.0, 44.0)), "Video");
        });
        out.textures_delta.clear();
        assert!(out.shapes.len() >= 2, "a rounded square and a glyph");
    }
}
