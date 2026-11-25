//! r42:arcus-upper — the bow arm of the figure that holds an Arcus instrument (a musician's
//! sleeve and hand, the wooden mannequin's), drawn live with the bow and shaded as the
//! baked body is (`lit`): from inside the shoulder, the elbow where a two-bone arm puts it
//! (out to the player's right, at a bowing height), the hand on the frog in a relaxed bow
//! grip — the palm over the stick, the fingers draped over it, the thumb bent under it at
//! the frog's front, the little finger on top.

use super::body::View;
use super::lit::{ball, tube, Lit};
use crate::kit::Brush;
use crate::scene::bowed_hold::{elbow, Arm};
use crate::scene::geo::V3;

/// The bow hand's joints: the wrist, the palm's middle, each finger's knuckle, middle
/// joint, last joint and tip (index … little finger), the thumb's base, knuckle, joint and
/// tip.
#[derive(Copy, Clone, Debug)]
pub struct BowHand {
    pub wrist: V3,
    pub palm: V3,
    pub fingers: [[V3; 4]; 4],
    pub thumb: [V3; 4],
}

/// Where the hand holds the bow at its `frog` (the hair's point there; the bow's way `dir`,
/// frog to tip; `n` the string's normal, the side the stick is on; `held` a violin's or a
/// viola's bow) — the hold the owner's reference and the method books show (sheet v7):
///
/// - the hand **on top of the stick**, its knuckles three centimetres over it and a little
///   toward the player, the knuckles' line along the stick, the hand rolled onto the index
///   (over is up in the room made square to the bow, not the string's normal: a cellist's
///   hand hung under the stick from the camera's side);
/// - the wrist relaxed, behind and over the knuckles, the forearm coming from the player's
///   side;
/// - the **fingers draped over the stick** to its far side, spread a little, the stick
///   across them at the middle phalanx (the index resting on it, toward the tip), the
///   middle finger opposite the thumb, the ring finger over the frog;
/// - the little finger draped as the others on a cello or a bass, **curved on top of the
///   stick** on a violin or a viola, its tip standing on it;
/// - the **thumb bent**, its tip touching the stick at the frog's front corner from the near
///   side underneath, its joint bowed out.
pub fn bow_hand(arm: &Arm, frog: V3, dir: V3, n: V3, held: bool) -> BowHand {
    let up = V3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    let lift = (up - dir * up.dot(dir)).norm();
    let mut over = (lift + n * 0.35).norm();
    over = (over - dir * over.dot(dir)).norm();
    // The far side of the stick (away from the player): the fingers drape over to it.
    let mut far = over.cross(dir).norm();
    if far.dot(arm.f) < 0.0 {
        far = -far;
    }
    // The hand's size: a realistic figure's own (its forearm against the woman's), the
    // mannequin's a little smaller.
    let k = arm.palm * if arm.joints.is_some() { 0.9 } else { 1.0 };
    // The stick's middle at the frog's front corner (`bow`'s stick: 2.1 cm over the hair).
    let s = frog + n * 1.95;
    let at = |u: f32, o: f32, f: f32| s + (dir * u + over * o + far * f) * k;
    // The hand (designed against the reference photographs at 3 × in
    // `tools/arcus-figures/hand_design.py`): its knuckles' middle over the stick and toward
    // the player, the way from the wrist to them sloping out and down to the stick, the
    // knuckles' line along the stick rolled onto the index.
    let kc = at(0.8, 3.6, -1.8);
    let hd = (far * 0.8 - over * 0.45 + dir * 0.2).norm();
    let kx = (dir - over * 0.3).norm();
    let wrist = kc - hd * (8.6 * k);
    let palm = kc - hd * (3.6 * k);
    // Each finger: where the stick crosses it (along the stick), its knuckle on the hand's
    // line, its middle joint over the stick's far top, its tip hanging on the far side.
    let along = [2.4, 0.7, -1.0, -2.6];
    let hang = [1.5, 1.7, 1.5];
    let mut fingers = [[V3::default(); 4]; 4];
    for (j, f) in fingers.iter_mut().enumerate() {
        let knuckle = kc + kx * ((1.5 - j as f32) * 1.9 * k);
        let u = along[j];
        let (mid, tip) = if j == 3 && held {
            // Curved on top: its middle joint arched up, its tip standing on the stick.
            (at(-2.2, 2.8, -0.6), at(-2.9, 0.55, 0.0))
        } else if j == 3 {
            (at(u, 1.3, 0.55), at(u - 0.2, -0.2, 0.95))
        } else {
            (at(u, 1.1, 0.7), at(u + 0.15, -hang[j], 0.75))
        };
        let last = mid.lerp(tip, 0.55);
        *f = [knuckle, mid, last, tip];
    }
    // The thumb: bent, its tip at the frog's front corner under the stick's near side.
    let t3 = at(0.3, -0.35, -0.45);
    let t2 = at(-0.5, -1.0, -1.45);
    let t1 = palm.lerp(t2, 0.45) - far * 0.6 - over * 0.4;
    let t0 = wrist.lerp(kc, 0.35) - far * 1.0 - over * 0.6;
    BowHand {
        wrist,
        palm,
        fingers,
        thumb: [t0, t1, t2, t3],
    }
}

