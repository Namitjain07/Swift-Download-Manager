//! Easing and small animation helpers. Everything eases out and runs 120 to 220 ms.

use eframe::egui::{self, Id};

/// Cubic ease-out: fast at first, settling gently. `t` is clamped to 0..=1.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// A value travelling from `from` to `to`, starting at time `start` (seconds).
#[derive(Clone, Copy)]
pub struct Glide {
    pub from: f32,
    pub to: f32,
    pub start: f64,
}

impl Glide {
    pub fn value_at(&self, now: f64, secs: f32) -> f32 {
        let p = ((now - self.start) as f32 / secs).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * ease_out(p)
    }
}

/// Eased 0..1 value that follows a yes/no state (hover, pressed, on/off).
pub fn fade(ctx: &egui::Context, id: Id, on: bool, secs: f32) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, on, secs, ease_out)
}

/// 0 to 1 over `secs` from the moment `id` starts being drawn: the first frame after it was not drawn
/// in the previous one (a dialog that has just been opened). Call it every frame the thing is drawn.
pub fn entrance(ctx: &egui::Context, id: Id, secs: f32) -> f32 {
    let (now, pass) = (ctx.input(|i| i.time), ctx.cumulative_pass_nr());
    let start = ctx.data(|d| d.get_temp::<(f64, u64)>(id)).filter(|(_, last)| pass <= last + 1).map_or(now, |(t0, _)| t0);
    ctx.data_mut(|d| d.insert_temp(id, (start, pass)));
    let k = ease_out(((now - start) as f32 / secs).clamp(0.0, 1.0));
    if k < 1.0 {
        ctx.request_repaint();
    }
    k
}

/// How far below its place a thing at progress `k` of its `entrance` still sits, in points.
pub fn rise(k: f32) -> f32 {
    (1.0 - k.clamp(0.0, 1.0)) * 14.0
}

/// Eases a number (a position, a size) towards `target`. Keeps the window repainting
/// only while it is still moving. The first call jumps straight to `target`.
pub fn glide(ctx: &egui::Context, id: Id, target: f32, secs: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    let Some(g) = ctx.data(|d| d.get_temp::<Glide>(id)) else {
        ctx.data_mut(|d| d.insert_temp(id, Glide { from: target, to: target, start: now }));
        return target;
    };
    let cur = g.value_at(now, secs);
    if g.to != target {
        ctx.data_mut(|d| d.insert_temp(id, Glide { from: cur, to: target, start: now }));
    }
    if (cur - target).abs() > 0.001 || g.to != target {
        ctx.request_repaint();
    }
    cur
}


/// Whether the window is on screen and in use: a hidden or minimized window needs no animation frames, and one in
/// the background only a few.
pub fn window_state(ctx: &egui::Context) -> (bool, bool) {
    ctx.input(|i| {
        let vp = i.viewport();
        (vp.minimized != Some(true), vp.focused != Some(false))
    })
}

/// Ask for the next frame of an animation that follows live data: about 30 times a second while the window is
/// in use, 4 times a second when it is behind other windows, not at all when it is minimized. (`request_repaint`
/// would draw every card again as fast as the machine can, which is what made many downloads at once stutter.)
pub fn repaint_soon(ctx: &egui::Context) {
    let (visible, focused) = window_state(ctx);
    if visible {
        ctx.request_repaint_after(std::time::Duration::from_millis(if focused { 33 } else { 250 }));
    }
}

/// One step of an exponential approach: `cur` moves towards `target` by the share of the way that `dt` seconds
/// make out of the time constant `tau`. The same result however often it is called, so the motion does not speed
/// up or slow down with the frame rate (`dt` is capped so that a long pause does not become one big jump).
pub fn approach(cur: f32, target: f32, dt: f64, tau: f32) -> f32 {
    let dt = dt.clamp(0.0, 0.25) as f32;
    cur + (target - cur) * (1.0 - (-dt / tau.max(0.001)).exp())
}

