//! r42:arcus-upper — Arcus's violin and viola for the scene engine (the cello's subject,
//! `scene::bowed`): their measures, their outline, the pose they are held in and the chinrest.
//!
//! **Sources** (measures are facts; no drawing is copied): the violin's body and bouts are the
//! Stradivari "Messiah" (1716) as its published measures give them — 356 mm long, 166 / 109 /
//! 208 mm across the upper, middle and lower bouts, ribs about 30–32 mm, arching about 15 mm;
//! the viola a 16-inch (406 mm) instrument as makers publish its measures — about 192 / 132 /
//! 238 mm across, ribs about 38–39 mm, arching about 17 mm. The set-up (bridge, fingerboard,
//! stop, neck, afterlength, tailpiece) is the trade's standard: a violin's stop 195 mm and neck
//! 130 mm (a 325 mm string; Percival measured 319 – 326 mm on five violins), a bridge 33 mm high and 41 mm across its feet, a fingerboard
//! 270 mm; the viola's in proportion over its string. The strings' gauges are Percival's
//! measured diameters (Glasgow thesis 2013, Table 3.2).
//!
//! **The outlines**: each its own three curves (`VIOLIN_HALF`, `VIOLA_HALF`), drawn from its
//! measures — the bouts' widths and places, the corners, the waist — as the cello's are.
//!
//! **The pose** (the owner's choice, B): a violin or a viola cannot stand on the floor and
//! nobody holds it here, so it floats where a seated player holds it — on the left shoulder at
//! about 1.1 m, its scroll to the player's left and forward and a little up, its top turned
//! up and toward the player's right, as every photograph of a violinist from the hall shows
//! it — with the bow across its strings at the contact (the live picture's, the cello's). Held
//! under the chin its lowest string lies on the player's left (`bowed::string_x` mirrors it),
//! so the bow's frog comes from the player's right. The room is the cello's, without its chair
//! and microphones: at a violin's distance their stands would cross it.

use super::bowed::{Dims, Pose};
use super::bowed_mesh::{Mesh, Rod, Role};
use super::geo::{v, V3};

pub const VIOLIN: Dims = Dims {
    body: 35.6,
    rib_lo: 3.2,
    rib_hi: 3.0,
    arch: 1.55,
    arch_back: 1.45,
    flat_back: false,
    stop: 19.5,
    neck: 13.0,
    bridge_h: 3.3,
    bridge_w: 4.1,
    board: 27.0,
    board_w: (2.35, 4.25),
    spacing: (0.55, 1.13),
    clear: 0.35,
    overstand: 1.3,
    after: 5.5,
    tail_len: 11.0,
    tail_w: (1.6, 4.0),
    pegbox: 5.6,
    scroll_r: 1.9,
    head_w: 2.45,
    endpin: 0.0,
    lean: 0.0,
    roll: 0.0,
    gauge: [0.040, 0.036, 0.031, 0.014],
    f_len: 7.4,
    f_upper: 2.1,
    f_lower: 4.8,
    machines: false,
    held: true,
};

pub const VIOLA: Dims = Dims {
    body: 40.6,
    rib_lo: 3.9,
    rib_hi: 3.7,
    arch: 1.7,
    arch_back: 1.6,
    flat_back: false,
    stop: 22.0,
    neck: 14.8,
    bridge_h: 3.6,
    bridge_w: 4.6,
    board: 30.0,
    board_w: (2.5, 4.7),
    spacing: (0.6, 1.25),
    clear: 0.4,
    overstand: 1.5,
    after: 6.2,
    tail_len: 12.5,
    tail_w: (1.7, 4.4),
    pegbox: 6.4,
    scroll_r: 2.15,
    head_w: 2.75,
    endpin: 0.0,
    lean: 0.0,
    roll: 0.0,
    gauge: [0.045, 0.033, 0.031, 0.0165],
    f_len: 8.4,
    f_upper: 2.4,
    f_lower: 5.5,
    machines: false,
    held: true,
};

