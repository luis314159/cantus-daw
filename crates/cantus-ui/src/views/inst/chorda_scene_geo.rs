//! The scene's geometry (ADR 0104): triangles in metres with their normals, texture
//! coordinates and material, and thin lines (strings, joints) drawn anti-aliased after the
//! shading — what the baker (`raster.rs`) renders with a depth buffer, so what is in front
//! is in front however it was built.

use super::mesh::{v3, Camera, V3};

/// What a surface is made of; `shade.rs` lights each one.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum M {
    /// The case in its finish (lacquer or wood; uv along the grain, in metres).
    Case,
    /// The case's inside (the rim's inner face, the keybed): the finish, in shade.
    Inner,
    Ivory,
    Ebony,
    Gold,
    Spruce,
    /// Damper felt (dark) and hammer felt (cream).
    Felt,
    HammerFelt,
    RedFelt,
    Steel,
    Copper,
    Nickel,
    Brass,
    Chrome,
    /// Stands' black satin metal, cables' rubber.
    Stand,
    Rubber,
    MicBody,
    Grille,
    MicDark,
    Floor,
    Rug,
    Wall,
    Wall2,
    Fabric,
    Fabric2,
    WoodPanel,
    Glass,
    /// Light itself (a lamp, a window, a LED): `k` picks its colour.
    Glow,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Vert {
    pub p: V3,
    pub n: V3,
    pub uv: [f32; 2],
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Tri {
    pub v: [Vert; 3],
    pub mat: M,
    /// A material's parameter (a glow's colour, a fabric's shade).
    pub k: u8,
}

/// A line `r` metres thick from `a` to `b`, drawn over the shaded image where nothing is in
/// front of it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Line {
    pub a: V3,
    pub b: V3,
    pub r: f32,
    pub mat: M,
    /// Its opacity (a joint between boards is faint).
    pub alpha: f32,
}

/// The scene's triangles and lines, and the camera they are seen through (so a rod knows
/// whether it is thick enough on screen to be a tube or only a line).
pub struct Geo {
    pub tris: Vec<Tri>,
    pub lines: Vec<Line>,
    pub cam: Camera,
    /// Pixels per point of the image baked (for the rod's choice).
    pub px_per_pt: f32,
    /// Leave out the floor and what lies on it (the mirror's pass).
    pub mirror: bool,
}

impl Geo {
    pub fn new(cam: Camera, px_per_pt: f32) -> Geo {
        Geo {
            tris: Vec::new(),
            lines: Vec::new(),
            cam,
            px_per_pt,
            mirror: false,
        }
    }

    pub fn clear(&mut self, cam: Camera, px_per_pt: f32) {
        self.tris.clear();
        self.lines.clear();
        self.cam = cam;
        self.px_per_pt = px_per_pt;
    }

    pub fn tri(&mut self, a: Vert, b: Vert, c: Vert, mat: M, k: u8) {
        self.tris.push(Tri {
            v: [a, b, c],
            mat,
            k,
        });
    }

    /// A flat quad (corners in order), normal `n`, uv from the corners' positions by `uv`.
    pub fn quad(&mut self, q: [V3; 4], n: V3, uv: impl Fn(V3) -> [f32; 2], mat: M, k: u8) {
        let vt = |p: V3| Vert { p, n, uv: uv(p) };
        self.tri(vt(q[0]), vt(q[1]), vt(q[2]), mat, k);
        self.tri(vt(q[0]), vt(q[2]), vt(q[3]), mat, k);
    }

