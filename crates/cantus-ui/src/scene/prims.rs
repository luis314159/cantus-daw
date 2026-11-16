//! The 3-D primitives every model is built from (port of `scene/prims.py`): a shaded facet,
//! a tube with real thickness, a box, an extruded outline, and the shadows they leave on
//! the floor.

use super::backend::Prim;
use super::display::{lin, Cap, Layer, List, Paint, Part};
use super::geo::{hull, ring, v, Camera, V3};
use super::light::{grain, Light};
use crate::kit::skin::scene::{ink, Mat, C3};
use egui::Pos2;

/// The level of detail: a shell's facets, a cymbal's wedges, its lathe rings and dents.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Lod {
    pub shell: usize,
    pub wedges: usize,
    pub lathe: usize,
    pub dents: usize,
    /// A hoop's facets, a shell's grain lines.
    pub hoop: usize,
    pub grain: usize,
}

pub const HIGH: Lod = Lod {
    shell: 240,
    wedges: 180,
    lathe: 64,
    dents: 90,
    hoop: 200,
    grain: 54,
};
pub const LOW: Lod = Lod {
    shell: 120,
    wedges: 96,
    lathe: 28,
    dents: 36,
    hoop: 120,
    grain: 24,
};

/// What a scene is being built with: the camera (or its mirror), the light, the list, and
/// the shadows and contacts collected for the floor.
pub struct Ctx<'a> {
    pub cam: Camera,
    pub light: Light,
    pub p: &'a mut List,
    pub shadow_polys: Vec<(Vec<V3>, f32)>,
    pub shadow_segs: Vec<(V3, V3, f32)>,
    pub contacts: Vec<(V3, f32)>,
    pub mirror: bool,
    /// The part the ops being emitted belong to.
    pub piece: Part,
    pub lod: Lod,
    /// The 3-D primitives, kept for a per-pixel backend when asked for.
    pub capture: Option<Vec<Prim>>,
}

