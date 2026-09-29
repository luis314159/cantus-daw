//! The small displays in Calor's cards, drawn with the kit's brush in the card's hue: an
//! oscillator's cycle from its levels and width, an envelope's shape from its own times
//! (on a square-root time axis, so a 2 ms attack and a 2 s release both read) with the
//! newest voice's level, and an LFO's shape.

use crate::kit::color::with_alpha;

use crate::kit::{Brush, Style, Tok};
use egui::{Pos2, Rect};

/// The tokens these displays paint with, beside the card's hue (for inspect mode).
pub const TOKENS: &[Tok] = &[Tok::Ink, Tok::Grid, Tok::GridStrong, Tok::TextDim];

/// The fill under a curve in a display: the hue, faint.
pub fn fill(s: &Style) -> egui::Color32 {
    with_alpha(s.hue, if s.t.dark { 0.20 } else { 0.14 })
}

/// A grid line, one physical pixel.
fn grid_h(b: &mut Brush, r: Rect, y: f32, s: &Style) {
    b.hline(r.left(), r.right(), b.snap_mid(y), b.px(), s.t.grid);
}

/// One sample of an oscillator's mixed wave at phase `ph` (0…1).
pub fn osc_sample(ph: f32, saw: f32, pulse: f32, tri: f32, pw: f32) -> f32 {
    let ph = ph.fract();
    let sw = 1.0 - 2.0 * ph;
    let pl = if ph < pw { 1.0 } else { -1.0 };
    let tr = 1.0 - 4.0 * (ph - 0.5).abs();
    saw * sw + pulse * pl + tri * tr
}

/// Two cycles of an oscillator's wave, from its three levels and its pulse width.
pub fn cycle(b: &mut Brush, d: Rect, s: &Style, (saw, pulse, tri, pw): (f32, f32, f32, f32)) {
    b.register(d, "calor.cycle", TOKENS, &[s.hue_tok]);
    let mid = d.center().y;
    grid_h(b, d, mid, s);
    let total = (saw + pulse + tri).max(1e-3);
    if saw + pulse + tri < 1e-3 {
        return;
    }
    let amp = d.height() * 0.36 / total.max(1.0);
    let inner = d.shrink2(egui::vec2(s.s(8.0), 0.0));
    let mut pts = [Pos2::ZERO; 161];
    for (i, p) in pts.iter_mut().enumerate() {
        let x = i as f32 / 160.0;
        let y = osc_sample(x * 2.0, saw, pulse, tri, pw);
        *p = Pos2::new(inner.left() + inner.width() * x, mid - y * amp);
    }
    b.area(&pts, mid, fill(s));
    b.polyline(&pts, s.s(2.0), s.hue);
}

/// An envelope's shape: attack, decay to the sustain, a hold, the release; `level` (0…1)
/// is the newest voice's, marked at the right.
pub fn envelope(
    b: &mut Brush,
    d: Rect,
    s: &Style,
    (a, dcy, sus, r): (f32, f32, f32, f32),
    level: f32,
) {
    b.register(d, "calor.envelope", TOKENS, &[s.hue_tok]);
    let inner = d.shrink2(egui::vec2(s.s(10.0), s.s(8.0)));
    let (a, dcy, r) = (a.max(0.0).sqrt(), dcy.max(0.0).sqrt(), r.max(0.0).sqrt());
    let hold = (a + dcy + r) * 0.25;
    let total = (a + dcy + hold + r).max(1e-3);
    let x = |t: f32| inner.left() + inner.width() * t / total;
    let y = |v: f32| inner.bottom() - inner.height() * v.clamp(0.0, 1.0);
    grid_h(b, d, inner.bottom(), s);
    grid_h(b, d, y(sus), s);
    let mut pts = [Pos2::ZERO; 50];
    let mut n = 0;
    let mut push = |p: Pos2| {
        pts[n] = p;
        n += 1;
    };
    for i in 0..=16 {
        let f = i as f32 / 16.0;
        push(Pos2::new(x(a * f), y(1.0 - (1.0 - f).powf(1.6))));
    }
    for i in 1..=16 {
        let f = i as f32 / 16.0;
        push(Pos2::new(
            x(a + dcy * f),
            y(sus + (1.0 - sus) * (1.0 - f).powi(3)),
        ));
    }
    for i in 0..=16 {
        let f = i as f32 / 16.0;
        push(Pos2::new(
            x(a + dcy + hold + r * f),
            y(sus * (1.0 - f).powi(3)),
        ));
    }
    b.area(&pts[..n], inner.bottom(), fill(s));
    b.polyline(&pts[..n], s.s(2.0), s.hue);
    if level > 0.0 {
        b.dot(Pos2::new(d.right() - s.s(5.0), y(level)), s.s(3.0), s.hue);
    }
}

/// A pseudo-random step for the sample-and-hold pictures: the same every frame.
fn step(i: usize) -> f32 {
    let mut h = (i as u32).wrapping_mul(0x9E37_79B9) ^ 0x5bd1_e995;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 32_767.5 - 1.0
}

/// One value of an LFO's shape (the `shape` index as the DSP numbers them) at phase `ph`.
pub fn lfo_sample(shape: usize, ph: f32) -> f32 {
    let f = ph.fract();
    let cycle = ph.floor() as usize;
    match shape {
        1 => (f * std::f32::consts::TAU).sin(),
        2 => 2.0 * f - 1.0,
        3 => 1.0 - 2.0 * f,
        4 => {
            if f < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        5 => step(cycle * 2 + usize::from(f >= 0.5)),
        6 => {
            let i = cycle * 2 + usize::from(f >= 0.5);
            let t = (f * 2.0).fract();
            let t = t * t * (3.0 - 2.0 * t);
            step(i) + (step(i + 1) - step(i)) * t
        }
        _ => 1.0 - 4.0 * (f - 0.5).abs(),
    }
}

/// Two cycles of an LFO's shape; `unipolar` lifts it above the axis.
pub fn lfo(b: &mut Brush, d: Rect, s: &Style, shape: usize, unipolar: bool, phase_deg: f32) {
    b.register(d, "calor.lfo", TOKENS, &[s.hue_tok]);
    let inner = d.shrink2(egui::vec2(s.s(8.0), s.s(8.0)));
    let base = if unipolar {
        inner.bottom()
    } else {
        inner.center().y
    };
    grid_h(b, d, base, s);
    let off = phase_deg / 360.0;
    let mut pts = [Pos2::ZERO; 161];
    for (i, p) in pts.iter_mut().enumerate() {
        let x = i as f32 / 160.0;
        let v = lfo_sample(shape, x * 2.0 + off);
        let y = if unipolar {
            inner.bottom() - (v * 0.5 + 0.5) * inner.height()
        } else {
            inner.center().y - v * inner.height() * 0.5
        };
        *p = Pos2::new(inner.left() + inner.width() * x, y);
    }
    b.area(&pts, base, fill(s));
    b.polyline(&pts, s.s(2.0), s.hue);
}
