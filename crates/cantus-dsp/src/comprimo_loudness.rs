//! Loudness for Comprimo, after ITU-R BS.1770-4 and EBU R128: K-weighting, momentary
//! (400 ms) and short-term (3 s) loudness on a fixed 100 ms grid, and the gated integrated
//! loudness over a list of 400 ms blocks.
//!
//! The grid is counted in samples from the start of the stream, never from the start of a
//! block, so every reading is independent of the block size — the same contract as the rest
//! of the compressor.
//!
//! The K-weighting coefficients are computed for any sample rate from the filters' analog
//! descriptions (the pre-filter shelf and the RLB high-pass), the way libebur128 does; at
//! 48 kHz they reproduce the coefficients printed in the standard. A test holds the
//! standard's own calibration: a 997 Hz sine at 0 dBFS in one channel reads −3.01 LKFS.

use crate::math;
use std::f64::consts::PI;

/// Loudness below anything measurable: what silence reads.
pub const SILENCE: f32 = -200.0;
/// The grid, seconds: one step every 100 ms (a 400 ms block overlapped by 75 %).
pub const HOP_S: f64 = 0.1;
/// Steps in a momentary (400 ms) and a short-term (3 s) window.
pub const MOMENTARY_HOPS: usize = 4;
pub const SHORT_HOPS: usize = 30;
/// BS.1770's absolute gate, LUFS.
pub const ABSOLUTE_GATE: f32 = -70.0;
/// BS.1770's relative gate, LU below the ungated mean.
pub const RELATIVE_GATE: f32 = -10.0;

/// Mean square (summed over channels, each weighted 1) to LUFS.
pub fn lufs(power: f64) -> f32 {
    if power <= 1e-20 {
        SILENCE
    } else {
        (-0.691 + 10.0 * math::log10(power)) as f32
    }
}

/// LUFS back to mean square.
pub fn power(lufs: f32) -> f64 {
    if lufs <= SILENCE {
        0.0
    } else {
        math::powf(10f64, (lufs as f64 + 0.691) / 10.0)
    }
}

/// Integrated loudness of a programme from its 400 ms block loudnesses (one per 100 ms
/// step): the absolute gate at −70 LUFS, then the relative gate 10 LU under the mean of
/// what passed. [`SILENCE`] when nothing passes.
pub fn integrated(blocks: &[f32]) -> f32 {
    let gated_mean = |gate: f32| -> Option<f64> {
        let (mut sum, mut n) = (0.0f64, 0usize);
        for &b in blocks {
            if b > gate {
                sum += power(b);
                n += 1;
            }
        }
        (n > 0).then(|| sum / n as f64)
    };
    let Some(abs) = gated_mean(ABSOLUTE_GATE) else {
        return SILENCE;
    };
    let relative = (lufs(abs) + RELATIVE_GATE).max(ABSOLUTE_GATE);
    gated_mean(relative).map(lufs).unwrap_or(SILENCE)
}

#[derive(Copy, Clone, Debug, Default)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    #[inline]
    fn tick(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        // A long silence decays the state toward the denormals; flush it well before.
        if self.z1.abs() < 1e-30 {
            self.z1 = 0.0;
        }
        if self.z2.abs() < 1e-30 {
            self.z2 = 0.0;
        }
        y
    }

    fn clear(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// The K-weighting of BS.1770 for two channels: a high shelf (the head) then the RLB
/// high-pass.
#[derive(Copy, Clone, Debug)]
pub struct KWeighting {
    stages: [[Biquad; 2]; 2],
}

impl KWeighting {
    pub fn new(sample_rate: f32) -> Self {
        let fs = sample_rate.max(1.0) as f64;
        // The pre-filter: a second-order high shelf, about +4 dB above 1.7 kHz.
        let f0 = 1_681.974_450_955_533;
        let g = 3.999_843_853_973_347;
        let q = 0.707_175_236_955_419_6;
        let k = math::tan(PI * f0 / fs);
        let vh = math::powf(10f64, g / 20.0);
        let vb = math::powf(vh, 0.499_666_774_154_541_6);
        let a0 = 1.0 + k / q + k * k;
        let shelf = Biquad {
            b0: (vh + vb * k / q + k * k) / a0,
            b1: 2.0 * (k * k - vh) / a0,
            b2: (vh - vb * k / q + k * k) / a0,
            a1: 2.0 * (k * k - 1.0) / a0,
            a2: (1.0 - k / q + k * k) / a0,
            ..Biquad::default()
        };
        // The RLB weighting: a second-order high-pass at 38 Hz.
        let f0 = 38.135_470_876_024_44;
        let q = 0.500_327_037_323_877_3;
        let k = math::tan(PI * f0 / fs);
        let a0 = 1.0 + k / q + k * k;
        let rlb = Biquad {
            b0: 1.0,
            b1: -2.0,
            b2: 1.0,
            a1: 2.0 * (k * k - 1.0) / a0,
            a2: (1.0 - k / q + k * k) / a0,
            ..Biquad::default()
        };
        KWeighting {
            stages: [[shelf, rlb]; 2],
        }
    }

    /// One stereo sample in; the sum of the two channels' weighted squares out.
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32) -> f64 {
        let mut e = 0.0;
        for (ch, x) in [l, r].into_iter().enumerate() {
            let [a, b] = &mut self.stages[ch];
            let y = b.tick(a.tick(x as f64));
            e += y * y;
        }
        e
    }

    pub fn reset(&mut self) {
        for ch in &mut self.stages {
            for s in ch {
                s.clear();
            }
        }
    }
}

