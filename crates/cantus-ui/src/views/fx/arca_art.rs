//! Arca's motif on Tabula's board (ADR 0134's faces, ADR 0137): the chest — a valve in fine
//! strokes, its filament lit, standing before a speaker's cone of rings. Original to Cantus;
//! the same each frame. // fx3:arca

use crate::kit::color::with_alpha;
use crate::kit::{Brush, Fill};
use egui::{pos2, vec2, Color32, Pos2, Rect};

/// Paint the chest over the art area `r` (`u` the enclosure's scale).
pub fn chest(b: &mut Brush, u: f32, r: Rect, print: Color32, accent: Color32) {
    let fine = 0.8 * u;
    // The cone: rings round a cap, at the right.
    let rad = (r.height() * 0.46).min(r.width() * 0.32);
    let c = pos2(r.right() - rad - 2.0 * u, r.center().y);
    for (i, f) in [1.0f32, 0.84, 0.66, 0.48].iter().enumerate() {
        let a = if i == 0 { 0.7 } else { 0.42 };
        b.circle_stroke(c, rad * f, fine, with_alpha(print, a));
    }
    b.circle(c, rad * 0.22, Fill::Solid(with_alpha(print, 0.35)));
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::FRAC_PI_4;
        let d = vec2(a.cos(), a.sin());
        b.segment(
            c + d * rad * 0.26,
            c + d * rad * 0.84,
            fine * 0.6,
            with_alpha(print, 0.25),
            false,
        );
    }
    // The valve: its glass, the plate inside, the filament glowing, its pins.
    let h = r.height() * 0.86;
    let w = h * 0.36;
    let v = Rect::from_min_size(pos2(r.left() + 4.0 * u, r.center().y - h * 0.5), vec2(w, h));
    let glow_c = pos2(v.center().x, v.top() + h * 0.48);
    b.circle(glow_c, w * 0.9, Fill::Solid(with_alpha(accent, 0.12)));
    b.circle(glow_c, w * 0.5, Fill::Solid(with_alpha(accent, 0.18)));
    let mut outline = [Pos2::ZERO; 23];
    for (j, p) in outline.iter_mut().enumerate() {
        // Up one side, round the dome, down the other.
        let t = j as f32 / 22.0;
        *p = if t < 0.3 {
            let y = v.bottom() - h * 0.12 - (t / 0.3) * h * 0.58;
            pos2(v.left(), y)
        } else if t < 0.7 {
            let a = std::f32::consts::PI * (1.0 + (t - 0.3) / 0.4);
            pos2(
                v.center().x + a.cos() * w * 0.5,
                v.top() + w * 0.5 + a.sin() * w * 0.5,
            )
        } else {
            let y = v.top() + w * 0.5 + ((t - 0.7) / 0.3) * (h * 0.88 - w * 0.5);
            pos2(v.right(), y.min(v.bottom() - h * 0.12))
        };
    }
    b.polyline(&outline, fine * 1.2, with_alpha(print, 0.85));
    let plate = Rect::from_center_size(
        pos2(v.center().x, v.top() + h * 0.46),
        vec2(w * 0.56, h * 0.36),
    );
    b.rrect_stroke(plate, 1.0 * u, fine, with_alpha(print, 0.6));
    b.segment(
        pos2(v.center().x, plate.top() + 2.0 * u),
        pos2(v.center().x, plate.bottom() - 2.0 * u),
        fine * 1.6,
        accent,
        false,
    );
    let base = Rect::from_min_max(pos2(v.left(), v.bottom() - h * 0.12), v.max);
    b.rrect(base, 1.5 * u, Fill::Solid(with_alpha(print, 0.45)));
    for i in 0..3 {
        let x = base.left() + base.width() * (0.25 + 0.25 * i as f32);
        b.segment(
            pos2(x, base.bottom()),
            pos2(x, base.bottom() + 3.0 * u),
            fine,
            with_alpha(print, 0.6),
            false,
        );
    }
}
