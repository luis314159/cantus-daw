//! The modulation family's Hilbert pair (ADR 0055): two chains of second-order allpass
//! sections whose outputs stay 90° apart over the audio band — the analytic signal a
//! single-sideband frequency shifter (Translatio) multiplies by a quadrature carrier.
//!
//! **The design** is the classic polyphase half-band one (Valenzuela & Constantinides,
//! 1983: an elliptic half-band filter as two allpass branches in `z²`), turned into a
//! 90° phase splitter by the substitution `z → j·z`: each section becomes
//! `(c − z⁻²)/(1 − c·z⁻²)`, the coefficients alternate between the two chains, and one chain
//! is one sample late. The coefficients come from closed-form elliptic-function series of
//! the transition width (our own implementation of the published maths), computed for the
//! sample rate when the unit is built: the 90° band starts at [`LOW_EDGE`] Hz at any rate.
//!
//! **How good**: a frequency shifter's unwanted sideband sits at `tan(ε/2)` of the wanted
//! one for a phase error `ε`; the design takes as many sections as it needs for
//! [`TARGET_DB`] of rejection from [`LOW_EDGE`] Hz to 20 kHz (or 0.45·fs), and the tests
//! measure it at 44.1 and 48 kHz — through the filter's exact response and through the
//! running shifter.

use crate::math;

/// The lowest frequency the 90° band must cover, Hz.
pub const LOW_EDGE: f64 = 20.0;
/// The highest, Hz (or 0.45 of the sample rate, if lower).
pub const HIGH_EDGE: f64 = 20_000.0;
/// The rejection the design aims for over the band, dB (the requirement is 60).
pub const TARGET_DB: f64 = 66.0;
/// The most sections the design may take (both chains together).
pub const MAX_COEFS: usize = 24;

/// The coefficients of a 90° pair for one sample rate.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Design {
    pub coefs: [f64; MAX_COEFS],
    pub n: usize,
    /// The worst rejection over the band, dB, as designed (the exact response).
    pub rejection_db: f64,
}

/// The elliptic series' parameters of a transition width (normalised to fs).
fn transition_param(transition: f64) -> (f64, f64) {
    let k = math::tan((1.0 - transition * 2.0) * std::f64::consts::FRAC_PI_4);
    let k = k * k;
    let kksqrt = math::powf(1.0 - k * k, 0.25);
    let e = 0.5 * (1.0 - kksqrt) / (1.0 + kksqrt);
    let e2 = e * e;
    let e4 = e2 * e2;
    let q = e * (1.0 + e4 * (2.0 + e4 * (15.0 + 150.0 * e4)));
    (k, q)
}

fn acc_num(q: f64, order: usize, c: usize) -> f64 {
    let mut acc = 0.0;
    let mut sign = 1.0;
    for i in 0..64 {
        let term = math::powi(q, (i * (i + 1)) as i32)
            * math::sin(((i * 2 + 1) * c) as f64 * std::f64::consts::PI / order as f64)
            * sign;
        acc += term;
        sign = -sign;
        if term.abs() < 1e-100 {
            break;
        }
    }
    acc
}

fn acc_den(q: f64, order: usize, c: usize) -> f64 {
    let mut acc = 0.0;
    let mut sign = -1.0;
    for i in 1..64 {
        let term = math::powi(q, (i * i) as i32)
            * math::cos((i * 2 * c) as f64 * std::f64::consts::PI / order as f64)
            * sign;
        acc += term;
        sign = -sign;
        if term.abs() < 1e-100 {
            break;
        }
    }
    acc
}

/// The `n` coefficients of the half-band design with transition width `transition`.
pub fn coefficients(n: usize, transition: f64) -> [f64; MAX_COEFS] {
    let (k, q) = transition_param(transition);
    let order = n * 2 + 1;
    let mut out = [0.0; MAX_COEFS];
    for (index, c) in out.iter_mut().enumerate().take(n.min(MAX_COEFS)) {
        let num = acc_num(q, order, index + 1) * math::powf(q, 0.25);
        let den = acc_den(q, order, index + 1) + 0.5;
        let ww = num / den;
        let wwsq = ww * ww;
        let x = ((1.0 - wwsq * k) * (1.0 - wwsq / k)).sqrt() / (1.0 + wwsq);
        *c = (1.0 - x) / (1.0 + x);
    }
    out
}

/// The phase of one section `(c − z⁻²)/(1 − c·z⁻²)` at `w` radians per sample.
fn section_phase(c: f64, w: f64) -> f64 {
    // Numerator c − e^{−2jw}, denominator 1 − c·e^{−2jw}.
    let (s2, c2) = math::sin_cos(2.0 * w);
    let num = math::atan2(s2, c - c2);
    let den = math::atan2(c * s2, 1.0 - c * c2);
    num - den
}

