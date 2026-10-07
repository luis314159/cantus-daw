//! Modulus's tables and its decimator: one sine table, band-limited waves per octave, and
//! the polyphase IIR half-band filters that bring an oversampled voice back to the host
//! rate.
//!
//! The tables are built once per process (the first unit made, on the main thread) and
//! shared; nothing here allocates on the audio thread. The sine is our own table, never
//! libm's `sin` on the audio path, so a render is the same on every OS.

use crate::math;
use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

/// Sine table: 4096 points and a guard point, read with linear interpolation from the top
/// 12 bits of a 32-bit phase. Worst error about −130 dB below full scale.
pub const SINE_BITS: u32 = 12;
pub const SINE_LEN: usize = 1 << SINE_BITS;
/// Band-limited waves: 2048 points per table.
pub const WAVE_BITS: u32 = 11;
pub const WAVE_LEN: usize = 1 << WAVE_BITS;
/// Mip levels per wave: level *k* holds harmonics up to `2^k`, `k` = 0…10 (1…1024).
pub const WAVE_LEVELS: usize = 11;
/// Waves with tables (the sine has its own): triangle, saw, square.
pub const TABLE_WAVES: usize = 3;

/// Everything read-only the voices share.
#[derive(Debug)]
pub struct Tables {
    pub sine: Vec<f32>,
    /// `waves[w][level]`, each `WAVE_LEN + 1` long.
    pub waves: Vec<Vec<Vec<f32>>>,
    /// Half-band coefficients (the Valenzuela–Constantinides polyphase design).
    pub halfband: [f32; HB_COEFS],
}

pub fn tables() -> Arc<Tables> {
    static T: OnceLock<Arc<Tables>> = OnceLock::new();
    T.get_or_init(|| Arc::new(Tables::build())).clone()
}

impl Tables {
    fn build() -> Tables {
        let sine = (0..=SINE_LEN)
            .map(|i| math::sin(2.0 * PI * i as f64 / SINE_LEN as f64) as f32)
            .collect();
        let waves = (0..TABLE_WAVES)
            .map(|w| (0..WAVE_LEVELS).map(|l| wave_table(w, 1 << l)).collect())
            .collect();
        Tables {
            sine,
            waves,
            halfband: design_halfband(HB_COEFS, HB_TRANSITION),
        }
    }

    /// The sine at a 32-bit phase.
    #[inline(always)]
    pub fn sin(&self, phase: u32) -> f32 {
        let i = (phase >> (32 - SINE_BITS)) as usize;
        let f = (phase & ((1 << (32 - SINE_BITS)) - 1)) as f32
            * (1.0 / (1u32 << (32 - SINE_BITS)) as f32);
        // SAFETY-free indexing: i < SINE_LEN and the table has a guard point.
        let a = self.sine[i];
        let b = self.sine[i + 1];
        a + (b - a) * f
    }

    /// A table wave (`w` = 0 triangle, 1 saw, 2 square) at mip `level`.
    #[inline(always)]
    pub fn wave(&self, w: usize, level: usize, phase: u32) -> f32 {
        let t = &self.waves[w][level];
        let i = (phase >> (32 - WAVE_BITS)) as usize;
        let f = (phase & ((1 << (32 - WAVE_BITS)) - 1)) as f32
            * (1.0 / (1u32 << (32 - WAVE_BITS)) as f32);
        let a = t[i];
        a + (t[i + 1] - a) * f
    }
}

/// The mip level for an operator at `hz` running at `rate`: the most harmonics that stay
/// below its Nyquist.
pub fn wave_level(hz: f32, rate: f32) -> usize {
    let h = (0.5 * rate / hz.abs().max(1.0)).max(1.0);
    (math::log2(h).floor() as usize).min(WAVE_LEVELS - 1)
}

/// Additive: triangle (odd, alternating 1/n²), saw (1/n), square (odd 1/n), with a gentle
/// taper on the top harmonics against the Gibbs ripple, normalised to a peak of 1.
fn wave_table(w: usize, harmonics: usize) -> Vec<f32> {
    let mut t = vec![0f64; WAVE_LEN + 1];
    for n in 1..=harmonics {
        let taper = {
            let x = n as f64 / (harmonics as f64 + 1.0);
            math::powi(math::cos(0.5 * PI * x), 2).max(0.0) * 0.5 + 0.5 * (1.0 - x)
        };
        let a = match w {
            0 if n % 2 == 1 => {
                let s = if (n / 2) % 2 == 0 { 1.0 } else { -1.0 };
                s / (n * n) as f64
            }
            1 => 1.0 / n as f64,
            2 if n % 2 == 1 => 1.0 / n as f64,
            _ => 0.0,
        } * if harmonics == 1 { 1.0 } else { taper };
        if a == 0.0 {
            continue;
        }
        for (i, v) in t.iter_mut().enumerate() {
            *v += a * math::sin(2.0 * PI * n as f64 * i as f64 / WAVE_LEN as f64);
        }
    }
    let peak = t.iter().fold(0f64, |m, x| m.max(x.abs())).max(1e-12);
    t.iter().map(|x| (x / peak) as f32).collect()
}

