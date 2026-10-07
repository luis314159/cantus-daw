//! Spatium's spring tank: the Spring style's late field.
//!
//! A spring carries a wave that **disperses** — its low frequencies travel slower than its
//! highs, so every round trip arrives as a falling chirp, the "boing" of a guitar amp's
//! reverb. After Välimäki, Parker & Abel, *Parametric Spring Reverberation Effect* (JAES
//! 2010), each spring here is a loop of:
//!
//! - a chain of [`CHIRP_SECTIONS`] **stretched all-pass** sections
//!   `A(z) = (a + z⁻ᴷ) / (1 + a z⁻ᴷ)` with `a < 0`, which delays the lows more than the
//!   highs up to the spring's transition frequency `fs / 2K` (about 4 kHz);
//! - a delay line (the rest of the round trip);
//! - an absorption filter fitted like the room's ([`crate::spatium::fdn::Absorber`]), with
//!   the chain's own group delay added to the loop length, so the decay curve holds for
//!   springs too.
//!
//! Three springs of different lengths are fed the mono input; two feed one side each and
//! the third both, in opposite polarity, so the tank is stereo.

// The DSP here walks several parallel arrays (lines, sections, grid points) with one
// index; that reads clearer than zipping five iterators.
#![allow(clippy::needless_range_loop)]

use crate::math;
use crate::spatium::fdn::{glide, Absorber, Section, GRID, MAX_PASS_DB, SECTIONS};
use crate::spatium::model::TargetCurve;

pub const SPRINGS: usize = 3;
pub const CHIRP_SECTIONS: usize = 36;
/// The all-pass coefficient: how strong the dispersion is.
const CHIRP_A: f32 = -0.62;
/// The transition frequency, Hz: the chirp covers the band below it.
const TRANSITION_HZ: f32 = 4_200.0;
/// State ring for the stretched sections (≥ the largest K at 192 kHz).
const RING: usize = 32;
/// Spring lengths relative to the first.
const RATIOS: [f32; SPRINGS] = [1.0, 1.23, 1.41];

/// Group delay of one stretched all-pass section at `w` (rad/sample), samples.
fn chirp_delay(a: f64, k: usize, w: f64) -> f64 {
    let kw = k as f64 * w;
    k as f64 * (1.0 - a * a) / (1.0 + 2.0 * a * math::cos(kw) + a * a)
}

#[derive(Clone, Debug)]
pub struct SpringDesign {
    pub lengths: [f32; SPRINGS],
    pub coefs: [[Section; SECTIONS]; SPRINGS],
    pub g0: [f32; SPRINGS],
    /// The mean loop length including the chain's delay at 500 Hz, samples.
    pub mean_loop: f32,
}

/// The stretch factor for a sample rate.
pub fn stretch(sample_rate: f32) -> usize {
    ((sample_rate / (2.0 * TRANSITION_HZ)).round() as usize).clamp(1, RING - 1)
}

