//! r42:arcus-upper — what an instrument asks of the figure that plays it (ADR 0159): the
//! **PlayRig** contract and the poser that fills it for a realistic figure. An instrument
//! says where the figure sits or stands and which way it faces, where its head goes (a chin
//! rest's point, or where it looks), what each hand holds (a pose already worked out — the
//! live arms' —, a grip from the library, or nothing: the arm at rest) and what stands in
//! front of the figure's limbs (the occluders the live layer splits them against); the
//! poser sets every bone. Arcus is its first client (`bowed_human::body` builds its rig from
//! the stylised skeleton); a test's held stick is the second, so the contract is not
//! Arcus-shaped.

use super::grip::{self, Grip};
use super::rig::{Aim, Rot};
use crate::scene::bowed_human::{
    self as hu, arm_aims, arm_lengths, ArmAim, Figure, Posed, BONES, CLAV, FOOT, HEAD, HIPS, NECK,
    SHIN, THIGH, UPPER1,
};
use crate::scene::geo::{v, V3};

const UP: V3 = V3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
};

/// How far the hip joints sit over what they are seated on (cm).
pub const HIP_OVER_SEAT: f32 = 9.5;

/// Where the head goes and how it is turned (`up` the crown's way, `face` the face's).
#[derive(Copy, Clone, Debug)]
pub struct Head {
    pub at: HeadAt,
    pub up: V3,
    pub face: V3,
}

#[derive(Copy, Clone, Debug)]
pub enum HeadAt {
    /// The jaw's left side rests here (a violin's chin rest): the body hangs from it.
    Jaw(V3),
    /// The eyes' middle, a little behind the face, goes here.
    Eyes(V3),
}

/// What carries the figure.
#[derive(Copy, Clone, Debug)]
pub enum Seat {
    /// Hung from the head (a chin rest): the trunk leant `lean` radians forward and turned
    /// `turn` radians (its left shoulder forward when positive); the chair's height follows.
    FromHead { lean: f32, turn: f32 },
    /// A chair or a stool: the hips over `at` (its x, z), its seat's top at `top` (y).
    Chair { at: V3, top: f32 },
    /// Standing at `at` on the floor (y 0), the feet `stance` cm apart.
    Standing { at: V3, stance: f32 },
}

/// Where the legs go.
#[derive(Copy, Clone, Debug)]
pub enum Legs {
    /// Seated, the knees forward from the hips, the feet under them.
    Forward,
    /// To these knees and ankles (left, right), the feet flat on the floor.
    To { knees: [V3; 2], ankles: [V3; 2] },
    /// Standing: straight down to the floor.
    Straight,
}

/// What a hand does. (A rig is built where a picture is baked, not per frame: the
/// worked-out pose is kept inline rather than boxed.)
#[allow(clippy::large_enum_variant)]
#[derive(Copy, Clone, Debug)]
pub enum Hand {
    /// A pose already worked out (the live arms: the elbow, the wrist, the fingers).
    Aim(ArmAim),
    /// A grip from the library, the elbow bent toward `pole`.
    Grip(Grip, V3),
    /// At rest: the wrist toward `wrist`, the elbow toward `pole`.
    Rest { wrist: V3, pole: V3 },
}

/// What stands in front of a limb: a capsule (the live layer paints what it hides first,
/// then stamps the picture back over it).
#[derive(Copy, Clone, Debug)]
pub struct Occluder {
    pub a: V3,
    pub b: V3,
    pub r: f32,
}

/// The contract an instrument fills.
#[derive(Clone, Debug)]
pub struct PlayRig {
    /// The figure's facing and its right (level).
    pub facing: V3,
    pub right: V3,
    pub head: Head,
    pub seat: Seat,
    pub legs: Legs,
    /// Left, right.
    pub hands: [Hand; 2],
    pub occluders: Vec<Occluder>,
}

/// The figure's own turn: its left (+x at rest) to the rig's left, its front to the rig's
/// front.
pub fn facing(rig: &PlayRig) -> Rot {
    Rot {
        x: -rig.right,
        y: UP,
        z: rig.facing,
    }
}

