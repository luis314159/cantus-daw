//! Tympanum's kit (port of `scene/kit.py`): the pieces as data (cm, the kit facing +z), and
//! how each is built — shells of lacquered wood facet by facet, chrome hoops with their
//! flange, lugs and rods, coated heads, lathed bronze cymbals — with every stand, pedal,
//! holder and leg. `tymp-ui` draws through the module and never edits it.

use super::display::{rad, Cap, Paint, Part};
use super::geo::{basis, bbox, hull, rad as dir, ring, tilt_axis, v, V3};
use super::light::{grain, mul, scale};
use super::prims::{
    facet, facet_v_bent, tube, Ctx, NS, NS_BUTT, OWN, OWN_BUTT, OWN_NS, OWN_NS_BUTT,
};
use super::stand;
use crate::kit::skin::scene::{ink, mat, Mat, C3};
use cantus_core::inst::tympanum_kit::{KitDef, PieceKind};
use egui::Pos2;
use std::f32::consts::TAU;

const IN: f32 = 2.54;

/// How a piece is built.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Kick,
    Snare,
    Drum,
    Hats,
    Cymbal,
}

/// One piece placed in the kit.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Piece {
    /// The piece's index (`p{n}`): what a click on it picks.
    pub index: u16,
    pub kind: Kind,
    /// Its centre, its head's axis (up, tilted toward the drummer), radius and depth (cm).
    pub c: V3,
    pub axis: V3,
    pub r: f32,
    pub d: f32,
    /// A cymbal stand's foot.
    pub base: Option<V3>,
    /// A floor tom's legs (their turn, deg).
    pub legs: Option<f32>,
    pub china: bool,
    /// Sorted this far further back (the kick, so the toms sit over it).
    pub depth_bias: f32,
}

#[allow(clippy::too_many_arguments)]
const fn piece(
    index: u16,
    kind: Kind,
    c: V3,
    axis: V3,
    r: f32,
    d: f32,
    base: Option<V3>,
    legs: Option<f32>,
    china: bool,
) -> Piece {
    Piece {
        index,
        kind,
        c,
        axis,
        r,
        d,
        base,
        legs,
        china,
        depth_bias: 0.0,
    }
}

/// The designer's studio kit (`kit-model.json`): a 22″ kick, 14″ snare, 10″ and 12″ toms,
/// 14″ and 16″ floor toms, 14″ hats, 16″ and 18″ crashes, a 20″ ride, a 10″ splash and an
/// 18″ china — in the order of `KIT`.
pub fn studio() -> Vec<Piece> {
    let d = |deg: f32, x: f32, z: f32| tilt_axis(deg, (x, z));
    let mut k = vec![
        piece(
            0,
            Kind::Kick,
            v(0.0, 28.5, 18.0),
            v(0.0, 0.0, 1.0),
            11.0 * IN,
            40.0,
            None,
            None,
            false,
        ),
        piece(
            1,
            Kind::Snare,
            v(34.0, 60.0, -6.0),
            d(9.0, 0.4, -1.0),
            7.0 * IN,
            16.0,
            None,
            None,
            false,
        ),
        piece(
            2,
            Kind::Drum,
            v(13.0, 80.0, 16.0),
            d(14.0, 0.35, -1.0),
            5.0 * IN,
            20.0,
            None,
            None,
            false,
        ),
        piece(
            3,
            Kind::Drum,
            v(-15.0, 82.0, 16.0),
            d(14.0, -0.35, -1.0),
            6.0 * IN,
            22.0,
            None,
            None,
            false,
        ),
        piece(
            4,
            Kind::Drum,
            v(-50.0, 50.0, -4.0),
            d(5.0, 0.3, -1.0),
            7.0 * IN,
            34.0,
            None,
            Some(0.0),
            false,
        ),
        piece(
            5,
            Kind::Drum,
            v(-82.0, 48.0, -30.0),
            d(5.0, 0.5, -1.0),
            8.0 * IN,
            38.0,
            None,
            Some(20.0),
            false,
        ),
        piece(
            6,
            Kind::Hats,
            v(66.0, 92.0, -4.0),
            d(2.0, 0.0, -1.0),
            7.0 * IN,
            0.0,
            None,
            None,
            false,
        ),
        piece(
            7,
            Kind::Cymbal,
            v(44.0, 138.0, 24.0),
            d(20.0, -0.2, -1.0),
            8.0 * IN,
            0.0,
            Some(v(84.0, 0.0, 4.0)),
            None,
            false,
        ),
        piece(
            8,
            Kind::Cymbal,
            v(-36.0, 146.0, 30.0),
            d(20.0, 0.3, -1.0),
            9.0 * IN,
            0.0,
            Some(v(-16.0, 0.0, -46.0)),
            None,
            false,
        ),
        piece(
            9,
            Kind::Cymbal,
            v(-78.0, 110.0, 14.0),
            d(14.0, 0.5, -1.0),
            10.0 * IN,
            0.0,
            Some(v(-124.0, 0.0, -4.0)),
            None,
            false,
        ),
        piece(
            10,
            Kind::Cymbal,
            v(82.0, 128.0, -14.0),
            d(16.0, -0.6, -1.0),
            5.0 * IN,
            0.0,
            Some(v(104.0, 0.0, -44.0)),
            None,
            false,
        ),
        piece(
            11,
            Kind::Cymbal,
            v(-118.0, 140.0, -26.0),
            d(16.0, 0.7, -1.0),
            9.0 * IN,
            0.0,
            Some(v(-150.0, 0.0, -56.0)),
            None,
            true,
        ),
    ];
    k[0].depth_bias = 18.0;
    settle(k)
}

/// How much of a definition's tilt toward the drummer a cymbal shows: a crash or a ride
/// leans a little toward the player (8–12°), and the definition's 14–20° set every
/// cymbal edge on, its underside to the picture's camera (round 40 cp3).
const CYMBAL_TILT: f32 = 0.5;

/// The pieces as a drummer sets them for the picture: each cymbal's lean toward the
/// player eased (its direction kept).
fn settle(mut k: Vec<Piece>) -> Vec<Piece> {
    for p in k.iter_mut().filter(|p| p.kind == Kind::Cymbal) {
        let tilt = p.axis.y.clamp(-1.0, 1.0).acos().to_degrees();
        p.axis = tilt_axis(tilt * CYMBAL_TILT, (p.axis.x, p.axis.z));
    }
    k
}

/// A kit definition's pieces placed for the picture (ADR 0124's `Place`: x right, y toward
/// the audience, z up; the azimuth the direction the head tilts toward, 0 the drummer; the
/// elevation the head's normal above the horizontal, 90 flat). A definition with no pieces
/// draws the studio kit.
pub fn of_def(def: &KitDef) -> Vec<Piece> {
    if def.pieces.is_empty() {
        return studio();
    }
    let k = def
        .pieces
        .iter()
        .map(|p| {
            let c = v(p.place.x, p.place.z, p.place.y);
            let az = p.place.azimuth.to_radians();
            let tilt = (90.0 - p.place.elevation).clamp(0.0, 90.0);
            let axis = tilt_axis(tilt, (az.sin(), -az.cos()));
            let r = p.diameter / 2.0;
            let kind = match p.kind {
                PieceKind::Kick => Kind::Kick,
                PieceKind::Snare => Kind::Snare,
                PieceKind::RackTom | PieceKind::FloorTom => Kind::Drum,
                PieceKind::HiHat => Kind::Hats,
                _ => Kind::Cymbal,
            };
            let mut pc = piece(
                p.index,
                kind,
                c,
                if kind == Kind::Kick {
                    v(0.0, 0.0, 1.0)
                } else {
                    axis
                },
                r,
                p.depth,
                None,
                None,
                p.kind == PieceKind::China,
            );
            if p.kind == PieceKind::FloorTom {
                pc.legs = Some(0.0);
            }
            if kind == Kind::Cymbal {
                // The stand's foot out and back from the cymbal, on the drummer's side.
                let out = v(c.x, 0.0, c.z).norm();
                pc.base = Some(v(c.x + out.x * 30.0, 0.0, c.z - 30.0));
            }
            if kind == Kind::Kick {
                pc.depth_bias = 18.0;
            }
            pc
        })
        .collect();
    settle(k)
}

