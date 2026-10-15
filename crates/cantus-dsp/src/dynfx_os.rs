//! Oversampling for the dynamics family (ADR 0056): 2×, 4×, 8× or 16× around a waveshaper,
//! built so that **what the shaper does not change, the oversampling does not touch**.
//!
//! - [`Up`] is a polyphase Kaiser-windowed-sinc interpolator whose phase 0 is a **pure
//!   delay** (the prototype is exactly 1 at its centre and exactly 0 at every multiple of
//!   the factor from it, as Terminus's): the first of each sample's points *is* the input
//!   sample [`HALF`] samples ago, to the bit.
//! - [`Down`] band-limits a **residual** — what the shaper changed, `shape(u) − u`, at the
//!   high rate — back to the base rate. The unit adds it to the input delayed by
//!   [`latency`]: when nothing was shaped the residual is exact zeros, the decimator holds
//!   only zeros and adds nothing, and the output is the input delayed, bit for bit. The
//!   filter's ripple and transition touch only the distortion, never the dry signal.
//!
//! One table per factor, computed once ([`Tables`]); per channel a ring for each side.
//! Latency at factor > 1 is `HALF + DOWN_HALF` base samples, whatever the factor (one
//! number for delay compensation per *switch position* is still a change; the units keep
//! it constant per configuration).

use crate::math;

/// The largest factor.
pub const MAX_FACTOR: usize = 16;
/// The factors, by a parameter's index (0 … 4).
pub const FACTORS: [usize; 5] = [1, 2, 4, 8, 16];
/// The interpolator's half-length in base samples (its delay).
pub const HALF: usize = 16;
/// The decimator's half-length in base samples (its delay).
pub const DOWN_HALF: usize = 24;
/// Taps of each interpolating phase.
const PHASE_TAPS: usize = 2 * HALF;
/// The interpolator's history.
const TAPS: usize = 2 * HALF + 1;
/// The longest decimator (16×), padded to a multiple of 8.
const MAX_DOWN: usize = (2 * DOWN_HALF * MAX_FACTOR + 1).div_ceil(8) * 8;
/// Kaiser β of the interpolator (Terminus's: ±0.02 dB to 0.44 fs) and of the decimator
/// (about 80 dB down from the base rate's Nyquist frequency).
const UP_BETA: f64 = 6.0;
const DOWN_BETA: f64 = 8.0;
/// The decimator's cut-off, a fraction of the base rate: its stop band starts at ½.
const DOWN_CUTOFF: f64 = 0.447;

/// Delay of the oversampled path at `factor`, in base samples: 0 at 1×.
pub fn latency(factor: usize) -> usize {
    if factor > 1 {
        HALF + DOWN_HALF
    } else {
        0
    }
}

/// The factor of a parameter's index (clamped).
pub fn factor_of(index: f32) -> usize {
    FACTORS[(index.round().max(0.0) as usize).min(FACTORS.len() - 1)]
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..80 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

fn kaiser(r: f64, beta: f64) -> f64 {
    bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / bessel_i0(beta)
}

/// One factor's filters.
#[derive(Clone, Debug)]
struct Table {
    /// Phases 1 … F−1: the taps in the order of the history window (from its
    /// second-oldest sample).
    up: [[f32; PHASE_TAPS]; MAX_FACTOR - 1],
    /// The decimator's taps, newest first, `down_len` of them (the rest zero).
    down: [f32; MAX_DOWN],
    down_len: usize,
}

impl Table {
    fn new(factor: usize) -> Table {
        let f = factor as f64;
        let mut up = [[0f32; PHASE_TAPS]; MAX_FACTOR - 1];
        // The prototype at the high rate: h[k] for k = 0 … 2·HALF·F, centre HALF·F.
        let len = 2 * HALF * factor + 1;
        let c = (len - 1) / 2;
        let proto = |k: usize| -> f64 {
            let off = k as i64 - c as i64;
            if off == 0 {
                return 1.0;
            }
            if off % factor as i64 == 0 {
                return 0.0;
            }
            let t = off as f64 / f;
            let sinc = math::sin(std::f64::consts::PI * t) / (std::f64::consts::PI * t);
            sinc * kaiser(off as f64 / c as f64, UP_BETA)
        };
        for (p, row) in up.iter_mut().enumerate().take(factor.saturating_sub(1)) {
            let p = p + 1;
            for (m, tap) in row.iter_mut().enumerate() {
                // Output p = Σ_j h[p + F·j] · x[n − j]; window index m + 1 holds
                // x[n − (TAPS − 2 − m)].
                let j = TAPS - 2 - m;
                let k = p + factor * j;
                *tap = if k < len { proto(k) as f32 } else { 0.0 };
            }
        }
        // The decimator: a low-pass at the high rate, unity at DC, symmetric.
        let mut down = [0f32; MAX_DOWN];
        let down_len = if factor > 1 {
            2 * DOWN_HALF * factor + 1
        } else {
            0
        };
        if down_len > 0 {
            let dc = (down_len - 1) / 2;
            let fc = DOWN_CUTOFF / f;
            let mut taps = [0f64; MAX_DOWN];
            let mut sum = 0.0;
            for (k, t) in taps.iter_mut().enumerate().take(down_len) {
                let m = k as f64 - dc as f64;
                let x = 2.0 * fc * m;
                let sinc = if m == 0.0 {
                    1.0
                } else {
                    math::sin(std::f64::consts::PI * x) / (std::f64::consts::PI * x)
                };
                *t = sinc * kaiser(m / dc as f64, DOWN_BETA);
                sum += *t;
            }
            for (d, t) in down.iter_mut().zip(&taps).take(down_len) {
                *d = (*t / sum) as f32;
            }
        }
        Table { up, down, down_len }
    }
}

/// Every factor's filters, made once (off the audio thread).
#[derive(Clone, Debug)]
pub struct Tables {
    t: Box<[Table; 5]>,
}

impl Default for Tables {
    fn default() -> Self {
        Self::new()
    }
}

impl Tables {
    pub fn new() -> Tables {
        Tables {
            t: Box::new(FACTORS.map(Table::new)),
        }
    }

    fn of(&self, factor: usize) -> &Table {
        let i = FACTORS.iter().position(|f| *f == factor).unwrap_or(0);
        &self.t[i]
    }
}

/// A dot product of fixed length, a multiple of 8: eight partial sums added in a fixed
/// order, so it vectorises and gives the same bits on every run.
#[inline]
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut s = [0f32; 8];
    for (ca, cb) in a.chunks_exact(8).zip(b.chunks_exact(8)) {
        for l in 0..8 {
            s[l] += ca[l] * cb[l];
        }
    }
    ((s[0] + s[4]) + (s[1] + s[5])) + ((s[2] + s[6]) + (s[3] + s[7]))
}

