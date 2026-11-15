//! From a [`Fig`] and a character's proportions to the joints it draws (the design rig's
//! `body.py`): the pelvis, a torso that leans about it, shoulders that roll and narrow with
//! a turn, legs by two-bone IK with **the ground rule** — a leg never stretches past its
//! length, the pelvis drops instead — and arms by hand target or by angle and bend.

use super::fig::{add, ik2, mul, rot0, sub, Fig};
use crate::rig::P;
use cantus_dsp::math;
use std::f32::consts::PI;

/// A character's proportions, in units of a figure about one unit tall.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Props {
    /// The hip's height standing (L).
    pub leg: f32,
    pub thigh: f32,
    pub shin: f32,
    /// Pelvis to the neck's base (T).
    pub torso: f32,
    /// The neck's base to the head's bottom.
    pub neck: f32,
    /// The head's radius.
    pub head: f32,
    /// Half the shoulders' width.
    pub shoulder: f32,
    /// Half the hips' width.
    pub hipw: f32,
    /// Half the distance between the feet standing.
    pub stance: f32,
    pub upper: f32,
    pub fore: f32,
    /// A foot's length.
    pub foot: f32,
}

impl Props {
    /// The arm's whole length (A).
    pub fn arm(&self) -> f32 {
        self.upper + self.fore
    }
}

/// Where everything is, in the character's units (before [`place`]).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Joints {
    pub pelvis: P,
    pub neck: P,
    /// The head's centre.
    pub head: P,
    /// The head's roll.
    pub head_deg: f32,
    pub torso_deg: f32,
    pub hips: [P; 2],
    pub knees: [P; 2],
    pub feet: [P; 2],
    /// Foot angles.
    pub toes: [f32; 2],
    pub shoulders: [P; 2],
    pub elbows: [P; 2],
    pub hands: [P; 2],
    pub squash: f32,
    pub lift: f32,
}

/// IK picking the bend so the joint bulges away from the body (`sign` +1) or toward it
/// (−1); `side` is −1 for the viewer's left limb.
fn outward(root: P, target: P, l1: f32, l2: f32, side: f32, sign: f32) -> (P, P) {
    let (j1, end) = ik2(root, target, l1, l2, 1.0);
    let (j2, _) = ik2(root, target, l1, l2, -1.0);
    let o1 = (j1.0 - (root.0 + end.0) / 2.0) * side;
    let o2 = (j2.0 - (root.0 + end.0) / 2.0) * side;
    let pick = if (o1 - o2).abs() < 1e-4 {
        // A horizontal limb: bulge down when out, up when in.
        if (j1.1 > j2.1) == (sign > 0.0) {
            j1
        } else {
            j2
        }
    } else if (o1 > o2) == (sign > 0.0) {
        j1
    } else {
        j2
    };
    (pick, end)
}

const SIDES: [f32; 2] = [-1.0, 1.0];

