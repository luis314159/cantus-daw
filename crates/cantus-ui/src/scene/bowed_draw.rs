//! r42:arcus-look — Arcus's instrument in its room, as the engine's projected vectors (the
//! *Drawn* picture, ADR 0126): the room (`rooms`), the floor's mirror, the key's shadow, the
//! cello's or the bass's surfaces (`bowed_mesh`) each cell a facet lit by the engine's one
//! shading function (`light::shade`: the varnish's clear coat, the strip lights' streaks,
//! the flame's figure), sorted far to near, and the picture's falloff — composed as
//! `compose::piano_scene` and `compose::kit_scene` compose theirs.

use super::bowed::{self, Bowed, Pose};
use super::bowed_mesh::{mesh, Mesh, Role};
use super::compose::{Built, Frame, Stage};
use super::display::{rad, Layer, List, Paint, Part};
use super::geo::{hull, v, Camera, V3};
use super::light::{scale_mat, Light};
use super::prims::{shadows, tube, Ctx, NS};
use super::{rooms, Room};
use crate::kit::skin::scene::{bowed as pal, ink, mat as pm, Mat, MatKind};
use egui::{Pos2, Vec2};

/// Arcus's play view: the instrument fitted tall in the middle of the picture, room round
/// it for the bow, a band under its endpin for the read-out's caption (at 100 %).
pub const BOWED_FRAME: Frame = Frame {
    inset: [24.0, 6.0, 24.0, 52.0],
    margin: 0.0,
    anchor: (0.5, 0.5),
};

/// [`BOWED_FRAME`] for a picture `size` points large: the caption's band grows with the
/// picture (the window's scale grows both), never under its 100 % height.
pub fn frame_for(size: Vec2) -> Frame {
    let mut f = BOWED_FRAME;
    f.inset[1] = f.inset[1].max(size.y * 0.016);
    f.inset[3] = f.inset[3].max(size.y * 0.122);
    f
}

/// The camera an instrument is seen from: three-quarter from the audience's left, close
/// and a little above the floor (sheet v4: the instrument fills the picture's height).
pub fn camera(b: Bowed, room: Room) -> Camera {
    let d = b.dims();
    let pose = Pose::of(b);
    let mid = pose.at(0.0, d.body * 0.78, 0.0);
    let el = match room {
        Room::Stage => 6.0,
        Room::Hall => 7.0,
        _ => 8.0,
    };
    Camera::new(v(mid.x, mid.y, mid.z), -24.0, el, 300.0 + 2.2 * d.body)
}

/// The points the camera fits: the body, the head, the endpin's foot.
pub fn points(b: Bowed) -> Vec<V3> {
    let d = b.dims();
    let pose = Pose::of(b);
    let o = bowed::outline(b, 2);
    let mut p: Vec<V3> = o
        .iter()
        .flat_map(|&(x, y)| [pose.at(x, y, d.arch), pose.at(x, y, -bowed::rib(d, y))])
        .collect();
    let top = bowed::nut_y(d) + d.pegbox + d.scroll_r * 1.8;
    for x in [-d.head_w * 1.4, d.head_w * 1.4] {
        p.push(pose.at(x, top, -d.scroll_r));
    }
    if !d.stand {
        // r42:arcus-upper
        p.push(pose.at(0.0, -d.endpin, -d.rib_lo * 0.5));
    } // r42:arcus-upper
    p
}

/// The material a role is drawn in.
pub fn material(b: Bowed, r: Role) -> Mat {
    let (top, maple) = match b {
        Bowed::Cello => (pal::CELLO_TOP, pal::CELLO_MAPLE),
        Bowed::Bass => (pal::BASS_TOP, pal::BASS_MAPLE),
        Bowed::Violin => (pal::VIOLIN_TOP, pal::VIOLIN_MAPLE), // r42:arcus-upper
        Bowed::Viola => (pal::VIOLA_TOP, pal::VIOLA_MAPLE),    // r42:arcus-upper
    };
    match r {
        Role::Top => top,
        Role::Maple => maple,
        Role::Inside => scale_mat(&maple, 0.38),
        Role::Ebony | Role::Purfling => pal::EBONY,
        Role::Bridge => pal::BRIDGE,
        Role::BridgeCut => scale_mat(&top, 0.4),
        Role::Nickel => pal::NICKEL,
        Role::Brass => pm::BRASS,
        Role::Rubber => pal::RUBBER,
        Role::Hole => scale_mat(&pal::RUBBER, 1.0),
        Role::Cut => pal::BRIDGE,
        Role::Silk(i) => Mat {
            kind: MatKind::Matte,
            albedo: pal::SILK[(i as usize).min(3)],
            ..pal::RUBBER
        },
        Role::Steel => pal::STEEL,
        Role::Wound => pal::TUNGSTEN,
        Role::Chair => crate::kit::skin::scene::FINISH_WALNUT,
        Role::Seat => pal::SEAT,
    }
}

