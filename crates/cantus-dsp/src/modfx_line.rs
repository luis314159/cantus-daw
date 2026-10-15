//! The modulation family's delay line (ADR 0055): one channel, written a sample at a time
//! and read at a fractional, moving delay — chorus voices, a flanger's sweep, a vibrato.
//!
//! The read is a 4-point, 3rd-order Hermite (Catmull–Rom) interpolation. At a whole number
//! of samples it returns the stored sample **exactly** (the fraction multiplies to zero),
//! which is what a through-zero flanger's centre null and a vibrato at depth 0 rely on.
//!
//! A [`ZeroRun`] beside the ring counts the zeros written in a row, so a unit asks "does
//! the line hold only zeros?" with one comparison (ADR 0031 §5.1).

use crate::rest::ZeroRun;

#[derive(Clone, Debug)]
pub struct ModLine {
    buf: Box<[f32]>,
    mask: usize,
    /// Where the next sample goes.
    w: usize,
    zeros: ZeroRun,
}

impl ModLine {
    /// A line that can be read up to `max_delay` samples back. Allocates: build it off the
    /// audio thread.
    pub fn new(max_delay: usize) -> ModLine {
        let len = (max_delay + 4).next_power_of_two();
        ModLine {
            buf: vec![0.0; len].into_boxed_slice(),
            mask: len - 1,
            w: 0,
            zeros: ZeroRun::FULL,
        }
    }

    /// The longest delay a read may ask for, samples.
    pub fn max_delay(&self) -> f32 {
        (self.buf.len() - 4) as f32
    }

    pub fn reset(&mut self) {
        self.buf.fill(0.0);
        self.w = 0;
        self.zeros.clear();
    }

    /// Every cell holds +0.0.
    #[inline]
    pub fn is_silent(&self) -> bool {
        self.zeros.covers(self.buf.len())
    }

    /// Write the next sample.
    #[inline]
    pub fn push(&mut self, x: f32) {
        self.buf[self.w] = x;
        self.w = (self.w + 1) & self.mask;
        self.zeros.push(x);
    }

    /// `frames` of zeros, written without touching the ring when it already holds only
    /// zeros (a unit skipped at rest).
    pub fn skip(&mut self, frames: usize) {
        if self.is_silent() {
            self.w = (self.w + frames) & self.mask;
        } else {
            for _ in 0..frames {
                self.push(0.0);
            }
        }
    }

    /// The sample written `k` pushes ago (0: the newest).
    #[inline]
    pub fn tap(&self, k: usize) -> f32 {
        self.buf[self.w.wrapping_sub(1 + k) & self.mask]
    }

    /// The signal `delay` samples ago (0: the newest sample), interpolated. `delay` is
    /// clamped into 0 … [`ModLine::max_delay`]; at a whole number the stored sample comes
    /// back to the bit.
    #[inline]
    pub fn read(&self, delay: f32) -> f32 {
        let d = delay.clamp(0.0, self.max_delay());
        let i = d as usize;
        let t = d - i as f32;
        let x0 = self.tap(i);
        if t == 0.0 {
            return x0;
        }
        // x0 is i samples ago, x1 one older; the newer neighbour falls back to x0 when
        // the read is within the newest sample.
        let xm1 = if i == 0 { x0 } else { self.tap(i - 1) };
        let x1 = self.tap(i + 1);
        let x2 = self.tap(i + 2);
        hermite(xm1, x0, x1, x2, t)
    }
}

/// 4-point, 3rd-order Hermite between `x0` (t = 0) and `x1` (t = 1).
#[inline]
pub fn hermite(xm1: f32, x0: f32, x1: f32, x2: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}

/// A one-pole low-pass (TPT), for the darker wet path of a bucket brigade.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OnePole {
    s: f32,
}

impl OnePole {
    /// The coefficient of a cut-off `hz` at `sample_rate`.
    pub fn coef(hz: f32, sample_rate: f32) -> f32 {
        let g = crate::math::tan(std::f32::consts::PI * (hz / sample_rate).clamp(1e-6, 0.49));
        g / (1.0 + g)
    }

    #[inline]
    pub fn run(&mut self, x: f32, g: f32) -> f32 {
        let v = (x - self.s) * g;
        let y = v + self.s;
        self.s = y + v;
        y
    }

    pub fn reset(&mut self) {
        self.s = 0.0;
    }

    /// The state is +0.0.
    pub fn is_zero(&self) -> bool {
        self.s.to_bits() == 0
    }

    /// Flush a state that is only a denormal's worth away from zero.
    #[inline]
    pub fn settle(&mut self, below: f32) {
        if self.s.abs() < below {
            self.s = 0.0;
        }
    }
}

/// A two-pole low-pass, Butterworth (Q = 1/√2), as a TPT state-variable filter: a
/// harmonic tremolo's crossover, a bucket brigade's reconstruction filter.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Lp2 {
    s1: f32,
    s2: f32,
}

/// The coefficients of an [`Lp2`] at a cut-off: `(g, k, a1)` of the TPT SVF.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Lp2Coef {
    a1: f32,
    a2: f32,
    a3: f32,
}

impl Lp2Coef {
    pub fn new(hz: f32, sample_rate: f32) -> Lp2Coef {
        let g = crate::math::tan(std::f32::consts::PI * (hz / sample_rate).clamp(1e-6, 0.49));
        let k = std::f32::consts::SQRT_2;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        Lp2Coef { a1, a2, a3: g * a2 }
    }
}

