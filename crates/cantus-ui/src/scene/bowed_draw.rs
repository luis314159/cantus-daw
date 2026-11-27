//! r42:arcus-look — Arcus's instrument in its room, as the engine's projected vectors (the
//! *Drawn* picture, ADR 0126): the room (`rooms`), the floor's mirror, the key's shadow, the
//! cello's or the bass's surfaces (`bowed_mesh`) each cell a facet lit by the engine's one
//! shading function (`light::shade`: the varnish's clear coat, the strip lights' streaks,
//! the flame's figure), sorted far to near, and the picture's falloff — composed as
//! `compose::piano_scene` and `compose::kit_scene` compose theirs.

use super::bowed::{self, Bowed};
use super::bowed_hold::{self as hold, Hold, Holder}; // r42:arcus-upper
use super::bowed_mesh::{mesh_for, Mesh, Role};
use super::compose::{Built, Frame, Stage};
use super::display::{rad, Layer, List, Paint, Part};
use super::geo::{hull, v, Camera, V3};
use super::light::{scale_mat, Light};
use super::prims::{shadows, tube, Ctx, NS};
use super::{rooms, Room};
use crate::kit::skin::scene::hold as hp; // r42:arcus-upper
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

/// r42:arcus-upper — a test camera for design sheets (`shoot_arcus --cam=…`): the eye and
/// what it looks at (world cm) and the half size of the box round it the picture fits.
/// Never set by the app.
static TEST_CAMERA: std::sync::Mutex<Option<(V3, V3, f32)>> = std::sync::Mutex::new(None);

/// r42:arcus-upper — look from `eye` at `at`, the picture fitting a box `half` cm round
/// it (None: the instrument's own camera). For design sheets only.
pub fn set_test_camera(cam: Option<(V3, V3, f32)>) {
    *TEST_CAMERA.lock().unwrap_or_else(|e| e.into_inner()) = cam;
}

fn test_camera() -> Option<(V3, V3, f32)> {
    *TEST_CAMERA.lock().unwrap_or_else(|e| e.into_inner())
}

/// r42:arcus-hand — a realistic figure holding a violin or a viola: the camera's azimuth
/// (degrees) and its height over the room's own (degrees). (r42:arcus-polish2: from the
/// audience at about chest height, as ref-04 is taken — the owner: "like the girl's": 8°
/// under the room's, 2° under level on the stage; the azimuth kept, square to the neck.)
pub const REAL_HELD_VIEW: (f32, f32) = (-20.0, -8.0);

/// r42:arcus-hand — design sheets only: another view for the realistic violinists (the
/// earlier one beside the new, `shoot_arcus --view=4,7`). Never set by the app.
static REAL_VIEW: std::sync::Mutex<Option<(f32, f32)>> = std::sync::Mutex::new(None);

/// r42:arcus-hand — set (or clear) [`REAL_VIEW`]. For design sheets only.
pub fn set_real_view(v: Option<(f32, f32)>) {
    *REAL_VIEW.lock().unwrap_or_else(|e| e.into_inner()) = v;
}

fn real_view() -> (f32, f32) {
    REAL_VIEW
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .unwrap_or(REAL_HELD_VIEW)
}