    /// A parametric surface `at(u, v)` on an `nu` × `nv` grid, smooth normals from its
    /// tangents (`∂u × ∂v`, flipped with `flip`), uv `(u·su, v·sv)` in metres.
    #[allow(clippy::too_many_arguments)]
    pub fn patch(
        &mut self,
        nu: usize,
        nv: usize,
        at: impl Fn(f32, f32) -> V3,
        flip: bool,
        su: f32,
        sv: f32,
        mat: M,
        k: u8,
    ) {
        let (nu, nv) = (nu.max(1), nv.max(1));
        let e = 1e-3;
        let vert = |u: f32, v: f32| {
            let p = at(u, v);
            let du = at((u + e).min(1.0), v) - at((u - e).max(0.0), v);
            let dv = at(u, (v + e).min(1.0)) - at(u, (v - e).max(0.0));
            let mut n = du.cross(dv).norm();
            if flip {
                n = -n;
            }
            Vert {
                p,
                n,
                uv: [u * su, v * sv],
            }
        };
        for j in 0..nv {
            for i in 0..nu {
                let (u0, u1) = (i as f32 / nu as f32, (i + 1) as f32 / nu as f32);
                let (v0, v1) = (j as f32 / nv as f32, (j + 1) as f32 / nv as f32);
                let a = vert(u0, v0);
                let b = vert(u1, v0);
                let c = vert(u1, v1);
                let d = vert(u0, v1);
                self.tri(a, b, c, mat, k);
                self.tri(a, c, d, mat, k);
            }
        }
    }