impl Lp2 {
    /// One sample (Simper's trapezoidal SVF): the low-pass output.
    #[inline]
    pub fn run(&mut self, x: f32, c: &Lp2Coef) -> f32 {
        let v3 = x - self.s2;
        let v1 = c.a1 * self.s1 + c.a2 * v3;
        let v2 = self.s2 + c.a2 * self.s1 + c.a3 * v3;
        self.s1 = 2.0 * v1 - self.s1;
        self.s2 = 2.0 * v2 - self.s2;
        v2
    }

    pub fn reset(&mut self) {
        *self = Lp2::default();
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

/// A first-order allpass, TPT: a phaser's stage. `g = tan(π·fc/fs)`; the phase passes
/// −90° at `fc`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Allpass1 {
    s: f32,
}

impl Allpass1 {
    /// One sample with `h = g / (1 + g)`.
    #[inline]
    pub fn run(&mut self, x: f32, h: f32) -> f32 {
        let v = (x - self.s) * h;
        let lp = v + self.s;
        self.s = lp + v;
        2.0 * lp - x
    }

    pub fn reset(&mut self) {
        self.s = 0.0;
    }

    pub fn state(&self) -> f32 {
        self.s
    }

    #[inline]
    pub fn settle(&mut self, below: f32) {
        if self.s.abs() < below {
            self.s = 0.0;
        }
    }
}

/// A parameter's glide: one pole, landing **exactly** on its target once within `snap`,
/// so a unit knows when it has stopped moving (ADR 0031 §5.1).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Glide {
    pub value: f32,
    pub target: f32,
    k: f32,
    snap: f32,
}

impl Glide {
    /// At `value`, gliding with a time constant of `ms` at `sample_rate`; landing within
    /// `snap` of the target.
    pub fn new(value: f32, ms: f32, sample_rate: f32, snap: f32) -> Glide {
        Glide {
            value,
            target: value,
            k: 1.0 - crate::math::exp(-1.0 / (ms * 0.001 * sample_rate).max(1.0)),
            snap,
        }
    }

    /// Jump to `v` (a reset).
    pub fn jump(&mut self, v: f32) {
        self.value = v;
        self.target = v;
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        if self.value != self.target {
            self.value += (self.target - self.value) * self.k;
            if (self.value - self.target).abs() < self.snap {
                self.value = self.target;
            }
        }
        self.value
    }

    #[inline]
    pub fn still(&self) -> bool {
        self.value == self.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_pole_low_pass_is_minus_three_db_at_its_cutoff() {
        let sr = 48_000.0;
        let c = Lp2Coef::new(1_000.0, sr);
        for (f, want_db) in [(100.0f32, 0.0f32), (1_000.0, -3.01), (10_000.0, -40.0)] {
            let mut lp = Lp2::default();
            let w = std::f32::consts::TAU * f / sr;
            let mut peak = 0.0f32;
            for n in 0..48_000 {
                let y = lp.run(crate::math::sin(w * (n % 48_000) as f32), &c);
                if n > 24_000 {
                    peak = peak.max(y.abs());
                }
            }
            let db = 20.0 * crate::math::log10(peak);
            let tol = if f > 5_000.0 { 3.0 } else { 0.1 };
            assert!((db - want_db).abs() < tol, "{f} Hz: {db} dB");
        }
    }

    #[test]
    fn the_allpass_keeps_the_level_and_turns_ninety_degrees_at_its_frequency() {
        let sr = 48_000.0f32;
        let fc = 2_000.0f32;
        let g = crate::math::tan(std::f32::consts::PI * fc / sr);
        let h = g / (1.0 + g);
        let mut ap = Allpass1::default();
        let w = std::f32::consts::TAU * fc / sr;
        let (mut peak, mut cross) = (0.0f32, 0.0f32);
        for n in 0..48_000 {
            let x = crate::math::sin(w * n as f32);
            let y = ap.run(x, h);
            if n > 24_000 {
                peak = peak.max(y.abs());
                cross += x * y;
            }
        }
        assert!((peak - 1.0).abs() < 1e-3);
        // At −90° the input and output are uncorrelated.
        assert!(cross.abs() / 24_000.0 < 1e-3, "{cross}");
    }

    #[test]
    fn a_whole_delay_returns_the_sample_to_the_bit() {
        let mut l = ModLine::new(64);
        let x: Vec<f32> = (0..100).map(|i| ((i * 37) % 11) as f32 * 0.1 - 0.4).collect();
        for v in &x {
            l.push(*v);
        }
        for k in 0..20 {
            assert_eq!(l.read(k as f32).to_bits(), x[99 - k].to_bits());
        }
    }

    #[test]
    fn a_fractional_delay_interpolates_a_slow_sine_closely() {
        let mut l = ModLine::new(64);
        let f = |n: f32| crate::math::sin(0.05 * n);
        for n in 0..100 {
            l.push(f(n as f32));
        }
        for d in [1.25f32, 3.5, 10.75] {
            let want = f(99.0 - d);
            assert!((l.read(d) - want).abs() < 1e-5, "{d}");
        }
    }

    #[test]
    fn it_knows_when_it_holds_only_zeros() {
        let mut l = ModLine::new(10);
        assert!(l.is_silent());
        l.push(1.0);
        assert!(!l.is_silent());
        for _ in 0..16 {
            l.push(0.0);
        }
        assert!(l.is_silent());
        l.skip(5);
        assert!(l.is_silent());
    }
}