/// The points a camera frames.
pub fn points(kit: &[Piece]) -> Vec<V3> {
    let mut pts = Vec::with_capacity(kit.len() * 8 + 3);
    for p in kit {
        for dx in [-p.r, p.r] {
            for dy in [-10.0, 10.0] {
                for dz in [-p.r, p.r] {
                    pts.push(p.c + v(dx, dy, dz));
                }
            }
        }
    }
    pts.extend([v(0.0, 0.0, 50.0), v(-90.0, 0.0, -40.0), v(80.0, 0.0, -20.0)]);
    pts
}

// ───────────────────────────────────────────────────────────── hardware

/// A tripod's legs, braces and collars round `base`.
pub fn tripod(c: &mut Ctx, base: V3, collar_h: f32, spread: f32, rot: f32, braces: bool) {
    let (bx, bz) = (base.x, base.z);
    let top = v(bx, collar_h, bz);
    let low = v(bx, collar_h * 0.42, bz);
    for k in 0..3 {
        let a = (rot + 120.0 * k as f32 + 90.0).to_radians();
        let foot = v(bx + spread * a.cos(), 1.0, bz + spread * a.sin());
        let knee = foot - (foot - top).norm() * 4.0;
        tube(c, top, knee, 0.75, &mat::CHROME, OWN);
        tube(c, knee, foot, 1.05, &mat::BLACK, OWN);
        c.contacts.push((foot, 2.6));
        if braces {
            tube(c, low, top + (foot - top) * 0.55, 0.45, &mat::CHROME, OWN);
        }
    }
    tube(
        c,
        v(bx, collar_h - 2.0, bz),
        v(bx, collar_h + 2.5, bz),
        1.75,
        &mat::BLACK,
        OWN_BUTT,
    );
    tube(
        c,
        v(bx, low.y - 1.5, bz),
        v(bx, low.y + 1.5, bz),
        1.45,
        &mat::BLACK,
        OWN_BUTT,
    );
}

/// The cymbal's felt and wing nut on its bell's top (`bell` cm over the plane at `at`).
fn wing_nut(c: &mut Ctx, at: V3, ax: V3, bell: f32) {
    tube(
        c,
        at - ax * 2.5,
        at + ax * (bell + 1.4),
        0.42,
        &mat::CHROME,
        OWN_NS,
    );
    tube(
        c,
        at + ax * (bell + 0.05),
        at + ax * (bell + 0.65),
        1.6,
        &mat::FELT,
        OWN_NS_BUTT,
    );
    let top = at + ax * (bell + 1.4);
    tube(c, top, top + ax * 1.2, 0.9, &mat::BLACK, OWN_NS);
    let side = ax.cross(v(0.0, 0.0, 1.0)).norm();
    let m = top + ax * 0.8;
    tube(c, m - side * 2.2, m + side * 2.2, 0.45, &mat::BLACK, OWN_NS);
}

fn cymbal_stand(c: &mut Ctx, p: &Piece, legs: stand::Legs) {
    let (cc, ax) = (p.c, p.axis);
    // Where the planner stood it (its own foot, or a little aside: `plan_legs`).
    let b = legs.base;
    let up = v(0.0, 1.0, 0.0);
    let top_h = (cc.y - 24.0).max(60.0);
    let top = v(b.x, top_h, b.z);
    // Round 40 polish: a heavy-duty boom stand — double-braced, three sections (25.4, 22
    // and 19 mm) with their clamps, a ratchet at the boom's root and at the cymbal.
    stand::tripod(c, b, 30.0, legs.spread, legs.angle - 90.0);
    let out = stand::toward_eye(c, top, 25.0 + b.x * 0.15);
    let h1 = 33.0 + (top_h - 33.0) * 0.42;
    let h2 = h1 + (top_h - h1) * 0.55;
    stand::chrome(c, v(b.x, 32.6, b.z), v(b.x, h1, b.z), 1.27, OWN);
    stand::clamp(c, v(b.x, h1, b.z), up, 1.27, out);
    stand::chrome(c, v(b.x, h1 + 1.7, b.z), v(b.x, h2, b.z), 1.1, OWN);
    stand::clamp(c, v(b.x, h2, b.z), up, 1.1, out);
    stand::chrome(c, v(b.x, h2 + 1.7, b.z), top, 0.95, OWN);
    let tilt = cc - ax * 5.4;
    let dv = (tilt - top).norm();
    let side = {
        let s = dv.cross(up);
        if s.len() > 0.1 {
            s.norm()
        } else {
            v(1.0, 0.0, 0.0)
        }
    };
    stand::ratchet(c, top + up * 1.0, side, 2.15);
    // The boom through its clamp, the counterweight at its tail.
    let tail = top - dv * 15.0;
    stand::chrome(c, tail, tilt - dv * 1.3, 0.95, OWN);
    stand::black(
        c,
        top + up * 1.0 - dv * 2.4,
        top + up * 1.0 + dv * 2.4,
        1.6,
        OWN_BUTT,
    );
    stand::chrome(c, tail - dv * 0.6, tail + dv * 6.5, 1.95, OWN_BUTT);
    stand::black(c, tail - dv * 1.0, tail - dv * 0.5, 1.7, OWN_NS_BUTT);
    // The cymbal's tilter, its felts and wing nut.
    stand::ratchet(c, tilt, side, 1.55);
    stand::under_felt(c, cc, ax);
    wing_nut(c, cc, ax, prof(0.0, p.china));
}

fn pedal(c: &mut Ctx, heel: V3, board_len: f32, width: f32, toward: (f32, f32)) {
    let (hx, hz) = (heel.x, heel.z);
    let (dx, dz) = toward;
    let toe = v(hx + dx * board_len, 7.0, hz + dz * board_len);
    let side = v(-dz, 0.0, dx).norm();
    let h = v(hx, 1.5, hz);
    let pts = [
        h + side * (width / 2.0),
        h - side * (width / 2.0),
        toe - side * (width / 2.0 * 0.9),
        toe + side * (width / 2.0 * 0.9),
    ];
    let mut n = (pts[1] - pts[0]).cross(pts[3] - pts[0]).norm();
    if n.y < 0.0 {
        n = -n;
    }
    let mid = (pts[0] + pts[2]) * 0.5;
    c.p.at(c.cam.depth(mid), c.piece);
    facet(c, &pts, &mat::BLACK, n, 1.0, None, 0.6, 1.0);
    for k in 0..4 {
        tube(c, pts[k], pts[(k + 1) % 4], 0.5, &mat::CHROME, NS);
    }
    c.shadow_polys.push((pts.to_vec(), 0.5));
}

