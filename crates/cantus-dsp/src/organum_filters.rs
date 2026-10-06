//! The small filters Organum's parts share (and Lamina borrows): a biquad with the shapes
//! they need, a one-pole, and the preamp's clipper with first-order antiderivative
//! anti-aliasing.
//!
//! Every state is flushed below 1e-20 on each sample (as `eq::Biquad` does), so a filter
//! fed exact zeros reaches exact zeros, whatever the block size.

use crate::math;
use std::f32::consts::PI;

#[inline]
fn flush(x: f32) -> f32 {
    if x.abs() < 1e-20 {
        0.0
    } else {
        x
    }
}

/// One second-order section, transposed direct form II. The coefficients are RBJ's
/// cookbook forms; `set_*` keeps the state so a change of coefficients does not click.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bq {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Default for Bq {
    fn default() -> Self {
        Bq::identity()
    }
}

impl Bq {
    /// A wire.
    pub const fn identity() -> Self {
        Bq {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn coefs(&mut self, b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) {
        let inv = 1.0 / a0;
        self.b0 = b0 * inv;
        self.b1 = b1 * inv;
        self.b2 = b2 * inv;
        self.a1 = a1 * inv;
        self.a2 = a2 * inv;
    }

    fn w0(sr: f32, hz: f32) -> (f32, f32) {
        let f = hz.clamp(1.0, sr * 0.49);
        math::sin_cos(2.0 * PI * f / sr)
    }

    /// Second-order low-pass; `q` 0.7071 is Butterworth.
    pub fn set_lowpass(&mut self, sr: f32, hz: f32, q: f32) {
        let (s, c) = Self::w0(sr, hz);
        let alpha = s / (2.0 * q.max(0.05));
        let b1 = 1.0 - c;
        self.coefs(b1 * 0.5, b1, b1 * 0.5, 1.0 + alpha, -2.0 * c, 1.0 - alpha);
    }

    /// Second-order high-pass.
    pub fn set_highpass(&mut self, sr: f32, hz: f32, q: f32) {
        let (s, c) = Self::w0(sr, hz);
        let alpha = s / (2.0 * q.max(0.05));
        let b1 = -(1.0 + c);
        self.coefs(-b1 * 0.5, b1, -b1 * 0.5, 1.0 + alpha, -2.0 * c, 1.0 - alpha);
    }

    /// A bell of `db` at `hz`. At exactly 0 dB it is the identity (bit-exact).
    pub fn set_peak(&mut self, sr: f32, hz: f32, db: f32, q: f32) {
        if db == 0.0 {
            self.set_identity();
            return;
        }
        let (s, c) = Self::w0(sr, hz);
        let a = math::powf(10f32, db / 40.0);
        let alpha = s / (2.0 * q.max(0.05));
        self.coefs(
            1.0 + alpha * a,
            -2.0 * c,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * c,
            1.0 - alpha / a,
        );
    }

    /// A low shelf of `db` below `hz` (slope 1). At exactly 0 dB it is the identity.
    pub fn set_low_shelf(&mut self, sr: f32, hz: f32, db: f32) {
        if db == 0.0 {
            self.set_identity();
            return;
        }
        let (s, c) = Self::w0(sr, hz);
        let a = math::powf(10f32, db / 40.0);
        let alpha = s / 2.0 * 2f32.sqrt();
        let sq = 2.0 * a.sqrt() * alpha;
        self.coefs(
            a * ((a + 1.0) - (a - 1.0) * c + sq),
            2.0 * a * ((a - 1.0) - (a + 1.0) * c),
            a * ((a + 1.0) - (a - 1.0) * c - sq),
            (a + 1.0) + (a - 1.0) * c + sq,
            -2.0 * ((a - 1.0) + (a + 1.0) * c),
            (a + 1.0) + (a - 1.0) * c - sq,
        );
    }

    /// A high shelf of `db` above `hz` (slope 1). At exactly 0 dB it is the identity.
    pub fn set_high_shelf(&mut self, sr: f32, hz: f32, db: f32) {
        if db == 0.0 {
            self.set_identity();
            return;
        }
        let (s, c) = Self::w0(sr, hz);
        let a = math::powf(10f32, db / 40.0);
        let alpha = s / 2.0 * 2f32.sqrt();
        let sq = 2.0 * a.sqrt() * alpha;
        self.coefs(
            a * ((a + 1.0) + (a - 1.0) * c + sq),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
            a * ((a + 1.0) + (a - 1.0) * c - sq),
            (a + 1.0) - (a - 1.0) * c + sq,
            2.0 * ((a - 1.0) - (a + 1.0) * c),
            (a + 1.0) - (a - 1.0) * c - sq,
        );
    }

    pub fn set_identity(&mut self) {
        self.b0 = 1.0;
        self.b1 = 0.0;
        self.b2 = 0.0;
        self.a1 = 0.0;
        self.a2 = 0.0;
    }

    pub fn is_identity(&self) -> bool {
        self.b0 == 1.0 && self.b1 == 0.0 && self.b2 == 0.0 && self.a1 == 0.0 && self.a2 == 0.0
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    #[inline]
    pub fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = flush(self.b1 * x - self.a1 * y + self.z2);
        self.z2 = flush(self.b2 * x - self.a2 * y);
        y
    }

    /// Magnitude at `hz`, dB — for tests and displays.
    pub fn magnitude_db(&self, sr: f32, hz: f32) -> f32 {
        let w = 2.0 * PI * hz / sr;
        let (s1, c1) = math::sin_cos(w);
        let (s2, c2) = math::sin_cos(2.0 * w);
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = -(self.b1 * s1 + self.b2 * s2);
        let dr = 1.0 + self.a1 * c1 + self.a2 * c2;
        let di = -(self.a1 * s1 + self.a2 * s2);
        10.0 * math::log10(((nr * nr + ni * ni) / (dr * dr + di * di).max(1e-30)).max(1e-30))
    }
}

/// The coefficient of a one-pole low-pass at `hz`: `y += a (x − y)`.
pub fn one_pole_coef(sr: f32, hz: f32) -> f32 {
    1.0 - math::exp(-2.0 * PI * hz.clamp(1.0, sr * 0.49) / sr)
}

/// A one-pole low-pass.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OnePole {
    pub a: f32,
    pub z: f32,
}

impl OnePole {
    pub fn set_hz(&mut self, sr: f32, hz: f32) {
        self.a = one_pole_coef(sr, hz);
    }

