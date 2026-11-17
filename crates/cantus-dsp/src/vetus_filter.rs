//! Vetus's filters (ADR 0133, provisional): one trapezoidal state-variable filter (Simper's
//! SVF) whose output is a mix of its three taps, so the same two states give a low-pass, a
//! high-pass, a band-pass or a bell — the anti-alias and smoothing filters of the rate
//! module, the medium's band and colour, the worn tape's dulling and the crackle's shape.
//!
//! Exact rest (ADR 0031 §5.1): a state that has decayed under [`FLUSH`] is set to +0.0 by
//! [`Svf::settle`], which the unit calls on its 16-sample grid, so a filter that has heard
//! only silence for a while *is* a new one.

use crate::math;

/// States under this are flushed to +0.0 (−300 dB: far under any audible level).
pub const FLUSH: f32 = 1e-15;

/// The coefficients of one [`Svf`]: the solver's three and the taps' mix.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SvfCoef {
    a1: f32,
    a2: f32,
    a3: f32,
    m0: f32,
    m1: f32,
    m2: f32,
}

impl Default for SvfCoef {
    /// A wire: the input's tap only.
    fn default() -> Self {
        SvfCoef {
            a1: 1.0,
            a2: 0.0,
            a3: 0.0,
            m0: 1.0,
            m1: 0.0,
            m2: 0.0,
        }
    }
}

/// `g = tan(π·f/fs)` with the frequency kept under Nyquist.
fn g_of(hz: f32, sample_rate: f32) -> f32 {
    math::tan(std::f32::consts::PI * (hz / sample_rate).clamp(1e-6, 0.4999))
}

impl SvfCoef {
    fn solver(g: f32, k: f32) -> (f32, f32, f32) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        (a1, a2, g * a2)
    }

    /// A low-pass at `hz` with quality `q`.
    pub fn low(hz: f32, q: f32, sample_rate: f32) -> SvfCoef {
        let k = 1.0 / q.max(0.05);
        let (a1, a2, a3) = Self::solver(g_of(hz, sample_rate), k);
        SvfCoef {
            a1,
            a2,
            a3,
            m0: 0.0,
            m1: 0.0,
            m2: 1.0,
        }
    }

    /// A high-pass at `hz` with quality `q`.
    pub fn high(hz: f32, q: f32, sample_rate: f32) -> SvfCoef {
        let k = 1.0 / q.max(0.05);
        let (a1, a2, a3) = Self::solver(g_of(hz, sample_rate), k);
        SvfCoef {
            a1,
            a2,
            a3,
            m0: 1.0,
            m1: -k,
            m2: -1.0,
        }
    }

    /// A band-pass at `hz` with quality `q`, unity at its centre.
    pub fn band(hz: f32, q: f32, sample_rate: f32) -> SvfCoef {
        let k = 1.0 / q.max(0.05);
        let (a1, a2, a3) = Self::solver(g_of(hz, sample_rate), k);
        SvfCoef {
            a1,
            a2,
            a3,
            m0: 0.0,
            m1: k,
            m2: 0.0,
        }
    }

    /// A bell at `hz`, `db` up or down, quality `q`.
    pub fn bell(hz: f32, q: f32, db: f32, sample_rate: f32) -> SvfCoef {
        let a = math::exp2(db / 40.0 * std::f32::consts::LOG2_10);
        let k = 1.0 / (q.max(0.05) * a);
        let (a1, a2, a3) = Self::solver(g_of(hz, sample_rate), k);
        SvfCoef {
            a1,
            a2,
            a3,
            m0: 1.0,
            m1: k * (a * a - 1.0),
            m2: 0.0,
        }
    }
}

/// One channel's state-variable filter.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Svf {
    s1: f32,
    s2: f32,
}

impl Svf {
    #[inline]
    pub fn run(&mut self, x: f32, c: &SvfCoef) -> f32 {
        let v3 = x - self.s2;
        let v1 = c.a1 * self.s1 + c.a2 * v3;
        let v2 = self.s2 + c.a2 * self.s1 + c.a3 * v3;
        self.s1 = 2.0 * v1 - self.s1;
        self.s2 = 2.0 * v2 - self.s2;
        c.m0 * x + c.m1 * v1 + c.m2 * v2
    }

    pub fn reset(&mut self) {
        *self = Svf::default();
    }

    /// Both states are +0.0.
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.s1.to_bits() == 0 && self.s2.to_bits() == 0
    }

    /// States that have decayed under [`FLUSH`] become +0.0.
    #[inline]
    pub fn settle(&mut self) {
        if self.s1.abs() < FLUSH && self.s2.abs() < FLUSH {
            self.s1 = 0.0;
            self.s2 = 0.0;
        }
    }
}

/// The quality factors of a Butterworth low-pass of `2·N` poles as `N` sections.
pub const BUTTER4: [f32; 2] = [0.541_196_1, 1.306_563];
pub const BUTTER6: [f32; 3] = [0.517_638_1, std::f32::consts::FRAC_1_SQRT_2, 1.931_851_6];

#[cfg(test)]
mod tests {
    use super::*;

    fn gain_db(c: &SvfCoef, hz: f32, sr: f32) -> f32 {
        let mut f = Svf::default();
        let w = std::f32::consts::TAU * hz / sr;
        let mut peak = 0.0f32;
        for n in 0..(sr as usize) {
            let y = f.run(math::sin(w * n as f32), c);
            if n > sr as usize / 2 {
                peak = peak.max(y.abs());
            }
        }
        20.0 * math::log10(peak)
    }

    #[test]
    fn the_shapes_are_where_they_are_asked() {
        let sr = 48_000.0;
        let q = std::f32::consts::FRAC_1_SQRT_2;
        assert!((gain_db(&SvfCoef::low(1_000.0, q, sr), 1_000.0, sr) + 3.01).abs() < 0.1);
        assert!((gain_db(&SvfCoef::high(1_000.0, q, sr), 1_000.0, sr) + 3.01).abs() < 0.1);
        assert!(gain_db(&SvfCoef::band(1_000.0, 2.0, sr), 1_000.0, sr).abs() < 0.1);
        assert!((gain_db(&SvfCoef::bell(1_000.0, 1.0, 6.0, sr), 1_000.0, sr) - 6.0).abs() < 0.1);
        assert!(gain_db(&SvfCoef::bell(1_000.0, 1.0, 6.0, sr), 50.0, sr).abs() < 0.3);
    }

    #[test]
    fn a_wire_is_a_wire_and_silence_settles() {
        let mut f = Svf::default();
        let c = SvfCoef::default();
        for x in [0.3f32, -0.7, 1e-30] {
            assert_eq!(f.run(x, &c).to_bits(), x.to_bits());
        }
        let mut f = Svf::default();
        let lp = SvfCoef::low(200.0, 0.7, 48_000.0);
        f.run(1.0, &lp);
        for _ in 0..200_000 {
            f.run(0.0, &lp);
            f.settle();
        }
        assert!(f.is_zero());
    }
}