fn hats_stand(c: &mut Ctx, p: &Piece, legs: stand::Legs) {
    let cc = p.c;
    let (x, z) = (cc.x, cc.z);
    let up = v(0.0, 1.0, 0.0);
    // Round 40 polish: the outer tube (29 mm) and the inner one with its clamp, the bottom
    // cymbal's seat, the clutch over the top one, the spring housings at the foot.
    stand::tripod(c, v(x, 0.0, z), 34.0, legs.spread, legs.angle - 90.0);
    let out = stand::toward_eye(c, v(x, cc.y, z), -30.0);
    let h1 = 36.0 + (cc.y - 14.0 - 36.0) * 0.62;
    stand::chrome(c, v(x, 36.6, z), v(x, h1, z), 1.45, OWN);
    stand::clamp(c, v(x, h1, z), up, 1.45, out);
    stand::chrome(c, v(x, h1 + 1.7, z), v(x, cc.y - 3.3, z), 1.05, OWN);
    // The seat: a chrome cup, its felt, the tilt screw.
    stand::chrome(c, v(x, cc.y - 3.5, z), v(x, cc.y - 2.3, z), 2.2, OWN_BUTT);
    tube(
        c,
        v(x, cc.y - 2.3, z),
        v(x, cc.y - 1.2, z),
        2.1,
        &mat::FELT,
        OWN_NS_BUTT,
    );
    stand::wing_bolt(c, v(x, cc.y - 2.9, z) + out * 2.6, out, up, 0.7);
    // The pull rod up through both bells, the clutch over the top one's bell.
    let bell = 2.4 + prof(0.0, false);
    stand::chrome(
        c,
        v(x, cc.y - 3.0, z),
        v(x, cc.y + bell + 5.2, z),
        0.45,
        OWN,
    );
    tube(
        c,
        v(x, cc.y + bell + 0.05, z),
        v(x, cc.y + bell + 0.75, z),
        1.6,
        &mat::FELT,
        OWN_NS_BUTT,
    );
    stand::black(
        c,
        v(x, cc.y + bell + 0.75, z),
        v(x, cc.y + bell + 3.7, z),
        1.15,
        OWN_BUTT,
    );
    stand::black(
        c,
        v(x, cc.y + bell + 3.7, z),
        v(x, cc.y + bell + 4.4, z),
        1.4,
        OWN_NS_BUTT,
    );
    stand::wing_bolt(c, v(x, cc.y + bell + 2.2, z) + out * 1.15, out, up, 0.75);
    pedal(c, v(x, 0.0, z - 6.0), 30.0, 9.0, (0.0, -1.0));
    stand::chrome(c, v(x, 4.0, z - 7.0), v(x, 33.0, z), 0.35, OWN_NS);
    // The pedal's frame and its two spring housings either side of the tube's foot.
    let side = v(1.0, 0.0, 0.0);
    stand::chrome(
        c,
        v(x - 6.0, 2.2, z - 3.0),
        v(x + 6.0, 2.2, z - 3.0),
        0.7,
        OWN,
    );
    for sg in [-1.0f32, 1.0] {
        let at = v(x, 0.0, z - 3.0) + side * (5.6 * sg);
        stand::chrome(c, at + up * 2.0, at + up * 19.0, 1.05, OWN);
        stand::black(
            c,
            at + up * 18.6,
            at + up * 20.2,
            1.3 * stand::THICK,
            OWN_NS_BUTT,
        );
        stand::black(
            c,
            at + up * 0.4,
            at + up * 2.4,
            1.4 * stand::THICK,
            OWN_BUTT,
        );
        c.contacts.push((at, 1.8));
    }
}

fn snare_stand(c: &mut Ctx, p: &Piece, legs: stand::Legs) {
    let (cc, ax, r, d) = (p.c, p.axis, p.r, p.d);
    let up = v(0.0, 1.0, 0.0);
    let hub = cc - ax * (d / 2.0 + 7.0);
    let b = v(hub.x, 0.0, hub.z);
    // Round 40 polish: two sections with their clamp, the basket's ratchet tilter, its
    // hub and three arms in rubber sleeves.
    stand::tripod(c, b, 24.0, legs.spread, legs.angle - 90.0);
    let out = stand::toward_eye(c, hub, 20.0);
    let t = v(b.x, hub.y - 2.4, b.z);
    let h1 = 27.0 + (t.y - 3.0 - 27.0) * 0.45;
    stand::chrome(c, v(b.x, 26.6, b.z), v(b.x, h1, b.z), 1.27, OWN);
    stand::clamp(c, v(b.x, h1, b.z), up, 1.27, out);
    stand::chrome(c, v(b.x, h1 + 1.7, b.z), t - up * 1.6, 1.0, OWN);
    let side = {
        let s = ax.cross(up);
        if s.len() > 0.05 {
            s.norm()
        } else {
            v(1.0, 0.0, 0.0)
        }
    };
    stand::ratchet(c, t, side, 2.2);
    stand::black(c, t, hub, 0.9, OWN);
    stand::black(c, hub - ax * 1.3, hub + ax * 0.9, 1.7, OWN);
    stand::black(c, hub - ax * 3.4, hub - ax * 1.3, 0.55, OWN_NS);
    stand::black(c, hub - ax * 3.9, hub - ax * 3.3, 1.2, OWN_NS_BUTT);
    for q in ring(cc - ax * (d / 2.0 + 1.5), ax, r * 0.86, 3, 0.6) {
        stand::chrome(c, hub, q, 0.58, OWN);
        stand::black(c, q - ax * 0.4, q + ax * 2.4, 0.9 * stand::THICK, OWN);
    }
}

fn kick_hardware(c: &mut Ctx, p: &Piece) {
    let (cc, r, d) = (p.c, p.r, p.d);
    let (front, back) = (cc.z + d / 2.0, cc.z - d / 2.0);
    for sx in [-1.0f32, 1.0] {
        let a = v(cc.x + sx * (r + 1.2), cc.y - 6.0, front - 9.0);
        let foot = v(cc.x + sx * (r + 13.0), 1.0, front + 3.0);
        let knee = foot - (foot - a).norm() * 3.5;
        tube(c, a, knee, 0.85, &mat::CHROME, OWN);
        tube(c, knee, foot, 1.15, &mat::BLACK, OWN);
        tube(c, a - v(sx * 2.4, 0.0, 0.0), a, 1.5, &mat::CHROME, OWN_BUTT);
        c.contacts.push((foot, 2.5));
    }
    pedal(c, v(cc.x, 0.0, back - 44.0), 26.0, 9.0, (0.0, 1.0));
    for px in [-6.0, 6.0] {
        tube(
            c,
            v(cc.x + px, 2.0, back - 16.0),
            v(cc.x + px, 18.0, back - 12.0),
            0.9,
            &mat::CHROME,
            OWN,
        );
    }
    tube(
        c,
        v(cc.x - 6.0, 18.0, back - 12.0),
        v(cc.x + 6.0, 18.0, back - 12.0),
        0.7,
        &mat::CHROME,
        OWN,
    );
    tube(
        c,
        v(cc.x, 18.0, back - 12.0),
        v(cc.x, 31.0, back - 3.5),
        0.35,
        &mat::CHROME,
        OWN,
    );
    tube(
        c,
        v(cc.x, 31.0, back - 4.8),
        v(cc.x, 31.0, back - 0.6),
        2.4,
        &mat::FELT,
        OWN,
    );
}

