//! Virtual-analog building blocks shared by the instruments (ADR 0023): zero-delay-feedback
//! filters, band-limited analog oscillators with sync and cross-modulation, envelopes with
//! circuit models, and seeded drift. Built for Calor; Unda and Gravis may take any of it.
//!
//! | Module | What |
//! |---|---|
//! | [`filter`] | ladder, cascade (OTA), diode ladder, state-variable, Sallen-Key — ZDF, nonlinear, self-oscillating at resonance 1.0 exactly on the cutoff |
//! | [`osc`] | saw + pulse + triangle from one phase; BLEP/BLAMP corrections (two- or four-point) at fractional times, so hard sync, exponential FM and PWM stay clean; a sub-oscillator |
//! | [`env`] | RC / ADS / Digital / One-shot envelopes that retrigger from where they are |
//! | [`drift`] | counter-based drift and voice cards (calibration errors per card) |
//! | [`math`] | our own `tanh`, `tan`, `exp2`, `sin` and hash, the same bits on every platform |
//! | [`response`] | the linear response of each filter model, for editors |
//!
//! Everything is allocation-free, `Copy` where it holds state, and deterministic: the
//! same input gives the same bits at every block size.

#[path = "va_drift.rs"]
pub mod drift;
#[path = "va_env.rs"]
pub mod env;
#[path = "va_filter.rs"]
pub mod filter;
#[path = "va_math.rs"]
pub mod math;
#[path = "va_osc.rs"]
pub mod osc;
#[path = "va_response.rs"]
pub mod response;

#[cfg(test)]
#[path = "va_testkit.rs"]
pub(crate) mod testkit;
#[cfg(test)]
#[path = "va_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "va_tests_osc.rs"]
mod tests_osc;

pub use drift::{card_trims, CardTrims, Variance, CARDS};
pub use env::{EnvCoefs, EnvModel, EnvSettings, EnvStage, VaEnv};
pub use filter::{FilterCoefs, FilterSettings, Model, Revision, Solve, VaFilter};
pub use osc::{Blep, Crossings, Kernel, Shape, SubOsc, SubShape, VaOsc};

/// A two-pole Butterworth low-pass (trapezoidal state-variable form): a circuit's
/// bandwidth, ahead of a nonlinearity, so partials far above hearing are not there to fold.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Bandwidth {
    s: [f32; 2],
    a: [f32; 3],
}

impl Bandwidth {
    /// Set the corner: `g = tan(π·fc/fs)`.
    pub fn set(&mut self, g: f32) {
        let a1 = 1.0 / (1.0 + g * (g + std::f32::consts::SQRT_2));
        self.a = [a1, g * a1, g * g * a1];
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let [a1, a2, a3] = self.a;
        let v3 = x - self.s[1];
        let v1 = a1 * self.s[0] + a2 * v3;
        let v2 = self.s[1] + a2 * self.s[0] + a3 * v3;
        self.s[0] = math::flush(2.0 * v1 - self.s[0]);
        self.s[1] = math::flush(2.0 * v2 - self.s[1]);
        v2
    }
}

/// A trapezoidal one-pole: the high-pass in front of or behind a filter, AC coupling, a
/// shelf. `g = tan(π·fc/fs)`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OnePole {
    s: f32,
}

impl OnePole {
    pub fn reset(&mut self) {
        self.s = 0.0;
    }

    /// The per-sample coefficient for `g`: work it out once, use it with
    /// [`lp_c`](Self::lp_c) and [`hp_c`](Self::hp_c).
    #[inline]
    pub fn coef(g: f32) -> f32 {
        g / (1.0 + g)
    }

    /// The low-pass output.
    #[inline]
    pub fn lp(&mut self, x: f32, g: f32) -> f32 {
        self.lp_c(x, Self::coef(g))
    }

    /// The high-pass output.
    #[inline]
    pub fn hp(&mut self, x: f32, g: f32) -> f32 {
        x - self.lp(x, g)
    }

    /// The low-pass output with a coefficient from [`coef`](Self::coef).
    #[inline]
    pub fn lp_c(&mut self, x: f32, c: f32) -> f32 {
        let v = (x - self.s) * c;
        let y = v + self.s;
        self.s = math::flush(y + v);
        y
    }

    /// The high-pass output with a coefficient from [`coef`](Self::coef).
    #[inline]
    pub fn hp_c(&mut self, x: f32, c: f32) -> f32 {
        x - self.lp_c(x, c)
    }

    pub fn is_silent(&self) -> bool {
        self.s == 0.0
    }
}
