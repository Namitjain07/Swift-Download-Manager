//! The look of the app: one design, light and dark.
//!
//! - `Palette` (see `palette.rs`) holds the colours.
//! - `Design` holds the sizes that every widget shares: radii, control and row heights, the type scale.
//! - `apply` is called once per frame. It fades between light and dark, and restyles egui only
//!   while the palette changes, so an idle window costs nothing.
//! - `install_fonts` loads Inter for text and Phosphor for icons (both bundled, so the app looks
//!   the same on every system).

pub mod palette;

use eframe::egui::{
    self, epaint::Shadow, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Id, Stroke, TextStyle, Theme, ThemePreference,
};
pub use palette::Palette;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Which theme the person wants. `System` follows the operating system live.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// Sizes that every widget reads instead of hard-coding numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Design {
    /// Tiles, thumbs, small chips.
    pub radius_sm: u8,
    /// Inputs, menu rows, tool tiles.
    pub radius_md: u8,
    /// Cards and grouped lists.
    pub radius_lg: u8,
    /// Dialogs.
    pub radius_xl: u8,
    /// Height of buttons and inputs.
    pub control_h: f32,
    /// Height of small buttons and chips-with-actions.
    pub small_h: f32,
    /// Height of a table row.
    pub row_h: f32,
    /// Width of the sidebar.
    pub sidebar_w: f32,
    /// Height of a sidebar row.
    pub nav_h: f32,
    /// Height of a menu row.
    pub menu_row_h: f32,
    /// Inner padding of cards, rows and dialogs.
    pub inset: f32,
    pub body: f32,
    pub caption: f32,
    pub overline: f32,
    pub title: f32,
    pub heading: f32,
    /// Switch track width and height (the iOS switch).
    pub switch: (f32, f32),
}

/// The one design.
pub const DESIGN: Design = Design {
    radius_sm: 8,
    radius_md: 12,
    radius_lg: 16,
    radius_xl: 24,
    control_h: 40.0,
    small_h: 32.0,
    row_h: 60.0,
    sidebar_w: 248.0,
    nav_h: 40.0,
    menu_row_h: 38.0,
    inset: 20.0,
    body: 14.0,
    caption: 12.0,
    overline: 11.0,
    title: 17.0,
    heading: 24.0,
    switch: (51.0, 31.0),
};

/// How high a surface floats above the page, and so how big its shadow is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    /// A card lying on the page.
    Rest,
    /// A card under the pointer, or a selected one.
    Hover,
    /// Menus, pickers and toasts.
    Popup,
    /// Dialogs.
    Dialog,
}

// ---------------------------------------------------------------- fonts

const INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold.ttf");
const PHOSPHOR: &[u8] = include_bytes!("../../assets/fonts/Phosphor.ttf");
const PHOSPHOR_FILL: &[u8] = include_bytes!("../../assets/fonts/Phosphor-Fill.ttf");

/// The family for text in the semibold weight: titles, button labels, numbers.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// The line-icon font (Phosphor Regular).
pub fn icon_font() -> FontFamily {
    FontFamily::Name("icons".into())
}

/// The solid-icon font (Phosphor Fill).
pub fn icon_fill_font() -> FontFamily {
    FontFamily::Name("icons-fill".into())
}

/// Loads the bundled fonts once per context. Safe to call every frame.
pub fn install_fonts(ctx: &egui::Context) {
    let flag = Id::unique("sdm_fonts_installed");
    if ctx.data(|d| d.get_temp::<bool>(flag)).unwrap_or(false) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(flag, true));
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [("inter", INTER_REGULAR), ("inter-semibold", INTER_SEMIBOLD), ("phosphor", PHOSPHOR), ("phosphor-fill", PHOSPHOR_FILL)] {
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }
    // Inter first; egui's own fonts stay behind it as fallbacks (emoji, other scripts).
    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    prop.insert(0, "inter".to_owned());
    let mut bold_chain = prop.clone();
    bold_chain.insert(0, "inter-semibold".to_owned());
    fonts.families.insert(semibold(), bold_chain);
    fonts.families.insert(icon_font(), vec!["phosphor".to_owned()]);
    fonts.families.insert(icon_fill_font(), vec!["phosphor-fill".to_owned()]);
    ctx.set_fonts(fonts);
}

