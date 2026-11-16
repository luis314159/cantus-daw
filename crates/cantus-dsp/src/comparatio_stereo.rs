//! The stereo comparison: correlation and width, whole and in three bands (the lows under
//! [`LOW_HZ`], the mids, the highs over [`HIGH_HZ`]).
//!
//! Each band is cut by Linkwitz–Riley slopes (two Butterworth sections, 24 dB/oct) in
//! `f64`; for every band the unit sums `L·L`, `R·R` and `L·R`, both over the whole
//! measurement and over the last three seconds (a ring of 100 ms steps). From the sums:
//!
//! - **correlation** `ρ = Σ LR / √(Σ LL · Σ RR)`: +1 mono, 0 unrelated, −1 out of phase;
//! - **width** `W = 2·S² / (M² + S²)` with `M = (L + R)/2` and `S = (L − R)/2`, which is
//!   `1 − 2·Σ LR / (Σ LL + Σ RR)`: 0 mono, 1 two unrelated channels of the same level,
//!   2 out of phase. For two channels of equal level it is exactly `1 − ρ`.
//!
//! Silence is ignored by the sums (it adds zero), so the readings weigh the music by its
//! energy, as an ear does. A band with no energy reads `None`, never a made-up number.

use crate::math;

/// The lows end here, Hz.
pub const LOW_HZ: f32 = 200.0;
/// The highs begin here, Hz.
pub const HIGH_HZ: f32 = 4_000.0;
/// Whole, lows, mids, highs.
pub const BANDS: usize = 4;
/// The index of each band in the readings.
pub const WHOLE: usize = 0;
pub const LOWS: usize = 1;
pub const MIDS: usize = 2;
pub const HIGHS: usize = 3;
/// 100 ms steps in the short window (3 s).
pub const WINDOW_STEPS: usize = 30;
/// Sums under this are no signal (−200 dB of energy).
const FLOOR: f64 = 1e-20;

/// One second-order section, transposed direct form II, in `f64`, with the family's flush.
#[derive(Copy, Clone, Debug, Default)]
struct Section {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}

impl Section {
    /// A Butterworth low- or high-pass at `hz` (the bilinear transform, prewarped).
    fn butterworth(sample_rate: f32, hz: f32, high: bool) -> Self {
        let fs = sample_rate.max(1.0) as f64;
        let f = (hz as f64).min(0.45 * fs);
        let k = math::tan(std::f64::consts::PI * f / fs);
        let q = std::f64::consts::FRAC_1_SQRT_2;
        let norm = 1.0 / (1.0 + k / q + k * k);
        let b = if high {
            [norm, -2.0 * norm, norm]
        } else {
            let b0 = k * k * norm;
            [b0, 2.0 * b0, b0]
        };
        Section {
            b,
            a: [2.0 * (k * k - 1.0) * norm, (1.0 - k / q + k * k) * norm],
            z: [0.0; 2],
        }
    }

    #[inline]
    fn tick(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        if self.z[0].abs() < 1e-30 && self.z[1].abs() < 1e-30 {
            self.z = [0.0; 2];
        }
        y
    }

    fn is_clear(&self) -> bool {
        crate::rest::all_zero_f64(&self.z)
    }
}

/// The three bands' filters for one channel: the lows (LR4 low-pass), the mids (LR4
/// high-pass then LR4 low-pass), the highs (LR4 high-pass).
#[derive(Copy, Clone, Debug)]
struct Split {
    low: [Section; 2],
    mid: [Section; 4],
    high: [Section; 2],
}

impl Split {
    fn new(sample_rate: f32) -> Self {
        let lp = |hz| Section::butterworth(sample_rate, hz, false);
        let hp = |hz| Section::butterworth(sample_rate, hz, true);
        Split {
            low: [lp(LOW_HZ), lp(LOW_HZ)],
            mid: [hp(LOW_HZ), hp(LOW_HZ), lp(HIGH_HZ), lp(HIGH_HZ)],
            high: [hp(HIGH_HZ), hp(HIGH_HZ)],
        }
    }

    /// One sample in; the whole, the lows, the mids and the highs out.
    #[inline]
    fn tick(&mut self, x: f64) -> [f64; BANDS] {
        let mut lo = x;
        for s in &mut self.low {
            lo = s.tick(lo);
        }
        let mut mid = x;
        for s in &mut self.mid {
            mid = s.tick(mid);
        }
        let mut hi = x;
        for s in &mut self.high {
            hi = s.tick(hi);
        }
        [x, lo, mid, hi]
    }