/// The interpolator of one channel.
#[derive(Clone, Debug)]
pub struct Up {
    ring: [f32; 2 * TAPS],
    pos: usize,
}

impl Default for Up {
    fn default() -> Self {
        Up {
            ring: [0.0; 2 * TAPS],
            pos: 0,
        }
    }
}

impl Up {
    pub fn reset(&mut self) {
        *self = Up::default();
    }

    pub fn is_clear(&self) -> bool {
        crate::rest::all_zero(&self.ring)
    }

    /// Push `x` and write the `factor` points of the waveform at `n − HALF + p/F` into
    /// `out[..factor]`; point 0 is `x[n − HALF]` exactly.
    #[inline]
    pub fn push(&mut self, x: f32, factor: usize, tables: &Tables, out: &mut [f32; MAX_FACTOR]) {
        self.ring[self.pos] = x;
        self.ring[self.pos + TAPS] = x;
        self.pos = if self.pos + 1 == TAPS { 0 } else { self.pos + 1 };
        let w = &self.ring[self.pos..self.pos + TAPS];
        out[0] = w[TAPS - 1 - HALF];
        if factor > 1 {
            let t = tables.of(factor);
            let tail = &w[1..];
            for (o, c) in out[1..factor].iter_mut().zip(t.up.iter()) {
                *o = dot(c, tail);
            }
        }
    }
}

/// The residual's decimator of one channel.
#[derive(Clone, Debug)]
pub struct Down {
    ring: Box<[f32; 2 * MAX_DOWN]>,
    pos: usize,
    /// High-rate zeros pushed in a row: at `len` the ring holds only zeros.
    zeros: usize,
}

impl Default for Down {
    fn default() -> Self {
        Down {
            ring: Box::new([0.0; 2 * MAX_DOWN]),
            pos: 0,
            zeros: usize::MAX,
        }
    }
}

impl Down {
    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.pos = 0;
        self.zeros = usize::MAX;
    }

    /// Only zeros in the history (cheap: counted as they are pushed).
    pub fn is_clear(&self) -> bool {
        self.zeros >= MAX_DOWN
    }

    /// Push one base sample's `factor` residual points (oldest first) and return the
    /// band-limited residual `DOWN_HALF` base samples back. Exact zeros while the history
    /// holds only zeros.
    #[inline]
    pub fn push(&mut self, pts: &[f32; MAX_FACTOR], factor: usize, tables: &Tables) -> f32 {
        let t = tables.of(factor);
        let len = t.down_len;
        if len == 0 {
            return 0.0;
        }
        let mut all_zero = true;
        for &v in &pts[..factor] {
            all_zero &= v.to_bits() == 0;
            self.ring[self.pos] = v;
            self.ring[self.pos + MAX_DOWN] = v;
            self.pos = if self.pos + 1 == MAX_DOWN { 0 } else { self.pos + 1 };
        }
        self.zeros = if all_zero {
            self.zeros.saturating_add(factor)
        } else {
            0
        };
        if self.zeros >= len {
            return 0.0;
        }
        // The newest `len` points against the taps (symmetric: the order does not matter).
        let start = self.pos + MAX_DOWN - len;
        dot_offset(&t.down[..len], &self.ring[start..start + len])
    }
}

/// A dot product of two slices of the same (any) length, in a fixed order.
#[inline]
fn dot_offset(a: &[f32], b: &[f32]) -> f32 {
    let n8 = a.len() / 8 * 8;
    let mut acc = dot(&a[..n8], &b[..n8]);
    for (x, y) in a[n8..].iter().zip(&b[n8..]) {
        acc += x * y;
    }
    acc
}

#[cfg(test)]
#[path = "dynfx_os_tests.rs"]
mod tests;
