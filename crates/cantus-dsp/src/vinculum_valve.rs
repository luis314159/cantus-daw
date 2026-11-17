//! The vari-mu's valve colour: what a push-pull valve stage adds, at twice the rate, with
//! no latency.
//!
//! **The curve.** A balanced push-pull stage cancels its even orders; what is left is odd,
//! led by the third. The colour is the cubic soft clipper `f(u) = u − u³/3` (`|u| ≤ 1`,
//! `±2/3` beyond), `u = x / h`, with the headroom `h` set so that a full-scale sine at full
//! colour and full depth has 1 % third harmonic: `A²/(12h²) = 0.01` at `A = 1`. The unit
//! scales the **residual** `h·(f(u) − u)` by a depth that rises with the valve's bias (the
//! reduction): a remote-cutoff valve driven further down its curve bends more.
//!
//! **Twice the rate, the residual only.** The input is interpolated to 2× by a polyphase
//! IIR half-band (two chains of first-order allpasses in `z⁻²`; Valenzuela and
//! Constantinides, *Digital signal processing schemes for efficient interpolation and
//! decimation*, IEE Proc. G 130:6, 1983 — the coefficients from their elliptic design,
//! computed here), shaped, and only the residual `f(u) − u` is decimated back by the same
//! half-band. The dry signal is never filtered: the colour adds the residual to it, so with
//! no colour nothing is touched, and nothing is delayed (latency 0 — the residual alone
//! carries the half-band's few samples of group delay). A cubic triples a band: at 2×
//! nothing under 16 kHz folds back, and what the curve makes above the base rate's Nyquist
//! frequency lies in the half-band's stop band (−96 dB).

use crate::math;

/// Allpass coefficients per half-band (six: −96 dB from 28 kHz at 48 kHz).
pub const COEFS: usize = 6;
/// The half-band's transition, a share of the doubled rate (passband to 0.2085).
pub const TRANSITION: f64 = 0.083;
/// The headroom: the input level, linear, at which the curve flattens (+9.2 dBFS).
pub const HEADROOM: f32 = 2.886_751_3; // √(1/0.12)

/// The elliptic half-band's allpass coefficients for `n` sections and a transition
/// `tb` (a share of the doubled rate): even indices on the first path, odd on the second.
pub fn halfband(n: usize, tb: f64) -> [f64; COEFS] {
    let mut out = [0.0; COEFS];
    let k = math::powi(math::tan((1.0 - 2.0 * tb) * std::f64::consts::PI / 4.0), 2);
    let kk = math::powf(1.0 - k * k, 0.25);
    let e = 0.5 * (1.0 - kk) / (1.0 + kk);
    let q = e + 2.0 * math::powi(e, 5) + 15.0 * math::powi(e, 9) + 150.0 * math::powi(e, 13);
    let order = (2 * n + 1) as f64;
    for (i, c) in out.iter_mut().enumerate().take(n) {
        let ci = (i + 1) as f64;
        let mut num = 0.0;
        let mut sign = 1.0;
        for m in 0..12 {
            let mf = m as f64;
            num += sign
                * math::powf(q, mf * (mf + 1.0))
                * math::sin((2.0 * mf + 1.0) * std::f64::consts::PI * ci / order);
            sign = -sign;
        }
        let mut den = 0.0;
        let mut sign = -1.0;
        for m in 1..12 {
            let mf = m as f64;
            den += sign
                * math::powf(q, mf * mf)
                * math::cos(2.0 * mf * std::f64::consts::PI * ci / order);
            sign = -sign;
        }
        let den = 1.0 + 2.0 * den;
        let w = 2.0 * math::powf(q, 0.25) * num / den;
        let ww = w * w;
        let x = ((1.0 - ww * k) * (1.0 - ww / k)).sqrt() / (1.0 + ww);
        *c = (1.0 - x) / (1.0 + x);
    }
    out
}

/// A chain of first-order allpasses `(a + z⁻¹)/(1 + a·z⁻¹)` at the base rate of its path.
#[derive(Copy, Clone, Debug, Default)]
struct Chain {
    /// Per section: the previous input and output.
    x1: [f32; COEFS / 2],
    y1: [f32; COEFS / 2],
}

impl Chain {
    #[inline]
    fn run(&mut self, a: &[f32; COEFS / 2], mut x: f32) -> f32 {
        for ((c, x1), y1) in a.iter().zip(&mut self.x1).zip(&mut self.y1) {
            let y = c * (x - *y1) + *x1;
            *x1 = x;
            *y1 = if y.abs() < 1e-20 { 0.0 } else { y };
            x = *y1;
        }
        x
    }

    fn is_zero(&self) -> bool {
        crate::rest::all_zero(&self.x1) && crate::rest::all_zero(&self.y1)
    }
}

/// One channel's colour: the interpolator's two paths, the decimator's two, and the odd
/// sample the decimator holds for a step.
#[derive(Copy, Clone, Debug, Default)]
struct Channel {
    up: [Chain; 2],
    down: [Chain; 2],
    odd: f32,
}

/// The valve colour of a stereo pair.
#[derive(Clone, Debug)]
pub struct Valve {
    /// The coefficients of the first and the second path.
    a: [[f32; COEFS / 2]; 2],
    ch: [Channel; 2],
}

impl Default for Valve {
    fn default() -> Self {
        Valve::new()
    }
}

/// The curve's residual `f(u) − u` (`u` in headroom units).
#[inline]
pub fn residual(u: f32) -> f32 {
    if u > 1.0 {
        2.0 / 3.0 - u
    } else if u < -1.0 {
        -2.0 / 3.0 - u
    } else {
        // `+ 0.0`: the cube of +0.0 negated is −0.0, which would keep the filters
        // from resting.
        -u * u * u / 3.0 + 0.0
    }
}

impl Valve {
    pub fn new() -> Valve {
        let c = halfband(COEFS, TRANSITION);
        let mut a = [[0f32; COEFS / 2]; 2];
        for (i, v) in c.iter().enumerate() {
            a[i % 2][i / 2] = *v as f32;
        }
        Valve {
            a,
            ch: [Channel::default(); 2],
        }
    }

    /// One sample of channel `c`: the colour's residual, linear, at full depth (the input
    /// itself is not returned — the caller adds the residual to its dry sample).
    #[inline]
    pub fn run(&mut self, c: usize, x: f32) -> f32 {
        let inv_h = 1.0 / HEADROOM;
        let ch = &mut self.ch[c];
        // Interpolate: the two phases of the doubled rate (the half-band's gain of 2).
        let u0 = ch.up[0].run(&self.a[0], x) * inv_h;
        let u1 = ch.up[1].run(&self.a[1], x) * inv_h;
        let r0 = residual(u0);
        let r1 = residual(u1);
        // Decimate: ½·(A₀(even) + A₁(odd one step back)).
        let d0 = ch.down[0].run(&self.a[0], r0);
        let d1 = ch.down[1].run(&self.a[1], ch.odd);
        ch.odd = r1;
        0.5 * (d0 + d1) * HEADROOM
    }

    /// Nothing held: silence in gives exact zeros out.
    pub fn is_zero(&self) -> bool {
        self.ch.iter().all(|c| {
            c.odd.to_bits() == 0
                && c.up.iter().all(Chain::is_zero)
                && c.down.iter().all(Chain::is_zero)
        })
    }

    pub fn reset(&mut self) {
        self.ch = [Channel::default(); 2];
    }
}

/// The same curve without oversampling, for the aliasing measurement.
#[inline]
pub fn naive(x: f32) -> f32 {
    residual(x / HEADROOM) * HEADROOM
}
