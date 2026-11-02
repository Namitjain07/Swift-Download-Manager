//! Spinner wheel: 12 spokes whose brightness trails behind a turning leader.

use crate::theme;
use eframe::egui::{pos2, vec2, Sense, Stroke, Ui};
use std::time::Duration;

const SPOKES: usize = 12;

/// Brightness (40..=255) of spoke `i` when the leader is at `phase` (0..1 is one turn).
pub fn spoke_alpha(i: usize, phase: f32) -> u8 {
    let lead = (phase.rem_euclid(1.0) * SPOKES as f32) as usize % SPOKES;
    let behind = (lead + SPOKES - i % SPOKES) % SPOKES;
    (255.0 - behind as f32 * (215.0 / (SPOKES - 1) as f32)).round() as u8
}

/// A small activity wheel, one turn per second. Repaints about 12 times a second while shown.
pub fn spinner(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let ctx = ui.ctx().clone();
    let pal = theme::current(&ctx);
    let phase = (ctx.input(|i| i.time) % 1.0) as f32;
    let (c, r0, r1) = (rect.center(), size * 0.26, size * 0.46);
    for i in 0..SPOKES {
        let a = i as f32 / SPOKES as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let (dx, dy) = (a.cos(), a.sin());
        let col = pal.secondary.gamma_multiply(spoke_alpha(i, phase) as f32 / 255.0);
        ui.painter().line_segment([pos2(c.x + dx * r0, c.y + dy * r0), pos2(c.x + dx * r1, c.y + dy * r1)], Stroke::new((size * 0.09).max(1.5), col));
    }
    ctx.request_repaint_after(Duration::from_millis(83));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoke_alpha_fades_around_the_wheel() {
        for step in 0..12 {
            let phase = step as f32 / 12.0 + 0.01;
            assert_eq!(spoke_alpha(step, phase), 255, "leader at step {step}");
            assert!(spoke_alpha((step + 11) % 12, phase) < 255, "the spoke behind the leader is dimmer");
            for i in 0..12 {
                assert!(spoke_alpha(i, phase) >= 40, "never fully invisible");
            }
        }
    }
}