impl<'a> Ctx<'a> {
    pub fn new(cam: Camera, light: Light, p: &'a mut List, lod: Lod) -> Ctx<'a> {
        Ctx {
            mirror: cam.mirror,
            cam,
            light,
            p,
            shadow_polys: Vec::new(),
            shadow_segs: Vec::new(),
            contacts: Vec::new(),
            piece: Part::None,
            lod,
            capture: None,
        }
    }

    /// Keep a facet for a per-pixel backend (when one asked).
    pub fn keep_facet(&mut self, pts: &[V3], n: V3, mat: &Mat, grain: f32) {
        let part = self.piece;
        if let Some(v) = self.capture.as_mut() {
            v.push(Prim::Facet {
                pts: pts.to_vec(),
                n,
                mat: *mat,
                grain,
                part,
            });
        }
    }

    /// Toward the eye from `p`.
    pub fn view(&self, p: V3) -> V3 {
        (self.cam.eye() - p).norm()
    }

    pub fn sp(&self, p: V3) -> Pos2 {
        self.cam.p(p)
    }

    pub fn sps(&self, pts: &[V3]) -> Vec<Pos2> {
        pts.iter().map(|&q| self.cam.p(q)).collect()
    }

    /// `mat` shaded at `p` facing `n`.
    pub fn shade(&self, mat: &Mat, n: V3, p: V3) -> C3 {
        self.light.shade(mat, n, self.view(p), p, None, 1.0)
    }

    pub fn shade_g(&self, mat: &Mat, n: V3, p: V3, g: f32) -> C3 {
        self.light.shade(mat, n, self.view(p), p, None, g)
    }

    /// A flat quad or polygon of `col` (projected).
    pub fn quad(&mut self, pts: &[V3], col: C3, op: f32, seam: f32) {
        let part = self.piece;
        if let Some(v) = self.capture.as_mut() {
            v.push(Prim::Flat {
                pts: pts.to_vec(),
                col,
                alpha: op,
                part,
            });
        }
        let sp = self.sps_near(pts);
        if sp.len() >= 3 {
            self.p.poly(sp, Paint::Solid(col), op, seam);
        }
    }

    /// A polygon projected after cutting what comes nearer the eye than 20 cm (floors and
    /// walls that run past the camera).
    pub fn sps_near(&self, pts: &[V3]) -> Vec<Pos2> {
        self.sps(&self.cam.clip_near(pts, 20.0))
    }
}

/// A flat facet, shaded once at its centre (visibility is the caller's).
#[allow(clippy::too_many_arguments)]
pub fn facet(
    c: &mut Ctx,
    pts: &[V3],
    mat: &Mat,
    n: V3,
    g: f32,
    tangent: Option<V3>,
    seam: f32,
    op: f32,
) -> C3 {
    c.keep_facet(pts, n, mat, g);
    let ctr = super::geo::centre(pts);
    let col = c.light.shade(mat, n, c.view(ctr), ctr, tangent, g);
    let sp = c.sps(pts);
    c.p.poly(sp, Paint::Solid(col), op, seam);
    col
}

/// A plain facet: grain 1, no tangent, the default seam.
pub fn facet0(c: &mut Ctx, pts: &[V3], mat: &Mat, n: V3) -> C3 {
    facet(c, pts, mat, n, 1.0, None, 0.6, 1.0)
}

/// A tall facet (bottom edge q0–q1, top edge q2–q3) shaded along its height: the normal
/// leans a little down at the foot and up at the head (a gently rounded surface), and the
/// occlusion darkens both ends where a hoop or a rim meets it. One gradient, five stops.
#[allow(clippy::too_many_arguments)]
pub fn facet_v(
    c: &mut Ctx,
    q: &[V3; 4],
    mat: &Mat,
    n: V3,
    g: f32,
    ao: (f32, f32),
    bend: f32,
    seam: f32,
) {
    facet_v_bent(c, q, mat, n, (n, n), false, g, ao, bend, seam);
}

/// [`facet_v`] of a strip of a curved surface: its normal at the left and the right edge
/// (`ends`), so the strip lights' band runs on smoothly from one strip into the next — one
/// long highlight down a grand's bentside, not a row of bars.
#[allow(clippy::too_many_arguments)]
pub fn facet_v_bent(
    c: &mut Ctx,
    q: &[V3; 4],
    mat: &Mat,
    n: V3,
    ends: (V3, V3),
    // True: the light's band is the caller's to draw, after every strip of the surface
    // ([`lacquer_band`]) — strips that reach into their neighbours would otherwise each
    // cover the edge of the band before them with a dark line.
    defer_band: bool,
    g: f32,
    ao: (f32, f32),
    bend: f32,
    seam: f32,
) {
    c.keep_facet(q, n, mat, g);
    let bot = (q[0] + q[1]) * 0.5;
    let top = (q[2] + q[3]) * 0.5;
    let axis = (top - bot).norm();
    let lacquer = mat.kind == crate::kit::skin::scene::MatKind::Lacquer;
    // A lacquer's streaks are drawn as a band across the face below, never lighting it
    // whole.
    let mut light = c.light;
    light.no_streak = lacquer;
    let mut cols = [(0.0, [0.0; 3]); 5];
    for (st, (f, k)) in cols.iter_mut().zip([
        (0.0, ao.0),
        (0.07, 0.92),
        (0.5, 1.0),
        (0.93, 0.95),
        (1.0, ao.1),
    ]) {
        let p = bot + (top - bot) * f;
        let nn = (n + axis * ((f - 0.5) * 2.0 * bend)).norm();
        let col = light.shade(mat, nn, c.view(p), p, None, g);
        *st = (f, [col[0] * k, col[1] * k, col[2] * k]);
    }
    let (a, b) = (c.sp(bot), c.sp(top));
    let sp = c.sps(q);
    c.p.poly(sp.clone(), lin(a, b, &cols), 1.0, seam);
    if lacquer && !defer_band {
        streak_band(c, q, mat, ends, sp);
    }
}

/// The strip lights' streak across a flat lacquered face (`q`: bottom edge q0–q1, top edge
/// q2–q3): sampled at the face's points from one side to the other, it lands as a crisp
/// band where the reflection lines up with a light — long along the face, sharp across —
/// as a photograph of a lacquered case shows; nothing where it does not.
/// The light's band over the lacquered strip `q` (its normals at its edges `ends`), drawn
/// apart from the strip (see [`facet_v_bent`]).
pub fn lacquer_band(c: &mut Ctx, q: &[V3; 4], mat: &Mat, ends: (V3, V3)) {
    if mat.kind == crate::kit::skin::scene::MatKind::Lacquer {
        let sp = c.sps(q);
        streak_band(c, q, mat, ends, sp);
    }
}

fn streak_band(c: &mut Ctx, q: &[V3; 4], mat: &Mat, ends: (V3, V3), sp: Vec<Pos2>) {
    const N: usize = 25;
    let left = (q[0] + q[3]) * 0.5;
    let right = (q[1] + q[2]) * 0.5;
    let mut stops = [(0.0, [0.0; 3], 0.0); N];
    let mut top = 0.0f32;
    // A white light in black lacquer reads white (its warm tint over black read gold).
    let tint = if super::light::maxc(mat.albedo) < 0.1 {
        let e = c.light.m.env_tint;
        let m = e[0].max(e[1]).max(e[2]).max(1e-3);
        e.map(|v| (v / m) * 0.5 + 0.5)
    } else {
        c.light.m.env_tint
    };
    for (i, st) in stops.iter_mut().enumerate() {
        let t = i as f32 / (N - 1) as f32;
        let p = left.lerp(right, t);
        let n = ends.0.lerp(ends.1, t).norm();
        // A tall light seen in a mirror: bright where it lines up, gone a degree or two
        // off — a crisp long band (the owner's favourite on the bentside), never a veil;
        // with the key's highlight and the soft box, at the clear coat's strength.
        let a = c.light.band(mat, n, c.view(p), p);
        top = top.max(a);
        *st = (t, tint, a);
    }
    if top < 0.02 {
        return;
    }
    let (a, b) = (c.sp(left), c.sp(right));
    c.p.poly(sp, super::display::lin_a(a, b, &stops), 1.0, 0.0);
}

/// How a tube is drawn beyond its ends and radius.
#[derive(Copy, Clone, Debug)]
pub struct TubeOpts {
    pub cap: Cap,
    pub shadow: bool,
    pub minw: f32,
    /// A part on its own (a stand's tube): sorted by its own middle.
    pub own: bool,
    /// Polished chrome's bands across it (a drum stand's tubes, round 40 polish): the
    /// dark room mirrored either side of the bright core, the edges dark where it turns
    /// away — what makes a tube read round, not a flat grey bar.
    pub bands: bool,
}

/// Round caps, a shadow, sorted with its group.
pub const T: TubeOpts = TubeOpts {
    cap: Cap::Round,
    shadow: true,
    minw: 1.0,
    own: false,
    bands: false,
};
/// Round caps, a shadow, its own group.
pub const OWN: TubeOpts = TubeOpts { own: true, ..T };
/// Butt caps, its own group.
pub const OWN_BUTT: TubeOpts = TubeOpts {
    cap: Cap::Butt,
    own: true,
    ..T
};
/// No shadow, its own group.
pub const OWN_NS: TubeOpts = TubeOpts {
    shadow: false,
    own: true,
    ..T
};
/// No shadow, butt caps, its own group.
pub const OWN_NS_BUTT: TubeOpts = TubeOpts {
    cap: Cap::Butt,
    shadow: false,
    own: true,
    ..T
};
/// No shadow, with its group.
pub const NS: TubeOpts = TubeOpts { shadow: false, ..T };
/// No shadow, butt caps, with its group.
pub const NS_BUTT: TubeOpts = TubeOpts {
    cap: Cap::Butt,
    shadow: false,
    ..T
};

/// A tube from `a` to `b` of radius `r`: its width from the camera, its shading across it
/// from the material's response at nine normals (a gradient across the tube on screen).
pub fn tube(c: &mut Ctx, a: V3, b: V3, r: f32, mat: &Mat, o: TubeOpts) {
    if c.mirror && r < 0.5 {
        // A wire leaves no clean reflection, only stray lines (a stand's tube does: the
        // mirror is crisp near the floor now).
        return;
    }
    let part = c.piece;
    if let Some(v) = c.capture.as_mut() {
        v.push(Prim::Tube {
            a,
            b,
            r,
            mat: *mat,
            part,
        });
    }
    let cam = c.cam;
    let mid = (a + b) * 0.5;
    let dep = cam.depth(mid).max(1.0);
    let (pa, pb) = (cam.p(a), cam.p(b));
    let w = o.minw.max(2.0 * r * cam.s / dep);
    if o.own {
        c.p.at(dep, c.piece);
    }
    let d = (b - a).norm();
    let vv = c.view(mid);
    let mut s = d.cross(vv);
    if s.dot(s) < 1e-8 {
        s = if d.y.abs() < 0.9 {
            d.cross(v(0.0, 1.0, 0.0))
        } else {
            d.cross(v(1.0, 0.0, 0.0))
        };
    }
    let s = s.norm();
    let mut f = s.cross(d).norm();
    if f.dot(vv) < 0.0 {
        f = -f;
    }
    let m = cam.p(mid);
    let q = cam.p(mid + s * r);
    let (dx, dy) = (q.x - m.x, q.y - m.y);
    let l = dx.hypot(dy);
    let l = if l > 0.0 { l } else { 1.0 };
    let (px, py) = (dx / l, dy / l);
    let mut stops = [(0.0, [0.0; 3]); 9];
    for (k, st) in stops.iter_mut().enumerate() {
        let th = (-80.0 + 160.0 * k as f32 / 8.0).to_radians();
        let n = (s * th.sin() + f * th.cos()).norm();
        let col = c.light.shade(mat, n, vv, mid, None, 1.0);
        let col = if o.bands {
            // The core bright, a dark band of the room either side, a lighter rim, the
            // silhouette dark.
            let a = th.abs();
            let band = 1.0 - 0.7 * (-((a - 0.8) / 0.3).powi(2)).exp();
            let rim = 0.15 + 0.85 * th.cos().max(0.0).powf(0.6);
            let core = 1.0 + 0.3 * (-(th / 0.25).powi(2)).exp();
            let k = band * rim * core;
            [col[0] * k, col[1] * k, col[2] * k]
        } else {
            col
        };
        *st = (k as f32 / 8.0, col);
    }
    let g = lin(
        Pos2::new(m.x - px * w / 2.0, m.y - py * w / 2.0),
        Pos2::new(m.x + px * w / 2.0, m.y + py * w / 2.0),
        &stops,
    );
    c.p.line(pa, pb, w, g, o.cap, 1.0);
    if o.shadow {
        c.shadow_segs.push((a, b, r));
    }
}

/// A box from corner `o` along edges `u`, `vv`, `w`: the faces that face the eye (of
/// `faces`, when given).
pub fn boxy(c: &mut Ctx, o: V3, u: V3, vv: V3, w: V3, mat: &Mat, faces: Option<&[usize]>) {
    let corner = |a: f32, b: f32, cc: f32| o + u * a + vv * b + w * cc;
    type F = ([(f32, f32, f32); 4], u8);
    let fs: [F; 6] = [
        ([(0., 0., 0.), (1., 0., 0.), (1., 1., 0.), (0., 1., 0.)], 0),
        ([(0., 0., 1.), (1., 0., 1.), (1., 1., 1.), (0., 1., 1.)], 1),
        ([(0., 0., 0.), (1., 0., 0.), (1., 0., 1.), (0., 0., 1.)], 2),
        ([(0., 1., 0.), (1., 1., 0.), (1., 1., 1.), (0., 1., 1.)], 3),
        ([(0., 0., 0.), (0., 1., 0.), (0., 1., 1.), (0., 0., 1.)], 4),
        ([(1., 0., 0.), (1., 1., 0.), (1., 1., 1.), (1., 0., 1.)], 5),
    ];
    for (i, (q, nk)) in fs.iter().enumerate() {
        if faces.is_some_and(|f| !f.contains(&i)) {
            continue;
        }
        let n = match nk {
            0 => -w,
            1 => w,
            2 => -vv,
            3 => vv,
            4 => -u,
            _ => u,
        }
        .norm();
        let pts: [V3; 4] = std::array::from_fn(|k| corner(q[k].0, q[k].1, q[k].2));
        let ctr = super::geo::centre(&pts);
        if c.cam.facing(ctr, n) {
            // Never a flat fill.
            facet_v(c, &pts, mat, n, 1.0, (0.82, 0.92), 0.10, 0.5);
        }
    }
    let mut all = Vec::with_capacity(8);
    for a in [0.0, 1.0] {
        for b in [0.0, 1.0] {
            for cc in [0.0, 1.0] {
                all.push(corner(a, b, cc));
            }
        }
    }
    c.shadow_polys.push((all, 0.6));
}

/// The sides of an outline (x, z) between heights `y0` and `y1`, facing out (or in).
#[allow(clippy::too_many_arguments)]
pub fn extrude(
    c: &mut Ctx,
    outline: &[(f32, f32)],
    y0: f32,
    y1: f32,
    mat: &Mat,
    outward: bool,
    seed: f32,
    inset: f32,
) {
    let n = outline.len();
    for k in 0..n {
        let (x0, z0) = outline[k];
        let (x1, z1) = outline[(k + 1) % n];
        let (mut nx, mut nz) = (z1 - z0, -(x1 - x0));
        if !outward {
            nx = -nx;
            nz = -nz;
        }
        let l = nx.hypot(nz);
        let l = if l > 0.0 { l } else { 1.0 };
        let nrm = v(nx / l, 0.0, nz / l);
        let pm = v(
            (x0 + x1) / 2.0 * inset,
            (y0 + y1) / 2.0,
            (z0 + z1) / 2.0 * inset,
        );
        if !c.cam.facing(pm, nrm) {
            continue;
        }
        let q = [
            v(x0 * inset, y0, z0 * inset),
            v(x1 * inset, y0, z1 * inset),
            v(x1 * inset, y1, z1 * inset),
            v(x0 * inset, y1, z0 * inset),
        ];
        let g = grain(k as f32 * 0.12, seed, mat.figure);
        facet(c, &q, mat, nrm, g, None, 0.6, 1.0);
    }
}

/// Where the key light throws `p` on the floor.
pub fn floor_point(c: &Ctx, p: V3) -> V3 {
    let l = c.light.ldir(p);
    let k = p.y / l.y.max(0.05);
    v(p.x - l.x * k, 0.2, p.z - l.z * k)
}

/// The key's shadows on the floor (an area light's wide penumbra and its core), the
/// hardware's as thin lines, and a dark contact under every foot.
pub fn shadows(c: &mut Ctx) {
    let strength = c.light.m.shadow;
    let cam = c.cam;
    let polys = std::mem::take(&mut c.shadow_polys);
    for (pts, op) in &polys {
        let sp: Vec<Pos2> = pts.iter().map(|&p| cam.p(floor_point(c, p))).collect();
        let core: Vec<Pos2> = pts
            .iter()
            .map(|&q| cam.p(floor_point(c, v(q.x, q.y * 0.6, q.z))))
            .collect();
        c.p.on(Layer::ShadowWide);
        c.p.poly(
            hull(&sp),
            Paint::Solid(ink::SHADE),
            (op * strength * 0.55).min(0.6),
            0.0,
        );
        c.p.on(Layer::Shadow);
        c.p.poly(
            hull(&core),
            Paint::Solid(ink::SHADE),
            (op * strength * 0.7).min(0.8),
            0.0,
        );
    }
    c.p.on(Layer::Shadow);
    let segs = std::mem::take(&mut c.shadow_segs);
    for &(a, b, r) in &segs {
        let (pa, pb) = (cam.p(floor_point(c, a)), cam.p(floor_point(c, b)));
        let w = (2.0 * r * cam.s / cam.depth(a).max(1.0)).max(1.0);
        c.p.line(
            pa,
            pb,
            w,
            Paint::Solid(ink::SHADE),
            Cap::Round,
            0.55 * strength,
        );
    }
    c.p.on(Layer::Contact);
    let contacts = std::mem::take(&mut c.contacts);
    for &(p, rr) in &contacts {
        let sp: Vec<Pos2> = ring(v(p.x, 0.2, p.z), v(0.0, 1.0, 0.0), rr, 16, 0.0)
            .into_iter()
            .map(|q| cam.p(q))
            .collect();
        c.p.poly(sp, Paint::Solid(ink::SHADE), 0.8 * strength, 0.0);
    }
    c.p.on(Layer::Objects);
}
