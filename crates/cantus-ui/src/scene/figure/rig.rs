//! r42:arcus-upper — a figure's rig (ADR 0159): turns, a posed bone's transform, how a bone
//! is aimed, and the two-bone reach every limb is set by (an elbow, a knee, a finger's
//! middle joint). Moved from `bowed_human` and `bowed_hold` (which re-export them) so every
//! instrument's figure shares them.

use crate::scene::geo::{v, V3};

/// A rotation: the images of the x, y and z axes.
#[derive(Copy, Clone, Debug)]
pub struct Rot {
    pub x: V3,
    pub y: V3,
    pub z: V3,
}

impl Default for Rot {
    fn default() -> Rot {
        Rot::I
    }
}

impl Rot {
    pub const I: Rot = Rot {
        x: V3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        y: V3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        z: V3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
    };

    pub fn apply(&self, p: V3) -> V3 {
        self.x * p.x + self.y * p.y + self.z * p.z
    }

    pub fn t(&self) -> Rot {
        Rot {
            x: v(self.x.x, self.y.x, self.z.x),
            y: v(self.x.y, self.y.y, self.z.y),
            z: v(self.x.z, self.y.z, self.z.z),
        }
    }

    pub fn mul(&self, o: &Rot) -> Rot {
        Rot {
            x: self.apply(o.x),
            y: self.apply(o.y),
            z: self.apply(o.z),
        }
    }

    /// The right-handed frame whose y is `y` and whose x is `x` made square to it.
    pub fn frame(x: V3, y: V3) -> Rot {
        let y = y.norm();
        let mut xx = x - y * x.dot(y);
        if xx.len() < 1e-5 {
            xx = crate::scene::geo::basis(y).0;
        }
        let x = xx.norm();
        Rot {
            x,
            y,
            z: x.cross(y),
        }
    }

    /// The turn of `a` radians about the unit axis `u`.
    pub fn about(u: V3, a: f32) -> Rot {
        let (s, c) = a.sin_cos();
        let t = |p: V3| p * c + u.cross(p) * s + u * (u.dot(p) * (1.0 - c));
        Rot {
            x: t(v(1.0, 0.0, 0.0)),
            y: t(v(0.0, 1.0, 0.0)),
            z: t(v(0.0, 0.0, 1.0)),
        }
    }

    /// The shortest turn taking the unit `a` to the unit `b`.
    pub fn between(a: V3, b: V3) -> Rot {
        let c = a.dot(b).clamp(-1.0, 1.0);
        let ax = a.cross(b);
        if ax.len() < 1e-6 {
            return if c > 0.0 {
                Rot::I
            } else {
                Rot::about(crate::scene::geo::basis(a).0, std::f32::consts::PI)
            };
        }
        Rot::about(ax.norm(), c.acos())
    }
}

/// A posed bone: a rest point `p` goes to `at + rot·S(p − head)`, `S` stretching along the
/// bone's rest way `y0` by `k` (a finger's phalanx brought to the length of the pose it
/// takes).
#[derive(Copy, Clone, Debug, Default)]
pub struct Xf {
    pub head: V3,
    pub y0: V3,
    pub k: f32,
    pub rot: Rot,
    pub at: V3,
}

impl Xf {
    pub fn apply(&self, p: V3) -> V3 {
        let q = p - self.head;
        let q = q + self.y0 * (q.dot(self.y0) * (self.k - 1.0));
        self.at + self.rot.apply(q)
    }
}

/// How a bone is set: its head (none: where its parent carries it), its way, its x (none:
/// its parent's turn of its rest x), its length (none: its own).
#[derive(Copy, Clone, Debug, Default)]
pub struct Aim {
    pub head: Option<V3>,
    pub y: V3,
    pub x: Option<V3>,
    pub len: Option<f32>,
}

/// Where the middle joint of a two-bone limb from `s` to `w` falls (lengths `a`, `b`), bent
/// toward `pole` (an elbow, a knee).
pub fn two_bone(s: V3, w: V3, a: f32, b: f32, pole: V3) -> V3 {
    let dv = w - s;
    let dist = dv.len().clamp(1.0, a + b - 0.5);
    let u = dv.norm();
    let x = (a * a - b * b + dist * dist) / (2.0 * dist);
    let hh = (a * a - x * x).max(0.0).sqrt();
    let q = (pole - u * pole.dot(u)).norm();
    s + u * x + q * hh
}
