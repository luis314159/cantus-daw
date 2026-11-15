//! Saltus's motif on Tabula's board (ADR 0134's faces, ADR 0135): the leap — a small keyboard
//! in fine strokes and, from one of its keys, two arcs leaping up to an octave and to a fifth,
//! landing on their keys. Original to Cantus; the same each frame. // fx3:saltus

use crate::kit::color::with_alpha;
use crate::kit::{Brush, Fill};
use egui::{pos2, vec2, Color32, Pos2, Rect};

/// Paint the leap over the art area `r` (`u` the enclosure's scale).
pub fn leap(b: &mut Brush, u: f32, r: Rect, print: Color32, accent: Color32) {
    let fine = 0.8 * u;
    // The keyboard: nine white keys over the name, the black ones over them.
    let kh = r.height() * 0.2;
    let keys = Rect::from_min_size(
        pos2(r.left(), r.bottom() - r.height() * 0.36 - kh),
        vec2(r.width(), kh),
    );
    let n = 9;
    let kw = keys.width() / n as f32;
    for i in 0..n {
        let k = Rect::from_min_size(
            pos2(keys.left() + i as f32 * kw, keys.top()),
            vec2(kw, kh),
        );
        b.rrect_stroke(k.shrink(0.6 * u), 1.2 * u, fine, with_alpha(print, 0.55));
    }
    for i in [1, 2, 4, 5, 6, 8] {
        let x = keys.left() + i as f32 * kw;
        let k = Rect::from_center_size(
            pos2(x, keys.top() + kh * 0.3),
            vec2(kw * 0.55, kh * 0.6),
        );
        b.rrect(k, 1.0 * u, Fill::Solid(with_alpha(print, 0.45)));
    }
    let key_x = |i: usize| keys.left() + (i as f32 + 0.5) * kw;
    let from = 1;
    // The key it leaps from, lit.
    b.rrect(
        Rect::from_min_size(
            pos2(keys.left() + from as f32 * kw, keys.top() + kh * 0.62),
            vec2(kw, kh * 0.38),
        )
        .shrink(1.2 * u),
        1.0 * u,
        Fill::Solid(with_alpha(accent, 0.85)),
    );
    // Two leaps: the octave (seven white keys on) high, the fifth lower.
    let base = keys.top() - 2.0 * u;
    let mut pts = [Pos2::ZERO; 41];
    for (to, h, w, a) in [
        (from + 7, r.height() * 0.4, 1.7, 0.95),
        (from + 4, r.height() * 0.24, 1.2, 0.6),
    ] {
        let (x0, x1) = (key_x(from), key_x(to));
        for (j, p) in pts.iter_mut().enumerate() {
            let t = j as f32 / 40.0;
            *p = pos2(x0 + (x1 - x0) * t, base - 4.0 * h * t * (1.0 - t));
        }
        b.polyline(&pts, fine * w, with_alpha(accent, a));
        b.circle(pts[40], 2.2 * u, Fill::Solid(with_alpha(accent, a)));
    }
    b.circle_stroke(pos2(key_x(from), base), 2.4 * u, fine, accent);
}
