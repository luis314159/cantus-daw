//! A biquad section and the three shapes a channel EQ is made of: a low shelf, a bell and
//! a high shelf (Robert Bristow-Johnson's cookbook forms, direct form I transposed).
//!
//! The coefficients are a pure function of the settings and the sample rate, and the state
//! is two numbers, so the filter is deterministic and block-size independent by
//! construction. A band at 0 dB is **exactly** the identity — `b0 = 1`, everything else 0 —
//! and the caller is expected to skip it anyway, so that a project that never touched the
//! EQ renders exactly as it did before there was one.

use crate::math;
use std::f32::consts::PI;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BandShape {
    LowShelf,
    Peak,
    HighShelf,
}

/// One second-order section. `process` is a transposed direct form II, which keeps the
/// state bounded for the coefficient ranges a channel EQ uses.
#[derive(Clone, Debug)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Default for Biquad {
    fn default() -> Self {
        Biquad::identity()
    }
}

impl Biquad {
    /// A wire.
    pub fn identity() -> Self {
        Biquad {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    /// Nothing left in the section: silence in gives silence out, bit for bit, so a
    /// caller may skip it without changing a sample. The per-sample denormal flush is
    /// what guarantees this state is actually reached.
    pub fn is_quiet(&self) -> bool {
        self.z1 == 0.0 && self.z2 == 0.0
    }

    /// Set the section to `shape` at `freq_hz` with `gain_db` and `q`. Frequencies are
    /// clamped below Nyquist; a gain of exactly zero makes the section the identity
    /// without going through the trigonometry, so "flat" is bit-exact.
    pub fn set(&mut self, shape: BandShape, sample_rate: f32, freq_hz: f32, gain_db: f32, q: f32) {
        if gain_db == 0.0 || !gain_db.is_finite() {
            let (z1, z2) = (self.z1, self.z2);
            *self = Biquad::identity();
            self.z1 = z1;
            self.z2 = z2;
            return;
        }
        let sr = sample_rate.max(1.0);
        let f = freq_hz.clamp(1.0, sr * 0.49);
        let q = q.clamp(0.05, 20.0);
        let a = math::powf(10f32, gain_db / 40.0);
        let w0 = 2.0 * PI * f / sr;
        let (sin, cos) = math::sin_cos(w0);
        let (b0, b1, b2, a0, a1, a2) = match shape {
            BandShape::Peak => {
                let alpha = sin / (2.0 * q);
                (
                    1.0 + alpha * a,
                    -2.0 * cos,
                    1.0 - alpha * a,
                    1.0 + alpha / a,
                    -2.0 * cos,
                    1.0 - alpha / a,
                )
            }
            BandShape::LowShelf => {
                // `q` here is the shelf slope parameter S = 1 → the classic 2nd-order shelf.
                let alpha = sin / 2.0 * ((a + 1.0 / a) * (1.0 / q - 1.0) + 2.0).max(0.0).sqrt();
                let sq = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + sq),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - sq),
                    (a + 1.0) + (a - 1.0) * cos + sq,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - sq,
                )
            }
            BandShape::HighShelf => {
                let alpha = sin / 2.0 * ((a + 1.0 / a) * (1.0 / q - 1.0) + 2.0).max(0.0).sqrt();
                let sq = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + sq),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - sq),
                    (a + 1.0) - (a - 1.0) * cos + sq,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - sq,
                )
            }
        };
        let inv = 1.0 / a0;
        self.b0 = b0 * inv;
        self.b1 = b1 * inv;
        self.b2 = b2 * inv;
        self.a1 = a1 * inv;
        self.a2 = a2 * inv;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        // Per-sample denormal flush, as the SVF does: deterministic whatever the block.
        if self.z1.abs() < 1e-20 {
            self.z1 = 0.0;
        }
        if self.z2.abs() < 1e-20 {
            self.z2 = 0.0;
        }
        y
    }

    /// The section's magnitude response at `hz`, in dB — what a curve display draws.
    pub fn magnitude_db(&self, sample_rate: f32, hz: f32) -> f32 {
        let w = 2.0 * PI * hz / sample_rate.max(1.0);
        let (s1, c1) = math::sin_cos(w);
        let (s2, c2) = math::sin_cos(2.0 * w);
        // H(z) = (b0 + b1 z⁻¹ + b2 z⁻²) / (1 + a1 z⁻¹ + a2 z⁻²) at z = e^{jw}.
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = -(self.b1 * s1 + self.b2 * s2);
        let dr = 1.0 + self.a1 * c1 + self.a2 * c2;
        let di = -(self.a1 * s1 + self.a2 * s2);
        let num = nr * nr + ni * ni;
        let den = (dr * dr + di * di).max(1e-30);
        10.0 * math::log10((num / den).max(1e-30))
    }
}

/// A three-band channel EQ on one channel of audio: low shelf, bell, high shelf, in that
/// order. Stereo is two of these.
#[derive(Clone, Debug, Default)]
pub struct ChannelEq {
    pub bands: [Biquad; 3],
}

impl ChannelEq {
    pub const SHAPES: [BandShape; 3] = [BandShape::LowShelf, BandShape::Peak, BandShape::HighShelf];

