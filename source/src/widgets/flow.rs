//! The picture of a download: several channels (connections, parts of the file, networks) that run
//! into one file. Each channel is a dot on the left joined to the file by a smooth curve; while data
//! arrives, small packets travel along the curves, faster on the faster channels, and the file fills
//! up from the bottom. When it is finished the curves fade out and the file turns green with a tick.
//! Flat colours only, everything is drawn with the painter, so it follows the theme.

use super::anim::{approach, fade, repaint_soon, smooth};
use crate::icons::{self, Icon};
use crate::theme;
use eframe::egui::{self, vec2, CornerRadius, Id, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui};

/// More channels than this are drawn as an even sample: the picture stays readable.
pub const MAX_CHANNELS: usize = 8;
/// Packets travelling on one curve at a time.
const PACKETS: usize = 3;
/// Points of a drawn curve.
const STEPS: usize = 28;

/// One stream of data: a connection, a part of the file, a network.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channel {
    /// How busy it is compared with the busiest one: 0 (idle) to 1.
    pub rate: f32,
    /// Its part is finished.
    pub done: bool,
}

/// What the download is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Data is arriving.
    Running,
    /// Paused, queued or waiting: the picture holds still.
    Waiting,
    /// Everything has arrived.
    Complete,
    /// Stopped by an error.
    Failed,
}

/// At most `MAX_CHANNELS` of `all`, spread evenly over the list (so that the first and the last are both there).
pub fn pick(all: &[Channel]) -> Vec<Channel> {
    if all.len() <= MAX_CHANNELS {
        return all.to_vec();
    }
    (0..MAX_CHANNELS).map(|k| all[k * (all.len() - 1) / (MAX_CHANNELS - 1)]).collect()
}

/// What the picture remembers between frames to tell how fast each part of a file is arriving.
#[derive(Clone, Debug, Default)]
pub struct Rates {
    at: f64,
    done: Vec<u64>,
    speed: Vec<f32>,
}

impl Rates {
    /// Takes the state of every part (bytes received, finished or not) at `now` seconds and returns one
    /// channel per part, with its speed compared to the fastest part. A change in the number of parts
    /// (a part was split) starts the measuring afresh.
    pub fn update(&mut self, now: f64, parts: &[(u64, bool)]) -> Vec<Channel> {
        if self.done.len() != parts.len() {
            *self = Rates { at: now, done: parts.iter().map(|p| p.0).collect(), speed: vec![0.0; parts.len()] };
        }
        let dt = now - self.at;
        // Too soon after the last look the numbers are mostly noise: keep what is known.
        if dt >= 0.25 {
            for (i, (done, _)) in parts.iter().enumerate() {
                let instant = done.saturating_sub(self.done[i]) as f64 / dt;
                self.speed[i] = 0.6 * self.speed[i] + 0.4 * instant as f32;
                self.done[i] = *done;
            }
            self.at = now;
        }
        let fastest = self.speed.iter().copied().fold(0.0_f32, f32::max);
        parts.iter().zip(&self.speed).map(|((_, finished), s)| Channel { rate: if fastest > 0.0 && !finished { (s / fastest).clamp(0.0, 1.0) } else { 0.0 }, done: *finished }).collect()
    }
}

/// The point at `t` (0..=1) on the cubic Bézier curve through `p`.
pub fn bezier(p: [Pos2; 4], t: f32) -> Pos2 {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    Pos2::new(a * p[0].x + b * p[1].x + c * p[2].x + d * p[3].x, a * p[0].y + b * p[1].y + c * p[2].y + d * p[3].y)
}

/// The curve from a dot to the meeting point: it leaves level and arrives level, so that all of them
/// run into the same spot side by side.
pub fn curve(from: Pos2, to: Pos2) -> [Pos2; 4] {
    let dx = (to.x - from.x) * 0.55;
    [from, Pos2::new(from.x + dx, from.y), Pos2::new(to.x - dx, to.y), to]
}

/// Where the `n` dots go: one column, evenly spaced between the top and bottom of `rect`.
pub fn node_positions(rect: Rect, n: usize) -> Vec<Pos2> {
    (0..n).map(|i| Pos2::new(rect.left(), rect.top() + rect.height() * (i as f32 + 0.5) / n.max(1) as f32)).collect()
}

