//! r40:scene-px — the photographic baker: a depth-buffered rasteriser writes which
//! triangle each sample sees (the place on it is found again from the sample's ray, so a
//! sample costs eight bytes, not sixteen); the key's soft shadow from a depth-only map with
//! a blocker search (contact-hard, wide where the occluder stands high); every sample shaded
//! (`shade.rs`) with its occlusion from the depth around it and the polished floor's mirror
//! image, and summed straight into its pixel; the wires drawn anti-aliased against the
//! pixels' depth; then depth of field behind the instrument, bloom, a filmic curve, a
//! vignette and a dither. Rows go out in bands to the bake's threads; the bits never depend
//! on how many there are. Nothing here touches the UI thread.

use super::geo::{Geo, Rod, Tri};
use super::light::Rig;
use super::mat::{noise, Kind, PMat, Tex};
use super::mesh::{ladd, lmix, lscale, Camera, Lin, V3};
use super::shade::{reflection, shade};
use egui::Pos2;

/// How finely to bake.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Quality {
    /// Samples a pixel side (1 … 3).
    pub ss: usize,
    /// Occlusion, the floor's mirror, depth of field and bloom.
    pub full: bool,
    /// The key's shadow map's side.
    pub shadow: usize,
    /// True reflections (the instrument mirrored in itself); off, a gloss sees the room.
    pub trace: bool,
}

/// What a bake cost.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub ms: f64,
    /// Each pass's time, ms: the key's shadow map, the floor's mirror, the reflections'
    /// hierarchy, the samples' depth and triangles, the shading (with its shadow lookups
    /// and traced reflections), the wires, depth of field and bloom with the tone curve.
    pub passes: [f64; 7],
    /// The largest the bake's buffers were at once (bytes).
    pub peak: usize,
    pub tris: usize,
}

/// A triangle projected into the samples.
#[derive(Copy, Clone)]
struct PTri {
    x: [f32; 3],
    y: [f32; 3],
    iz: [f32; 3],
    lo: Pos2,
    hi: Pos2,
}

fn world(cam: &Camera, p: V3) -> V3 {
    if cam.flip {
        V3 { y: -p.y, ..p }
    } else {
        p
    }
}

/// The nearest a sample may be (m): a triangle reaching nearer is clipped there.
const NEAR: f32 = 0.05;

fn ptri(cam: &Camera, p: [V3; 3]) -> Option<PTri> {
    let mut x = [0.0; 3];
    let mut y = [0.0; 3];
    let mut iz = [0.0; 3];
    for i in 0..3 {
        let z = (world(cam, p[i]) - cam.eye).dot(cam.fwd).max(NEAR);
        let q = cam.project(p[i]);
        x[i] = q.x;
        y[i] = q.y;
        iz[i] = 1.0 / z;
    }
    if !(x.iter().chain(&y).all(|v| v.is_finite())) {
        return None;
    }
    let lo = Pos2::new(x[0].min(x[1]).min(x[2]), y[0].min(y[1]).min(y[2]));
    let hi = Pos2::new(x[0].max(x[1]).max(x[2]), y[0].max(y[1]).max(y[2]));
    Some(PTri { x, y, iz, lo, hi })
}

/// Each triangle projected (its index, its projection), clipped against the near plane: a
/// floor running under the camera still covers the picture to its foot.
fn project(cam: &Camera, tris: &[Tri], skip: &[bool]) -> Vec<(u32, PTri)> {
    let mut out = Vec::with_capacity(tris.len());
    for (i, t) in tris.iter().enumerate() {
        if skip.get(i).copied().unwrap_or(false) {
            continue;
        }
        let p: [V3; 3] = std::array::from_fn(|k| t.v[k].p);
        let z: [f32; 3] = std::array::from_fn(|k| (world(cam, p[k]) - cam.eye).dot(cam.fwd));
        let inside = z.iter().filter(|&&d| d >= NEAR).count();
        if inside == 3 {
            if let Some(q) = ptri(cam, p) {
                out.push((i as u32, q));
            }
            continue;
        }
        if inside == 0 {
            continue;
        }
        // Sutherland–Hodgman against z = NEAR.
        let mut poly: Vec<V3> = Vec::with_capacity(4);
        for k in 0..3 {
            let (a, b) = (k, (k + 1) % 3);
            if z[a] >= NEAR {
                poly.push(p[a]);
            }
            if (z[a] >= NEAR) != (z[b] >= NEAR) {
                let t = (NEAR - z[a]) / (z[b] - z[a]);
                poly.push(p[a] + (p[b] - p[a]) * t);
            }
        }
        for k in 1..poly.len().saturating_sub(1) {
            if let Some(q) = ptri(cam, [poly[0], poly[k], poly[k + 1]]) {
                out.push((i as u32, q));
            }
        }
    }
    out
}

/// A projected triangle's card (r42:arcus-upper): its texture, its corners' coordinates
/// and (r42:arcus-hair) their degrees in from the hairline (the hair feathers out over the
/// front edge) — the samples its coverage leaves bare are not its. The scalp under the
/// cards (no texture) is cut the same way at its edge, so it thins into the skin.
#[derive(Copy, Clone)]
struct Cut {
    card: Option<&'static crate::scene::figure::cards::Card>,
    uv: [[f32; 2]; 3],
    line: [f32; 3],
    who: u8,
}

/// Each projected triangle's card, if it is one.
fn cuts(geo: &Geo, pt: &[(u32, PTri)]) -> Vec<Option<Cut>> {
    pt.iter()
        .map(|&(i, _)| {
            let t = &geo.tris[i as usize];
            match geo.mats[t.m as usize].tex {
                Tex::Scalp(who) => Some(Cut {
                    card: None,
                    uv: t.v.map(|v| v.uv),
                    line: t.v.map(|v| v.uv[1]),
                    who,
                }),
                Tex::Card(k) => crate::scene::figure::cards::card(k).map(|card| Cut {
                    card: Some(card),
                    uv: t.v.map(|v| v.uv),
                    // (A hair card's degrees in from the hairline ride in `uv2`; a brow's or a
                    // lash's is whole: far inside.)
                    line: t.v.map(|v| {
                        if k % 3 == 0 && v.uv2[1] > 0.5 {
                            v.uv2[0]
                        } else {
                            90.0
                        }
                    }),
                    who: k / 3,
                }),
                _ => None,
            }
        })
        .collect()
}

/// r42:arcus-hair — alpha to coverage: the level a sample's coverage must pass, an ordered
/// 4 × 4 pattern (each 2 × 2 block of it — a pixel's samples at two a side — four evenly
/// spaced levels), turned by the triangle so that two layers of hair at half coverage
/// cover three quarters, not the same half twice.
fn coverage_level(px: usize, py: usize, tri: u32) -> f32 {
    const BAYER: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
    let b = (BAYER[(py & 3) * 4 + (px & 3)] as f32 + 0.5) / 16.0;
    let h = (tri.wrapping_mul(0x9E37_79B9) >> 8) as f32 / (1u32 << 24) as f32;
    (b + h).fract()
}

/// Rasterise into rows `y0..y1` of a `w`-wide buffer: depth and (when given) triangle.
fn raster_rows(
    pt: &[(u32, PTri)],
    cut: &[Option<Cut>],
    w: usize,
    y0: usize,
    y1: usize,
    depth: &mut [f32],
    mut tri: Option<&mut [u32]>,
) {
    for (j, &(i, ref t)) in pt.iter().enumerate() {
        let card = cut.get(j).copied().flatten();
        // (A card casts no shadow — the shadow map is the pass without triangles: its
        // strands' shadows on the scalp under them read as dark patches in the hair. The
        // scalp casts its own, whole.)
        let card = match card {
            Some(c) if tri.is_none() && c.card.is_some() => continue,
            Some(_) if tri.is_none() => None,
            c => c,
        };
        if t.hi.y < y0 as f32 || t.lo.y > y1 as f32 || t.hi.x < 0.0 || t.lo.x > w as f32 {
            continue;
        }
        let area = (t.x[1] - t.x[0]) * (t.y[2] - t.y[0]) - (t.x[2] - t.x[0]) * (t.y[1] - t.y[0]);
        if area.abs() < 1e-8 {
            continue;
        }
        let inv = 1.0 / area;
        // (r42:arcus-hair: a card's texture read at the level its texels reach a sample.)
        // (A brow's or a lash's hairs, a sample wide, read a level softer: their coverage
        // a soft tone, not a line of dots.)
        let lod = card
            .and_then(|c| {
                c.card
                    .map(|t| t.lod(c.uv, area * 0.5) + if t.hair { 0.0 } else { 3.0 })
            })
            .unwrap_or(0.0);
        let ya = ((t.lo.y - 0.5).ceil().max(y0 as f32)) as usize;
        let yb = ((t.hi.y - 0.5).floor() + 1.0).min(y1 as f32).max(0.0) as usize;
        let xa = ((t.lo.x - 0.5).ceil().max(0.0)) as usize;
        let xb = ((t.hi.x - 0.5).floor() + 1.0).min(w as f32).max(0.0) as usize;
        let spans = area.abs() >= 1.0;
        for py in ya..yb {
            let sy = py as f32 + 0.5;
            let row = (py - y0) * w;
            let (xa, xb) = if spans {
                span(t, inv, sy, xa, xb)
            } else {
                (xa, xb)
            };
            for px in xa..xb {
                let sx = px as f32 + 0.5;
                let w0 = ((t.x[1] - sx) * (t.y[2] - sy) - (t.x[2] - sx) * (t.y[1] - sy)) * inv;
                let w1 = ((t.x[2] - sx) * (t.y[0] - sy) - (t.x[0] - sx) * (t.y[2] - sy)) * inv;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-5 || w1 < -1e-5 || w2 < -1e-5 {
                    continue;
                }
                let z = 1.0 / (w0 * t.iz[0] + w1 * t.iz[1] + w2 * t.iz[2]);
                let k = row + px;
                if z < depth[k] {
                    if let Some(c) = card {
                        // Perspective-correct: the coordinates over depth, interpolated.
                        let (a, b, d) = (w0 * t.iz[0], w1 * t.iz[1], w2 * t.iz[2]);
                        let uv = c.uv;
                        let u = (a * uv[0][0] + b * uv[1][0] + d * uv[2][0]) * z;
                        let v = (a * uv[0][1] + b * uv[1][1] + d * uv[2][1]) * z;
                        let line = (a * c.line[0] + b * c.line[1] + d * c.line[2]) * z;
                        // (r42:arcus-hair: its coverage over the pixel's samples, not cut
                        // at half: soft strands, soft edges.)
                        // (The scalp's pattern the same over all its triangles: a smooth ramp,
                        // never a seam of dither where two meet.)
                        let (cover, seed) = match c.card {
                            Some(tex) if tex.hair => (
                                crate::scene::figure::cards::thin(
                                    tex.alpha([u, v], lod),
                                    crate::scene::figure::cards::feather(c.who, line),
                                ),
                                i,
                            ),
                            // (A brow's or a lash's softer level's coverage, its hairs' tone
                            // kept: thin, it fell to a few dots.)
                            Some(tex) => ((tex.alpha([u, v], lod) * 3.0).min(1.0), i),
                            None => (crate::scene::figure::cards::scalp_cover(c.who, line, u), 0),
                        };
                        if cover <= coverage_level(px, py, seed) {
                            continue;
                        }
                    }
                    depth[k] = z;
                    if let Some(tr) = tri.as_deref_mut() {
                        tr[k] = i;
                    }
                }
            }
        }
    }
}

