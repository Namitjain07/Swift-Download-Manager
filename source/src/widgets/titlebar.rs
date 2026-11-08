//! The title bar, drawn by the app instead of the system so that it has the colours, the corners and
//! the animation of the theme in use: the name of the app on the left, the minimise, maximise and
//! close buttons on the right, and the rest of the bar is the handle to drag the window by.
//!
//! * Windows and Linux: the system title bar is switched off (`viewport`) and these buttons take its place.
//! * macOS: the system keeps its round traffic lights; the window content reaches under them and the bar
//!   leaves room for them (`Mode::Overlay`).
//! * `--native-title-bar` switches back to the system's bar on every system (`Mode::Native`).
//!
//! Without the system's frame the window must also be resized by its edges: `edge_at` and `resize_by_edges`.

use super::anim::fade;
use crate::icons::{self, Icon};
use crate::theme;
use eframe::egui::{self, vec2, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Painter, Pos2, Rect, ResizeDirection, Sense, Stroke, StrokeKind, Ui, ViewportBuilder, ViewportCommand};
use std::sync::atomic::{AtomicBool, Ordering};

/// Who draws the title bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// This app: our own bar and buttons.
    Custom,
    /// macOS: our own bar, the system's traffic lights over its left end.
    Overlay,
    /// The system's bar; the app draws none.
    Native,
}

static NATIVE: AtomicBool = AtomicBool::new(false);

/// `--native-title-bar`: keep the title bar of the system.
pub fn use_native(on: bool) {
    NATIVE.store(on, Ordering::Relaxed);
}

pub fn mode() -> Mode {
    if NATIVE.load(Ordering::Relaxed) {
        Mode::Native
    } else if cfg!(target_os = "macos") {
        Mode::Overlay
    } else {
        Mode::Custom
    }
}

/// The window options for a mode: no system bar when the app draws its own.
pub fn viewport(builder: ViewportBuilder, mode: Mode) -> ViewportBuilder {
    match mode {
        Mode::Custom => builder.with_decorations(false),
        Mode::Overlay => builder.with_fullsize_content_view(true).with_title_shown(false).with_titlebar_shown(false),
        Mode::Native => builder,
    }
}

/// Width of one caption button; the traffic lights of macOS need about 78 points on the left.
pub const CAPTION_W: f32 = 46.0;
const TRAFFIC_LIGHTS_W: f32 = 78.0;
/// How far inside the window an edge still starts a resize.
pub const EDGE: f32 = 5.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Caption {
    Minimize,
    Maximize,
    Close,
}

impl Caption {
    fn label(self, maximized: bool) -> &'static str {
        match self {
            Caption::Minimize => "Minimize",
            Caption::Maximize if maximized => "Restore",
            Caption::Maximize => "Maximize",
            Caption::Close => "Close",
        }
    }
}

