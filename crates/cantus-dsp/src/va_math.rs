//! The arithmetic every virtual-analog block shares: our own `tanh`, `tan`, `exp2` and
//! `sin`, and a counter-based hash for seeded randomness.
//!
//! **Why our own.** `f32::tan`, `exp2` and `powf` come from the platform's libm and are
//! not guaranteed to round the same way on every operating system; a render meant to null
//! across machines cannot depend on them (research §4.8). These are rational or polynomial
//! approximations evaluated in a fixed order, with no fused multiply-add, so they give the
//! same bits everywhere — and they are faster than libm on the audio thread, where the
//! filters call them every sample.
//!
//! The hash is a counter-based generator: a value is a pure function of its inputs (a
//! seed, a card, an index), so drift and noise are the same whatever the block size, and
//! whatever was played before.

use std::f32::consts::{LN_2, PI};

/// `tanh(x)`, from its [5/4] Padé approximant, clamped to ±1. Error below 5·10⁻⁴ up to the
/// point where the rational reaches 1 (|x| ≈ 3.65) and then saturates.
#[inline]
pub fn tanh(x: f32) -> f32 {
    let x2 = x * x;
    if x2 >= TANH_EDGE2 {
        return if x > 0.0 { 1.0 } else { -1.0 };
    }
    x * pade_tanh_ratio(x2)
}

/// `tanh(x) / x` — the small-signal gain a saturator gives a signal sitting at `x`: 1 at 0,
/// falling towards `1/|x|`. The "cheap zero-delay" filters linearise each nonlinearity
/// with it (a saturator becomes a gain for one sample).
#[inline]
pub fn tanh_gain(x: f32) -> f32 {
    let x2 = x * x;
    if x2 >= TANH_EDGE2 {
        return 1.0 / x.abs();
    }
    pade_tanh_ratio(x2)
}

/// Where the [5/4] Padé of tanh crosses 1: beyond it the saturator is flat.
const TANH_EDGE2: f32 = 3.65 * 3.65;

#[inline]
fn pade_tanh_ratio(x2: f32) -> f32 {
    (945.0 + 105.0 * x2 + x2 * x2) / (945.0 + 420.0 * x2 + 15.0 * x2 * x2)
}

/// `tan(π·r)` for `r` in `[0, 0.5)`: the pre-warped gain of a trapezoidal integrator whose
/// cutoff is `r` times the sample rate. The [5/4] Padé of `tan` on `[0, π/4]`, and
/// `1 / tan(π/2 − θ)` above; relative error below 10⁻⁶ over the whole range.
#[inline]
pub fn tan_pi(r: f32) -> f32 {
    let r = r.clamp(0.0, 0.4999);
    if r <= 0.25 {
        pade_tan(PI * r)
    } else {
        1.0 / pade_tan(PI * (0.5 - r))
    }
}

#[inline]
fn pade_tan(x: f32) -> f32 {
    let x2 = x * x;
    x * (945.0 - 105.0 * x2 + x2 * x2) / (945.0 - 420.0 * x2 + 15.0 * x2 * x2)
}

/// `floor(x)` for `|x| < 2³¹`, without a library call (the platform's `floorf` and
/// `roundf` cost more than the rest of a filter stage on some targets).
#[inline]
pub fn floor(x: f32) -> f32 {
    let t = x as i32 as f32;
    if t > x {
        t - 1.0
    } else {
        t
    }
}

/// The nearest integer, halves away from zero (as `f32::round`), for `|x| < 2³¹`.
#[inline]
pub fn round_i32(x: f32) -> i32 {
    if x >= 0.0 {
        (x + 0.5) as i32
    } else {
        (x - 0.5) as i32
    }
}

/// `2^x`: the exponent from the bits, the fraction from a degree-6 Taylor series on
/// `[−½, ½]` (relative error ≈ 10⁻⁷, a ten-thousandth of a cent in a frequency).
#[inline]
pub fn exp2(x: f32) -> f32 {
    let x = x.clamp(-126.0, 126.0);
    let xi = round_i32(x);
    let t = (x - xi as f32) * LN_2;
    let p = 1.0
        + t * (1.0
            + t * (0.5
                + t * (1.0 / 6.0 + t * (1.0 / 24.0 + t * (1.0 / 120.0 + t * (1.0 / 720.0))))));
    p * f32::from_bits(((xi + 127) as u32) << 23)
}

/// `e^x`, through [`exp2`].
#[inline]
pub fn exp(x: f32) -> f32 {
    exp2(x * std::f32::consts::LOG2_E)
}

/// `sin(2π·p)` for any `p` (in turns): folded to a quarter wave and a degree-11 Taylor
/// series; error below 10⁻⁷.
#[inline]
pub fn sin_turns(p: f32) -> f32 {
    let mut x = p - floor(p); // 0..1
    if x >= 0.5 {
        x -= 1.0; // −0.5..0.5
    }
    if x > 0.25 {
        x = 0.5 - x;
    } else if x < -0.25 {
        x = -0.5 - x;
    }
    let t = 2.0 * PI * x;
    let t2 = t * t;
    t * (1.0
        - t2 / 6.0 * (1.0 - t2 / 20.0 * (1.0 - t2 / 42.0 * (1.0 - t2 / 72.0 * (1.0 - t2 / 110.0)))))
}

/// A note's frequency from its (fractional) MIDI number, A4 = 440 Hz.
#[inline]
pub fn midi_hz(note: f32) -> f32 {
    440.0 * exp2((note - 69.0) / 12.0)
}

/// Decibels to a linear gain.
#[inline]
pub fn db_gain(db: f32) -> f32 {
    exp2(db * (1.0 / 6.020_6))
}

// ---- seeded randomness ---------------------------------------------------------------------

/// The splitmix64 finaliser: a bijective mix of 64 bits.
#[inline]
pub fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A hash of three words: the counter-based generator's one primitive.
#[inline]
pub fn hash3(a: u64, b: u64, c: u64) -> u64 {
    mix64(a ^ mix64(b.wrapping_add(0x9e37_79b9_7f4a_7c15) ^ mix64(c)))
}

/// A hash as a uniform number in `[−1, 1)`.
#[inline]
pub fn bipolar(h: u64) -> f32 {
    ((h >> 40) as f32) * (1.0 / (1u64 << 23) as f32) - 1.0
}

/// A hash as a uniform number in `[0, 1)`.
#[inline]
pub fn unipolar(h: u64) -> f32 {
    ((h >> 40) as f32) * (1.0 / (1u64 << 24) as f32)
}

/// A cheap sequential generator for audio-rate noise (xorshift32), seeded from a hash so
/// every voice's noise is its own and repeatable.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u64) -> Self {
        // xorshift must never hold zero.
        Noise((mix64(seed) as u32) | 1)
    }

    /// White noise in `[−1, 1)`.
    #[inline]
    pub fn next(&mut self) -> f32 {
        let mut s = self.0;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.0 = s;
        (s >> 8) as f32 * (1.0 / (1u32 << 23) as f32) - 1.0
    }
}

/// Flush a value to zero once it is below the level anything can hear (and before it can
/// become a denormal and slow the CPU down).
#[inline]
pub fn flush(x: f32) -> f32 {
    if x.abs() < 1e-15 {
        0.0
    } else {
        x
    }
}
