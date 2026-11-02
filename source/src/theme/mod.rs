//! The look of the app: two designs (Drishti and iOS), each in light and dark.
//!
//! - `Palette` (see `palette.rs`) holds the colours.
//! - `Design` holds the sizes and rules that differ between the designs: radii, control and row
//!   heights, whether cards have borders, whether floating layers cast a shadow, and so on.
//! - `apply` is called once per frame. It fades between light and dark, and restyles egui only
//!   while the palette or the design changes, so an idle window costs nothing.

pub mod palette;

use eframe::egui::{self, epaint::Shadow, Color32, CornerRadius, FontId, Id, Stroke, TextStyle, Theme, ThemePreference};
pub use palette::Palette;
use serde::{Deserialize, Serialize};

/// Which theme the person wants. `System` follows the operating system live.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// Which design is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeStyle {
    /// The Drishti design system: dark first, one blue accent, 36 px controls, lists on surfaces.
    #[default]
    Drishti,
    /// The flat iOS look: grouped cards, capsule buttons.
    Ios,
}

impl ThemeStyle {
    /// (light, dark) colours of this design.
    pub fn palettes(self) -> (Palette, Palette) {
        match self {
            ThemeStyle::Drishti => (Palette::DRISHTI_LIGHT, Palette::DRISHTI_DARK),
            ThemeStyle::Ios => (Palette::IOS_LIGHT, Palette::IOS_DARK),
        }
    }
}

/// Sizes and rules of a design. Widgets read these instead of hard-coding numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Design {
    pub style: ThemeStyle,
    /// Icon tiles, segmented thumbs, small buttons.
    pub radius_sm: u8,
    /// Inputs, buttons, nav rows.
    pub radius_md: u8,
    /// Cards, grouped lists.
    pub radius_lg: u8,
    /// Dialogs and sheets.
    pub radius_xl: u8,
    /// Height of our buttons and chips-with-actions.
    pub control_h: f32,
    /// Minimum height of egui's own widgets (text fields, combo boxes).
    pub widget_h: f32,
    /// Height of a download row.
    pub row_h: f32,
    /// Width of the sidebar.
    pub sidebar_w: f32,
    /// Height of a sidebar row.
    pub nav_h: f32,
    /// Inset of the hairline under a list row.
    pub inset: f32,
    pub body: f32,
    pub caption: f32,
    pub overline: f32,
    pub heading: f32,
    /// Buttons are fully round (iOS) instead of rounded rectangles (Drishti).
    pub capsule_buttons: bool,
    /// Cards have a hairline border (iOS); Drishti steps up one surface instead.
    pub card_border: bool,
    /// Dialogs and menus cast the design system's sheet shadow.
    pub sheet_shadow: bool,
    /// Status is shown as a tinted chip with a dot and a word.
    pub state_chips: bool,
    /// The download list sits on a rounded surface above the page ground.
    pub floating_list: bool,
    pub icon_stroke: f32,
    /// Switch track width and height.
    pub switch: (f32, f32),
}

impl Design {
    pub const fn of(style: ThemeStyle) -> Design {
        match style {
            ThemeStyle::Drishti => Design {
                style,
                radius_sm: 8,
                radius_md: 10,
                radius_lg: 14,
                radius_xl: 20,
                control_h: 36.0,
                widget_h: 36.0,
                row_h: 48.0,
                sidebar_w: 220.0,
                nav_h: 32.0,
                inset: 16.0,
                body: 14.0,
                caption: 12.0,
                overline: 11.0,
                heading: 17.0,
                capsule_buttons: false,
                card_border: false,
                sheet_shadow: true,
                state_chips: true,
                floating_list: true,
                icon_stroke: 1.75,
                switch: (44.0, 26.0),
            },
            ThemeStyle::Ios => Design {
                style,
                radius_sm: 8,
                radius_md: 10,
                radius_lg: 12,
                radius_xl: 14,
                control_h: 30.0,
                widget_h: 28.0,
                row_h: 34.0,
                sidebar_w: 200.0,
                nav_h: 28.0,
                inset: 14.0,
                body: 13.0,
                caption: 11.0,
                overline: 11.0,
                heading: 17.0,
                capsule_buttons: true,
                card_border: true,
                sheet_shadow: false,
                state_chips: false,
                floating_list: false,
                icon_stroke: 1.5,
                switch: (51.0, 31.0),
            },
        }
    }
}

/// Seconds the colours take to cross-fade when the theme changes.
const FADE_SECS: f32 = 0.2;

fn palette_id() -> Id {
    Id::unique("sdm_palette")
}

fn design_id() -> Id {
    Id::unique("sdm_design")
}

/// The palette being shown right now (blended while the theme is fading).
pub fn current(ctx: &egui::Context) -> Palette {
    ctx.data(|d| d.get_temp::<Palette>(palette_id())).unwrap_or(Palette::DRISHTI_DARK)
}