// ---- the half-band decimator -------------------------------------------------------------

/// Coefficients per half-band stage.
pub const HB_COEFS: usize = 12;
/// Transition band, relative to the stage's input rate: the pass band ends at
/// `(0.25 − HB_TRANSITION / 2)·fs_in`, 0.45 of the output's Nyquist.
pub const HB_TRANSITION: f64 = 0.05;

/// Design a polyphase IIR half-band (two chains of first-order all-passes in z⁻²), after
/// Valenzuela and Constantinides (1983): the elliptic design written out for the all-pass
/// coefficients. Published mathematics; the public-domain HIIR library documents it too.
pub fn design_halfband(n: usize, transition: f64) -> [f32; HB_COEFS] {
    let order = (2 * n + 1) as f64;
    let mut k = math::tan((1.0 - transition * 2.0) * PI / 4.0);
    k *= k;
    let kksqrt = math::powf(1.0 - k * k, 0.25);
    let e = 0.5 * (1.0 - kksqrt) / (1.0 + kksqrt);
    let e4 = math::powi(e, 4);
    let q = e * (1.0 + e4 * (2.0 + e4 * (15.0 + 150.0 * e4)));
    let mut out = [0f32; HB_COEFS];
    for (index, c) in out.iter_mut().enumerate().take(n) {
        let ci = (index + 1) as f64;
        let mut num = 0.0;
        let mut i = 0i32;
        let mut sign = 1.0;
        loop {
            let term =
                math::powi(q, i * (i + 1)) * math::sin((2 * i + 1) as f64 * ci * PI / order) * sign;
            num += term;
            sign = -sign;
            i += 1;
            if term.abs() < 1e-100 || i > 64 {
                break;
            }
        }
        num *= math::powf(q, 0.25);
        let mut den = 0.0;
        let mut i = 1i32;
        let mut sign = -1.0;
        loop {
            let term = math::powi(q, i * i) * math::cos((2 * i) as f64 * ci * PI / order) * sign;
            den += term;
            sign = -sign;
            i += 1;
            if term.abs() < 1e-100 || i > 64 {
                break;
            }
        }
        den += 0.5;
        let ww = num / den;
        let wwsq = ww * ww;
        let x = ((1.0 - wwsq * k) * (1.0 - wwsq / k)).sqrt() / (1.0 + wwsq);
        *c = ((1.0 - x) / (1.0 + x)) as f32;
    }
    out
}

/// One 2:1 stage: two all-pass chains, the even coefficients on one, the odd on the other.
#[derive(Copy, Clone, Debug, Default)]
pub struct HalfBand {
    x: [f32; HB_COEFS],
    y: [f32; HB_COEFS],
}

impl HalfBand {
    /// Two input samples (oldest first) in, one out.
    #[inline(always)]
    pub fn process(&mut self, c: &[f32; HB_COEFS], in0: f32, in1: f32) -> f32 {
        let mut a = in1;
        let mut b = in0;
        let mut i = 0;
        while i < HB_COEFS {
            let ya = (a - self.y[i]) * c[i] + self.x[i];
            self.x[i] = a;
            self.y[i] = ya;
            a = ya;
            let yb = (b - self.y[i + 1]) * c[i + 1] + self.x[i + 1];
            self.x[i + 1] = b;
            self.y[i + 1] = yb;
            b = yb;
            i += 2;
        }
        0.5 * (a + b)
    }

    pub fn reset(&mut self) {
        *self = HalfBand::default();
    }

    /// Flush values too small to hear, so a quiet tail never turns denormal.
    #[inline]
    pub fn flush(&mut self) {
        for v in self.x.iter_mut().chain(self.y.iter_mut()) {
            if v.abs() < 1e-24 {
                *v = 0.0;
            }
        }
    }
}

/// Up to three stages: 8:1, 4:1 or 2:1 back to the host rate.
#[derive(Copy, Clone, Debug, Default)]
pub struct Decimator {
    stages: [HalfBand; 3],
}

impl Decimator {
    pub fn reset(&mut self) {
        for s in &mut self.stages {
            s.reset();
        }
    }

    /// Reduce `n` (1, 2, 4 or 8) samples to one. `buf` is used in place.
    #[inline]
    pub fn process(&mut self, c: &[f32; HB_COEFS], buf: &mut [f32; 8], n: usize) -> f32 {
        let mut len = n;
        let mut stage = 0;
        while len > 1 {
            let half = len / 2;
            for i in 0..half {
                buf[i] = self.stages[stage].process(c, buf[2 * i], buf[2 * i + 1]);
            }
            len = half;
            stage += 1;
        }
        buf[0]
    }

    pub fn flush(&mut self) {
        for s in &mut self.stages {
            s.flush();
        }
    }
}
