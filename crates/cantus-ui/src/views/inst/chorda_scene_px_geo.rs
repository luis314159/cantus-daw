//! r40:scene-px — the photographic renderer's geometry: closed meshes in metres (the scene
//! module's centimetres ÷ 100, its axes: x right, y up, z toward the viewer, the floor at
//! y = 0) with smooth normals, texture coordinates in metres and a material each; thin
//! lines (strings, cables). Every primitive the instruments are built from: quads, convex
//! and concave plates (ear-clipped), extruded outlines with rounded edges, tubes, lathes
//! (cymbals, bells, hoops, knobs), boxes.

use super::mat::{MatId, PMat};
use super::mesh::{v3, V3};

/// A point from the scene module's centimetres.
pub fn cm(x: f32, y: f32, z: f32) -> V3 {
    v3(x / 100.0, y / 100.0, z / 100.0)
}

/// A scene-module vector in metres.
pub fn of(p: crate::scene::geo::V3) -> V3 {
    cm(p.x, p.y, p.z)
}

/// A corner of a triangle.
#[derive(Copy, Clone, Debug)]
pub struct Vert {
    pub p: V3,
    pub n: V3,
    pub uv: [f32; 2],
    /// r42:arcus-upper — a realistic figure's skin texture coordinates (her u 0…1, his
    /// 2…3), or none (negative): the skin's fine detail sampled per sample (`place`).
    pub uv2: [f32; 2],
}

