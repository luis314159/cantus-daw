//! The baker (ADR 0104): our own CPU renderer for the scene. A depth-buffered rasterizer
//! writes, per sample, which triangle is seen and where on it; every sample is then shaded
//! (`shade.rs`) with the key light's shadow (a shadow map from the spot, filtered soft),
//! ambient occlusion from the depth buffer, and what its gloss reflects; thin lines (the
//! strings, the boards' joints) are drawn anti-aliased against the depth; a polished floor
//! gets the piano's mirror image (a second pass, blurred); then depth of field on the far
//! room, bloom round the lights, a filmic tone curve, a vignette and a dither, and the
//! samples are averaged down to the image's pixels. Rows are shared among worker threads;
//! nothing here touches the UI thread.

use super::geo::{Geo, Line, Tri, M};
use super::mesh::{ladd, lmix, lscale, v3, Camera, Lin, V3};
use super::shade::{fresnel, Stage};
use egui::Pos2;

/// How finely to bake.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Quality {
    /// Samples per pixel side (1 or 2).
    pub ss: usize,
    /// Ambient occlusion, the floor's mirror, depth of field and bloom.
    pub full: bool,
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

fn project(cam: &Camera, tris: &[Tri], out: &mut Vec<Option<PTri>>) {
    out.clear();
    for t in tris {
        let mut ok = true;
        let mut x = [0.0; 3];
        let mut y = [0.0; 3];
        let mut iz = [0.0; 3];
        for i in 0..3 {
            let z = (if cam.flip {
                V3 {
                    y: -t.v[i].p.y,
                    ..t.v[i].p
                }
            } else {
                t.v[i].p
            } - cam.eye)
                .dot(cam.fwd);
            if z < 0.05 {
                ok = false;
                break;
            }
            let q = cam.project(t.v[i].p);
            x[i] = q.x;
            y[i] = q.y;
            iz[i] = 1.0 / z;
        }
        if !ok {
            out.push(None);
            continue;
        }
        let lo = Pos2::new(x[0].min(x[1]).min(x[2]), y[0].min(y[1]).min(y[2]));
        let hi = Pos2::new(x[0].max(x[1]).max(x[2]), y[0].max(y[1]).max(y[2]));
        out.push(Some(PTri { x, y, iz, lo, hi }));
    }
}

/// Rasterize `pt` into rows `y0..y1` of a `w`-wide buffer: depth, triangle, barycentrics.
fn raster_rows(
    pt: &[Option<PTri>],
    w: usize,
    y0: usize,
    y1: usize,
    depth: &mut [f32],
    tri: &mut [u32],
    bary: &mut [[f32; 2]],
) {
    for (i, t) in pt.iter().enumerate() {
        let Some(t) = t else { continue };
        if t.hi.y < y0 as f32 || t.lo.y > y1 as f32 || t.hi.x < 0.0 || t.lo.x > w as f32 {
            continue;
        }
        let area = (t.x[1] - t.x[0]) * (t.y[2] - t.y[0]) - (t.x[2] - t.x[0]) * (t.y[1] - t.y[0]);
        if area.abs() < 1e-8 {
            continue;
        }
        let inv = 1.0 / area;
        let ya = ((t.lo.y - 0.5).ceil().max(y0 as f32)) as usize;
        let yb = ((t.hi.y - 0.5).floor() + 1.0).min(y1 as f32).max(0.0) as usize;
        let xa = ((t.lo.x - 0.5).ceil().max(0.0)) as usize;
        let xb = ((t.hi.x - 0.5).floor() + 1.0).min(w as f32).max(0.0) as usize;
        // A sliver's edge functions are too steep to bound a row by; it walks its box.
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
                let a = w0 * t.iz[0];
                let b = w1 * t.iz[1];
                let c = w2 * t.iz[2];
                let s = a + b + c;
                let z = 1.0 / s;
                let k = row + px;
                if z < depth[k] {
                    depth[k] = z;
                    tri[k] = i as u32;
                    bary[k] = [b / s, c / s];
                }
            }
        }
    }
}