fn tom_holder(c: &mut Ctx, kick: &Piece, toms: &[Piece]) {
    let (cc, r) = (kick.c, kick.r);
    let base = v(cc.x, cc.y + r + 0.8, cc.z + 4.0);
    let post_top = v(cc.x, cc.y + r + 12.0, cc.z + 3.0);
    tube(
        c,
        base - v(3.0, 0.0, 0.0),
        base + v(3.0, 0.0, 0.0),
        1.6,
        &mat::BLACK,
        OWN,
    );
    tube(c, base, post_top, 1.25, &mat::CHROME, OWN);
    tube(
        c,
        post_top - v(0.0, 2.0, 0.0),
        post_top + v(0.0, 1.5, 0.0),
        1.9,
        &mat::BLACK,
        OWN_BUTT,
    );
    let out = stand::toward_eye(c, post_top, -20.0);
    stand::wing_bolt(c, post_top + out * 2.0, out, v(0.0, 1.0, 0.0), 0.9);
    for t in toms {
        // The bracket on the shell's quarter toward the post and the front, where its arm
        // shows between the toms (round 40 cp3: on the back, the toms seemed to float).
        let toward =
            (v(post_top.x - t.c.x, 0.0, post_top.z - t.c.z).norm() + v(0.0, 0.0, 0.9)).norm();
        let brk = t.c - t.axis * (t.d / 2.0 - 7.0) + toward * (t.r + 1.4);
        let elbow = v(brk.x, post_top.y + 2.0, brk.z);
        stand::chrome(c, post_top, elbow, 0.95, OWN);
        stand::chrome(c, elbow, brk, 0.95, OWN);
        tube(
            c,
            brk - t.axis * 2.5,
            brk + t.axis * 2.5,
            1.5,
            &mat::BLACK,
            OWN,
        );
        stand::wing_bolt(c, brk + toward * 1.5, toward, t.axis, 0.8);
    }
}

fn floor_legs(c: &mut Ctx, p: &Piece, legs: stand::Legs) {
    let (cc, ax, r, d) = (p.c, p.axis, p.r, p.d);
    let reach = legs.spread;
    for k in 0..3 {
        let a = (legs.angle + 120.0 * k as f32).to_radians();
        let rd = v(a.cos(), 0.0, a.sin());
        let brk = cc + ax * (d * 0.18) + rd * (r + 1.8);
        let foot = v(cc.x + reach * a.cos(), 1.2, cc.z + reach * a.sin());
        let dir = (foot - brk).norm();
        let knee = foot - dir * 4.2;
        // Round 40 polish: the bracket's block and wing bolt, a 16 mm leg, a rubber foot.
        stand::black(
            c,
            brk - ax * 2.8,
            brk + ax * 2.8,
            1.5 * stand::THICK,
            OWN_BUTT,
        );
        stand::chrome(c, brk + ax * 3.6, knee, 0.78, OWN);
        stand::black(c, knee - dir * 0.6, foot, 1.3 * stand::THICK, OWN);
        stand::wing_bolt(c, brk + rd * 1.5, rd, ax, 0.85);
        c.contacts.push((foot, 2.9));
    }
}

// ───────────────────────────────────────────────────────────── drums

/// A hoop's facets: those seen from inside (behind the head) and outside.
struct Hoop {
    inner: Vec<([V3; 4], V3)>,
    outer: Vec<([V3; 4], V3)>,
    edge: Vec<(V3, V3, V3)>,
    mat: Mat,
}

fn hoop(c: &Ctx, plane: V3, ax: V3, r: f32, outward: f32, wood: Option<&Mat>, n: usize) -> Hoop {
    let cam = c.cam;
    let (e1, e2) = basis(ax);
    let o = ax * outward;
    let (h_lo, h_mid, h_hi) = if wood.is_some() {
        (-1.6, 0.35, 1.4)
    } else {
        (-0.9, 0.35, 1.15)
    };
    let mut h = Hoop {
        inner: Vec::new(),
        outer: Vec::new(),
        edge: Vec::new(),
        // A kick's wooden hoops in the shells' own finish (round 40).
        mat: wood.map_or(mat::CHROME, |f| *f),
    };
    for k in 0..n {
        let (a0, a1) = (TAU * k as f32 / n as f32, TAU * (k + 1) as f32 / n as f32);
        let rm = dir(e1, e2, (a0 + a1) / 2.0);
        let (r0, r1) = (dir(e1, e2, a0), dir(e1, e2, a1));
        let pm = plane + rm * (r + 0.45);
        if cam.facing(pm, rm) {
            for (lo, hi, tilt) in [(h_lo, h_mid, -0.25), (h_mid, h_hi, 0.55)] {
                let q = [
                    plane + r0 * (r + 0.45) + o * lo,
                    plane + r1 * (r + 0.45) + o * lo,
                    plane + r1 * (r + 0.45) + o * hi,
                    plane + r0 * (r + 0.45) + o * hi,
                ];
                h.outer.push((q, (rm + o * tilt).norm()));
            }
            h.edge.push((
                plane + r0 * (r + 0.3) + o * h_hi,
                plane + r1 * (r + 0.3) + o * h_hi,
                (o + rm * 0.3).norm(),
            ));
        } else {
            let q = [
                plane + r0 * (r + 0.1),
                plane + r1 * (r + 0.1),
                plane + r1 * (r + 0.1) + o * h_hi,
                plane + r0 * (r + 0.1) + o * h_hi,
            ];
            h.inner.push((q, -rm));
        }
    }
    h
}

fn hoop_inner(c: &mut Ctx, h: &Hoop) {
    for (q, n) in &h.inner {
        facet(c, q, &h.mat, *n, 1.0, None, 0.7, 1.0);
    }
}

fn hoop_outer(c: &mut Ctx, h: &Hoop) {
    for (q, n) in &h.outer {
        facet(c, q, &h.mat, *n, 1.0, None, 0.7, 1.0);
    }
    // The flange's rolled top: a chrome hoop's brightest line (round 40 cp8: on the stage
    // the drawn hoops were dark bands beside the photograph's bright rims).
    let chrome = h.mat.kind == crate::kit::skin::scene::MatKind::Chrome;
    for &(a, b, n) in &h.edge {
        let col = c.shade(&h.mat, n, a);
        let col = if chrome {
            super::light::lerp(scale(col, 1.5), ink::GLINT, 0.35)
        } else {
            col
        };
        let (pa, pb) = (c.sp(a), c.sp(b));
        c.p.line(pa, pb, 1.2, Paint::Solid(col), Cap::Round, 1.0);
    }
}

/// The key's highlight along a chrome tube from `a` to `b` (radius `r`): a fine bright
/// line where the tube's normal halves the key and the eye — a lug or a rod catching the
/// light (round 40 cp3: the drawn hardware read as grey bars).
pub(super) fn glint(c: &mut Ctx, a: V3, b: V3, r: f32) {
    let m = (a + b) * 0.5;
    let axis = (b - a).norm();
    let hv = (c.light.ldir(m) + c.view(m)).norm();
    let nh = hv - axis * hv.dot(axis);
    if nh.len() < 0.3 || nh.norm().dot(c.view(m)) < 0.1 {
        return;
    }
    let k = c.light.spot(m).clamp(0.3, 1.0);
    let off = nh.norm() * (r * 0.92);
    let (pa, pb) = (
        c.sp(a + off + axis * (r * 0.4)),
        c.sp(b + off - axis * (r * 0.4)),
    );
    c.p.line(pa, pb, 0.7, Paint::Solid(ink::GLINT), Cap::Round, 0.85 * k);
}

