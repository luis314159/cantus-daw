//! Round 40 (visual): the pianos' shared geometry — what both pictures (Drawn and
//! Photographic) build from, so the parts that must clear each other clear in both: a
//! grand's lid in two leaves (the main lid and the front flap folded back on it, as a real
//! grand's is opened), the music rest standing in the open strip in front of the lid, the
//! prop stick from its cup on the bentside's rim to the lid's underside; the upright's
//! carcass, keyboard, music shelf and top, whose front leaf folds back over the rest.
//! Every length in cm: x right (the treble), y up, z toward the player; the grands' keys'
//! front at z = +15, the case's front at z = 0.

use super::geo::{v, V3};
use super::piano::{outline, slab, GRAND_LID, RIM_HI, UPRIGHT_LID};
use cantus_core::inst::chorda_look::Body;

/// The lid's thickness. `lid_point` is its top face; closed, its underside lies on the rim.
pub const LID_TH: f32 = 2.4;
/// The front flap's hinge (z): the main lid runs from here to the tail; the flap (from here
/// to the case's front) is folded back over it whenever the lid is opened or the desk is
/// up, so the desk's strip — the tuning pins, the dampers, the desk — is open.
pub const LID_FRONT: f32 = -34.0;
/// What must stand between the desk and the lid (or its stick) at any angle.
pub const CLEARANCE: f32 = 3.0;

/// The lid's hinge line (x) on the bass side.
pub fn hinge_x(body: Body) -> f32 {
    -74.0 * if body == Body::Parlour { 0.96 } else { 1.0 }
}

/// A point of the lid's top face at plan (x, z), the lid opened by `ang` (rad).
pub fn lid_point(x: f32, z: f32, ang: f32, body: Body) -> V3 {
    let h = hinge_x(body);
    let dx = x - h;
    v(h + dx * ang.cos(), RIM_HI + LID_TH + dx * ang.sin(), z)
}

/// The lid's outward normal (its top face's) opened by `ang` (rad).
pub fn lid_normal(ang: f32) -> V3 {
    v(-ang.sin(), ang.cos(), 0.0)
}

/// The main lid in plan: the case's outline behind the front flap's hinge.
pub fn lid_outline(body: Body, n: usize) -> Vec<(f32, f32)> {
    slab(&outline(body, n), -10_000.0, LID_FRONT)
}

/// The front flap folded back, in plan where it lies on the main lid: as deep as the case's
/// front strip, a few millimetres inside the lid's edge.
pub fn flap_outline(body: Body, n: usize) -> Vec<(f32, f32)> {
    slab(&outline(body, n), 2.0 * LID_FRONT + 0.6, LID_FRONT)
        .into_iter()
        .map(|(x, z)| (x * 0.992, z))
        .collect()
}

/// A point on the music rest's plane: `x` across, `t` from its ledge (0) to its top (1),
/// `out` toward the player (its board's back face is `out` = 0).
pub fn desk_on(x: f32, t: f32, out: f32) -> V3 {
    v(x, RIM_HI + 30.0 * t, -18.0 - 10.0 * t + out)
}

/// The music rest's half width (the ledge's, a little wider than the board's).
pub const DESK_HALF: f32 = 41.0;

/// Every corner of the music rest — the board with its thickness, the ledge and its lip —
/// for the clearance tests.
pub fn desk_extent() -> Vec<V3> {
    let mut p = Vec::new();
    for x in [-DESK_HALF, DESK_HALF] {
        for t in [0.0, 1.0] {
            for out in [0.0, 1.2, 3.5] {
                p.push(desk_on(x, t, out));
            }
        }
        p.push(v(x, RIM_HI - 2.5, -14.5));
    }
    p
}

/// The bentside's outer x at depth `z` (the treble side of the outline).
pub fn rim_x(body: Body, z: f32) -> f32 {
    let o = outline(body, 14);
    let mut best = 0.0f32;
    for i in 0..o.len() {
        let (a, b) = (o[i], o[(i + 1) % o.len()]);
        if (a.1 - z) * (b.1 - z) <= 0.0 && a.1 != b.1 {
            let t = (z - a.1) / (b.1 - a.1);
            best = best.max(a.0 + (b.0 - a.0) * t);
        }
    }
    best
}

/// Where the prop stick stands (plan depth): a little behind the case's middle.
pub fn prop_z(body: Body) -> f32 {
    let zmin = outline(body, 14)
        .iter()
        .map(|p| p.1)
        .fold(f32::MAX, f32::min);
    zmin * 0.45
}

/// The prop stick, from its cup on the bentside's rim to the lid's underside (`None` while
/// the lid is down on the rim): (foot, head).
pub fn prop(body: Body, lid_deg: f32) -> Option<(V3, V3)> {
    if lid_deg <= 4.0 {
        return None;
    }
    let z = prop_z(body);
    let xr = rim_x(body, z);
    let ang = lid_deg.to_radians();
    let foot = v(xr - 1.4, RIM_HI, z);
    let head = lid_point(xr - 7.0, z, ang, body) - lid_normal(ang) * LID_TH;
    Some((foot, head))
}