/// The sizes and rules of the design in use.
pub fn design(ctx: &egui::Context) -> Design {
    ctx.data(|d| d.get_temp::<Design>(design_id())).unwrap_or(Design::of(ThemeStyle::Drishti))
}

/// The hairline between list rows and around cards: Drishti's translucent `separator` token, or the
/// softened iOS separator.
pub fn hairline(ctx: &egui::Context) -> Color32 {
    let (p, d) = (current(ctx), design(ctx));
    if d.style == ThemeStyle::Drishti { p.separator } else { p.separator.gamma_multiply(0.7) }
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
pub fn apply(ctx: &egui::Context, mode: ThemeMode, style: ThemeStyle) {
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
    let (light, dark_pal) = style.palettes();
    let pal = light.lerp(&dark_pal, t);
    let design = Design::of(style);
    let applied_id = Id::unique("sdm_applied");
    if ctx.data(|d| d.get_temp::<(Palette, Design)>(applied_id)) == Some((pal, design)) {
        return;
    }
    ctx.data_mut(|d| {
        d.insert_temp(palette_id(), pal);
        d.insert_temp(design_id(), design);
        d.insert_temp(applied_id, (pal, design));
    });
    ctx.all_styles_mut(|s| {
        layout_style(s, &design);
        color_style(s, &pal, &design, t > 0.5);
    });
}

/// Spacing, type scale, radii and shadows of the design.
fn layout_style(style: &mut egui::Style, d: &Design) {
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(if d.capsule_buttons { 14.0 } else { 16.0 }, 6.0);
    style.spacing.interact_size.y = d.widget_h;
    style.spacing.window_margin = egui::Margin::same(20);
    style.spacing.menu_margin = egui::Margin::same(6);
    style.text_styles.insert(TextStyle::Body, FontId::proportional(d.body));
    style.text_styles.insert(TextStyle::Button, FontId::proportional(d.body));
    style.text_styles.insert(TextStyle::Small, FontId::proportional(d.caption));
    style.text_styles.insert(TextStyle::Monospace, FontId::monospace(d.caption));
    style.text_styles.insert(TextStyle::Heading, FontId::proportional(d.heading));
    let v = &mut style.visuals;
    v.striped = false;
    v.window_corner_radius = CornerRadius::same(d.radius_xl);
    v.menu_corner_radius = CornerRadius::same(d.radius_md);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(d.radius_md.min(if d.capsule_buttons { 8 } else { 10 }));
    }
}