/// The samples of row `sy` that can be inside `t` (whose doubled area's inverse is `inv`),
/// within `xa..xb`: each edge function is linear along the row, so the row's inside is an
/// interval. It is widened by two samples and every sample in it still takes the exact test,
/// so the picture is the box walk's to the bit — only the samples plainly outside are skipped
/// (a large thin triangle's box is mostly empty: the key light's map tested ~60 samples for
/// each it kept).
fn span(t: &PTri, inv: f32, sy: f32, xa: usize, xb: usize) -> (usize, usize) {
    let (inv, sy) = (inv as f64, sy as f64);
    let (x, y) = (t.x.map(|v| v as f64), t.y.map(|v| v as f64));
    // w_i(sx) = c_i + d_i · sx for the edges opposite vertices 0 and 1; w_2 = 1 − w_0 − w_1.
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
    // sx = px + 0.5.
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

/// The bake's workers, made once per bake and kept for all its passes (round 39, fixes): the
/// rows' passes and each blur used to start their own scoped threads — up to 13 a call,
/// about twenty calls a full bake, a few hundred thread starts (several milliseconds on
/// Windows). `threads` in all, the baking thread among them; a pass hands out its bands to
/// whichever is free, so the bits never depend on how many there are.
pub(super) struct Crew {
    pool: Option<rayon::ThreadPool>,
    threads: usize,
}

impl Crew {
    pub(super) fn new(threads: usize) -> Crew {
        let threads = threads.max(1);
        let pool = if threads > 1 {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads - 1)
                .thread_name(|i| format!("cantus-chorda-bake-{i}"))
                .build()
                .ok()
        } else {
            None
        };
        let threads = if pool.is_some() { threads } else { 1 };
        Crew { pool, threads }
    }

    /// Runs `work` on as many of the crew as there are `parts` (at most all of it, this
    /// thread among them), and returns when every one has returned.
    fn run(&self, parts: usize, work: &(impl Fn() + Sync)) {
        let n = self.threads.min(parts.max(1));
        match &self.pool {
            Some(pool) if n > 1 => pool.in_place_scope(|s| {
                for _ in 1..n {
                    s.spawn(|_| work());
                }
                work();
            }),
            _ => work(),
        }
    }

    /// Runs `f` on each of `items`, one item to a worker (the last on this thread), and
    /// returns when every one is done.
    fn each<T: Send>(&self, mut items: Vec<T>, f: impl Fn(usize, T) + Sync) {
        let Some(last) = items.pop() else { return };
        let n = items.len();
        match &self.pool {
            Some(pool) if n > 0 => pool.in_place_scope(|s| {
                let f = &f;
                for (i, item) in items.into_iter().enumerate() {
                    s.spawn(move |_| f(i, item));
                }
                f(n, last);
            }),
            _ => {
                for (i, item) in items.into_iter().enumerate() {
                    f(i, item);
                }
                f(n, last);
            }
        }
    }

    /// How many work at once.
    fn threads(&self) -> usize {
        self.threads
    }
}

/// Rows a thread takes at a time. The scene's cost is uneven down the picture (the piano's
/// rows cost many times the sky's), so the rows go out in small bands to whichever thread is
/// free instead of one block each — a fixed split left most threads idle while one finished
/// the piano. A row is computed the same whoever takes it: the bits never depend on it.
const BAND: usize = 8;

/// Run `f(first_row, end_row, chunk_a, chunk_b, chunk_c)` over the rows (each `w` long) of
/// three buffers, in bands of [`BAND`] rows shared among the `crew`.
fn rows3<A: Send, B: Send, C: Send>(
    h: usize,
    w: usize,
    crew: &Crew,
    a: &mut [A],
    b: &mut [B],
    c: &mut [C],
    f: impl Fn(usize, usize, &mut [A], &mut [B], &mut [C]) + Sync,
) {
    if h == 0 || w == 0 {
        return;
    }
    let n = BAND * w;
    let bands = std::sync::Mutex::new(
        a[..h * w]
            .chunks_mut(n)
            .zip(b[..h * w].chunks_mut(n))
            .zip(c[..h * w].chunks_mut(n))
            .enumerate(),
    );
    let work = || loop {
        let next = bands.lock().unwrap().next();
        let Some((i, ((a, b), c))) = next else { break };
        let y = i * BAND;
        f(y, y + a.len() / w, a, b, c);
    };
    crew.run(h.div_ceil(BAND), &work);
}

