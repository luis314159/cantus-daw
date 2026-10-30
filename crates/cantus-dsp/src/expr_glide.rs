//! How a voice follows its **per-note expression** (round 33, ADR 0080): a bend, a pressure
//! or a timbre arrives as a stream of values (an MPE controller's every millisecond or so, a
//! clip's curve on a 1/96-beat grid), and a voice that jumped to each would step — a
//! zipper on a slow bend, clicks on a pressure swell. Each value is a *target*; the voice
//! moves its own value toward it at every control tick with a one-pole of
//! [`TAU_S`], fast enough to keep a vibrato's shape, slow enough to join the steps.
//!
//! **Exact where nothing moves.** A value that is at its target is never touched, so a voice
//! that hears no expression renders exactly as before (the null rule), and the first value
//! a note receives — before its first sample — is taken at once ([`set`]'s `fresh`), so a
//! note that starts bent starts in tune with its bend rather than gliding into it.

use crate::math;

/// The smoothing's time constant, seconds.
pub const TAU_S: f32 = 0.004;

/// The share of the distance to cover after `elapsed` samples at `sr` (0 when none passed).
#[inline]
pub fn coef(elapsed: f32, sr: f32) -> f32 {
    if elapsed <= 0.0 {
        return 0.0;
    }
    1.0 - math::exp(-elapsed / (sr * TAU_S).max(1.0))
}

/// Move `cur` toward `to` by the share `k`; within a millionth it lands exactly.
#[inline]
pub fn toward(cur: &mut f32, to: f32, k: f32) {
    if *cur == to {
        return;
    }
    let d = to - *cur;
    if d.abs() <= 1e-6 || k >= 1.0 {
        *cur = to;
    } else {
        *cur += d * k;
    }
}

/// A new target: taken at once by a voice that has not rendered yet (`fresh`).
#[inline]
pub fn set(cur: &mut f32, to: &mut f32, value: f32, fresh: bool) {
    *to = value;
    if fresh {
        *cur = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_its_target_nothing_moves_and_a_step_is_joined() {
        let mut x = 0.0f32;
        toward(&mut x, 0.0, coef(16.0, 48_000.0));
        assert_eq!(x.to_bits(), 0.0f32.to_bits());
        let k = coef(16.0, 48_000.0);
        let mut steps = 0;
        while x != 2.0 {
            let before = x;
            toward(&mut x, 2.0, k);
            assert!(x > before && x <= 2.0);
            steps += 1;
            assert!(steps < 10_000);
        }
        // Most of the way in about 4 τ: a step is a glide of a few milliseconds.
        let tau_ticks = (TAU_S * 48_000.0 / 16.0) as usize;
        assert!(steps > tau_ticks, "{steps}");
        let (mut c, mut t) = (0.0, 0.0);
        set(&mut c, &mut t, 1.5, true);
        assert_eq!((c, t), (1.5, 1.5));
        set(&mut c, &mut t, 0.5, false);
        assert_eq!((c, t), (1.5, 0.5));
    }
}
