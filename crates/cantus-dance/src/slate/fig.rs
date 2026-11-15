//! A figure's pose (round 40, ADR 0122): the design rig's `Pose`
//! (`docs/round40/design/dancer/src/rig.py`), ported number for number so the app draws what
//! the owner approved. It is independent of the character's proportions: lengths are shares
//! of the character's leg (L), arm (A) or torso (T); angles are degrees.
//!
//! It adds over the family's [`Body`](crate::rig::dance::Body) a pelvis that moves, legs
//! with knees (two-bone IK, [`ik2`]), hands that reach a target, a chest that isolates from
//! the hips, and **snap time** ([`snap_time`]): pose-to-pose holds on the grid, the hit on
//! the beat and the settle just after it.
//!
//! The easing here is the design rig's own (a cubic ease-out, an ease-out-back with its
//! overshoot as an argument), not `rig.rs`'s: the shapes of the approved GIFs depend on it.

use crate::rig::P;
use cantus_dsp::math;
use std::f32::consts::PI;

pub fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn lerp_p(a: P, b: P, t: f32) -> P {
    (lerp(a.0, b.0, t), lerp(a.1, b.1, t))
}

pub fn lerp_pair(a: [P; 2], b: [P; 2], t: f32) -> [P; 2] {
    [lerp_p(a[0], b[0], t), lerp_p(a[1], b[1], t)]
}

pub fn smooth(t: f32) -> f32 {
    let t = clamp01(t);
    t * t * (3.0 - 2.0 * t)
}

/// Cubic ease-out (the design rig's).
pub fn ease_out(t: f32) -> f32 {
    let u = 1.0 - clamp01(t);
    1.0 - u * u * u
}

/// Cubic ease-in.
pub fn ease_in(t: f32) -> f32 {
    let t = clamp01(t);
    t * t * t
}

pub fn ease_in_out(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - u * u * u / 2.0
    }
}

/// Overshoots by `s` (the design's default 1.9) and settles.
pub fn ease_out_back(t: f32, s: f32) -> f32 {
    let t = clamp01(t) - 1.0;
    1.0 + (s + 1.0) * t * t * t + s * t * t
}

pub fn sin1(t: f32) -> f32 {
    math::sin(2.0 * PI * t)
}

pub fn cos1(t: f32) -> f32 {
    math::cos(2.0 * PI * t)
}

/// 0 at both ends, 1 in the middle (a hop).
pub fn arc(t: f32) -> f32 {
    let t = clamp01(t);
    4.0 * t * (1.0 - t)
}

/// 1 on the beat, 0 half way: how far down a bounce is (`ph` the phase in the beat).
pub fn beat_down(ph: f32, sharp: f32) -> f32 {
    let x = (0.5 + 0.5 * math::cos(2.0 * PI * ph)).max(0.0);
    if x <= 0.0 {
        0.0
    } else {
        math::powf(x, sharp)
    }
}

/// Python's `%` for a positive modulus (the result has the modulus's sign).
pub fn pmod(a: f32, m: f32) -> f32 {
    a.rem_euclid(m)
}

pub fn add(a: P, b: P) -> P {
    (a.0 + b.0, a.1 + b.1)
}

pub fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}

pub fn mul(a: P, k: f32) -> P {
    (a.0 * k, a.1 * k)
}

pub fn length(a: P) -> f32 {
    (a.0 * a.0 + a.1 * a.1).sqrt()
}

/// `p` turned by `deg` about `about` (clockwise on screen, y down).
pub fn rot(p: P, deg: f32, about: P) -> P {
    let (s, c) = math::sin_cos(deg * (PI / 180.0));
    let (x, y) = (p.0 - about.0, p.1 - about.1);
    (about.0 + x * c - y * s, about.1 + x * s + y * c)
}

/// `p` turned by `deg` about the origin.
pub fn rot0(p: P, deg: f32) -> P {
    rot(p, deg, (0.0, 0.0))
}

/// Two-bone IK: the joint between `root` and the reached end (`target`, pulled in when out
/// of reach). `bend` +1/−1 picks the side the joint bulges to (+1: to the right of the
/// root → target line, y down).
pub fn ik2(root: P, target: P, l1: f32, l2: f32, bend: f32) -> (P, P) {
    let mut d = sub(target, root);
    let mut dist = length(d);
    let mut target = target;
    let reach = (l1 + l2) * 0.999;
    if dist > reach {
        d = mul(d, reach / dist);
        dist = reach;
        target = add(root, d);
    }
    let dist = dist.max(1e-6);
    let a = (l1 * l1 - l2 * l2 + dist * dist) / (2.0 * dist);
    let h = (l1 * l1 - a * a).max(0.0).sqrt();
    let (ux, uy) = (d.0 / dist, d.1 / dist);
    let mid = (root.0 + ux * a, root.1 + uy * a);
    ((mid.0 - uy * h * bend, mid.1 + ux * h * bend), target)
}