    /// A box between `lo` and `hi`, every face (the depth buffer keeps what is seen), uv in
    /// metres along each face; its edges rounded off by a bevel of `bevel` metres (a
    /// lacquered edge catches the light).
    pub fn boxed(&mut self, lo: V3, hi: V3, bevel: f32, mat: M, k: u8) {
        let b = bevel
            .min((hi.x - lo.x) * 0.3)
            .min((hi.y - lo.y) * 0.3)
            .min((hi.z - lo.z) * 0.3)
            .max(0.0);
        let (x0, x1, y0, y1, z0, z1) = (lo.x, hi.x, lo.y, hi.y, lo.z, hi.z);
        let uvx = |p: V3| [p.z, p.y];
        let uvy = |p: V3| [p.x, p.z];
        let uvz = |p: V3| [p.x, p.y];
        // The six faces, inset by the bevel.
        self.quad(
            [
                v3(x1, y0 + b, z0 + b),
                v3(x1, y0 + b, z1 - b),
                v3(x1, y1 - b, z1 - b),
                v3(x1, y1 - b, z0 + b),
            ],
            v3(1.0, 0.0, 0.0),
            uvx,
            mat,
            k,
        );
        self.quad(
            [
                v3(x0, y0 + b, z1 - b),
                v3(x0, y0 + b, z0 + b),
                v3(x0, y1 - b, z0 + b),
                v3(x0, y1 - b, z1 - b),
            ],
            v3(-1.0, 0.0, 0.0),
            uvx,
            mat,
            k,
        );
        self.quad(
            [
                v3(x0 + b, y1, z0 + b),
                v3(x1 - b, y1, z0 + b),
                v3(x1 - b, y1, z1 - b),
                v3(x0 + b, y1, z1 - b),
            ],
            v3(0.0, 1.0, 0.0),
            uvy,
            mat,
            k,
        );
        self.quad(
            [
                v3(x0 + b, y0, z1 - b),
                v3(x1 - b, y0, z1 - b),
                v3(x1 - b, y0, z0 + b),
                v3(x0 + b, y0, z0 + b),
            ],
            v3(0.0, -1.0, 0.0),
            uvy,
            mat,
            k,
        );
        self.quad(
            [
                v3(x0 + b, y0 + b, z1),
                v3(x1 - b, y0 + b, z1),
                v3(x1 - b, y1 - b, z1),
                v3(x0 + b, y1 - b, z1),
            ],
            v3(0.0, 0.0, 1.0),
            uvz,
            mat,
            k,
        );
        self.quad(
            [
                v3(x1 - b, y0 + b, z0),
                v3(x0 + b, y0 + b, z0),
                v3(x0 + b, y1 - b, z0),
                v3(x1 - b, y1 - b, z0),
            ],
            v3(0.0, 0.0, -1.0),
            uvz,
            mat,
            k,
        );
        if b <= 0.0 {
            return;
        }
        // The twelve bevels, their normals turning between the two faces (smooth).
        let edge = |g: &mut Geo, a0: V3, a1: V3, b0: V3, b1: V3, na: V3, nb: V3| {
            let (va0, va1) = (
                Vert {
                    p: a0,
                    n: na,
                    uv: [a0.x + a0.z, a0.y],
                },
                Vert {
                    p: a1,
                    n: na,
                    uv: [a1.x + a1.z, a1.y],
                },
            );
            let (vb0, vb1) = (
                Vert {
                    p: b0,
                    n: nb,
                    uv: [b0.x + b0.z, b0.y],
                },
                Vert {
                    p: b1,
                    n: nb,
                    uv: [b1.x + b1.z, b1.y],
                },
            );
            g.tri(va0, va1, vb1, mat, k);
            g.tri(va0, vb1, vb0, mat, k);
        };
        let (px, nx, py, ny, pz, nz) = (
            v3(1.0, 0.0, 0.0),
            v3(-1.0, 0.0, 0.0),
            v3(0.0, 1.0, 0.0),
            v3(0.0, -1.0, 0.0),
            v3(0.0, 0.0, 1.0),
            v3(0.0, 0.0, -1.0),
        );
        // Along x (the four edges between y and z faces).
        edge(
            self,
            v3(x0 + b, y1, z1 - b),
            v3(x1 - b, y1, z1 - b),
            v3(x0 + b, y1 - b, z1),
            v3(x1 - b, y1 - b, z1),
            py,
            pz,
        );
        edge(
            self,
            v3(x0 + b, y1, z0 + b),
            v3(x1 - b, y1, z0 + b),
            v3(x0 + b, y1 - b, z0),
            v3(x1 - b, y1 - b, z0),
            py,
            nz,
        );
        edge(
            self,
            v3(x0 + b, y0, z1 - b),
            v3(x1 - b, y0, z1 - b),
            v3(x0 + b, y0 + b, z1),
            v3(x1 - b, y0 + b, z1),
            ny,
            pz,
        );
        edge(
            self,
            v3(x0 + b, y0, z0 + b),
            v3(x1 - b, y0, z0 + b),
            v3(x0 + b, y0 + b, z0),
            v3(x1 - b, y0 + b, z0),
            ny,
            nz,
        );
        // Along z.
        edge(
            self,
            v3(x1 - b, y1, z0 + b),
            v3(x1 - b, y1, z1 - b),
            v3(x1, y1 - b, z0 + b),
            v3(x1, y1 - b, z1 - b),
            py,
            px,
        );
        edge(
            self,
            v3(x0 + b, y1, z0 + b),
            v3(x0 + b, y1, z1 - b),
            v3(x0, y1 - b, z0 + b),
            v3(x0, y1 - b, z1 - b),
            py,
            nx,
        );
        edge(
            self,
            v3(x1 - b, y0, z0 + b),
            v3(x1 - b, y0, z1 - b),
            v3(x1, y0 + b, z0 + b),
            v3(x1, y0 + b, z1 - b),
            ny,
            px,
        );
        edge(
            self,
            v3(x0 + b, y0, z0 + b),
            v3(x0 + b, y0, z1 - b),
            v3(x0, y0 + b, z0 + b),
            v3(x0, y0 + b, z1 - b),
            ny,
            nx,
        );
        // Along y.
        edge(
            self,
            v3(x1, y0 + b, z1 - b),
            v3(x1, y1 - b, z1 - b),
            v3(x1 - b, y0 + b, z1),
            v3(x1 - b, y1 - b, z1),
            px,
            pz,
        );
        edge(
            self,
            v3(x1, y0 + b, z0 + b),
            v3(x1, y1 - b, z0 + b),
            v3(x1 - b, y0 + b, z0),
            v3(x1 - b, y1 - b, z0),
            px,
            nz,
        );
        edge(
            self,
            v3(x0, y0 + b, z1 - b),
            v3(x0, y1 - b, z1 - b),
            v3(x0 + b, y0 + b, z1),
            v3(x0 + b, y1 - b, z1),
            nx,
            pz,
        );
        edge(
            self,
            v3(x0, y0 + b, z0 + b),
            v3(x0, y1 - b, z0 + b),
            v3(x0 + b, y0 + b, z0),
            v3(x0 + b, y1 - b, z0),
            nx,
            nz,
        );
        // The eight corners, where three bevels meet.
        for (cx, sx) in [(x0, -1.0f32), (x1, 1.0)] {
            for (cy, sy) in [(y0, -1.0f32), (y1, 1.0)] {
                for (cz, sz) in [(z0, -1.0f32), (z1, 1.0)] {
                    let n = v3(sx, sy, sz).norm();
                    let a = Vert {
                        p: v3(cx, cy - sy * b, cz - sz * b),
                        n,
                        uv: [cz, cy],
                    };
                    let bb = Vert {
                        p: v3(cx - sx * b, cy, cz - sz * b),
                        n,
                        uv: [cx, cz],
                    };
                    let c = Vert {
                        p: v3(cx - sx * b, cy - sy * b, cz),
                        n,
                        uv: [cx, cy],
                    };
                    self.tri(a, bb, c, mat, k);
                }
            }
        }
    }

