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

/// The bow hand's joints (the grip library's, ADR 0159).
pub type BowHand = crate::scene::figure::grip::Joints;

/// Where the hand holds the bow at its `frog` (the hair's point there; the bow's way `dir`,
/// frog to tip; `n` the string's normal, the side the stick is on; `held` a violin's or a
/// viola's bow): the grip library's bow hold (`figure::grip::bow_frog`), the hand the
/// figure's own size (the mannequin's a little smaller).
pub fn bow_hand(arm: &Arm, frog: V3, dir: V3, n: V3, held: bool) -> BowHand {
    let k = arm.palm * if arm.joints.is_some() { 0.9 } else { 1.0 };
    crate::scene::figure::grip::bow_frog(frog, dir, n, arm.f, held, k)
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