/// `fig` posed for `rig`: every bone, and the seat's height (0 standing).
pub fn pose(fig: &Figure, rig: &PlayRig) -> (Posed, f32) {
    let b = &fig.bones;
    let (f, r) = (rig.facing, rig.right);
    let g = facing(rig);
    let head_rot = Rot::frame(rig.head.up.cross(rig.head.face), rig.head.up)
        .mul(&Rot::frame(b[HEAD].x, b[HEAD].tail - b[HEAD].head).t());
    let top_rest = b[HEAD].head;
    let (anchor, target) = match rig.head.at {
        HeadAt::Jaw(p) => (fig.mark("jaw.L"), p),
        HeadAt::Eyes(p) => {
            let eyes = (fig.mark("eye.L") + fig.mark("eye.R")) * 0.5 - v(0.0, 0.0, 6.5);
            (eyes, p)
        }
    };
    let t_top = target - head_rot.apply(anchor - top_rest);
    let trunk_rest = g.apply(top_rest - b[HIPS].head);
    let hip_drop = b[HIPS].head.y - b[hu::side(0) + THIGH].head.y;
    let (dir, hips_at, g) = match rig.seat {
        Seat::FromHead { lean, turn } => {
            let dir = (UP * lean.cos() + f * lean.sin()).norm();
            let hips = t_top - dir * trunk_rest.len();
            // Turned about the vertical so the left shoulder comes forward.
            let tr = |a: f32| Rot::about(UP, a);
            let a = if tr(0.2).apply(-r).dot(f) > 0.0 {
                turn
            } else {
                -turn
            };
            (dir, hips, tr(a).mul(&g))
        }
        Seat::Chair { at, top } => {
            let hips = v(at.x, top + HIP_OVER_SEAT + hip_drop, at.z);
            ((t_top - hips).norm(), hips, g)
        }
        Seat::Standing { at, .. } => {
            // The hips over the feet at the legs' length; the trunk leans to the head.
            let o = hu::side(0);
            let legs = b[o + THIGH].len() + b[o + hu::SHIN].len();
            let ankle_h = b[o + FOOT].head.y - fig.mark("sole").y;
            let hips = v(at.x, ankle_h + legs * 0.995 + hip_drop, at.z);
            ((t_top - hips).norm(), hips, g)
        }
    };
    let trunk_rest = g.apply(top_rest - b[HIPS].head);
    let root = Rot::between(trunk_rest.norm(), dir).mul(&g);
    let off = hips_at - root.apply(b[HIPS].head);
    let mut aims: [Option<Aim>; BONES] = [None; BONES];
    // The neck halfway between the trunk's way and the head's, the head as the rig turns it.
    let trunk_up = root.apply((b[NECK].tail - b[NECK].head).norm());
    aims[NECK] = Some(Aim {
        head: None,
        y: (trunk_up + rig.head.up).norm(),
        x: None,
        len: None,
    });
    aims[HEAD] = Some(Aim {
        head: None,
        y: rig.head.up,
        x: Some(rig.head.up.cross(rig.head.face)),
        len: None,
    });
    let mut first = hu::pose(fig, &aims, &root, off);
    let mut off = off;
    if let Seat::FromHead { .. } = rig.seat {
        // What the neck's bend moved, taken back (the chair follows).
        off = off + (target - first.at(HEAD, anchor));
        first = hu::pose(fig, &aims, &root, off);
    }
    let sole = fig.mark("sole").y;
    for (i, sd) in [0usize, 1].into_iter().enumerate() {
        let o = hu::side(sd);
        let hip = first.at(HIPS, b[o + THIGH].head);
        let (lt, ls) = (b[o + THIGH].len(), b[o + SHIN].len());
        let ankle_h = b[o + FOOT].head.y - sole;
        let out = if sd == 0 { -r } else { r };
        let (knee_t, ankle_t) = match (rig.legs, rig.seat) {
            (Legs::To { knees, ankles }, _) => {
                (knees[i], v(ankles[i].x, ankle_h, ankles[i].z) + out * 2.0)
            }
            (Legs::Straight, Seat::Standing { at, stance }) => {
                let a = v(at.x, ankle_h, at.z) + out * (stance * 0.5);
                ((hip + a) * 0.5 + f * 2.0, a)
            }
            (Legs::Straight, _) => {
                let a = v(hip.x, ankle_h, hip.z);
                ((hip + a) * 0.5 + f * 2.0, a)
            }
            (Legs::Forward, _) => {
                let k = hip + f * (lt * 0.96) + out * 3.0;
                (k, v(k.x, ankle_h, k.z) + f * 6.0 + out * 1.5)
            }
        };
        let kn = crate::scene::figure::rig::two_bone(
            hip,
            ankle_t,
            lt,
            ls,
            knee_t - (hip + ankle_t) * 0.5,
        );
        aims[o + THIGH] = Some(Aim {
            head: None,
            y: (kn - hip).norm(),
            x: None,
            len: None,
        });
        aims[o + SHIN] = Some(Aim {
            head: None,
            y: (ankle_t - kn).norm(),
            x: None,
            len: None,
        });
        // The foot flat, turned the way the knee points.
        let fr = (b[o + FOOT].tail - b[o + FOOT].head).norm();
        let flat = v(kn.x - hip.x, 0.0, kn.z - hip.z);
        let heading = if flat.len() > 0.5 {
            flat.norm()
        } else {
            v(f.x, 0.0, f.z).norm()
        };
        let turn = Rot::between(v(f.x, 0.0, f.z).norm(), heading);
        aims[o + FOOT] = Some(Aim {
            head: None,
            y: turn.mul(&g).apply(fr),
            x: Some(turn.mul(&g).apply(b[o + FOOT].x)),
            len: None,
        });
        let sh = first.at(o + CLAV, b[o + UPPER1].head);
        let a = arm_for(fig, sd, sh, &rig.hands[sd]);
        arm_aims(fig, &mut aims, sd, &a, sh);
    }
    let posed = hu::pose(fig, &aims, &root, off);
    let seat = match rig.seat {
        Seat::Standing { .. } => 0.0,
        _ => posed.at(HIPS, b[hu::side(0) + THIGH].head).y - HIP_OVER_SEAT,
    };
    (posed, seat)
}

/// An arm's pose for what its hand does, from the shoulder `sh`.
pub fn arm_for(fig: &Figure, sd: usize, sh: V3, hand: &Hand) -> ArmAim {
    let (up, fo) = arm_lengths(fig, sd);
    match *hand {
        Hand::Aim(a) => a,
        Hand::Rest { wrist, pole } => {
            let elbow = super::rig::two_bone(sh, wrist, up, fo, pole);
            ArmAim {
                elbow,
                wrist: elbow + (wrist - elbow).norm() * fo,
                hand: None,
            }
        }
        Hand::Grip(g, pole) => {
            let k = hu::hand_reach(fig, sd) / grip::WOMAN_REACH;
            let j = grip::joints(&g, k, sd);
            let elbow = super::rig::two_bone(sh, j.wrist, up, fo, pole);
            ArmAim {
                elbow,
                wrist: j.wrist,
                hand: Some(hu::HandAim {
                    fingers: j.fingers,
                    thumb: j.thumb,
                    exact: None,
                    rots: None,
                }),
            }
        }
    }
}