/// Pose-to-pose time: the pose holds a key, then travels to the next in `mv` of a `step`
/// (eased out with an overshoot), crossing the key **on** its grid line and settling just
/// after it — pop and lock: the hit on the beat, the settle after it. `amount` 0 leaves
/// the time as it is.
pub fn snap_time(b: f32, step: f32, mv: f32, amount: f32, overshoot: f32) -> f32 {
    if amount == 0.0 {
        return b;
    }
    // Where ease_out_back first reaches 1.
    let lead = (1.0 - overshoot / (overshoot + 1.0)) * mv * step;
    let b2 = b + lead;
    let k = (b2 / step).floor();
    let f = (b2 - k * step) / step;
    let g = if f < mv {
        ease_out_back(f / mv, overshoot)
    } else {
        1.0
    };
    let warped = (k - 1.0 + g) * step;
    let w = lerp(b, warped, amount);
    // On a grid line it is on it exactly: a hair before would count as the beat before
    // (which hand hits, which side it sways to).
    let line = (w / step).round() * step;
    if (w - line).abs() < 1e-5 {
        line
    } else {
        w
    }
}

/// [`snap_time`] with the design's defaults (a half-beat grid, 0.32 of it to travel, an
/// overshoot of 1.9).
pub fn snap(b: f32, amount: f32) -> f32 {
    snap_time(b, 0.5, 0.32, amount, 1.9)
}

/// The eyes a pose asks for (a character draws them its own way; Compás has none and cuts a
/// visor only for `Cool`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Eyes {
    #[default]
    Open,
    Happy,
    Closed,
    Wide,
    Wink,
    Cool,
    Sleepy,
}

/// A figure's pose (the design rig's `Pose`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Fig {
    /// The pelvis's offset from standing (L units; y + is down).
    pub hip: P,
    /// The whole body off the ground (units of height; + is up).
    pub lift: f32,
    /// The torso about the pelvis (+ leans to the viewer's right).
    pub lean: f32,
    /// The chest's isolation from the torso line (T units).
    pub chest: P,
    /// Shoulder roll (+: the right shoulder up), degrees.
    pub roll: f32,
    /// The head down (+) or up (−).
    pub nod: f32,
    /// The head's roll (+ clockwise).
    pub tilt: f32,
    /// Feet from where they stand (L units; y − is lifted).
    pub feet: [P; 2],
    /// Foot angles (the tip up +).
    pub toe: [f32; 2],
    /// Hand targets from the neck's base (A units), when `fk` is `None`.
    pub hands: [P; 2],
    /// +1 the elbow bulges outward, −1 inward.
    pub elbow: [f32; 2],
    /// Arms by angle and bend instead of hand targets, as the family's `Body::arm`: degrees
    /// from hanging (0) through out to the side (90) to straight up (180); the forearm turns
    /// `bend` further the same way (negative: back toward the body).
    pub fk: Option<[(f32, f32); 2]>,
    /// +1 the knee bulges outward, −1 inward.
    pub knee: [f32; 2],
    /// > 0 squashed, < 0 stretched (the area kept).
    pub squash: f32,
    /// −1 … 1: a hint of three-quarter (the shoulders narrow).
    pub turn: f32,
    pub eyes: Eyes,
    /// 0 closed … 1 open (singing).
    pub mouth: f32,
    /// 1 on the beat, decaying (the floor ring, the lights).
    pub accent: f32,
    /// What the music is doing (0…1): dots, a woofer.
    pub level: f32,
    /// 1 the frame two hands meet, decaying.
    pub claps: f32,
    /// 1 on a landing, decaying (dust).
    pub landed: f32,
    /// A pointer's or antenna's swing (Tono), degrees.
    pub point: f32,
    /// Beats since the move began (what a character animates itself).
    pub t: f32,
    /// A signature move's own progress (0…1), per character.
    pub sig: f32,
    /// A character's own morph (Onda: sine 0 → square 1).
    pub shape: f32,
}

