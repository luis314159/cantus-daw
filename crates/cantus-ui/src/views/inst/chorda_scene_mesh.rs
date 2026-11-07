//! The scene's maths (ADR 0104): points and directions in metres (y up, the floor at
//! y = 0), the pinhole camera (and its mirror image in a polished floor), and colours in
//! linear light.

use egui::Pos2;

/// A point or a direction in the scene, in metres.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn norm(self) -> V3 {
        let l = self.len();
        if l > 1e-9 {
            self * (1.0 / l)
        } else {
            self
        }
    }
    pub fn lerp(self, o: V3, t: f32) -> V3 {
        self + (o - self) * t
    }
}

impl std::ops::Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl std::ops::Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl std::ops::Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, k: f32) -> V3 {
        v3(self.x * k, self.y * k, self.z * k)
    }
}
impl std::ops::Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}

/// The camera: where it stands, its axes, and how many points a metre makes at one metre.
#[derive(Copy, Clone, Debug)]
pub struct Camera {
    pub eye: V3,
    pub right: V3,
    pub up: V3,
    pub fwd: V3,
    pub k: f32,
    pub c: Pos2,
    /// Seen in the floor's mirror: every point drawn at its reflection (y → −y), faces
    /// and light judged from the mirrored eye.
    pub flip: bool,
}

impl Camera {
    /// From `eye` toward `target`, `k` points per metre at a metre, centred on `c`.
    pub fn look(eye: V3, target: V3, k: f32, c: Pos2) -> Camera {
        let fwd = (target - eye).norm();
        let right = fwd.cross(v3(0.0, 1.0, 0.0)).norm();
        let up = right.cross(fwd);
        Camera {
            eye,
            right,
            up,
            fwd,
            k,
            c,
            flip: false,
        }
    }

    /// The same camera seen in the floor's mirror.
    pub fn mirrored(&self) -> Camera {
        Camera {
            flip: true,
            ..*self
        }
    }

    fn m(&self, p: V3) -> V3 {
        if self.flip {
            v3(p.x, -p.y, p.z)
        } else {
            p
        }
    }

    /// The eye as the drawn geometry sees it.
    fn seen_from(&self) -> V3 {
        self.m(self.eye)
    }

    /// Distance along the view axis (never below a centimetre).
    pub fn depth(&self, p: V3) -> f32 {
        (self.m(p) - self.eye).dot(self.fwd).max(0.01)
    }

    pub fn project(&self, p: V3) -> Pos2 {
        let d = self.m(p) - self.eye;
        let z = d.dot(self.fwd).max(0.01);
        Pos2::new(
            self.c.x + self.k * d.dot(self.right) / z,
            self.c.y - self.k * d.dot(self.up) / z,
        )
    }

    /// Points per metre at `p`.
    pub fn scale_at(&self, p: V3) -> f32 {
        self.k / self.depth(p)
    }

    /// Whether a face at `p` with outward normal `n` faces the camera.
    pub fn faces(&self, p: V3, n: V3) -> bool {
        (self.seen_from() - p).dot(n) > 0.0
    }

    /// The unit direction from `p` to the eye.
    pub fn toward_eye(&self, p: V3) -> V3 {
        (self.seen_from() - p).norm()
    }
}

/// Colour helpers in linear light: a scene colour is lit and mixed as `[f32; 3]`, made a
/// (premultiplied) `Color32` only when it becomes a vertex.
pub type Lin = [f32; 3];

pub fn lin(c: [u8; 3]) -> Lin {
    let f = |u: u8| egui::ecolor::linear_f32_from_gamma_u8(u);
    [f(c[0]), f(c[1]), f(c[2])]
}

pub fn lmix(a: Lin, b: Lin, t: f32) -> Lin {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

pub fn lmul(a: Lin, b: Lin) -> Lin {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

pub fn lscale(a: Lin, k: f32) -> Lin {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn ladd(a: Lin, b: Lin) -> Lin {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