/// The camera an instrument is seen from: three-quarter from the audience's left, close
/// and a little above the floor (sheet v4: the instrument fills the picture's height).
pub fn camera(b: Bowed, h: Holder, room: Room) -> Camera {
    if let Some((eye, at, _)) = test_camera() {
        // r42:arcus-upper
        let d = eye - at;
        let dist = d.len();
        let el = (d.y / dist).asin().to_degrees();
        let az = d.x.atan2(d.z).to_degrees();
        return Camera::new(at, az, el, dist);
    }
    let d = b.dims();
    let pose = hold::pose(b, h); // r42:arcus-upper
    let mid = pose.at(0.0, d.body * 0.78, 0.0);
    let el = match room {
        Room::Stage => 6.0,
        Room::Hall => 7.0,
        _ => 8.0,
    };
    // r42:arcus-upper — a violin or a viola held: from the hall's middle and a little
    // higher, onto its top and across the bow; closer, as a photographer frames a small
    // instrument. On its stand: as the cello, closer.
    let stand = d.held && h.hold == Hold::Stand;
    let real = matches!(h.hold, Hold::Woman | Hold::Man);
    let (az, el) = if d.held && real {
        // r42:arcus-hand — a realistic violinist from the audience's side, squarer to the
        // neck and lower than the stylised figure's view (the owner's photograph 5 and the
        // professional violinists measured in tools/arcus-figures/left_hand_refs.md: from
        // the hall the left hand shows its back at a slant, the fingers arched over the
        // fingerboard; from over the scroll it was foreshortened into a paw).
        let (a, e) = real_view();
        (a, el + e)
    } else if d.held && !stand {
        (4.0, el + 7.0)
    } else if stand {
        // From higher, onto its top: its outline reads as a violin's, not a cello's.
        (-20.0, el + 16.0)
    } else {
        (-24.0, el)
    };
    let dist = if stand {
        140.0 + 2.2 * d.body
    } else if d.held && h.hold.figure() {
        // Its player in the picture too: farther, so the figure keeps its proportions.
        420.0 + 2.2 * d.body
    } else if d.held {
        250.0 + 2.2 * d.body
    } else {
        300.0 + 2.2 * d.body
    };
    Camera::new(v(mid.x, mid.y, mid.z), az, el, dist) // r42:arcus-upper
}