impl Default for Fig {
    fn default() -> Self {
        Fig {
            hip: (0.0, 0.0),
            lift: 0.0,
            lean: 0.0,
            chest: (0.0, 0.0),
            roll: 0.0,
            nod: 0.0,
            tilt: 0.0,
            feet: [(0.0, 0.0); 2],
            toe: [0.0; 2],
            hands: [(-0.40, 0.92), (0.40, 0.92)],
            elbow: [1.0; 2],
            fk: None,
            knee: [1.0; 2],
            squash: 0.0,
            turn: 0.0,
            eyes: Eyes::Open,
            mouth: 0.0,
            accent: 0.0,
            level: 0.6,
            claps: 0.0,
            landed: 0.0,
            point: 0.0,
            t: 0.0,
            sig: 0.0,
            shape: 0.0,
        }
    }
}

impl Fig {
    /// `p` → `q` by `t` (`Body::lerp`'s rule); the face switches half way. Arms by angle
    /// and arms by target do not mix: the one nearer wins (no character mixes them).
    pub fn blend(p: &Fig, q: &Fig, t: f32) -> Fig {
        let l = |a: f32, b: f32| lerp(a, b, t);
        let l2 = |a: [f32; 2], b: [f32; 2]| [l(a[0], b[0]), l(a[1], b[1])];
        let lp = |a: P, b: P| lerp_p(a, b, t);
        let fk = match (p.fk, q.fk) {
            (Some(a), Some(b)) => Some([
                (l(a[0].0, b[0].0), l(a[0].1, b[0].1)),
                (l(a[1].0, b[1].0), l(a[1].1, b[1].1)),
            ]),
            (a, b) => {
                if t >= 0.5 {
                    b
                } else {
                    a
                }
            }
        };
        let late = t >= 0.5;
        Fig {
            hip: lp(p.hip, q.hip),
            lift: l(p.lift, q.lift),
            lean: l(p.lean, q.lean),
            chest: lp(p.chest, q.chest),
            roll: l(p.roll, q.roll),
            nod: l(p.nod, q.nod),
            tilt: l(p.tilt, q.tilt),
            feet: [lp(p.feet[0], q.feet[0]), lp(p.feet[1], q.feet[1])],
            toe: l2(p.toe, q.toe),
            hands: [lp(p.hands[0], q.hands[0]), lp(p.hands[1], q.hands[1])],
            elbow: l2(p.elbow, q.elbow),
            fk,
            knee: l2(p.knee, q.knee),
            squash: l(p.squash, q.squash),
            turn: l(p.turn, q.turn),
            eyes: if late { q.eyes } else { p.eyes },
            mouth: l(p.mouth, q.mouth),
            accent: l(p.accent, q.accent),
            level: l(p.level, q.level),
            claps: l(p.claps, q.claps),
            landed: l(p.landed, q.landed),
            point: l(p.point, q.point),
            t: l(p.t, q.t),
            sig: l(p.sig, q.sig),
            shape: l(p.shape, q.shape),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_time_hits_the_grid_line_and_holds_between() {
        // On a half-beat line the warped time is on the line (the hit); just before the
        // next travel it holds the key.
        for k in 0..8 {
            let b = k as f32 * 0.5;
            let lead = (1.0 - 1.9 / 2.9) * 0.32 * 0.5;
            assert!((snap(b, 1.0) - b).abs() < 1e-4, "{b}: {}", snap(b, 1.0));
            let held_a = snap(b + 0.2, 1.0);
            let held_b = snap(b + 0.5 - lead - 0.01, 1.0);
            assert!((held_a - held_b).abs() < 1e-5, "held between the snaps");
        }
        assert_eq!(snap(1.37, 0.0), 1.37);
    }

    #[test]
    fn ik_keeps_the_bones_their_length() {
        let (j, end) = ik2((0.0, 0.0), (0.1, 0.3), 0.2, 0.2, 1.0);
        assert!((length(sub(j, (0.0, 0.0))) - 0.2).abs() < 1e-4);
        assert!((length(sub(end, j)) - 0.2).abs() < 1e-4);
        // Out of reach: pulled in along the line.
        let (_, far) = ik2((0.0, 0.0), (0.0, 1.0), 0.2, 0.2, 1.0);
        assert!(far.1 < 0.4 && far.1 > 0.39);
    }

    #[test]
    fn a_blend_ends_where_it_should() {
        let a = Fig::default();
        let b = Fig {
            lean: 10.0,
            eyes: Eyes::Wide,
            ..Fig::default()
        };
        assert_eq!(Fig::blend(&a, &b, 0.0), a);
        assert_eq!(Fig::blend(&a, &b, 1.0), b);
        assert_eq!(Fig::blend(&a, &b, 0.4).eyes, Eyes::Open);
    }
}