/// Run `f(first_row, end_row, chunk)` over the rows (each `w` long) of `a`, in bands of
/// [`BAND`] rows shared among the `crew`.
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
        let next = bands.lock().unwrap().next();
        let Some((i, a)) = next else { break };
        let y = i * BAND;
        f(y, y + a.len() / w, a);
    };
    crew.run(h.div_ceil(BAND), &work);
}

/// A depth buffer of `geo` seen from `cam`: samples' triangle and barycentrics.
struct GBuf {
    w: usize,
    h: usize,
    depth: Vec<f32>,
    tri: Vec<u32>,
    bary: Vec<[f32; 2]>,
}

fn gbuf(geo: &Geo, cam: &Camera, w: usize, h: usize, crew: &Crew) -> GBuf {
    let mut pt = Vec::with_capacity(geo.tris.len());
    project(cam, &geo.tris, &mut pt);
    let mut depth = vec![f32::MAX; w * h];
    let mut tri = vec![u32::MAX; w * h];
    let mut bary = vec![[0.0f32; 2]; w * h];
    rows3(
        h,
        w,
        crew,
        &mut depth,
        &mut tri,
        &mut bary,
        |y0, y1, d, t, b| raster_rows(&pt, w, y0, y1, d, t, b),
    );
    GBuf {
        w,
        h,
        depth,
        tri,
        bary,
    }
}

/// The key light's shadow map: depth from the spot.
struct Shadow {
    cam: Camera,
    n: usize,
    depth: Vec<f32>,
}

impl Shadow {
    fn of(geo: &Geo, st: &Stage, n: usize, crew: &Crew) -> Shadow {
        let cam = Camera::look(
            st.key_pos,
            st.key_aim * 4.0 + st.key_pos,
            n as f32 * 0.32,
            Pos2::new(n as f32 * 0.5, n as f32 * 0.5),
        );
        let g = gbuf(geo, &cam, n, n, crew);
        Shadow {
            cam,
            n,
            depth: g.depth,
        }
    }

    /// How much of the key light reaches `p` (normal `n`): 0 … 1, soft.
    fn light(&self, p: V3, n: V3) -> f32 {
        let q = p + n * 0.012;
        let z = (q - self.cam.eye).dot(self.cam.fwd);
        if z < 0.05 {
            return 1.0;
        }
        let s = self.cam.project(q);
        let mut lit = 0.0;
        let mut cnt = 0.0;
        for dy in -2..=2 {
            for dx in -2..=2 {
                let x = (s.x + dx as f32 * 1.3) as isize;
                let y = (s.y + dy as f32 * 1.3) as isize;
                cnt += 1.0;
                if x < 0 || y < 0 || x >= self.n as isize || y >= self.n as isize {
                    lit += 1.0;
                    continue;
                }
                let d = self.depth[y as usize * self.n + x as usize];
                if z <= d + 0.02 + 0.004 * z {
                    lit += 1.0;
                }
            }
        }
        lit / cnt
    }
}

/// A sample's place: the point, its normal, its texture coordinates.
fn surface(geo: &Geo, t: u32, b: [f32; 2]) -> (V3, V3, [f32; 2], M, u8) {
    let tri = &geo.tris[t as usize];
    let b0 = 1.0 - b[0] - b[1];
    let v = &tri.v;
    let p = v[0].p * b0 + v[1].p * b[0] + v[2].p * b[1];
    let n = (v[0].n * b0 + v[1].n * b[0] + v[2].n * b[1]).norm();
    let uv = [
        v[0].uv[0] * b0 + v[1].uv[0] * b[0] + v[2].uv[0] * b[1],
        v[0].uv[1] * b0 + v[1].uv[1] * b[0] + v[2].uv[1] * b[1],
    ];
    (p, n, uv, tri.mat, tri.k)
}

