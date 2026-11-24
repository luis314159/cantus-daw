//! r42:arcus-upper — the figure's live limbs shaded as the baked body is (the owner,
//! 2026-11-24: flat cartoon tubes broke the picture): each arm, finger and hand a lit
//! tube or ball under the scene's key light — its diffuse light across the curve, the
//! ambient in its shade, a soft rim on its far edge — drawn in strips that follow the
//! surface's normal, so a sleeve reads round in both looks and every theme.

use super::body::View;
use crate::kit::appearance::rgb;
use crate::kit::Brush;
use crate::scene::geo::V3;
use egui::{Color32, Pos2};

/// The light the limbs are shaded in: the eye, the key's place, the ambient's share (the
/// evening's is lower), the rim's.
#[derive(Copy, Clone, Debug)]
pub struct Lit {
    pub eye: V3,
    pub key: V3,
    pub ambient: f32,
    pub rim: f32,
}

impl Lit {
    /// The scene's light round `mid` (the instrument's middle): the key softbox high to
    /// the front left, as both pictures place it.
    pub fn round(v: &View, mid: V3, dark: bool) -> Lit {
        Lit {
            eye: v.cam.eye(),
            key: mid
                + V3 {
                    x: -110.0,
                    y: 360.0 - mid.y,
                    z: 200.0,
                },
            ambient: if dark { 0.30 } else { 0.42 },
            rim: if dark { 0.22 } else { 0.12 },
        }
    }

    /// `col` lit at `p` with normal `n`.
    fn shade(&self, col: [u8; 3], p: V3, n: V3) -> Color32 {
        let l = (self.key - p).norm();
        let e = (self.eye - p).norm();
        let diffuse = n.dot(l).max(0.0);
        let rim = (1.0 - n.dot(e).max(0.0)).powi(3) * self.rim;
        let k = self.ambient + (1.0 - self.ambient) * diffuse;
        let c = |x: u8| ((x as f32 * k + 255.0 * rim).min(255.0)) as u8;
        rgb([c(col[0]), c(col[1]), c(col[2])])
    }
}

/// How many strips a limb is drawn in across its width.
const STRIPS: usize = 9;

/// A lit tube from `a` (`ra` thick) to `e` (`re`), its ends rounded by lit balls.
#[allow(clippy::too_many_arguments)]
pub fn tube(b: &mut Brush, v: &View, lit: &Lit, a: V3, e: V3, ra: f32, re: f32, col: [u8; 3]) {
    let axis = e - a;
    let len = axis.len();
    if len < 1e-3 {
        ball(b, v, lit, a, ra, col);
        return;
    }
    let u = axis * (1.0 / len);
    let mid = a + axis * 0.5;
    let to_eye = (lit.eye - mid).norm();
    let side = u.cross(to_eye).norm();
    let front = side.cross(u).norm();
    let front = if front.dot(to_eye) < 0.0 {
        -front
    } else {
        front
    };
    ball(b, v, lit, a, ra, col);
    ball(b, v, lit, e, re, col);
    let at = |s: f32, t: f32| {
        let r = ra + (re - ra) * t;
        let n = side * s + front * (1.0 - s * s).max(0.0).sqrt();
        (a + axis * t + n * r, n)
    };
    for i in 0..STRIPS {
        let s0 = -1.0 + 2.0 * i as f32 / STRIPS as f32;
        let s1 = -1.0 + 2.0 * (i + 1) as f32 / STRIPS as f32;
        let sm = 0.5 * (s0 + s1);
        let (pm, nm) = at(sm, 0.5);
        let col = lit.shade(col, pm, nm);
        let q: [Pos2; 4] = [
            v.p(at(s0, 0.0).0),
            v.p(at(s1 + 0.02, 0.0).0),
            v.p(at(s1 + 0.02, 1.0).0),
            v.p(at(s0, 1.0).0),
        ];
        b.polygon(&q, col);
    }
}

/// A lit ball round `c`, `r` thick: rings from its rim inward, each a little nearer the
/// light, so its brightest point sits toward the key.
pub fn ball(b: &mut Brush, v: &View, lit: &Lit, c: V3, r: f32, col: [u8; 3]) {
    let to_eye = (lit.eye - c).norm();
    let l = (lit.key - c).norm();
    // The light's way across the disc, as the eye sees it.
    let lx = (l - to_eye * l.dot(to_eye)).norm();
    let pc = v.p(c);
    let rr = r * v.scale(c);
    if rr < 0.3 {
        return;
    }
    let toward = v.p(c + lx * r) - pc;
    const RINGS: usize = 10;
    for k in 0..RINGS {
        let f = 1.0 - k as f32 / RINGS as f32;
        // The ring's normal: the widest turned from the light, the smallest toward it.
        let t = (1.0 - f) * 2.0 - 1.0;
        let n = (lx * (t * 0.55) + to_eye * (1.0 - 0.3 * t * t).sqrt()).norm();
        let off = toward * ((1.0 - f) * 0.3);
        b.circle(
            pc + off,
            rr * f,
            crate::kit::Fill::Solid(lit.shade(col, c + n * r, n)),
        );
    }
}