/// The points the camera fits: the body, the head, the endpin's foot (r42:arcus-upper: a
/// held instrument's margin, a figure's head and shoulders).
pub fn points(b: Bowed, h: Holder) -> Vec<V3> {
    if let Some((_, at, r)) = test_camera() {
        // r42:arcus-upper
        return [-1.0f32, 1.0]
            .iter()
            .flat_map(|&x| [-1.0f32, 1.0].map(move |y| at + v(x * r, y * r * 0.8, 0.0)))
            .collect();
    }
    let d = b.dims();
    let pose = hold::pose(b, h); // r42:arcus-upper
    let o = bowed::outline(b, 2);
    let mut p: Vec<V3> = o
        .iter()
        .flat_map(|&(x, y)| [pose.at(x, y, d.arch), pose.at(x, y, -bowed::rib(d, y))])
        .collect();
    let top = bowed::nut_y(d) + d.pegbox + d.scroll_r * 1.8;
    for x in [-d.head_w * 1.4, d.head_w * 1.4] {
        p.push(pose.at(x, top, -d.scroll_r));
    }
    if !d.held {
        // r42:arcus-upper
        p.push(pose.at(0.0, -d.endpin, -d.rib_lo * 0.5));
    } else if h.hold != Hold::Stand {
        // Held: a margin round it, so the room shows as round the cello. // r42:arcus-upper
        let mid = pose.at(0.0, 0.5 * (d.body + d.neck), 0.0);
        let len = d.body + d.neck + d.pegbox;
        for (dx, dy) in [(-0.47, 0.0), (0.47, 0.0), (0.0, 0.3), (0.0, -0.3)] {
            p.push(mid + v(dx * len, dy * len, 0.0));
        }
    } // r42:arcus-upper
    p.extend(hold::fit(b, h)); // r42:arcus-upper
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
        // r42:arcus-upper — what holds it.
        Role::Stand => hp::STAND,
        Role::Pad => hp::PAD,
        Role::Skin(i) => hp::skin(i),
        Role::SkinShade(i) => hp::skin_shade(i),
        Role::Cloth(i) => hp::cloth(i),
        Role::Trousers => hp::flat(hp::TROUSERS),
        Role::Shoes => hp::flat(hp::SHOES),
        Role::Hair => hp::flat(hp::HAIR),
        Role::Feature => hp::flat(hp::FEATURE),
        Role::Blush => hp::flat(hp::BLUSH),
        Role::Wood(i) => hp::wood(i, false),
        Role::Joint(i) => hp::wood(i, true),
        Role::Velvet => hp::flat(hp::VELVET),
        Role::Wool => hp::flat(hp::WOOL),
        Role::Shirt => hp::flat(hp::SHIRT),
        Role::Tie => hp::flat(hp::TIE),
        Role::HairFair => hp::hair(hp::HAIR_FAIR),
        Role::Eye => hp::eye(hp::EYE_WHITE),
        Role::Iris => hp::eye(hp::IRIS),
        Role::Lash => hp::flat(hp::LASH),
        Role::Nail(i) => hp::nail(i),
        // r42:arcus-upper — never drawn in this picture (`bowed_draw_tris` leaves them out).
        Role::Card(_) | Role::Scalp(_) => hp::flat(hp::LASH),
        Role::Shell(0) => hp::hair(hp::HAIR_FAIR),
        Role::Shell(_) => hp::hair(hp::HAIR_DARK), // r42:arcus-polish4
        Role::Wisp => hp::hair(hp::HAIR_FAIR),
        Role::WispDark => hp::hair(hp::HAIR_DARK), // r42:arcus-polish4
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
/// r42:arcus-polish4 — a plate's ring count when `p` is one of the body's two plates (the
/// top, the back: the rings from the outline in to the middle — the first patch's layout),
/// else none.
fn plate_rows(m: &Mesh, r: Role, p: &super::bowed_mesh::Patch) -> Option<usize> {
    let first = &m.patches.first()?.1;
    (matches!(r, Role::Top | Role::Maple)
        && p.rows > 2
        && p.rows == first.rows
        && p.cols == first.cols)
        .then_some(p.rows)
}

fn smooth01(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

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
    // r42:arcus-polish5 — `CANTUS_TUNE`'s `drawn_skip` (a bit each: 1 the decals, 2 the
    // patches, 4 the figures' triangles, 8 the polygons, 16 the rods) leaves a kind out, to
    // find what drew a stray pixel. Unset: everything.
    let skip = super::figure::tune::tune("drawn_skip", 0.0) as u32;
    // r42:arcus-polish6 — the instrument's faces turned to the camera, on screen (and their
    // boxes): the collar's band that passes under it is kept out of them
    // (`bowed_draw_tris::draw`).
    let mut cover: Vec<Cover> = Vec::new();
    for (d, &a) in dec.iter().zip(&at) {
        if a < f32::MAX && skip & 1 == 0 {
            c.p.at(a - 0.01, Part::Bowed);
            c.p.poly(d.pts.clone(), d.paint.clone(), d.op, 0.0);
        }
    }
    for (role, p) in &m.patches {
        if *role == Role::Purfling || skip & 2 != 0 {
            continue;
        }
        let mat = material(b, *role);
        // Held, the fingerboard runs toward the camera over the top's large cells: its ebony
        // a little nearer, so no cell of the top shows through its side. // r42:arcus-upper
        let held = b.dims().held && *role == Role::Ebony; // r42:arcus-upper
        let bi = bias(*role) + if held { 1.0 } else { 0.0 }; // r42:arcus-upper
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
            let mut ca = c.light.shade(&mat, na, c.view(ea), ea, None, g);
            let mut cb = c.light.shade(&mat, nb, c.view(eb), eb, None, g);
            // r42:arcus-polish4 — on the concert stage a plate's varnish is not one value:
            // darker toward its edges (the varnish pooled by the purfling, the edge's wear
            // dark), lighter over the arching, a slow mottle of wear across it (ref-04/06).
            if let (Some(cc), Some(rows)) = (&c.light.concert, plate_rows(m, *role, p)) {
                let r = cell[0] / p.cols;
                let at = |row: usize, uv: [f32; 2]| {
                    let k = row as f32 / (rows - 1).max(1) as f32;
                    let edge = 1.0 - cc.d_plate * (1.0 - smooth01(k / 0.55));
                    let mottle = (uv[0] * 0.33 + 1.3).sin() * (uv[1] * 0.21 + 0.4).sin()
                        + 0.5 * (uv[0] * 0.71 - uv[1] * 0.37 + 2.1).sin();
                    edge * (1.0 + cc.d_mottle * mottle)
                };
                let (ua, ub) = (p.uv[cell[0]], p.uv[cell[3]]);
                ca = super::light::scale(ca, at(r, ua));
                cb = super::light::scale(cb, at(r + 1, ub));
            }
            c.p.at(cam.depth(ctr) - bi, Part::Bowed);
            let sp = c.sps(&q);
            let (pa, pb) = (cam.p(ea), cam.p(eb));
            let paint = if (pa - pb).length() > 0.5 {
                super::display::lin(pa, pb, &[(0.0, ca), (1.0, cb)])
            } else {
                Paint::Solid(super::light::lerp(ca, cb, 0.5))
            };
            // A figure's fine cells overlap a little more, so no seam grid shows on its
            // dark hair and cloth. // r42:arcus-upper
            let seam = if figure_role(*role) { 1.1 } else { 0.45 }; // r42:arcus-upper
            if *role == Role::Ebony {
                let g = grown(&sp, COVER_GROW);
                cover.push((super::geo::bbox(&g), g));
            }
            c.p.poly(sp, paint, 1.0, seam);
        }
    }
    for (role, poly) in &m.polys {
        if poly.pts.len() < 3 || *role != Role::Ebony {
            continue;
        }
        if cam.facing(super::geo::centre(&poly.pts), poly.n) {
            let sp = grown(&c.sps(&poly.pts), COVER_GROW);
            cover.push((super::geo::bbox(&sp), sp));
        }
    }
    if skip & 4 == 0 {
        super::bowed_draw_tris::draw(c, b, m, &cover); // r42:arcus-upper
    }
    for (role, poly) in &m.polys {
        if poly.pts.len() < 3 || matches!(role, Role::Hole | Role::Cut) || skip & 8 != 0 {
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
        // (r42:arcus-polish2: her loose hairs are the Photographic picture's: drawn as
        // tubes a point wide they were wires.)
        if matches!(role, Role::Wisp | Role::WispDark) || skip & 16 != 0 {
            continue;
        }
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
                // (r42:arcus-polish2: a string one even colour while thinner than a
                // point and a half — its gradient across dotted it.)
                o.flat = 1.5;
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
pub fn bowed_scene(b: Bowed, h: Holder, st: &Stage) -> Built {
    let stage = st.room == Room::Stage;
    let pose = hold::pose(b, h); // r42:arcus-upper
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
    let mut cam = camera(b, h, st.room); // r42:arcus-upper
    let fb = {
        let [l, t, r, bt] = st.frame.inset;
        (
            l,
            t,
            (st.size.x - l - r).max(8.0),
            (st.size.y - t - bt).max(8.0),
        )
    };
    cam.fit(&points(b, h), fb, st.frame.margin, st.frame.anchor); // r42:arcus-upper
                                                                  // r42:arcus-polish3 — a realistic figure on the stage: the concert's own light and hall
                                                                  // (`figure::concert`), the stage dark in either theme.
    let concert =
        (stage && super::bowed_human::who(h.hold).is_some()).then(super::figure::concert::get);
    if let Some(cc) = &concert {
        light = Light::concert(cc, mid, cam.eye());
    }
    let mesh = mesh_for(b, h); // r42:arcus-upper
    let mut list = List::default();
    let lod = if st.size.x.min(st.size.y) * st.ppp < 200.0 {
        super::prims::LOW
    } else {
        super::prims::HIGH
    };
    let info = {
        let mut c = Ctx::new(cam, light, &mut list, lod);
        match &concert {
            Some(cc) => super::hall::concert(&mut c, mid, cam.eye(), cc), // r42:arcus-polish3
            None => rooms::build(&mut c, st.room, v(mid.x, 0.0, mid.z), st.look),
        }
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
    if !d.held || h.hold == Hold::Stand {
        // r42:arcus-upper
        c.contacts.push((mesh.foot, d.rib_lo * 0.42));
    } // r42:arcus-upper
    if d.held && h.hold == Hold::Air {
        wall_shadow(&mut c, &mesh, mid.z - rooms::def(st.room).back); // r42:arcus-upper
    }
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
    let outline: Vec<Pos2> = points(b, h).iter().map(|&q| cam.p(q)).collect(); // r42:arcus-upper
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

/// r42:arcus-upper — a violin or a viola in the air: its soft shadow on the wall behind
/// (`wall_z`), thrown by a light beside the camera to its left, as a
/// photographer's softbox throws it — so the instrument is seen to hang in the room, not
/// float over the picture. Wide and faint, a core a little darker.
fn wall_shadow(c: &mut Ctx, mesh: &Mesh, wall_z: f32) {
    let eye = c.cam.eye();
    let lamp = eye + v(-120.0, 5.0, 0.0);
    let on_wall = |p: V3, k: f32| {
        let dir = p - lamp;
        let t = (wall_z - lamp.z) / dir.z.min(-1e-3);
        let q = lamp + dir * t;
        let m = super::geo::centre(&mesh.shadow);
        let w = lamp + (m - lamp) * ((wall_z - lamp.z) / (m - lamp).z.min(-1e-3));
        w + (q - w) * k
    };
    let cam = c.cam;
    for (k, op, layer) in [
        (1.22, 0.07, Layer::Far),
        (1.12, 0.09, Layer::Far),
        (1.04, 0.11, Layer::Far),
        (0.97, 0.13, Layer::Far),
    ] {
        let sp: Vec<Pos2> = mesh.shadow.iter().map(|&p| cam.p(on_wall(p, k))).collect();
        c.p.on(layer);
        c.p.poly(
            hull(&sp),
            Paint::Solid(ink::SHADE),
            op * c.light.m.shadow,
            0.0,
        );
    }
    c.p.on(Layer::Objects);
}

/// r42:arcus-polish6 — an instrument's face on screen and its box (min x, min y, max x,
/// max y): the chin rest's (its ebony: the cup, its lip, its foot; the tailpiece beside it),
/// which the collar's band passes under.
pub type Cover = ((f32, f32, f32, f32), Vec<Pos2>);

/// r42:arcus-polish6 — how far (points) a chin rest's face is grown for the band's keep-out:
/// its cells' own seams close the hairline gaps between them, and the band showed through
/// them as threads.
const COVER_GROW: f32 = 0.6;

/// `pts` moved out from their middle by `d` (points).
pub fn grown(pts: &[Pos2], d: f32) -> Vec<Pos2> {
    let n = pts.len().max(1) as f32;
    let c = pts.iter().fold(egui::Vec2::ZERO, |a, p| a + p.to_vec2()) / n;
    pts.iter()
        .map(|p| {
            let v = p.to_vec2() - c;
            let l = v.length();
            if l < 1e-4 {
                *p
            } else {
                (c + v * ((l + d) / l)).to_pos2()
            }
        })
        .collect()
}

/// r42:arcus-upper — a surface of the figure that holds the instrument (not the
/// instrument's, nor the room's).
fn figure_role(r: Role) -> bool {
    matches!(
        r,
        Role::Skin(_)
            | Role::SkinShade(_)
            | Role::Cloth(_)
            | Role::Trousers
            | Role::Shoes
            | Role::Hair
            | Role::Wood(_)
            | Role::Joint(_)
            | Role::Velvet
            | Role::Wool
            | Role::Shirt
            | Role::Tie
            | Role::HairFair
            | Role::Eye
            | Role::Iris
            | Role::Lash
            | Role::Nail(_)
            | Role::Shell(_)
            | Role::Wisp
            | Role::WispDark
    )
}