/// A head: its radial falloff toward the point nearest the light, its mottle, its collar
/// rings (coated); or a black resonant head.
fn head(c: &mut Ctx, plane: V3, ax: V3, r: f32, coated: bool) -> Vec<Pos2> {
    let cam = c.cam;
    let pts = ring(plane, ax, r, 120, 0.0);
    let sp = c.sps(&pts);
    let n = if cam.facing(plane, ax) { ax } else { -ax };
    let l = c.light.ldir(plane);
    let inplane = l - n * l.dot(n);
    let hot_dir = if inplane.dot(inplane) > 1e-6 {
        inplane.norm()
    } else {
        V3::default()
    };
    let hot = c.sp(plane + hot_dir * (r * 0.45));
    let (x0, y0, x1, y1) = bbox(&sp);
    let rr = (x1 - x0).max(y1 - y0) * 0.95;
    // A coated head is off-white, the coating's warm grey (round 40 cp4: a flat white).
    let base = mul(c.shade(&mat::COATED, n, plane), ink::COATING);
    // Under daylight a coated head stays a warm off-white a little darker than the room's
    // walls, its collar and mottle visible (round 40 cp5: it lit to paper white).
    let mx = base[0].max(base[1]).max(base[2]);
    let base = if mx > 0.78 {
        scale(base, 0.78 / mx)
    } else {
        base
    };
    let stops: [(f32, C3, f32); 3] = if coated {
        [
            (0.0, scale(base, 1.10), 1.0),
            (0.55, base, 1.0),
            (1.0, scale(base, 0.80), 1.0),
        ]
    } else {
        let b = ink::BLACK_HEAD;
        [(0.0, b[0], 1.0), (0.45, b[1], 1.0), (1.0, b[2], 1.0)]
    };
    let ry = rr * (y1 - y0) / (x1 - x0).max(1.0) + rr * 0.1;
    c.p.poly(sp.clone(), rad(hot, rr, ry, &stops), 1.0, 0.0);
    if coated {
        // The coating's mottle: small irregular flecks.
        let (e1, e2) = basis(ax);
        let count = (((x1 - x0) * (y1 - y0) / 18.0) as usize).min(140);
        let fleck = scale(base, 0.82);
        for k in 0..count {
            let kf = k as f32;
            let h1 = (kf * 12.9898 + r).sin() * 43758.547;
            let h2 = (kf * 78.233 + r * 3.1).sin() * 24634.635;
            let fa = TAU * (h1 - h1.floor());
            let fr = (h2 - h2.floor()).sqrt() * 0.92;
            let q = c.sp(plane + dir(e1, e2, fa) * (r * fr) + n * 0.05);
            c.p.ellipse(q, 0.7, 0.45, Paint::Solid(fleck), 0.6);
        }
        // The coating's clouds: a few broad, faint blotches.
        for k in 0..5 {
            let kf = k as f32;
            let h1 = (kf * 4.898 + r * 1.7).sin() * 3758.547;
            let h2 = (kf * 7.233 + r * 0.9).sin() * 4634.635;
            let fa = TAU * (h1 - h1.floor());
            let fr = (h2 - h2.floor()).sqrt() * 0.6;
            let q = c.sp(plane + dir(e1, e2, fa) * (r * fr) + n * 0.05);
            let (bx, by) = ((x1 - x0) * 0.16, (y1 - y0) * 0.16);
            let tone = if k % 2 == 0 {
                scale(base, 0.9)
            } else {
                scale(base, 1.06)
            };
            let st: [(f32, C3, f32); 2] = [(0.0, tone, 0.5), (1.0, tone, 0.0)];
            c.p.ellipse(q, bx, by, rad(q, bx, by, &st), 1.0);
        }
        // The collar where the head turns down over the bearing edge: a darker band under
        // the hoop, its crest catching the light just inside, and the hoop's own shadow
        // a crescent on the side toward the light (round 40 cp3: a flat white disc).
        let outer = c.sps(&ring(plane + n * 0.02, ax, r, 96, 0.0));
        let mut band = outer.clone();
        band.extend(
            c.sps(&ring(plane + n * 0.02, ax, r * 0.87, 96, 0.0))
                .into_iter()
                .rev(),
        );
        c.p.poly(band, Paint::Solid(scale(base, 0.72)), 0.6, 0.0);
        let crest = c.sps(&ring(plane + n * 0.03, ax, r * 0.865, 96, 0.0));
        c.p.polyline(crest, 0.9, Paint::Solid(scale(base, 1.12)), 0.4, true);
        let shade_ring = ring(plane + n * 0.03, ax, r * 0.94, 96, 0.0);
        for k in 0..shade_ring.len() {
            let (a, b) = (shade_ring[k], shade_ring[(k + 1) % shade_ring.len()]);
            let rm = ((a + b) * 0.5 - plane).norm();
            let wl = rm.dot(hot_dir).max(0.0);
            let (pa, pb) = (c.sp(a), c.sp(b));
            c.p.line(
                pa,
                pb,
                1.0 + 2.2 * wl,
                Paint::Solid(scale(base, 0.5)),
                Cap::Round,
                0.22 + 0.4 * wl,
            );
        }
        c.p.polyline(outer, 0.9, Paint::Solid(scale(base, 0.45)), 0.6, true);
    }
    sp
}

