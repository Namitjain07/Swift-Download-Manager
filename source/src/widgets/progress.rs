//! Capsule progress bars and the per-connection strip.

use super::anim::glide;
use crate::engine::Segment;
use crate::theme;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Id, Rect, Sense, Ui};

/// Width of the filled part: nothing at 0, never thinner than the bar is tall (so the
/// capsule stays round), and never wider than the track.
pub fn capsule_fill_width(frac: f32, width: f32, height: f32) -> f32 {
    if !frac.is_finite() || frac <= 0.0 {
        return 0.0;
    }
    (width * frac.min(1.0)).max(height).min(width)
}

fn capsule(rect: Rect) -> CornerRadius {
    CornerRadius::same((rect.height() * 0.5).ceil() as u8)
}

/// A capsule bar. The shown value glides to `frac` instead of jumping. `text` is drawn over it.
pub fn capsule_bar(ui: &mut Ui, id: Id, frac: f32, tint: Color32, height: f32, text: Option<&str>) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, height), Sense::hover());
    let ctx = ui.ctx().clone();
    let pal = theme::current(&ctx);
    let shown = glide(&ctx, id.with("frac"), if frac.is_finite() { frac.clamp(0.0, 1.0) } else { 0.0 }, 0.22);
    let p = ui.painter();
    if theme::design(&ctx).is_winui() && text.is_none() {
        // The WinUI 3 ProgressBar: a 3 px accent bar with rounded ends on a 1 px track line, both centred in the row.
        let centre = rect.center().y;
        let track = Rect::from_min_max(egui::pos2(rect.left(), centre - 0.5), egui::pos2(rect.right(), centre + 0.5));
        p.rect_filled(track, CornerRadius::same(1), pal.ctl_strong.gamma_multiply(0.6));
        let fw = capsule_fill_width(shown, rect.width(), 3.0);
        if fw > 0.0 {
            let fill = Rect::from_min_size(egui::pos2(rect.left(), centre - 1.5), egui::vec2(fw, 3.0));
            p.rect_filled(fill, CornerRadius::same(2), tint);
        }
        return;
    }
    p.rect_filled(rect, capsule(rect), pal.track);
    let fw = capsule_fill_width(shown, rect.width(), rect.height());
    if fw > 0.0 {
        let fill = Rect::from_min_size(rect.min, egui::vec2(fw, rect.height()));
        p.rect_filled(fill, capsule(fill), tint);
    }
    if let Some(t) = text {
        p.text(rect.center(), Align2::CENTER_CENTER, t, FontId::new(theme::design(&ctx).caption, theme::semibold()), pal.label);
    }
}

/// Overall progress plus a strip showing what each connection has fetched.
pub fn segment_bar(ui: &mut Ui, frac: f32, total: u64, segs: &[Segment]) {
    let id = ui.next_auto_id();
    let pal = theme::current(ui.ctx());
    capsule_bar(ui, id, frac, pal.accent, 8.0, None);
    ui.add_space(2.0);
    let w = ui.available_width();
    let (strip, _) = ui.allocate_exact_size(egui::vec2(w, 10.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(strip, capsule(strip), pal.track);
    if total > 0 {
        let last = segs.len().saturating_sub(1);
        for (i, s) in segs.iter().enumerate() {
            let x0 = strip.left() + strip.width() * (s.start as f32 / total as f32);
            let x1 = strip.left() + strip.width() * ((s.start + s.done.min(s.len())) as f32 / total as f32);
            if x1 - x0 < 0.5 {
                continue;
            }
            let r = Rect::from_min_max(egui::pos2(x0, strip.top()), egui::pos2(x1, strip.bottom()));
            // Rounded outer ends; the parts in between meet with a hairline gap.
            let mut cr = CornerRadius::ZERO;
            if i == 0 {
                cr.nw = 5;
                cr.sw = 5;
            }
            if i == last {
                cr.ne = 5;
                cr.se = 5;
            }
            p.rect_filled(r.shrink2(egui::vec2(0.5, 0.0)), cr, pal.accent_ink);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capsule_fill_width_is_clamped_and_never_thinner_than_the_bar_is_tall() {
        assert_eq!(capsule_fill_width(0.0, 100.0, 6.0), 0.0);
        assert_eq!(capsule_fill_width(0.01, 100.0, 6.0), 6.0);
        assert_eq!(capsule_fill_width(0.5, 100.0, 6.0), 50.0);
        assert_eq!(capsule_fill_width(1.0, 100.0, 6.0), 100.0);
        assert_eq!(capsule_fill_width(7.0, 100.0, 6.0), 100.0);
        assert_eq!(capsule_fill_width(f32::NAN, 100.0, 6.0), 0.0);
    }
}
