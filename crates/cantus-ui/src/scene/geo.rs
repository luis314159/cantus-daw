//! Vectors in centimetres (x to the viewer's right, y up, z toward the viewer), rings round
//! an axis, convex hulls, and the camera: look-at, the perspective divide, `fit` to a box,
//! and its mirror in the floor (port of the design's `scene/geo.py`).

use egui::Pos2;
use std::ops::{Add, Mul, Neg, Sub};

/// A point or a direction in the world, cm.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// A vector.
pub const fn v(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, k: f32) -> V3 {
        v(self.x * k, self.y * k, self.z * k)
    }
}

impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v(-self.x, -self.y, -self.z)
    }
}

impl V3 {
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: V3) -> V3 {
        v(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// Unit length (a zero vector stays as it is).
    pub fn norm(self) -> V3 {
        let n = self.len();
        if n > 0.0 {
            self * (1.0 / n)
        } else {
            self
        }
    }

    /// Mirrored in the floor (y = 0).
    pub fn flip_y(self) -> V3 {
        v(self.x, -self.y, self.z)
    }

    pub fn lerp(self, o: V3, t: f32) -> V3 {
        self + (o - self) * t
    }
}

/// The midpoint of points.
pub fn centre(pts: &[V3]) -> V3 {
    let n = pts.len().max(1) as f32;
    pts.iter().fold(V3::default(), |a, &p| a + p) * (1.0 / n)
}

/// A drum's axis: up, tilted by `tilt_deg` toward an (x, z) direction (default the drummer).
pub fn tilt_axis(tilt_deg: f32, toward: (f32, f32)) -> V3 {
    let a = tilt_deg.to_radians();
    let (dx, dz) = toward;
    let n = dx.hypot(dz);
    let n = if n > 0.0 { n } else { 1.0 };
    v(a.sin() * dx / n, a.cos(), a.sin() * dz / n).norm()
}

/// The basis of a plane perpendicular to `axis` (as `ring` builds it).
pub fn basis(axis: V3) -> (V3, V3) {
    let r = if axis.z.abs() < 0.9 {
        v(0.0, 0.0, 1.0)
    } else {
        v(1.0, 0.0, 0.0)
    };
    let e1 = axis.cross(r).norm();
    let e2 = axis.cross(e1);
    (e1, e2)
}

/// The direction at angle `a` in the plane `(e1, e2)`.
pub fn rad(e1: V3, e2: V3, a: f32) -> V3 {
    e1 * a.cos() + e2 * a.sin()
}

/// A circle in 3-D round an axis, `n` points from angle `start`.
pub fn ring(c: V3, axis: V3, radius: f32, n: usize, start: f32) -> Vec<V3> {
    let (e1, e2) = basis(axis);
    (0..n)
        .map(|k| {
            let a = start + std::f32::consts::TAU * k as f32 / n as f32;
            c + rad(e1, e2, a) * radius
        })
        .collect()
}

/// The convex hull of screen points (Andrew's monotone chain), counter-clockwise on screen.
pub fn hull(pts: &[Pos2]) -> Vec<Pos2> {
    let mut p: Vec<Pos2> = pts
        .iter()
        .filter(|q| q.x.is_finite() && q.y.is_finite())
        .map(|q| Pos2::new((q.x * 100.0).round() / 100.0, (q.y * 100.0).round() / 100.0))
        .collect();
    p.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let cr = |o: Pos2, a: Pos2, b: Pos2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut lo: Vec<Pos2> = Vec::with_capacity(p.len());
    for &q in &p {
        while lo.len() >= 2 && cr(lo[lo.len() - 2], lo[lo.len() - 1], q) <= 0.0 {
            lo.pop();
        }
        lo.push(q);
    }
    let mut hi: Vec<Pos2> = Vec::with_capacity(p.len());
    for &q in p.iter().rev() {
        while hi.len() >= 2 && cr(hi[hi.len() - 2], hi[hi.len() - 1], q) <= 0.0 {
            hi.pop();
        }
        hi.push(q);
    }
    lo.pop();
    hi.pop();
    lo.extend(hi);
    lo
}

/// The bounding box of screen points: (min x, min y, max x, max y).
pub fn bbox(pts: &[Pos2]) -> (f32, f32, f32, f32) {
    pts.iter().fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(a, b, c, d), p| (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
    )
}

/// Whether `p` is inside `poly` (even–odd).
pub fn inside(poly: &[Pos2], p: Pos2) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        if (a.y > p.y) != (b.y > p.y) {
            let dy = b.y - a.y;
            let dy = if dy != 0.0 { dy } else { 1e-9 };
            if p.x < (b.x - a.x) * (p.y - a.y) / dy + a.x {
                c = !c;
            }
        }
    }
    c
}

