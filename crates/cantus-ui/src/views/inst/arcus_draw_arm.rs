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

/// Where the hand holds the bow at its `frog` (the bow's way `dir`, frog to tip; `n` the
/// string's normal, the side the stick is on): the palm **over** the stick — over is up
/// in the room, made square to the bow (a cellist's hand sits on top of the stick, its
/// knuckles up, as a violinist's does; the string's normal alone put a cellist's hand
/// beside the stick, hanging under it from the camera's side) — the fingers draped over
/// its far side, the little finger curved on top, the thumb bent under it at the frog.
pub fn bow_hand(arm: &Arm, frog: V3, dir: V3, n: V3) -> BowHand {
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
    let k = if arm.joints.is_some() { 0.9 } else { 1.0 };
    let palm = frog + over * 2.4 + dir * 1.6 - far * 0.6;
    let wrist = palm + over * 2.6 - dir * 3.4 - far * 1.4;
    let mut fingers = [[V3::default(); 4]; 4];
    for (j, f) in fingers.iter_mut().enumerate() {
        let along = (j as f32 - 1.5) * 1.65 * k;
        let knuckle = palm + dir * along + far * (1.6 * k) - over * 0.2;
        let (bend, reach) = if j == 3 { (1.0, 0.6) } else { (1.9, 2.4) };
        let mid = knuckle + far * (bend * k) - over * (1.0 * k);
        let tip = mid - over * (reach * k) - far * (0.2 * k) + dir * (0.2 * k);
        let last = mid.lerp(tip, 0.55) + far * (0.25 * k);
        *f = [knuckle, mid, last, tip];
    }
    let t0 = palm - far * (1.4 * k) - dir * (1.2 * k) + over * 0.3;
    let t1 = frog + dir * 0.7 - over * 0.4 - far * 0.3;
    let base = wrist.lerp(t0, 0.45) - far * (0.6 * k);
    BowHand {
        wrist,
        palm,
        fingers,
        thumb: [base, t0, t0.lerp(t1, 0.5) - over * (0.2 * k), t1],
    }
}

/// The arm from its shoulder to the bow's `frog` (the bow's way `dir`, frog to tip; `n`
/// the string's normal, the side the stick is on).
#[allow(clippy::too_many_arguments)]
pub fn right_arm(b: &mut Brush, v: &View, lit: &Lit, arm: &Arm, frog: V3, dir: V3, n: V3) {
    let h = bow_hand(arm, frog, dir, n);
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