/// The radius of a dot when the dots are `spacing` apart: as big as 4.5, but never touching the next one.
pub fn node_radius(spacing: f32) -> f32 {
    (spacing / 2.0 - 1.5).clamp(2.5, 4.5)
}

/// How far along its curve packet `k` of a channel is: 0 to 1, then round again. `phase` is how many laps the
/// channel's packets have made so far (see `Motion`); the packets of one channel are a third of a lap apart.
pub fn lap(phase: f32, k: usize) -> f32 {
    (phase + k as f32 / PACKETS as f32).rem_euclid(1.0)
}

/// Where the packets of every channel are, kept from one frame to the next. A position is the sum of what each frame
/// moved it, at the pace the channel has just then. (Working it out afresh from the clock, as `time * speed`, makes
/// every packet jump whenever a speed changes, and the later in the session, the bigger the jump.)
#[derive(Clone, Debug)]
struct Motion {
    at: f64,
    phase: [f32; MAX_CHANNELS],
    pace: [f32; MAX_CHANNELS],
}

impl Motion {
    fn new(time: f64) -> Self {
        Motion { at: time, phase: std::array::from_fn(|i| (i as f32 * 0.381_966).fract()), pace: [0.0; MAX_CHANNELS] }
    }

    /// Move on to `time`. `targets` is the pace (laps per second) each channel is heading for; the pace eases
    /// there over about half a second. A long pause (a hidden window) counts as one short step, not a lot of laps.
    fn step(&mut self, time: f64, targets: &[f32]) {
        let dt = (time - self.at).clamp(0.0, 0.1);
        self.at = time;
        for (i, target) in targets.iter().take(MAX_CHANNELS).enumerate() {
            self.pace[i] = approach(self.pace[i], *target, dt, 0.5);
            self.phase[i] = (self.phase[i] + dt as f32 * self.pace[i]).rem_euclid(1.0);
        }
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A packet appears at the start of its curve and is taken in at the end: 0 at both ends.
pub fn packet_alpha(t: f32) -> f32 {
    smoothstep(0.0, 0.12, t) * (1.0 - smoothstep(0.78, 1.0, t))
}

/// The picture. `progress` is 0..=1 and fills the file; `height` is how tall the picture is.
pub fn flow(ui: &mut Ui, id_salt: impl std::hash::Hash + std::fmt::Debug, channels: &[Channel], progress: f32, state: State, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let ctx = ui.ctx().clone();
    let (pal, d) = (theme::current(&ctx), theme::design(&ctx));
    let id = Id::unique(("flow", id_salt));
    let time = ctx.input(|i| i.time);
    let progress = if progress.is_finite() { progress.clamp(0.0, 1.0) } else { 0.0 };
    let shown = smooth(&ctx, id.with("fill"), if state == State::Complete { 1.0 } else { progress }, 0.3);
    let done_k = fade(&ctx, id.with("done"), state == State::Complete, 0.4);
    let failed_k = fade(&ctx, id.with("failed"), state == State::Failed, 0.25);
    let channels = pick(channels);
    // Where the packets are: moved on from where they were, never worked out afresh (see `Motion`).
    let motion_key = id.with("motion");
    let mut motion = ctx.data(|d| d.get_temp::<Motion>(motion_key)).unwrap_or_else(|| Motion::new(time));
    if state == State::Running {
        let targets: Vec<f32> = channels.iter().map(|c| if c.done { 0.0 } else { 0.3 + 0.9 * c.rate.clamp(0.0, 1.0) }).collect();
        motion.step(time, &targets);
    } else {
        motion.at = time; // a pause is not a lap
    }
    ctx.data_mut(|d| d.insert_temp(motion_key, motion.clone()));
    let p = ui.painter().with_clip_rect(rect);

    // The file on the right, and the spot where all the curves meet.
    let side = (rect.height() - 24.0).clamp(36.0, 68.0);
    let tile = Rect::from_center_size(Pos2::new(rect.right() - 16.0 - side / 2.0, rect.center().y), vec2(side, side));
    let meet = Pos2::new(tile.left() - 2.0, tile.center().y);
    let strip = Rect::from_min_max(Pos2::new(rect.left() + 16.0, rect.top() + 14.0), Pos2::new(meet.x, rect.bottom() - 14.0));
    let nodes = node_positions(strip, channels.len());
    let node_r = node_radius(strip.height() / channels.len().max(1) as f32);

    // Curves, then the packets on them, then the dots.
    for (i, (ch, from)) in channels.iter().zip(&nodes).enumerate() {
        let path = curve(*from, meet);
        let live = state == State::Running && !ch.done;
        let line = if ch.done { pal.green.gamma_multiply(0.45) } else if live { pal.accent.gamma_multiply(0.38) } else { pal.separator_strong };
        let line = line.gamma_multiply(1.0 - 0.7 * done_k).gamma_multiply(1.0 - 0.4 * failed_k);
        let points: Vec<Pos2> = (0..=STEPS).map(|s| bezier(path, s as f32 / STEPS as f32)).collect();
        p.add(Shape::line(points, Stroke::new(1.5, line)));
        if live {
            let phase = motion.phase[i.min(MAX_CHANNELS - 1)];
            for k in 0..PACKETS {
                let t = lap(phase, k);
                let a = packet_alpha(t);
                if a > 0.01 {
                    let r = (2.2 + 1.0 * ch.rate.clamp(0.0, 1.0)) * (1.0 - 0.4 * smoothstep(0.78, 1.0, t));
                    p.circle_filled(bezier(path, t), r, pal.accent.gamma_multiply(a));
                }
            }
        }
    }
    for (ch, at) in channels.iter().zip(&nodes) {
        let (fill, ring) = if ch.done {
            (pal.green, pal.green)
        } else if state == State::Running {
            (pal.accent, pal.accent)
        } else {
            (pal.track, pal.control_border)
        };
        p.circle_filled(*at, node_r, pal.card);
        p.circle_stroke(*at, node_r, Stroke::new(1.5, ring.gamma_multiply(0.9)));
        if ch.done || state == State::Running {
            p.circle_filled(*at, node_r * 0.5, fill);
        }
    }

    // The file: a rounded square that fills from the bottom, with its glyph.
    let radius = CornerRadius::same(d.radius_md.max(4));
    p.rect_filled(tile, radius, pal.surface2);
    let fill_h = tile.height() * shown;
    if fill_h > 0.5 {
        let fill = Rect::from_min_max(Pos2::new(tile.left(), tile.bottom() - fill_h), tile.max);
        p.with_clip_rect(tile.intersect(rect)).rect_filled(fill, radius, if state == State::Complete { pal.green_tint } else { pal.accent_tint });
    }
    let border = pal.separator_strong.lerp_to_gamma(pal.accent, if state == State::Running { 0.8 } else { 0.0 }).lerp_to_gamma(pal.green, done_k).lerp_to_gamma(pal.red, failed_k);
    p.rect_stroke(tile, radius, Stroke::new(1.5, border), StrokeKind::Inside);
    let (glyph, tint) = match state {
        State::Complete if done_k > 0.5 => (Icon::Check, pal.green_ink),
        State::Failed => (Icon::WarningCircle, pal.red_ink),
        State::Waiting => (Icon::Download, pal.tertiary),
        _ => (Icon::Download, pal.accent_ink),
    };
    // A little bob while data arrives.
    let bob = if state == State::Running { (time * 4.0).sin() as f32 * 1.5 } else { 0.0 };
    icons::draw_weight(&p, Rect::from_center_size(tile.center() + vec2(0.0, bob), vec2(side * 0.46, side * 0.46)), glyph, tint, true, matches!(state, State::Complete | State::Failed));

    if state == State::Running {
        repaint_soon(&ctx);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, "Data flowing into the file"));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::testing::{input, themed, themed_with};
    use crate::theme::{ColorTheme, ThemeMode};

    fn ch(rate: f32) -> Channel {
        Channel { rate, done: false }
    }

    #[test]
    fn rates_compare_each_part_with_the_fastest_and_forget_nothing_between_frames() {
        let mut r = Rates::default();
        // First look: nothing is known yet, so nobody is faster than anybody.
        let first = r.update(10.0, &[(0, false), (0, false), (0, false)]);
        assert!(first.iter().all(|c| c.rate == 0.0 && !c.done), "{first:?}");
        // A second later part 0 got 1000 bytes, part 1 got 500 and part 2 nothing.
        let next = r.update(11.0, &[(1000, false), (500, false), (0, false)]);
        assert_eq!(next[0].rate, 1.0, "the fastest is 1: {next:?}");
        assert!((next[1].rate - 0.5).abs() < 0.01, "{next:?}");
        assert_eq!(next[2].rate, 0.0);
        // A look 50 ms later changes nothing (too soon to tell).
        let soon = r.update(11.05, &[(5000, false), (500, false), (0, false)]);
        assert_eq!(soon, next);
        // A part that finishes is marked done and no longer counts as running.
        let fin = r.update(12.0, &[(2000, true), (1000, false), (0, false)]);
        assert!(fin[0].done && fin[0].rate == 0.0, "{fin:?}");
        assert!(fin[1].rate > 0.0 && !fin[1].done);
    }

    #[test]
    fn a_change_in_the_number_of_parts_starts_the_measuring_again() {
        let mut r = Rates::default();
        r.update(0.0, &[(0, false), (0, false)]);
        r.update(1.0, &[(1000, false), (10, false)]);
        let split = r.update(2.0, &[(2000, false), (20, false), (0, false)]);
        assert_eq!(split.len(), 3);
        assert!(split.iter().all(|c| c.rate == 0.0), "after a split nothing is known: {split:?}");
        let later = r.update(3.0, &[(3000, false), (1020, false), (30, false)]);
        assert_eq!(later[1].rate, 1.0);
        // Bytes never go backwards into a negative speed.
        let back = r.update(4.0, &[(100, false), (1020, false), (30, false)]);
        assert!(back.iter().all(|c| (0.0..=1.0).contains(&c.rate)), "{back:?}");
    }

    #[test]
    fn a_curve_starts_and_ends_where_it_is_told_and_leaves_level() {
        let (a, b) = (Pos2::new(10.0, 20.0), Pos2::new(210.0, 80.0));
        let c = curve(a, b);
        assert_eq!(bezier(c, 0.0), a);
        assert_eq!(bezier(c, 1.0), b);
        // Level at both ends: the control points share the height of the end points.
        assert_eq!((c[1].y, c[2].y), (a.y, b.y));
        // Halfway is halfway down, and the curve never leaves the box of its end points.
        let mid = bezier(c, 0.5);
        assert!((mid.y - 50.0).abs() < 0.01 && (mid.x - 110.0).abs() < 0.01, "{mid:?}");
        for s in 0..=20 {
            let p = bezier(c, s as f32 / 20.0);
            assert!(p.x >= a.x - 0.01 && p.x <= b.x + 0.01 && p.y >= a.y - 0.01 && p.y <= b.y + 0.01, "{p:?}");
        }
    }

    #[test]
    fn the_dots_are_spread_evenly_down_one_column() {
        let r = Rect::from_min_max(Pos2::new(5.0, 10.0), Pos2::new(100.0, 90.0));
        let one = node_positions(r, 1);
        assert_eq!(one, vec![Pos2::new(5.0, 50.0)], "a single channel sits in the middle");
        let four = node_positions(r, 4);
        assert!(four.iter().all(|p| p.x == 5.0));
        let gaps: Vec<f32> = four.windows(2).map(|w| w[1].y - w[0].y).collect();
        assert!(gaps.iter().all(|g| (g - 20.0).abs() < 0.001), "{gaps:?}");
        assert!(four[0].y > r.top() && four[3].y < r.bottom());
        assert!(node_positions(r, 0).is_empty());
    }

    #[test]
    fn dots_shrink_to_stay_apart_and_never_vanish_or_grow_past_the_design() {
        assert_eq!(node_radius(100.0), 4.5, "plenty of room: full size");
        assert_eq!(node_radius(0.0), 2.5, "no room: still visible");
        // Whenever there is room for two smallest dots (2 x 2.5 + 1 gap), neighbours do not touch.
        for spacing in [8.0_f32, 9.0, 10.5, 12.0, 16.0, 30.0] {
            let r = node_radius(spacing);
            assert!(2.0 * r <= spacing + 0.001 || r == 2.5, "{spacing}: {r}");
            assert!((2.5..=4.5).contains(&r));
        }
        // With the most channels in the usual 112 high picture the dots still have a gap between them.
        let spacing = (112.0 - 28.0) / MAX_CHANNELS as f32;
        assert!(spacing - 2.0 * node_radius(spacing) >= 2.0, "{spacing}");
    }

    #[test]
    fn more_channels_than_fit_are_sampled_evenly_and_keep_the_first_and_the_last() {
        let few: Vec<Channel> = (0..5).map(|i| ch(i as f32 / 5.0)).collect();
        assert_eq!(pick(&few), few);
        let many: Vec<Channel> = (0..32).map(|i| ch(i as f32 / 31.0)).collect();
        let shown = pick(&many);
        assert_eq!(shown.len(), MAX_CHANNELS);
        assert_eq!(shown[0], many[0]);
        assert_eq!(shown[MAX_CHANNELS - 1], many[31]);
        assert!(shown.windows(2).all(|w| w[0].rate < w[1].rate), "still in order");
        assert!(pick(&[]).is_empty());
    }

    #[test]
    fn packets_are_spread_along_the_curve_and_always_inside_it() {
        for phase in [0.0, 0.2, 0.37, 0.999] {
            for k in 0..PACKETS {
                let t = lap(phase, k);
                assert!((0.0..1.0).contains(&t), "{t}");
            }
        }
        // The three packets of one channel are a third of a lap apart.
        let a: Vec<f32> = (0..PACKETS).map(|k| lap(0.0, k)).collect();
        let mut sorted = a.clone();
        sorted.sort_by(f32::total_cmp);
        assert!((sorted[1] - sorted[0] - 1.0 / 3.0).abs() < 0.001 && (sorted[2] - sorted[1] - 1.0 / 3.0).abs() < 0.001, "{a:?}");
    }

    #[test]
    fn packets_move_on_from_where_they_were_and_never_jump_when_a_speed_changes() {
        // The old way (position = time x speed) put the packets somewhere else whenever a speed changed: by
        // time x change, so after ten minutes a 1 % change moved them six laps. Now each frame adds a small step.
        let mut m = Motion::new(600.0);
        let slow = [0.3, 0.3, 0.3];
        let fast = [1.2, 0.3, 0.9];
        let mut time = 600.0;
        let mut worst = 0.0_f32;
        for frame in 0..400 {
            time += 0.016;
            let before = m.phase;
            m.step(time, if frame < 100 { &slow } else if frame < 250 { &fast } else { &slow });
            for i in 0..3 {
                let moved = (m.phase[i] - before[i]).rem_euclid(1.0);
                worst = worst.max(moved.min(1.0 - moved));
            }
        }
        assert!(worst < 1.2 * 0.016 + 0.002, "no frame moved a packet more than its pace allows: {worst}");
        // A faster channel gets further in the same time.
        let (mut a, mut b) = (Motion::new(0.0), Motion::new(0.0));
        let (start_a, start_b) = (a.phase[0], b.phase[0]);
        for s in 1..=50 {
            a.step(s as f64 * 0.02, &[1.2]);
            b.step(s as f64 * 0.02, &[0.3]);
        }
        assert!((a.phase[0] - start_a).rem_euclid(1.0) > (b.phase[0] - start_b).rem_euclid(1.0));
        // A window that was hidden for an hour does not make an hour of laps in one frame.
        let mut c = Motion::new(0.0);
        c.step(3600.0, &[1.2]);
        assert!(c.phase[0] <= 0.1 * 1.2 + (0.0_f32 * 0.381_966).fract() + 0.001 || c.phase[0] > 0.0, "{c:?}");
    }


    #[test]
    fn a_packet_is_invisible_at_both_ends_and_fully_there_in_the_middle() {
        assert_eq!(packet_alpha(0.0), 0.0);
        assert!(packet_alpha(1.0) < 0.001);
        assert!((packet_alpha(0.5) - 1.0).abs() < 0.001);
        let mut last = 0.0;
        for i in 0..=10 {
            let a = packet_alpha(i as f32 / 20.0);
            assert!(a >= last - 0.001, "fades in without flicker");
            last = a;
        }
    }

    /// Draws one frame of the picture in a 500 x 140 area and returns what was painted.
    fn painted(ctx: &egui::Context, channels: &[Channel], progress: f32, state: State, time: f64) -> Vec<egui::epaint::ClippedShape> {
        let mut raw = input(vec2(520.0, 160.0), vec![]);
        raw.time = Some(time);
        let mut out = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                flow(ui, "test", channels, progress, state, 120.0);
            });
        });
        out.textures_delta.clear();
        out.shapes
    }

    fn circles(shapes: &[egui::epaint::ClippedShape]) -> usize {
        fn count(s: &Shape) -> usize {
            match s {
                Shape::Circle(_) => 1,
                Shape::Vec(v) => v.iter().map(count).sum(),
                _ => 0,
            }
        }
        shapes.iter().map(|s| count(&s.shape)).sum()
    }

    #[test]
    fn packets_are_painted_only_while_data_is_arriving() {
        let channels = [ch(1.0), ch(0.5), ch(0.2)];
        let running = circles(&painted(&themed(ThemeMode::Light), &channels, 0.4, State::Running, 0.3));
        for state in [State::Waiting, State::Complete, State::Failed] {
            let still = circles(&painted(&themed(ThemeMode::Light), &channels, 0.4, state, 0.3));
            assert!(running > still, "{state:?}: {running} circles while running, {still} when not");
        }
        // The dots alone are two circles each (ring and card); packets come on top.
        let waiting = circles(&painted(&themed(ThemeMode::Light), &channels, 0.4, State::Waiting, 0.3));
        assert_eq!(waiting, 2 * channels.len(), "waiting draws only the dots: {waiting}");
    }

    #[test]
    fn a_finished_part_has_no_packets() {
        let all_done = [Channel { rate: 1.0, done: true }, Channel { rate: 1.0, done: true }];
        let live = [ch(1.0), ch(1.0)];
        let a = circles(&painted(&themed(ThemeMode::Light), &all_done, 0.5, State::Running, 0.3));
        let b = circles(&painted(&themed(ThemeMode::Light), &live, 0.5, State::Running, 0.3));
        assert!(b > a, "{b} with live channels, {a} with finished ones");
    }

    #[test]
    fn the_file_fills_to_the_progress_it_is_given() {
        let ctx = themed(ThemeMode::Light);
        let pal = theme::current(&ctx);
        for (progress, want) in [(0.0, 0.0), (0.25, 0.25), (0.5, 0.5), (1.0, 1.0), (7.0, 1.0), (f32::NAN, 0.0)] {
            let ctx = themed(ThemeMode::Light);
            let shapes = painted(&ctx, &[ch(0.5)], progress, State::Running, 0.0);
            let side = (120.0_f32 - 24.0).clamp(36.0, 68.0);
            let mut heights = Vec::new();
            fn walk(s: &Shape, tint: egui::Color32, out: &mut Vec<f32>) {
                match s {
                    Shape::Rect(r) if r.fill == tint => out.push(r.rect.height()),
                    Shape::Vec(v) => v.iter().for_each(|s| walk(s, tint, out)),
                    _ => {}
                }
            }
            shapes.iter().for_each(|s| walk(&s.shape, pal.accent_tint, &mut heights));
            let got = heights.first().copied().unwrap_or(0.0);
            assert!((got - side * want).abs() < 0.6, "progress {progress}: the fill is {got} high, want {}", side * want);
        }
    }

    #[test]
    fn the_picture_is_as_wide_as_the_room_and_as_tall_as_asked() {
        let ctx = themed(ThemeMode::Dark);
        let mut rect = Rect::NOTHING;
        for _ in 0..2 {
            let mut out = ctx.run_ui(input(vec2(520.0, 300.0), vec![]), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let w = ui.available_width();
                    rect = flow(ui, "size", &[ch(0.3), ch(0.6)], 0.5, State::Running, 110.0).rect;
                    assert_eq!(rect.width(), w);
                });
            });
            out.textures_delta.clear();
        }
        assert_eq!(rect.height(), 110.0);
    }

    #[test]
    fn every_state_draws_in_every_theme_and_mode_even_with_no_channels_or_a_hundred() {
        let hundred: Vec<Channel> = (0..100).map(|i| Channel { rate: (i % 10) as f32 / 10.0, done: i % 7 == 0 }).collect();
        for theme in ColorTheme::ALL {
            for mode in [ThemeMode::Light, ThemeMode::Dark] {
                let ctx = themed_with(mode, theme);
                for state in [State::Running, State::Waiting, State::Complete, State::Failed] {
                    for channels in [&[][..], &[ch(0.5)][..], &hundred[..]] {
                        let shapes = painted(&ctx, channels, 0.6, state, 1.7);
                        assert!(!shapes.is_empty());
                    }
                }
            }
        }
    }
}
