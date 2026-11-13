//! Fragor's noise gate, before the pedals: a fuzz turns hiss and string noise into a roar,
//! so the gate closes on what falls below its threshold.
//!
//! The detector is Porta's design reduced to what a pedalboard's gate needs (Porta, ADR
//! 0055, is the full unit): a peak follower on both channels linked, a state machine with
//! **hysteresis** (opens at the threshold, closes 6 dB below it) and a **hold**, and a gain
//! that opens in 1 ms and closes in 60 ms. Fully open the gain is exactly 1.0 (it snaps
//! within 10⁻⁶), closed exactly 0.0, so an open gate is a wire to the bit and a closed one
//! is silence.

use crate::math;

/// The threshold at or below which the gate is off.
pub const OFF_DB: f32 = -90.0;
/// Closes this far below the threshold.
pub const HYSTERESIS_DB: f32 = 6.0;
pub const HOLD_MS: f32 = 30.0;
pub const ATTACK_MS: f32 = 1.0;
pub const RELEASE_MS: f32 = 60.0;
/// The peak follower's fall.
const FALL_MS: f32 = 40.0;

#[derive(Clone, Debug)]
pub struct Gate {
    env: f32,
    open: bool,
    hold: u32,
    gain: f32,
    // Coefficients for the sample rate.
    fall: f32,
    att: f32,
    rel: f32,
    hold_len: u32,
}

impl Gate {
    pub fn new(sample_rate: f32) -> Gate {
        let k = |ms: f32| math::exp(-1.0 / (ms * 0.001 * sample_rate.max(1.0)));
        Gate {
            env: 0.0,
            open: false,
            hold: 0,
            gain: 0.0,
            fall: k(FALL_MS),
            att: k(ATTACK_MS),
            rel: k(RELEASE_MS),
            hold_len: (HOLD_MS * 0.001 * sample_rate) as u32,
        }
    }

    /// As new: closed, its follower empty.
    pub fn reset(&mut self) {
        self.env = 0.0;
        self.open = false;
        self.hold = 0;
        self.gain = 0.0;
    }

    /// As [`Self::reset`] leaves it.
    pub fn is_clear(&self) -> bool {
        crate::rest::z(self.env) && !self.open && self.hold == 0 && crate::rest::z(self.gain)
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The gain now (0 … 1).
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// One sample: the linked peak `level` in, the gain to apply out. `threshold` is linear.
    #[inline]
    pub fn tick(&mut self, level: f32, threshold: f32) -> f32 {
        self.env = if level > self.env {
            level
        } else {
            let e = self.env * self.fall;
            if e < 1e-20 {
                0.0
            } else {
                e
            }
        };
        let close = threshold * math::db_to_gain(-HYSTERESIS_DB);
        if self.env >= threshold {
            self.open = true;
            self.hold = self.hold_len;
        } else if self.open && self.env < close {
            if self.hold > 0 {
                self.hold -= 1;
            } else {
                self.open = false;
            }
        }
        let (target, k) = if self.open {
            (1.0, self.att)
        } else {
            (0.0, self.rel)
        };
        self.gain = target + (self.gain - target) * k;
        if (self.gain - target).abs() < 1e-6 {
            self.gain = target;
        }
        self.gain
    }
}