// ---------------------------------------------------------------- palette access

fn palette_id() -> Id {
    Id::unique("sdm_palette")
}

/// The palette being shown right now (blended while the theme is fading).
pub fn current(ctx: &egui::Context) -> Palette {
    ctx.data(|d| d.get_temp::<Palette>(palette_id())).unwrap_or(Palette::LIGHT)
}

/// The sizes of the design.
pub fn design(_ctx: &egui::Context) -> Design {
    DESIGN
}

/// The hairline between list rows and around cards.
pub fn hairline(ctx: &egui::Context) -> Color32 {
    current(ctx).separator
}

/// A drop shadow of the given height for the current mode.
pub fn shadow(ctx: &egui::Context, level: Elevation) -> Shadow {
    let dark = ctx.global_style().visuals.dark_mode;
    let (offset, blur, spread, light_a, dark_a) = match level {
        Elevation::Rest => ([0, 1], 4, 0, 18, 70),
        Elevation::Hover => ([0, 6], 18, 0, 34, 110),
        Elevation::Popup => ([0, 4], 24, 0, 40, 130),
        Elevation::Dialog => ([0, 10], 34, 0, 72, 170),
    };
    Shadow { offset, blur, spread, color: Color32::from_black_alpha(if dark { dark_a } else { light_a }) }
}

/// Pick light or dark from the mode (and the operating system for `System`).
fn wants_dark(ctx: &egui::Context, mode: ThemeMode) -> bool {
    match mode {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::System => ctx.system_theme() == Some(Theme::Dark),
    }
}

/// Seconds the colours take to cross-fade when the theme changes.
const FADE_SECS: f32 = 0.2;

/// Call once per frame. Fades towards the wanted palette and writes it into egui's style
/// only while something changes, so an idle window costs nothing.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    install_fonts(ctx);
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
    let applied_id = Id::unique("sdm_applied");
    if ctx.data(|d| d.get_temp::<Palette>(applied_id)) == Some(pal) {
        return;
    }
    ctx.data_mut(|d| {
        d.insert_temp(palette_id(), pal);
        d.insert_temp(applied_id, pal);
    });
    ctx.all_styles_mut(|s| {
        layout_style(s);
        color_style(s, &pal, t > 0.5);
    });
}

/// Spacing, type scale and radii.
fn layout_style(style: &mut egui::Style) {
    let d = DESIGN;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(16.0, 8.0);
    style.spacing.interact_size = egui::vec2(40.0, d.control_h);
    style.spacing.window_margin = egui::Margin::same(24);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.menu_width = 260.0;
    style.spacing.icon_width = 22.0;
    style.spacing.icon_width_inner = 12.0;
    style.spacing.icon_spacing = 8.0;
    style.spacing.combo_height = 320.0;
    style.spacing.tooltip_width = 360.0;
    style.spacing.scroll = egui::style::ScrollStyle::floating();
    style.text_styles.insert(TextStyle::Body, FontId::new(d.body, FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Button, FontId::new(d.body, semibold()));
    style.text_styles.insert(TextStyle::Small, FontId::new(d.caption, FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace));
    style.text_styles.insert(TextStyle::Heading, FontId::new(d.heading, semibold()));
    let v = &mut style.visuals;
    v.striped = false;
    v.window_corner_radius = CornerRadius::same(d.radius_xl);
    v.menu_corner_radius = CornerRadius::same(d.radius_lg);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(d.radius_md);
        w.expansion = 0.0;
    }
}

