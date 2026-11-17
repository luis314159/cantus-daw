//! Cedo's hardware primitives (design sheet v1): what every part of the ducking engine is
//! drawn with — the colourway's finishes (`kit::skin::engines`), hex bolts with their
//! washers, turned steel rods lit along their length, clevis pins, round-head rivets,
//! contact shadows, a turned boss. Vectors at the caller's scale; nothing animates by
//! itself. Original to Cantus.

use crate::kit::color::{mix, with_alpha, Hsl};
use crate::kit::skin::engines as pal;
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::math::{cos, sin};
use egui::{vec2, Color32, Pos2, Rect, Vec2};
use std::f32::consts::TAU;

pub fn c(h: Hsl) -> Color32 {
    h.rgb()
}

/// A number in 0 … 1 for `i`: the same every frame (a dimple's place, a bolt's turn).
pub fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x51ED_270B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// The colourway's finishes: Workshop, Locomotive, Exhibition.
pub fn finish(s: &Style) -> &'static pal::Finish {
    match s.skin.key {
        "fx.cedo.locomotive" => &pal::LOCOMOTIVE,
        "fx.cedo.exhibition" => &pal::EXHIBITION,
        _ => &pal::WORKSHOP,
    }
}

/// The direction of `a` radians, clockwise from up (the screen's y grows downward).
pub fn dir(a: f32) -> Vec2 {
    vec2(sin(a), -cos(a))
}

/// A soft shadow on the bed under a part: an ellipse of `radii` at `p`.
pub fn shadow(b: &mut Brush, clip: Rect, p: Pos2, radii: Vec2, depth: f32) {
    let s = c(pal::SHADOW);
    b.radial(
        clip,
        p,
        radii,
        &[
            (0.0, with_alpha(s, 0.55 * depth)),
            (0.6, with_alpha(s, 0.30 * depth)),
            (1.0, with_alpha(s, 0.0)),
        ],
        8,
    );
}

/// A hex bolt head at `p` of radius `rad` on its washer, turned `turn` (0 … 1).
pub fn bolt(b: &mut Brush, p: Pos2, rad: f32, turn: f32, metal: &pal::Tone) {
    let k = rad / 5.0;
    b.circle(
        p + vec2(0.5, 1.2) * k,
        rad * 1.5,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.45)),
    );
    b.circle(p, rad * 1.42, Fill::V(c(metal.light), c(metal.dark)));
    b.circle_stroke(p, rad * 1.42, 0.5 * k, with_alpha(c(metal.dark), 0.9));
    let mut hex = [Pos2::ZERO; 6];
    for (i, q) in hex.iter_mut().enumerate() {
        *q = p + dir(TAU * (turn + i as f32 / 6.0)) * rad;
    }
    b.polygon(&hex, c(metal.mid));
    // The facets lit from above: the upper three lighter.
    for i in 0..6 {
        let a = hex[i];
        let z = hex[(i + 1) % 6];
        let mid = a + (z - a) * 0.5;
        let lit = mid.y < p.y;
        let tri = [p, a, z];
        b.polygon(
            &tri,
            with_alpha(
                c(if lit { metal.light } else { metal.dark }),
                if lit { 0.55 } else { 0.45 },
            ),
        );
    }
    b.circle(p, rad * 0.45, Fill::V(c(metal.light), c(metal.mid)));
}

/// A round-head rivet at `p`.
pub fn rivet(b: &mut Brush, p: Pos2, rad: f32, metal: &pal::Tone) {
    b.circle(
        p + vec2(0.3, 0.8) * rad / 3.0,
        rad * 1.15,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.4)),
    );
    b.circle(p, rad, Fill::V(c(metal.light), c(metal.dark)));
    b.circle(
        p + vec2(-0.3, -0.35) * rad,
        rad * 0.38,
        Fill::Solid(with_alpha(c(pal::GLINT), 0.55)),
    );
}

/// A turned rod from `a` to `z`, `w` thick: its shadowed side, its body, the light along
/// its upper side.
pub fn rod(b: &mut Brush, a: Pos2, z: Pos2, w: f32, metal: &pal::Tone) {
    let d = z - a;
    let len = d.length().max(1e-3);
    let n = vec2(-d.y, d.x) / len;
    let n = if n.y > 0.0 { -n } else { n };
    b.segment(a, z, w, c(metal.dark), true);
    b.segment(
        a + n * w * 0.12,
        z + n * w * 0.12,
        w * 0.62,
        c(metal.mid),
        true,
    );
    b.segment(
        a + n * w * 0.26,
        z + n * w * 0.26,
        w * 0.2,
        with_alpha(c(metal.light), 0.9),
        false,
    );
}

/// A clevis pin through a boss at `p`: the boss, the pin's head, its split pin's glint.
pub fn pin(b: &mut Brush, p: Pos2, boss: f32, metal: &pal::Tone, pin_metal: &pal::Tone) {
    b.circle(p, boss, Fill::V(c(metal.light), c(metal.dark)));
    b.circle_stroke(p, boss, 0.06 * boss, with_alpha(c(metal.dark), 0.9));
    b.circle(
        p,
        boss * 0.55,
        Fill::V(c(pin_metal.light), c(pin_metal.dark)),
    );
    b.circle(
        p + vec2(-0.18, -0.2) * boss,
        boss * 0.16,
        Fill::Solid(with_alpha(c(pal::GLINT), 0.7)),
    );
}

/// A turned boss (a hub, a bearing's face): rings cut on the lathe, lit from the top left.
pub fn turned(b: &mut Brush, p: Pos2, rad: f32, metal: &pal::Tone) {
    b.circle(p, rad, Fill::V(c(metal.light), c(metal.dark)));
    let rings = (rad / 2.2).clamp(2.0, 9.0) as usize;
    for i in 1..rings {
        let r = rad * i as f32 / rings as f32;
        let a = if i % 2 == 0 { 0.16 } else { 0.10 };
        b.circle_stroke(p, r, 0.35, with_alpha(c(metal.dark), a));
    }
    b.arc(
        p,
        rad * 0.82,
        -2.4,
        -0.5,
        rad * 0.12,
        with_alpha(c(pal::GLINT), 0.35),
        false,
    );
}

/// A line of `n` points along `a … z` (for polylines built without allocating).
pub fn lerp(a: Pos2, z: Pos2, t: f32) -> Pos2 {
    a + (z - a) * t
}

/// A colour between a tone's body and its light.
pub fn lit(t: &pal::Tone, amount: f32) -> Color32 {
    mix(c(t.mid), c(t.light), amount.clamp(0.0, 1.0))
}