/// The violin's half-outline (mm from the lower edge; the viewer's right half), as the cello's
/// (`bowed::CELLO_HALF`): the lower bout to its corner, the C-bout, the upper bout to the neck —
/// drawn to the 'Messiah''s measures: 356 mm long, 104 / 54.5 / 83 mm half-widths at the lower,
/// middle and upper bouts, the corners at 147 and 229 mm, the C-bouts' waist at 190 mm.
pub const VIOLIN_HALF: [&[(f32, f32)]; 3] = [
    &[
        (0.0, 0.0),
        (32.0, 2.0),
        (60.0, 8.0),
        (82.0, 22.0),
        (97.0, 43.0),
        (104.0, 72.0),
        (102.5, 100.0),
        (96.0, 120.0),
        (88.0, 133.0),
        (82.0, 141.0),
        (87.0, 147.0),
    ],
    &[
        (87.0, 147.0),
        (76.0, 151.0),
        (66.0, 159.0),
        (59.0, 170.0),
        (55.5, 181.0),
        (54.5, 190.0),
        (55.5, 199.0),
        (59.0, 209.0),
        (65.0, 218.0),
        (71.0, 224.0),
        (75.0, 229.0),
    ],
    &[
        (75.0, 229.0),
        (70.0, 233.0),
        (70.0, 240.0),
        (74.0, 252.0),
        (80.0, 268.0),
        (83.0, 284.0),
        (81.0, 302.0),
        (75.0, 318.0),
        (63.0, 333.0),
        (46.0, 345.0),
        (25.0, 353.0),
        (0.0, 356.0),
    ],
];

/// The viola's (a 16-inch, 406 mm instrument; 119 / 66 / 96 mm half-widths, the corners at 168
/// and 261 mm, the waist at 217 mm).
pub const VIOLA_HALF: [&[(f32, f32)]; 3] = [
    &[
        (0.0, 0.0),
        (37.0, 2.0),
        (68.0, 9.0),
        (93.0, 25.0),
        (110.0, 49.0),
        (119.0, 82.0),
        (117.0, 114.0),
        (110.0, 137.0),
        (101.0, 152.0),
        (94.0, 161.0),
        (99.0, 168.0),
    ],
    &[
        (99.0, 168.0),
        (87.0, 173.0),
        (77.0, 182.0),
        (69.0, 195.0),
        (66.5, 207.0),
        (66.0, 217.0),
        (67.0, 227.0),
        (71.0, 238.0),
        (78.0, 248.0),
        (85.0, 255.0),
        (89.0, 261.0),
    ],
    &[
        (89.0, 261.0),
        (83.0, 266.0),
        (83.0, 274.0),
        (87.0, 288.0),
        (93.0, 306.0),
        (96.0, 325.0),
        (94.0, 345.0),
        (87.0, 363.0),
        (73.0, 380.0),
        (53.0, 394.0),
        (29.0, 402.0),
        (0.0, 406.0),
    ],
];

/// Where a held violin or viola is: its end button at the player's neck, the body along
/// `TOWARD` (the scroll's way: the player's left, forward, up), its top facing `UP` (turned to the
/// player's right), seated height. The world: x to the viewer's right, y up, z toward the
/// viewer; the player faces the viewer.
pub fn held(d: &Dims) -> Pose {
    held_along(d, TOWARD, UP, V3::default())
}

/// r42:arcus-polish2 — how a realistic violinist holds it (the owner: "ese brazo aún se ve
/// raro"; the coordinator's measure of `refs-violin`): its scroll a little under level, not
/// 18° over it — side views (ref-38, ref-39) put the scroll 0–9° under the chin's end, the
/// front-left ones (ref-24, ref-26, ref-28) under it too —, its top turned ~35° toward the
/// E side as before (inside the photographs' 30–45°), the end button where it was, at the
/// neck, so the chin keeps its rest. Held this way the bow arm hangs from a relaxed
/// shoulder with its elbow a visible corner and the left elbow comes down under the violin
/// (`tests/arcus_arm_pose.rs`). The stylised musician and the mannequin keep `held`.
pub fn held_real(d: &Dims) -> Pose {
    let (toward, up, shift) = held_tuned().unwrap_or((REAL_TOWARD, UP, V3::default()));
    held_along(d, toward, up, shift)
}

