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

/// The arm from its shoulder to the bow's `frog` (the bow's way `dir`, frog to tip; `n`
/// the string's normal, the side the stick is on).
#[allow(clippy::too_many_arguments)]
pub fn right_arm(b: &mut Brush, v: &View, lit: &Lit, arm: &Arm, frog: V3, dir: V3, n: V3) {
    // The far side of the stick (away from the player): the fingers drape over to it.
    let mut far = n.cross(dir).norm();
    if far.dot(arm.f) < 0.0 {
        far = -far;
    }
    let palm = frog + n * 2.4 + dir * 1.6 - far * 0.6;
    let wrist = palm + n * 2.6 - dir * 3.4 - far * 1.4;
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
    if let Some(j) = arm.joints {
        ball(b, v, lit, arm.shoulder, arm.r.0 * 1.25, j);
        ball(b, v, lit, el, arm.r.0 * 1.05, j);
        ball(b, v, lit, wrist, arm.r.1 * 0.95, j);
    }
    hand(b, v, lit, arm, wrist, palm, frog, dir, n, far);
}

/// The bow hand: the palm from the wrist over the stick, four fingers draped over it (the
/// little finger short, on top), the thumb under it at the frog's front.
#[allow(clippy::too_many_arguments)]
fn hand(
    b: &mut Brush,
    v: &View,
    lit: &Lit,
    arm: &Arm,
    wrist: V3,
    palm: V3,
    frog: V3,
    dir: V3,
    n: V3,
    far: V3,
) {
    let skin = arm.hand;
    let k = if arm.joints.is_some() { 0.9 } else { 1.0 };
    tube(b, v, lit, wrist, palm, 2.5 * k, 2.8 * k, skin);
    for j in 0..4 {
        let along = (j as f32 - 1.5) * 1.65 * k;
        let knuckle = palm + dir * along + far * (1.6 * k) - n * 0.2;
        let (bend, reach, r) = if j == 3 {
            // The little finger, curved, its tip on top of the stick.
            (1.0, 0.6, 0.62)
        } else {
            (1.9, 2.4, 0.76)
        };
        let mid = knuckle + far * (bend * k) - n * (1.0 * k);
        let tip = mid - n * (reach * k) - far * (0.2 * k) + dir * (0.2 * k);
        tube(b, v, lit, knuckle, mid, r * 1.08 * k, r * k, skin);
        tube(b, v, lit, mid, tip, r * k, r * 0.88 * k, skin);
    }
    // The thumb, bent, under the stick at the frog's front.
    let t0 = palm - far * (1.4 * k) - dir * (1.2 * k) + n * 0.3;
    let t1 = frog + dir * 0.7 - n * 0.4 - far * 0.3;
    tube(b, v, lit, t0, t1, 1.0 * k, 0.85 * k, skin);
}
