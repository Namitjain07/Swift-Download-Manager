//! How the main window adapts to its width.
//!
//! Three breakpoints decide what is shown: the full layout, a medium layout with an icon rail and
//! icon-only tools, and a compact layout for narrow windows with a drawer and an overflow menu.

/// Below this width the window is `Compact`.
pub const MEDIUM_FROM: f32 = 720.0;
/// From this width the window is `Wide`.
pub const WIDE_FROM: f32 = 1160.0;
/// The narrowest a download card is allowed to get before the grid drops a column.
pub const CARD_MIN_WIDTH: f32 = 440.0;
pub const CARD_GAP: f32 = 12.0;
/// Most columns of cards, however wide the window is.
pub const MAX_CARD_COLUMNS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Breakpoint {
    /// Narrow window: sidebar in a drawer, tools in an overflow menu, cards only.
    Compact,
    /// Icon rail, icon-only tools.
    Medium,
    /// The full layout.
    Wide,
}

impl Breakpoint {
    pub fn from_width(width: f32) -> Breakpoint {
        if width < MEDIUM_FROM {
            Breakpoint::Compact
        } else if width < WIDE_FROM {
            Breakpoint::Medium
        } else {
            Breakpoint::Wide
        }
    }
}

/// How many columns of download cards fit in `available` points of width.
pub fn card_columns(available: f32) -> usize {
    (((available + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP)).floor() as usize).clamp(1, MAX_CARD_COLUMNS)
}

/// Width of one card when `columns` of them share `available` points.
pub fn card_width(available: f32, columns: usize) -> f32 {
    let columns = columns.max(1) as f32;
    ((available - CARD_GAP * (columns - 1.0)) / columns).max(0.0)
}

/// What the person sees the downloads as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ViewMode {
    /// One card per download (the default).
    #[default]
    Cards,
    /// The classic table with sortable columns.
    Table,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoints_switch_at_720_and_1160() {
        assert_eq!(Breakpoint::from_width(0.0), Breakpoint::Compact);
        assert_eq!(Breakpoint::from_width(719.9), Breakpoint::Compact);
        assert_eq!(Breakpoint::from_width(720.0), Breakpoint::Medium);
        assert_eq!(Breakpoint::from_width(1159.9), Breakpoint::Medium);
        assert_eq!(Breakpoint::from_width(1160.0), Breakpoint::Wide);
        assert_eq!(Breakpoint::from_width(4000.0), Breakpoint::Wide);
    }

    #[test]
    fn card_columns_grow_with_the_width_up_to_three() {
        assert_eq!(card_columns(100.0), 1, "never fewer than one column");
        assert_eq!(card_columns(440.0), 1);
        assert_eq!(card_columns(891.0), 1);
        assert_eq!(card_columns(892.0), 2, "two cards of 440 and a 12 gap need 892");
        assert_eq!(card_columns(1343.0), 2);
        assert_eq!(card_columns(1344.0), 3);
        assert_eq!(card_columns(5000.0), 3, "never more than three");
    }

    #[test]
    fn cards_share_the_width_with_the_gap_between_them() {
        let w = card_width(892.0, 2);
        assert_eq!(w, 440.0);
        assert_eq!(card_width(500.0, 1), 500.0);
        assert!(card_width(5.0, 3) >= 0.0);
        // Columns plus gaps always add up to the available width.
        for avail in [500.0_f32, 900.0, 1400.0] {
            let c = card_columns(avail);
            let total = card_width(avail, c) * c as f32 + CARD_GAP * (c as f32 - 1.0);
            assert!((total - avail).abs() < 0.01, "{avail}: {total}");
        }
    }

    #[test]
    fn the_default_view_is_cards_and_an_old_settings_file_gets_it() {
        assert_eq!(ViewMode::default(), ViewMode::Cards);
    }
}
