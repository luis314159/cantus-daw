//! The look of the action pair, Fuga and Volta (round 37, ADR 0105): hard two-tone cel
//! shading with a coloured rim light — a back-light edge along the far side of every big
//! part — angular shapes, and a thin, crisp ink that is heavier where a shadow would fall.
//! Every fill is opaque (ADR 0090 §2); a part made of several pieces is shaded piece by
//! piece, so a light never slides off one piece onto the floor between them.

use crate::render::Canvas;
use crate::rig::draw::{self as d, Ink, Look, Shape, Sink};
use crate::rig::P;

/// A hard-shaded material: the lit colour, the shadow, and the rim the back-light paints.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Hard {
    pub base: [u8; 4],
    pub shade: [u8; 4],
    pub rim: [u8; 4],
}

impl Hard {
    /// As a `Look` for the rig's two-band painting (the rim unused).
    pub fn look(&self) -> Look {
        Look {
            base: self.base,
            shade: self.shade,
            light: self.base,
            rim: [0, 0, 0, 0],
        }
    }
}

/// How much of a piece the rim takes (its far, lower-right edge) and how far the shadow
/// reaches in from it.
const RIM: f32 = 0.93;
const LIT: f32 = 0.7;

fn paint(c: &mut Canvas, s: &mut Shape, h: &Hard, rim: bool) {
    // Through the rig's own fills (counted, allocation-checked): a Look per step.
    if rim {
        d::bands_of(c, s, &flat(h.rim), 1);
        s.shrink_toward(RIM, 0.12, 0.1);
    }
    d::bands_of(c, s, &flat(h.shade), 1);
    s.shrink_toward(LIT, 0.16, 0.12);
    d::bands_of(c, s, &flat(h.base), 1);
}

fn flat(col: [u8; 4]) -> Look {
    Look {
        base: col,
        shade: col,
        light: col,
        rim: [0, 0, 0, 0],
    }
}

/// One piece, hard-shaded (with its rim when `rim`), no ink.
pub fn hard(c: &mut Canvas, shape: d::ShapeFn, h: &Hard, rim: bool) {
    let mut s = d::build(shape, 0.0, c.transform);
    paint(c, &mut s, h, rim);
}

/// Several pieces (`n`, the `k`th drawn by `piece`), each hard-shaded on its own.
pub fn hard_each(
    c: &mut Canvas,
    n: usize,
    piece: &dyn Fn(usize, &mut dyn Sink, f32),
    h: &Hard,
    rim: bool,
) {
    for k in 0..n {
        let mut s = d::build(&|sk, g| piece(k, sk, g), 0.0, c.transform);
        paint(c, &mut s, h, rim);
    }
}

/// A piece with its ink and hard shading.
pub fn part(c: &mut Canvas, shape: d::ShapeFn, h: &Hard, ink: &Ink, rim: bool) {
    d::outline(c, shape, ink);
    hard(c, shape, h, rim);
}

/// A closed polygon with sharp corners, grown by `g` (each point pushed out from the
/// middle of them all), at most 24 points.
pub fn poly_into(s: &mut dyn Sink, pts: &[P], g: f32) {
    let k = pts.len().min(24);
    if k < 3 {
        return;
    }
    if g == 0.0 {
        return d::polygon_into(s, &pts[..k]);
    }
    let n = k as f32;
    let c = (
        pts[..k].iter().map(|p| p.0).sum::<f32>() / n,
        pts[..k].iter().map(|p| p.1).sum::<f32>() / n,
    );
    let mut grown = [(0.0, 0.0); 24];
    for (o, p) in grown.iter_mut().zip(pts[..k].iter()) {
        let (dx, dy) = (p.0 - c.0, p.1 - c.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        *o = (p.0 + dx / len * g, p.1 + dy / len * g);
    }
    d::polygon_into(s, &grown[..k]);
}

/// An angular plate along `a` → `z`: its half-width at each share of the way (`prof`, at
/// most 8), straight edges between, grown by `g`.
pub fn slab_into(s: &mut dyn Sink, a: P, z: P, prof: &[(f32, f32)], g: f32) {
    let (dx, dy) = (z.0 - a.0, z.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-5);
    let (ux, uy) = (dx / len, dy / len);
    let (nx, ny) = (-uy, ux);
    let n = prof.len().min(8);
    if n < 2 {
        return;
    }
    let mut pts = [(0.0, 0.0); 16];
    for (i, &(u, h)) in prof.iter().take(n).enumerate() {
        let along = -g + u * (len + 2.0 * g);
        let c = (a.0 + ux * along, a.1 + uy * along);
        pts[i] = (c.0 + nx * (h + g), c.1 + ny * (h + g));
        pts[2 * n - 1 - i] = (c.0 - nx * (h + g), c.1 - ny * (h + g));
    }
    d::polygon_into(s, &pts[..2 * n]);
}

/// A point `t` of the way from `a` to `z`, pushed `off` along the left normal.
pub fn along(a: P, z: P, t: f32, off: f32) -> P {
    let (dx, dy) = (z.0 - a.0, z.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-5);
    (a.0 + dx * t - dy / len * off, a.1 + dy * t + dx / len * off)
}

/// How tall the pair are drawn: their teen proportions are drawn about 1.3 units tall and
/// scaled about the feet to stand about as tall as the rest of the family (one unit to
/// the eyes, ADR 0087 §2).
pub const SCALE: f32 = 0.82;

/// Draw `f` with the canvas scaled by [`SCALE`] about the feet (the pixels per unit too,
/// so the ground and the extras agree).
pub fn at_scale(c: &mut Canvas, f: impl FnOnce(&mut Canvas)) {
    let (t, px) = (c.transform, c.px_per_unit);
    c.transform = t.pre_scale(SCALE, SCALE);
    c.px_per_unit = px * SCALE;
    f(c);
    c.transform = t;
    c.px_per_unit = px;
}

/// The ground under a character drawn at [`SCALE`] (`rig::ground` of the canvas as the
/// renderer placed it, scaled the same): the floor its shadow, dust and confetti belong
/// on, whatever the lift and the spin.
pub fn ground(c: &Canvas, pose: &crate::pose::Pose) -> tiny_skia::Transform {
    let px = c.px_per_unit / SCALE;
    let t = c.transform.pre_scale(1.0 / SCALE, 1.0 / SCALE);
    let (s, k) = cantus_dsp::math::sin_cos(crate::rig::rad(pose.spin));
    let feet_x = t.tx - px * (-0.5 * s);
    let feet_y = t.ty - px * (0.5 * k - 0.5);
    tiny_skia::Transform::from_row(px, 0.0, 0.0, px, feet_x, feet_y + pose.lift * 0.6 * px)
        .pre_scale(SCALE, SCALE)
}