    /// A slab: the parallelepiped from `o` along the edges `a`, `b` and `c` (a desk leaning
    /// back, a lid on its hinge), uv in metres along `a` and `b`.
    pub fn slab(&mut self, o: V3, a: V3, b: V3, c: V3, mat: M, k: u8) {
        let centre = o + (a + b + c) * 0.5;
        let face = |g: &mut Geo, p: V3, e1: V3, e2: V3| {
            let mut n = e1.cross(e2).norm();
            let mid = p + (e1 + e2) * 0.5;
            if (mid - centre).dot(n) < 0.0 {
                n = -n;
            }
            let (l1, l2) = (e1.len(), e2.len());
            let vt = |q: V3, u: f32, v: f32| Vert {
                p: q,
                n,
                uv: [u * l1, v * l2],
            };
            let (p0, p1, p2, p3) = (
                vt(p, 0.0, 0.0),
                vt(p + e1, 1.0, 0.0),
                vt(p + e1 + e2, 1.0, 1.0),
                vt(p + e2, 0.0, 1.0),
            );
            g.tri(p0, p1, p2, mat, k);
            g.tri(p0, p2, p3, mat, k);
        };
        face(self, o, a, b);
        face(self, o + c, a, b);
        face(self, o, a, c);
        face(self, o + b, a, c);
        face(self, o, b, c);
        face(self, o + a, b, c);
    }

    /// Points per metre on screen at `p` (in the baked image's pixels).
    pub fn px_at(&self, p: V3) -> f32 {
        self.cam.scale_at(p) * self.px_per_pt
    }

    /// A round rod from `a` to `b`, radius `r`: a tube where it is thick enough on screen,
    /// a line where it is thinner than about two pixels.
    pub fn rod(&mut self, a: V3, b: V3, r: f32, mat: M, k: u8) {
        let px = self.px_at(a.lerp(b, 0.5)) * 2.0 * r;
        if px < 2.2 {
            self.lines.push(Line {
                a,
                b,
                r,
                mat,
                alpha: 1.0,
            });
            return;
        }
        let sides = if px < 6.0 {
            6
        } else if px < 20.0 {
            10
        } else {
            16
        };
        self.tube(a, b, r, r, sides, mat, k);
    }

