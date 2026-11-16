//! Purus's de-crackle: the small, dense impulses of a worn record taken out from under loud
//! music by **subtracting each impulse** — never by rebuilding the samples it sits on.
//!
//! Rebuilding the samples under an impulse (the de-click's way, the editor's) costs about as
//! much as a small impulse under loud, bright music: what the model cannot predict of the
//! music is lost with it. An impulse is short, so most of its energy is high; most music's
//! is not. So:
//!
//! - **Find** it in the high band: the window is filtered by a linear-phase high-pass `h`
//!   (a windowed-sinc low-pass of [`CUT_HZ`] taken from a unit impulse; `2·H + 1` taps,
//!   `H` = 16 at 48 kHz), where an impulse stands far above the music. A sample is a
//!   suspect where the high band's magnitude is a local peak (±3 samples) above `k` times
//!   the block's robust scale (its median magnitude / 0.6745).
//! - **Estimate** it: the impulse is a few samples of unknown amplitude, `u_j` at offsets
//!   −1, 0, 1, 2 from the peak; seen in the high band as `Σ u_j·h(t − j)`. The amplitudes
//!   are the least-squares fit of that shape to the high band around the peak — the system's
//!   matrix is the high-pass's own autocorrelation, the same at every peak, factored once.
//! - **Accept** it only if the shape explains at least [`EXPLAINED`] of the high band's
//!   energy over its support: a transient of the music (a pick's attack, a consonant) is
//!   longer and is not explained by a four-sample impulse.
//! - **Subtract** it — the whole impulse, all its frequencies, `u_j` from the samples it
//!   sits on — and take its shape out of the high band, so the next suspect is judged
//!   without it. The music under it is changed only by the fit's error (the music's own
//!   high band, projected on an impulse's shape).
//!
//! It works in the de-click's window (`purus_click.rs`): no latency of its own.

use crate::math;

/// The high band's lower edge, Hz.
pub const CUT_HZ: f32 = 5_000.0;
/// The impulse's samples fitted: offsets −1 … 2 from the peak.
pub const SHAPE: usize = 4;
const FIRST: isize = -1;
/// The share of the high band's energy over an impulse's support its fitted shape must
/// explain.
pub const EXPLAINED: f64 = 0.6;
/// Suspects closer than this to the last impulse taken are judged with it.
const SPACING: usize = 4;

/// The de-crackle's fixed parts: the high-pass and the fit's factored matrix.
#[derive(Clone, Debug)]
pub struct Crackle {
    /// The half-length of the high-pass, and its taps `h[i + H]`, i = −H … H.
    half: usize,
    h: Vec<f64>,
    /// The Cholesky factor of the fit's matrix (lower, row-major).
    l: [f64; SHAPE * SHAPE],
}

impl Crackle {
    pub fn new(sample_rate: f32) -> Self {
        let half = ((16.0 * sample_rate / 48_000.0).round() as usize).clamp(8, 64);
        let n = 2 * half + 1;
        let fc = (CUT_HZ / sample_rate) as f64;
        // A Blackman-windowed sinc low-pass of unit gain at DC, taken from an impulse.
        let mut lp: Vec<f64> = (0..n)
            .map(|k| {
                let i = k as f64 - half as f64;
                let sinc = if i == 0.0 {
                    2.0 * fc
                } else {
                    math::sin(2.0 * std::f64::consts::PI * fc * i) / (std::f64::consts::PI * i)
                };
                let p = 2.0 * std::f64::consts::PI * k as f64 / (n - 1) as f64;
                sinc * (0.42 - 0.5 * math::cos(p) + 0.08 * math::cos(2.0 * p))
            })
            .collect();
        let dc: f64 = lp.iter().sum();
        lp.iter_mut().for_each(|v| *v /= dc);
        let h: Vec<f64> = lp
            .iter()
            .enumerate()
            .map(|(k, v)| if k == half { 1.0 - v } else { -v })
            .collect();
        // The fit's matrix: the high-pass's autocorrelation at the offsets' differences.
        let ac = |lag: usize| -> f64 { (0..n - lag).map(|k| h[k] * h[k + lag]).sum() };
        let mut g = [0.0f64; SHAPE * SHAPE];
        for a in 0..SHAPE {
            for b in 0..SHAPE {
                g[a * SHAPE + b] = ac(a.abs_diff(b));
            }
        }
        let mut l = [0.0f64; SHAPE * SHAPE];
        for j in 0..SHAPE {
            let mut s = g[j * SHAPE + j];
            for k in 0..j {
                s -= l[j * SHAPE + k] * l[j * SHAPE + k];
            }
            let d = s.max(1e-12).sqrt();
            l[j * SHAPE + j] = d;
            for i in j + 1..SHAPE {
                let mut v = g[i * SHAPE + j];
                for k in 0..j {
                    v -= l[i * SHAPE + k] * l[j * SHAPE + k];
                }
                l[i * SHAPE + j] = v / d;
            }
        }
        Crackle { half, h, l }
    }