impl SpringDesign {
    /// `size_ms` is the first spring's delay-line length.
    pub fn new(absorber: &Absorber, curve: &TargetCurve, size_ms: f32) -> Self {
        let fs = absorber.sample_rate() as f64;
        let k = stretch(absorber.sample_rate());
        let mut lengths = [0.0f32; SPRINGS];
        let mut coefs = [[Section::IDENTITY; SECTIONS]; SPRINGS];
        let mut g0 = [1.0f32; SPRINGS];
        let mut mean_loop = 0.0;
        for s in 0..SPRINGS {
            lengths[s] = (size_ms * RATIOS[s] * 0.001 * absorber.sample_rate()).round();
            let mut t = [0.0f64; GRID];
            for (j, hz) in absorber.grid_hz().iter().enumerate() {
                let w = 2.0 * std::f64::consts::PI * hz / fs;
                let m =
                    lengths[s] as f64 + CHIRP_SECTIONS as f64 * chirp_delay(CHIRP_A as f64, k, w);
                let gamma = -60.0 / (fs * curve.t60(*hz as f32) as f64);
                t[j] = (m * gamma).clamp(-MAX_PASS_DB, -2e-4);
            }
            g0[s] = absorber.fit(&t, &mut coefs[s]);
            let w500 = 2.0 * std::f64::consts::PI * 500.0 / fs;
            mean_loop +=
                lengths[s] + (CHIRP_SECTIONS as f64 * chirp_delay(CHIRP_A as f64, k, w500)) as f32;
        }
        SpringDesign {
            lengths,
            coefs,
            g0,
            mean_loop: mean_loop / SPRINGS as f32,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpringTank {
    k: usize,
    n: usize,
    /// Per spring, per section: the last RING inputs and outputs.
    xs: Vec<f32>,
    ys: Vec<f32>,
    size: usize,
    mask: usize,
    line: Vec<f32>,
    w: usize,
    len: [f32; SPRINGS],
    len_target: [f32; SPRINGS],
    glide: f32,
    coefs: [[Section; SECTIONS]; SPRINGS],
    g0: [f32; SPRINGS],
    state: [[[f32; 4]; SECTIONS]; SPRINGS],
    fb: [f32; SPRINGS],
    freeze: f32,
    freeze_target: f32,
    smooth: f32,
    configured: bool,
}

impl SpringTank {
    pub fn new(sample_rate: f32, max_ms: f32) -> Self {
        let size =
            ((sample_rate * max_ms * 0.001 * RATIOS[SPRINGS - 1]) as usize + 8).next_power_of_two();
        SpringTank {
            k: stretch(sample_rate),
            n: 0,
            xs: vec![0.0; SPRINGS * CHIRP_SECTIONS * RING],
            ys: vec![0.0; SPRINGS * CHIRP_SECTIONS * RING],
            size,
            mask: size - 1,
            line: vec![0.0; size * SPRINGS],
            w: 0,
            len: [1000.0; SPRINGS],
            len_target: [1000.0; SPRINGS],
            glide: 1.0 - math::exp(-1.0 / (0.08 * sample_rate)),
            coefs: [[Section::IDENTITY; SECTIONS]; SPRINGS],
            g0: [0.5; SPRINGS],
            state: [[[0.0; 4]; SECTIONS]; SPRINGS],
            fb: [0.0; SPRINGS],
            freeze: 0.0,
            freeze_target: 0.0,
            smooth: 1.0 - math::exp(-1.0 / (0.05 * sample_rate)),
            configured: false,
        }
    }

    pub fn reset(&mut self) {
        self.xs.iter_mut().for_each(|x| *x = 0.0);
        self.ys.iter_mut().for_each(|x| *x = 0.0);
        self.line.iter_mut().for_each(|x| *x = 0.0);
        self.state = [[[0.0; 4]; SECTIONS]; SPRINGS];
        self.fb = [0.0; SPRINGS];
        self.n = 0;
        self.w = 0;
        self.len = self.len_target;
        self.freeze = self.freeze_target;
    }

    pub fn set_design(&mut self, d: &SpringDesign) {
        let cap = (self.size - 4) as f32;
        for s in 0..SPRINGS {
            self.len_target[s] = d.lengths[s].clamp(8.0, cap);
        }
        self.coefs = d.coefs;
        self.g0 = d.g0;
        if !self.configured {
            self.len = self.len_target;
            self.configured = true;
        }
    }

    pub fn set_freeze(&mut self, on: bool) {
        self.freeze_target = if on { 1.0 } else { 0.0 };
    }

    #[inline]
    pub fn tick(&mut self, input: f32) -> (f32, f32) {
        let k = self.k;
        let slot = self.n & (RING - 1);
        let old = self.n.wrapping_sub(k) & (RING - 1);
        glide(&mut self.freeze, self.freeze_target, self.smooth, 1e-5);
        let mut out = [0.0f32; SPRINGS];
        for s in 0..SPRINGS {
            // Read the loop's delay line (linear interpolation while the length glides).
            glide(&mut self.len[s], self.len_target[s], self.glide, 1e-3);
            let d = self.len[s];
            let n = d.floor();
            let f = d - n;
            let n = n as usize;
            let base = s * self.size;
            let a = self.line[base + (self.w.wrapping_sub(n) & self.mask)];
            let b = self.line[base + (self.w.wrapping_sub(n + 1) & self.mask)];
            let raw = a + (b - a) * f;
            // Absorption.
            let mut v = raw;
            for (c, st) in self.coefs[s].iter().zip(self.state[s].iter_mut()) {
                let d = c.d0 * v + c.d1 * st[0] + c.d2 * st[1] + 2.0 * st[2]
                    - st[3]
                    - c.e1 * st[2]
                    - c.e2 * st[3];
                let d = if d.abs() < 1e-18 { 0.0 } else { d };
                st[1] = st[0];
                st[0] = v;
                st[3] = st[2];
                st[2] = d;
                v += d;
            }
            v *= self.g0[s];
            if self.freeze > 0.0 {
                v += (raw * crate::spatium::fdn::FREEZE_GAIN - v) * self.freeze;
            }
            out[s] = v;
            // The chirp chain, then back into the line.
            let mut x = input * (1.0 - self.freeze) + v;
            for sec in 0..CHIRP_SECTIONS {
                let i = (s * CHIRP_SECTIONS + sec) * RING;
                let xo = self.xs[i + old];
                let yo = self.ys[i + old];
                let y = CHIRP_A * x + xo - CHIRP_A * yo;
                let y = if y.abs() < 1e-18 { 0.0 } else { y };
                self.xs[i + slot] = x;
                self.ys[i + slot] = y;
                x = y;
            }
            self.line[base + self.w] = x;
        }
        self.n = self.n.wrapping_add(1);
        self.w = (self.w + 1) & self.mask;
        (out[0] + 0.6 * out[2], out[1] - 0.6 * out[2])
    }
}