/// The title bar. Call it first in the window, before the header.
pub fn show(ui: &mut Ui) {
    let mode = mode();
    if mode == Mode::Native {
        return;
    }
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let (maximized, fullscreen) = ctx.input(|i| (i.viewport().maximized.unwrap_or(false), i.viewport().fullscreen.unwrap_or(false)));
    if fullscreen {
        return;
    }
    egui::Panel::top("titlebar")
        .frame(egui::Frame::NONE.fill(pal.chrome))
        .show_separator_line(false)
        .resizable(false)
        .exact_size(d.titlebar)
        .show(ui, |ui| {
            let bar = ui.max_rect();
            // The bar itself: drag to move, double-click to maximise or restore.
            let handle = ui.interact(bar, Id::unique("titlebar_handle"), Sense::click_and_drag());
            if handle.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if handle.double_clicked() && mode == Mode::Custom {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
            // The name of the app, with its mark.
            let left = bar.left() + if mode == Mode::Overlay { TRAFFIC_LIGHTS_W } else { 12.0 };
            let mark = Rect::from_center_size(Pos2::new(left + 9.0, bar.center().y), vec2(18.0, 18.0));
            ui.painter().rect_filled(mark, CornerRadius::same(d.radius_sm.min(5)), pal.accent_solid);
            icons::draw_weight(ui.painter(), Rect::from_center_size(mark.center(), vec2(12.0, 12.0)), Icon::Download, pal.on_accent, true, false);
            ui.painter().text(Pos2::new(mark.right() + 9.0, bar.center().y), Align2::LEFT_CENTER, crate::app::APP_NAME, FontId::new(d.caption + 0.5, theme::semibold()), pal.label);
            if mode == Mode::Custom {
                let mut x = bar.right();
                for kind in [Caption::Close, Caption::Maximize, Caption::Minimize] {
                    x -= CAPTION_W;
                    let cell = Rect::from_min_size(Pos2::new(x, bar.top()), vec2(CAPTION_W, bar.height()));
                    if caption_button(ui, cell, kind, maximized) {
                        ctx.send_viewport_cmd(match kind {
                            Caption::Minimize => ViewportCommand::Minimized(true),
                            Caption::Maximize => ViewportCommand::Maximized(!maximized),
                            Caption::Close => ViewportCommand::Close,
                        });
                    }
                }
            }
        });
}

/// One of the three buttons. Square and flush with the corner in the sharp shapes, a capsule in the
/// round one; the close button turns red under the pointer, as everywhere.
fn caption_button(ui: &mut Ui, cell: Rect, kind: Caption, maximized: bool) -> bool {
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let resp = ui.interact(cell, Id::unique("caption").with(kind as u8), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, kind.label(maximized)));
    let hover = fade(&ctx, resp.id.with("hover"), resp.hovered(), 0.12);
    let down = fade(&ctx, resp.id.with("down"), resp.is_pointer_button_down_on(), 0.06);
    let inset = vec2(4.0, 4.0) * d.pill;
    let face = cell.shrink2(inset);
    // Windows 11 caption buttons are square and flush with the corner of the window.
    let radius = if d.is_winui() { CornerRadius::ZERO } else { CornerRadius::same(d.round(face.height()).min(14)) };
    let red = Color32::from_rgb(0xC4, 0x2B, 0x1C);
    let (fill, ink) = if kind == Caption::Close {
        (red.gamma_multiply(0.85 + 0.15 * down), pal.label.lerp_to_gamma(Color32::WHITE, hover))
    } else {
        (if d.is_winui() { pal.hover.lerp_to_gamma(pal.press, down) } else { pal.press.gamma_multiply(0.5 + 0.5 * down) }, pal.label)
    };
    ui.painter().rect_filled(face, radius, fill.gamma_multiply(hover));
    glyph(ui.painter(), face.center(), kind, maximized, ink, ctx.pixels_per_point());
    resp.clicked()
}

/// A hair-line picture of the button, snapped to whole pixels so that it stays sharp.
fn glyph(p: &Painter, centre: Pos2, kind: Caption, maximized: bool, ink: Color32, ppp: f32) {
    let snap = |v: f32| ((v * ppp).floor() + 0.5) / ppp;
    let c = Pos2::new(snap(centre.x), snap(centre.y));
    let stroke = Stroke::new((1.0 / ppp).max(1.0), ink);
    let r = 5.0;
    match kind {
        Caption::Minimize => {
            p.line_segment([Pos2::new(c.x - r, c.y), Pos2::new(c.x + r, c.y)], stroke);
        }
        Caption::Close => {
            p.line_segment([Pos2::new(c.x - r, c.y - r), Pos2::new(c.x + r, c.y + r)], stroke);
            p.line_segment([Pos2::new(c.x - r, c.y + r), Pos2::new(c.x + r, c.y - r)], stroke);
        }
        Caption::Maximize if !maximized => {
            p.rect_stroke(Rect::from_center_size(c, vec2(2.0 * r, 2.0 * r)), CornerRadius::same(1), stroke, StrokeKind::Middle);
        }
        Caption::Maximize => {
            // Restore: a window in front and the corner of the one behind it.
            let front = Rect::from_min_size(Pos2::new(c.x - r, c.y - r + 2.0), vec2(2.0 * r - 2.0, 2.0 * r - 2.0));
            p.rect_stroke(front, CornerRadius::same(1), stroke, StrokeKind::Middle);
            let (right, top) = (front.right() + 2.0, front.top() - 2.0);
            p.add(egui::Shape::line(vec![Pos2::new(front.left() + 2.0, top), Pos2::new(right, top), Pos2::new(right, front.bottom() - 2.0), Pos2::new(front.right() + 0.5, front.bottom() - 2.0)], stroke));
        }
    }
}

/// Which edge or corner of a window of `screen` the point `pos` is on, within `margin` of it.
pub fn edge_at(pos: Pos2, screen: Rect, margin: f32) -> Option<ResizeDirection> {
    if !screen.contains(pos) {
        return None;
    }
    let (left, right) = (pos.x - screen.left() < margin, screen.right() - pos.x < margin);
    let (top, bottom) = (pos.y - screen.top() < margin, screen.bottom() - pos.y < margin);
    Some(match (left, right, top, bottom) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (_, true, true, _) => ResizeDirection::NorthEast,
        (true, _, _, true) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::West,
        (_, true, ..) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (_, _, _, true) => ResizeDirection::South,
        _ => return None,
    })
}

