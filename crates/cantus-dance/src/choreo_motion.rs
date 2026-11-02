//! How the body moves through the stage (ADR 0087 §2): the choreography's second half —
//! travel, jumps, a pirouette's facing, energy and the beat's accent. Pure functions of
//! the song position and the dancer's numbers, each tested on its own.

use cantus_core::inst::dancer::Move;
use cantus_dsp::math;

/// How far one step travels across the stage, as a share of its width.
pub const STEP_TRAVEL: f32 = 0.04;
/// The stage's edges a walk turns around at (its feet), and a wander's reach either side.
pub const WALK_LO: f32 = 0.08;
pub const WALK_HI: f32 = 0.92;
pub const WANDER: f32 = 0.15;
/// A Jump move's height at energy 0 and what full energy adds (shares of its height).
pub const JUMP_BASE: f32 = 0.12;
pub const JUMP_ENERGY: f32 = 0.30;
/// The share of a Jump's cycle spent crouching before take-off; it lands on the beat.
pub const JUMP_CROUCH: f32 = 0.25;

/// 0…1 eased in and out (smoothstep).
pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Steps taken by `t` beats: one a beat, the foot moving in the second half of the beat
/// and landing **on** it.
pub fn stepped(t: f64) -> f64 {
    let whole = t.floor();
    whole + f64::from(smooth(((t - whole) as f32 - 0.5) * 2.0))
}

/// A walk folded between two edges: where it is and which way it goes as `u` grows (−1…1,
/// through 0 while it turns round at an edge, over `ease` of travel either side of it).
pub fn pingpong(u: f32, lo: f32, hi: f32, ease: f32) -> (f32, f32) {
    let w = (hi - lo).max(1e-4);
    let t = (u - lo).rem_euclid(2.0 * w);
    let (x, dir) = if t < w {
        (lo + t, 1.0)
    } else {
        (hi - (t - w), -1.0)
    };
    let to_turn = t.min((t - w).abs()).min(2.0 * w - t);
    (x, dir * smooth(to_turn / ease.max(1e-4)))
}

/// Where it stands and which way travel turns it: `step` 0 in place, 1 walking left,
/// 2 walking right (turning round at the stage's edges), 3 wandering about `x`. Counted
/// from the song's start at its pace, so stopping and starting never jump.
pub fn travel(step: i32, x: f32, beats: f64, speed: f32) -> (f32, f32) {
    let steps = stepped(beats * f64::from(speed)) as f32 * STEP_TRAVEL;
    let ease = STEP_TRAVEL * 0.5;
    match step {
        1 => {
            let (x, dir) = pingpong(x - steps, WALK_LO, WALK_HI, ease);
            (x, -dir)
        }
        2 => pingpong(x + steps, WALK_LO, WALK_HI, ease),
        3 => {
            let lo = (x - WANDER).max(WALK_LO.min(x));
            let hi = (x + WANDER).min(WALK_HI.max(x));
            pingpong(x + steps, lo, hi, ease)
        }
        _ => (x, 0.0),
    }
}

/// Which way it faces: the `turn` knob, or — left at 0 — the way it walks (a
/// three-quarter turn).
pub fn facing(turn: f32, walking: f32) -> f32 {
    if turn.abs() > 1e-3 {
        turn.clamp(-1.0, 1.0)
    } else {
        (walking * 0.8).clamp(-1.0, 1.0)
    }
}

/// A move's own lift off the ground at a phase: a Jump crouches, springs and lands on the
/// next beat; every other move keeps its feet down (its own bounce is the character's).
pub fn arc(mv: Move, phase: f32, energy: f32) -> f32 {
    if mv != Move::Jump || phase < JUMP_CROUCH {
        return 0.0;
    }
    let t = (phase - JUMP_CROUCH) / (1.0 - JUMP_CROUCH);
    math::sin(std::f32::consts::PI * t).max(0.0) * (JUMP_BASE + JUMP_ENERGY * energy)
}

/// A pirouette's facing at a phase: right round through the front and the back, slow out
/// and in. `None` for every other move.
pub fn pirouette(mv: Move, phase: f32) -> Option<f32> {
    (mv == Move::Spin).then(|| math::cos(std::f32::consts::TAU * smooth(phase)))
}

/// How big the moves are: the knob, with the level it hears by `react`, and a note's
/// velocity (a soft note, a smaller move).
pub fn energy(knob: f32, react: f32, level: f32, velocity: f32) -> f32 {
    let react = react.clamp(0.0, 1.0);
    let e = knob.clamp(0.0, 1.0) * (1.0 - react) + react * level.clamp(0.0, 1.0);
    (e * (0.35 + 0.65 * velocity.clamp(0.0, 1.0))).clamp(0.0, 1.0)
}

/// The beat in the bar (0-based) and the bar's beats.
pub fn beat_in_bar(beats: f64, bar_start: f64, bar_beats: f32) -> (u32, u32) {
    let n = bar_beats.round().max(1.0) as u32;
    let b = (beats - bar_start).floor().max(0.0) as u32;
    (b.min(n - 1), n)
}