/// Colours of every egui widget, from the palette.
fn color_style(style: &mut egui::Style, p: &Palette, dark: bool) {
    let v = &mut style.visuals;
    v.dark_mode = dark;
    v.panel_fill = p.bg;
    v.window_fill = p.raised;
    v.window_stroke = Stroke::new(1.0, if dark { p.separator_strong } else { p.separator });
    v.extreme_bg_color = p.card;
    v.text_edit_bg_color = Some(p.card);
    v.faint_bg_color = p.bg;
    v.code_bg_color = p.surface2;
    v.hyperlink_color = p.blue_ink;
    v.warn_fg_color = p.orange_ink;
    v.error_fg_color = p.red_ink;
    v.weak_text_color = Some(p.tertiary);
    // Text selection, and the 2 px ring around the field being typed in.
    v.selection.bg_fill = p.accent_tint;
    v.selection.stroke = Stroke::new(2.0, p.label);
    v.slider_trailing_fill = true;
    let shadow_for = |level| {
        let mut s = shadow_level(level, dark);
        s.color = Color32::from_rgba_unmultiplied(p.shadow.r(), p.shadow.g(), p.shadow.b(), s.color.a());
        s
    };
    v.window_shadow = shadow_for(Elevation::Dialog);
    v.popup_shadow = shadow_for(Elevation::Popup);

    v.widgets.noninteractive.bg_fill = p.card;
    v.widgets.noninteractive.weak_bg_fill = p.card;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.separator);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.label);
    // Native egui widgets (text fields, menu rows, drop-downs that are not ours) read these.
    // Fields rest on a visible border, darken it on hover and take the 2 px ring while focused.
    v.widgets.inactive.bg_fill = p.surface2;
    v.widgets.inactive.weak_bg_fill = p.surface2;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.control_border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.label);
    v.widgets.hovered.bg_fill = p.surface3;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.label);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.label);
    v.widgets.active.bg_fill = p.surface3;
    v.widgets.active.weak_bg_fill = p.press;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.label);
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.label);
    v.widgets.open.bg_fill = p.surface3;
    v.widgets.open.weak_bg_fill = p.hover;
    v.widgets.open.bg_stroke = Stroke::new(1.0, p.label);
    v.widgets.open.fg_stroke = Stroke::new(1.0, p.label);
}

fn shadow_level(level: Elevation, dark: bool) -> Shadow {
    let (offset, blur, light_a, dark_a) = match level {
        Elevation::Rest => ([0, 1], 4, 18, 70),
        Elevation::Hover => ([0, 6], 18, 34, 110),
        Elevation::Popup => ([0, 4], 24, 40, 130),
        Elevation::Dialog => ([0, 10], 34, 72, 170),
    };
    Shadow { offset, blur, spread: 0, color: Color32::from_black_alpha(if dark { dark_a } else { light_a }) }
}