/// The arm from its shoulder to the bow's `frog` (the bow's way `dir`, frog to tip; `n`
/// the string's normal, the side the stick is on).
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub fn right_arm(
    b: &mut Brush,
    v: &View,
    lit: &Lit,
    arm: &Arm,
    frog: V3,
    dir: V3,
    n: V3,
    held: bool,
) {
    let h = bow_hand(arm, frog, dir, n, held);
    let wrist = h.wrist;
    let el = elbow(arm.shoulder, wrist, arm.upper, arm.fore, arm.pole);
    tube(
        b,
        v,
        lit,
        arm.shoulder,
        el,
        arm.r.0 * 1.05,
        arm.r.0,
        arm.sleeve,
    );
    tube(
        b,
        v,
        lit,
        el,
        wrist,
        arm.r.1 * 1.05,
        arm.r.1 * 0.82,
        arm.sleeve,
    );
    hand(b, v, lit, arm, &h);
    // The mannequin's ball joints over the limbs' ends (the wrist's over the hand's).
    if let Some(j) = arm.joints {
        ball(b, v, lit, arm.shoulder, arm.r.0 * 1.25, j);
        ball(b, v, lit, el, arm.r.0 * 1.05, j);
        ball(b, v, lit, wrist, arm.r.1 * 1.15, j);
    }
}

/// The bow hand: the palm from the wrist over the stick, four fingers draped over it (the
/// little finger short, on top), the thumb under it at the frog's front.
fn hand(b: &mut Brush, v: &View, lit: &Lit, arm: &Arm, h: &BowHand) {
    let skin = arm.hand;
    let k = if arm.joints.is_some() { 0.9 } else { 1.0 };
    tube(b, v, lit, h.wrist, h.palm, 2.5 * k, 2.8 * k, skin);
    for (j, f) in h.fingers.iter().enumerate() {
        let r = if j == 3 { 0.62 } else { 0.76 };
        tube(b, v, lit, f[0], f[1], r * 1.08 * k, r * k, skin);
        tube(b, v, lit, f[1], f[2], r * k, r * 0.94 * k, skin);
        tube(b, v, lit, f[2], f[3], r * 0.94 * k, r * 0.88 * k, skin);
    }
    // The thumb, bent, under the stick at the frog's front.
    tube(b, v, lit, h.thumb[1], h.thumb[2], 1.0 * k, 0.92 * k, skin);
    tube(b, v, lit, h.thumb[2], h.thumb[3], 0.92 * k, 0.85 * k, skin);
}