/// The samples of row `sy` that can be inside `t`: each edge function is linear along the
/// row, so the inside is an interval, widened by two samples (every sample in it still
/// takes the exact test).
fn span(t: &PTri, inv: f32, sy: f32, xa: usize, xb: usize) -> (usize, usize) {
    let (inv, sy) = (inv as f64, sy as f64);
    let (x, y) = (t.x.map(|v| v as f64), t.y.map(|v| v as f64));
    let c0 = (x[1] * (y[2] - sy) - x[2] * (y[1] - sy)) * inv;
    let d0 = (y[1] - y[2]) * inv;
    let c1 = (x[2] * (y[0] - sy) - x[0] * (y[2] - sy)) * inv;
    let d1 = (y[2] - y[0]) * inv;
    let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
    for (c, d) in [(c0, d0), (c1, d1), (1.0 - c0 - c1, -d0 - d1)] {
        let at = (-1e-5 - c) / d;
        if d > 0.0 {
            lo = lo.max(at);
        } else if d < 0.0 {
            hi = hi.min(at);
        } else if c < -1e-5 {
            return (xa, xa);
        }
    }
    if lo.is_nan() || hi.is_nan() || lo > hi {
        return (xa, xa);
    }
    let a = (lo - 0.5).floor() - 2.0;
    let b = (hi - 0.5).ceil() + 3.0;
    let a = if a.is_finite() {
        a.clamp(xa as f64, xb as f64) as usize
    } else {
        xa
    };
    let b = if b.is_finite() {
        b.clamp(xa as f64, xb as f64) as usize
    } else {
        xb
    };
    (a, b.max(a))
}

/// The bake's workers: with more than one asked for, the app's bake pool (below normal
/// priority, kept between bakes, `pool.rs`) runs every part — the caller only waits; with
/// one, the caller bakes alone (tests, the bits' check).
pub struct Crew {
    pool: Option<std::sync::Arc<rayon::ThreadPool>>,
    threads: usize,
}

impl Crew {
    pub fn new(threads: usize) -> Crew {
        let pool = if threads > 1 {
            super::pool::pool()
        } else {
            None
        };
        let threads = if pool.is_some() { threads } else { 1 };
        Crew { pool, threads }
    }

    fn run(&self, parts: usize, work: &(impl Fn() + Sync)) {
        let n = self.threads.min(parts.max(1));
        match &self.pool {
            Some(pool) if n > 1 => pool.scope(|s| {
                for _ in 0..n {
                    s.spawn(|_| work());
                }
            }),
            _ => work(),
        }
    }
}

const BAND: usize = 8;

/// Run `f(first_row, end_row, chunk_a, chunk_b)` over the rows (each `w` long) of two
/// buffers, in bands shared among the crew.
fn rows2<A: Send, B: Send>(
    h: usize,
    w: usize,
    crew: &Crew,
    a: &mut [A],
    b: &mut [B],
    f: impl Fn(usize, usize, &mut [A], &mut [B]) + Sync,
) {
    if h == 0 || w == 0 {
        return;
    }
    let n = BAND * w;
    let bands = std::sync::Mutex::new(
        a[..h * w]
            .chunks_mut(n)
            .zip(b[..h * w].chunks_mut(n))
            .enumerate(),
    );
    let work = || loop {
        let next = bands.lock().map(|mut b| b.next()).ok().flatten();
        let Some((i, (a, b))) = next else { break };
        let y = i * BAND;
        f(y, y + a.len() / w, a, b);
    };
    crew.run(h.div_ceil(BAND), &work);
}

/// Run `f(first_row, end_row, chunk)` over the rows of `a`, in bands shared among the crew.
fn rows1<A: Send>(
    h: usize,
    w: usize,
    crew: &Crew,
    a: &mut [A],
    f: impl Fn(usize, usize, &mut [A]) + Sync,
) {
    if h == 0 || w == 0 {
        return;
    }
    let bands = std::sync::Mutex::new(a[..h * w].chunks_mut(BAND * w).enumerate());
    let work = || loop {
        let next = bands.lock().map(|mut b| b.next()).ok().flatten();
        let Some((i, a)) = next else { break };
        let y = i * BAND;
        f(y, y + a.len() / w, a);
    };
    crew.run(h.div_ceil(BAND), &work);
}

/// A depth buffer (and which triangle each sample sees) of `geo` from `cam`.
fn gbuf(
    geo: &Geo,
    cam: &Camera,
    w: usize,
    h: usize,
    crew: &Crew,
    tris: bool,
    skip: &[bool],
) -> (Vec<f32>, Vec<u32>) {
    let pt = project(cam, &geo.tris, skip);
    let cut = cuts(geo, &pt);
    let mut depth = vec![f32::MAX; w * h];
    let mut tri = if tris {
        vec![u32::MAX; w * h]
    } else {
        Vec::new()
    };
    if tris {
        rows2(h, w, crew, &mut depth, &mut tri, |y0, y1, d, t| {
            raster_rows(&pt, &cut, w, y0, y1, d, Some(t))
        });
    } else {
        rows1(h, w, crew, &mut depth, |y0, y1, d| {
            raster_rows(&pt, &cut, w, y0, y1, d, None)
        });
    }
    (depth, tri)
}

/// r42:arcus-hair — whether `m` is a realistic figure's (skin, cloth, hair, eyes).
fn figure_mat(m: &super::mat::PMat) -> bool {
    matches!(
        m.tex,
        Tex::Skin | Tex::Velvet | Tex::Hair | Tex::Card(_) | Tex::Scalp(_) | Tex::Eye
    )
}

/// What a triangle's grain is read from.
#[derive(Copy, Clone, PartialEq)]
enum Grain {
    /// `dP/du` from its corners' coordinates.
    Uv,
    /// r42:arcus-upper — hair, or the scalp under the cards: the in-plane way its `u` (the
    /// across coordinate) grows.
    Hair,
    /// r42:arcus-hair — a card (`figure::cards`): a hair card's strands run the way its
    /// texture's maps say.
    Card(u8),
    /// r42:arcus-upper — a realistic figure's skin: its texture's fine detail on its
    /// albedo's.
    Skin,
    /// r42:arcus-hair — an eye: its uv the place in its texture (`uv2`).
    Eye,
    /// r42:arcus-polish — a figure's velvet: its uv its edge's distance and its fold (from
    /// the fold's coordinates in `uv2`, `figure::cloth::fold`).
    Cloth,
}

fn grain_of(geo: &Geo, t: &Tri) -> Grain {
    match geo.mats[t.m as usize].tex {
        Tex::Hair | Tex::Scalp(_) => Grain::Hair,
        Tex::Card(k) => Grain::Card(k),
        Tex::Skin => Grain::Skin,
        Tex::Eye => Grain::Eye,
        Tex::Velvet => Grain::Cloth,
        _ => Grain::Uv,
    }
}

