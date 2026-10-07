//! State-variable filter, trapezoidal-integrated (Andrew Simper / Cytomic form).
//!
//! Stable at any cutoff, resonance can be modulated per sample without zipper noise, and
//! it produces low/band/high outputs from the same state.

use crate::math;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FilterMode {
    Low,
    Band,
    High,
}

#[derive(Clone, Debug)]
pub struct Svf {
    sample_rate: f32,
    ic1eq: f32,
    ic2eq: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
    mode: FilterMode,
}

impl Svf {
    pub fn new(sample_rate: f32) -> Self {
        let mut f = Svf {
            sample_rate,
            ic1eq: 0.0,
            ic2eq: 0.0,
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
            k: 1.0,
            mode: FilterMode::Low,
        };
        f.set(1_000.0, 0.0);
        f
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn set_mode(&mut self, mode: FilterMode) {
        self.mode = mode;
    }

    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    /// `cutoff_hz` is clamped below Nyquist; `resonance` 0..=1.
    /// Resonance 0 is Butterworth (Q = 1/sqrt 2, -3 dB at cutoff); 1 is near self-oscillation.
    #[inline]
    pub fn set(&mut self, cutoff_hz: f32, resonance: f32) {
        let max = self.sample_rate * 0.45;
        let fc = cutoff_hz.clamp(10.0, max);
        let g = math::tan(std::f32::consts::PI * fc / self.sample_rate);
        let r = resonance.clamp(0.0, 1.0);
        let k = std::f32::consts::SQRT_2 - (std::f32::consts::SQRT_2 - 0.1) * r;
        self.k = k;
        self.a1 = 1.0 / (1.0 + g * (g + k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    #[inline]
    pub fn process(&mut self, v0: f32) -> f32 {
        let v3 = v0 - self.ic2eq;
        let v1 = self.a1 * self.ic1eq + self.a2 * v3;
        let v2 = self.ic2eq + self.a2 * self.ic1eq + self.a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        // Per-sample denormal flush: deterministic regardless of block size.
        if self.ic1eq.abs() < 1e-20 {
            self.ic1eq = 0.0;
        }
        if self.ic2eq.abs() < 1e-20 {
            self.ic2eq = 0.0;
        }
        match self.mode {
            FilterMode::Low => v2,
            FilterMode::Band => v1,
            FilterMode::High => v0 - self.k * v1 - v2,
        }
    }

    /// Flush denormals so an idle filter never burns CPU on subnormal arithmetic.
    #[inline]
    pub fn flush_denormals(&mut self) {
        if self.ic1eq.abs() < 1e-20 {
            self.ic1eq = 0.0;
        }
        if self.ic2eq.abs() < 1e-20 {
            self.ic2eq = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response_db(f: &mut Svf, hz: f32, sr: f32) -> f32 {
        f.reset();
        let n = 48_000;
        let mut acc = 0f64;
        for i in 0..n {
            let x = math::sin(std::f32::consts::TAU * hz * i as f32 / sr);
            let y = f.process(x);
            if i > n / 2 {
                acc += (y * y) as f64;
            }
        }
        let rms = (acc / (n / 2) as f64).sqrt() as f32;
        20.0 * math::log10(rms / (0.5f32).sqrt())
    }

    #[test]
    fn lowpass_attenuates_highs() {
        let sr = 48_000.0;
        let mut f = Svf::new(sr);
        f.set(1_000.0, 0.0);
        let pass = response_db(&mut f, 100.0, sr);
        let cut = response_db(&mut f, 8_000.0, sr);
        assert!(pass > -1.0, "passband {pass} dB");
        assert!(cut < -30.0, "stopband {cut} dB");
        let corner = response_db(&mut f, 1_000.0, sr);
        assert!((corner + 3.0).abs() < 1.0, "corner {corner} dB");
    }

    #[test]
    fn resonance_boosts_corner() {
        let sr = 48_000.0;
        let mut f = Svf::new(sr);
        f.set(1_000.0, 0.9);
        let corner = response_db(&mut f, 1_000.0, sr);
        assert!(corner > 6.0, "corner {corner} dB");
    }

    #[test]
    fn stable_at_extremes() {
        let mut f = Svf::new(44_100.0);
        for (fc, r) in [(5.0, 1.0), (30_000.0, 1.0), (20_000.0, 0.0), (10.0, 0.0)] {
            f.set(fc, r);
            f.reset();
            let mut peak = 0f32;
            for i in 0..100_000 {
                let x = if i == 0 { 1.0 } else { 0.0 };
                peak = peak.max(f.process(x).abs());
            }
            assert!(peak.is_finite() && peak < 10.0, "fc {fc} r {r} peak {peak}");
        }
    }
}