    fn is_clear(&self) -> bool {
        self.low
            .iter()
            .chain(&self.mid)
            .chain(&self.high)
            .all(Section::is_clear)
    }

    fn reset(&mut self) {
        for s in self
            .low
            .iter_mut()
            .chain(&mut self.mid)
            .chain(&mut self.high)
        {
            s.z = [0.0; 2];
        }
    }
}

/// `Σ LL`, `Σ RR`, `Σ LR` for each band.
pub type Sums = [[f64; 3]; BANDS];

/// What one band reads.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Band {
    /// −1 … +1; `None` with no signal in the band.
    pub correlation: Option<f64>,
    /// 0 … 2; `None` with no signal in the band.
    pub width: Option<f64>,
}

/// Correlation and width from a band's sums.
pub fn band_of(s: [f64; 3]) -> Band {
    let [ll, rr, lr] = s;
    let width = (ll + rr > FLOOR).then(|| (1.0 - 2.0 * lr / (ll + rr)).clamp(0.0, 2.0));
    let correlation = (ll > FLOOR && rr > FLOOR).then(|| (lr / (ll * rr).sqrt()).clamp(-1.0, 1.0));
    Band { correlation, width }
}

/// The stereo analyser of one source.
#[derive(Clone, Debug)]
pub struct Stereo {
    split: [Split; 2],
    /// Since the measurement began.
    total: Sums,
    /// The step in progress.
    step: Sums,
    /// The last [`WINDOW_STEPS`] steps.
    ring: [Sums; WINDOW_STEPS],
    pos: usize,
}

impl Stereo {
    pub fn new(sample_rate: f32) -> Self {
        Stereo {
            split: [Split::new(sample_rate); 2],
            total: [[0.0; 3]; BANDS],
            step: [[0.0; 3]; BANDS],
            ring: [[[0.0; 3]; BANDS]; WINDOW_STEPS],
            pos: 0,
        }
    }

    pub fn process(&mut self, left: &[f32], right: &[f32]) {
        if self.is_clear() && crate::rest::all_zero(left) && crate::rest::all_zero(right) {
            // A clear filter fed zeros gives zeros: nothing to add.
            return;
        }
        for (&l, &r) in left.iter().zip(right) {
            let bl = self.split[0].tick(l as f64);
            let br = self.split[1].tick(r as f64);
            for b in 0..BANDS {
                let s = &mut self.step[b];
                s[0] += bl[b] * bl[b];
                s[1] += br[b] * br[b];
                s[2] += bl[b] * br[b];
            }
        }
    }

    /// Close a 100 ms step.
    pub fn end_step(&mut self) {
        for b in 0..BANDS {
            for k in 0..3 {
                self.total[b][k] += self.step[b][k];
            }
        }
        self.ring[self.pos] = self.step;
        self.pos = (self.pos + 1) % WINDOW_STEPS;
        self.step = [[0.0; 3]; BANDS];
    }

    /// Whole, lows, mids, highs, over the whole measurement.
    pub fn total(&self) -> [Band; BANDS] {
        let mut out = [Band::default(); BANDS];
        for (o, s) in out.iter_mut().zip(&self.total) {
            *o = band_of(*s);
        }
        out
    }

    /// Whole, lows, mids, highs, over the last three seconds.
    pub fn recent(&self) -> [Band; BANDS] {
        let mut sums = [[0.0; 3]; BANDS];
        for step in &self.ring {
            for b in 0..BANDS {
                for k in 0..3 {
                    sums[b][k] += step[b][k];
                }
            }
        }
        let mut out = [Band::default(); BANDS];
        for (o, s) in out.iter_mut().zip(&sums) {
            *o = band_of(*s);
        }
        out
    }

    /// A new measurement: the sums empty, the filters running on.
    pub fn restart(&mut self) {
        self.total = [[0.0; 3]; BANDS];
        self.step = [[0.0; 3]; BANDS];
        self.ring = [[[0.0; 3]; BANDS]; WINDOW_STEPS];
        self.pos = 0;
    }

    pub fn reset(&mut self) {
        self.restart();
        for s in &mut self.split {
            s.reset();
        }
    }

    /// The filters hold nothing (the sums are readings, not sound).
    pub fn is_clear(&self) -> bool {
        self.split.iter().all(Split::is_clear)
    }
}