/// Where the sample `(sx, sy)` of `cam` meets triangle `t`: the point, the smooth normal,
/// the uv and the grain (`dP/du`). A hair card's uv is its strand's detail (and 1), its
/// grain the way its strands run (r42:arcus-hair).
fn place(cam: &Camera, t: &Tri, sx: f32, sy: f32, grain: Grain) -> (V3, V3, [f32; 2], V3) {
    let hair = grain == Grain::Hair;
    let p: [V3; 3] = std::array::from_fn(|i| world(cam, t.v[i].p));
    let d = cam.fwd + cam.right * ((sx - cam.c.x) / cam.k) - cam.up * ((sy - cam.c.y) / cam.k);
    let (e1, e2) = (p[1] - p[0], p[2] - p[0]);
    let nf = e1.cross(e2);
    let den = d.dot(nf);
    let (b1, b2) = if den.abs() > 1e-12 {
        let tt = (p[0] - cam.eye).dot(nf) / den;
        let q = cam.eye + d * tt - p[0];
        // Solve q = b1 e1 + b2 e2 in the plane.
        let (d11, d12, d22) = (e1.dot(e1), e1.dot(e2), e2.dot(e2));
        let (q1, q2) = (q.dot(e1), q.dot(e2));
        let det = d11 * d22 - d12 * d12;
        if det.abs() > 1e-18 {
            ((q1 * d22 - q2 * d12) / det, (q2 * d11 - q1 * d12) / det)
        } else {
            (1.0 / 3.0, 1.0 / 3.0)
        }
    } else {
        (1.0 / 3.0, 1.0 / 3.0)
    };
    let (b1, b2) = (b1.clamp(0.0, 1.0), b2.clamp(0.0, 1.0));
    let b0 = (1.0 - b1 - b2).max(0.0);
    let v = &t.v;
    let pw = v[0].p * b0 + v[1].p * b1 + v[2].p * b2;
    let n = (v[0].n * b0 + v[1].n * b1 + v[2].n * b2).norm();
    let mut uv = [
        v[0].uv[0] * b0 + v[1].uv[0] * b1 + v[2].uv[0] * b2,
        v[0].uv[1] * b0 + v[1].uv[1] * b1 + v[2].uv[1] * b2,
    ];
    // r42:arcus-upper — a realistic figure's skin: its own texture's fine detail (pores,
    // the brows' hairs, the lips' lines) on the vertex detail's luminance.
    if grain == Grain::Eye {
        uv = [
            v[0].uv2[0] * b0 + v[1].uv2[0] * b1 + v[2].uv2[0] * b2,
            v[0].uv2[1] * b0 + v[1].uv2[1] * b1 + v[2].uv2[1] * b2,
        ];
    } else if v[0].uv2[0] >= 0.0 && grain == Grain::Skin {
        let u2 = v[0].uv2[0] * b0 + v[1].uv2[0] * b1 + v[2].uv2[0] * b2;
        let v2 = v[0].uv2[1] * b0 + v[1].uv2[1] * b1 + v[2].uv2[1] * b2;
        uv[0] *= crate::scene::figure::cards::skin_detail(u2, v2);
    } else if grain == Grain::Cloth {
        let code = v[0].uv2[0] * b0 + v[1].uv2[0] * b1 + v[2].uv2[0] * b2;
        let at = v[0].uv2[1] * b0 + v[1].uv2[1] * b1 + v[2].uv2[1] * b2;
        uv[1] = crate::scene::figure::cloth::fold(code, at);
    }
    // dP/du from the triangle's uv.
    let (du1, dv1) = (v[1].uv[0] - v[0].uv[0], v[1].uv[1] - v[0].uv[1]);
    let (du2, dv2) = (v[2].uv[0] - v[0].uv[0], v[2].uv[1] - v[0].uv[1]);
    let det = du1 * dv2 - du2 * dv1;
    let (a1, a2) = (v[1].p - v[0].p, v[2].p - v[0].p);
    let tg = if det.abs() > 1e-12 && !hair {
        (a1 * dv2 - a2 * dv1) * (1.0 / det)
    } else {
        // r42:arcus-upper — `v` the same at every corner, or hair (its `v` the hairline's
        // blend, not a way): the way `u` grows in the triangle's plane (an edge, or `dP/du`
        // along a sliver at the hairline, stood in for it: a faceted sheen).
        let nn = a1.cross(a2);
        let g = (nn.cross(a1) * du2 + a2.cross(nn) * du1) * (1.0 / nn.dot(nn).max(1e-12));
        if g.len() > 1e-9 {
            g
        } else {
            a1
        }
    };
    // r42:arcus-hair — a hair card: its strand's detail at the level a sample reaches, and
    // the way its strands run (dP/du·du + dP/dv·dv along the texture's way).
    if let Grain::Card(k) = grain {
        if let Some(c) = crate::scene::figure::cards::card(k).filter(|c| c.hair) {
            // (A card triangle whose texture coordinates are degenerate still takes its
            // strand's detail — given back as its raw coordinate, it read as pale patches
            // over his crown — at the first level, its grain its edge's.)
            let ok = det.abs() > 1e-12;
            let dpdv = if ok {
                (a2 * du1 - a1 * du2) * (1.0 / det)
            } else {
                V3::default()
            };
            let texel = tg.cross(dpdv).len() / (c.w * c.h) as f32;
            let z = (world(cam, pw) - cam.eye).dot(cam.fwd).max(NEAR);
            let foot = z / cam.k;
            let lod = if texel > 1e-14 {
                0.5 * (foot * foot / texel).max(1.0).log2()
            } else {
                0.0
            };
            let (detail, way) = c.strand(uv, lod);
            let detail =
                1.0 + (detail - 1.0) * crate::scene::figure::cards::hair_look(k / 3).detail;
            let along = match way {
                Some(w) if ok => tg * w[0] + dpdv * w[1],
                _ => tg,
            };
            let along = if along.len() > 1e-9 { along } else { a1 };
            return (pw, n, [detail, 1.0], along.norm());
        }
    }
    (pw, n, uv, tg.norm())
}

/// How far the smooth normal of `t` turns over one sample at `(sx, sy)` (the larger of the
/// turn one sample right and one sample down, as a change of the unit normal): a thin
/// tube's whole half-round lies in two or three samples, and a mirror shaded at single
/// normals there beads and twists like a rope (round 40, cp3) — the shading spreads its
/// reflection over this much instead. The triangle's plane is extended past its edges.
fn normal_turn(cam: &Camera, t: &Tri, sx: f32, sy: f32) -> V3 {
    let p: [V3; 3] = std::array::from_fn(|i| world(cam, t.v[i].p));
    let (e1, e2) = (p[1] - p[0], p[2] - p[0]);
    let nf = e1.cross(e2);
    let (d11, d12, d22) = (e1.dot(e1), e1.dot(e2), e2.dot(e2));
    let det = d11 * d22 - d12 * d12;
    let n_at = |sx: f32, sy: f32| -> Option<V3> {
        let d = cam.fwd + cam.right * ((sx - cam.c.x) / cam.k) - cam.up * ((sy - cam.c.y) / cam.k);
        let den = d.dot(nf);
        if den.abs() <= 1e-12 || det.abs() <= 1e-18 {
            return None;
        }
        let tt = (p[0] - cam.eye).dot(nf) / den;
        let q = cam.eye + d * tt - p[0];
        let (q1, q2) = (q.dot(e1), q.dot(e2));
        let (b1, b2) = ((q1 * d22 - q2 * d12) / det, (q2 * d11 - q1 * d12) / det);
        let v = &t.v;
        let n = v[0].n * (1.0 - b1 - b2) + v[1].n * b1 + v[2].n * b2;
        (n.len() > 1e-6).then(|| n.norm())
    };
    let (Some(n0), Some(nx), Some(ny)) = (n_at(sx, sy), n_at(sx + 1.0, sy), n_at(sx, sy + 1.0))
    else {
        return V3::default();
    };
    let (dx, dy) = (nx - n0, ny - n0);
    if dx.dot(dx) >= dy.dot(dy) {
        dx
    } else {
        dy
    }
}

/// A half-resolution cell of traced reflection: the material it was traced on (none:
/// `u16::MAX`), the normal and depth there, the reflection.
#[derive(Copy, Clone)]
struct RCell {
    m: u16,
    n: V3,
    z: f32,
    c: Lin,
}

impl RCell {
    const NONE: RCell = RCell {
        m: u16::MAX,
        n: V3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        z: 0.0,
        c: [0.0; 3],
    };
}

/// Sixteen points in the unit disc, spread evenly (a Poisson disc).
const DISC: [(f32, f32); 16] = [
    (-0.942, -0.399),
    (0.946, -0.769),
    (-0.094, -0.929),
    (0.345, 0.294),
    (-0.916, 0.458),
    (-0.815, -0.879),
    (-0.383, 0.277),
    (0.975, 0.756),
    (0.443, -0.975),
    (0.537, -0.474),
    (-0.265, -0.419),
    (0.792, 0.191),
    (-0.242, 0.997),
    (-0.814, 0.914),
    (0.200, 0.787),
    (0.144, -0.141),
];

/// The key light's shadow: depth seen from the softbox's middle.
pub struct Shadow {
    cam: Camera,
    n: usize,
    depth: Vec<f32>,
    /// The softbox's half side (m): the penumbra grows with it.
    size: f32,
}