/// How much nearer than its own middle an op is sorted (cm): the strings over what they
/// cross.
fn bias(r: Role) -> f32 {
    match r {
        Role::BridgeCut => 0.05,
        Role::Steel | Role::Wound | Role::Silk(_) => 1.2,
        _ => 0.0,
    }
}

/// The wood's figure at a cell (its grain coordinates, cm): the spruce top's slow
/// variation (its fine grain is drawn as lines over it), the maple's flame across its
/// length.
fn figure(r: Role, m: &Mat, uv: [f32; 2]) -> f32 {
    match r {
        Role::Top => 1.0 + m.figure * 0.35 * (uv[1] * 0.21 + 0.6 * (uv[0] * 0.05).sin()).sin(),
        Role::Maple | Role::Inside => {
            let flame = (uv[0] * 1.9 + 0.9 * (uv[1] * 0.21).sin()).sin();
            1.0 + m.figure * 0.5 * (0.8 * flame + 0.3 * (uv[1] * 0.37 + 2.0).sin())
        }
        _ => 1.0,
    }
}

/// What lies on the top (its purfling, the f-holes' cuts and holes, the spruce's grain):
/// each piece's screen outline, its box, its paint and opacity — sorted just nearer than
/// the nearest of the top's cells under it, so no cell of the top covers it and whatever
/// stands over the top still does.
struct Decal {
    pts: Vec<Pos2>,
    bx: (f32, f32, f32, f32),
    paint: Paint,
    op: f32,
    /// Drawn just after the decal before it, whatever it overlaps (an f-hole's dark inside
    /// over its pale cut, never under it).
    after: bool,
}

fn decals(c: &Ctx, b: Bowed, m: &Mesh) -> Vec<Decal> {
    let cam = c.cam;
    let mut out = Vec::new();
    let mut push = |pts: Vec<Pos2>, paint: Paint, op: f32, after: bool| {
        if pts.len() >= 3 {
            let bx = super::geo::bbox(&pts);
            out.push(Decal {
                pts,
                bx,
                paint,
                op,
                after,
            });
        }
    };
    for (role, p) in &m.patches {
        if *role != Role::Purfling {
            continue;
        }
        let mat = material(b, *role);
        for cell in p.cells() {
            let q: Vec<V3> = cell.iter().map(|&i| p.pts[i]).collect();
            let ctr = super::geo::centre(&q);
            let n = p.nrm[cell[0]];
            if !cam.facing(ctr, n) {
                continue;
            }
            let col = c.light.shade(&mat, n, c.view(ctr), ctr, None, 1.0);
            push(c.sps(&q), Paint::Solid(col), 1.0, false);
        }
    }
    for (role, poly) in &m.polys {
        if !matches!(role, Role::Hole | Role::Cut) {
            continue;
        }
        let ctr = super::geo::centre(&poly.pts);
        if !cam.facing(ctr, poly.n) {
            continue;
        }
        let col = match role {
            Role::Hole => pal::HOLE,
            _ => c.light.lit(pal::CUT, poly.n, ctr),
        };
        push(
            c.sps(&poly.pts),
            Paint::Solid(col),
            1.0,
            *role == Role::Hole,
        );
    }
    // The spruce's grain: fine dark lines along the top, about a pixel wide.
    let top = material(b, Role::Top);
    let ez = m.normal;
    for (k, line) in m.grain.iter().enumerate() {
        if line.len() < 2 {
            continue;
        }
        let mid = line[line.len() / 2];
        let w = (0.07 * cam.s / cam.depth(mid).max(1.0)).max(0.4);
        let sp = c.sps(line);
        let side = |i: usize| {
            let (a, e) = (sp[i.saturating_sub(1)], sp[(i + 1).min(sp.len() - 1)]);
            let d = (e - a).normalized();
            egui::vec2(-d.y, d.x) * (w * 0.5)
        };
        let col = c.light.shade(&top, ez, c.view(mid), mid, None, 0.6);
        let op = 0.20 + 0.16 * ((k as f32 * 2.17).sin() * 0.5 + 0.5);
        let paint = Paint::Solid(super::light::scale(col, 0.6));
        let mut ribbon: Vec<Pos2> = (0..sp.len()).map(|q| sp[q] + side(q)).collect();
        ribbon.extend((0..sp.len()).rev().map(|q| sp[q] - side(q)));
        push(ribbon, paint, op, false);
    }
    out
}

fn overlaps(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3
}