/// The camera: a look-at with the perspective divide (focal 1) and the screen transform
/// `fit` sets. `mirror` sees the world mirrored in the floor: the floor's reflection.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Camera {
    pub pos: V3,
    pub f: V3,
    pub r: V3,
    pub u: V3,
    pub s: f32,
    pub ox: f32,
    pub oy: f32,
    pub mirror: bool,
}

impl Camera {
    /// A camera aimed at `target` from azimuth `az_deg`, elevation `el_deg`, `dist` away.
    pub fn new(target: V3, az_deg: f32, el_deg: f32, dist: f32) -> Camera {
        let (az, el) = (az_deg.to_radians(), el_deg.to_radians());
        let pos = target + v(el.cos() * az.sin(), el.sin(), el.cos() * az.cos()) * dist;
        let f = (target - pos).norm();
        let r = f.cross(v(0.0, 1.0, 0.0)).norm();
        let u = r.cross(f);
        Camera {
            pos,
            f,
            r,
            u,
            s: 1.0,
            ox: 0.0,
            oy: 0.0,
            mirror: false,
        }
    }

    /// The same camera seeing the floor's reflection.
    pub fn mirrored(&self) -> Camera {
        Camera {
            mirror: true,
            ..*self
        }
    }

    /// The eye a facing test and the shading see from (mirrored in the floor for the
    /// reflection).
    pub fn eye(&self) -> V3 {
        if self.mirror {
            self.pos.flip_y()
        } else {
            self.pos
        }
    }

    fn world(&self, p: V3) -> V3 {
        if self.mirror {
            p.flip_y()
        } else {
            p
        }
    }

    /// Before the screen transform: (x, y, depth).
    pub fn raw(&self, p: V3) -> (f32, f32, f32) {
        let d = self.world(p) - self.pos;
        let z = d.dot(self.f).max(1e-3);
        (d.dot(self.r) / z, -d.dot(self.u) / z, z)
    }

    /// On screen.
    pub fn p(&self, p: V3) -> Pos2 {
        let (x, y, _) = self.raw(p);
        Pos2::new(self.ox + x * self.s, self.oy + y * self.s)
    }

    /// A planar polygon cut where it comes nearer the eye than `near` cm (Sutherland–Hodgman
    /// against the near plane): a floor or a wall that runs past the camera projects to a
    /// sane outline instead of points thrown far off the picture (which a rasteriser then
    /// drops whole).
    pub fn clip_near(&self, pts: &[V3], near: f32) -> Vec<V3> {
        let d = |p: V3| self.depth(p) - near;
        if pts.iter().all(|&p| d(p) >= 0.0) {
            return pts.to_vec();
        }
        let mut out = Vec::with_capacity(pts.len() + 2);
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let (da, db) = (d(a), d(b));
            if da >= 0.0 {
                out.push(a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                out.push(a.lerp(b, da / (da - db)));
            }
        }
        out
    }

    /// What the painter sorts by.
    pub fn depth(&self, p: V3) -> f32 {
        (self.world(p) - self.pos).dot(self.f)
    }

    /// A face at `point` with `normal` faces the eye.
    pub fn facing(&self, point: V3, normal: V3) -> bool {
        normal.dot(self.eye() - point) > 0.0
    }

    /// Scale and place so `pts` fill the box `(x, y, w, h)` with a margin, their box at
    /// `anchor` inside it.
    pub fn fit(&mut self, pts: &[V3], bx: (f32, f32, f32, f32), margin: f32, anchor: (f32, f32)) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &q in pts {
            let (x, y, _) = self.raw(q);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        let (bx0, by0, bw, bh) = bx;
        let (w, h) = ((x1 - x0).max(1e-6), (y1 - y0).max(1e-6));
        self.s = ((bw - 2.0 * margin) / w).min((bh - 2.0 * margin) / h);
        self.ox = bx0 + margin + (bw - 2.0 * margin - w * self.s) * anchor.0 - x0 * self.s;
        self.oy = by0 + margin + (bh - 2.0 * margin - h * self.s) * anchor.1 - y0 * self.s;
    }

    /// Un-project a screen point onto the plane through `at` with `normal` (the head's
    /// plane for a click's articulation).
    pub fn unproject(&self, sp: Pos2, at: V3, normal: V3) -> Option<V3> {
        let x = (sp.x - self.ox) / self.s;
        let y = (sp.y - self.oy) / self.s;
        let dir = (self.f + self.r * x - self.u * y).norm();
        let den = dir.dot(normal);
        if den.abs() < 1e-6 {
            return None;
        }
        let t = (at - self.pos).dot(normal) / den;
        (t > 0.0).then(|| self.pos + dir * t)
    }
}