impl Shadow {
    fn of(geo: &Geo, rig: &Rig, n: usize, crew: &Crew) -> Shadow {
        let k = &rig.key;
        let half = k.cone.0.clamp(-0.99, 0.999).acos() * 1.12;
        let kk = n as f32 * 0.5 / half.tan().max(0.05);
        let cam = Camera::look(
            k.pos,
            k.focus,
            kk,
            Pos2::new(n as f32 * 0.5, n as f32 * 0.5),
        );
        let (depth, _) = gbuf(geo, &cam, n, n, crew, false, &[]);
        Shadow {
            cam,
            n,
            depth,
            size: k.size,
        }
    }

    fn at(&self, x: f32, y: f32) -> f32 {
        let (x, y) = (x as isize, y as isize);
        if x < 0 || y < 0 || x >= self.n as isize || y >= self.n as isize {
            return f32::MAX;
        }
        self.depth[y as usize * self.n + x as usize]
    }

    /// Whether the key reaches `p` (normal `n`), one look-up: 0 or 1.
    pub fn hard(&self, p: V3, n: V3) -> f32 {
        let q = p + n * 0.008;
        let z = (q - self.cam.eye).dot(self.cam.fwd);
        if z < 0.05 {
            return 1.0;
        }
        let s = self.cam.project(q);
        let bias = 0.012 + 0.004 * z;
        if self.at(s.x, s.y) >= z - bias {
            1.0
        } else {
            0.0
        }
    }

    /// How much of the key reaches `p` (normal `n`): 0 … 1, its penumbra from where the
    /// occluders stand (a blocker search, then a filter as wide as the penumbra).
    pub fn light(&self, p: V3, n: V3) -> f32 {
        let q = p + n * 0.008;
        let z = (q - self.cam.eye).dot(self.cam.fwd);
        if z < 0.05 {
            return 1.0;
        }
        let s = self.cam.project(q);
        let bias = 0.012 + 0.004 * z;
        let rot =
            noise(p.x * 913.0 + p.y * 571.0, p.z * 911.0 - p.y * 389.0, 5) * std::f32::consts::TAU;
        let (cr, sr) = (rot.cos(), rot.sin());
        let px_m = self.cam.k / z;
        let search = (self.size * px_m * 0.6).clamp(1.5, 28.0);
        let (mut sum, mut cnt) = (0.0, 0.0);
        for (dx, dy) in DISC {
            let (rx, ry) = (dx * cr - dy * sr, dx * sr + dy * cr);
            let d = self.at(s.x + rx * search, s.y + ry * search);
            if d < z - bias {
                sum += d;
                cnt += 1.0;
            }
        }
        if cnt == 0.0 {
            return 1.0;
        }
        let zb = sum / cnt;
        let pen = (self.size * (z - zb) / zb.max(0.05) * px_m).clamp(1.0, 36.0);
        let mut lit = 0.0;
        for (dx, dy) in DISC {
            let (rx, ry) = (dx * cr - dy * sr, dx * sr + dy * cr);
            if self.at(s.x + rx * pen, s.y + ry * pen) >= z - bias {
                lit += 1.0;
            }
        }
        lit / DISC.len() as f32
    }

    /// r42:arcus-upper — [`Shadow::light`] without its grain, for skin and hair (the owner,
    /// 2026-11-24: a speckle on the face and the chest): the disc turned the same way
    /// everywhere and twice (32 points), each look-up compared bilinearly, so a penumbra
    /// over a cheek or a neck is a smooth ramp.
    pub fn soft(&self, p: V3, n: V3) -> f32 {
        let q = p + n * 0.008;
        let z = (q - self.cam.eye).dot(self.cam.fwd);
        if z < 0.05 {
            return 1.0;
        }
        let s = self.cam.project(q);
        // (r42:arcus-upper: a little more bias than 1.2 cm — a face turned from the key
        // showed the map's texels as fine diagonal stripes at 200 %.)
        let bias = 0.018 + 0.006 * z;
        let px_m = self.cam.k / z;
        let search = (self.size * px_m * 0.6).clamp(1.5, 28.0);
        let (mut sum, mut cnt) = (0.0, 0.0);
        for (dx, dy) in DISC {
            let d = self.at(s.x + dx * search, s.y + dy * search);
            if d < z - bias {
                sum += d;
                cnt += 1.0;
            }
        }
        if cnt == 0.0 {
            return 1.0;
        }
        let zb = sum / cnt;
        let pen = (self.size * (z - zb) / zb.max(0.05) * px_m).clamp(1.5, 36.0);
        // The second turn: the first disc rotated by half its mean angle between points.
        let (cr, sr) = (0.98079f32, 0.19509f32);
        let mut lit = 0.0;
        for (dx, dy) in DISC {
            for (rx, ry) in [(dx, dy), (dx * cr - dy * sr, dx * sr + dy * cr)] {
                lit += self.lit_bilinear(s.x + rx * pen, s.y + ry * pen, z - bias);
            }
        }
        lit / (2 * DISC.len()) as f32
    }

    /// Whether depth `z` is lit at `(x, y)`, its four texels' answers blended bilinearly.
    fn lit_bilinear(&self, x: f32, y: f32, z: f32) -> f32 {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let l = |xx: f32, yy: f32| {
            if self.at(xx + 0.5, yy + 0.5) >= z {
                1.0
            } else {
                0.0
            }
        };
        let a = l(x0, y0) * (1.0 - tx) + l(x0 + 1.0, y0) * tx;
        let b = l(x0, y0 + 1.0) * (1.0 - tx) + l(x0 + 1.0, y0 + 1.0) * tx;
        a * (1.0 - ty) + b * ty
    }

    /// r42:arcus-upper — the key's reach for a surface of `m`: smooth on skin and hair.
    pub fn light_for(&self, m: &PMat, p: V3, n: V3) -> f32 {
        if matches!(m.tex, Tex::Skin | Tex::Hair | Tex::Scalp(_) | Tex::Card(_)) {
            self.soft(p, n)
        } else {
            self.light(p, n)
        }
    }
}

/// The occlusion at sample `(x, y)` from the depths round it.
fn occlusion(depth: &[f32], w: usize, h: usize, x: usize, y: usize, k_px: f32) -> f32 {
    const DIRS: [(f32, f32); 8] = [
        (1.0, 0.0),
        (0.707, 0.707),
        (0.0, 1.0),
        (-0.707, 0.707),
        (-1.0, 0.0),
        (-0.707, -0.707),
        (0.0, -1.0),
        (0.707, -0.707),
    ];
    let z = depth[y * w + x];
    if z == f32::MAX {
        return 1.0;
    }
    let r = (0.14 * k_px / z).clamp(2.0, 64.0);
    let mut occ = 0.0;
    for (i, (dx, dy)) in DIRS.iter().enumerate() {
        let rr = r * (0.4 + 0.6 * ((i * 5 + x * 3 + y * 7) % 4) as f32 / 3.0);
        let sx = (x as f32 + dx * rr) as isize;
        let sy = (y as f32 + dy * rr) as isize;
        if sx < 0 || sy < 0 || sx >= w as isize || sy >= h as isize {
            continue;
        }
        let d = depth[sy as usize * w + sx as usize];
        let diff = z - d;
        if diff > 0.01 && diff < 0.5 {
            occ += 1.0 - diff / 0.5;
        }
    }
    (1.0 - occ / 8.0 * 1.15).clamp(0.2, 1.0)
}

/// A separable box blur (twice: near a Gaussian) of `img`, radius `r`.
fn blur(img: &mut [Lin], w: usize, h: usize, r: usize, crew: &Crew) {
    if r == 0 || w == 0 || h == 0 {
        return;
    }
    let mut tmp = vec![[0.0f32; 3]; w * h];
    for _ in 0..2 {
        // Along the rows.
        {
            let src = &*img;
            rows1(h, w, crew, &mut tmp, |y0, y1, out| {
                for y in y0..y1 {
                    box1(
                        &src[y * w..(y + 1) * w],
                        &mut out[(y - y0) * w..(y - y0 + 1) * w],
                        r,
                    );
                }
            });
        }
        // Down the columns: each output row sums its window of rows.
        let src = &tmp;
        rows1(h, w, crew, img, |y0, y1, out| {
            for y in y0..y1 {
                let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
                let k = 1.0 / (b - a + 1) as f32;
                let o = &mut out[(y - y0) * w..(y - y0 + 1) * w];
                o.fill([0.0; 3]);
                for yy in a..=b {
                    for (x, c) in o.iter_mut().enumerate() {
                        *c = ladd(*c, src[yy * w + x]);
                    }
                }
                for c in o.iter_mut() {
                    *c = lscale(*c, k);
                }
            }
        });
    }
}

/// A running box sum along one row.
fn box1(row: &[Lin], o: &mut [Lin], r: usize) {
    let w = row.len();
    let mut acc = [0.0f32; 3];
    for v in row.iter().take(r.min(w - 1) + 1) {
        acc = ladd(acc, *v);
    }
    for x in 0..w {
        let n = (x + r).min(w - 1) - x.saturating_sub(r) + 1;
        o[x] = lscale(acc, 1.0 / n as f32);
        if x + r + 1 < w {
            acc = ladd(acc, row[x + r + 1]);
        }
        if x >= r {
            let v = row[x - r];
            acc = [acc[0] - v[0], acc[1] - v[1], acc[2] - v[2]];
        }
    }
}

