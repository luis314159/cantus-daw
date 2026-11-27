//! r42:arcus-hands — a finger posed to a fingertip as a real one bends: its three joints
//! together (the owner, 2026-11-25, of the bow hands: fingers hanging straight down like a
//! claw; the earlier reach bent a finger at its middle joint only, the last two phalanges
//! one rigid stick). The last joint follows the middle one (`COUPLE`: a real finger's DIP
//! turns about two thirds of its PIP when it curls freely — Kapandji's hand; Hahn et al.
//! 1995), so a finger curls round a stick or arches over a string the way photographs of
//! professional bow holds show it.
//!
//! In the plane through the knuckle, the tip and the pose's middle joint: the chain's reach
//! for a middle joint's bend `b` (the last joint's `COUPLE · b`) does not depend on the base
//! joint's angle, so `b` is found by bisection for the knuckle-to-tip distance and the base
//! joint is then turned to point the chain at the tip. A tip out of reach straightens the
//! finger and stretches it (as the playing finger on its stop must land exactly); one too
//! near curls it to the most bend.

use crate::scene::geo::V3;

/// How far the last joint bends for the middle joint's bend.
pub const COUPLE: f32 = 0.7;
/// The middle joint's most bend (radians, ~115°).
const MOST: f32 = 2.0;

/// The reach of a chain of `l` bent `b` at its middle joint (and `COUPLE · b` at its last),
/// as the vector from its base in its own plane (x along the unbent finger, y toward the
/// back of the finger).
fn reach(l: [f32; 3], b: f32) -> (f32, f32) {
    let c = COUPLE * b;
    (
        l[0] + l[1] * b.cos() + l[2] * (b + c).cos(),
        -l[1] * b.sin() - l[2] * (b + c).sin(),
    )
}

/// The finger's joints (knuckle, middle joint, last joint, tip) from knuckle `k0` toward
/// `tip`, bent away from `pole` (a point on the finger's back side: the pose's middle joint),
/// its phalanges `l` long.
pub fn curl(k0: V3, tip: V3, pole: V3, l: [f32; 3]) -> [V3; 4] {
    let d = tip - k0;
    let dist = d.len();
    let e = if dist > 1e-5 {
        d * (1.0 / dist)
    } else {
        V3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        }
    };
    let mid = (k0 + tip) * 0.5;
    let pr = pole - mid;
    let mut y = pr - e * pr.dot(e);
    if y.len() < 1e-5 {
        y = crate::scene::geo::basis(e).0;
    }
    let y = y.norm();
    let full = l[0] + l[1] + l[2];
    let (l, b) = if dist >= full * 0.999 {
        // Out of reach: straight, stretched to the tip.
        let k = dist / full.max(1e-5);
        ([l[0] * k, l[1] * k, l[2] * k], 0.0)
    } else {
        let len = |b: f32| {
            let (x, yy) = reach(l, b);
            (x * x + yy * yy).sqrt()
        };
        if len(MOST) >= dist {
            (l, MOST)
        } else {
            let (mut lo, mut hi) = (0.0f32, MOST);
            for _ in 0..24 {
                let m = (lo + hi) * 0.5;
                if len(m) > dist {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            (l, (lo + hi) * 0.5)
        }
    };
    let (rx, ry) = reach(l, b);
    // The base joint turned so the chain points at the tip.
    let phi = -ry.atan2(rx);
    let way = |a: f32| e * a.cos() + y * a.sin();
    let p1 = k0 + way(phi) * l[0];
    let p2 = p1 + way(phi - b) * l[1];
    let p3 = p2 + way(phi - b - COUPLE * b) * l[2];
    [k0, p1, p2, p3]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::geo::v;

    #[test]
    fn a_finger_reaches_its_tip_with_its_own_lengths() {
        let l = [4.0, 2.4, 2.3];
        let k0 = v(0.0, 0.0, 0.0);
        let tip = v(5.5, -1.0, 0.0);
        let j = curl(k0, tip, v(3.0, 3.0, 0.0), l);
        assert!((j[3] - tip).len() < 1e-3, "{:?}", j[3]);
        for (k, want) in l.iter().enumerate() {
            assert!(((j[k + 1] - j[k]).len() - want).abs() < 1e-4);
        }
        // Bent toward the pole's side: the middle joint over the line to the tip.
        assert!(j[1].y > 0.0);
        // The last joint bends with the middle one (about COUPLE of it).
        let a = |p: V3, q: V3, r: V3| (q - p).norm().dot((r - q).norm()).clamp(-1.0, 1.0).acos();
        let (pip, dip) = (a(j[0], j[1], j[2]), a(j[1], j[2], j[3]));
        assert!((dip / pip - COUPLE).abs() < 0.02, "{pip} {dip}");
    }

    #[test]
    fn a_tip_out_of_reach_straightens_and_stretches_the_finger() {
        let l = [4.0, 2.4, 2.3];
        let tip = v(10.0, 0.0, 0.0);
        let j = curl(v(0.0, 0.0, 0.0), tip, v(3.0, 3.0, 0.0), l);
        assert!((j[3] - tip).len() < 1e-3);
        assert!(j[1].y.abs() < 1e-4 && j[2].y.abs() < 1e-4);
    }
}