fn cursor_for(dir: ResizeDirection) -> CursorIcon {
    match dir {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

/// A window without the system's frame has to resize itself: the pointer turns into a resize arrow on
/// the edges and a press there starts the resize. Also draws the thin outline that stands in for the
/// frame. Call it last in the frame.
pub fn frame_and_edges(ctx: &egui::Context) {
    let mode = mode();
    if mode != Mode::Custom {
        return;
    }
    let (maximized, fullscreen) = ctx.input(|i| (i.viewport().maximized.unwrap_or(false), i.viewport().fullscreen.unwrap_or(false)));
    if fullscreen {
        return;
    }
    let screen = ctx.content_rect();
    if !maximized {
        let pal = theme::current(ctx);
        ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::unique("window_outline"))).rect_stroke(screen, CornerRadius::ZERO, Stroke::new(1.0, pal.separator_strong), StrokeKind::Inside);
        if let Some(dir) = ctx.input(|i| i.pointer.latest_pos()).and_then(|pos| edge_at(pos, screen, EDGE)) {
            ctx.set_cursor_icon(cursor_for(dir));
            if ctx.input(|i| i.pointer.primary_pressed()) {
                ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::{themed, themed_with};
    use crate::theme::{ColorTheme, ThemeMode};

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))
    }

    #[test]
    fn the_eight_edges_and_corners_are_told_apart() {
        let s = screen();
        let at = |x: f32, y: f32| edge_at(Pos2::new(x, y), s, EDGE);
        assert_eq!(at(500.0, 2.0), Some(ResizeDirection::North));
        assert_eq!(at(500.0, 698.0), Some(ResizeDirection::South));
        assert_eq!(at(2.0, 300.0), Some(ResizeDirection::West));
        assert_eq!(at(998.0, 300.0), Some(ResizeDirection::East));
        assert_eq!(at(2.0, 2.0), Some(ResizeDirection::NorthWest));
        assert_eq!(at(998.0, 2.0), Some(ResizeDirection::NorthEast));
        assert_eq!(at(2.0, 698.0), Some(ResizeDirection::SouthWest));
        assert_eq!(at(998.0, 698.0), Some(ResizeDirection::SouthEast));
    }

    #[test]
    fn the_middle_of_the_window_and_points_outside_it_are_not_edges() {
        let s = screen();
        assert_eq!(edge_at(Pos2::new(500.0, 350.0), s, EDGE), None);
        assert_eq!(edge_at(Pos2::new(EDGE + 0.5, 350.0), s, EDGE), None, "just inside the margin");
        assert_eq!(edge_at(Pos2::new(-1.0, 350.0), s, EDGE), None, "outside the window");
        assert_eq!(edge_at(Pos2::new(1200.0, 20.0), s, EDGE), None);
    }

    #[test]
    fn every_direction_has_its_own_cursor() {
        use ResizeDirection::*;
        let all = [North, South, East, West, NorthEast, NorthWest, SouthEast, SouthWest];
        let cursors: std::collections::BTreeSet<_> = all.iter().map(|d| format!("{:?}", cursor_for(*d))).collect();
        assert_eq!(cursors.len(), 8);
    }

    #[test]
    fn the_modes_choose_who_draws_the_bar() {
        use_native(true);
        assert_eq!(mode(), Mode::Native);
        use_native(false);
        assert_eq!(mode(), if cfg!(target_os = "macos") { Mode::Overlay } else { Mode::Custom });
    }

    /// Runs the bar once in a window of `size` and returns the commands it sent.
    fn commands_after(ctx: &egui::Context, size: egui::Vec2, events: Vec<egui::Event>) -> Vec<ViewportCommand> {
        let mut out = ctx.run_ui(crate::theme::testing::input(size, events), show);
        out.textures_delta.clear();
        out.viewport_output.values().flat_map(|v| v.commands.clone()).collect()
    }

    #[test]
    fn the_three_buttons_minimise_maximise_and_close_the_window() {
        if mode() != Mode::Custom {
            return;
        }
        let ctx = themed(ThemeMode::Light);
        let size = vec2(900.0, 600.0);
        let d = theme::design(&ctx);
        let button = |from_right: f32| Pos2::new(size.x - CAPTION_W * from_right + CAPTION_W / 2.0, d.titlebar / 2.0);
        for (at, wanted) in [(button(3.0), "Minimized(true)"), (button(2.0), "Maximized(true)"), (button(1.0), "Close")] {
            let mut sent = Vec::new();
            let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
            commands_after(&ctx, size, vec![]);
            commands_after(&ctx, size, vec![egui::Event::PointerMoved(at)]);
            sent.extend(commands_after(&ctx, size, vec![button(true)]));
            sent.extend(commands_after(&ctx, size, vec![button(false)]));
            let names: Vec<String> = sent.iter().map(|c| format!("{c:?}")).collect();
            assert!(names.iter().any(|n| n == wanted), "pressing at {at:?} should send {wanted}, sent {names:?}");
        }
    }

    #[test]
    fn dragging_the_bar_moves_the_window_and_a_double_click_maximises_it() {
        if mode() != Mode::Custom {
            return;
        }
        let ctx = themed(ThemeMode::Light);
        let size = vec2(900.0, 600.0);
        let at = Pos2::new(400.0, theme::design(&ctx).titlebar / 2.0);
        let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let moved = |to: Pos2| egui::Event::PointerMoved(to);
        commands_after(&ctx, size, vec![]);
        commands_after(&ctx, size, vec![moved(at)]);
        commands_after(&ctx, size, vec![ev(true)]);
        let sent = commands_after(&ctx, size, vec![moved(at + vec2(30.0, 6.0))]);
        assert!(sent.iter().any(|c| matches!(c, ViewportCommand::StartDrag)), "dragging sends StartDrag: {sent:?}");

        let ctx = themed(ThemeMode::Light);
        let mut sent = Vec::new();
        commands_after(&ctx, size, vec![]);
        commands_after(&ctx, size, vec![moved(at)]);
        for _ in 0..2 {
            commands_after(&ctx, size, vec![ev(true)]);
            sent.extend(commands_after(&ctx, size, vec![ev(false)]));
        }
        assert!(sent.iter().any(|c| matches!(c, ViewportCommand::Maximized(true))), "a double click maximises: {sent:?}");
    }

    #[test]
    fn a_click_on_the_bar_away_from_the_buttons_closes_nothing() {
        if mode() != Mode::Custom {
            return;
        }
        let ctx = themed(ThemeMode::Light);
        let sent = {
            let mut all = Vec::new();
            let at = Pos2::new(300.0, 10.0);
            let size = vec2(900.0, 600.0);
            let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
            commands_after(&ctx, size, vec![]);
            commands_after(&ctx, size, vec![egui::Event::PointerMoved(at)]);
            all.extend(commands_after(&ctx, size, vec![button(true)]));
            all.extend(commands_after(&ctx, size, vec![button(false)]));
            all
        };
        assert!(!sent.iter().any(|c| matches!(c, ViewportCommand::Close | ViewportCommand::Minimized(_))), "{sent:?}");
    }

    #[test]
    fn the_bar_is_as_tall_as_the_theme_says_and_fills_the_width() {
        if mode() != Mode::Custom {
            return;
        }
        for theme in ColorTheme::ALL {
            let ctx = themed_with(ThemeMode::Light, theme);
            let (mut rect, mut seen) = (Rect::NOTHING, false);
            for _ in 0..3 {
                let mut out = ctx.run_ui(crate::theme::testing::input(vec2(900.0, 600.0), vec![]), |ui| {
                    show(ui);
                    rect = ui.ctx().content_rect().with_max_y(ui.cursor().min.y);
                    seen = true;
                });
                out.textures_delta.clear();
            }
            assert!(seen);
            let want = theme.shape().design().titlebar;
            assert!((rect.height() - want).abs() < 1.0, "{} should have a {want} tall bar, has {}", theme.name(), rect.height());
        }
    }

    #[test]
    fn the_glyphs_draw_in_every_state_without_panicking() {
        let ctx = themed(ThemeMode::Dark);
        let mut out = ctx.run_ui(crate::theme::testing::input(vec2(200.0, 100.0), vec![]), |ui| {
            for ppp in [1.0, 1.25, 1.5, 2.0] {
                for kind in [Caption::Minimize, Caption::Maximize, Caption::Close] {
                    for maximized in [false, true] {
                        glyph(ui.painter(), Pos2::new(50.0, 50.0), kind, maximized, Color32::WHITE, ppp);
                    }
                }
            }
        });
        out.textures_delta.clear();
        assert!(!out.shapes.is_empty());
    }
}
