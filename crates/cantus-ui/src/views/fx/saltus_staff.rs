//! Saltus's staff (ADR 0135, provisional): the band over the leap is a grand staff where the
//! last seconds are written as they are sung — the pitch heard as a line moving through the
//! staff (its vibrato, its slides), each voice's line beside it in its colour — and at the
//! right edge, now, the notes themselves: the note heard and every voice as note heads, with
//! their sharps and ledger lines. What the keyboard shows as a leap, the staff shows as music.

use super::super::canvas::{self, Ink};
use crate::kit::color::with_alpha;
use crate::kit::text::{Role, Text};
use crate::kit::Brush;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};
use std::collections::VecDeque;
use std::f32::consts::PI;

/// The staff's step of each pitch class (C D E F G A B = 0 … 6) and whether it is sharp.
const STEP: [(i32, bool); 12] = [
    (0, false),
    (0, true),
    (1, false),
    (1, true),
    (2, false),
    (3, false),
    (3, true),
    (4, false),
    (4, true),
    (5, false),
    (5, true),
    (6, false),
];

/// Middle C's place on the staff, in half spaces from C0.
const MIDDLE_C: i32 = 28;
/// The lines: the bass staff G2 … A3, the treble staff E4 … F5.
const LINES: [i32; 10] = [18, 20, 22, 24, 26, 30, 32, 34, 36, 38];
/// The places the band shows (A1 … E6): a note outside is drawn at the edge.
const LOW: i32 = 12;
const HIGH: i32 = 44;

#[cfg(test)]
mod tests {
    use super::place;

    /// Middle C sits between the staves, the bass staff's top line is A3, the treble's
    /// bottom line E4, a sharp sits on its natural's place.
    #[test]
    fn notes_sit_where_the_staff_says() {
        assert_eq!(place(60), (28, false));
        assert_eq!(place(57), (26, false));
        assert_eq!(place(64), (30, false));
        assert_eq!(place(66), (31, true));
        assert_eq!(place(43), (18, false));
        assert_eq!(place(77), (38, false));
    }
}

/// A note's place on the staff (half spaces from C0) and whether it carries a sharp.
pub fn place(m: i32) -> (i32, bool) {
    let (step, sharp) = STEP[m.rem_euclid(12) as usize];
    (m.div_euclid(12) * 7 + step - 7, sharp)
}

/// A fractional pitch's place, gliding between the notes' places.
fn place_f(x: f32) -> f32 {
    let m = x.floor() as i32;
    let f = x - m as f32;
    let (a, b) = (place(m).0 as f32, place(m + 1).0 as f32);
    a + (b - a) * f
}

/// A note to draw now: its pitch, its colour, how lit.
#[derive(Copy, Clone, Debug)]
pub struct Head {
    pub midi: f32,
    pub color: Color32,
    pub alpha: f32,
}