    /// A tube (a cone when `r0` ≠ `r1`) from `a` to `b`, `sides` around, its ends capped.
    #[allow(clippy::too_many_arguments)]
    pub fn tube(&mut self, a: V3, b: V3, r0: f32, r1: f32, sides: usize, mat: M, k: u8) {
        let axis = (b - a).norm();
        let helper = if axis.y.abs() < 0.9 {
            v3(0.0, 1.0, 0.0)
        } else {
            v3(1.0, 0.0, 0.0)
        };
        let s = axis.cross(helper).norm();
        let t = axis.cross(s).norm();
        let len = (b - a).len();
        let ring = |i: usize| {
            let ang = i as f32 / sides as f32 * std::f32::consts::TAU;
            s * ang.cos() + t * ang.sin()
        };
        for i in 0..sides {
            let (d0, d1) = (ring(i), ring(i + 1));
            let u0 = i as f32 / sides as f32 * 0.1;
            let u1 = (i + 1) as f32 / sides as f32 * 0.1;
            let va0 = Vert {
                p: a + d0 * r0,
                n: d0,
                uv: [u0, 0.0],
            };
            let va1 = Vert {
                p: a + d1 * r0,
                n: d1,
                uv: [u1, 0.0],
            };
            let vb0 = Vert {
                p: b + d0 * r1,
                n: d0,
                uv: [u0, len],
            };
            let vb1 = Vert {
                p: b + d1 * r1,
                n: d1,
                uv: [u1, len],
            };
            self.tri(va0, va1, vb1, mat, k);
            self.tri(va0, vb1, vb0, mat, k);
            let ca = Vert {
                p: a,
                n: -axis,
                uv: [0.0, 0.0],
            };
            let cb = Vert {
                p: b,
                n: axis,
                uv: [0.0, 0.0],
            };
            self.tri(
                ca,
                Vert { n: -axis, ..va1 },
                Vert { n: -axis, ..va0 },
                mat,
                k,
            );
            self.tri(cb, Vert { n: axis, ..vb0 }, Vert { n: axis, ..vb1 }, mat, k);
        }
    }

    /// A tube along a path (a cable, a bridge), radius `r`.
    pub fn hose(&mut self, pts: &[V3], r: f32, mat: M, k: u8) {
        for w in pts.windows(2) {
            self.rod(w[0], w[1], r, mat, k);
        }
    }

    /// A sphere (a grille's dome, a ball), `n` bands.
    pub fn sphere(&mut self, c: V3, r: f32, n: usize, mat: M, k: u8) {
        let n = n.max(4);
        let at = |u: f32, v: f32| {
            let th = u * std::f32::consts::TAU;
            let ph = v * std::f32::consts::PI;
            c + v3(ph.sin() * th.cos(), -ph.cos(), ph.sin() * th.sin()) * r
        };
        self.patch(2 * n, n, at, true, r * 6.3, r * 3.1, mat, k);
    }

    /// A disc of radius `r` in the plane through `c` with normal `n`.
    pub fn disc(&mut self, c: V3, n: V3, r: f32, segs: usize, mat: M, k: u8) {
        let helper = if n.y.abs() < 0.9 {
            v3(0.0, 1.0, 0.0)
        } else {
            v3(1.0, 0.0, 0.0)
        };
        let s = n.cross(helper).norm();
        let t = n.cross(s).norm();
        let cv = Vert {
            p: c,
            n,
            uv: [0.0, 0.0],
        };
        for i in 0..segs {
            let a0 = i as f32 / segs as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / segs as f32 * std::f32::consts::TAU;
            let p0 = c + (s * a0.cos() + t * a0.sin()) * r;
            let p1 = c + (s * a1.cos() + t * a1.sin()) * r;
            self.tri(
                cv,
                Vert {
                    p: p0,
                    n,
                    uv: [p0.x, p0.z],
                },
                Vert {
                    p: p1,
                    n,
                    uv: [p1.x, p1.z],
                },
                mat,
                k,
            );
        }
    }

    /// A thin line (a string, a joint) at `alpha`.
    pub fn line(&mut self, a: V3, b: V3, r: f32, mat: M, alpha: f32) {
        self.lines.push(Line {
            a,
            b,
            r,
            mat,
            alpha,
        });
    }
}