/// Helpers shared by the tests of every module.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// Runs one frame that only applies the theme.
    pub fn frame(ctx: &egui::Context, mode: ThemeMode) {
        let mut out = ctx.run_ui(Default::default(), |ui| apply(ui.ctx(), mode));
        out.textures_delta.clear();
    }

    /// A context with the bundled fonts and the theme applied, as the app has it.
    pub fn themed(mode: ThemeMode) -> egui::Context {
        let ctx = egui::Context::default();
        frame(&ctx, mode);
        ctx
    }

    /// Input for one frame on a screen of `size` carrying `events`.
    pub fn input(size: egui::Vec2, events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)), events, ..Default::default() }
    }

    /// Runs one frame of `add` in the central panel. Texture changes are cleared (egui insists they are handled).
    pub fn run(ctx: &egui::Context, size: egui::Vec2, events: Vec<egui::Event>, mut add: impl FnMut(&mut egui::Ui)) {
        let mut out = ctx.run_ui(input(size, events), |ui| {
            egui::CentralPanel::default().show(ui, |ui| add(ui));
        });
        out.textures_delta.clear();
    }

    /// Lays `add` out once, then moves the pointer to `at`, presses and releases the primary
    /// button there: what a click does, frame by frame.
    pub fn click(ctx: &egui::Context, size: egui::Vec2, at: egui::Pos2, mut add: impl FnMut(&mut egui::Ui)) {
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        run(ctx, size, vec![], &mut add);
        run(ctx, size, vec![egui::Event::PointerMoved(at)], &mut add);
        run(ctx, size, vec![button(true)], &mut add);
        run(ctx, size, vec![button(false)], &mut add);
    }

    /// Presses and releases a key.
    pub fn key(ctx: &egui::Context, size: egui::Vec2, key: egui::Key, mut add: impl FnMut(&mut egui::Ui)) {
        let event = |pressed| egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: egui::Modifiers::NONE };
        run(ctx, size, vec![], &mut add);
        run(ctx, size, vec![event(true)], &mut add);
        run(ctx, size, vec![event(false)], &mut add);
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{frame, themed};
    use super::*;

    #[test]
    fn forced_modes_apply_their_palette_on_the_first_frame() {
        for (mode, want) in [(ThemeMode::Dark, Palette::DARK), (ThemeMode::Light, Palette::LIGHT)] {
            let ctx = themed(mode);
            assert_eq!(current(&ctx), want, "{mode:?}");
        }
    }

    #[test]
    fn switching_mode_blends_instead_of_jumping() {
        let ctx = egui::Context::default();
        frame(&ctx, ThemeMode::Light);
        frame(&ctx, ThemeMode::Dark); // egui returns the old value on the frame an animation starts
        frame(&ctx, ThemeMode::Dark);
        let mid = current(&ctx);
        assert_ne!(mid, Palette::LIGHT);
        assert_ne!(mid, Palette::DARK);
    }

    #[test]
    fn the_design_is_pill_and_card_shaped_and_roomy() {
        let ctx = themed(ThemeMode::Light);
        let st = ctx.global_style();
        assert_eq!(st.visuals.window_corner_radius, CornerRadius::same(24), "dialogs have 24 px corners");
        assert_eq!(st.visuals.menu_corner_radius, CornerRadius::same(16));
        assert_ne!(st.visuals.window_shadow, Shadow::NONE, "dialogs float on a soft shadow");
        assert!(!st.visuals.striped);
        assert_eq!(st.text_styles[&TextStyle::Body].size, 14.0);
        assert_eq!(st.text_styles[&TextStyle::Small].size, 12.0);
        assert_eq!(st.spacing.interact_size.y, 40.0);
        assert_eq!(st.visuals.window_fill, Palette::LIGHT.raised);
        assert_eq!(st.visuals.panel_fill, Palette::LIGHT.bg);
        assert_eq!(design(&ctx), DESIGN);
    }

    #[test]
    fn the_bundled_fonts_are_installed_exactly_once() {
        let ctx = themed(ThemeMode::Light);
        let defs = ctx.fonts(|f| f.definitions().clone());
        for name in ["inter", "inter-semibold", "phosphor", "phosphor-fill"] {
            assert!(defs.font_data.contains_key(name), "{name} is missing");
        }
        for family in [FontFamily::Proportional, semibold(), icon_font(), icon_fill_font()] {
            assert!(!defs.families[&family].is_empty(), "{family:?} has no fonts");
        }
        assert_eq!(defs.families[&FontFamily::Proportional][0], "inter", "Inter leads the proportional family");
        assert_eq!(defs.families[&semibold()][0], "inter-semibold");
        // Calling it again changes nothing.
        install_fonts(&ctx);
        assert_eq!(ctx.fonts(|f| f.definitions().clone()), defs);
    }

    #[test]
    fn shadows_grow_with_elevation_and_are_stronger_in_the_dark() {
        let light = themed(ThemeMode::Light);
        let dark = themed(ThemeMode::Dark);
        let levels = [Elevation::Rest, Elevation::Hover, Elevation::Popup, Elevation::Dialog];
        for pair in levels.windows(2) {
            assert!(shadow(&light, pair[0]).blur <= shadow(&light, pair[1]).blur, "{:?} then {:?}", pair[0], pair[1]);
        }
        for level in levels {
            assert!(shadow(&dark, level).color.a() > shadow(&light, level).color.a());
        }
    }
}