/// The staff in `r`: its lines, clefs, the trail (oldest at the left), and the heads now;
/// returns where each head was drawn, in `heads`' order.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    b: &mut Brush,
    ink: &Ink,
    r: Rect,
    trail: &VecDeque<[f32; 4]>,
    len: usize,
    colors: [Color32; 4],
    heads: &[Head],
    k: f32,
) -> Vec<Pos2> {
    let hs = r.height() / (HIGH - LOW + 2) as f32;
    let mid = r.center().y + (MIDDLE_C - (LOW + HIGH) / 2) as f32 * hs;
    let y = |p: f32| mid - (p - MIDDLE_C as f32) * hs;
    let (x0, x1) = (r.left(), r.right());
    for l in LINES {
        b.hline(x0, x1, y(l as f32), 0.7 * k, ink.grid_strong);
    }
    // The brace's line joining the two staves, and the clefs.
    b.vline(x0, y(38.0), y(18.0), 1.0 * k, ink.grid_strong);
    let s = 2.0 * hs;
    treble(b, pos2(x0 + 1.9 * s, y(32.0)), s * 0.92, ink.text, k);
    bass(b, pos2(x0 + 1.5 * s, y(24.0)), s * 0.92, ink.text, k);
    // The trail: from just after the clefs to the heads, newest at the right.
    let (t0, t1) = (x0 + 5.0 * s, x1 - 4.0 * s);
    let n = trail.len();
    let at = |age: usize| t1 - (t1 - t0) * age as f32 / (len.max(2) - 1) as f32;
    let clip = |p: f32| p.clamp(LOW as f32 - 1.0, HIGH as f32 + 1.0);
    for (lane, col) in colors.into_iter().enumerate() {
        for j in 1..n {
            let (a, c) = (trail[n - j][lane], trail[n - 1 - j][lane]);
            if a.is_nan() || c.is_nan() || (a - c).abs() > 1.5 {
                continue;
            }
            let fade = 1.0 - j as f32 / len as f32;
            let (w, alpha) = if lane == 0 {
                (1.6, 0.85 * fade)
            } else {
                (1.1, 0.5 * fade)
            };
            let pa = pos2(at(j - 1), y(clip(place_f(a))));
            let pc = pos2(at(j), y(clip(place_f(c))));
            b.segment(pa, pc, w * k, with_alpha(col, alpha), false);
        }
    }
    // Now: a hairline, and the heads with their ledger lines and sharps.
    b.vline(t1 + 1.5 * s, r.top(), r.bottom(), 0.6 * k, ink.grid);
    let mut placed: Vec<(i32, f32)> = Vec::new();
    let mut at_heads = Vec::with_capacity(heads.len());
    for h in heads {
        let m = h.midi.round() as i32;
        let (p, sharp) = place(m);
        let p = p.clamp(LOW, HIGH);
        // A head a step from one already drawn moves to the side (a second).
        let mut x = t1 + 1.5 * s;
        if placed
            .iter()
            .any(|(q, qx)| (q - p).abs() <= 1 && (qx - x).abs() < 1.0)
        {
            x += 1.25 * s;
        }
        placed.push((p, x));
        let c = with_alpha(h.color, h.alpha);
        let cy = y(p as f32);
        let ledgers: Vec<i32> = if p == MIDDLE_C {
            vec![MIDDLE_C]
        } else if p < 18 {
            (p..18).filter(|q| q % 2 == 0).collect()
        } else if p > 38 {
            (40..=p).filter(|q| q % 2 == 0).collect()
        } else {
            Vec::new()
        };
        for q in ledgers {
            b.hline(
                x - 0.95 * s,
                x + 0.95 * s,
                y(q as f32),
                0.8 * k,
                ink.grid_strong,
            );
        }
        head(b, pos2(x, cy), s, c);
        at_heads.push(pos2(x, cy));
        if sharp {
            canvas::put(
                b,
                pos2(x - 0.9 * s, cy),
                Align2::RIGHT_CENTER,
                &Text::new("#", Role::Label, c).caps(false),
                k,
                None,
            );
        }
    }
    at_heads
}

/// A filled note head: an ellipse tilted up to the right, a space tall.
fn head(b: &mut Brush, c: Pos2, s: f32, col: Color32) {
    let (rx, ry, tilt) = (0.62 * s, 0.42 * s, -0.35f32);
    let pts: [Pos2; 20] = std::array::from_fn(|i| {
        let a = i as f32 / 20.0 * 2.0 * PI;
        let (x, y) = (rx * a.cos(), ry * a.sin());
        c + vec2(
            x * tilt.cos() - y * tilt.sin(),
            x * tilt.sin() + y * tilt.cos(),
        )
    });
    b.polygon(&pts, col);
}

