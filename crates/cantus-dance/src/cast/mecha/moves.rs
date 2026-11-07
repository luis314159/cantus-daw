//! Volta's own moves (ADR 0105), on top of the family's choreography (`rig::dance`): its
//! point, bounce and wave are **pop and lock** — the motion snaps to each half beat and
//! locks there, with a pop of the chest on the hit; its jump is a **rocket jump** (a fist
//! up through the air, the jets in its back firing, `draw::jet_power`); its freeze is a
//! hero stance, fists on hips, chin up. Phase 0 is the beat, as everywhere.

use crate::pose::Pose;
use crate::rig::dance::{self, Body, Style};
use crate::rig::{self, lerp, TAU};
use cantus_core::inst::dancer::Move;
use cantus_dsp::math;

/// The phase snapped: each of `steps` parts of the cycle reaches its pose in its first
/// fifth and locks there.
fn snapped(p: f32, steps: f32) -> f32 {
    let q = p * steps;
    let n = q.floor();
    let into = q - n;
    ((n + rig::smooth((into / 0.2).min(1.0))) / steps).min(1.0)
}

/// A move as Volta dances it.
pub(super) fn dance_move(mv: Move, p: f32, cycle: u32, pose: &Pose, st: &Style) -> Body {
    match mv {
        Move::Point | Move::Bounce | Move::Wave => pop_lock(mv, p, cycle, pose, st),
        Move::Jump => rocket(p, pose, st),
        Move::Freeze => hero(p, cycle, pose),
        _ => dance::dance(mv, p, cycle, pose, st),
    }
}

/// Pop and lock: the family's move on a snapped phase, and the chest's pop on each hit.
fn pop_lock(mv: Move, p: f32, cycle: u32, pose: &Pose, st: &Style) -> Body {
    let steps = if mv == Move::Wave { 4.0 } else { 2.0 };
    let mut b = dance::dance(mv, snapped(p, steps), cycle, pose, st);
    let hit = rig::pulse((p * steps).rem_euclid(1.0), 0.0, 0.14);
    let x = dance::extreme(pose);
    b.squash += (0.03 + 0.05 * x) * hit;
    b.y += 0.012 * hit;
    b.tilt += 3.0 * hit * rig::side(cycle);
    b
}

/// The rocket jump: the family's jump, a fist thrown up through the air.
fn rocket(p: f32, pose: &Pose, st: &Style) -> Body {
    let mut b = dance::jump(p, pose, st);
    let air = if p >= 0.55 {
        rig::arc(rig::window(p, 0.55, 1.0))
    } else {
        0.0
    };
    let up = rig::smooth((air * 2.0).min(1.0));
    b.arm[1] = lerp(b.arm[1], 172.0, up);
    b.bend[1] = lerp(b.bend[1], 0.0, up);
    b.spread[1] = lerp(b.spread[1], 0.0, up);
    b.arm[0] = lerp(b.arm[0], 34.0, up);
    b.bend[0] = lerp(b.bend[0], 70.0, up);
    b.foot = [(-0.02, -0.05 * air), (0.04, -0.14 * air)];
    b.lean += 5.0 * up;
    b
}

/// The hero stance: struck on the beat and held, fists on hips, feet wide, chin up, the
/// shoulders breathing.
fn hero(p: f32, cycle: u32, pose: &Pose) -> Body {
    let hit = if cycle == 0 {
        rig::ease_out_back(rig::window(p, 0.0, 0.06))
    } else {
        1.0
    };
    let breath = 0.008 * math::sin(pose.seconds as f32 * TAU / 2.6);
    let mut f = Body::rest();
    f.foot = [(-0.06, 0.0), (0.06, 0.0)];
    f.arm = [36.0, 36.0];
    f.bend = [112.0, 112.0];
    f.spread = [0.0, 0.0];
    f.nod = 8.0;
    f.yaw = 10.0;
    f.squash = breath;
    f.squint = 0.3;
    Body::lerp(&Body::rest(), &f, hit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapped_phase_meets_the_cycle_and_locks_between_hits() {
        assert_eq!(snapped(0.0, 2.0), 0.0);
        assert!((snapped(0.999, 2.0) - 1.0).abs() < 1e-3);
        // Locked from a fifth of each half beat until the next hit.
        assert_eq!(snapped(0.12, 2.0), snapped(0.45, 2.0));
        assert!(snapped(0.55, 2.0) > snapped(0.45, 2.0));
    }

    #[test]
    fn the_jets_fire_only_in_a_jump() {
        let pose = |mv: Move, phase: f32| Pose {
            mv,
            phase,
            energy: 1.0,
            ..Pose::default()
        };
        assert!(super::super::draw::jet_power(&pose(Move::Jump, 0.62)) > 0.5);
        assert_eq!(super::super::draw::jet_power(&pose(Move::Jump, 0.1)), 0.0);
        for mv in Move::ALL.into_iter().filter(|m| *m != Move::Jump) {
            assert_eq!(
                super::super::draw::jet_power(&pose(mv, 0.62)),
                0.0,
                "{mv:?}"
            );
        }
    }
}
