//! Vetus's converter (ADR 0133, provisional): **bits** (a quantiser) and **rate** (a sample
//! and hold), the two things a cheap sampler did to a sound.
//!
//! - **Bits.** `2^bits` levels over −1 … 1 (a step `q = 2^(1−bits)`, fractional bits a
//!   continuous step, so a sweep never jumps a level count). Three characters: *rounding*
//!   (mid-tread: silence stays silent), *truncation* (towards −∞, a cheap converter's: its
//!   error is one-sided, a step's worth of offset and more even harmonics), and *μ-law*
//!   (μ = 255, the telephone's companding: small signals keep more levels, loud ones fewer).
//!   Optional **dither**: triangular, one step peak, from a hash of the seed and the song
//!   sample (so offline equals live). An input of exactly zero stays exactly zero (digital
//!   silence is never dithered: the unit can rest). **At 24 bits without dither it is a
//!   wire**, to the bit: 24 bits is the float's own mantissa.
//! - **Rate.** The signal is held for `fs / rate` samples at a time: a phase that advances by
//!   `rate / fs` each sample takes a new value when it passes 1. *None*: the raw hold — what
//!   lies above the new Nyquist folds back (aliasing) and the hold's steps image the band
//!   upwards: the sound of a 90s sampler. *Anti-alias*: a 6-pole Butterworth low-pass at
//!   0.45 of the new rate before the hold, so nothing folds. *Anti-alias and smoothing*: a
//!   4-pole one after it too, so the steps are smoothed away: only the bandwidth is lost.
//!   While nothing sounds the phase is a function of the stream sample (`n·rate/fs`), so a
//!   unit that rested is where a new one would be.

use crate::math;
use crate::vetus::filter::{Svf, SvfCoef, BUTTER4, BUTTER6};
use crate::vetus::noise::{hash3, unit};

/// The quantiser's characters (`vetus.bits_mode.{i}`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum BitsMode {
    #[default]
    Round,
    Truncate,
    MuLaw,
}

impl BitsMode {
    pub fn from_index(v: f32) -> BitsMode {
        match v.round() as i32 {
            1 => BitsMode::Truncate,
            2 => BitsMode::MuLaw,
            _ => BitsMode::Round,
        }
    }
}

const MU: f32 = 255.0;
const SEED: u64 = 0xB175_D17E;

/// The quantiser for both channels.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bits {
    pub depth: f32,
    pub dither: bool,
    pub mode: BitsMode,
    /// The step, `2^(1 − depth)`.
    q: f32,
    inv_q: f32,
    /// `ln(1 + μ)`.
    ln_mu: f32,
}

impl Bits {
    pub fn new(depth: f32, dither: bool, mode: BitsMode) -> Bits {
        let depth = depth.clamp(1.0, 24.0);
        let q = math::exp2(1.0 - depth);
        Bits {
            depth,
            dither,
            mode,
            q,
            inv_q: 1.0 / q,
            ln_mu: math::ln(1.0 + MU),
        }
    }

    /// Whether it touches nothing: 24 bits, no dither.
    pub fn is_wire(&self) -> bool {
        self.depth >= 24.0 && !self.dither
    }

    /// The step in the quantiser's own domain.
    pub fn step(&self) -> f32 {
        self.q
    }

    #[inline]
    fn grid(&self, v: f32, d: f32) -> f32 {
        let u = v * self.inv_q + d;
        let r = match self.mode {
            BitsMode::Truncate => u.floor(),
            _ => u.round(),
        };
        r * self.q
    }

    /// Sample `x` of channel `ch` at song sample `n`.
    #[inline]
    pub fn run(&self, x: f32, ch: usize, n: i64) -> f32 {
        if self.is_wire() || x == 0.0 {
            return x;
        }
        let d = if self.dither {
            let h = hash3(SEED, ch as u64, n);
            unit(h) + unit(crate::modfx_clock::hash64(h)) - 1.0
        } else {
            0.0
        };
        match self.mode {
            BitsMode::MuLaw => {
                let a = x.abs();
                let c = math::ln(1.0 + MU * a) / self.ln_mu;
                let cq = self.grid(c, d).max(0.0);
                let y = (math::exp(cq * self.ln_mu) - 1.0) / MU;
                y.copysign(x)
            }
            _ => self.grid(x, d),
        }
    }
}