/// The treble clef's path, in staff spaces from the G line (y up): from the hook's dot, up
/// the stem to the loop above the staff, down across it into the bowl round the G line.
const TREBLE: [(f32, f32); 29] = [
    (-0.55, -2.35),
    (-0.4, -2.75),
    (0.0, -2.8),
    (0.22, -2.4),
    (0.27, -1.5),
    (0.31, 0.5),
    (0.33, 2.0),
    (0.36, 3.0),
    (0.6, 3.65),
    (0.72, 4.25),
    (0.55, 4.62),
    (0.18, 4.55),
    (-0.15, 4.1),
    (-0.38, 3.4),
    (-0.52, 2.65),
    (-0.75, 1.9),
    (-1.05, 1.0),
    (-1.15, 0.1),
    (-0.8, -0.75),
    (0.0, -1.05),
    (0.85, -0.75),
    (1.2, 0.05),
    (0.95, 0.85),
    (0.3, 1.15),
    (-0.3, 0.9),
    (-0.5, 0.3),
    (-0.25, -0.25),
    (0.2, -0.2),
    (0.2, -0.2),
];

/// The bass clef's curve, in spaces from the F line (y up): from its dot, over and down.
const BASS: [(f32, f32); 10] = [
    (-0.45, 0.0),
    (-0.4, 0.6),
    (0.0, 1.0),
    (0.6, 1.05),
    (1.1, 0.7),
    (1.25, 0.1),
    (1.05, -0.7),
    (0.55, -1.5),
    (-0.1, -2.15),
    (-0.7, -2.6),
];

/// A pen's stroke through `pts` (Catmull–Rom), as engraved: a broad nib held at 35°, thin
/// on the hairlines and full on the swells, between `thin` and `full` spaces wide.
#[allow(clippy::too_many_arguments)]
fn pen(
    b: &mut Brush,
    o: Pos2,
    s: f32,
    pts: &[(f32, f32)],
    thin: f32,
    full: f32,
    col: Color32,
    k: f32,
) {
    let at = |p: (f32, f32)| o + vec2(p.0 * s, -p.1 * s);
    let nib = 35f32.to_radians();
    let n = pts.len();
    let mut last: Option<Pos2> = None;
    for i in 0..n.saturating_sub(1) {
        let p0 = pts[i.saturating_sub(1)];
        let (p1, p2) = (pts[i], pts[i + 1]);
        let p3 = pts[(i + 2).min(n - 1)];
        for j in 0..=10 {
            let t = j as f32 / 10.0;
            let (t2, t3) = (t * t, t * t * t);
            let c = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b
                    + (c - a) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (3.0 * b - a - 3.0 * c + d) * t3)
            };
            let q = at((c(p0.0, p1.0, p2.0, p3.0), c(p0.1, p1.1, p2.1, p3.1)));
            if let Some(a) = last {
                let d = q - a;
                if d.length() > 0.05 {
                    // Screen y runs down: the nib's angle is taken in the page's sense.
                    let ang = (-d.y).atan2(d.x);
                    let w = (thin + (full - thin) * (ang - nib).sin().abs()) * s;
                    b.segment(a, q, w.max(0.8 * k), col, true);
                    last = Some(q);
                }
            } else {
                last = Some(q);
            }
        }
    }
}

/// A treble clef round the G line at `g` (`s` a space).
fn treble(b: &mut Brush, g: Pos2, s: f32, col: Color32, k: f32) {
    pen(b, g, s, &TREBLE, 0.06, 0.32, col, k);
    b.dot(g + vec2(-0.52 * s, 2.3 * s), 0.27 * s, col);
}

/// A bass clef on the F line at `f`: its dot, its curve, the two dots round the line.
fn bass(b: &mut Brush, f: Pos2, s: f32, col: Color32, k: f32) {
    pen(b, f, s, &BASS, 0.06, 0.40, col, k);
    b.dot(f + vec2(-0.45 * s, 0.0), 0.34 * s, col);
    for dy in [-0.5f32, 0.5] {
        b.dot(f + vec2(1.65 * s, dy * s), 0.13 * s, col);
    }
}