/// How strong the beat is now, 0…1: the downbeat strongest, the bar's middle next, a pulse
/// that falls away through the beat, bigger with energy. What beat-reactive extras (a
/// sparkle, a note, a puff) read; 0 stopped.
pub fn accent(playing: bool, beat: u32, beats_in_bar: u32, beat_phase: f32, energy: f32) -> f32 {
    if !playing {
        return 0.0;
    }
    let strength = if beat == 0 {
        1.0
    } else if beats_in_bar.is_multiple_of(2) && beat == beats_in_bar / 2 {
        0.7
    } else {
        0.45
    };
    let f = 1.0 - beat_phase.clamp(0.0, 1.0);
    let fall = f * f * f * f;
    strength * fall * (0.3 + 0.7 * energy.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_lands_on_the_beat() {
        for b in 0..8 {
            let t = f64::from(b);
            assert_eq!(stepped(t), t, "on beat {b}");
            assert_eq!(stepped(t + 0.4), t, "still before the step");
            assert!(stepped(t + 0.75) > t && stepped(t + 0.75) < t + 1.0);
        }
    }

    #[test]
    fn a_walk_turns_round_at_the_edges_and_never_leaves_the_stage() {
        let mut last = travel(1, 0.5, 0.0, 1.0);
        let mut turned = false;
        for i in 1..2_000 {
            let b = f64::from(i) * 0.05;
            let (x, dir) = travel(1, 0.5, b, 1.0);
            assert!((WALK_LO..=WALK_HI).contains(&x), "beat {b}: {x}");
            assert!((x - last.0).abs() <= STEP_TRAVEL, "no jump at beat {b}");
            if dir > 0.5 {
                turned = true;
            }
            last = (x, dir);
        }
        assert!(turned, "walking left, it came back right");
        // Left walks left first; right walks right; in place stays.
        assert!(travel(1, 0.5, 4.0, 1.0).0 < 0.5 && travel(1, 0.5, 4.0, 1.0).1 < 0.0);
        assert!(travel(2, 0.5, 4.0, 1.0).0 > 0.5 && travel(2, 0.5, 4.0, 1.0).1 > 0.0);
        assert_eq!(travel(0, 0.3, 9.0, 1.0), (0.3, 0.0));
        // Twice the speed, twice the steps.
        assert!((travel(2, 0.2, 2.0, 2.0).0 - travel(2, 0.2, 4.0, 1.0).0).abs() < 1e-6);
    }

    #[test]
    fn a_wander_stays_near_home() {
        for i in 0..400 {
            let (x, _) = travel(3, 0.4, f64::from(i) * 0.1, 1.0);
            assert!((0.4 - WANDER - 1e-5..=0.4 + WANDER + 1e-5).contains(&x));
        }
    }

    #[test]
    fn facing_follows_the_knob_or_the_walk() {
        assert_eq!(facing(-0.5, 1.0), -0.5);
        assert_eq!(facing(0.0, 1.0), 0.8);
        assert_eq!(facing(0.0, -1.0), -0.8);
        assert_eq!(facing(0.0, 0.0), 0.0);
    }

    #[test]
    fn a_jump_crouches_springs_and_lands_on_the_beat() {
        assert_eq!(arc(Move::Jump, 0.1, 1.0), 0.0, "crouching");
        let top = arc(Move::Jump, JUMP_CROUCH + (1.0 - JUMP_CROUCH) * 0.5, 1.0);
        assert!((top - (JUMP_BASE + JUMP_ENERGY)).abs() < 1e-4);
        assert!(arc(Move::Jump, 0.999, 1.0) < 0.01, "down by the beat");
        assert!(arc(Move::Jump, 0.6, 0.0) < arc(Move::Jump, 0.6, 1.0));
        assert_eq!(arc(Move::Bounce, 0.6, 1.0), 0.0);
    }

    #[test]
    fn a_pirouette_goes_right_round() {
        assert_eq!(pirouette(Move::Clap, 0.3), None);
        assert!((pirouette(Move::Spin, 0.0).unwrap() - 1.0).abs() < 1e-5);
        assert!((pirouette(Move::Spin, 0.5).unwrap() + 1.0).abs() < 1e-5);
        assert!((pirouette(Move::Spin, 0.999).unwrap() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn energy_mixes_the_knob_the_level_and_the_velocity() {
        assert_eq!(energy(0.7, 0.0, 1.0, 1.0), 0.7);
        assert_eq!(
            energy(0.7, 1.0, 0.2, 1.0),
            0.2,
            "react fully: the level decides"
        );
        assert!((energy(0.8, 0.5, 0.0, 1.0) - 0.4).abs() < 1e-6);
        assert!(energy(1.0, 0.0, 0.0, 0.2) < energy(1.0, 0.0, 0.0, 1.0));
        assert_eq!(energy(2.0, -1.0, 5.0, 3.0), 1.0, "clamped");
    }

    #[test]
    fn the_downbeat_is_the_strongest_accent() {
        assert_eq!(beat_in_bar(9.5, 8.0, 4.0), (1, 4));
        assert_eq!(
            beat_in_bar(13.0, 8.0, 4.0),
            (3, 4),
            "never past the bar's last beat"
        );
        let a = |b| accent(true, b, 4, 0.0, 1.0);
        assert!(a(0) > a(2) && a(2) > a(1) && a(1) == a(3));
        assert!(
            accent(true, 0, 4, 0.5, 1.0) < a(0),
            "it falls through the beat"
        );
        assert_eq!(accent(false, 0, 4, 0.0, 1.0), 0.0);
        assert!(accent(true, 0, 4, 0.0, 0.0) < a(0));
    }
}