/// Colours of every egui widget from the palette.
fn color_style(style: &mut egui::Style, p: &Palette, d: &Design, dark: bool) {
    let drishti = d.style == ThemeStyle::Drishti;
    let v = &mut style.visuals;
    v.dark_mode = dark;
    v.panel_fill = p.bg;
    v.window_fill = p.raised;
    v.window_stroke = Stroke::new(1.0, if drishti { p.separator_strong } else { p.separator });
    v.extreme_bg_color = if drishti { p.surface2 } else { p.bg };
    v.faint_bg_color = p.bg;
    v.code_bg_color = p.bg;
    v.hyperlink_color = p.blue_ink;
    v.warn_fg_color = p.orange_ink;
    v.error_fg_color = p.red_ink;
    v.weak_text_color = Some(p.tertiary);
    v.selection.bg_fill = if drishti { p.blue_tint } else { p.blue.gamma_multiply(0.28) };
    v.selection.stroke = Stroke::new(if drishti { 2.0 } else { 1.0 }, p.blue);
    v.slider_trailing_fill = true;
    if d.sheet_shadow {
        let shade = |a: u8| Color32::from_black_alpha(if dark { a } else { a / 3 });
        v.window_shadow = Shadow { offset: [0, 12], blur: 40, spread: 0, color: shade(140) };
        v.popup_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: shade(120) };
    } else {
        v.window_shadow = Shadow::NONE;
        v.popup_shadow = Shadow::NONE;
    }
    v.widgets.noninteractive.bg_fill = p.card;
    v.widgets.noninteractive.weak_bg_fill = p.card;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.separator);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.label);
    if drishti {
        for (w, fill) in [(&mut v.widgets.inactive, p.surface2), (&mut v.widgets.hovered, p.surface3), (&mut v.widgets.active, p.surface3), (&mut v.widgets.open, p.surface3)] {
            w.bg_fill = fill;
            w.weak_bg_fill = fill;
            w.bg_stroke = Stroke::new(1.0, p.control_border);
            w.fg_stroke = Stroke::new(1.0, p.label);
            w.expansion = 0.0;
        }
    } else {
        let steps = [(&mut v.widgets.inactive, 0.35), (&mut v.widgets.hovered, 0.55), (&mut v.widgets.active, 0.75), (&mut v.widgets.open, 0.55)];
        for (w, k) in steps {
            w.bg_fill = p.separator.gamma_multiply(k);
            w.weak_bg_fill = p.separator.gamma_multiply(k);
            w.bg_stroke = Stroke::NONE;
            w.fg_stroke = Stroke::new(1.0, p.label);
            w.expansion = 0.0;
        }
    }
    // Window title bars are drawn with this fill; matching the dialog keeps them seamless.
    v.widgets.open.weak_bg_fill = p.raised;
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::epaint::Shadow;
    use eframe::egui::CornerRadius;

    fn frame(ctx: &egui::Context, mode: ThemeMode, style: ThemeStyle) {
        let mut out = ctx.run_ui(Default::default(), |ui| apply(ui.ctx(), mode, style));
        out.textures_delta.clear();
    }

    #[test]
    fn forced_modes_apply_their_palette_on_the_first_frame() {
        for style in [ThemeStyle::Drishti, ThemeStyle::Ios] {
            let (light, dark) = style.palettes();
            for (mode, want) in [(ThemeMode::Dark, dark), (ThemeMode::Light, light)] {
                let ctx = egui::Context::default();
                frame(&ctx, mode, style);
                assert_eq!(current(&ctx), want, "{style:?} {mode:?}");
            }
        }
    }

    #[test]
    fn switching_mode_blends_instead_of_jumping() {
        let ctx = egui::Context::default();
        let style = ThemeStyle::Drishti;
        frame(&ctx, ThemeMode::Light, style);
        frame(&ctx, ThemeMode::Dark, style); // egui returns the old value on the frame an animation starts
        frame(&ctx, ThemeMode::Dark, style);
        let mid = current(&ctx);
        assert_ne!(mid, Palette::DRISHTI_LIGHT);
        assert_ne!(mid, Palette::DRISHTI_DARK);
    }

    #[test]
    fn drishti_is_the_default_design() {
        assert_eq!(ThemeStyle::default(), ThemeStyle::Drishti);
    }

    #[test]
    fn ios_design_is_flat_and_compact() {
        let ctx = egui::Context::default();
        frame(&ctx, ThemeMode::Dark, ThemeStyle::Ios);
        let st = ctx.global_style();
        assert_eq!(st.visuals.window_shadow, Shadow::NONE);
        assert_eq!(st.visuals.popup_shadow, Shadow::NONE);
        assert!(!st.visuals.striped);
        assert_eq!(st.visuals.window_corner_radius, CornerRadius::same(14));
        assert_eq!(st.visuals.menu_corner_radius, CornerRadius::same(10));
        assert_eq!(st.text_styles[&TextStyle::Body].size, 13.0);
        assert_eq!(st.text_styles[&TextStyle::Small].size, 11.0);
        assert_eq!(st.text_styles[&TextStyle::Heading].size, 17.0);
        assert_eq!(st.spacing.interact_size.y, 28.0);
        assert_eq!(design(&ctx).style, ThemeStyle::Ios);
    }

    #[test]
    fn drishti_design_has_36px_controls_20px_sheets_and_a_sheet_shadow() {
        let ctx = egui::Context::default();
        frame(&ctx, ThemeMode::Dark, ThemeStyle::Drishti);
        let st = ctx.global_style();
        assert_eq!(st.visuals.window_corner_radius, CornerRadius::same(20));
        assert_ne!(st.visuals.window_shadow, Shadow::NONE, "dialogs cast the sheet shadow");
        assert!(!st.visuals.striped);
        assert_eq!(st.text_styles[&TextStyle::Body].size, 14.0);
        assert_eq!(st.text_styles[&TextStyle::Small].size, 12.0);
        assert_eq!(st.spacing.interact_size.y, 36.0);
        assert_eq!(st.visuals.window_fill, Palette::DRISHTI_DARK.raised);
        assert_eq!(st.visuals.panel_fill, Palette::DRISHTI_DARK.bg);
        assert_eq!(design(&ctx).style, ThemeStyle::Drishti);
    }

    #[test]
    fn changing_the_design_restyles_without_changing_the_mode() {
        let ctx = egui::Context::default();
        frame(&ctx, ThemeMode::Dark, ThemeStyle::Ios);
        assert_eq!(ctx.global_style().visuals.window_corner_radius, CornerRadius::same(14));
        frame(&ctx, ThemeMode::Dark, ThemeStyle::Drishti);
        assert_eq!(ctx.global_style().visuals.window_corner_radius, CornerRadius::same(20));
        assert_eq!(current(&ctx), Palette::DRISHTI_DARK);
        frame(&ctx, ThemeMode::Dark, ThemeStyle::Ios);
        assert_eq!(current(&ctx), Palette::IOS_DARK);
    }

    #[test]
    fn the_two_designs_really_differ() {
        let (a, b) = (Design::of(ThemeStyle::Drishti), Design::of(ThemeStyle::Ios));
        assert!(a.control_h > b.control_h && a.row_h > b.row_h && a.sidebar_w > b.sidebar_w);
        assert!(!a.capsule_buttons && b.capsule_buttons);
        assert!(!a.card_border && b.card_border);
        assert!(a.sheet_shadow && !b.sheet_shadow);
    }
}