/// Draw `mesh` through `c` (its camera, or the floor mirror's).
pub fn draw(c: &mut Ctx, b: Bowed, m: &Mesh) {
    c.piece = Part::Bowed;
    let cam = c.cam;
    let dec = if c.mirror {
        Vec::new()
    } else {
        decals(c, b, m)
    };
    // Each decal sorted just nearer than the nearest of the top's cells it overlaps: after
    // every cell under it, before anything standing over the top.
    let mut at = vec![f32::MAX; dec.len()];
    if let Some((_, top)) = m.patches.first() {
        for cell in top.cells() {
            let q = cell.map(|i| top.pts[i]);
            let sp: Vec<Pos2> = q.iter().map(|&w| cam.p(w)).collect();
            let bx = super::geo::bbox(&sp);
            let dep = cam.depth(super::geo::centre(&q));
            for (d, a) in dec.iter().zip(at.iter_mut()) {
                if overlaps(bx, d.bx) {
                    *a = a.min(dep);
                }
            }
        }
    }
    for i in 1..dec.len() {
        if dec[i].after {
            at[i] = at[i].min(at[i - 1]) - 0.002;
        }
    }
    for (d, &a) in dec.iter().zip(&at) {
        if a < f32::MAX {
            c.p.at(a - 0.01, Part::Bowed);
            c.p.poly(d.pts.clone(), d.paint.clone(), d.op, 0.0);
        }
    }
    for (role, p) in &m.patches {
        if *role == Role::Purfling {
            continue;
        }
        let mat = material(b, *role);
        let bi = bias(*role);
        for cell in p.cells() {
            let mut q: Vec<V3> = Vec::with_capacity(4);
            let mut nq: Vec<V3> = Vec::with_capacity(4);
            for &i in &cell {
                if q.last().is_none_or(|l: &V3| (*l - p.pts[i]).len() > 1e-4) {
                    q.push(p.pts[i]);
                    nq.push(p.nrm[i]);
                }
            }
            if q.len() > 3 && (q[0] - q[q.len() - 1]).len() < 1e-4 {
                q.pop();
                nq.pop();
            }
            if q.len() < 3 {
                continue;
            }
            let n = nq.iter().fold(V3::default(), |a, &x| a + x).norm();
            let ctr = super::geo::centre(&q);
            if !cam.facing(ctr, n) {
                continue;
            }
            let uv = cell.iter().fold([0.0f32; 2], |a, &i| {
                [a[0] + p.uv[i][0] * 0.25, a[1] + p.uv[i][1] * 0.25]
            });
            let g = figure(*role, &mat, uv);
            c.keep_facet(&q, n, &mat, g);
            // Shaded at the middles of its two row edges, a gradient between: the arching
            // and the scroll's turns read round, not faceted.
            let (ea, eb) = (
                (p.pts[cell[0]] + p.pts[cell[1]]) * 0.5,
                (p.pts[cell[3]] + p.pts[cell[2]]) * 0.5,
            );
            let na = (p.nrm[cell[0]] + p.nrm[cell[1]]).norm();
            let nb = (p.nrm[cell[3]] + p.nrm[cell[2]]).norm();
            let ca = c.light.shade(&mat, na, c.view(ea), ea, None, g);
            let cb = c.light.shade(&mat, nb, c.view(eb), eb, None, g);
            c.p.at(cam.depth(ctr) - bi, Part::Bowed);
            let sp = c.sps(&q);
            let (pa, pb) = (cam.p(ea), cam.p(eb));
            let paint = if (pa - pb).length() > 0.5 {
                super::display::lin(pa, pb, &[(0.0, ca), (1.0, cb)])
            } else {
                Paint::Solid(super::light::lerp(ca, cb, 0.5))
            };
            c.p.poly(sp, paint, 1.0, 0.45);
        }
    }
    for (role, poly) in &m.polys {
        if poly.pts.len() < 3 || matches!(role, Role::Hole | Role::Cut) {
            continue;
        }
        let ctr = super::geo::centre(&poly.pts);
        if !cam.facing(ctr, poly.n) {
            continue;
        }
        let mat = material(b, *role);
        c.keep_facet(&poly.pts, poly.n, &mat, 1.0);
        let col = c.light.shade(&mat, poly.n, c.view(ctr), ctr, None, 1.0);
        c.p.at(cam.depth(ctr) - bias(*role), Part::Bowed);
        let sp = c.sps(&poly.pts);
        c.p.poly(sp, Paint::Solid(col), 1.0, 0.3);
    }
    for (role, rod) in &m.rods {
        let mat = material(b, *role);
        let len = (rod.b - rod.a).len();
        // Long rods (the strings, the endpin) in pieces, each sorted by its own middle.
        let n = ((len / 4.0).ceil() as usize).clamp(1, 40);
        for k in 0..n {
            let (a, e) = (
                rod.a.lerp(rod.b, k as f32 / n as f32),
                rod.a.lerp(rod.b, (k + 1) as f32 / n as f32),
            );
            c.p.at(cam.depth((a + e) * 0.5) - bias(*role), Part::Bowed);
            let mut o = NS;
            o.minw = if matches!(role, Role::Steel | Role::Wound | Role::Silk(_)) {
                0.55
            } else {
                1.0
            };
            tube(c, a, e, rod.r, &mat, o);
        }
    }
}