/// A loudness meter on the 100 ms grid: feed it samples, close each step with
/// [`Meter::end_hop`], read momentary and short-term loudness between steps.
#[derive(Copy, Clone, Debug)]
pub struct Meter {
    k: KWeighting,
    acc: f64,
    /// Mean square of each of the last [`SHORT_HOPS`] steps, a ring.
    ring: [f64; SHORT_HOPS],
    pos: usize,
}

impl Meter {
    pub fn new(sample_rate: f32) -> Self {
        Meter {
            k: KWeighting::new(sample_rate),
            acc: 0.0,
            ring: [0.0; SHORT_HOPS],
            pos: 0,
        }
    }

    #[inline]
    pub fn add(&mut self, l: f32, r: f32) {
        self.acc += self.k.tick(l, r);
    }

    /// Close a step of `len` samples.
    pub fn end_hop(&mut self, len: usize) {
        self.ring[self.pos] = self.acc / len.max(1) as f64;
        self.pos = (self.pos + 1) % SHORT_HOPS;
        self.acc = 0.0;
    }

    fn mean_of_last(&self, n: usize) -> f64 {
        let mut sum = 0.0;
        for i in 0..n {
            sum += self.ring[(self.pos + SHORT_HOPS - 1 - i) % SHORT_HOPS];
        }
        sum / n as f64
    }

    /// The last 400 ms, LUFS.
    pub fn momentary(&self) -> f32 {
        lufs(self.mean_of_last(MOMENTARY_HOPS))
    }

    /// The last 3 s, LUFS.
    pub fn short_term(&self) -> f32 {
        lufs(self.mean_of_last(SHORT_HOPS))
    }

    pub fn reset(&mut self) {
        self.k.reset();
        self.acc = 0.0;
        self.ring = [0.0; SHORT_HOPS];
        self.pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, amp: f32, sr: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| amp * math::sin(std::f32::consts::TAU * hz * i as f32 / sr))
            .collect()
    }

    fn measure(sr: f32, l: &[f32], r: &[f32]) -> Meter {
        let mut m = Meter::new(sr);
        let hop = (sr as f64 * HOP_S).round() as usize;
        for (i, (a, b)) in l.iter().zip(r).enumerate() {
            m.add(*a, *b);
            if (i + 1) % hop == 0 {
                m.end_hop(hop);
            }
        }
        m
    }

    /// The standard's calibration: 997 Hz at 0 dBFS in one channel is −3.01 LKFS; in both
    /// channels, 0.0. Held at the three common rates.
    #[test]
    fn a_full_scale_sine_reads_what_the_standard_says() {
        for sr in [44_100.0f32, 48_000.0, 96_000.0] {
            let n = (sr * 4.0) as usize;
            let s = sine(997.0, 1.0, sr, n);
            let silent = vec![0.0; n];
            let one = measure(sr, &s, &silent).short_term();
            assert!((one - -3.01).abs() < 0.05, "one channel at {sr}: {one}");
            let both = measure(sr, &s, &s).short_term();
            assert!(both.abs() < 0.05, "two channels at {sr}: {both}");
            let quiet = sine(997.0, 0.1, sr, n);
            let m = measure(sr, &quiet, &quiet);
            assert!((m.short_term() - -20.0).abs() < 0.05);
            assert!((m.momentary() - -20.0).abs() < 0.05);
        }
    }

    /// The K-weighting at 48 kHz is the one printed in BS.1770-4.
    #[test]
    fn the_filters_at_48k_are_the_printed_ones() {
        let k = KWeighting::new(48_000.0);
        let [shelf, rlb] = k.stages[0];
        let close = |a: f64, b: f64| (a - b).abs() < 1e-6;
        assert!(close(shelf.b0, 1.535_124_859_586_97));
        assert!(close(shelf.b1, -2.691_696_189_406_38));
        assert!(close(shelf.b2, 1.198_392_810_852_85));
        assert!(close(shelf.a1, -1.690_659_293_182_41));
        assert!(close(shelf.a2, 0.732_480_774_215_85));
        assert!(close(rlb.a1, -1.990_047_454_833_98));
        assert!(close(rlb.a2, 0.990_072_250_366_21));
    }

    #[test]
    fn integrated_loudness_gates_silence_and_quiet_passages() {
        // Ten seconds at −20, then ten of silence: the silence is gated out.
        let mut blocks = vec![-20.0f32; 100];
        blocks.extend(std::iter::repeat_n(SILENCE, 100));
        assert!((integrated(&blocks) - -20.0).abs() < 1e-3);
        // A passage 20 LU under the rest falls under the relative gate.
        let mut blocks = vec![-20.0f32; 100];
        blocks.extend(std::iter::repeat_n(-40.0, 100));
        assert!((integrated(&blocks) - -20.0).abs() < 1e-3);
        // Two passages 6 LU apart both count: the mean of their powers.
        let mut blocks = vec![-20.0f32; 100];
        blocks.extend(std::iter::repeat_n(-26.0, 100));
        let want = lufs((power(-20.0) + power(-26.0)) / 2.0);
        assert!((integrated(&blocks) - want).abs() < 1e-3);
        assert_eq!(integrated(&[]), SILENCE);
        assert_eq!(integrated(&[SILENCE, -80.0]), SILENCE);
    }
}
