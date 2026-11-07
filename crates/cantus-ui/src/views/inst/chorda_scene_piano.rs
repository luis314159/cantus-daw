//! The piano (ADR 0104), built in 3-D from its plan: a grand's outline (a convex shoulder,
//! a concave waist, a round tail — `edge_x`) raised into the rim (outer and inner skins,
//! its top), the soundboard, the cast frame with its holes, struts and tuning pins, the
//! strings (the bass over-strung), bridges and dampers, 88 keys each its own key, the key
//! slip and cheeks, the fallboard, the desk with its fretwork, tapered legs on brass
//! ferrules and casters, the lyre and pedals, and the lid on its stick at the Lid control's
//! angle; or an upright's carcass between its side cheeks, mouldings, top lid, turned
//! columns on their toes, keys, fallboard and pedals. Materials, not colours: `shade.rs`
//! lights them per pixel.
//!
//! The world's plan: the keyboard's front at z = 0, the player at z > 0 facing −z, the bass
//! at x = 0 (the spine, where a grand's lid is hinged), the treble at x = `w`.

use super::geo::{Geo, Vert, M};
use super::mesh::{v3, V3};
use cantus_core::inst::chorda_look::Body;

/// A piano's dimensions (m).
#[derive(Copy, Clone, Debug)]
pub struct Piano {
    pub body: Body,
    /// The case's width, and a grand's length (an upright's depth).
    pub w: f32,
    pub l: f32,
    /// A grand's case: its underside and its rim's top; an upright's height in `y1`.
    pub y0: f32,
    pub y1: f32,
    a: f32,
    b: f32,
    xt: f32,
}

/// The lid's widest angle (the full stick), radians: 36°. An upright's top opens 24°.
pub const FULL_STICK: f32 = 0.628;
pub const UPRIGHT_TOP: f32 = 0.42;
/// The width of the 88 keys (m) and the white keys' top.
const KEYS_W: f32 = 1.23;
const KEY_Y: f32 = 0.745;

impl Piano {
    pub fn of(body: Body) -> Piano {
        let (w, l, a, b, xt) = match body {
            Body::Concert => (1.57, 2.74, 0.20, 0.86, 0.40),
            Body::Grand => (1.49, 2.00, 0.16, 0.84, 0.42),
            Body::Parlour => (1.48, 1.55, 0.11, 0.80, 0.46),
            Body::Upright => (1.50, 0.60, 0.0, 0.0, 0.0),
        };
        let (y0, y1) = if body == Body::Upright {
            (0.0, 1.25)
        } else {
            (0.64, 0.995)
        };
        Piano {
            body,
            w,
            l,
            y0,
            y1,
            a,
            b,
            xt,
        }
    }

    /// The bentside's (or the tail's) x at depth `z` (0 … −l).
    pub fn edge_x(&self, z: f32) -> f32 {
        let zeta = (-z / self.l).clamp(0.0, 1.0);
        let xt = self.xt * self.w;
        if zeta <= self.a {
            self.w
        } else if zeta <= self.b {
            let t = (zeta - self.a) / (self.b - self.a);
            let s = t * t * (3.0 - 2.0 * t);
            self.w - (self.w - xt) * s
        } else {
            let t = (zeta - self.b) / (1.0 - self.b);
            xt * (1.0 - t * t).max(0.0).sqrt()
        }
    }