    /// How far before and after a block the window must reach, samples.
    pub fn reach(&self) -> usize {
        2 * self.half + SHAPE + 4
    }

    /// Take the crackle out of `win[b0..b1]` at `k` robust scales, `hp` and `mag` scratch
    /// (`hp` as long as `win`, `mag` at least `b1 − b0`); how many impulses were taken.
    pub fn run(
        &self,
        win: &mut [f32],
        b0: usize,
        b1: usize,
        k: f64,
        hp: &mut [f64],
        mag: &mut [f64],
    ) -> usize {
        let hh = self.half;
        let lo = b0 - hh - SHAPE;
        let hi = b1 + hh + SHAPE;
        // The high band.
        for t in lo..hi {
            let mut v = 0.0;
            for (i, c) in self.h.iter().enumerate() {
                v += c * win[t + hh - i] as f64;
            }
            hp[t] = v;
        }
        let n = b1 - b0;
        for (m, v) in mag[..n].iter_mut().zip(&hp[b0..b1]) {
            *m = v.abs();
        }
        mag[..n].sort_unstable_by(|p, q| p.total_cmp(q));
        let sigma = mag[n / 2] / 0.6745;
        if sigma <= 1e-12 {
            return 0;
        }
        let bar = k * sigma;
        let mut taken = 0;
        let mut t = b0;
        while t < b1 {
            let v = hp[t].abs();
            let peak = v > bar && (t - 3..=t + 3).all(|s| hp[s].abs() <= v);
            if !peak {
                t += 1;
                continue;
            }
            // The fit: rhs_j = Σ_m h[m]·hp[t + j + m].
            let mut u = [0.0f64; SHAPE];
            for (j, r) in u.iter_mut().enumerate() {
                let at = t as isize + FIRST + j as isize;
                let mut acc = 0.0;
                for (i, c) in self.h.iter().enumerate() {
                    acc += c * hp[(at + i as isize - hh as isize) as usize];
                }
                *r = acc;
            }
            let rhs = u;
            self.solve(&mut u);
            let fit: f64 = u.iter().zip(&rhs).map(|(a, b)| a * b).sum();
            let s0 = (t as isize + FIRST - hh as isize) as usize;
            let s1 = t + (SHAPE - 1) + hh + 1;
            let local: f64 = hp[s0..s1].iter().map(|v| v * v).sum();
            if fit >= EXPLAINED * local {
                for (j, a) in u.iter().enumerate() {
                    let at = (t as isize + FIRST + j as isize) as usize;
                    win[at] -= *a as f32;
                    for (i, c) in self.h.iter().enumerate() {
                        hp[at + i - hh] -= a * c;
                    }
                }
                taken += 1;
                t += SPACING;
            } else {
                t += 1;
            }
        }
        taken
    }

    fn solve(&self, x: &mut [f64; SHAPE]) {
        let l = &self.l;
        for i in 0..SHAPE {
            let mut v = x[i];
            for k in 0..i {
                v -= l[i * SHAPE + k] * x[k];
            }
            x[i] = v / l[i * SHAPE + i];
        }
        for i in (0..SHAPE).rev() {
            let mut v = x[i];
            for k in i + 1..SHAPE {
                v -= l[k * SHAPE + i] * x[k];
            }
            x[i] = v / l[i * SHAPE + i];
        }
    }
}