/// The picture's falloff (as the other scenes'): a radial darkening toward the corners.
fn falloff(list: &mut List, size: Vec2, light: &Light) {
    let (w, h) = (size.x, size.y);
    list.on(Layer::Front);
    let pts = vec![
        Pos2::new(0.0, 0.0),
        Pos2::new(w, 0.0),
        Pos2::new(w, h),
        Pos2::new(0.0, h),
    ];
    let g = rad(
        Pos2::new(w / 2.0, h * 0.55),
        w * 0.72,
        h * 0.95,
        &[(0.55, ink::SHADE, 0.0), (1.0, ink::SHADE, light.m.falloff)],
    );
    list.poly(pts, g, 1.0, 0.0);
}

/// Arcus's cello or double bass standing in its room.
pub fn bowed_scene(b: Bowed, st: &Stage) -> Built {
    let stage = st.room == Room::Stage;
    let pose = Pose::of(b);
    let d = b.dims();
    let mid = pose.at(0.0, d.body * 0.5, 0.0);
    let mut light = Light::new(
        st.evening,
        stage,
        v(-90.0, 380.0, 160.0),
        v(mid.x, 0.0, mid.z),
    );
    if stage {
        light.m.spot_r = 120.0 + d.body * 0.6;
    }
    light.floor = rooms::floor_paint(st.look.floor.unwrap_or(rooms::def(st.room).floor)).a;
    let mut cam = camera(b, st.room);
    let fb = {
        let [l, t, r, bt] = st.frame.inset;
        (
            l,
            t,
            (st.size.x - l - r).max(8.0),
            (st.size.y - t - bt).max(8.0),
        )
    };
    cam.fit(&points(b), fb, st.frame.margin, st.frame.anchor);
    let mesh = mesh(b);
    let mut list = List::default();
    let lod = if st.size.x.min(st.size.y) * st.ppp < 200.0 {
        super::prims::LOW
    } else {
        super::prims::HIGH
    };
    let info = {
        let mut c = Ctx::new(cam, light, &mut list, lod);
        rooms::build(&mut c, st.room, v(mid.x, 0.0, mid.z), st.look)
    };
    let y0 = cam.p(v(0.0, 0.0, mid.z)).y;
    list.mirror_fade = Some((y0 - 30.0, y0 + st.size.y * 0.45));
    if info.gloss > 0.15 {
        list.force = Some(Layer::Mirror);
        let mut m = Ctx::new(cam.mirrored(), light, &mut list, lod);
        draw(&mut m, b, &mesh);
        list.force = None;
    }
    let mut c = Ctx::new(cam, light, &mut list, lod);
    c.capture = st.capture.then(Vec::new);
    draw(&mut c, b, &mesh);
    // The two microphones, by the engine's own gear.
    let _ = super::gear::kit_mics(&mut c, &super::bowed_props::mics(b));
    c.piece = Part::Bowed;
    c.shadow_polys.push((mesh.shadow.clone(), 0.55));
    for s in &mesh.shadows {
        c.shadow_polys.push((s.clone(), 0.5));
    }
    c.contacts.push((mesh.foot, d.rib_lo * 0.42));
    shadows(&mut c);
    let prims = c.capture.take();
    drop(c);
    let model = prims.map(|prims| super::backend::Model {
        prims,
        cam,
        light,
        room: st.room,
        look: st.look,
        gloss: info.gloss,
        frame: st.frame,
        size: st.size,
        subject: None,
        evening: st.evening,
    });
    falloff(&mut list, st.size, &light);
    let outline: Vec<Pos2> = points(b).iter().map(|&q| cam.p(q)).collect();
    let dep = cam.depth(mid);
    list.hit(Part::Bowed, hull(&outline), dep);
    let outlines = list
        .hits
        .iter()
        .map(|h| (h.part, h.depth, h.outline.clone()))
        .collect();
    Built {
        list,
        outlines,
        mics: Vec::new(),
        cam,
        gloss: info.gloss,
        evening: st.evening,
        size: st.size,
        pieces: Vec::new(),
        model,
    }
}