/// The phase difference between the two chains at `w` (the second chain's sample of delay
/// included), radians.
pub fn phase_difference(coefs: &[f64], w: f64) -> f64 {
    let mut a = 0.0;
    let mut b = -w;
    for (i, c) in coefs.iter().enumerate() {
        if i % 2 == 0 {
            a += section_phase(*c, w);
        } else {
            b += section_phase(*c, w);
        }
    }
    a - b
}

/// The rejection of the unwanted sideband at a phase difference `d`, dB (positive).
pub fn rejection_of(d: f64) -> f64 {
    // Wanted ∝ |1 + e^{j(d − π/2)}|, unwanted ∝ |1 − e^{j(d − π/2)}|: their ratio is
    // tan(ε/2) for an error ε from 90°.
    // Either sign of quadrature will do: which one is the upper sideband is the shifter's.
    let wrap = |e: f64| e - (e / std::f64::consts::TAU).round() * std::f64::consts::TAU;
    let e = wrap(d - std::f64::consts::FRAC_PI_2)
        .abs()
        .min(wrap(d + std::f64::consts::FRAC_PI_2).abs());
    let r = math::tan(e * 0.5);
    if r <= 0.0 {
        300.0
    } else {
        -20.0 * math::log10(r)
    }
}

/// The worst rejection of `coefs` over `lo … hi` Hz at `sample_rate`, on a log grid.
pub fn worst_rejection(coefs: &[f64], sample_rate: f64, lo: f64, hi: f64) -> f64 {
    let points = 200;
    let mut worst = f64::INFINITY;
    for i in 0..=points {
        let f = lo * math::powf(hi / lo, i as f64 / points as f64);
        let w = std::f64::consts::TAU * f / sample_rate;
        worst = worst.min(rejection_of(phase_difference(coefs, w)));
    }
    worst
}

/// The design for `sample_rate`: the fewest sections that reach [`TARGET_DB`] over the
/// band, the transition chosen so the band starts at [`LOW_EDGE`].
pub fn design(sample_rate: f32) -> Design {
    let sr = f64::from(sample_rate);
    let hi = HIGH_EDGE.min(0.45 * sr);
    let mut best = Design {
        coefs: [0.0; MAX_COEFS],
        n: 0,
        rejection_db: 0.0,
    };
    // The band edge at the transition: a little below the lowest frequency asked for.
    let transition = LOW_EDGE * 0.8 / sr;
    for n in (4..=MAX_COEFS).step_by(2) {
        let coefs = coefficients(n, transition);
        let r = worst_rejection(&coefs[..n], sr, LOW_EDGE, hi);
        if r > best.rejection_db {
            best = Design {
                coefs,
                n,
                rejection_db: r,
            };
        }
        if r >= TARGET_DB {
            break;
        }
    }
    best
}

/// One chain's section state: the last two inputs and outputs.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Section {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

/// A running 90° pair on one channel.
#[derive(Clone, Debug)]
pub struct Hilbert {
    c: [f32; MAX_COEFS],
    n: usize,
    s: [Section; MAX_COEFS],
    /// The second chain's sample of delay.
    late: f32,
}

impl Hilbert {
    pub fn new(d: &Design) -> Hilbert {
        let mut c = [0.0f32; MAX_COEFS];
        for (o, v) in c.iter_mut().zip(&d.coefs[..d.n]) {
            *o = *v as f32;
        }
        Hilbert {
            c,
            n: d.n,
            s: [Section::default(); MAX_COEFS],
            late: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.s = [Section::default(); MAX_COEFS];
        self.late = 0.0;
    }

    /// The pair for one input sample: `(i, q)`, `q` 90° behind `i`.
    #[inline]
    pub fn run(&mut self, x: f32) -> (f32, f32) {
        let (mut a, mut b) = (x, x);
        for k in 0..self.n {
            let c = self.c[k];
            let s = &mut self.s[k];
            let input = if k % 2 == 0 { a } else { b };
            // y = c·(x + y[n−2]) − x[n−2]
            let y = c * (input + s.y2) - s.x2;
            s.x2 = s.x1;
            s.x1 = input;
            s.y2 = s.y1;
            s.y1 = y;
            if k % 2 == 0 {
                a = y;
            } else {
                b = y;
            }
        }
        let q = self.late;
        self.late = b;
        (a, q)
    }

    /// Every state is +0.0.
    pub fn is_zero(&self) -> bool {
        self.late.to_bits() == 0
            && self.s[..self.n].iter().all(|s| {
                s.x1.to_bits() == 0 && s.x2.to_bits() == 0 && s.y1.to_bits() == 0 && s.y2.to_bits() == 0
            })
    }

    /// The largest state, in magnitude.
    pub fn level(&self) -> f32 {
        self.s[..self.n]
            .iter()
            .fold(self.late.abs(), |m, s| m.max(s.y1.abs()).max(s.y2.abs()).max(s.x1.abs()).max(s.x2.abs()))
    }
}

#[cfg(test)]
#[path = "modfx_hilbert_tests.rs"]
mod tests;
