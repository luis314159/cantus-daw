//! A transient detector on **level differences, not a threshold** (ADR 0056), built on the
//! family's follower (`dynfx_follow`): Impetus's and Discrimen's ears.
//!
//! The rectified level first goes through a **running maximum** over [`HOLD_MS`] — two
//! half-cycles of a 50 Hz tone — so a steady tone is a steady level: no ripple for the
//! envelopes to read as attacks (and no gain wobbling at the waveform's rate). An onset
//! still rises at once (a maximum takes a new peak immediately). Then two pairs of
//! envelopes over it:
//! - the **attack pair** shares a release and differs in attack: a fast one ([`FAST_MS`])
//!   and a slow one (the *speed*). At an onset the fast one leaps and the slow one lags:
//!   their ratio, in dB, is how much of a transient there is now.
//! - the **sustain pair** shares the fast attack and differs in release: a short one and a
//!   long one (the *length*). As a note decays the long one stays above: their ratio is
//!   how much of the sound is its tail.
//!
//! Both are ratios, so the reading is the same at any level (to the bit when the level
//! changes by a power of two: the maximum and the followers are homogeneous) — a quiet hit
//! and a loud one are shaped alike, and nothing has to be set to the material's level.
//!
//! [`Svf`] is the family's band split: a Butterworth state-variable filter whose low- and
//! high-pass are 90° apart at the crossover, so a band boosted or cut never cancels
//! against the rest.

use crate::dynfx_follow::{Follower, Quiet, Times, SETTLE};

/// The fast attack of both pairs, ms.
pub const FAST_MS: f32 = 0.3;
/// The running maximum's window, ms.
pub const HOLD_MS: f32 = 10.0;
/// The largest difference read, dB.
pub const MAX_DB: f32 = 24.0;
/// The running maximum's capacity (10 ms at 384 kHz).
const CAP: usize = 4_096;

/// The envelopes' times and the hold's length.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DiffTimes {
    att_fast: Times,
    att_slow: Times,
    rel_fast: Times,
    rel_slow: Times,
    hold: u32,
}

impl DiffTimes {
    /// `speed_ms`: the slow attack (how long a transient lasts); `length_ms`: the long
    /// release (how long a tail is heard as one).
    pub fn new(speed_ms: f32, length_ms: f32, sample_rate: f32) -> DiffTimes {
        let speed = speed_ms.max(FAST_MS * 2.0);
        let length = length_ms.max(10.0);
        // The attack pair falls together, a little slower than the transient lasts.
        let att_rel = (speed * 4.0).max(20.0);
        DiffTimes {
            att_fast: Times::new(FAST_MS, att_rel, sample_rate),
            att_slow: Times::new(speed, att_rel, sample_rate),
            rel_fast: Times::new(FAST_MS, (length / 8.0).max(5.0), sample_rate),
            rel_slow: Times::new(FAST_MS, length, sample_rate),
            hold: ((HOLD_MS * 0.001 * sample_rate) as u32).clamp(1, CAP as u32 - 1),
        }
    }
}

/// `a` over `b` in dB, clamped to `0 … MAX_DB`.
#[inline]
fn ratio_db(a: f32, b: f32) -> f32 {
    if a <= b || a <= 0.0 {
        return 0.0;
    }
    if b <= a * 1e-3 {
        // Below −60 dB under it (or silence before an onset): the most there is.
        return MAX_DB;
    }
    (20.0 * crate::math::log10(a / b)).min(MAX_DB)
}

/// The largest of the last `hold` samples: a monotonic queue in a fixed ring.
#[derive(Clone, Debug)]
pub(crate) struct RunMax {
    val: Box<[f32]>,
    at: Box<[u32]>,
    head: usize,
    len: usize,
    t: u32,
}

impl Default for RunMax {
    fn default() -> Self {
        RunMax {
            val: vec![0.0; CAP].into_boxed_slice(),
            at: vec![0; CAP].into_boxed_slice(),
            head: 0,
            len: 0,
            t: 0,
        }
    }
}

impl RunMax {
    #[inline]
    pub(crate) fn push(&mut self, v: f32, hold: u32) -> f32 {
        self.t = self.t.wrapping_add(1);
        // Drop what is no larger than the new value from the back, what is too old from
        // the front.
        while self.len > 0 {
            let back = (self.head + self.len - 1) % CAP;
            if self.val[back] <= v {
                self.len -= 1;
            } else {
                break;
            }
        }
        let i = (self.head + self.len) % CAP;
        self.val[i] = v;
        self.at[i] = self.t;
        self.len += 1;
        while self.len > 1 && self.t.wrapping_sub(self.at[self.head]) >= hold {
            self.head = (self.head + 1) % CAP;
            self.len -= 1;
        }
        self.val[self.head]
    }