/// The light leaving a surface toward `eye`.
#[allow(clippy::too_many_arguments)]
fn light_at(
    st: &Stage,
    sh: Option<&Shadow>,
    eye: V3,
    p: V3,
    n: V3,
    uv: [f32; 2],
    mat: M,
    k: u8,
    fp: f32,
    ao: f32,
) -> Lin {
    let s = st.surf(mat, k, p, uv, fp);
    if let Some(g) = s.glow {
        return g;
    }
    let v = (eye - p).norm();
    let n = if n.dot(v) < 0.0 { -n } else { n };
    let (kc, l) = st.key_at(p);
    let vis = if kc[0] + kc[1] + kc[2] > 1e-4 {
        sh.map_or(1.0, |sh| sh.light(p, n))
    } else {
        0.0
    };
    let ndl = n.dot(l).max(0.0);
    let ndv = n.dot(v).clamp(0.0, 1.0);
    let diff_k = if s.metal { 0.25 } else { 1.0 };
    let fill = st.fill(n);
    let rim = st.rim_dir;
    let rim_l = n.dot(rim).max(0.0) * (1.0 - ndv).powi(2);
    // The light the lit floor and rug throw back up: what faces down (a lid's underside,
    // the case's belly) is never black.
    let below = (-n.y).max(0.0);
    let (kf, _) = st.key_at(v3(p.x, 0.0, p.z));
    let mut c = [0.0f32; 3];
    for i in 0..3 {
        let bounce = kf[i] * 0.16 * below * ao;
        c[i] = s.base[i] * diff_k * (kc[i] * ndl * vis + fill[i] * ao + bounce)
            + st.rim[i] * rim_l * 0.6 * s.base[i].max(0.08);
    }
    let h = (l + v).norm();
    let norm = (s.shine + 8.0) / 25.0;
    let spec = n.dot(h).max(0.0).powf(s.shine)
        * s.spec
        * norm
        * vis
        * fresnel(n.dot(h).clamp(0.0, 1.0)).max(if s.metal { 0.6 } else { 0.0 });
    let r = n * (2.0 * ndv) - v;
    let mut e = st.env(p, r);
    let f = if s.metal { 1.0 } else { fresnel(ndv) };
    if s.metal {
        e = [
            e[0] * s.base[0] * 1.6,
            e[1] * s.base[1] * 1.6,
            e[2] * s.base[2] * 1.6,
        ];
    }
    let refl = lscale(e, f * s.coat * (0.5 + 0.5 * ao));
    let tint = if s.metal { s.base } else { [1.0; 3] };
    for i in 0..3 {
        c[i] += refl[i] + kc[i] * spec * tint[i] * 0.35;
    }
    st.hazed(p, c)
}

/// Shade a G-buffer into linear colour (`ao` the occlusion per sample, 1 none).
fn shade_buf(
    geo: &Geo,
    st: &Stage,
    sh: Option<&Shadow>,
    cam: &Camera,
    g: &GBuf,
    ao: &[f32],
    crew: &Crew,
) -> Vec<Lin> {
    let mut out = vec![[0.0f32; 3]; g.w * g.h];
    let eye = if cam.flip {
        V3 {
            y: -cam.eye.y,
            ..cam.eye
        }
    } else {
        cam.eye
    };
    let w = g.w;
    rows1(g.h, w, crew, &mut out, |y0, y1, o| {
        for y in y0..y1 {
            for x in 0..w {
                let k = y * w + x;
                let t = g.tri[k];
                if t == u32::MAX {
                    o[(y - y0) * w + x] = lscale(st.sky, 0.5);
                    continue;
                }
                let (p, n, uv, m, kk) = surface(geo, t, g.bary[k]);
                let fp = g.depth[k] / cam.k;
                o[(y - y0) * w + x] = light_at(st, sh, eye, p, n, uv, m, kk, fp, ao[k]);
            }
        }
    });
    out
}