/// The filmic curve (Narkowicz's fit of ACES).
fn aces(x: f32) -> f32 {
    let x = x.max(0.0);
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// The filmic curve on the luminance (the hue kept, the saturation trimmed by `sat`), the
/// brightest highlights rolling to white as film's do (r42:arcus-upper: public, the live
/// arms are toned as the bake is).
pub fn tone(c: Lin, sat: f32) -> Lin {
    let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    if l <= 1e-6 {
        return [0.0; 3];
    }
    let lm = aces(l);
    let k = lm / l;
    let kept: Lin = std::array::from_fn(|i| (lm + (c[i] * k - lm) * sat).clamp(0.0, 1.0));
    let per: Lin = c.map(aces);
    let w = ((lm - 0.55) / 0.45).clamp(0.0, 1.0);
    lmix(kept, per, w * w)
}

/// Draw the wires over the pixels, anti-aliased, where nothing stands in front.
#[allow(clippy::too_many_arguments)]
fn lines(
    geo: &Geo,
    rig: &Rig,
    sh: &Shadow,
    cam: &Camera,
    depth: &[f32],
    col: &mut [Lin],
    w: usize,
    h: usize,
    crew: &Crew,
) {
    struct Stroke {
        pa: Pos2,
        d: egui::Vec2,
        len: f32,
        dir: egui::Vec2,
        nrm: egui::Vec2,
        wpx: f32,
        cover: f32,
        za: f32,
        zb: f32,
        c: Lin,
        bx: [usize; 4],
    }
    let eye = cam.eye;
    let mut strokes = Vec::with_capacity(geo.lines.len());
    for l in &geo.lines {
        let (za, zb) = ((l.a - eye).dot(cam.fwd), (l.b - eye).dot(cam.fwd));
        if za < 0.05 || zb < 0.05 {
            continue;
        }
        let (pa, pb) = (cam.project(l.a), cam.project(l.b));
        let mid = l.a.lerp(l.b, 0.5);
        let width = 2.0 * l.r * cam.k / (mid - eye).dot(cam.fwd);
        let (wpx, cover) = if width < 1.0 {
            (1.0, width.max(0.05))
        } else {
            (width, 1.0)
        };
        let v = (eye - mid).norm();
        let axis = (l.b - l.a).norm();
        let side = (v - axis * v.dot(axis)).norm();
        let (_, lk) = rig.key.at(mid);
        let n = (side + (lk - axis * lk.dot(axis)).norm() * 0.6).norm();
        let m = &geo.mats[l.m as usize];
        let c = shade(
            rig,
            m,
            eye,
            mid,
            n,
            V3::default(),
            axis,
            [mid.x, mid.z],
            0.001,
            1.0,
            &|p, n| sh.light(p, n),
            None,
            None,
        );
        let d = pb - pa;
        let len = d.length().max(1e-3);
        let dir = d / len;
        let hw = wpx * 0.5 + 0.75;
        let lo = pa.min(pb) - egui::vec2(hw, hw);
        let hi = pa.max(pb) + egui::vec2(hw, hw);
        strokes.push(Stroke {
            pa,
            d,
            len,
            dir,
            nrm: egui::vec2(-dir.y, dir.x),
            wpx,
            cover: cover * l.alpha,
            za,
            zb,
            c,
            bx: [
                lo.x.clamp(0.0, w as f32) as usize,
                (hi.x + 1.0).clamp(0.0, w as f32) as usize,
                lo.y.clamp(0.0, h as f32) as usize,
                (hi.y + 1.0).clamp(0.0, h as f32) as usize,
            ],
        });
    }
    rows1(h, w, crew, col, |r0, r1, out| {
        for s in &strokes {
            let reach = (s.wpx * 0.5 + 0.5) as f64;
            for y in s.bx[2].max(r0)..s.bx[3].min(r1) {
                let (mut x0, mut x1) = (s.bx[0], s.bx[1]);
                let (nx, ny) = (s.nrm.x as f64, s.nrm.y as f64);
                if nx.abs() > 1e-6 {
                    let base = (y as f64 + 0.5 - s.pa.y as f64) * ny;
                    let ea = (-reach - base) / nx + s.pa.x as f64 - 0.5;
                    let eb = (reach - base) / nx + s.pa.x as f64 - 0.5;
                    let (lo, hi) = (ea.min(eb).floor() - 2.0, ea.max(eb).ceil() + 3.0);
                    if lo.is_finite() && hi.is_finite() {
                        x0 = lo.clamp(s.bx[0] as f64, s.bx[1] as f64) as usize;
                        x1 = (hi.clamp(s.bx[0] as f64, s.bx[1] as f64) as usize).max(x0);
                    }
                }
                for x in x0..x1 {
                    let q = Pos2::new(x as f32 + 0.5, y as f32 + 0.5) - s.pa;
                    let t = (q.dot(s.dir) / s.len).clamp(0.0, 1.0);
                    let dist = (q - s.d * t).dot(s.nrm).abs();
                    let cov = (s.wpx * 0.5 + 0.5 - dist).clamp(0.0, 1.0) * s.cover;
                    if cov <= 0.0 {
                        continue;
                    }
                    let k = y * w + x;
                    let z = 1.0 / ((1.0 - t) / s.za + t / s.zb);
                    if z > depth[k] + 0.01 + 0.002 * z {
                        continue;
                    }
                    let o = &mut out[(y - r0) * w + x];
                    *o = lmix(*o, s.c, cov);
                }
            }
        }
    });
}

/// A rod narrower than this on screen (pixels) is drawn as a tube stroke: two samples a
/// pixel cannot hold a round this thin — its mirror beaded and twisted like a rope down
/// every stand, and its coverage came and went along a slanted mast (round 40 cp3; a
/// stand's mast is 3–6 pixels wide in the kit's picture).
const THIN_PX: f32 = 8.0;

/// The rods `cam` sees thinner than [`THIN_PX`] (and long beside their width).
fn thin_rods(geo: &Geo, cam: &Camera) -> Vec<Rod> {
    geo.rods
        .iter()
        .filter(|r| {
            let (za, zb) = ((r.a - cam.eye).dot(cam.fwd), (r.b - cam.eye).dot(cam.fwd));
            if za < 0.05 || zb < 0.05 {
                return false;
            }
            let wpx = 2.0 * r.r * cam.k / za.min(zb);
            let len = (cam.project(r.a) - cam.project(r.b)).length();
            wpx < THIN_PX && len > 2.5 * wpx
        })
        .copied()
        .collect()
}

/// How much of the key reaches rod `r` along its length (`rod_key_at` reads it): looked
/// up off the rod toward the key at steps of 1.5 cm and smoothed. A polished rod's mirror
/// of the key's face is dimmed by it — looked up per sample, a penumbra's own grain (its
/// filter turned per sample) beaded the highlight down every stand in a pool's edge
/// (round 40 cp3).
fn rod_key(r: &Rod, rig: &Rig, sh: &Shadow) -> Vec<f32> {
    let wl = (r.b - r.a).len();
    let nk = ((wl / 0.015) as usize).clamp(4, 240);
    let mut key: Vec<f32> = (0..nk)
        .map(|i| {
            let q = r.a + (r.b - r.a) * (i as f32 / (nk - 1) as f32);
            let (kc, l) = rig.key.at(q);
            if kc[0] + kc[1] + kc[2] <= 1e-4 {
                0.0
            } else {
                sh.light(q + l * (r.r + 0.03), l)
            }
        })
        .collect();
    for _ in 0..2 {
        let k0 = key.clone();
        for (i, kv) in key.iter_mut().enumerate() {
            let (lo, hi) = (i.saturating_sub(2), (i + 3).min(nk));
            *kv = k0[lo..hi].iter().sum::<f32>() / (hi - lo) as f32;
        }
    }
    key
}

/// [`rod_key`]'s value a fraction `t` along the rod.
fn key_at(key: &[f32], t: f32) -> f32 {
    let kf = t.clamp(0.0, 1.0) * (key.len() - 1) as f32;
    let ki = (kf as usize).min(key.len() - 2);
    key[ki] + (key[ki + 1] - key[ki]) * (kf - ki as f32)
}

/// Draw the thin rods as tubes over the pixels, anti-aliased against the depth: across
/// each, the normal turns round its half-round on screen (so the room's mirror and the
/// key's highlight lie along it as one long line, the edges dark), every pixel shaded at
/// its place across with its footprint's spread. Where a tube covers most of a pixel the
/// pixel takes its depth (depth of field and bloom see it where it stands).
#[allow(clippy::too_many_arguments)]
fn tubes(
    rods: &[Rod],
    geo: &Geo,
    rig: &Rig,
    sh: &Shadow,
    cam: &Camera,
    pdepth: &mut [f32],
    col: &mut [Lin],
    w: usize,
    h: usize,
    crew: &Crew,
) {
    if rods.is_empty() {
        return;
    }
    struct Stroke {
        rod: Rod,
        pa: Pos2,
        d: egui::Vec2,
        len: f32,
        nrm: egui::Vec2,
        wa: f32,
        wb: f32,
        za: f32,
        zb: f32,
        axis: V3,
        /// How much of the key reaches the rod along its length (smoothed): what its
        /// mirror of the key's face is dimmed by. Looked up per pixel, a penumbra's own
        /// grain beaded the highlight down a mast.
        key: Vec<f32>,
        bx: [usize; 4],
    }
    let eye = cam.eye;
    let strokes: Vec<Stroke> = rods
        .iter()
        .map(|r| {
            let (za, zb) = ((r.a - eye).dot(cam.fwd), (r.b - eye).dot(cam.fwd));
            let (pa, pb) = (cam.project(r.a), cam.project(r.b));
            let (wa, wb) = (2.0 * r.r * cam.k / za, 2.0 * r.r * cam.k / zb);
            let d = pb - pa;
            let len = d.length().max(1e-3);
            let dir = d / len;
            let hw = wa.max(wb) * 0.5 + 1.0;
            let lo = pa.min(pb) - egui::vec2(hw, hw);
            let hi = pa.max(pb) + egui::vec2(hw, hw);
            Stroke {
                rod: *r,
                key: rod_key(r, rig, sh),
                pa,
                d,
                len,
                nrm: egui::vec2(-dir.y, dir.x),
                wa,
                wb,
                za,
                zb,
                axis: (r.b - r.a).norm(),
                bx: [
                    lo.x.clamp(0.0, w as f32) as usize,
                    (hi.x + 1.0).clamp(0.0, w as f32) as usize,
                    lo.y.clamp(0.0, h as f32) as usize,
                    (hi.y + 1.0).clamp(0.0, h as f32) as usize,
                ],
            }
        })
        .collect();
    rows2(h, w, crew, col, pdepth, |r0, r1, out, pd| {
        for s in &strokes {
            let m = &geo.mats[s.rod.m as usize];
            for y in s.bx[2].max(r0)..s.bx[3].min(r1) {
                for x in s.bx[0]..s.bx[1] {
                    let q = Pos2::new(x as f32 + 0.5, y as f32 + 0.5) - s.pa;
                    let t = (q.dot(s.d) / (s.len * s.len)).clamp(0.0, 1.0);
                    let sd = (q - s.d * t).dot(s.nrm);
                    let wpx = s.wa + (s.wb - s.wa) * t;
                    let hw = (wpx * 0.5).max(0.5);
                    let cov = (hw + 0.5 - sd.abs()).clamp(0.0, 1.0) * wpx.min(1.0);
                    if cov <= 0.0 {
                        continue;
                    }
                    let k = (y - r0) * w + x;
                    let z = 1.0 / ((1.0 - t) / s.za + t / s.zb);
                    // The tube's face where this pixel sees it (its axis less the round),
                    // tested tightly: a collar round the mast stays in front of it.
                    let us = (sd / (wpx * 0.5).max(0.5)).clamp(-1.0, 1.0);
                    let zf = z - s.rod.r * (1.0 - us * us).sqrt();
                    if zf > pd[k] + 0.002 + 0.0003 * zf {
                        continue;
                    }
                    // The place on the axis (perspective-correct), the view across it.
                    let tw = (t / s.zb) * z;
                    let at = s.rod.a + (s.rod.b - s.rod.a) * tw;
                    let v = (eye - at).norm();
                    let side = (v - s.axis * v.dot(s.axis)).norm();
                    let mut across = s.axis.cross(side).norm();
                    let probe = cam.project(at + across * 0.01) - cam.project(at);
                    if probe.dot(s.nrm) < 0.0 {
                        across = -across;
                    }
                    let n_at = |u: f32| {
                        let u = u.clamp(-0.995, 0.995);
                        (across * u + side * (1.0 - u * u).sqrt()).norm()
                    };
                    let u = (sd / hw).clamp(-1.0, 1.0);
                    let du = 1.0 / hw;
                    let n = n_at(u);
                    let n2 = if u + du <= 1.0 {
                        n_at(u + du)
                    } else {
                        n_at(u - du)
                    };
                    let kv = key_at(&s.key, tw);
                    let lit = |n: V3, dn: V3| {
                        shade(
                            rig,
                            m,
                            eye,
                            at + n * s.rod.r,
                            n,
                            dn,
                            s.axis,
                            [tw * s.len, 0.0],
                            z / cam.k,
                            1.0,
                            &|_, _| kv,
                            None,
                            None,
                        )
                    };
                    // r42:arcus-hands — a tube under a pixel and a half wide: its round
                    // averaged (five ways across it), as the pixel sees all of it.
                    let c = if geo.spread_thin && wpx < 1.5 {
                        let mut acc = [0.0f32; 3];
                        for i in 0..5 {
                            let ui = -0.8 + 0.4 * i as f32;
                            let ni = n_at(ui);
                            acc = ladd(acc, lscale(lit(ni, n_at(ui + 0.4) - ni), 0.2));
                        }
                        acc
                    } else {
                        lit(n, n2 - n)
                    };
                    // A chrome tube's edges dark (what a photograph's polished stand shows
                    // where it turns away), its mirror bright down the middle.
                    let c = if m.kind == Kind::Metal {
                        let e = 1.0 - 0.6 * super::mat::smooth(0.55, 1.0, u.abs());
                        lscale(c, e)
                    } else {
                        c
                    };
                    out[k] = lmix(out[k], c, cov);
                    if cov > 0.5 {
                        pd[k] = pd[k].min(zf);
                    }
                }
            }
        }
    });
}

/// The floor's mirror image: the instrument (triangles from `split`) seen from under the
/// floor, a pixel a sample, blurred as the floor's gloss blurs it.
#[allow(clippy::too_many_arguments)]
fn mirror(
    geo: &Geo,
    split: usize,
    rig: &Rig,
    cam: &Camera,
    w: usize,
    h: usize,
    shadow: &Shadow,
    crew: &Crew,
) -> (Vec<Lin>, usize) {
    let mcam = Camera { flip: true, ..*cam };
    let pt = project(&mcam, &geo.tris[split..], &[]);
    let mut md = vec![f32::MAX; w * h];
    let mut mt = vec![u32::MAX; w * h];
    rows2(h, w, crew, &mut md, &mut mt, |y0, y1, d, t| {
        // (No cards in the floor's soft mirror: a figure's hair there is a blur.)
        raster_rows(&pt, &[], w, y0, y1, d, Some(t))
    });
    let bytes = pt.len() * std::mem::size_of::<(u32, PTri)>() + w * h * 20;
    drop(pt);
    let meye = V3 {
        y: -cam.eye.y,
        ..cam.eye
    };
    let cap = 0.5;
    let mut mc = vec![[0.0f32; 3]; w * h];
    rows1(h, w, crew, &mut mc, |y0, y1, out| {
        for y in y0..y1 {
            for x in 0..w {
                let t = mt[y * w + x];
                if t == u32::MAX {
                    continue;
                }
                let tr = &geo.tris[split + t as usize];
                let (p, n, uv, tg) =
                    place(&mcam, tr, x as f32 + 0.5, y as f32 + 0.5, grain_of(geo, tr));
                let m = &geo.mats[tr.m as usize];
                let fp = md[y * w + x] / mcam.k;
                let c = shade(
                    rig,
                    m,
                    meye,
                    p,
                    n,
                    V3::default(),
                    tg,
                    uv,
                    fp,
                    1.0,
                    &|p, n| shadow.light(p, n),
                    None,
                    None,
                );
                // No firefly: a chrome rod's glint, one sample wide, would spread a speck.
                out[(y - y0) * w + x] = c.map(|v| v.min(cap));
            }
        }
    });
    let rough = 0.06 + 0.25 * (1.0 - rig.room.floor.coat);
    blur(
        &mut mc,
        w,
        h,
        ((rough * 10.0 * cam.k / 900.0) as usize).clamp(2, 6),
        crew,
    );
    (mc, bytes)
}

/// The picture: `geo` (its room first, the instrument from triangle `split`, which alone
/// the floor mirrors) lit by `rig`, seen by `cam` (in pixels), `w` × `h` pixels.
#[allow(clippy::too_many_arguments)]
pub fn bake(
    geo: &Geo,
    split: usize,
    rig: &Rig,
    cam: &Camera,
    w: usize,
    h: usize,
    q: Quality,
    threads: usize,
) -> (egui::ColorImage, Stats) {
    let t0 = std::time::Instant::now();
    let ss = q.ss.clamp(1, 3);
    let (sw, sh_) = (w * ss, h * ss);
    let scam = Camera {
        k: cam.k * ss as f32,
        c: Pos2::new(cam.c.x * ss as f32, cam.c.y * ss as f32),
        ..*cam
    };
    let crew = &Crew::new(threads);
    let mut peak = 0usize;
    let mut lap = std::time::Instant::now();
    let mut passes = [0.0f64; 7];
    let mut mark = |i: usize| {
        passes[i] = lap.elapsed().as_secs_f64() * 1e3;
        lap = std::time::Instant::now();
    };
    let shadow = Shadow::of(geo, rig, q.shadow, crew);
    mark(0);
    let base = q.shadow * q.shadow * 4 + geo.tris.len() * std::mem::size_of::<Tri>();
    let mirror = if q.full && rig.room.gloss > 0.04 && split < geo.tris.len() {
        let (m, bytes) = mirror(geo, split, rig, cam, w, h, &shadow, crew);
        peak = peak.max(base + bytes);
        Some(m)
    } else {
        None
    };
    mark(1);
    let bvh =
        (q.full && q.trace && split < geo.tris.len()).then(|| super::trace::Bvh::build(geo, split));
    let tracer = bvh.as_ref().map(|bvh| super::shade::Trace {
        bvh,
        mats: &geo.mats,
        shadow: &shadow,
    });
    let trace = tracer.as_ref();
    let bvh_bytes = bvh.as_ref().map_or(0, |b| b.bytes());
    let base = base + bvh_bytes;
    mark(2);
    // Rods too thin on screen for the samples to hold their round: out of the samples,
    // drawn as shaded tube strokes after the shading (their triangles still cast shadows
    // and show in reflections).
    let thin = thin_rods(geo, cam);
    let mut skip = Vec::new();
    if !thin.is_empty() {
        skip = vec![false; geo.tris.len()];
        for r in &thin {
            for t in r.tris.0..r.tris.1 {
                skip[t as usize] = true;
            }
        }
    }
    let (depth, tri) = gbuf(geo, &scam, sw, sh_, crew, true, &skip);
    drop(skip);
    // Every polished rod's key along its length (see `rod_key`), and which rod each
    // triangle belongs to.
    let mut rod_of: Vec<u32> = Vec::new();
    let mut keys: Vec<Vec<f32>> = Vec::new();
    for (i, r) in geo.rods.iter().enumerate() {
        if geo.mats[r.m as usize].kind != Kind::Metal {
            continue;
        }
        if rod_of.is_empty() {
            rod_of = vec![u32::MAX; geo.tris.len()];
            keys = vec![Vec::new(); geo.rods.len()];
        }
        keys[i] = rod_key(r, rig, &shadow);
        for t in r.tris.0..r.tris.1 {
            rod_of[t as usize] = i as u32;
        }
    }
    let (rod_of, keys) = (&rod_of, &keys);
    mark(3);
    let mbytes = mirror.as_ref().map_or(0, |m| m.len() * 12);
    peak = peak
        .max(base + sw * sh_ * 8 + geo.tris.len() * std::mem::size_of::<(u32, PTri)>() + mbytes);
    // Traced reflections at half resolution (one every other pixel each way, at its middle
    // sample), shared out to the samples round them that see the same material facing the
    // same way at the same depth; a sample with no such neighbour traces its own.
    let (rw, rh) = (w.div_ceil(2), h.div_ceil(2));
    let mut rbuf: Vec<RCell> = if trace.is_some() {
        vec![RCell::NONE; rw * rh]
    } else {
        Vec::new()
    };
    if let Some(tr) = trace {
        let (depth, tri, shadow) = (&depth, &tri, &shadow);
        rows1(rh, rw, crew, &mut rbuf, |j0, j1, out| {
            for j in j0..j1 {
                for i in 0..rw {
                    let (x, y) = ((2 * i).min(w - 1), (2 * j).min(h - 1));
                    let (sx, sy) = (x * ss + ss / 2, y * ss + ss / 2);
                    let k = sy * sw + sx;
                    let t = tri[k];
                    if t == u32::MAX {
                        continue;
                    }
                    let trg = &geo.tris[t as usize];
                    let m = &geo.mats[trg.m as usize];
                    let (p, n, uv, _) = place(
                        &scam,
                        trg,
                        sx as f32 + 0.5,
                        sy as f32 + 0.5,
                        grain_of(geo, trg),
                    );
                    let fp = depth[k] / scam.k * ss as f32;
                    if let Some(c) = reflection(
                        rig,
                        m,
                        cam.eye,
                        p,
                        n,
                        uv,
                        fp,
                        &|p, n| shadow.light(p, n),
                        tr,
                    ) {
                        out[(j - j0) * rw + i] = RCell {
                            m: trg.m,
                            n,
                            z: depth[k],
                            c,
                        };
                    }
                }
            }
        });
    }
    peak = peak.max(base + sw * sh_ * 8 + mbytes + rbuf.len() * std::mem::size_of::<RCell>());
    let rbuf = &rbuf;
    // The reflection the half-resolution cells give a sample at pixel coordinates (`px`,
    // `py`) seeing material `mid` facing `n` at depth `z`.
    let refl_at = |px: f32, py: f32, mid: u16, n: V3, z: f32| -> Option<Lin> {
        if rbuf.is_empty() {
            return None;
        }
        let (fx, fy) = ((px / 2.0).max(0.0), (py / 2.0).max(0.0));
        let (i0, j0) = ((fx as usize).min(rw - 1), (fy as usize).min(rh - 1));
        let (tx, ty) = (fx - i0 as f32, fy - j0 as f32);
        // Only where all four cells see this surface and agree (a smooth stretch of the
        // reflection): a fine reflection (strings, an edge's line) is traced at full
        // resolution.
        let mut acc = [0.0f32; 3];
        let (mut lo, mut hi) = (f32::MAX, 0.0f32);
        for (di, dj, wt) in [
            (0, 0, (1.0 - tx) * (1.0 - ty)),
            (1, 0, tx * (1.0 - ty)),
            (0, 1, (1.0 - tx) * ty),
            (1, 1, tx * ty),
        ] {
            let (i, j) = ((i0 + di).min(rw - 1), (j0 + dj).min(rh - 1));
            let c = &rbuf[j * rw + i];
            if c.m != mid || c.n.dot(n) <= 0.97 || (c.z - z).abs() >= 0.02 * z + 0.005 {
                return None;
            }
            let l = 0.2126 * c.c[0] + 0.7152 * c.c[1] + 0.0722 * c.c[2];
            lo = lo.min(l);
            hi = hi.max(l);
            acc = ladd(acc, lscale(c.c, wt));
        }
        (hi - lo <= 0.12 * hi + 0.002).then_some(acc)
    };
    // Shade every sample into its pixel.
    let mut col = vec![[0.0f32; 3]; w * h];
    let mut pdepth = vec![f32::MAX; w * h];
    peak = peak.max(base + sw * sh_ * 8 + mbytes + w * h * 16);
    let eye = cam.eye;
    let bg = rig.void;
    let gloss = rig.room.gloss;
    let ex = rig.exposure;
    {
        let (depth, tri, mirror, shadow) = (&depth, &tri, &mirror, &shadow);
        rows2(h, w, crew, &mut col, &mut pdepth, |y0, y1, out, pd| {
            for y in y0..y1 {
                for x in 0..w {
                    // Samples summed weighted by how bright they are on screen (Karis): a
                    // glint one sample wide resolves to a fine line, not a bead.
                    let mut acc = [0.0f32; 3];
                    let mut wsum = 0.0f32;
                    let mut zmin = f32::MAX;
                    // r42:arcus-hair — ...but a pixel a figure's silhouette crosses sums its
                    // samples evenly: her light hair or a lit jaw weighted down against the
                    // room behind lost its share of the edge, and the silhouette stepped.
                    let even = ss > 1
                        && (0..ss * ss).any(|i| {
                            let t = tri[(y * ss + i / ss) * sw + x * ss + i % ss];
                            t != u32::MAX && figure_mat(&geo.mats[geo.tris[t as usize].m as usize])
                        });
                    let mut add = |acc: &mut Lin, c: Lin| {
                        let wt = if even {
                            1.0
                        } else {
                            1.0 / (1.0 + ex * (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]))
                        };
                        *acc = ladd(*acc, lscale(c, wt));
                        wsum += wt;
                    };
                    // A pixel whose samples all see one triangle of a plain lacquer or a
                    // matte is shaded once at its middle (its textures are filtered to
                    // the pixel); an edge or a metal's glint takes every
                    // sample.
                    let k0 = (y * ss) * sw + x * ss;
                    let t0 = tri[k0];
                    let uniform = ss > 1
                        && t0 != u32::MAX
                        && (0..ss).all(|dy| {
                            (0..ss).all(|dx| tri[(y * ss + dy) * sw + x * ss + dx] == t0)
                        })
                        && {
                            let m = &geo.mats[geo.tris[t0 as usize].m as usize];
                            matches!(m.kind, Kind::Lacquer | Kind::Matte)
                        };
                    // ...unless the key's shadow edge crosses it: a penumbra is a filtered
                    // estimate, and its own grain wants every sample to average out.
                    let once = uniform.then(|| {
                        let tr = &geo.tris[t0 as usize];
                        let c = (ss as f32) * 0.5;
                        let (sxf, syf) = (x as f32 * ss as f32 + c, y as f32 * ss as f32 + c);
                        let (p, n, uv, tg) = place(&scam, tr, sxf, syf, grain_of(geo, tr));
                        let sv = shadow.light_for(&geo.mats[tr.m as usize], p, n);
                        (tr, p, n, uv, tg, sv)
                    });
                    if let Some((tr, p, n, uv, tg, sv)) =
                        once.filter(|o| o.5 <= 0.01 || o.5 >= 0.99)
                    {
                        let vis = |q: V3, nq: V3| {
                            if q == p && nq == n {
                                sv
                            } else {
                                shadow.light_for(&geo.mats[tr.m as usize], q, nq)
                            }
                        };
                        let m = &geo.mats[tr.m as usize];
                        let z = (0..ss)
                            .flat_map(|dy| (0..ss).map(move |dx| (dy, dx)))
                            .map(|(dy, dx)| depth[(y * ss + dy) * sw + x * ss + dx])
                            .fold(f32::MAX, f32::min);
                        let fp = z / scam.k * ss as f32;
                        let ao = if q.full {
                            occlusion(depth, sw, sh_, x * ss + ss / 2, y * ss + ss / 2, scam.k)
                        } else {
                            1.0
                        };
                        let given = refl_at(x as f32, y as f32, tr.m, n, z);
                        let mut c = shade(
                            rig,
                            m,
                            eye,
                            p,
                            n,
                            V3::default(),
                            tg,
                            uv,
                            fp,
                            ao,
                            &vis,
                            trace,
                            given,
                        );
                        if let (Some(mc), Tex::Floor) = (mirror, m.tex) {
                            let v = (eye - p).norm();
                            let f = 0.04 + 0.96 * (1.0 - v.y.abs()).powi(5);
                            let kk = gloss * (0.25 + 0.75 * f).min(1.0) * ao;
                            c = ladd(c, lscale(mc[y * w + x], kk));
                        }
                        out[(y - y0) * w + x] = c;
                        pd[(y - y0) * w + x] = z;
                        continue;
                    }
                    for dy in 0..ss {
                        for dx in 0..ss {
                            let (sx, sy) = (x * ss + dx, y * ss + dy);
                            let k = sy * sw + sx;
                            let t = tri[k];
                            if t == u32::MAX {
                                add(&mut acc, bg);
                                continue;
                            }
                            zmin = zmin.min(depth[k]);
                            let tr = &geo.tris[t as usize];
                            let (p, n, uv, tg) = place(
                                &scam,
                                tr,
                                sx as f32 + 0.5,
                                sy as f32 + 0.5,
                                grain_of(geo, tr),
                            );
                            let m = &geo.mats[tr.m as usize];
                            let fp = depth[k] / scam.k;
                            let ao = if q.full {
                                occlusion(depth, sw, sh_, sx, sy, scam.k)
                            } else {
                                1.0
                            };
                            let given = refl_at(
                                (sx as f32 + 0.5) / ss as f32 - 0.5,
                                (sy as f32 + 0.5) / ss as f32 - 0.5,
                                tr.m,
                                n,
                                depth[k],
                            );
                            let turn = if m.kind == Kind::Metal {
                                normal_turn(&scam, tr, sx as f32 + 0.5, sy as f32 + 0.5)
                            } else {
                                V3::default()
                            };
                            let rod = rod_of.get(t as usize).copied().unwrap_or(u32::MAX);
                            let mut c = if rod != u32::MAX {
                                let r = &geo.rods[rod as usize];
                                let ax = r.b - r.a;
                                let along = (p - r.a).dot(ax) / ax.dot(ax).max(1e-9);
                                let kv = key_at(&keys[rod as usize], along);
                                shade(
                                    rig,
                                    m,
                                    eye,
                                    p,
                                    n,
                                    turn,
                                    tg,
                                    uv,
                                    fp,
                                    ao,
                                    &|_, _| kv,
                                    trace,
                                    given,
                                )
                            } else {
                                shade(
                                    rig,
                                    m,
                                    eye,
                                    p,
                                    n,
                                    turn,
                                    tg,
                                    uv,
                                    fp,
                                    ao,
                                    &|p, n| shadow.light_for(m, p, n),
                                    trace,
                                    given,
                                )
                            };
                            if let (Some(mc), Tex::Floor) = (mirror, m.tex) {
                                let v = (eye - p).norm();
                                let f = 0.04 + 0.96 * (1.0 - v.y.abs()).powi(5);
                                let kk = gloss * (0.25 + 0.75 * f).min(1.0) * ao;
                                c = ladd(c, lscale(mc[y * w + x], kk));
                            }
                            add(&mut acc, c);
                        }
                    }
                    out[(y - y0) * w + x] = lscale(acc, 1.0 / wsum.max(1e-6));
                    pd[(y - y0) * w + x] = zmin;
                }
            }
        });
    }
    mark(4);
    drop(depth);
    drop(tri);
    drop(mirror);
    lines(geo, rig, &shadow, cam, &pdepth, &mut col, w, h, crew);
    tubes(
        &thin,
        geo,
        rig,
        &shadow,
        cam,
        &mut pdepth,
        &mut col,
        w,
        h,
        crew,
    );
    mark(5);
    if q.full {
        post(rig, cam, &mut col, &pdepth, w, h, crew);
        peak = peak.max(base + w * h * (12 * 2 + 4));
    }
    // To pixels: the exposure, the tone curve, a vignette and a dither.
    let mut px = vec![0u8; w * h * 4];
    let colr = &col;
    let ex = rig.exposure;
    let sat = 0.85;
    rows1(h, w * 4, crew, &mut px, |y0, y1, out| {
        for y in y0..y1 {
            for x in 0..w {
                let c = colr[y * w + x];
                let (u, v) = (x as f32 / w as f32 - 0.5, y as f32 / h as f32 - 0.47);
                let r = (u * u * 1.6 + v * v * 2.2).sqrt();
                let vig = 1.0 - 0.38 * ((r - 0.38) / 0.55).clamp(0.0, 1.0).powf(1.6);
                let dither = (noise(x as f32 * 0.73, y as f32 * 0.91, 7) - 0.5) / 255.0;
                let t = tone(c.map(|v| v * vig * ex), sat);
                let to8 = |t: f32| {
                    ((egui::ecolor::gamma_u8_from_linear_f32(t) as f32 / 255.0 + dither)
                        .clamp(0.0, 1.0)
                        * 255.0)
                        .round() as u8
                };
                let o = ((y - y0) * w + x) * 4;
                out[o..o + 4].copy_from_slice(&[to8(t[0]), to8(t[1]), to8(t[2]), 255]);
            }
        }
    });
    let img = egui::ColorImage::from_rgba_premultiplied([w, h], &px);
    mark(6);
    let stats = Stats {
        ms: t0.elapsed().as_secs_f64() * 1e3,
        passes,
        peak,
        tris: geo.tris.len(),
    };
    (img, stats)
}

