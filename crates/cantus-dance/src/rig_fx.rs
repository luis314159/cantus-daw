//! What the beat adds around a character (ADR 0090): sparkles, music notes, a dust puff on
//! a landing, confetti on a big jump, a sleepy *z*, a little heart. Each is a pure function
//! of its age `t` (0 when it is born, 1 when it is gone) and a seed, so a character draws
//! them from its pose alone — the same frame on screen and in a video.
//!
//! They are drawn with an outline in the character's ink, so they read on a light stage, a
//! dark one and over anything behind a transparent window; as every shape of the rig, each
//! is a fill or two through the pooled pen (no strokes, no gradients).

use super::draw::{self, alpha, ellipse_into, polygon_into, rect_into, Ink, Sink};
use super::{ease_out, ease_out_back, hash, lerp, rotate, window, P};
use crate::render::Canvas;
use cantus_dsp::math;

/// How a piece grows in and fades out over its life.
fn life(t: f32, grow: f32, fade: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let a = if grow > 0.0 {
        ease_out_back(window(t, 0.0, grow))
    } else {
        1.0
    };
    a * (1.0 - window(t, 1.0 - fade, 1.0))
}

/// A four-pointed star of radius `r` at `at`, turned by `deg`, grown by `g`.
fn star_into(s: &mut dyn Sink, at: P, r: f32, deg: f32, g: f32) {
    let pts: [P; 8] = std::array::from_fn(|i| {
        let rr = if i % 2 == 0 {
            r + g
        } else {
            r * 0.28 + g * 0.7
        };
        rotate((at.0, at.1 - rr), at, i as f32 * 45.0 + deg)
    });
    // Edges curved inward a little: a star of light, not of glass.
    s.move_to(pts[0].0, pts[0].1);
    for i in 0..8 {
        let (a, b) = (pts[i], pts[(i + 1) % 8]);
        let mid = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
        let ctrl = (lerp(mid.0, at.0, 0.35), lerp(mid.1, at.1, 0.35));
        s.quad_to(ctrl.0, ctrl.1, b.0, b.1);
    }
    s.close();
}

/// A four-pointed sparkle at `at`, `size` across, with a halo.
pub fn sparkle(c: &mut Canvas, at: P, size: f32, t: f32, fill: [u8; 4], ink: &Ink) {
    let k = life(t, 0.3, 0.45);
    if k <= 0.01 {
        return;
    }
    let r = size * 0.5 * k;
    let turn = 30.0 * t;
    draw::flat(
        c,
        &|s, _| ellipse_into(s, at, r * 1.25, r * 1.25, 0.0),
        alpha(fill, 0.22 * k.min(1.0)),
    );
    let star = |s: &mut dyn Sink, g: f32| star_into(s, at, r, turn, g);
    draw::outline(c, &star, &ink.thin(0.55 * k.min(1.0)));
    draw::flat(c, &star, fill);
}

/// A music note floating up from `at` over its life: an eighth (`kind` even) or two beamed
/// sixteenths (odd), swaying as it rises.
pub fn note(c: &mut Canvas, at: P, size: f32, t: f32, fill: [u8; 4], ink: &Ink, kind: u32) {
    let k = life(t, 0.18, 0.4);
    if k <= 0.01 {
        return;
    }
    let rise = ease_out(t) * size * 2.2;
    let sway = math::sin(t * 7.0) * size * 0.25;
    let s = size * k;
    let o = (at.0 + sway, at.1 - rise);
    let tilt = -18.0 + 10.0 * math::sin(t * 5.0);
    let beamed = kind % 2 == 1;
    let heads: [P; 2] = if beamed {
        [(-0.32, 0.0), (0.32, -0.12)]
    } else {
        [(0.0, 0.0), (0.0, 0.0)]
    };
    let count = if beamed { 2 } else { 1 };
    let shape = |sk: &mut dyn Sink, g: f32| {
        let mut tops = [(0.0, 0.0); 2];
        for (i, h) in heads.iter().take(count).enumerate() {
            let hc = (o.0 + h.0 * s, o.1 + h.1 * s);
            ellipse_into(sk, hc, s * 0.27 + g, s * 0.2 + g, tilt);
            let stem_x = hc.0 + s * 0.27 * 0.82;
            let top = (stem_x, hc.1 - s * 0.95);
            let len = hc.1 - s * 0.04 - top.1;
            rect_into(sk, (stem_x, top.1 + len * 0.5), s * 0.15, len, 0.0, g);
            tops[i] = top;
        }
        if beamed {
            let (a, b) = (tops[0], tops[1]);
            let w = s * 0.13;
            polygon_into(
                sk,
                &[
                    (a.0 - s * 0.075 - g, a.1 - g),
                    (b.0 + s * 0.075 + g, b.1 - g),
                    (b.0 + s * 0.075 + g, b.1 + w + g),
                    (a.0 - s * 0.075 - g, a.1 + w + g),
                ],
            );
        } else {
            // The flag: a curl from the top of the stem.
            let a = tops[0];
            let x = a.0 + s * 0.06;
            draw::blob_grown_into(
                sk,
                &[
                    (x - s * 0.08, a.1),
                    (x + s * 0.22, a.1 + s * 0.2),
                    (x + s * 0.26, a.1 + s * 0.52),
                    (x + s * 0.12, a.1 + s * 0.36),
                    (x - s * 0.08, a.1 + s * 0.24),
                ],
                g,
            );
        }
    };
    let ink = Ink {
        color: alpha(ink.color, k.min(1.0)),
        width: ink.width * 0.7,
    };
    draw::outline(c, &shape, &ink);
    draw::flat(c, &shape, alpha(fill, k.min(1.0)));
}

