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

/// The staff in `r`: its lines, clefs, the trail (oldest at the left), and the heads now.
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
) {
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
    treble(b, pos2(x0 + 2.2 * s, y(32.0)), s, ink.text, k);
    bass(b, pos2(x0 + 2.0 * s, y(24.0)), s, ink.text, k);
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

/// A treble clef curled round the G line at `g` (`s` a space), in fine strokes.
fn treble(b: &mut Brush, g: Pos2, s: f32, col: Color32, k: f32) {
    let mut pts = Vec::with_capacity(96);
    // The spiral round the G line, opening outward.
    for i in 0..=40 {
        let t = i as f32 / 40.0;
        let a = PI * 0.5 + t * 2.1 * PI;
        let rad = s * (0.25 + 0.75 * t);
        pts.push(g + vec2(a.cos() * rad * 0.85, -a.sin() * rad));
    }
    // Up through the staff to a loop above it, then down the stem below it.
    let top = g + vec2(0.15 * s, -3.4 * s);
    let start = *pts.last().unwrap_or(&g);
    for i in 1..=14 {
        let t = i as f32 / 14.0;
        let p = start + (top - start) * t + vec2((t * PI).sin() * 0.5 * s, 0.0);
        pts.push(p);
    }
    for i in 1..=10 {
        let t = i as f32 / 10.0;
        let a = t * PI;
        pts.push(top + vec2(-a.sin() * 0.55 * s, (1.0 - a.cos()) * 0.45 * s));
    }
    let from = *pts.last().unwrap_or(&g);
    let foot = g + vec2(0.05 * s, 1.7 * s);
    for i in 1..=16 {
        let t = i as f32 / 16.0;
        pts.push(from + (foot - from) * t);
    }
    for i in 1..=8 {
        let a = i as f32 / 8.0 * PI;
        pts.push(foot + vec2(-(1.0 - a.cos()) * 0.3 * s, a.sin() * 0.35 * s));
    }
    b.polyline(&pts, 1.1 * k, col);
    if let Some(end) = pts.last() {
        b.dot(*end, 0.32 * s, col);
    }
}

/// A bass clef on the F line at `f`: its dot, its curve, the two dots round the line.
fn bass(b: &mut Brush, f: Pos2, s: f32, col: Color32, k: f32) {
    let c = f + vec2(0.25 * s, 0.0);
    let mut pts = Vec::with_capacity(40);
    for i in 0..=24 {
        let a = PI + i as f32 / 24.0 * PI * 1.35;
        pts.push(c + vec2(a.cos() * 0.75 * s, a.sin() * 0.85 * s));
    }
    let from = *pts.last().unwrap_or(&c);
    let tail = c + vec2(-0.85 * s, 2.4 * s);
    for i in 1..=12 {
        let t = i as f32 / 12.0;
        pts.push(from + (tail - from) * t + vec2(0.0, -(t * PI).sin() * 0.2 * s));
    }
    b.polyline(&pts, 1.2 * k, col);
    b.dot(c + vec2(-0.75 * s, 0.0), 0.3 * s, col);
    for dy in [-0.5f32, 0.5] {
        b.dot(c + vec2(1.25 * s, dy * s), 0.16 * s, col);
    }
}