/// The grands' lid angles every clearance is held at: closed, every half degree to the
/// full stick.
pub fn lid_angles() -> impl Iterator<Item = f32> {
    (0..=(GRAND_LID * 2.0) as i32).map(|k| k as f32 * 0.5)
}

// ─────────────────────────────────────────────────────────── the upright

/// The upright's carcass: width, depth (its back at z = −UP_D), the top's height.
pub const UP_W: f32 = 152.0;
pub const UP_D: f32 = 58.0;
pub const UP_H: f32 = 128.0;
/// The top's thickness, its overhang at the front, its front leaf's hinge (z).
pub const UP_TOP_TH: f32 = 2.6;
pub const UP_TOP_FRONT: f32 = 2.5;
pub const UP_LEAF_HINGE: f32 = -24.0;
/// The music shelf's top (y) and its front (z); the sheet leans on the upper panel.
pub const UP_SHELF_Y: f32 = 86.0;

/// The upright's front leaf folded by `lid_deg` (0 closed … half of UPRIGHT_LID and more
/// folded back flat over the top — an upright is played with its leaf down or folded, a
/// leaf standing up is only ever seen on its way): its eight corners (the top face first).
pub fn upright_leaf(lid_deg: f32) -> [V3; 8] {
    let phi = (2.0 * lid_deg / UPRIGHT_LID).clamp(0.0, 1.0) * std::f32::consts::PI;
    let (x0, x1) = (-UP_W / 2.0 - 1.0, UP_W / 2.0 + 1.0);
    let hinge = (UP_H + UP_TOP_TH, UP_LEAF_HINGE);
    let len = UP_TOP_FRONT - UP_LEAF_HINGE;
    // Along the leaf (from the hinge to its front edge) and through it (down from its top).
    let along = (phi.sin(), phi.cos()); // (dy, dz)
    let down = (-phi.cos(), phi.sin());
    let at = |x: f32, a: f32, d: f32| {
        v(
            x,
            hinge.0 + along.0 * a + down.0 * d,
            hinge.1 + along.1 * a + down.1 * d,
        )
    };
    [
        at(x0, 0.0, 0.0),
        at(x1, 0.0, 0.0),
        at(x1, len, 0.0),
        at(x0, len, 0.0),
        at(x0, 0.0, UP_TOP_TH),
        at(x1, 0.0, UP_TOP_TH),
        at(x1, len, UP_TOP_TH),
        at(x0, len, UP_TOP_TH),
    ]
}

/// The upright's music shelf, its sheet and the fallboard: their corners (for the
/// clearance test against the top's leaf).
pub fn upright_front_extent() -> Vec<V3> {
    let mut p = Vec::new();
    for x in [-50.0, 50.0] {
        p.push(v(x, UP_SHELF_Y + 1.5, 6.0));
        p.push(v(x, 114.0, 0.8));
        p.push(v(x, 84.0, 2.0));
    }
    p
}

/// The side panels' profile (z, y): the carcass's depth to the floor, the arm out over the
/// leg at the keys' height (its underside a corbel, its front rounded), shallow above.
pub const UP_SIDE: [(f32, f32); 15] = [
    (-UP_D, 0.0),
    (4.5, 0.0),
    (4.5, 58.0),
    (7.0, 61.5),
    (12.0, 63.2),
    (24.0, 64.0),
    (24.0, 78.0),
    (23.4, 79.6),
    (21.8, 80.6),
    (19.5, 81.0),
    (3.0, 81.0),
    (1.5, 82.5),
    (1.5, UP_H),
    (-UP_D + 1.0, UP_H),
    (-UP_D, UP_H - 1.0),
];

/// A key block's profile (z, y): on the keybed beside the keys, rounded at its front top.
pub const UP_BLOCK: [(f32, f32); 7] = [
    (-2.0, 70.6),
    (24.2, 70.6),
    (24.2, 77.6),
    (23.6, 79.1),
    (22.2, 79.9),
    (20.6, 80.2),
    (-2.0, 80.2),
];

/// A front leg's profile (z, y), tapering toward its toe; the toe block's (on its caster).
pub const UP_LEG: [(f32, f32); 4] = [(17.0, 4.5), (21.0, 4.5), (22.0, 63.8), (16.0, 63.8)];
pub const UP_TOE: [(f32, f32); 5] = [
    (5.0, 1.2),
    (23.0, 1.2),
    (24.0, 2.2),
    (24.0, 4.5),
    (5.0, 4.5),
];
/// The legs' x spans (inside the side panels, under the key blocks).
pub const UP_LEG_X: [(f32, f32); 2] = [
    (-UP_W / 2.0 + 5.0, -UP_W / 2.0 + 11.0),
    (UP_W / 2.0 - 11.0, UP_W / 2.0 - 5.0),
];
/// How far toward the player the upright's bench stands beyond a grand's.
pub const UP_BENCH_DZ: f32 = 9.0;
