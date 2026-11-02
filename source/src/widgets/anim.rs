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
}