/// The realistic violinists' body's way: the first's turned down to 3° under level.
pub const REAL_TOWARD: V3 = V3 {
    x: 0.85,
    y: -0.05,
    z: 0.40,
};

fn held_along(d: &Dims, toward: V3, up: V3, shift: V3) -> Pose {
    let ey = toward.norm();
    let up = up.norm();
    let ez = (up - ey * up.dot(ey)).norm();
    let ex = ey.cross(ez);
    // The end button at the player's neck (r42:arcus-polish2: kept where it was when the
    // body's way turns, so the chin stays on its rest): from the instrument's middle over the
    // room's middle along the first way.
    let half = 0.5 * (d.body + d.neck + d.pegbox);
    let o = v(0.0, HEIGHT, 0.0) - TOWARD_FIRST.norm() * half + shift;
    Pose { o, ex, ey, ez }
}

/// For finding the realistic violinists' hold (`CANTUS_HELD` = the body's way, the top's way and the end button's
/// shift, nine numbers): a tuning aid, read once.
fn held_tuned() -> Option<(V3, V3, V3)> {
    use std::sync::OnceLock;
    static H: OnceLock<Option<(V3, V3, V3)>> = OnceLock::new();
    *H.get_or_init(|| {
        let s = std::env::var("CANTUS_HELD").ok()?;
        let n: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        (n.len() == 9).then(|| {
            (
                v(n[0], n[1], n[2]),
                v(n[3], n[4], n[5]),
                v(n[6], n[7], n[8]),
            )
        })
    })
}

/// The body's way the end button was first placed by (round 42's): the button stays put.
const TOWARD_FIRST: V3 = TOWARD;

/// The body's way, end button to scroll (the player's left is the viewer's right).
pub const TOWARD: V3 = V3 {
    x: 0.85,
    y: 0.30,
    z: 0.40,
};
/// Where the top faces before it is squared to the body's way: up, and turned to the player's
/// right (the viewer's left) and a little toward the hall.
pub const UP: V3 = V3 {
    x: -0.45,
    y: 0.82,
    z: 0.36,
};
/// The instrument's middle over the floor, cm (a seated player's shoulder).
pub const HEIGHT: f32 = 112.0;

/// The chinrest a violin or a viola is never seen without: an ebony cup over the lower bout on
/// the bass side (where the player's jaw rests, left of the tailpiece as the player sees it),
/// clamped to the end block by two nickel barrels — its measures the usual Guarneri model's
/// (a cup about 7 × 5 cm for the violin, its rim 2 cm over the top), in proportion on the viola.
pub fn chinrest(m: &mut Mesh, d: &Dims, pose: &Pose) {
    let k = d.body / 35.6;
    let z = d.arch * 0.55;
    // The cup, its rounded hollow facing the jaw, and its raised outer lip.
    let cup = pose.at(-4.4 * k, 3.3 * k, z + 1.3 * k);
    super::bowed_head::paddle(
        m,
        Role::Ebony,
        cup,
        pose.ex,
        pose.ey,
        3.5 * k,
        2.6 * k,
        0.75 * k,
    );
    let lip = pose.at(-6.2 * k, 3.0 * k, z + 1.9 * k);
    let tilt = (pose.ez * 0.8 - pose.ex * 0.6).norm();
    super::bowed_head::paddle(
        m,
        Role::Ebony,
        lip,
        pose.ey,
        tilt,
        3.0 * k,
        0.75 * k,
        0.6 * k,
    );
    // Its foot over the edge, and the two barrels down the ribs.
    let foot = pose.at(-3.0 * k, 0.9 * k, z + 0.4 * k);
    super::bowed_head::paddle(
        m,
        Role::Ebony,
        foot,
        pose.ex,
        pose.ey,
        2.2 * k,
        0.9 * k,
        0.5 * k,
    );
    for x in [-2.2f32, -6.4] {
        let a = pose.at(x * k, 1.2 * k, d.arch * 0.5 + 0.9 * k);
        let b = pose.at(x * k, -0.15 * k, -d.rib_lo * 0.5);
        m.rods.push((Role::Nickel, Rod { a, b, r: 0.22 * k }));
    }
}