    #[inline]
    pub fn run(&mut self, x: f32) -> f32 {
        self.z = flush(self.z + self.a * (x - self.z));
        self.z
    }

    pub fn reset(&mut self) {
        self.z = 0.0;
    }
}

/// A linear glide toward a target over a fixed number of samples, counted per sample, so it
/// lands on the target exactly and at the same sample whatever the block size.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Glide {
    pub value: f32,
    target: f32,
    step: f32,
    left: u32,
    len: u32,
}

impl Glide {
    pub fn new(value: f32, len: u32) -> Self {
        Glide {
            value,
            target: value,
            step: 0.0,
            left: 0,
            len: len.max(1),
        }
    }

    pub fn set(&mut self, target: f32) {
        if target == self.target {
            return;
        }
        self.target = target;
        self.left = self.len;
        self.step = (target - self.value) / self.len as f32;
    }

    /// Jump to the target at once (the first configuration, a reset).
    pub fn snap(&mut self, target: f32) {
        self.target = target;
        self.value = target;
        self.left = 0;
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    pub fn moving(&self) -> bool {
        self.left > 0
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        if self.left > 0 {
            self.left -= 1;
            self.value = if self.left == 0 {
                self.target
            } else {
                self.value + self.step
            };
        }
        self.value
    }
}

/// `ln(cosh(z))`, stable for any `z`.
#[inline]
fn ln_cosh(z: f32) -> f32 {
    let a = z.abs();
    a + math::ln_1p(math::exp(-2.0 * a)) - std::f32::consts::LN_2
}

/// An asymmetric soft clipper, `f(x) = tanh(x + b) − tanh(b)`, with first-order
/// antiderivative anti-aliasing (Parker, Zavalishin & Le Bihan, DAFx-16): the output is the
/// mean of `f` over the segment between two input samples, which suppresses the aliases a
/// memoryless curve folds back. The bias `b` gives the even harmonics of a tube stage.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Clipper {
    bias: f32,
    tb: f32,
    x1: f32,
    f1: f32,
}