/// Depth of field behind the instrument, and bloom.
fn post(rig: &Rig, cam: &Camera, col: &mut [Lin], pdepth: &[f32], w: usize, h: usize, crew: &Crew) {
    let far_end = rig.focus_far;
    let mut far = col.to_vec();
    // r42:arcus-upper — behind a realistic figure: a wider blur, reached sooner.
    let (radius, ramp) = if rig.soft_bg > 0.0 {
        (((cam.k / 300.0) as usize).clamp(2, 9), 0.22)
    } else {
        (((cam.k / 700.0) as usize).clamp(1, 4), 0.6)
    };
    let coc_at = |z: f32| {
        if z == f32::MAX {
            1.0
        } else {
            ((z - far_end) / (far_end * ramp)).clamp(0.0, 1.0)
        }
    };
    if rig.soft_bg > 0.0 {
        // Only what lies behind is blurred into what lies behind (each sample weighted by
        // how far behind it is, the sum over the weights): a wide blur of everything drew
        // the figure's bright hair out into a halo round it.
        let mut wt: Vec<Lin> = pdepth.iter().map(|&z| [coc_at(z); 3]).collect();
        for (c, m) in far.iter_mut().zip(wt.iter()) {
            *c = lscale(*c, m[0]);
        }
        blur(&mut far, w, h, radius, crew);
        blur(&mut wt, w, h, radius, crew);
        for ((c, m), o) in far.iter_mut().zip(wt.iter()).zip(col.iter()) {
            *c = if m[0] > 1e-3 {
                lscale(*c, 1.0 / m[0])
            } else {
                *o
            };
        }
    } else {
        blur(&mut far, w, h, radius, crew);
    }
    {
        let far = &far;
        rows1(h, w, crew, col, |y0, y1, c| {
            for k in y0 * w..y1 * w {
                let coc = coc_at(pdepth[k]);
                if coc > 0.0 {
                    let o = &mut c[k - y0 * w];
                    *o = lmix(*o, far[k], coc);
                }
            }
        });
    }
    drop(far);
    let (bw, bh) = (w / 4, h / 4);
    if bw <= 4 || bh <= 4 {
        return;
    }
    let mut bright = vec![[0.0f32; 3]; bw * bh];
    let ex = rig.exposure.max(1e-3);
    // Round 40 (visual): only a real light blooms — a chrome hoop's glint is a fine line,
    // not a white halo round the drum (each sample's part capped).
    let (thr, amt) = (3.2, 0.18);
    {
        let colr = &*col;
        rows1(bh, bw, crew, &mut bright, |y0, y1, out| {
            for y in y0..y1 {
                for x in 0..bw {
                    let mut acc = [0.0f32; 3];
                    for dy in 0..4 {
                        for dx in 0..4 {
                            let c = colr[(y * 4 + dy) * w + x * 4 + dx];
                            acc = ladd(
                                acc,
                                std::array::from_fn(|i| (c[i] * ex - thr).clamp(0.0, 4.0)),
                            );
                        }
                    }
                    out[(y - y0) * bw + x] = lscale(acc, 1.0 / (16.0 * ex));
                }
            }
        });
    }
    let mut wide = bright.clone();
    blur(&mut bright, bw, bh, 2, crew);
    blur(&mut wide, bw, bh, 7, crew);
    rows1(h, w, crew, col, |y0, y1, cc| {
        for y in y0..y1 {
            for x in 0..w {
                let k = (y / 4).min(bh - 1) * bw + (x / 4).min(bw - 1);
                let b = ladd(lscale(bright[k], 0.5 * amt), lscale(wide[k], 0.6 * amt));
                let c = &mut cc[(y - y0) * w + x];
                *c = ladd(*c, b);
            }
        }
    });
}