fn drum(c: &mut Ctx, p: &Piece, fin: &Mat) -> Vec<Pos2> {
    let cam = c.cam;
    let (cc, ax, r, d) = (p.c, p.axis, p.r, p.d);
    let kick = p.kind == Kind::Kick;
    let (top_c, bot_c) = (cc + ax * (d / 2.0), cc - ax * (d / 2.0));
    let (e1, e2) = basis(ax);
    let seed = p.index as f32 * 3.7 + 1.3;
    let n = c.lod.shell;
    for k in 0..n {
        let (a0, a1) = (TAU * k as f32 / n as f32, TAU * (k + 1) as f32 / n as f32);
        let am = (a0 + a1) / 2.0;
        let rm = dir(e1, e2, am);
        if !cam.facing(cc + rm * r, rm) {
            continue;
        }
        let q = [
            bot_c + dir(e1, e2, a0) * r,
            bot_c + dir(e1, e2, a1) * r,
            top_c + dir(e1, e2, a1) * r,
            top_c + dir(e1, e2, a0) * r,
        ];
        // The shell's true normal at each edge: the light's band runs round it smoothly.
        facet_v_bent(
            c,
            &q,
            fin,
            rm,
            (dir(e1, e2, a0), dir(e1, e2, a1)),
            false,
            // A drum's lacquered shell shows its figure softly (round 40 cp8: at the case's
            // strength the shells read as painted stripes beside the photograph's satin).
            grain(am, seed, fin.figure * 0.4),
            (0.55, 0.58),
            0.10,
            0.5,
        );
    }
    if fin.wood {
        let line = scale(fin.dark, 0.55);
        for i in 0..c.lod.grain {
            let a = (i as f32 * 2.399 + seed) % TAU;
            let rm = dir(e1, e2, a);
            if !cam.facing(cc + rm * r, rm) {
                continue;
            }
            let pts: Vec<Pos2> = (0..13)
                .map(|j| {
                    let jf = j as f32;
                    c.sp(bot_c
                        + ax * (d * jf / 12.0)
                        + dir(e1, e2, a + 0.018 * (jf / 12.0 * 9.0 + i as f32).sin()) * (r + 0.02))
                })
                .collect();
            let w = 0.45 + 0.35 * ((i * 7) % 3) as f32 / 2.0;
            let op = 0.07 + 0.06 * ((i * 5) % 3) as f32 / 2.0;
            c.p.polyline(pts, w, Paint::Solid(line), op, false);
        }
    }
    let nl = if matches!(p.kind, Kind::Snare | Kind::Kick) {
        10
    } else if r > 16.0 {
        8
    } else {
        6
    };
    for k in 0..nl {
        let a = TAU * (k as f32 + 0.5) / nl as f32;
        let rm = dir(e1, e2, a);
        let pm = cc + rm * (r + 1.0);
        if rm.dot(c.view(pm)) < 0.12 {
            continue;
        }
        let spans: Vec<(f32, f32)> = match p.kind {
            Kind::Snare => vec![(0.30, 0.44), (-0.44, -0.30)],
            Kind::Kick => vec![(0.32, 0.45), (-0.45, -0.32)],
            _ => {
                let l = (d * 0.34).min(7.0) / d;
                vec![(-l / 2.0, l / 2.0)]
            }
        };
        for &(s0, s1) in &spans {
            let (la, lb) = (
                cc + ax * (d * s0) + rm * (r + 1.1),
                cc + ax * (d * s1) + rm * (r + 1.1),
            );
            tube(c, la, lb, 0.95, &mat::CHROME, NS);
            glint(c, la, lb, 0.95);
        }
        let ends: Vec<f32> = spans.iter().flat_map(|&(a, b)| [a, b]).collect();
        for sgn in [1.0f32, -1.0] {
            let hp = cc + ax * (sgn * (d / 2.0 + 0.6)) + rm * (r + 1.7);
            let end = if sgn > 0.0 {
                ends.iter().copied().fold(f32::MIN, f32::max)
            } else {
                ends.iter().copied().fold(f32::MAX, f32::min)
            };
            tube(
                c,
                hp,
                cc + ax * (d * end) + rm * (r + 1.5),
                0.32,
                &mat::CHROME,
                NS_BUTT,
            );
            tube(
                c,
                hp - ax * (sgn * 0.2),
                hp + ax * (sgn * 0.9),
                0.62,
                &mat::CHROME,
                NS,
            );
        }
    }
    if p.kind == Kind::Drum && r < 17.0 {
        for a in [0.2, 3.3] {
            let rm = dir(e1, e2, a);
            let pm = cc + rm * (r + 0.2);
            if rm.dot(c.view(pm)) > 0.35 {
                let q = c.sp(pm + ax * (d * 0.22));
                c.p.ellipse(q, 1.5, 1.5, Paint::Solid(ink::VENT), 1.0);
                c.p.ellipse(q, 0.6, 0.6, Paint::Solid(ink::VENT_HOLE), 1.0);
                break;
            }
        }
    }
    let top_vis = cam.facing(top_c, ax);
    let (vis_c, hid_c) = if top_vis {
        (top_c, bot_c)
    } else {
        (bot_c, top_c)
    };
    let sgn = if top_vis { 1.0 } else { -1.0 };
    let wood = kick.then_some(fin);
    let hh = hoop(c, hid_c, ax, r, -sgn, wood, c.lod.hoop);
    hoop_outer(c, &hh);
    head(c, vis_c, ax, r, !kick);
    if kick {
        // The port where the photograph's is: up and to the right of the centre as we
        // face the head (round 40 cp8: the two pictures put it in different places).
        let pc = vis_c + v(r * 0.42, r * 0.40, 0.0);
        let port = c.sps(&ring(pc, ax, r * 0.15, 48, 0.0));
        c.p.poly(port, Paint::Solid(ink::PORT), 1.0, 0.0);
        let rim = c.shade(&mat::CHROME, ax, pc);
        let pr = c.sps(&ring(pc + ax * 0.3, ax, r * 0.155, 48, 0.0));
        c.p.polyline(pr, 1.4, Paint::Solid(rim), 1.0, true);
        // The resonant head's printed ring, where the photograph's is (round 40: a black
        // disc alone read as a hole).
        let lr = c.sps(&ring(vis_c + ax * 0.1, ax, r * 0.71, 120, 0.0));
        c.p.polyline(lr, 1.0, Paint::Solid(ink::KICK_RING), 0.85, true);
    }
    let hv = hoop(c, vis_c, ax, r, sgn, wood, c.lod.hoop);
    hoop_inner(c, &hv);
    hoop_outer(c, &hv);
    if kick {
        // The wooden hoop's inlay: a bright line round its middle.
        let at = vis_c + ax * (sgn * -0.1);
        let pts = ring(at, ax, r + 0.95, 120, 0.0);
        let col = c.shade(&mat::CHROME, ax * sgn, at);
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            if (a - at).dot(c.view(a)) > 0.0 && (b - at).dot(c.view(b)) > 0.0 {
                let (pa, pb) = (c.sp(a), c.sp(b));
                c.p.line(pa, pb, 1.0, Paint::Solid(col), Cap::Round, 0.8);
            }
        }
    }
    if kick {
        for k in 0..10 {
            let a = TAU * (k as f32 + 0.5) / 10.0;
            let rm = dir(e1, e2, a);
            let p0 = vis_c + ax * 0.6 + rm * (r + 0.7);
            if rm.dot(c.view(p0)) < 0.05 {
                continue;
            }
            tube(c, p0 - ax * 1.8, p0 + ax * 0.6, 0.8, &mat::CHROME, NS);
        }
    }
    let mut sh = ring(top_c, ax, r, 36, 0.0);
    sh.extend(ring(bot_c, ax, r, 36, 0.0));
    c.shadow_polys.push((sh, 0.55));
    let mut o = c.sps(&ring(top_c, ax, r + 0.6, 72, 0.0));
    o.extend(c.sps(&ring(bot_c, ax, r + 0.6, 72, 0.0)));
    hull(&o)
}

// ───────────────────────────────────────────────────────────── cymbals

const RHO: [f32; 16] = [
    0.0, 0.08, 0.15, 0.20, 0.235, 0.26, 0.32, 0.40, 0.48, 0.56, 0.64, 0.72, 0.80, 0.87, 0.94, 1.0,
];

/// The bell's radius, a fraction of the cymbal's (a 5″ bell on an 18″ crash, 6″ on a
/// ride): round 40 cp3 — a bell of a fifth of the radius, 1 cm proud, read as no bell.
pub const BELL: f32 = 0.26;

/// A cymbal's profile: its height (cm) over the plane at radius fraction `rho` — the bell
/// a dome standing about 2 cm off the bow at a clear shoulder, the bow a taper of 1.6 cm
/// to the edge, a china's upturned edge. Both pictures build from it.
pub fn prof(rho: f32, china: bool) -> f32 {
    let mut h = if rho <= BELL {
        1.6 + 2.1
            * (std::f32::consts::FRAC_PI_2 * rho / BELL)
                .cos()
                .max(0.0)
                .powf(0.55)
    } else {
        1.6 * (1.0 - (rho - BELL) / (1.0 - BELL)).max(0.0).powf(1.3)
    };
    if china && rho > 0.84 {
        h += (rho - 0.84) / 0.16 * 1.8;
    }
    h
}

