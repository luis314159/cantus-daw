//! r42:arcus-upper — what the live limbs share (ADR 0159): the capsules a limb's pieces and
//! the things a hand holds are, for the shade they cast on the corners near them (the
//! creases between fingers, a finger's side against a stick, the palm under the fingers).
//! The painter itself (skinning an arm per frame, splitting it against an occluder, the
//! light by the normal's way) is `views/inst/arcus_draw_flesh.rs`.

use crate::scene::geo::V3;

/// A limb's piece or a held thing as an occluder: from `a` to `b`, `r` thick, the bone it
/// is (`usize::MAX`: an object the hand holds).
#[derive(Copy, Clone, Debug)]
pub struct Capsule {
    pub a: V3,
    pub b: V3,
    pub r: f32,
    pub bone: usize,
}

/// How much of the light round `p` (normal `n`) the capsule `c` takes: 0 far away or
/// facing away, rising as the corner nears and faces it.
pub fn shade(c: &Capsule, p: V3, n: V3) -> f32 {
    let ab = c.b - c.a;
    let t = ((p - c.a).dot(ab) / ab.dot(ab).max(1e-6)).clamp(0.0, 1.0);
    let d = c.a + ab * t - p;
    let dist = d.len();
    let gap = (dist - c.r).max(0.0);
    if gap > 2.2 {
        return 0.0;
    }
    let facing = (n.dot(d * (1.0 / dist.max(1e-4))) * 0.7 + 0.3).max(0.0);
    let f = 1.0 - gap / 2.2;
    f * f * facing * (c.r / (c.r + gap)).min(1.0)
}