/// Ambient occlusion from the depth buffer: how many nearby samples stand in front.
fn occlusion(g: &GBuf, k_px: f32, crew: &Crew) -> Vec<f32> {
    let mut ao = vec![1.0f32; g.w * g.h];
    let (w, h) = (g.w, g.h);
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
    rows1(h, w, crew, &mut ao, |y0, y1, o| {
        for y in y0..y1 {
            for x in 0..w {
                let z = g.depth[y * w + x];
                if z == f32::MAX {
                    continue;
                }
                let r = (0.16 * k_px / z).clamp(2.0, 60.0);
                let mut occ = 0.0;
                for (i, (dx, dy)) in DIRS.iter().enumerate() {
                    let rr = r * (0.45 + 0.55 * ((i * 5 + x + y) % 4) as f32 / 3.0);
                    let sx = (x as f32 + dx * rr) as isize;
                    let sy = (y as f32 + dy * rr) as isize;
                    if sx < 0 || sy < 0 || sx >= w as isize || sy >= h as isize {
                        continue;
                    }
                    let d = g.depth[sy as usize * w + sx as usize];
                    let diff = z - d;
                    if diff > 0.015 && diff < 0.6 {
                        occ += 1.0 - diff / 0.6;
                    }
                }
                o[(y - y0) * w + x] = (1.0 - occ / 8.0 * 1.1).clamp(0.25, 1.0);
            }
        }
    });
    ao
}

/// Draw the lines over the shaded samples, anti-aliased, where nothing stands in front.
fn lines(
    geo: &Geo,
    st: &Stage,
    sh: Option<&Shadow>,
    cam: &Camera,
    g: &GBuf,
    col: &mut [Lin],
    crew: &Crew,
) {
    /// One line, projected and lit, ready to cover its samples.
    struct Stroke {
        pa: Pos2,
        d: egui::Vec2,
        len: f32,
        dir: egui::Vec2,
        nrm: egui::Vec2,
        wpx: f32,
        cover: f32,
        alpha: f32,
        za: f32,
        zb: f32,
        c: Lin,
        x0: usize,
        x1: usize,
        y0: usize,
        y1: usize,
    }
    let eye = cam.eye;
    let mut strokes = Vec::with_capacity(geo.lines.len());
    for l in &geo.lines {
        let Line {
            a,
            b,
            r,
            mat,
            alpha,
        } = *l;
        let za = (a - eye).dot(cam.fwd);
        let zb = (b - eye).dot(cam.fwd);
        if za < 0.05 || zb < 0.05 {
            continue;
        }
        let (pa, pb) = (cam.project(a), cam.project(b));
        let mid = a.lerp(b, 0.5);
        let width = 2.0 * r * cam.k / ((mid - eye).dot(cam.fwd));
        let (wpx, cover) = if width < 1.0 {
            (1.0, width.max(0.05))
        } else {
            (width, 1.0)
        };
        let v = (eye - mid).norm();
        let axis = (b - a).norm();
        let side = (v - axis * v.dot(axis)).norm();
        let (_, lk) = st.key_at(mid);
        let n = (side + (lk - axis * lk.dot(axis)).norm() * 0.6).norm();
        let c = light_at(st, sh, eye, mid, n, [mid.x, mid.z], mat, 0, 0.001, 1.0);
        let d = pb - pa;
        let len = d.length().max(1e-3);
        let dir = d / len;
        let nrm = egui::vec2(-dir.y, dir.x);
        let hw = wpx * 0.5 + 0.75;
        let lo = pa.min(pb) - egui::vec2(hw, hw);
        let hi = pa.max(pb) + egui::vec2(hw, hw);
        // Clamped before the cast: a cable's end near the lens projects far off the image.
        strokes.push(Stroke {
            pa,
            d,
            len,
            dir,
            nrm,
            wpx,
            cover,
            alpha,
            za,
            zb,
            c,
            x0: lo.x.clamp(0.0, g.w as f32) as usize,
            x1: (hi.x + 1.0).clamp(0.0, g.w as f32) as usize,
            y0: lo.y.clamp(0.0, g.h as f32) as usize,
            y1: (hi.y + 1.0).clamp(0.0, g.h as f32) as usize,
        });
    }
    // Every band of rows draws every line in order over its own rows, so where lines cross
    // they blend in the same order as one thread would. A long string's box is mostly empty:
    // a row walks only the samples near the line (|q · nrm| is a sample's distance to it,
    // linear along the row; widened by two samples, each still takes the exact test).
    let w = g.w;
    rows1(g.h, w, crew, col, |r0, r1, out| {
        for s in &strokes {
            let reach = (s.wpx * 0.5 + 0.5) as f64;
            for y in s.y0.max(r0)..s.y1.min(r1) {
                let (mut x0, mut x1) = (s.x0, s.x1);
                let (nx, ny) = (s.nrm.x as f64, s.nrm.y as f64);
                if nx.abs() > 1e-6 {
                    let base = (y as f64 + 0.5 - s.pa.y as f64) * ny;
                    // (x + 0.5 − pa.x)·nx + base ∈ [−reach, reach]
                    let ea = (-reach - base) / nx + s.pa.x as f64 - 0.5;
                    let eb = (reach - base) / nx + s.pa.x as f64 - 0.5;
                    let (lo, hi) = (ea.min(eb).floor() - 2.0, ea.max(eb).ceil() + 3.0);
                    if lo.is_finite() && hi.is_finite() {
                        x0 = lo.clamp(s.x0 as f64, s.x1 as f64) as usize;
                        x1 = (hi.clamp(s.x0 as f64, s.x1 as f64) as usize).max(x0);
                    }
                }
                for x in x0..x1 {
                    let q = Pos2::new(x as f32 + 0.5, y as f32 + 0.5) - s.pa;
                    let t = (q.dot(s.dir) / s.len).clamp(0.0, 1.0);
                    let dist = (q - s.d * t).dot(s.nrm).abs();
                    let cov = ((s.wpx * 0.5 + 0.5 - dist).clamp(0.0, 1.0)) * s.cover * s.alpha;
                    if cov <= 0.0 {
                        continue;
                    }
                    let k = y * w + x;
                    let z = 1.0 / ((1.0 - t) / s.za + t / s.zb);
                    if z > g.depth[k] + 0.01 + 0.002 * z {
                        continue;
                    }
                    let o = &mut out[(y - r0) * w + x];
                    *o = lmix(*o, s.c, cov);
                }
            }
        }
    });
}