/// A dust puff where something landed: little clouds rolling out along the ground from
/// `at`, `size` wide in all.
pub fn dust(c: &mut Canvas, at: P, size: f32, t: f32, fill: [u8; 4], seed: i64) {
    if !(0.0..1.0).contains(&t) {
        return;
    }
    let fade = 1.0 - window(t, 0.35, 1.0);
    let spread = ease_out(t);
    let puffs = |s: &mut dyn Sink, _: f32| {
        for i in 0..6 {
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let j = (i / 2) as f32;
            let h = hash(seed * 31 + i as i64);
            let x = at.0 + side * size * (0.18 + 0.2 * j + 0.1 * h) * (0.4 + spread);
            let y = at.1 - size * (0.04 + 0.05 * j * spread + 0.06 * h * spread);
            let r = size * (0.09 - 0.018 * j) * (0.5 + 0.8 * spread);
            ellipse_into(s, (x, y), r, r * 0.8, 0.0);
        }
    };
    draw::flat(c, &puffs, alpha(fill, 0.8 * fade));
}

/// Confetti thrown up from `at`: `n` pieces in `colors`, falling and fluttering (one fill
/// per colour).
pub fn confetti(c: &mut Canvas, at: P, size: f32, t: f32, n: usize, colors: &[[u8; 4]], seed: i64) {
    if colors.is_empty() || !(0.0..1.0).contains(&t) {
        return;
    }
    let fade = 1.0 - window(t, 0.7, 1.0);
    for (ci, col) in colors.iter().enumerate() {
        let pieces = |s: &mut dyn Sink, _: f32| {
            for i in (ci..n).step_by(colors.len()) {
                let h1 = hash(seed * 97 + i as i64 * 3);
                let h2 = hash(seed * 97 + i as i64 * 3 + 1);
                let h3 = hash(seed * 97 + i as i64 * 3 + 2);
                let ang = lerp(-75.0, 75.0, h1);
                let speed = size * (0.9 + 0.7 * h2);
                let (sn, co) = (super::sin_deg(ang), super::cos_deg(ang));
                let x = at.0 + sn * speed * t * 1.1;
                let y = at.1 - co * speed * t * 1.6 + size * 1.5 * t * t;
                let spin = (h3 * 360.0) + t * 720.0 * (h2 - 0.5);
                let flip = math::cos(t * 18.0 + h3 * 6.0).abs().max(0.15);
                let w = size * 0.14;
                rect_into(s, (x, y), w * flip, w * 0.5, spin, 0.0);
            }
        };
        draw::flat(c, &pieces, alpha(*col, fade));
    }
}

/// A sleepy *z* rising from `at`.
pub fn zzz(c: &mut Canvas, at: P, size: f32, t: f32, fill: [u8; 4], ink: &Ink) {
    let k = life(t, 0.2, 0.4);
    if k <= 0.01 {
        return;
    }
    let s = size * k;
    let o = (at.0 + t * size * 0.8, at.1 - ease_out(t) * size * 1.6);
    let bar = s * 0.16;
    let z = |sk: &mut dyn Sink, g: f32| {
        rect_into(sk, (o.0, o.1 - s * 0.35), s * 0.7, bar, 0.0, g);
        rect_into(sk, (o.0, o.1 + s * 0.35), s * 0.7, bar, 0.0, g);
        rect_into(sk, o, s * 0.95, bar, -45.0, g);
    };
    draw::outline(c, &z, &ink.thin(0.6));
    draw::flat(c, &z, alpha(fill, k.min(1.0)));
}

/// A little heart at `at`, rising a little and fading.
pub fn heart(c: &mut Canvas, at: P, size: f32, t: f32, fill: [u8; 4], ink: &Ink) {
    let k = life(t, 0.25, 0.4);
    if k <= 0.01 {
        return;
    }
    let o = (at.0, at.1 - ease_out(t) * size * 0.9);
    let shape = |sk: &mut dyn Sink, g: f32| {
        let s = size * 0.5 * k + g;
        sk.move_to(o.0, o.1 + s * 0.9);
        sk.cubic_to(
            o.0 - s * 1.4,
            o.1 - s * 0.1,
            o.0 - s * 0.7,
            o.1 - s * 1.1,
            o.0,
            o.1 - s * 0.35,
        );
        sk.cubic_to(
            o.0 + s * 0.7,
            o.1 - s * 1.1,
            o.0 + s * 1.4,
            o.1 - s * 0.1,
            o.0,
            o.1 + s * 0.9,
        );
        sk.close();
    };
    draw::outline(c, &shape, &ink.thin(0.6));
    draw::flat(c, &shape, alpha(fill, k.min(1.0)));
}