fn cymbal(c: &mut Ctx, p: &Piece, centre: V3, china: bool) -> Vec<Pos2> {
    let w = c.lod.wedges;
    let cam = c.cam;
    let (cc, ax, r) = (centre, p.axis, p.r);
    let (e1, e2) = basis(ax);
    let up = cam.facing(cc, ax);
    let bz = mat::BRONZE;
    // The edge's thickness: the underside's rim, a band below the top's edge (round 40
    // cp3: a cymbal seen nearly edge on reads by its edge and its bell).
    let rim: Vec<Pos2> = (0..96)
        .map(|k| c.sp(cc + dir(e1, e2, TAU * k as f32 / 96.0) * r + ax * -0.45))
        .collect();
    c.p.poly(rim.clone(), Paint::Solid(scale(bz.albedo, 0.24)), 1.0, 0.0);
    for i in (1..RHO.len()).rev() {
        let (ra, rb) = (RHO[i - 1], RHO[i]);
        let (ha, hb) = (prof(ra, china), prof(rb, china));
        let slope = (hb - ha) / ((rb - ra) * r).max(1e-3);
        let gk = 1.0 + if i % 2 == 1 { 0.05 } else { -0.05 };
        for k in 0..w {
            let (a0, a1) = (TAU * k as f32 / w as f32, TAU * (k + 1) as f32 / w as f32);
            let am = (a0 + a1) / 2.0;
            let radial = dir(e1, e2, am);
            let tangent = dir(e1, e2, am + std::f32::consts::FRAC_PI_2);
            let mut n = (ax - radial * slope).norm();
            if !up {
                n = -n;
            }
            let q = [
                cc + dir(e1, e2, a0) * (ra * r) + ax * ha,
                cc + dir(e1, e2, a1) * (ra * r) + ax * ha,
                cc + dir(e1, e2, a1) * (rb * r) + ax * hb,
                cc + dir(e1, e2, a0) * (rb * r) + ax * hb,
            ];
            facet(c, &q, &bz, n, gk, Some(tangent), 0.5, 1.0);
        }
    }
    // The lathe's grooves over the bow: fine rings, dark and light by turns.
    let nl = c.lod.lathe;
    let ring_at = |c: &Ctx, rr: f32, lift: f32| -> Vec<Pos2> {
        let h = prof(rr, china) + lift;
        (0..72)
            .map(|k| c.sp(cc + dir(e1, e2, TAU * k as f32 / 72.0) * (rr * r) + ax * h))
            .collect()
    };
    for j in 0..nl {
        let rr = BELL + 0.02 + (0.99 - BELL - 0.02) * j as f32 / nl as f32;
        let (col, op) = if j % 2 == 1 {
            (ink::LATHE_DARK, 0.17)
        } else {
            (ink::LATHE_LIGHT, 0.12)
        };
        let pts = ring_at(c, rr, 0.02);
        c.p.polyline(pts, 0.35, Paint::Solid(col), op, true);
    }
    // The bell: its shoulder a dark line where it leaves the bow, a lit ring just above,
    // and the light's soft glow on its dome.
    let sh = ring_at(c, BELL, 0.03);
    c.p.polyline(sh, 0.7, Paint::Solid(ink::LATHE_DARK), 0.32, true);
    let lr = ring_at(c, BELL * 0.86, 0.03);
    c.p.polyline(lr, 0.6, Paint::Solid(ink::LATHE_LIGHT), 0.22, true);
    {
        let l = c.light.ldir(cc);
        let lp = l - ax * l.dot(ax);
        let hot = if lp.dot(lp) > 1e-6 { lp.norm() } else { e1 };
        let hc = c.sp(cc + hot * (BELL * r * 0.35) + ax * prof(BELL * 0.35, china));
        let (x0, y0, x1, y1) = bbox(&ring_at(c, BELL, 0.0));
        let (rx, ry) = (((x1 - x0) * 0.32).max(0.8), ((y1 - y0) * 0.45).max(0.6));
        let glint = ink::BRONZE_GLINT;
        let stops: [(f32, C3, f32); 2] = [(0.0, glint, 0.55), (1.0, glint, 0.0)];
        c.p.ellipse(hc, rx, ry, rad(hc, rx, ry, &stops), 1.0);
    }
    // The hammering: small dents over the bow, each catching the light a little.
    let dents = if r > 15.0 {
        c.lod.dents
    } else {
        c.lod.dents / 2
    };
    let sz = (1.4 * cam.s / cam.depth(cc).max(1.0) * 1.2).max(0.6);
    for k in 0..dents {
        let kf = k as f32;
        let h1 = (kf * 12.9898 + r * 78.233).sin() * 43758.547;
        let h2 = (kf * 39.346 + r * 11.135).sin() * 23421.63;
        let rr = BELL + 0.04 + (0.96 - BELL - 0.04) * (h1 - h1.floor());
        let a = TAU * (h2 - h2.floor());
        let q = c.sp(cc + dir(e1, e2, a) * (rr * r) + ax * (prof(rr, china) + 0.05));
        let (col, op) = if k % 3 != 0 {
            (ink::DENT_LIGHT, 0.08)
        } else {
            (ink::DENT_DARK, 0.06)
        };
        c.p.ellipse(q, sz * 0.55, sz * 0.32, Paint::Solid(col), op);
    }
    // The edge: where it faces us its turned lip catches the light; behind, a dark line.
    let edge3: Vec<V3> = (0..96)
        .map(|k| cc + dir(e1, e2, TAU * k as f32 / 96.0) * r)
        .collect();
    let edge: Vec<Pos2> = edge3.iter().map(|&q| c.sp(q)).collect();
    c.p.polyline(
        edge.clone(),
        0.7,
        Paint::Solid(scale(bz.albedo, 0.42)),
        1.0,
        true,
    );
    for k in 0..edge3.len() {
        let (a, b) = (edge3[k], edge3[(k + 1) % edge3.len()]);
        let m = (a + b) * 0.5;
        let rm = (m - cc).norm();
        if rm.dot(c.view(m)) <= 0.05 {
            continue;
        }
        // The lip in the bronze's own colour, lit where the key reaches it (a white rim
        // read as a silver plate's).
        let lit = c.light.spot(m).clamp(0.35, 1.0);
        let col = scale(bz.albedo, 0.75 + 0.45 * lit);
        c.p.line(
            edge[k],
            edge[(k + 1) % edge.len()],
            0.8,
            Paint::Solid(col),
            Cap::Round,
            0.8,
        );
    }
    let sh: Vec<V3> = (0..24)
        .map(|k| cc + dir(e1, e2, TAU * k as f32 / 24.0) * r)
        .collect();
    c.shadow_polys.push((sh, 0.22));
    let mut all = edge;
    all.extend(rim);
    hull(&all)
}

/// What the kit's build tells the scene: each piece's outline on screen at its depth.
pub type Outlines = Vec<(u16, f32, Vec<Pos2>)>;

/// Each piece's legs as it stands before untangling (`None`: no legs of its own): a
/// tripod's centre, collar, reach and turn, a floor tom's legs.
fn home_legs(p: &Piece) -> Option<stand::Legs> {
    let t = stand::THICK;
    let tripod = |base: V3, collar: f32, spread: f32, rot: f32, upright: f32| stand::Legs {
        base,
        collar: collar - 0.6,
        inner: 1.7 * t,
        spread,
        angle: rot + 90.0,
        upright,
        movable: false,
    };
    match p.kind {
        Kind::Cymbal => {
            let b = p.base.unwrap_or(v(p.c.x, 0.0, p.c.z - 30.0));
            Some(stand::Legs {
                movable: true,
                ..tripod(b, 30.0, 33.0, b.x * 1.3, (p.c.y - 24.0).max(60.0))
            })
        }
        Kind::Hats => Some(tripod(v(p.c.x, 0.0, p.c.z), 34.0, 30.0, 40.0, p.c.y)),
        Kind::Snare => {
            let hub = p.c - p.axis * (p.d / 2.0 + 7.0);
            Some(tripod(v(hub.x, 0.0, hub.z), 24.0, 28.0, 15.0, hub.y))
        }
        Kind::Drum => p.legs.map(|turn| stand::Legs {
            base: v(p.c.x, 0.0, p.c.z),
            collar: p.c.y + p.d * 0.18,
            inner: p.r + 1.8,
            spread: p.r + 9.0,
            angle: turn + 60.0,
            upright: 0.0,
            movable: false,
        }),
        Kind::Kick => None,
    }
}