/// The rate module's filters (`vetus.rate_filter.{i}`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum RateFilter {
    #[default]
    None,
    AntiAlias,
    Both,
}

impl RateFilter {
    pub fn from_index(v: f32) -> RateFilter {
        match v.round() as i32 {
            1 => RateFilter::AntiAlias,
            2 => RateFilter::Both,
            _ => RateFilter::None,
        }
    }
}

/// The sample and hold with its two filters, both channels.
#[derive(Clone, Debug)]
pub struct Rate {
    sr: f32,
    /// The phase before the next sample: a new value is taken when it passes 1.
    phase: f64,
    held: [f32; 2],
    aa: [[Svf; 3]; 2],
    smooth: [[Svf; 2]; 2],
    aa_c: [SvfCoef; 3],
    smooth_c: [SvfCoef; 2],
    /// The rate the filters were designed for.
    designed: f32,
}

impl Rate {
    pub fn new(sample_rate: f32) -> Rate {
        Rate {
            sr: sample_rate,
            phase: 0.0,
            held: [0.0; 2],
            aa: [[Svf::default(); 3]; 2],
            smooth: [[Svf::default(); 2]; 2],
            aa_c: [SvfCoef::default(); 3],
            smooth_c: [SvfCoef::default(); 2],
            designed: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.held = [0.0; 2];
        for f in self.aa.iter_mut().flatten() {
            f.reset();
        }
        for f in self.smooth.iter_mut().flatten() {
            f.reset();
        }
    }

    /// The filters for a new rate (called on the unit's grid, never per sample).
    pub fn design(&mut self, rate: f32) {
        if rate == self.designed {
            return;
        }
        self.designed = rate;
        let fc = (0.45 * rate).min(0.45 * self.sr);
        for (c, q) in self.aa_c.iter_mut().zip(BUTTER6) {
            *c = SvfCoef::low(fc, q, self.sr);
        }
        for (c, q) in self.smooth_c.iter_mut().zip(BUTTER4) {
            *c = SvfCoef::low(fc, q, self.sr);
        }
    }

    /// Nothing held and every filter +0.0.
    pub fn quiet(&self) -> bool {
        self.held.iter().all(|h| h.to_bits() == 0)
            && self.aa.iter().flatten().all(Svf::is_zero)
            && self.smooth.iter().flatten().all(Svf::is_zero)
    }

    pub fn settle(&mut self) {
        for f in self.aa.iter_mut().flatten() {
            f.settle();
        }
        for f in self.smooth.iter_mut().flatten() {
            f.settle();
        }
    }

    /// The phase a new unit would have before stream sample `n` at this rate.
    #[inline]
    fn canonical(n: i64, inc: f64) -> f64 {
        (n as f64 * inc).rem_euclid(1.0)
    }

    /// Skipped at rest up to stream sample `n`: the phase a new unit's would be there.
    pub fn rest_at(&mut self, n: i64, rate: f32) {
        let inc = (f64::from(rate) / f64::from(self.sr)).min(1.0);
        self.phase = Self::canonical(n, inc);
    }

    /// One stereo sample at stream sample `n`, the new rate `rate` Hz.
    #[inline]
    pub fn run(&mut self, x: [f32; 2], n: i64, rate: f32, filter: RateFilter) -> [f32; 2] {
        let inc = (f64::from(rate) / f64::from(self.sr)).min(1.0);
        let silent = x[0] == 0.0 && x[1] == 0.0;
        if silent && self.quiet() {
            // Silence in a quiet hold: where a new unit's phase would be.
            self.phase = Self::canonical(n + 1, inc);
            return [0.0; 2];
        }
        let mut v = x;
        if filter != RateFilter::None {
            for (ch, s) in v.iter_mut().enumerate() {
                for (f, c) in self.aa[ch].iter_mut().zip(&self.aa_c) {
                    *s = f.run(*s, c);
                }
            }
        }
        // A new unit's phase is 0 before sample 0, so it takes sample 0: the phase passes
        // 1 when it would reach it.
        if self.phase < inc || inc >= 1.0 {
            self.held = v;
        }
        let mut p = self.phase + inc;
        if p >= 1.0 {
            p -= 1.0;
        }
        self.phase = p;
        let mut y = self.held;
        if filter == RateFilter::Both {
            for (ch, s) in y.iter_mut().enumerate() {
                for (f, c) in self.smooth[ch].iter_mut().zip(&self.smooth_c) {
                    *s = f.run(*s, c);
                }
            }
        }
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twenty_four_bits_without_dither_is_a_wire() {
        let b = Bits::new(24.0, false, BitsMode::Truncate);
        for i in 0..10_000 {
            let x = math::sin(i as f32 * 0.0137) * 0.9 + 1e-7 * (i % 7) as f32;
            assert_eq!(b.run(x, 0, i).to_bits(), x.to_bits());
        }
    }

    #[test]
    fn the_levels_are_two_to_the_bits() {
        let b = Bits::new(3.0, false, BitsMode::Round);
        let mut levels: Vec<i32> = (-1000..=1000)
            .map(|i| (b.run(i as f32 / 1000.0, 0, 0) * 4.0).round() as i32)
            .collect();
        levels.sort_unstable();
        levels.dedup();
        // q = 1/4: −1, −¾ … 1 — eight steps across the range and both ends.
        assert_eq!(levels.len(), 9);
        assert_eq!(Bits::new(8.0, false, BitsMode::Round).step(), 1.0 / 128.0);
    }

    #[test]
    fn silence_stays_silent_even_dithered() {
        for mode in [BitsMode::Round, BitsMode::Truncate, BitsMode::MuLaw] {
            let b = Bits::new(4.0, true, mode);
            for n in 0..1000 {
                assert_eq!(b.run(0.0, 1, n).to_bits(), 0);
            }
        }
    }

    #[test]
    fn mu_law_keeps_more_levels_for_quiet_signals() {
        let lin = Bits::new(8.0, false, BitsMode::Round);
        let mu = Bits::new(8.0, false, BitsMode::MuLaw);
        let err = |b: &Bits, a: f32| {
            (0..2000)
                .map(|i| {
                    let x = a * math::sin(i as f32 * 0.031);
                    (b.run(x, 0, 0) - x).abs()
                })
                .fold(0.0f32, f32::max)
        };
        assert!(err(&mu, 0.01) < err(&lin, 0.01) * 0.25);
        assert!(err(&mu, 0.9) > err(&lin, 0.9));
    }

    #[test]
    fn the_hold_takes_a_value_every_fs_over_rate_samples() {
        let mut r = Rate::new(48_000.0);
        r.design(12_000.0);
        let x: Vec<f32> = (0..64).map(|i| i as f32 + 1.0).collect();
        let y: Vec<f32> = x
            .iter()
            .enumerate()
            .map(|(n, v)| r.run([*v, -*v], n as i64, 12_000.0, RateFilter::None)[0])
            .collect();
        assert_eq!(&y[..9], &[1.0, 1.0, 1.0, 1.0, 5.0, 5.0, 5.0, 5.0, 9.0]);
    }

    #[test]
    fn at_the_full_rate_the_raw_hold_is_a_wire() {
        let mut r = Rate::new(48_000.0);
        r.design(48_000.0);
        for n in 0..500i64 {
            let x = [math::sin(n as f32 * 0.3), (n as f32 * 0.11).fract() - 0.5];
            assert_eq!(r.run(x, n, 48_000.0, RateFilter::None), x);
        }
    }
}