    /// Configure band `i` (0 low, 1 mid, 2 high).
    pub fn set_band(&mut self, i: usize, sample_rate: f32, freq_hz: f32, gain_db: f32, q: f32) {
        if let Some(b) = self.bands.get_mut(i) {
            b.set(Self::SHAPES[i], sample_rate, freq_hz, gain_db, q);
        }
    }

    pub fn reset(&mut self) {
        for b in &mut self.bands {
            b.reset();
        }
    }

    /// Every section quiet: see [`Biquad::is_quiet`].
    pub fn is_quiet(&self) -> bool {
        self.bands.iter().all(Biquad::is_quiet)
    }

    #[inline]
    pub fn process(&mut self, buf: &mut [f32]) {
        for x in buf.iter_mut() {
            let mut v = *x;
            v = self.bands[0].process(v);
            v = self.bands[1].process(v);
            v = self.bands[2].process(v);
            *x = v;
        }
    }

    /// The combined response at `hz`, in dB.
    pub fn magnitude_db(&self, sample_rate: f32, hz: f32) -> f32 {
        self.bands
            .iter()
            .map(|b| b.magnitude_db(sample_rate, hz))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Steady-state gain by measurement, to check the closed form against real samples.
    fn measured_db(b: &mut Biquad, hz: f32, sr: f32) -> f32 {
        b.reset();
        let n = 48_000;
        let mut acc = 0f64;
        for i in 0..n {
            let x = math::sin(std::f32::consts::TAU * hz * i as f32 / sr);
            let y = b.process(x);
            if i > n / 2 {
                acc += (y * y) as f64;
            }
        }
        let rms = (acc / (n / 2) as f64).sqrt() as f32;
        20.0 * math::log10(rms / (0.5f32).sqrt())
    }

    #[test]
    fn a_bell_boosts_its_centre_and_leaves_the_far_field_alone() {
        let sr = 48_000.0;
        let mut b = Biquad::identity();
        b.set(BandShape::Peak, sr, 1_000.0, 6.0, 1.0);
        let centre = measured_db(&mut b, 1_000.0, sr);
        assert!((centre - 6.0).abs() < 0.2, "centre {centre} dB");
        let far = measured_db(&mut b, 60.0, sr);
        assert!(far.abs() < 0.3, "far field {far} dB");
        // The closed form agrees with the measurement.
        let formula = b.magnitude_db(sr, 1_000.0);
        assert!(
            (formula - centre).abs() < 0.2,
            "formula {formula} vs {centre}"
        );
    }

    #[test]
    fn shelves_lift_one_end_and_not_the_other() {
        let sr = 48_000.0;
        let mut low = Biquad::identity();
        low.set(BandShape::LowShelf, sr, 200.0, -9.0, 0.71);
        assert!((low.magnitude_db(sr, 20.0) + 9.0).abs() < 0.3);
        assert!(low.magnitude_db(sr, 8_000.0).abs() < 0.2);
        let mut high = Biquad::identity();
        high.set(BandShape::HighShelf, sr, 4_000.0, 6.0, 0.71);
        assert!((high.magnitude_db(sr, 18_000.0) - 6.0).abs() < 0.3);
        assert!(high.magnitude_db(sr, 50.0).abs() < 0.2);
    }

    #[test]
    fn zero_gain_is_a_wire_to_the_bit() {
        let mut b = Biquad::identity();
        b.set(BandShape::Peak, 48_000.0, 1_000.0, 0.0, 0.5);
        let mut eq = ChannelEq::default();
        for i in 0..3 {
            eq.set_band(i, 48_000.0, 500.0 * (i + 1) as f32, 0.0, 0.71);
        }
        let src: Vec<f32> = (0..1000)
            .map(|i| ((i * 7919) % 1000) as f32 / 500.0 - 1.0)
            .collect();
        let mut buf = src.clone();
        eq.process(&mut buf);
        assert_eq!(buf, src, "a flat EQ changed a sample");
        for x in &src {
            assert_eq!(b.process(*x), *x);
        }
    }

    #[test]
    fn stable_at_the_extremes_the_commands_allow() {
        let sr = 44_100.0;
        for (shape, f, g, q) in [
            (BandShape::LowShelf, 20.0, 18.0, 10.0),
            (BandShape::LowShelf, 20_000.0, -18.0, 0.1),
            (BandShape::Peak, 20_000.0, 18.0, 10.0),
            (BandShape::Peak, 20.0, -18.0, 0.1),
            (BandShape::HighShelf, 20_000.0, 18.0, 10.0),
            (BandShape::HighShelf, 20.0, -18.0, 0.1),
        ] {
            let mut b = Biquad::identity();
            b.set(shape, sr, f, g, q);
            b.reset();
            let mut peak = 0f32;
            for i in 0..100_000 {
                let x = if i == 0 { 1.0 } else { 0.0 };
                peak = peak.max(b.process(x).abs());
            }
            assert!(
                peak.is_finite() && peak < 100.0,
                "{shape:?} {f} Hz {g} dB q {q}: impulse peak {peak}"
            );
        }
    }
}