    /// What it holds can never come out again above zero.
    pub(crate) fn is_zero(&self) -> bool {
        self.len == 0 || self.val[self.head].to_bits() == 0
    }

    pub(crate) fn reset(&mut self) {
        self.head = 0;
        self.len = 0;
        self.t = 0;
    }
}

/// The detector's state.
#[derive(Clone, Debug, Default)]
pub struct Diff {
    max: RunMax,
    att_fast: Follower,
    att_slow: Follower,
    rel_fast: Follower,
    rel_slow: Follower,
    quiet: Quiet,
}

impl Diff {
    /// One sample of a rectified level (`|x|`, the louder channel): `(attack, sustain)` in
    /// dB, each ≥ 0.
    #[inline]
    pub fn run(&mut self, level: f32, t: &DiffTimes) -> (f32, f32) {
        // Not a number: silence (the running maximum would hold it for its whole window).
        let level = if level.is_finite() { level } else { 0.0 };
        let m = self.max.push(level, t.hold);
        let af = self.att_fast.run(m, &t.att_fast);
        let asl = self.att_slow.run(m, &t.att_slow);
        let rf = self.rel_fast.run(m, &t.rel_fast);
        let rs = self.rel_slow.run(m, &t.rel_slow);
        let out = (ratio_db(af, asl), ratio_db(rs, rf));
        if self.quiet.note(level) {
            for f in [
                &mut self.att_fast,
                &mut self.att_slow,
                &mut self.rel_fast,
                &mut self.rel_slow,
            ] {
                f.settle(SETTLE);
            }
        }
        out
    }

    /// Every envelope is +0.0 and the hold holds nothing: as new.
    pub fn is_zero(&self) -> bool {
        self.max.is_zero()
            && self.att_fast.is_zero()
            && self.att_slow.is_zero()
            && self.rel_fast.is_zero()
            && self.rel_slow.is_zero()
    }

    pub fn reset(&mut self) {
        self.max.reset();
        for f in [
            &mut self.att_fast,
            &mut self.att_slow,
            &mut self.rel_fast,
            &mut self.rel_slow,
        ] {
            f.reset();
        }
        self.quiet.reset();
    }

    /// `frames` of silence skipped at rest.
    pub fn skip(&mut self, frames: usize) {
        self.quiet.skip(frames);
    }
}

/// A Butterworth state-variable filter (Simper's trapezoidal form): low- and high-pass of
/// one crossover, `lp + bp·√2 + hp = x`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Svf {
    s1: f32,
    s2: f32,
}

/// An [`Svf`]'s coefficients at a crossover.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SvfCoef {
    a1: f32,
    a2: f32,
    a3: f32,
}

impl SvfCoef {
    pub fn new(hz: f32, sample_rate: f32) -> SvfCoef {
        let g = crate::math::tan(std::f32::consts::PI * (hz / sample_rate).clamp(1e-6, 0.49));
        let k = std::f32::consts::SQRT_2;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        SvfCoef { a1, a2, a3: g * a2 }
    }
}

impl Svf {
    /// One sample: `(low-pass, high-pass)`.
    #[inline]
    pub fn run(&mut self, x: f32, c: &SvfCoef) -> (f32, f32) {
        let v3 = x - self.s2;
        let v1 = c.a1 * self.s1 + c.a2 * v3;
        let v2 = self.s2 + c.a2 * self.s1 + c.a3 * v3;
        self.s1 = 2.0 * v1 - self.s1;
        self.s2 = 2.0 * v2 - self.s2;
        (v2, x - std::f32::consts::SQRT_2 * v1 - v2)
    }

    pub fn is_zero(&self) -> bool {
        self.s1.to_bits() == 0 && self.s2.to_bits() == 0
    }

