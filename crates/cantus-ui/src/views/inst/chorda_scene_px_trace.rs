//! r40:scene-px — true reflections: a bounding-volume hierarchy over the instrument's
//! triangles (and its wires as thin ribbons), so a gloss sees what really stands in its
//! mirror direction — the lid's underside the frame, the strings and the pins under it; the
//! case the bench, the legs, the keys; a chrome stand the drums beside it; a cymbal the
//! next one. A ray that meets nothing of the instrument sees the room (`Rig::env`). The
//! point a ray meets is lit once — the key with its shadow, the fill, the ambient, its
//! own coat's view of the room — never traced again.

use super::geo::{Geo, Tri, Vert};
use super::mesh::{v3, V3};

/// A node: its box, and either its first child (the second follows) or its triangles.
#[derive(Copy, Clone)]
struct Node {
    lo: [f32; 3],
    hi: [f32; 3],
    /// Inner: the left child's index. Leaf: the first index into `order`.
    first: u32,
    /// 0 for an inner node, else how many triangles the leaf holds.
    count: u32,
}

/// The hierarchy over a set of triangles: the instrument's (borrowed from its geometry,
/// never copied) and its wires' ribbons (its own).
pub struct Bvh<'a> {
    nodes: Vec<Node>,
    order: Vec<u32>,
    base: &'a [Tri],
    ribbons: Vec<Tri>,
}

/// Where a ray met a triangle: which, how far, its barycentrics.
#[derive(Copy, Clone, Debug)]
pub struct Hit {
    pub tri: u32,
    pub t: f32,
    pub b1: f32,
    pub b2: f32,
}

const LEAF: usize = 4;
const BINS: usize = 12;

fn bound(t: &Tri) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for v in &t.v {
        let p = [v.p.x, v.p.y, v.p.z];
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    (lo, hi)
}

fn area(lo: [f32; 3], hi: [f32; 3]) -> f32 {
    let d = [
        (hi[0] - lo[0]).max(0.0),
        (hi[1] - lo[1]).max(0.0),
        (hi[2] - lo[2]).max(0.0),
    ];
    d[0] * d[1] + d[1] * d[2] + d[2] * d[0]
}

fn grow(lo: &mut [f32; 3], hi: &mut [f32; 3], a: [f32; 3], b: [f32; 3]) {
    for i in 0..3 {
        lo[i] = lo[i].min(a[i]);
        hi[i] = hi[i].max(b[i]);
    }
}

impl<'a> Bvh<'a> {
    /// Triangle `i` (the instrument's, then the ribbons).
    #[inline]
    pub fn tri(&self, i: u32) -> &Tri {
        let i = i as usize;
        if i < self.base.len() {
            &self.base[i]
        } else {
            &self.ribbons[i - self.base.len()]
        }
    }