/// A number that follows `target` smoothly, for values that change all the time (progress, speed). Unlike `glide`
/// it does not start a new tween whenever the target moves, and it asks for frames at 30 a second, not all of them.
/// The first call jumps straight to `target`.
pub fn smooth(ctx: &egui::Context, id: Id, target: f32, tau: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    let Some((cur, at)) = ctx.data(|d| d.get_temp::<(f32, f64)>(id)) else {
        ctx.data_mut(|d| d.insert_temp(id, (target, now)));
        return target;
    };
    // The same frame asked twice (two passes) changes nothing.
    let next = if now > at { approach(cur, target, now - at, tau) } else { cur };
    ctx.data_mut(|d| d.insert_temp(id, (next, now.max(at))));
    if (next - target).abs() > 0.0005 {
        repaint_soon(ctx);
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_is_monotonic_with_fixed_endpoints() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        let mut last = 0.0;
        for i in 1..=10 {
            let v = ease_out(i as f32 / 10.0);
            assert!(v > last, "not increasing at step {i}");
            last = v;
        }
        // Eases out: the first half covers more than half the distance.
        assert!(ease_out(0.5) > 0.5);
    }

    #[test]
    fn glide_runs_from_start_to_target_over_its_duration() {
        let g = Glide { from: 10.0, to: 50.0, start: 100.0 };
        assert_eq!(g.value_at(100.0, 0.2), 10.0);
        assert_eq!(g.value_at(100.2, 0.2), 50.0);
        assert_eq!(g.value_at(200.0, 0.2), 50.0);
        let mid = g.value_at(100.1, 0.2);
        assert!(mid > 30.0 && mid < 50.0, "ease-out is past the halfway value, got {mid}");
    }

    /// One frame at `time` seconds; `draw` says whether the thing is drawn in it.
    fn frame(ctx: &egui::Context, time: f64, draw: bool) -> Option<f32> {
        let mut raw = crate::theme::testing::input(egui::vec2(100.0, 100.0), vec![]);
        raw.time = Some(time);
        let mut k = None;
        let mut out = ctx.run_ui(raw, |ui| {
            if draw {
                k = Some(entrance(ui.ctx(), Id::unique("thing"), 0.2));
            }
        });
        out.textures_delta.clear();
        k
    }

    #[test]
    fn an_entrance_starts_at_nothing_and_arrives_after_its_time() {
        let ctx = egui::Context::default();
        assert_eq!(frame(&ctx, 10.0, true), Some(0.0), "the first frame shows nothing yet");
        let mid = frame(&ctx, 10.1, true).unwrap();
        assert!(mid > 0.5 && mid < 1.0, "ease-out: well on the way after half the time, got {mid}");
        assert_eq!(frame(&ctx, 10.2, true), Some(1.0));
        assert_eq!(frame(&ctx, 15.0, true), Some(1.0), "and stays there while it is drawn");
    }

    #[test]
    fn an_entrance_starts_again_when_the_thing_was_not_drawn_for_a_frame() {
        let ctx = egui::Context::default();
        frame(&ctx, 1.0, true);
        assert_eq!(frame(&ctx, 1.5, true), Some(1.0));
        // Closed for a frame, then opened again.
        frame(&ctx, 2.0, false);
        assert_eq!(frame(&ctx, 2.1, true), Some(0.0), "a reopened dialog animates in again");
    }

    #[test]
    fn approaching_a_target_takes_the_same_path_however_many_frames_it_is_cut_into() {
        // One 100 ms step, or ten of 10 ms: the same place, so 20 and 120 frames a second look alike.
        let one = approach(0.0, 1.0, 0.1, 0.2);
        let ten = (0..10).fold(0.0, |v, _| approach(v, 1.0, 0.01, 0.2));
        assert!((one - ten).abs() < 0.001, "{one} vs {ten}");
        assert!(one > 0.3 && one < 0.5, "a tenth of a second on a 0.2 s constant: {one}");
        // Never overshoots, and a long pause is not one giant leap (capped at a quarter second).
        assert!(approach(0.0, 1.0, 60.0, 0.2) < 0.75);
        assert_eq!(approach(0.7, 0.7, 1.0, 0.2), 0.7);
        assert_eq!(approach(0.0, 1.0, -5.0, 0.2), 0.0, "time never runs backwards");
    }

    #[test]
    fn smooth_starts_at_the_target_and_then_trails_it_without_asking_for_every_frame() {
        let ctx = egui::Context::default();
        let id = Id::unique("smooth");
        let at = |time: f64, target: f32| {
            let mut raw = crate::theme::testing::input(egui::vec2(100.0, 100.0), vec![]);
            raw.time = Some(time);
            let mut v = 0.0;
            let mut out = ctx.run_ui(raw, |ui| v = smooth(ui.ctx(), id, target, 0.2));
            out.textures_delta.clear();
            (v, out.viewport_output.values().next().map(|o| o.repaint_delay))
        };
        assert_eq!(at(1.0, 0.5).0, 0.5, "the first value is the target itself");
        let (v, _) = at(1.05, 1.0);
        assert!(v > 0.5 && v < 1.0, "trailing the target: {v}");
        // After the pass that lays things out, what it asks for is a frame in about 30 ms, not "now".
        let (_, delay) = at(1.05, 1.0);
        let delay = delay.expect("a viewport output");
        assert!(delay >= std::time::Duration::from_millis(30), "about 30 frames a second, not one per pass: {delay:?}");
        // Same time again: no change.
        assert_eq!(at(1.05, 1.0).0, v);
    }

    #[test]
    fn the_rise_is_14_points_at_first_and_nothing_at_the_end() {
        assert_eq!(rise(0.0), 14.0);
        assert_eq!(rise(1.0), 0.0);
        assert_eq!(rise(2.0), 0.0, "never rises above its place");
        assert_eq!(rise(-1.0), 14.0);
        assert!(rise(0.25) > rise(0.75));
    }
}