    /// Flush states that are only a denormal's worth away from zero.
    #[inline]
    pub fn settle(&mut self, below: f32) {
        if self.s1.abs() < below && self.s2.abs() < below {
            self.s1 = 0.0;
            self.s2 = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// A hit: an instant rise and an exponential fall over a tone.
    fn hit(n: usize, amp: f32) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let env = crate::math::exp(-(i as f32) / 4_800.0);
                amp * env * crate::math::sin(i as f32 * 0.05)
            })
            .collect()
    }

    #[test]
    fn an_onset_reads_as_attack_and_a_tail_as_sustain() {
        let t = DiffTimes::new(15.0, 300.0, SR);
        let mut d = Diff::default();
        let x = hit(24_000, 0.8);
        let mut first = (0f32, 0f32);
        let mut late = (0f32, 0f32);
        for (i, v) in x.iter().enumerate() {
            let r = d.run(v.abs(), &t);
            if i < 480 {
                first.0 = first.0.max(r.0);
            }
            if (9_600..14_400).contains(&i) {
                late.0 = late.0.max(r.0);
                late.1 = late.1.max(r.1);
            }
        }
        assert!(first.0 > 12.0, "the onset: {first:?}");
        assert!(late.0 < 3.0, "no attack in the tail: {late:?}");
        assert!(late.1 > 3.0, "the tail is sustain: {late:?}");
    }

    /// A steady tone, even a low one, reads as neither: no ripple left to read.
    #[test]
    fn a_steady_tone_is_neither_attack_nor_tail() {
        let t = DiffTimes::new(15.0, 300.0, SR);
        for hz in [55.0f32, 440.0, 3_000.0] {
            let mut d = Diff::default();
            let mut worst = (0f32, 0f32);
            for i in 0..96_000 {
                let v = 0.5 * crate::math::sin(std::f32::consts::TAU * hz * i as f32 / SR);
                let r = d.run(v.abs(), &t);
                if i > 48_000 {
                    worst = (worst.0.max(r.0), worst.1.max(r.1));
                }
            }
            assert!(worst.0 < 0.05 && worst.1 < 0.05, "{hz} Hz: {worst:?}");
        }
    }

    /// The same hit 12 dB quieter (a quarter, a power of two) reads the same, to the bit.
    #[test]
    fn the_reading_does_not_depend_on_the_level() {
        let t = DiffTimes::new(10.0, 200.0, SR);
        let (mut a, mut b) = (Diff::default(), Diff::default());
        let x = hit(20_000, 0.9);
        for v in &x {
            let ra = a.run(v.abs(), &t);
            let rb = b.run((v * 0.25).abs(), &t);
            assert_eq!(ra.0.to_bits(), rb.0.to_bits());
            assert_eq!(ra.1.to_bits(), rb.1.to_bits());
        }
    }

    /// Review 1: a sample that is not a number reads as silence — it never locks the
    /// envelopes (a NaN in a follower stays NaN for good).
    #[test]
    fn a_non_finite_level_locks_nothing() {
        let t = DiffTimes::new(10.0, 200.0, SR);
        let mut d = Diff::default();
        for v in hit(2_000, 0.8) {
            d.run(v.abs(), &t);
        }
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let (a, s) = d.run(bad, &t);
            assert!(a.is_finite() && s.is_finite());
        }
        let mut steady = (0f32, 0f32);
        for i in 0..48_000 {
            let v = 0.5 * crate::math::sin(std::f32::consts::TAU * 440.0 * i as f32 / SR);
            steady = d.run(v.abs(), &t);
        }
        assert!(steady.0 < 0.05 && steady.1 < 0.05, "{steady:?}");
        let mut n = 0;
        while !d.is_zero() {
            d.run(0.0, &t);
            n += 1;
            assert!(n < 30 * 48_000, "no rest after a NaN");
        }
    }

    #[test]
    fn after_silence_it_is_as_new() {
        let t = DiffTimes::new(10.0, 1_000.0, SR);
        let mut d = Diff::default();
        for v in hit(4_800, 1.0) {
            d.run(v.abs(), &t);
        }
        let mut n = 0;
        while !d.is_zero() {
            d.run(0.0, &t);
            n += 1;
            assert!(n < 30 * 48_000);
        }
        let mut fresh = Diff::default();
        for v in hit(3_000, 0.7) {
            let a = d.run(v.abs(), &t);
            let b = fresh.run(v.abs(), &t);
            assert_eq!(
                (a.0.to_bits(), a.1.to_bits()),
                (b.0.to_bits(), b.1.to_bits())
            );
        }
    }

    #[test]
    fn the_split_is_twelve_db_an_octave_each_way() {
        let c = SvfCoef::new(800.0, SR);
        let peak = |hz: f32, high: bool| {
            let mut f = Svf::default();
            let mut m = 0f32;
            for i in 0..48_000 {
                let x = crate::math::sin(std::f32::consts::TAU * hz * i as f32 / SR);
                let (lp, hp) = f.run(x, &c);
                if i > 24_000 {
                    m = m.max(if high { hp } else { lp }.abs());
                }
            }
            20.0 * crate::math::log10(m)
        };
        assert!(peak(55.0, true) < -22.0, "{}", peak(55.0, true));
        assert!(peak(8_000.0, false) < -38.0, "{}", peak(8_000.0, false));
        assert!((peak(800.0, true) + 3.0).abs() < 0.2);
        assert!((peak(800.0, false) + 3.0).abs() < 0.2);
    }
}
