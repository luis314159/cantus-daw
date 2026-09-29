//! Analog oscillators: saw, pulse and triangle at once from one phase, band-limited by
//! correcting each discontinuity where it happens — a step with a BLEP, a corner with a
//! BLAMP (Välimäki & Huovilainen 2007; Esqueda, Välimäki & Bilbao, DAFx-16) — so hard
//! sync, exponential cross-modulation and pulse-width modulation stay clean.
//!
//! **Event corrections, two samples late.** Every discontinuity is found at its fractional
//! time between two samples; its residual is added to the samples around it, the ones
//! before it included. A [`Blep`] line holds the last samples until nothing can touch them
//! any more, so a whole voice's generators (three oscillators, the sub, the noise) share
//! one line and one constant latency of two samples.
//!
//! **Two kernels.** [`Kernel::Narrow`] is the classic two-point polynomial (a triangular
//! band-limiting kernel); [`Kernel::Wide`] integrates a cubic B-spline over four samples,
//! whose residual falls twice as fast in dB per octave above the band — the difference
//! between aliasing you can find and aliasing under the floor, at a few multiplies per
//! discontinuity.
//!
//! The phase is `f64`: at 20 Hz and 192 kHz an increment is 10⁻⁴, where an `f32` phase
//! would wander by a cent.

#[path = "va_osc_core.rs"]
mod core;
pub use core::{SubOsc, SubShape, VaOsc};

/// The band-limiting kernel of a [`Blep`] line.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Kernel {
    /// Two points (the triangular kernel).
    Narrow,
    /// Four points (the cubic B-spline).
    #[default]
    Wide,
}

/// A voice's generators, summed, with the corrections of their discontinuities; read two
/// samples late. Call [`begin`](Self::begin), add the naive values and the events, then
/// [`end`](Self::end).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Blep {
    ring: [f32; 4],
    pos: usize,
    pub kernel: Kernel,
}

impl Blep {
    pub fn new(kernel: Kernel) -> Self {
        Blep {
            kernel,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        self.ring = [0.0; 4];
        self.pos = 0;
    }

    /// Start sample `n`.
    #[inline]
    pub fn begin(&mut self) {
        self.pos = (self.pos + 1) & 3;
    }

    #[inline]
    fn at(&mut self, offset: usize) -> &mut f32 {
        // offset: 0 = n−2, 1 = n−1, 2 = n, 3 = n+1.
        &mut self.ring[(self.pos + 2 + offset) & 3]
    }

    /// Add a naive value to sample `n`.
    #[inline]
    pub fn add(&mut self, v: f32) {
        *self.at(2) += v;
    }

    /// A step of height `h` at time `n − 1 + τ`, `τ ∈ (0, 1]`.
    #[inline]
    pub fn step(&mut self, h: f32, tau: f32) {
        if h == 0.0 {
            return;
        }
        let t = tau.clamp(0.0, 1.0);
        let u = 1.0 - t;
        match self.kernel {
            Kernel::Narrow => {
                *self.at(1) += h * (0.5 * u * u);
                *self.at(2) -= h * (0.5 * t * t);
            }
            Kernel::Wide => {
                let (t2, u2) = (t * t, u * u);
                let (t4, u4) = (t2 * t2, u2 * u2);
                *self.at(0) += h * (u4 * (1.0 / 24.0));
                *self.at(1) += h * (0.5 + (-4.0 * t + 2.0 * t2 * t - 0.75 * t4) * (1.0 / 6.0));
                *self.at(2) += h * (-0.5 + (4.0 * u - 2.0 * u2 * u + 0.75 * u4) * (1.0 / 6.0));
                *self.at(3) -= h * (t4 * (1.0 / 24.0));
            }
        }
    }

    /// A change of slope of `d` per sample at time `n − 1 + τ`.
    #[inline]
    pub fn ramp(&mut self, d: f32, tau: f32) {
        if d == 0.0 {
            return;
        }
        let t = tau.clamp(0.0, 1.0);
        let u = 1.0 - t;
        match self.kernel {
            Kernel::Narrow => {
                *self.at(1) += d * (u * u * u * (1.0 / 6.0));
                *self.at(2) += d * (t * t * t * (1.0 / 6.0));
            }
            Kernel::Wide => {
                let (t2, u2) = (t * t, u * u);
                let (t4, u4) = (t2 * t2, u2 * u2);
                *self.at(0) += d * (u4 * u * (1.0 / 120.0));
                *self.at(1) += d
                    * (1.0 / 120.0 + 0.5 * u + (2.0 * t2 - 0.5 * t4 + 0.15 * t4 * t) * (1.0 / 6.0)
                        - 0.275);
                *self.at(2) += d
                    * (7.0 / 30.0 - 0.5 * u + (2.0 * u2 - 0.5 * u4 + 0.15 * u4 * u) * (1.0 / 6.0));
                *self.at(3) += d * (t4 * t * (1.0 / 120.0));
            }
        }
    }

    /// Finish sample `n`: the sample two before it, now complete.
    #[inline]
    pub fn end(&mut self) -> f32 {
        let slot = self.at(0);
        let out = *slot;
        *slot = 0.0;
        out
    }

    /// Samples of delay between a naive value and its output.
    pub const LATENCY: usize = 2;
}

/// What one oscillator plays: the level of each wave, the pulse width, and the shape of
/// its circuit revision.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Shape {
    pub saw: f32,
    pub pulse: f32,
    pub tri: f32,
    /// Pulse width, 0…1 (clamped a little inside, per sample, by the increment).
    pub pw: f32,
    /// The saw's curvature (a capacitor charging, not a line): 0 ideal.
    pub curve: f32,
    /// The triangle's rounding at its peaks, 0…0.5: 0 ideal.
    pub round: f32,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            saw: 1.0,
            pulse: 0.0,
            tri: 0.0,
            pw: 0.5,
            curve: 0.0,
            round: 0.0,
        }
    }
}

impl Shape {
    /// Whether anything sounds.
    pub fn is_silent(&self) -> bool {
        self.saw == 0.0 && self.pulse == 0.0 && self.tri == 0.0
    }

    /// The naive value at phase `t` with pulse width `pw`.
    #[inline]
    fn value(&self, t: f32, pw: f32) -> f32 {
        let mut v = 0.0;
        if self.saw != 0.0 {
            let s = 2.0 * t - 1.0;
            v += self.saw * (s + self.curve * (s * s - 1.0 / 3.0));
        }
        if self.pulse != 0.0 {
            let p = if t < pw { 1.0 } else { -1.0 };
            v += self.pulse * (p - (2.0 * pw - 1.0));
        }
        if self.tri != 0.0 {
            let x = if t < 0.5 {
                4.0 * t - 1.0
            } else {
                3.0 - 4.0 * t
            };
            v += self.tri * ((1.0 + self.round) * x - self.round * x * x * x);
        }
        v
    }

    /// The slope, per cycle, at phase `t`.
    #[inline]
    fn slope(&self, t: f32) -> f32 {
        let mut d = 0.0;
        if self.saw != 0.0 {
            d += self.saw * (2.0 + 4.0 * self.curve * (2.0 * t - 1.0));
        }
        if self.tri != 0.0 {
            let (x, dx) = if t < 0.5 {
                (4.0 * t - 1.0, 4.0)
            } else {
                (3.0 - 4.0 * t, -4.0)
            };
            d += self.tri * dx * ((1.0 + self.round) - 3.0 * self.round * x * x);
        }
        d
    }
}

/// Where an oscillator's phase crossed its wrap and its half-way point this sample, as `τ`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Crossings {
    pub wrap: Option<f32>,
    pub half: Option<f32>,
}