    /// The depth at which the bentside is at `x` (bisection; the bentside is monotone).
    pub fn z_at_x(&self, x: f32) -> f32 {
        let (mut lo, mut hi) = (self.a, 1.0f32);
        for _ in 0..16 {
            let mid = 0.5 * (lo + hi);
            if self.edge_x(-mid * self.l) > x {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        -0.5 * (lo + hi) * self.l
    }

    /// What the camera must hold in the frame (the lid at full stick included).
    pub fn frame_points(&self) -> [V3; 8] {
        let (w, l) = (self.w, self.l);
        if self.body == Body::Upright {
            let h = self.y1;
            [
                v3(0.0, 0.0, 0.25),
                v3(w, 0.0, 0.25),
                v3(0.0, 0.0, -l),
                v3(w, 0.0, -l),
                v3(0.0, h + 0.25, 0.0),
                v3(w, h + 0.25, 0.0),
                v3(0.0, h, -l),
                v3(w, h, -l),
            ]
        } else {
            let lift = v3(
                w * FULL_STICK.cos(),
                self.y1 + w * FULL_STICK.sin(),
                -l * 0.4,
            );
            [
                v3(0.0, 0.0, 0.2),
                v3(w, 0.0, 0.2),
                v3(0.0, 0.0, -l),
                v3(w, 0.0, -l * 0.35),
                v3(0.0, self.y1, -l),
                v3(w, self.y1, 0.0),
                lift,
                v3(0.0, self.y1 + 0.3, 0.0),
            ]
        }
    }

    pub fn centre(&self) -> V3 {
        if self.body == Body::Upright {
            v3(self.w * 0.5, 0.62, -0.1)
        } else {
            v3(self.w * 0.5, 0.75, -self.l * 0.45)
        }
    }

    /// The lid's far corner over the treble at `lid` (0 … 1), for the probe.
    pub fn lid_tip(&self, lid: f32) -> V3 {
        if self.body == Body::Upright {
            let b = lid.clamp(0.0, 1.0) * UPRIGHT_TOP;
            v3(
                self.w,
                self.y1 + self.l * b.sin(),
                -self.l + self.l * b.cos(),
            )
        } else {
            let a = lid.clamp(0.0, 1.0) * FULL_STICK;
            let z = -self.l * 0.3;
            let x = self.edge_x(z);
            v3(x * a.cos(), self.y1 + x * a.sin(), z)
        }
    }
}

/// How many points the grand's bentside and tail are drawn with.
const NB: usize = 64;
const NO: usize = NB + 1 + 6;

/// Points (or normals) in the plan, all round the outline.
type Plan2 = [(f32, f32); NO];

/// The grand's outline in the plan (bentside and tail from the treble's front corner, then
/// the spine back to the front) and each point's outward normal.
fn outline(p: &Piano) -> (Plan2, Plan2) {
    let mut pts = [(0.0f32, 0.0f32); NO];
    for (i, q) in pts.iter_mut().enumerate().take(NB + 1) {
        let u = i as f32 / NB as f32;
        let zeta = 1.0 - (1.0 - u).powf(1.25);
        let z = -zeta * p.l;
        *q = (p.edge_x(z), z);
    }
    for k in 1..=6 {
        pts[NB + k] = (0.0, -p.l + p.l * k as f32 / 6.0);
    }
    let mut nrm = [(0.0f32, 0.0f32); NO];
    for i in 0..NO {
        let a = pts[i.saturating_sub(1)];
        let b = pts[(i + 1).min(NO - 1)];
        let (tx, tz) = (b.0 - a.0, b.1 - a.1);
        let l = (tx * tx + tz * tz).sqrt().max(1e-6);
        nrm[i] = (-tz / l, tx / l);
    }
    for n in nrm.iter_mut().skip(NB + 1) {
        *n = (-1.0, 0.0);
    }
    (pts, nrm)
}

/// A grand's three legs in the plan.
fn legs(p: &Piano) -> [(f32, f32); 3] {
    let zt = -p.l * 0.83;
    [
        (0.12, -0.10),
        (p.w - 0.12, -0.10),
        ((p.edge_x(zt) * 0.5).max(0.2), zt),
    ]
}

/// A tapered leg on a brass ferrule and a caster.
fn leg(g: &mut Geo, p: &Piano, x: f32, z: f32) {
    let top = v3(x, p.y0, z);
    let foot = v3(x, 0.11, z);
    g.tube(foot, top, 0.042, 0.056, 12, M::Case, 0);
    // The collar under the case.
    g.tube(v3(x, p.y0 - 0.05, z), top, 0.062, 0.062, 12, M::Case, 0);
    g.tube(v3(x, 0.075, z), foot, 0.04, 0.043, 12, M::Brass, 0);
    g.tube(
        v3(x - 0.02, 0.035, z + 0.01),
        v3(x + 0.02, 0.035, z + 0.01),
        0.032,
        0.032,
        12,
        M::Rubber,
        0,
    );
    g.tube(v3(x, 0.06, z), v3(x, 0.08, z), 0.03, 0.035, 10, M::Brass, 0);
}

/// The lyre and its three pedals, under the keyboard's middle.
fn lyre(g: &mut Geo, p: &Piano) {
    let cx = p.w * 0.5;
    let zc = 0.03;
    for dx in [-0.075f32, 0.075] {
        g.tube(
            v3(cx + dx * 0.75, 0.15, zc + 0.01),
            v3(cx + dx, p.y0, zc),
            0.018,
            0.022,
            8,
            M::Case,
            0,
        );
    }
    g.boxed(
        v3(cx - 0.19, 0.05, zc - 0.04),
        v3(cx + 0.19, 0.15, zc + 0.05),
        0.012,
        M::Case,
        0,
    );
    for dx in [-0.1f32, 0.0, 0.1] {
        g.tube(
            v3(cx + dx, 0.095, zc + 0.04),
            v3(cx + dx, 0.085, zc + 0.17),
            0.014,
            0.017,
            10,
            M::Brass,
            0,
        );
        g.boxed(
            v3(cx + dx - 0.022, 0.075, zc + 0.12),
            v3(cx + dx + 0.022, 0.092, zc + 0.19),
            0.006,
            M::Brass,
            0,
        );
    }
}

/// The grand, whole.
pub fn grand(g: &mut Geo, p: &Piano, lid: f32) {
    let (pts, nrm) = outline(p);
    let alpha = lid.clamp(0.0, 1.0) * FULL_STICK;
    for (x, z) in legs(p) {
        leg(g, p, x, z);
    }
    lyre(g, p);
    let at = |i: usize, y: f32| v3(pts[i].0, y, pts[i].1);
    let n_of = |i: usize| v3(nrm[i].0, 0.0, nrm[i].1);
    let inner = 0.045;
    let ins = |i: usize, y: f32| at(i, y) - n_of(i) * inner;
    let y_sb = 0.80;
    // The rim: outer skin, inner skin, its top, the case's underside; uv along the rim.
    let mut run = 0.0f32;
    let up = v3(0.0, 1.0, 0.0);
    for i in 0..NO - 1 {
        let seg = (at(i + 1, 0.0) - at(i, 0.0)).len();
        let (u0, u1) = (run, run + seg);
        run = u1;
        let vt = |p: V3, n: V3, u: f32| Vert { p, n, uv: [u, p.y] };
        let (na, nb) = (n_of(i), n_of(i + 1));
        let (a0, a1, b0, b1) = (at(i, p.y0), at(i, p.y1), at(i + 1, p.y0), at(i + 1, p.y1));
        g.tri(vt(a0, na, u0), vt(b0, nb, u1), vt(b1, nb, u1), M::Case, 0);
        g.tri(vt(a0, na, u0), vt(b1, nb, u1), vt(a1, na, u0), M::Case, 0);
        let (c0, c1, d0, d1) = (
            ins(i, y_sb),
            ins(i, p.y1),
            ins(i + 1, y_sb),
            ins(i + 1, p.y1),
        );
        g.tri(
            vt(c0, -na, u0),
            vt(d1, -nb, u1),
            vt(d0, -nb, u1),
            M::Inner,
            0,
        );
        g.tri(
            vt(c0, -na, u0),
            vt(c1, -na, u0),
            vt(d1, -nb, u1),
            M::Inner,
            0,
        );
        g.tri(vt(a1, up, u0), vt(b1, up, u1), vt(d1, up, u1), M::Case, 0);
        g.tri(vt(a1, up, u0), vt(d1, up, u1), vt(c1, up, u0), M::Case, 0);
    }
    // The underside, a fan from the middle (seen only in a polished floor).
    let mid = v3(p.w * 0.4, p.y0, -p.l * 0.45);
    let down = v3(0.0, -1.0, 0.0);
    for i in 0..NO - 1 {
        let vt = |q: V3| Vert {
            p: q,
            n: down,
            uv: [q.x, q.z],
        };
        g.tri(vt(mid), vt(at(i + 1, p.y0)), vt(at(i, p.y0)), M::Inner, 0);
    }
    interior(g, p, y_sb);
    keyboard(g, p);
    lid_and_desk(g, p, alpha);
}

/// Soundboard, frame, strings, bridges, dampers and pins.
fn interior(g: &mut Geo, p: &Piano, y_sb: f32) {
    let zf = -0.03;
    let zb = -p.l * 0.99;
    g.patch(
        24,
        6,
        |u, v| {
            let z = zf + (zb - zf) * u;
            v3(0.025 + v * (p.edge_x(z) - 0.07).max(0.0), y_sb, z)
        },
        true,
        1.0,
        1.0,
        M::Spruce,
        0,
    );
    // The frame: gold, with its holes, struts and the pin field.
    let y_pl = super::shade::PLATE_Y - 0.015;
    let zpf = -0.02;
    let zpb = -p.l * 0.955;
    g.patch(
        28,
        8,
        |u, v| {
            let z = zpf + (zpb - zpf) * u;
            v3(0.05 + v * (p.edge_x(z) - 0.16).max(0.0), y_pl, z)
        },
        true,
        1.0,
        1.0,
        M::Gold,
        0,
    );
    // The frame's rim: a raised lip round it.
    let mut lip = [V3::default(); 30];
    for (i, q) in lip.iter_mut().enumerate() {
        let z = zpf + (zpb - zpf) * i as f32 / 29.0;
        *q = v3(p.edge_x(z) - 0.11, y_pl + 0.012, z);
    }
    g.hose(&lip, 0.016, M::Gold, 0);
    let holes = [
        (0.52, 0.40, 0.085),
        (0.66, 0.27, 0.06),
        (0.38, 0.60, 0.09),
        (0.30, 0.79, 0.065),
        (0.70, 0.14, 0.05),
    ];
    for (fx, fz, r) in holes {
        let z = -fz * p.l;
        let x = (fx * p.w).min(p.edge_x(z) - 0.2);
        g.disc(
            v3(x, y_pl + 0.001, z),
            v3(0.0, 1.0, 0.0),
            r,
            24,
            M::Spruce,
            1,
        );
        let mut ring = [V3::default(); 25];
        for (k, q) in ring.iter_mut().enumerate() {
            let a = k as f32 / 24.0 * std::f32::consts::TAU;
            *q = v3(x + a.cos() * r, y_pl + 0.006, z + a.sin() * r);
        }
        g.hose(&ring, 0.008, M::Gold, 0);
    }
    let struts = [
        (0.30, 0.20, 0.92),
        (0.47, 0.37, 0.84),
        (0.64, 0.56, 0.62),
        (0.81, 0.79, 0.34),
    ];
    for (fx0, fx1, fz) in struts {
        let a = v3(fx0 * p.w, y_pl + 0.016, -0.16);
        let zb2 = -fz * p.l;
        let b = v3((fx1 * p.w).min(p.edge_x(zb2) - 0.18), y_pl + 0.016, zb2);
        g.tube(a, b, 0.02, 0.016, 8, M::Gold, 0);
    }
    // The pin field and its pins.
    g.boxed(
        v3(0.06, y_pl, -0.14),
        v3(p.w - 0.1, y_pl + 0.01, -0.03),
        0.004,
        M::Gold,
        0,
    );
    for row in 0..2 {
        let z = -0.06 - 0.04 * row as f32;
        for i in 0..72u32 {
            let x = 0.09 + (p.w - 0.24) * (i as f32 + 0.5 * row as f32) / 72.0;
            g.rod(
                v3(x, y_pl + 0.01, z),
                v3(x, y_pl + 0.032, z),
                0.0035,
                M::Nickel,
                0,
            );
        }
    }
    // The bridges, then the strings: tenor and treble straight back, the bass across.
    let y_st = super::shade::PLATE_Y + 0.025;
    let nt = 72;
    let xs = |i: usize| 0.33 * p.w + (p.w - 0.46 - 0.33 * p.w) * i as f32 / (nt - 1) as f32;
    let mut bridge = [V3::default(); 24];
    for (k, q) in bridge.iter_mut().enumerate() {
        let x = xs(k * (nt - 1) / 23);
        *q = v3(x, y_st - 0.012, p.z_at_x(x + 0.13) + 0.04);
    }
    g.hose(&bridge, 0.012, M::Spruce, 2);
    for i in 0..nt {
        let x = xs(i);
        let e = p.z_at_x(x + 0.13);
        g.line(
            v3(x, y_st, -0.14),
            v3(x, y_st, e + 0.01),
            0.0009,
            M::Steel,
            0.9,
        );
        g.line(
            v3(x + 0.004, y_st, -0.14),
            v3(x + 0.004, y_st, e + 0.01),
            0.0008,
            M::Steel,
            0.7,
        );
    }
    let nb = 20;
    let mut bb = [V3::default(); 20];
    for (i, q) in bb.iter_mut().enumerate() {
        let t = i as f32 / (nb - 1) as f32;
        let a = v3(0.06 * p.w + 0.22 * p.w * t, y_st + 0.02, -0.17);
        let ze = -p.l * (0.93 - 0.12 * t);
        let b = v3(
            (0.12 * p.w + 0.32 * p.w * t).min(p.edge_x(ze) - 0.12),
            y_st + 0.02,
            ze,
        );
        *q = b + v3(0.0, -0.016, 0.03);
        g.rod(a, b, 0.0024, M::Copper, 0);
    }
    g.hose(&bb, 0.011, M::Spruce, 2);
    // The dampers: felt blocks on their wooden heads, up to E6, and over the bass.
    for i in 0..nt {
        let x = xs(i);
        if x > 0.80 * p.w {
            break;
        }
        g.boxed(
            v3(x - 0.0055, y_st + 0.004, -0.32),
            v3(x + 0.0055, y_st + 0.022, -0.27),
            0.002,
            M::Felt,
            0,
        );
        g.boxed(
            v3(x - 0.005, y_st + 0.022, -0.315),
            v3(x + 0.005, y_st + 0.04, -0.285),
            0.002,
            M::Spruce,
            2,
        );
    }
    for i in 0..nb {
        let x = 0.06 * p.w + 0.22 * p.w * i as f32 / (nb - 1) as f32;
        g.boxed(
            v3(x - 0.006, y_st + 0.024, -0.34),
            v3(x + 0.006, y_st + 0.045, -0.29),
            0.002,
            M::Felt,
            0,
        );
    }
}

/// The keyboard end: the key slip, the cheeks, the fallboard and the 88 keys.
fn keyboard(g: &mut Geo, p: &Piano) {
    let c = (p.w - KEYS_W) * 0.5;
    let zf = 0.168;
    g.boxed(
        v3(c, 0.645, -0.02),
        v3(p.w - c, 0.733, zf),
        0.006,
        M::Case,
        0,
    );
    // The fallboard behind the keys, its polished front, a brass emblem.
    g.boxed(
        v3(c, KEY_Y - 0.01, -0.075),
        v3(p.w - c, 0.87, -0.012),
        0.008,
        M::Case,
        0,
    );
    g.boxed(
        v3(p.w * 0.5 - 0.06, 0.822, -0.013),
        v3(p.w * 0.5 + 0.06, 0.832, -0.009),
        0.002,
        M::Brass,
        0,
    );
    // The cheeks stand forward of the keys and the slip, from the keybed up.
    g.boxed(
        v3(0.0, 0.64, -0.08),
        v3(c, 0.815, zf + 0.022),
        0.01,
        M::Case,
        0,
    );
    g.boxed(
        v3(p.w - c, 0.64, -0.08),
        v3(p.w, 0.815, zf + 0.022),
        0.01,
        M::Case,
        0,
    );
    keys(g, c + 0.002, c + KEYS_W - 0.002, -0.01, 0.152, KEY_Y);
    // The red felt strip where the keys meet the fallboard.
    g.boxed(
        v3(c, KEY_Y - 0.004, -0.012),
        v3(p.w - c, KEY_Y + 0.004, -0.004),
        0.0,
        M::RedFelt,
        0,
    );
}

/// The 88 keys between `x0` and `x1`, from `z0` (back) to `z1` (front), their tops at `y`.
fn keys(g: &mut Geo, x0: f32, x1: f32, z0: f32, z1: f32, y: f32) {
    let kw = (x1 - x0) / 52.0;
    let gap = 0.0009;
    for j in 0..52 {
        let a = x0 + j as f32 * kw + gap;
        let b = x0 + (j + 1) as f32 * kw - gap;
        g.boxed(v3(a, y - 0.022, z0), v3(b, y, z1), 0.0012, M::Ivory, 0);
    }
    let bw = 0.0128;
    let zb = z0 + (z1 - z0) * 0.62;
    for j in 0..51 {
        let note = j % 7; // 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G
        if matches!(note, 1 | 4) {
            continue;
        }
        let x = x0 + (j + 1) as f32 * kw;
        g.boxed(
            v3(x - bw * 0.5, y - 0.004, z0),
            v3(x + bw * 0.5, y + 0.011, zb),
            0.0022,
            M::Ebony,
            0,
        );
    }
}

/// The desk, the lid at `alpha` (radians), its stick and hinges.
fn lid_and_desk(g: &mut Geo, p: &Piano, alpha: f32) {
    let y = p.y1 + 0.004;
    let cut = -0.33;
    let (s, c) = alpha.sin_cos();
    let thick = 0.024;
    let rot = |x: f32, z: f32, off: f32| v3(x * c - off * s, y + x * s + off * c, z);
    let zb = -p.l;
    let over = 0.01;
    let ex = |z: f32| p.edge_x(z) + over;
    // The main lid: its top, its underside, its edge all along the bentside and the tail.
    let (nu, nv) = (40, 10);
    g.patch(
        nu,
        nv,
        |u, v| {
            let z = cut + (zb - cut) * u;
            rot(ex(z) * v, z, thick)
        },
        false,
        2.7,
        1.5,
        M::Case,
        0,
    );
    g.patch(
        nu,
        nv,
        |u, v| {
            let z = cut + (zb - cut) * u;
            rot(ex(z) * v, z, 0.0)
        },
        true,
        2.7,
        1.5,
        M::Case,
        2,
    );
    g.patch(
        nu,
        1,
        |u, v| {
            let z = cut + (zb - cut) * u;
            rot(ex(z), z, thick * v)
        },
        true,
        2.7,
        0.02,
        M::Case,
        0,
    );
    g.patch(
        4,
        1,
        |u, v| rot(ex(cut) * u, cut, thick * v),
        false,
        1.5,
        0.02,
        M::Case,
        0,
    );
    // The front flap: on the lid when it is open, closed over the strings when it is shut.
    if alpha < 0.03 {
        g.slab(
            v3(0.0, y, -0.015),
            v3(0.0, 0.0, cut + 0.015),
            v3(ex(-0.1), 0.0, 0.0),
            v3(0.0, thick, 0.0),
            M::Case,
            0,
        );
    } else {
        let fl = rot(0.0, cut, thick);
        g.slab(
            fl,
            v3(0.0, 0.0, 0.3),
            rot(ex(cut) * 0.97, cut, thick) - fl,
            v3(-s, c, 0.0) * thick,
            M::Case,
            0,
        );
    }
    // The hinges along the spine.
    for z in [-0.5f32, -1.2, -2.0] {
        if -z < p.l - 0.2 {
            g.tube(
                v3(-0.004, y + 0.006, z - 0.05),
                v3(-0.004, y + 0.006, z + 0.05),
                0.008,
                0.008,
                8,
                M::Brass,
                0,
            );
        }
    }
    // The stick: from the rim's inner ledge near the bentside to the lid's underside a hand
    // inside its edge, leaning in (a real prop is never plumb).
    if alpha > 0.03 {
        let zs = -0.36 * p.l;
        let e = p.edge_x(zs);
        let base = v3(e - 0.07, p.y1, zs);
        let top = rot(e - 0.16, zs, 0.0);
        g.tube(base, top, 0.012, 0.011, 10, M::Case, 0);
        g.tube(
            top + v3(0.0, -0.02, 0.0),
            top,
            0.014,
            0.014,
            10,
            M::Brass,
            0,
        );
    }
    // The music desk on its rail, leaning back, its panel pierced with fretwork.
    let zd = -0.30;
    let dh = 0.30;
    let lean = 0.07;
    let (dx0, dx1) = (p.w * 0.5 - 0.34, p.w * 0.5 + 0.34);
    g.boxed(
        v3(dx0 - 0.02, p.y1, zd - 0.01),
        v3(dx1 + 0.02, p.y1 + 0.03, zd + 0.06),
        0.006,
        M::Case,
        0,
    );
    let o = v3(dx0, p.y1 + 0.03, zd);
    g.slab(
        o,
        v3(dx1 - dx0, 0.0, 0.0),
        v3(0.0, dh, -lean),
        v3(0.0, 0.0, -0.016),
        M::Case,
        1,
    );
    g.boxed(
        v3(dx0, p.y1 + 0.03, zd),
        v3(dx1, p.y1 + 0.055, zd + 0.035),
        0.005,
        M::Case,
        0,
    );
}

/// The upright, whole: its carcass between side cheeks, mouldings, keys, fallboard,
/// pedals, columns on toes and the top lid at `lid` (0 … 1 of its widest).
pub fn upright(g: &mut Geo, p: &Piano, lid: f32) {
    let (wd, h, d) = (p.w, p.y1, p.l);
    let c = (wd - KEYS_W) * 0.5;
    let side = 0.03;
    // The carcass, recessed between the cheeks; the plinth at its foot.
    g.boxed(
        v3(side, 0.05, -d + 0.02),
        v3(wd - side, h - 0.01, -0.04),
        0.004,
        M::Case,
        0,
    );
    g.boxed(
        v3(side, 0.0, -d + 0.02),
        v3(wd - side, 0.06, -0.025),
        0.008,
        M::Case,
        0,
    );
    for x0 in [0.0f32, wd - side] {
        g.boxed(
            v3(x0, 0.0, -d),
            v3(x0 + side, h - 0.005, 0.015),
            0.008,
            M::Case,
            0,
        );
        // The cheek's moulding: a raised bead round its panel.
        let xo = if x0 > 0.0 { wd + 0.002 } else { -0.002 };
        let bead = [
            v3(xo, 0.12, -d + 0.07),
            v3(xo, 0.12, -0.07),
            v3(xo, h - 0.12, -0.07),
            v3(xo, h - 0.12, -d + 0.07),
            v3(xo, 0.12, -d + 0.07),
        ];
        g.hose(&bead, 0.005, M::Case, 0);
    }
    // The lower front's raised frame round a recessed field.
    let lp = [
        v3(0.14, 0.12, -0.036),
        v3(wd - 0.14, 0.12, -0.036),
        v3(wd - 0.14, 0.58, -0.036),
        v3(0.14, 0.58, -0.036),
        v3(0.14, 0.12, -0.036),
    ];
    g.hose(&lp, 0.009, M::Case, 0);
    // The upper front, standing proud of the lower, its moulded panel.
    g.boxed(
        v3(side, 0.88, -0.04),
        v3(wd - side, h - 0.03, 0.0),
        0.008,
        M::Case,
        0,
    );
    let fr = [
        v3(0.16, 0.96, 0.001),
        v3(wd - 0.16, 0.96, 0.001),
        v3(wd - 0.16, h - 0.1, 0.001),
        v3(0.16, h - 0.1, 0.001),
        v3(0.16, 0.96, 0.001),
    ];
    g.hose(&fr, 0.006, M::Case, 0);
    // The pedals in their slot.
    g.boxed(
        v3(wd * 0.5 - 0.2, 0.05, -0.05),
        v3(wd * 0.5 + 0.2, 0.11, -0.03),
        0.004,
        M::Inner,
        0,
    );
    for dx in [-0.12f32, 0.0, 0.12] {
        g.tube(
            v3(wd * 0.5 + dx, 0.08, -0.04),
            v3(wd * 0.5 + dx, 0.068, 0.07),
            0.013,
            0.016,
            10,
            M::Brass,
            0,
        );
        g.boxed(
            v3(wd * 0.5 + dx - 0.022, 0.058, 0.02),
            v3(wd * 0.5 + dx + 0.022, 0.074, 0.09),
            0.006,
            M::Brass,
            0,
        );
    }
    // The toes and the turned columns under the key arms.
    for x0 in [0.02f32, wd - 0.11] {
        g.boxed(
            v3(x0, 0.0, -0.03),
            v3(x0 + 0.09, 0.065, 0.27),
            0.012,
            M::Case,
            0,
        );
        let (cx, cz) = (x0 + 0.045, 0.215);
        let prof = [
            (0.065, 0.03),
            (0.10, 0.034),
            (0.13, 0.024),
            (0.30, 0.03),
            (0.34, 0.036),
            (0.38, 0.026),
            (0.55, 0.026),
            (0.58, 0.034),
            (0.6, 0.03),
        ];
        for w2 in prof.windows(2) {
            let ((y0, r0), (y1, r1)) = (w2[0], w2[1]);
            g.tube(v3(cx, y0, cz), v3(cx, y1, cz), r0, r1, 14, M::Case, 0);
        }
        g.boxed(
            v3(x0, 0.6, 0.17),
            v3(x0 + 0.09, 0.645, 0.26),
            0.008,
            M::Case,
            0,
        );
    }
    // The keybed, the key slip, the arms, the keys.
    g.boxed(
        v3(side, 0.64, -0.03),
        v3(wd - side, 0.70, 0.245),
        0.006,
        M::Case,
        0,
    );
    g.boxed(
        v3(c, 0.69, 0.236),
        v3(wd - c, 0.738, 0.258),
        0.006,
        M::Case,
        0,
    );
    g.boxed(v3(0.0, 0.64, -0.03), v3(c, 0.835, 0.268), 0.012, M::Case, 0);
    g.boxed(
        v3(wd - c, 0.64, -0.03),
        v3(wd, 0.835, 0.268),
        0.012,
        M::Case,
        0,
    );
    keys(g, c + 0.002, c + KEYS_W - 0.002, 0.075, 0.234, KEY_Y);
    g.boxed(
        v3(c, KEY_Y - 0.004, 0.068),
        v3(wd - c, KEY_Y + 0.004, 0.077),
        0.0,
        M::RedFelt,
        0,
    );
    // The fallboard sloping back to the upper front, its rounded lip, the music ledge.
    g.slab(
        v3(c, KEY_Y + 0.004, 0.075),
        v3(wd - 2.0 * c, 0.0, 0.0),
        v3(0.0, 0.88 - KEY_Y - 0.004, -0.075),
        v3(0.0, 0.012, 0.012),
        M::Case,
        0,
    );
    g.boxed(
        v3(c, KEY_Y + 0.002, 0.07),
        v3(wd - c, KEY_Y + 0.032, 0.092),
        0.012,
        M::Case,
        0,
    );
    g.boxed(
        v3(c + 0.05, 0.88, 0.0),
        v3(wd - c - 0.05, 0.905, 0.05),
        0.008,
        M::Case,
        0,
    );
    g.boxed(
        v3(wd * 0.5 - 0.05, 0.832, 0.035),
        v3(wd * 0.5 + 0.05, 0.84, 0.045),
        0.002,
        M::Brass,
        0,
    );
    // The top lid, hinged at the back, a slab overhanging the case; what it shows inside
    // when it is up (the pins, the hammers' line).
    let beta = lid.clamp(0.0, 1.0) * UPRIGHT_TOP;
    let (s, co) = beta.sin_cos();
    if beta > 0.02 {
        g.boxed(
            v3(side, h - 0.02, -d + 0.04),
            v3(wd - side, h - 0.005, -0.05),
            0.0,
            M::Felt,
            0,
        );
        g.boxed(
            v3(0.08, h - 0.004, -d + 0.2),
            v3(wd - 0.08, h, -d + 0.36),
            0.003,
            M::Gold,
            0,
        );
        for row in 0..2 {
            for i in 0..56u32 {
                let x = 0.1 + (wd - 0.2) * (i as f32 + 0.5 * row as f32) / 56.0;
                let z = -d + 0.25 + 0.06 * row as f32;
                g.rod(v3(x, h, z), v3(x, h + 0.02, z), 0.0035, M::Nickel, 0);
            }
        }
        g.boxed(
            v3(0.12, h - 0.03, -0.14),
            v3(wd - 0.12, h - 0.005, -0.1),
            0.004,
            M::HammerFelt,
            0,
        );
        let base = v3(wd - 0.15, h, -0.15);
        let r = d - 0.15;
        g.tube(
            base,
            v3(wd - 0.15, h + r * s, -d + r * co),
            0.006,
            0.006,
            8,
            M::Brass,
            0,
        );
    }
    let hinge = v3(-0.015, h, -d);
    let along = v3(0.0, s, co) * (d + 0.03);
    g.slab(
        hinge,
        v3(wd + 0.03, 0.0, 0.0),
        along,
        v3(0.0, co, -s) * 0.026,
        M::Case,
        0,
    );
}