/// The joints of `p` for a character of proportions `pr`.
pub fn solve(p: &Fig, pr: &Props) -> Joints {
    let l = pr.leg;
    let mut pelvis = (p.hip.0 * l, -l + p.hip.1 * l);
    let torso_deg = p.lean;
    let up = rot0((0.0, -pr.torso), torso_deg);
    let mut neck = add(
        add(pelvis, up),
        (p.chest.0 * pr.torso, p.chest.1 * pr.torso),
    );
    // Shoulders: across the torso's top, rolled, narrowed by a turn.
    let sw = pr.shoulder * (1.0 - 0.35 * p.turn.abs());
    let across = rot0((sw, 0.0), torso_deg - p.roll);
    let mut shoulders = [sub(neck, across), add(neck, across)];
    let hw = pr.hipw * (1.0 - 0.3 * p.turn.abs());
    let hacross = rot0((hw, 0.0), torso_deg * 0.4);
    let mut hips = [sub(pelvis, hacross), add(pelvis, hacross)];
    let head_deg = torso_deg * 0.6 + p.tilt;
    let hup = rot0((0.0, -(pr.neck + pr.head)), torso_deg * 0.5 + p.tilt * 0.3);
    let mut head = add(add(neck, hup), (0.0, p.nod * 0.0012));
    // The ground holds: a leg never stretches past its length — the pelvis drops instead.
    let want: [P; 2] =
        std::array::from_fn(|i| (SIDES[i] * pr.stance + p.feet[i].0 * l, p.feet[i].1 * l));
    let reach = (pr.thigh + pr.shin) * 0.995;
    let mut drop = 0.0f32;
    for i in 0..2 {
        let dx = want[i].0 - hips[i].0;
        let gap = want[i].1 - hips[i].1;
        let room = (reach * reach - dx * dx).max(0.0).sqrt();
        drop = drop.max(gap - room);
    }
    if drop > 0.0 {
        let down = |q: P| (q.0, q.1 + drop);
        pelvis = down(pelvis);
        neck = down(neck);
        shoulders = shoulders.map(down);
        hips = hips.map(down);
        head = down(head);
    }
    let mut feet = [(0.0, 0.0); 2];
    let mut knees = [(0.0, 0.0); 2];
    for i in 0..2 {
        let (k, f) = outward(hips[i], want[i], pr.thigh, pr.shin, SIDES[i], p.knee[i]);
        feet[i] = f;
        knees[i] = k;
    }
    let mut elbows = [(0.0, 0.0); 2];
    let mut hands = [(0.0, 0.0); 2];
    for i in 0..2 {
        let side = SIDES[i];
        if let Some(fk) = p.fk {
            let (mut a, bend) = fk[i];
            a -= torso_deg * side; // the arms turn with the torso
            let dir = |deg: f32| {
                let (s, c) = math::sin_cos(deg * (PI / 180.0));
                (side * s, c)
            };
            let e = add(shoulders[i], mul(dir(a), pr.upper));
            elbows[i] = e;
            hands[i] = add(e, mul(dir(a + bend), pr.fore));
            continue;
        }
        let t = add(neck, mul(p.hands[i], pr.arm()));
        let (e, h) = outward(shoulders[i], t, pr.upper, pr.fore, side, p.elbow[i]);
        elbows[i] = e;
        hands[i] = h;
    }
    Joints {
        pelvis,
        neck,
        head,
        head_deg,
        torso_deg,
        hips,
        knees,
        feet,
        toes: p.toe,
        shoulders,
        elbows,
        hands,
        squash: p.squash,
        lift: p.lift,
    }
}

/// Squash about the ground (the area kept), then lift: what every drawn point goes through.
pub fn place(pt: P, j: &Joints) -> P {
    let s = j.squash;
    (pt.0 * (1.0 + s * 0.6), pt.1 * (1.0 - s) - j.lift)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PR: Props = Props {
        leg: 0.43,
        thigh: 0.222,
        shin: 0.222,
        torso: 0.275,
        neck: 0.030,
        head: 0.092,
        shoulder: 0.066,
        hipw: 0.040,
        stance: 0.056,
        upper: 0.148,
        fore: 0.150,
        foot: 0.05,
    };

    #[test]
    fn standing_its_feet_are_on_the_ground_and_its_head_on_top() {
        let j = solve(&Fig::default(), &PR);
        for f in j.feet {
            assert!(f.1.abs() < 1e-4, "{f:?}");
        }
        assert!(j.head.1 < j.neck.1 && j.neck.1 < j.pelvis.1);
    }

    #[test]
    fn a_foot_far_out_drops_the_pelvis_rather_than_stretch_the_leg() {
        let wide = Fig {
            feet: [(-0.6, 0.0), (0.6, 0.0)],
            ..Fig::default()
        };
        let j = solve(&wide, &PR);
        assert!(j.pelvis.1 > -PR.leg + 0.01, "the pelvis dropped");
        for i in 0..2 {
            let leg = super::super::fig::length(sub(j.feet[i], j.hips[i]));
            assert!(leg <= (PR.thigh + PR.shin) * 1.0001);
            assert!(j.feet[i].1.abs() < 1e-3, "the foot stays down");
        }
    }
}