impl Clipper {
    pub fn new(bias: f32) -> Self {
        let mut c = Clipper {
            bias,
            tb: math::tanh(bias),
            x1: 0.0,
            f1: 0.0,
        };
        c.f1 = c.anti(0.0);
        c
    }

    #[inline]
    fn anti(&self, x: f32) -> f32 {
        ln_cosh(x + self.bias) - x * self.tb
    }

    #[inline]
    pub fn curve(&self, x: f32) -> f32 {
        math::tanh(x + self.bias) - self.tb
    }

    /// The small-signal slope at zero, `1 − tanh²(b)`.
    pub fn slope(&self) -> f32 {
        1.0 - self.tb * self.tb
    }

    #[inline]
    pub fn run(&mut self, x: f32) -> f32 {
        let fx = self.anti(x);
        let d = x - self.x1;
        let y = if d.abs() > 1e-3 {
            (fx - self.f1) / d
        } else {
            self.curve(0.5 * (x + self.x1))
        };
        self.x1 = x;
        self.f1 = fx;
        y
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.f1 = self.anti(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_linkwitz_riley_pair_sums_flat() {
        let sr = 48_000.0;
        let (mut l1, mut l2, mut h1, mut h2) = (
            Bq::identity(),
            Bq::identity(),
            Bq::identity(),
            Bq::identity(),
        );
        for b in [&mut l1, &mut l2] {
            b.set_lowpass(sr, 800.0, std::f32::consts::FRAC_1_SQRT_2);
        }
        for b in [&mut h1, &mut h2] {
            b.set_highpass(sr, 800.0, std::f32::consts::FRAC_1_SQRT_2);
        }
        // An impulse through both branches: the sum's spectrum is flat (an allpass).
        let n = 4096;
        let mut re = vec![0f32; n];
        for (i, v) in re.iter_mut().enumerate() {
            let x = if i == 0 { 1.0 } else { 0.0 };
            *v = l2.run(l1.run(x)) + h2.run(h1.run(x));
        }
        let mut im = vec![0f32; n];
        crate::spectrum::fft(&mut re, &mut im);
        for k in 1..n / 2 {
            let m = (re[k] * re[k] + im[k] * im[k]).sqrt();
            assert!((m - 1.0).abs() < 1e-3, "bin {k}: {m}");
        }
    }

    #[test]
    fn a_glide_lands_exactly_on_its_target() {
        let mut g = Glide::new(0.0, 10);
        g.set(1.0);
        for _ in 0..9 {
            assert!(g.next() < 1.0);
        }
        assert_eq!(g.next(), 1.0);
        assert!(!g.moving());
    }

    #[test]
    fn the_clipper_is_zero_at_zero_and_bounded() {
        let mut c = Clipper::new(0.2);
        assert_eq!(c.curve(0.0), 0.0);
        for i in 0..1000 {
            let x = math::sin(i as f32 * 0.37) * 50.0;
            let y = c.run(x);
            assert!(y.abs() <= 1.3, "{y}");
        }
        c.reset();
        // Silence in, silence out, once settled.
        let mut last = 1.0;
        for _ in 0..4 {
            last = c.run(0.0);
        }
        assert_eq!(last, 0.0);
    }
}