    /// The hierarchy over `geo`'s triangles from `split` (the instrument) and its wires.
    pub fn build(geo: &'a Geo, split: usize) -> Bvh<'a> {
        let base = &geo.tris[split..];
        let mut tris: Vec<Tri> = Vec::with_capacity(geo.lines.len() * 2);
        for l in &geo.lines {
            // A horizontal-ish ribbon three radii wide: what a mirror sees of a wire.
            let d = (l.b - l.a).norm();
            let side = d.cross(v3(0.0, 1.0, 0.0));
            let side = if side.len() < 0.3 {
                d.cross(v3(1.0, 0.0, 0.0))
            } else {
                side
            }
            .norm()
                * (l.r * 1.5);
            let n = side.cross(d).norm();
            let n = if n.y < 0.0 { -n } else { n };
            let vt = |p: V3| Vert {
                p,
                n,
                uv: [p.x, p.z],
            };
            // In pieces of at most 12 cm: a long diagonal wire in one box would overlap
            // half the frame's.
            let len = (l.b - l.a).len();
            let pieces = ((len / 0.12).ceil() as usize).max(1);
            for k in 0..pieces {
                let (pa, pb) = (
                    l.a.lerp(l.b, k as f32 / pieces as f32),
                    l.a.lerp(l.b, (k + 1) as f32 / pieces as f32),
                );
                let (a0, a1, b0, b1) = (vt(pa - side), vt(pa + side), vt(pb - side), vt(pb + side));
                tris.push(Tri {
                    v: [a0, b0, b1],
                    m: l.m,
                });
                tris.push(Tri {
                    v: [a0, b1, a1],
                    m: l.m,
                });
            }
        }
        let n = base.len() + tris.len();
        let boxes: Vec<([f32; 3], [f32; 3])> = base.iter().chain(&tris).map(bound).collect();
        let cent: Vec<[f32; 3]> = boxes
            .iter()
            .map(|(a, b)| {
                [
                    (a[0] + b[0]) * 0.5,
                    (a[1] + b[1]) * 0.5,
                    (a[2] + b[2]) * 0.5,
                ]
            })
            .collect();
        let mut order: Vec<u32> = (0..n as u32).collect();
        let mut nodes = Vec::with_capacity(2 * n / LEAF + 1);
        nodes.push(Node {
            lo: [0.0; 3],
            hi: [0.0; 3],
            first: 0,
            count: n as u32,
        });
        // An explicit stack: (node, first, count).
        let mut stack = vec![(0usize, 0usize, n)];
        while let Some((ni, first, count)) = stack.pop() {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            let (mut clo, mut chi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for &i in &order[first..first + count] {
                let (a, b) = boxes[i as usize];
                grow(&mut lo, &mut hi, a, b);
                let c = cent[i as usize];
                grow(&mut clo, &mut chi, c, c);
            }
            nodes[ni].lo = lo;
            nodes[ni].hi = hi;
            if count <= LEAF {
                nodes[ni].first = first as u32;
                nodes[ni].count = count as u32;
                continue;
            }
            // Binned surface-area split on the centroids' widest axis.
            let ax = (0..3)
                .max_by(|&a, &b| (chi[a] - clo[a]).total_cmp(&(chi[b] - clo[b])))
                .unwrap_or(0);
            let ext = chi[ax] - clo[ax];
            let mut mid = first + count / 2;
            if ext > 1e-7 {
                let bin_of = |i: u32| {
                    (((cent[i as usize][ax] - clo[ax]) / ext * BINS as f32) as usize).min(BINS - 1)
                };
                let mut bins = [([f32::MAX; 3], [f32::MIN; 3], 0usize); BINS];
                for &i in &order[first..first + count] {
                    let b = &mut bins[bin_of(i)];
                    let (a, c) = boxes[i as usize];
                    grow(&mut b.0, &mut b.1, a, c);
                    b.2 += 1;
                }
                let mut best = (f32::MAX, BINS / 2);
                for s in 1..BINS {
                    let (mut l0, mut l1, mut ln) = ([f32::MAX; 3], [f32::MIN; 3], 0);
                    let (mut r0, mut r1, mut rn) = ([f32::MAX; 3], [f32::MIN; 3], 0);
                    for (k, b) in bins.iter().enumerate() {
                        if b.2 == 0 {
                            continue;
                        }
                        if k < s {
                            grow(&mut l0, &mut l1, b.0, b.1);
                            ln += b.2;
                        } else {
                            grow(&mut r0, &mut r1, b.0, b.1);
                            rn += b.2;
                        }
                    }
                    if ln == 0 || rn == 0 {
                        continue;
                    }
                    let cost = area(l0, l1) * ln as f32 + area(r0, r1) * rn as f32;
                    if cost < best.0 {
                        best = (cost, s);
                    }
                }
                let sl = &mut order[first..first + count];
                let mut i = 0;
                let mut j = sl.len();
                while i < j {
                    if bin_of(sl[i]) < best.1 {
                        i += 1;
                    } else {
                        j -= 1;
                        sl.swap(i, j);
                    }
                }
                if i > 0 && i < count {
                    mid = first + i;
                }
            }
            let left = nodes.len();
            for _ in 0..2 {
                nodes.push(Node {
                    lo: [0.0; 3],
                    hi: [0.0; 3],
                    first: 0,
                    count: 0,
                });
            }
            nodes[ni].first = left as u32;
            nodes[ni].count = 0;
            stack.push((left, first, mid - first));
            stack.push((left + 1, mid, first + count - mid));
        }
        Bvh {
            nodes,
            order,
            base,
            ribbons: tris,
        }
    }

    /// The bytes it holds.
    pub fn bytes(&self) -> usize {
        self.nodes.len() * std::mem::size_of::<Node>()
            + self.order.len() * 4
            + self.ribbons.len() * std::mem::size_of::<Tri>()
    }

    /// The nearest triangle the ray `o + t d` meets with `t` in (`t0`, `t1`).
    pub fn hit(&self, o: V3, d: V3, t0: f32, t1: f32) -> Option<Hit> {
        if self.order.is_empty() {
            return None;
        }
        let inv = [1.0 / d.x, 1.0 / d.y, 1.0 / d.z];
        let org = [o.x, o.y, o.z];
        let slab = |n: &Node, tmax: f32| -> f32 {
            let (mut a, mut b) = (t0, tmax);
            for i in 0..3 {
                let (u, v) = ((n.lo[i] - org[i]) * inv[i], (n.hi[i] - org[i]) * inv[i]);
                let (u, v) = if u < v { (u, v) } else { (v, u) };
                a = a.max(u);
                b = b.min(v);
            }
            if a <= b {
                a
            } else {
                f32::MAX
            }
        };
        let mut best: Option<Hit> = None;
        let mut tmax = t1;
        let mut stack = [0u32; 64];
        let mut sp = 0usize;
        if slab(&self.nodes[0], tmax) == f32::MAX {
            return None;
        }
        stack[sp] = 0;
        sp += 1;
        while sp > 0 {
            sp -= 1;
            let n = &self.nodes[stack[sp] as usize];
            if n.count > 0 {
                for &ti in &self.order[n.first as usize..(n.first + n.count) as usize] {
                    if let Some((t, b1, b2)) = tri_hit(self.tri(ti), o, d, t0, tmax) {
                        tmax = t;
                        best = Some(Hit { tri: ti, t, b1, b2 });
                    }
                }
                continue;
            }
            let (l, r) = (n.first as usize, n.first as usize + 1);
            let (tl, tr) = (slab(&self.nodes[l], tmax), slab(&self.nodes[r], tmax));
            // The nearer child last, so it pops first.
            let (near, far, tn, tf) = if tl <= tr {
                (l, r, tl, tr)
            } else {
                (r, l, tr, tl)
            };
            if tf < f32::MAX && sp < 64 {
                stack[sp] = far as u32;
                sp += 1;
            }
            if tn < f32::MAX && sp < 64 {
                stack[sp] = near as u32;
                sp += 1;
            }
        }
        best
    }

    /// Whether anything of the instrument stands between `o` and `o + d t1`.
    pub fn blocked(&self, o: V3, d: V3, t1: f32) -> bool {
        self.hit(o, d, 1e-3, t1).is_some()
    }
}

/// Möller–Trumbore, both faces.
#[inline]
fn tri_hit(t: &Tri, o: V3, d: V3, t0: f32, t1: f32) -> Option<(f32, f32, f32)> {
    let (p0, p1, p2) = (t.v[0].p, t.v[1].p, t.v[2].p);
    let (e1, e2) = (p1 - p0, p2 - p0);
    let pv = d.cross(e2);
    let det = e1.dot(pv);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - p0;
    let u = s.dot(pv) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > t0 && t < t1).then_some((t, u, v))
}

/// The point, smooth normal, uv and grain of `h` on its triangle.
pub fn place(t: &Tri, h: &Hit) -> (V3, V3, [f32; 2], V3) {
    let (b1, b2) = (h.b1, h.b2);
    let b0 = 1.0 - b1 - b2;
    let v = &t.v;
    let p = v[0].p * b0 + v[1].p * b1 + v[2].p * b2;
    let n = (v[0].n * b0 + v[1].n * b1 + v[2].n * b2).norm();
    let uv = [
        v[0].uv[0] * b0 + v[1].uv[0] * b1 + v[2].uv[0] * b2,
        v[0].uv[1] * b0 + v[1].uv[1] * b1 + v[2].uv[1] * b2,
    ];
    let (du1, dv1) = (v[1].uv[0] - v[0].uv[0], v[1].uv[1] - v[0].uv[1]);
    let (du2, dv2) = (v[2].uv[0] - v[0].uv[0], v[2].uv[1] - v[0].uv[1]);
    let det = du1 * dv2 - du2 * dv1;
    let (a1, a2) = (v[1].p - v[0].p, v[2].p - v[0].p);
    let tg = if det.abs() > 1e-12 {
        (a1 * dv2 - a2 * dv1) * (1.0 / det)
    } else {
        a1
    };
    (p, n, uv, tg.norm())
}