/// A separable box blur of `img` (`w` × `h`), radius `r`, run twice (near a Gaussian).
fn blur(img: &mut [Lin], w: usize, h: usize, r: usize, crew: &Crew) {
    if r == 0 || w == 0 || h == 0 {
        return;
    }
    let mut tmp = vec![[0.0f32; 3]; w * h];
    for _ in 0..2 {
        // Along the rows: each row alone.
        {
            let img = &*img;
            rows1(h, w, crew, &mut tmp, |y0, y1, out| {
                for y in y0..y1 {
                    let mut acc = [0.0f32; 3];
                    let row = &img[y * w..(y + 1) * w];
                    for v in row.iter().take(r.min(w - 1) + 1) {
                        acc = ladd(acc, *v);
                    }
                    let o = &mut out[(y - y0) * w..(y - y0 + 1) * w];
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
            });
        }
        // Down the columns: each column's running sum in its own order, the columns shared
        // among the threads in bands (walked row by row, for the cache).
        let bands = crew.threads().clamp(1, w);
        let per = w.div_ceil(bands);
        let mut cols: Vec<Vec<&mut [Lin]>> = (0..bands).map(|_| Vec::with_capacity(h)).collect();
        for row in img[..w * h].chunks_mut(w) {
            let mut rest = row;
            for band in cols.iter_mut() {
                let n = per.min(rest.len());
                let (a, t) = rest.split_at_mut(n);
                band.push(a);
                rest = t;
            }
        }
        let tmp = &tmp;
        crew.each(cols, |b, mut band| {
            let x0 = b * per;
            let bw = band.first().map_or(0, |r| r.len());
            if bw == 0 {
                return;
            }
            {
                let mut acc = vec![[0.0f32; 3]; bw];
                for y in 0..=r.min(h - 1) {
                    for (cx, a) in acc.iter_mut().enumerate() {
                        *a = ladd(*a, tmp[y * w + x0 + cx]);
                    }
                }
                for (y, out) in band.iter_mut().enumerate() {
                    let n = (y + r).min(h - 1) - y.saturating_sub(r) + 1;
                    for (cx, a) in acc.iter_mut().enumerate() {
                        out[cx] = lscale(*a, 1.0 / n as f32);
                        if y + r + 1 < h {
                            *a = ladd(*a, tmp[(y + r + 1) * w + x0 + cx]);
                        }
                        if y >= r {
                            let o = tmp[(y - r) * w + x0 + cx];
                            *a = [a[0] - o[0], a[1] - o[1], a[2] - o[2]];
                        }
                    }
                }
            }
        });
    }
}

/// The filmic curve (Narkowicz's fit of ACES).
fn aces(x: f32) -> f32 {
    let x = x.max(0.0);
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// Bake `geo` (and the floor's mirror image `mirror`, when the floor is polished: its
/// strength) into an image `w` × `h` pixels; `cam` is the camera in those pixels.
#[allow(clippy::too_many_arguments)]
pub fn bake(
    geo: &Geo,
    mirror: Option<(&Geo, f32)>,
    st: &Stage,
    cam: &Camera,
    w: usize,
    h: usize,
    q: Quality,
    threads: usize,
) -> egui::ColorImage {
    let ss = q.ss.max(1);
    let (sw, sh_) = (w * ss, h * ss);
    let scam = Camera {
        k: cam.k * ss as f32,
        c: Pos2::new(cam.c.x * ss as f32, cam.c.y * ss as f32),
        ..*cam
    };
    let crew = &Crew::new(threads);
    let shadow = Shadow::of(geo, st, if q.full { 1536 } else { 768 }, crew);
    let g = gbuf(geo, &scam, sw, sh_, crew);
    let ao = if q.full {
        occlusion(&g, scam.k, crew)
    } else {
        vec![1.0; sw * sh_]
    };
    let mut col = shade_buf(geo, st, Some(&shadow), &scam, &g, &ao, crew);
    lines(geo, st, Some(&shadow), &scam, &g, &mut col, crew);
    // The polished floor's mirror image, at half the samples, blurred as a gloss blurs it.
    if let (Some((mg, strength)), true) = (mirror, q.full) {
        let (mw, mh) = (sw / 2, sh_ / 2);
        let mcam = Camera {
            k: scam.k * 0.5,
            c: Pos2::new(scam.c.x * 0.5, scam.c.y * 0.5),
            flip: true,
            ..scam
        };
        let mgb = gbuf(mg, &mcam, mw, mh, crew);
        let mao = vec![1.0; mw * mh];
        let mut mc = shade_buf(mg, st, Some(&shadow), &mcam, &mgb, &mao, crew);
        for (i, t) in mgb.tri.iter().enumerate() {
            if *t == u32::MAX {
                mc[i] = [0.0; 3];
            }
        }
        blur(&mut mc, mw, mh, 2, crew);
        rows1(sh_, sw, crew, &mut col, |y0, y1, c| {
            for y in y0..y1 {
                for x in 0..sw {
                    let k = y * sw + x;
                    let t = g.tri[k];
                    if t == u32::MAX || geo.tris[t as usize].mat != M::Floor {
                        continue;
                    }
                    let m = mc[(y / 2).min(mh - 1) * mw + (x / 2).min(mw - 1)];
                    let fall = ao[k];
                    let c = &mut c[(y - y0) * sw + x];
                    *c = ladd(*c, lscale(m, strength * fall));
                }
            }
        });
    }
    if q.full {
        // Depth of field: the far room softens behind the piano.
        // In focus: the whole instrument, to its farthest corner (a grand's tail, an
        // upright's back) and a little beyond; only what stands behind it softens.
        let far_end = st
            .piano
            .frame_points()
            .iter()
            .map(|q| cam.depth(*q))
            .fold(0.0f32, f32::max)
            + 0.6;
        let mut far = col.clone();
        blur(&mut far, sw, sh_, (2 * ss).max(2), crew);
        rows1(sh_, sw, crew, &mut col, |y0, y1, c| {
            for k in y0 * sw..y1 * sw {
                let z = g.depth[k];
                let coc = ((z - far_end) / (far_end * 0.8)).clamp(0.0, 1.0);
                if coc > 0.0 {
                    let c = &mut c[k - y0 * sw];
                    *c = lmix(*c, far[k], coc);
                }
            }
        });
        // Bloom: what is brighter than white spills round itself.
        let (bw, bh) = (sw / 4, sh_ / 4);
        if bw > 4 && bh > 4 {
            let mut bright = vec![[0.0f32; 3]; bw * bh];
            let colr = &col;
            rows1(bh, bw, crew, &mut bright, |y0, y1, out| {
                for y in y0..y1 {
                    for x in 0..bw {
                        let mut acc = [0.0f32; 3];
                        for dy in 0..4 {
                            for dx in 0..4 {
                                let c = colr[(y * 4 + dy) * sw + x * 4 + dx];
                                acc = ladd(
                                    acc,
                                    [
                                        (c[0] - 0.9).max(0.0),
                                        (c[1] - 0.9).max(0.0),
                                        (c[2] - 0.9).max(0.0),
                                    ],
                                );
                            }
                        }
                        out[(y - y0) * bw + x] = lscale(acc, 1.0 / 16.0);
                    }
                }
            });
            blur(&mut bright, bw, bh, 3 * ss, crew);
            let mut wide = bright.clone();
            blur(&mut wide, bw, bh, 9 * ss, crew);
            rows1(sh_, sw, crew, &mut col, |y0, y1, cc| {
                for y in y0..y1 {
                    for x in 0..sw {
                        let k = (y / 4).min(bh - 1) * bw + (x / 4).min(bw - 1);
                        let b = ladd(lscale(bright[k], 0.7), lscale(wide[k], 0.8));
                        let c = &mut cc[(y - y0) * sw + x];
                        *c = ladd(*c, b);
                    }
                }
            });
        }
    }
    // Down to pixels, through the tone curve, the vignette and a dither.
    let mut px = vec![0u8; w * h * 4];
    let n = (ss * ss) as f32;
    let col = &col;
    rows1(h, w * 4, crew, &mut px, |y0, y1, out| {
        for y in y0..y1 {
            for x in 0..w {
                let mut acc = [0.0f32; 3];
                for dy in 0..ss {
                    for dx in 0..ss {
                        acc = ladd(acc, col[(y * ss + dy) * sw + x * ss + dx]);
                    }
                }
                let c = lscale(acc, 1.0 / n);
                let (u, v) = (x as f32 / w as f32 - 0.5, y as f32 / h as f32 - 0.47);
                let r = (u * u * 1.6 + v * v * 2.2).sqrt();
                let vig = 1.0 - 0.45 * ((r - 0.35) / 0.55).clamp(0.0, 1.0).powf(1.6);
                let dither =
                    (super::shade::noise(x as f32 * 0.73, y as f32 * 0.91, 7) - 0.5) / 255.0;
                let to8 = |v: f32| {
                    let t = aces(v * vig * 1.35);
                    (egui::ecolor::gamma_u8_from_linear_f32(t) as f32 / 255.0 + dither)
                        .clamp(0.0, 1.0)
                };
                let o = ((y - y0) * w + x) * 4;
                out[o..o + 4].copy_from_slice(&[
                    (to8(c[0]) * 255.0).round() as u8,
                    (to8(c[1]) * 255.0).round() as u8,
                    (to8(c[2]) * 255.0).round() as u8,
                    255,
                ]);
            }
        }
    });
    egui::ColorImage::from_rgba_premultiplied([w, h], &px)
}