/// Round 40 polish (the coordinator: the right side's legs piled into a tangle): every
/// stand's legs turned and, where they must, drawn in so that none runs through another's
/// on the floor and as few as can be cross in the picture — the hi-hat first (its pedal
/// clear), then the snare, the floor toms and the cymbals. Planned with the picture's own
/// camera (never the mirror's), so the reflection matches.
pub fn plan_legs(cam: &super::geo::Camera, kit: &[Piece]) -> Vec<(u16, stand::Legs)> {
    let cam = super::geo::Camera {
        mirror: false,
        ..*cam
    };
    let mut fixed = Vec::new();
    for p in kit.iter().filter(|p| p.kind == Kind::Kick) {
        let (cc, r, d) = (p.c, p.r, p.d);
        let (front, back) = (cc.z + d / 2.0, cc.z - d / 2.0);
        for sx in [-1.0f32, 1.0] {
            fixed.push((
                v(cc.x + sx * (r + 1.2), cc.y - 6.0, front - 9.0),
                v(cc.x + sx * (r + 13.0), 1.0, front + 3.0),
            ));
        }
        // The shell's footprint and the pedal.
        let (x0, x1) = (cc.x - r, cc.x + r);
        fixed.push((v(x0, 1.0, back), v(x1, 1.0, back)));
        fixed.push((v(x0, 1.0, front), v(x1, 1.0, front)));
        fixed.push((v(x0, 1.0, back), v(x0, 1.0, front)));
        fixed.push((v(x1, 1.0, back), v(x1, 1.0, front)));
        fixed.push((v(cc.x, 1.0, back - 44.0), v(cc.x, 1.0, back - 18.0)));
    }
    for p in kit.iter().filter(|p| p.kind == Kind::Hats) {
        fixed.push((v(p.c.x, 1.0, p.c.z - 3.0), v(p.c.x, 1.0, p.c.z - 36.0)));
    }
    let rank = |p: &Piece| match p.kind {
        Kind::Hats => 0,
        Kind::Snare => 1,
        Kind::Drum => 2,
        _ => 3,
    };
    let mut order: Vec<&Piece> = kit.iter().filter(|p| home_legs(p).is_some()).collect();
    order.sort_by_key(|p| rank(p));
    let mut legs: Vec<stand::Legs> = order.iter().filter_map(|p| home_legs(p)).collect();
    stand::untangle(&cam, &mut legs, &fixed);
    order.iter().map(|p| p.index).zip(legs).collect()
}

/// The whole kit: hardware first (each tube at its own depth), then each piece as one
/// group at its centre's depth. Returns each piece's outline on screen.
pub fn build(c: &mut Ctx, kit: &[Piece], fin: &Mat) -> Outlines {
    let cam = c.cam;
    let mut out = Vec::with_capacity(kit.len());
    let toms: Vec<Piece> = kit
        .iter()
        .filter(|p| p.kind == Kind::Drum && p.legs.is_none())
        .copied()
        .collect();
    let plan = plan_legs(&cam, kit);
    let legs_of = |p: &Piece| {
        plan.iter()
            .find(|(i, _)| *i == p.index)
            .map(|(_, l)| *l)
            .or_else(|| home_legs(p))
    };
    for p in kit {
        c.piece = Part::Piece(p.index);
        let legs = legs_of(p);
        match (p.kind, legs) {
            (Kind::Cymbal, Some(l)) => cymbal_stand(c, p, l),
            (Kind::Hats, Some(l)) => hats_stand(c, p, l),
            (Kind::Snare, Some(l)) => snare_stand(c, p, l),
            (Kind::Kick, _) => {
                kick_hardware(c, p);
                tom_holder(c, p, &toms);
            }
            (Kind::Drum, Some(l)) => floor_legs(c, p, l),
            _ => {}
        }
        let dep = cam.depth(p.c) + p.depth_bias;
        c.p.at(dep, Part::Piece(p.index));
        let o = match p.kind {
            Kind::Drum | Kind::Snare | Kind::Kick => drum(c, p, fin),
            Kind::Hats => {
                let mut o1 = cymbal(c, p, p.c, false);
                o1.extend(cymbal(c, p, p.c + p.axis * 2.4, false));
                hull(&o1)
            }
            Kind::Cymbal => cymbal(c, p, p.c, p.china),
        };
        c.p.hit(Part::Piece(p.index), o.clone(), dep);
        out.push((p.index, dep, o));
    }
    out
}

/// Where a point on piece `p` lies: 0 the head's centre … 1 its rim, past 1 the hoop (the
/// click's articulation zone).
pub fn zone_at(cam: &super::geo::Camera, p: &Piece, sp: Pos2) -> Option<f32> {
    let face = if cam.facing(p.c, p.axis) {
        p.axis
    } else {
        -p.axis
    };
    let plane = match p.kind {
        Kind::Cymbal | Kind::Hats => p.c,
        _ => p.c + face * (p.d / 2.0),
    };
    let hit = cam.unproject(sp, plane, p.axis)?;
    let off = hit - plane;
    let off = off - p.axis * off.dot(p.axis);
    Some(off.len() / p.r.max(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::geo::Camera;

    /// The picture's camera on the stage (as `compose::kit_scene` aims it, before fitting).
    fn cam() -> Camera {
        Camera::new(v(-4.0, 80.0, 0.0), 16.0, 15.0, 600.0)
    }

    /// Crossings between different stands' legs: on the floor (x–z), and on screen.
    fn crossings(cam: &Camera, legs: &[stand::Legs]) -> (usize, usize) {
        let segs: Vec<(usize, V3, V3)> = legs
            .iter()
            .enumerate()
            .flat_map(|(i, l)| l.legs().into_iter().map(move |(a, b)| (i, a, b)))
            .collect();
        let x = |a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)| {
            let o = |p: (f32, f32), q: (f32, f32), r: (f32, f32)| {
                (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
            };
            o(c, d, a) * o(c, d, b) < 0.0 && o(a, b, c) * o(a, b, d) < 0.0
        };
        let (mut floor, mut screen) = (0, 0);
        for (i, s) in segs.iter().enumerate() {
            for t in &segs[i + 1..] {
                if s.0 == t.0 {
                    continue;
                }
                let f = |p: V3| (p.x, p.z);
                let q = |p: V3| {
                    let r = cam.p(p);
                    (r.x, r.y)
                };
                floor += x(f(s.1), f(s.2), f(t.1), f(t.2)) as usize;
                screen += x(q(s.1), q(s.2), q(t.1), q(t.2)) as usize;
            }
        }
        (floor, screen)
    }

    /// Round 40 polish: the stands' legs untangled — for both modelled kits no leg runs
    /// through another stand's on the floor, and fewer cross in the picture than before.
    #[test]
    fn no_stand_s_legs_run_through_another_s() {
        for id in ["studio", "jazz"] {
            let def = cantus_core::inst::tympanum_kit::modelled_kit(id).expect("a kit");
            let kit = of_def(&def);
            let cam = cam();
            let home: Vec<stand::Legs> = kit.iter().filter_map(home_legs).collect();
            let planned: Vec<stand::Legs> =
                plan_legs(&cam, &kit).into_iter().map(|(_, l)| l).collect();
            let (f0, s0) = crossings(&cam, &home);
            let (f1, s1) = crossings(&cam, &planned);
            println!("{id}: on the floor {f0} → {f1}, in the picture {s0} → {s1}");
            assert_eq!(f1, 0, "{id}: legs through each other on the floor");
            assert!(s1 <= s0, "{id}: {s1} crossings in the picture, {s0} before");
        }
    }
}