pub fn vt(p: V3, n: V3, uv: [f32; 2]) -> Vert {
    Vert {
        p,
        n,
        uv,
        uv2: [-1.0, -1.0],
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Tri {
    pub v: [Vert; 3],
    pub m: MatId,
}

/// A thin wire drawn anti-aliased against the depth (a string, a cable), radius `r` m.
#[derive(Copy, Clone, Debug)]
pub struct Line {
    pub a: V3,
    pub b: V3,
    pub r: f32,
    pub m: MatId,
    pub alpha: f32,
}

/// A rod as built (its axis, radius m, material, and its triangles `tris`): one too thin
/// on screen for the samples to hold its round is drawn as a tube stroke instead
/// (`raster::tubes`), its triangles kept for shadows and reflections.
#[derive(Copy, Clone, Debug)]
pub struct Rod {
    pub a: V3,
    pub b: V3,
    pub r: f32,
    pub m: MatId,
    pub tris: (u32, u32),
}

/// A scene's geometry and its materials.
#[derive(Default)]
pub struct Geo {
    pub tris: Vec<Tri>,
    pub lines: Vec<Line>,
    pub rods: Vec<Rod>,
    pub mats: Vec<PMat>,
    /// r42:arcus-hands — a tube thinner than a pixel is shaded as the average of its round
    /// (a pixel sees the whole of it): Arcus's strings, whose one mirror sample a pixel
    /// caught the room's vertical light strips in turn and dashed every string along the
    /// fingerboard. Off elsewhere (the pianos' pictures stay as they were).
    pub spread_thin: bool,
}

/// A box's face: its corners (as fractions along the box's edges) and its normal.
type Face = ([(f32, f32, f32); 4], V3);

/// A unit vector perpendicular to `a`.
pub fn perp(a: V3) -> V3 {
    let r = if a.y.abs() < 0.9 {
        v3(0.0, 1.0, 0.0)
    } else {
        v3(1.0, 0.0, 0.0)
    };
    a.cross(r).norm()
}

impl Geo {
    /// Register a material.
    pub fn mat(&mut self, m: PMat) -> MatId {
        self.mats.push(m);
        (self.mats.len() - 1) as MatId
    }

    pub fn tri(&mut self, a: Vert, b: Vert, c: Vert, m: MatId) {
        self.tris.push(Tri { v: [a, b, c], m });
    }

    /// A flat quad (corners in order), one normal, uv from `uv`.
    pub fn quad(&mut self, q: [V3; 4], n: V3, uv: impl Fn(V3) -> [f32; 2], m: MatId) {
        let v: [Vert; 4] = std::array::from_fn(|i| vt(q[i], n, uv(q[i])));
        self.tri(v[0], v[1], v[2], m);
        self.tri(v[0], v[2], v[3], m);
    }

    /// A quad whose uv run along its two edges (`size` metres each way).
    pub fn quad_uv(&mut self, q: [V3; 4], n: V3, m: MatId) {
        let (a, b) = ((q[1] - q[0]).len(), (q[3] - q[0]).len());
        let uv = [[0.0, 0.0], [a, 0.0], [a, b], [0.0, b]];
        let v: [Vert; 4] = std::array::from_fn(|i| vt(q[i], n, uv[i]));
        self.tri(v[0], v[1], v[2], m);
        self.tri(v[0], v[2], v[3], m);
    }

    /// A strip of quads between two rows of points with their normals.
    pub fn strip(&mut self, a: &[(V3, V3)], b: &[(V3, V3)], va: f32, vb: f32, m: MatId) {
        let mut u = 0.0;
        for i in 0..a.len().min(b.len()).saturating_sub(1) {
            let du = (a[i + 1].0 - a[i].0).len();
            let p = [
                vt(a[i].0, a[i].1, [u, va]),
                vt(a[i + 1].0, a[i + 1].1, [u + du, va]),
                vt(b[i + 1].0, b[i + 1].1, [u + du, vb]),
                vt(b[i].0, b[i].1, [u, vb]),
            ];
            self.tri(p[0], p[1], p[2], m);
            self.tri(p[0], p[2], p[3], m);
            u += du;
        }
    }

    /// A flat plate in the plane `y` over the outline (x, z) in metres, facing up or down;
    /// any simple polygon (ear-clipped). uv: (x, z).
    pub fn plate(&mut self, o: &[(f32, f32)], y: f32, up: bool, m: MatId) {
        let n = if up {
            v3(0.0, 1.0, 0.0)
        } else {
            v3(0.0, -1.0, 0.0)
        };
        for [a, b, c] in ear_clip(o) {
            let p = |i: usize| vt(v3(o[i].0, y, o[i].1), n, [o[i].0, o[i].1]);
            if up {
                self.tri(p(a), p(c), p(b), m);
            } else {
                self.tri(p(a), p(b), p(c), m);
            }
        }
    }

    /// A plate in any plane: the outline (x, z) mapped by `at` to the world, normal `n`.
    pub fn plate_map(&mut self, o: &[(f32, f32)], at: impl Fn(f32, f32) -> V3, n: V3, m: MatId) {
        for [a, b, c] in ear_clip(o) {
            let p = |i: usize| vt(at(o[i].0, o[i].1), n, [o[i].0, o[i].1]);
            self.tri(p(a), p(b), p(c), m);
        }
    }

    /// The sides of a closed outline (x, z), metres, from `y0` to `y1`, facing out (or in),
    /// with smooth normals where the outline bends gently; uv along the outline and up.
    pub fn wall(&mut self, o: &[(f32, f32)], y0: f32, y1: f32, outward: bool, m: MatId) {
        let ns = outline_normals(o, outward);
        let n = o.len();
        let mut u = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            let (a, b) = (o[i], o[j]);
            let du = (b.0 - a.0).hypot(b.1 - a.1);
            let (na, nb) = (ns[i].1, ns[j].0);
            let q = [
                vt(v3(a.0, y0, a.1), na, [u, y0]),
                vt(v3(b.0, y0, b.1), nb, [u + du, y0]),
                vt(v3(b.0, y1, b.1), nb, [u + du, y1]),
                vt(v3(a.0, y1, a.1), na, [u, y1]),
            ];
            if outward {
                self.tri(q[0], q[2], q[1], m);
                self.tri(q[0], q[3], q[2], m);
            } else {
                self.tri(q[0], q[1], q[2], m);
                self.tri(q[0], q[2], q[3], m);
            }
            u += du;
        }
    }

    /// A rounded edge round an outline whose flat face is at height `y`: the normal turns
    /// from up (or down, `under`) to out over a quarter circle of radius `r`, the outline
    /// itself the edge's outer line — the bevel a long crisp highlight runs along. The flat
    /// face meets it `r` inside the outline, the side `r` below (above) `y`.
    pub fn round_edge(&mut self, o: &[(f32, f32)], y: f32, r: f32, under: bool, m: MatId) {
        let ns = outline_normals(o, true);
        let steps = 4;
        let n = o.len();
        let up = if under { -1.0 } else { 1.0 };
        let ring = |k: usize| -> Vec<(V3, V3)> {
            let a = std::f32::consts::FRAC_PI_2 * k as f32 / steps as f32;
            (0..=n)
                .map(|i| {
                    let i = i % n;
                    let (px, pz) = o[i];
                    let nn = (ns[i].0 + ns[i].1).norm();
                    let c = v3(px - nn.x * r, y - up * r, pz - nn.z * r);
                    let p = c + nn * (r * a.sin()) + v3(0.0, up * r * a.cos(), 0.0);
                    (p, (nn * a.sin() + v3(0.0, up * a.cos(), 0.0)).norm())
                })
                .collect()
        };
        let rows: Vec<Vec<(V3, V3)>> = (0..=steps).map(ring).collect();
        for k in 0..steps {
            self.strip(&rows[k], &rows[k + 1], k as f32 * r, (k + 1) as f32 * r, m);
        }
    }

    /// A round tube from `a` to `b`, radius `r0` at `a` and `r1` at `b`, `sides` round,
    /// capped when `caps`. uv: (along, round) in metres.
    #[allow(clippy::too_many_arguments)]
    pub fn tube(&mut self, a: V3, b: V3, r0: f32, r1: f32, sides: usize, caps: bool, m: MatId) {
        let ax = b - a;
        let len = ax.len();
        if len < 1e-6 {
            return;
        }
        let d = ax * (1.0 / len);
        let e1 = perp(d);
        let e2 = d.cross(e1);
        let slope = (r0 - r1) / len;
        let n = sides.max(3);
        let ring = |k: usize| {
            let t = std::f32::consts::TAU * k as f32 / n as f32;
            e1 * t.cos() + e2 * t.sin()
        };
        for k in 0..n {
            let (ra, rb) = (ring(k), ring(k + 1));
            let (na, nb) = ((ra + d * slope).norm(), (rb + d * slope).norm());
            let (ua, ub) = (
                k as f32 / n as f32 * r0.max(r1) * std::f32::consts::TAU,
                (k + 1) as f32 / n as f32 * r0.max(r1) * std::f32::consts::TAU,
            );
            let q = [
                vt(a + ra * r0, na, [0.0, ua]),
                vt(a + rb * r0, nb, [0.0, ub]),
                vt(b + rb * r1, nb, [len, ub]),
                vt(b + ra * r1, na, [len, ua]),
            ];
            self.tri(q[0], q[1], q[2], m);
            self.tri(q[0], q[2], q[3], m);
            if caps {
                if r0 > 0.0 {
                    self.tri(
                        vt(a, -d, [0.0, 0.0]),
                        vt(a + rb * r0, -d, [0.0, 0.0]),
                        vt(a + ra * r0, -d, [0.0, 0.0]),
                        m,
                    );
                }
                if r1 > 0.0 {
                    self.tri(
                        vt(b, d, [len, 0.0]),
                        vt(b + ra * r1, d, [len, 0.0]),
                        vt(b + rb * r1, d, [len, 0.0]),
                        m,
                    );
                }
            }
        }
    }

    /// A rod: a capped tube of one radius, its sides from its size.
    pub fn rod(&mut self, a: V3, b: V3, r: f32, m: MatId) {
        let sides = if r < 0.006 {
            8
        } else if r < 0.02 {
            12
        } else {
            20
        };
        let first = self.tris.len() as u32;
        self.tube(a, b, r, r, sides, true, m);
        self.rods.push(Rod {
            a,
            b,
            r,
            m,
            tris: (first, self.tris.len() as u32),
        });
    }

    /// A surface of revolution round `axis` through `c`: the profile (radius, height along
    /// the axis) in metres, `segs` round. uv: (round, along the profile) in metres — the
    /// round direction is a lathe's grooves (`dP/du` its tangent).
    pub fn lathe(&mut self, c: V3, axis: V3, prof: &[(f32, f32)], segs: usize, m: MatId) {
        let ax = axis.norm();
        let e1 = perp(ax);
        let e2 = ax.cross(e1);
        let n = segs.max(6);
        // The profile's normals (in the (radial, axial) plane), smoothed at its joints.
        let np = prof.len();
        let mut pn = vec![(0.0f32, 0.0f32); np];
        for i in 0..np {
            let (p0, p1) = (prof[i.saturating_sub(1)], prof[(i + 1).min(np - 1)]);
            let (dr, dh) = (p1.0 - p0.0, p1.1 - p0.1);
            let l = dr.hypot(dh).max(1e-9);
            pn[i] = (dh / l, -dr / l);
        }
        let mut s = vec![0.0f32; np];
        for i in 1..np {
            s[i] = s[i - 1] + (prof[i].0 - prof[i - 1].0).hypot(prof[i].1 - prof[i - 1].1);
        }
        for k in 0..n {
            let (t0, t1) = (
                std::f32::consts::TAU * k as f32 / n as f32,
                std::f32::consts::TAU * (k + 1) as f32 / n as f32,
            );
            let (r0, r1) = (e1 * t0.cos() + e2 * t0.sin(), e1 * t1.cos() + e2 * t1.sin());
            for i in 0..np - 1 {
                let p = |rd: V3, j: usize, t: f32| {
                    let (rr, hh) = prof[j];
                    let (nr, nh) = pn[j];
                    vt(
                        c + rd * rr + ax * hh,
                        (rd * nr + ax * nh).norm(),
                        [t * rr.max(1e-3), s[j]],
                    )
                };
                let q = [
                    p(r0, i, t0),
                    p(r1, i, t1),
                    p(r1, i + 1, t1),
                    p(r0, i + 1, t0),
                ];
                self.tri(q[0], q[2], q[1], m);
                self.tri(q[0], q[3], q[2], m);
            }
        }
    }

    /// A box from corner `o` along edges `u`, `v`, `w` (any orientation), flat faces.
    pub fn boxed(&mut self, o: V3, u: V3, v: V3, w: V3, m: MatId) {
        let c = |a: f32, b: f32, d: f32| o + u * a + v * b + w * d;
        let faces: [Face; 6] = [
            ([(0., 0., 0.), (0., 1., 0.), (1., 1., 0.), (1., 0., 0.)], -w),
            ([(0., 0., 1.), (1., 0., 1.), (1., 1., 1.), (0., 1., 1.)], w),
            ([(0., 0., 0.), (1., 0., 0.), (1., 0., 1.), (0., 0., 1.)], -v),
            ([(0., 1., 0.), (0., 1., 1.), (1., 1., 1.), (1., 1., 0.)], v),
            ([(0., 0., 0.), (0., 0., 1.), (0., 1., 1.), (0., 1., 0.)], -u),
            ([(1., 0., 0.), (1., 1., 0.), (1., 1., 1.), (1., 0., 1.)], u),
        ];
        for (q, n) in faces {
            let p: [V3; 4] = std::array::from_fn(|k| c(q[k].0, q[k].1, q[k].2));
            // Wind each face so its normal is the right-hand one.
            let nn = n.norm();
            let g = (p[1] - p[0]).cross(p[2] - p[0]);
            let p = if g.dot(nn) < 0.0 {
                [p[0], p[3], p[2], p[1]]
            } else {
                p
            };
            self.quad_uv(p, nn, m);
        }
    }

    /// An axis-aligned box between two corners.
    pub fn aabb(&mut self, lo: V3, hi: V3, m: MatId) {
        let d = hi - lo;
        self.boxed(
            lo,
            v3(d.x, 0.0, 0.0),
            v3(0.0, d.y, 0.0),
            v3(0.0, 0.0, d.z),
            m,
        );
    }

    /// A wire.
    pub fn line(&mut self, a: V3, b: V3, r: f32, m: MatId, alpha: f32) {
        self.lines.push(Line { a, b, r, m, alpha });
    }
}

/// Each outline point's normals in plan: (the incoming edge's side, the outgoing edge's
/// side), equal (averaged) where the outline turns by less than ~35°.
pub fn outline_normals(o: &[(f32, f32)], outward: bool) -> Vec<(V3, V3)> {
    let n = o.len();
    // Counter-clockwise in (x, z) (positive area): the outside is each edge's right.
    let sgn = if signed_area(o) > 0.0 { 1.0 } else { -1.0 } * if outward { 1.0 } else { -1.0 };
    let edge_n = |i: usize| {
        let (a, b) = (o[i], o[(i + 1) % n]);
        v3((b.1 - a.1) * sgn, 0.0, -(b.0 - a.0) * sgn).norm()
    };
    (0..n)
        .map(|i| {
            let (ni, no) = (edge_n((i + n - 1) % n), edge_n(i));
            if ni.dot(no) > 0.82 {
                let s = (ni + no).norm();
                (s, s)
            } else {
                (ni, no)
            }
        })
        .collect()
}

/// Twice the signed area of a plan outline (x, z): positive counter-clockwise seen from
/// above (+y), x right, z toward the viewer.
pub fn signed_area(o: &[(f32, f32)]) -> f32 {
    let n = o.len();
    (0..n)
        .map(|i| {
            let (a, b) = (o[i], o[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f32>()
}

/// Triangulate a simple polygon by ear clipping (indices, the polygon's own winding).
pub fn ear_clip(o: &[(f32, f32)]) -> Vec<[usize; 3]> {
    let n = o.len();
    let mut out = Vec::with_capacity(n.saturating_sub(2));
    if n < 3 {
        return out;
    }
    let ccw = signed_area(o) > 0.0;
    let mut idx: Vec<usize> = (0..n).collect();
    let cross = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    };
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (o[ia], o[ib], o[ic]);
            let cr = cross(a, b, c);
            if (cr > 0.0) != ccw || cr.abs() < 1e-12 {
                continue;
            }
            let inside = idx.iter().any(|&j| {
                if j == ia || j == ib || j == ic {
                    return false;
                }
                let p = o[j];
                let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
                if ccw {
                    d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0
                } else {
                    d1 <= 0.0 && d2 <= 0.0 && d3 <= 0.0
                }
            });
            if inside {
                continue;
            }
            out.push([ia, ib, ic]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}
