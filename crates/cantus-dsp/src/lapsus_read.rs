//! Lapsus's memory of the past and how it is read (ADR 0132, provisional).
//!
//! **The history** is a stereo ring the unit writes every input sample into, by its stream
//! sample; it is built off the audio thread ([`Ring::new`], handed in through the engine's
//! memory path, ADR 0022) and never grows there. A read names a position in stream samples
//! and the window it may see — no earlier than the cycle's start, the history's start (a
//! seek, a reset) or what the ring still holds, no later than now — and reads **zero**
//! outside it, so what was before the cycle never reaches it.
//!
//! **The interpolator** depends on how far behind *now* the position is, because a
//! symmetric kernel needs samples on both sides and the future is not written yet:
//!
//! - a whole sample is that sample, to the bit (the null);
//! - under one sample behind, a line between the last two; from two on, a four-point
//!   Hermite; from 16 on, a 32-tap Kaiser-windowed sinc (β 9, ≈ 90 dB) — crossfaded across
//!   1 … 2 and 16 … 24 samples by distance, so a tape stop leaving *now* changes kernel
//!   without a step;
//! - read faster than real time (a scratch's forward stroke, a rewind), the sinc's cut-off
//!   follows the speed — kernels at 1, ½ and ¼ of Nyquist (32, 64 and 128 taps), blended by
//!   the speed — so what would fold above Nyquist is filtered away first, where the history
//!   is far enough behind *now* to hold the longer kernel (`exemplar_resample`'s rule,
//!   without its mip-maps: the history is live).
//!
//! The kernels are tables built once per process (sample-rate free), off the audio thread.

use crate::math;
use crate::modfx_line::hermite;
use std::sync::OnceLock;

/// Phases in each kernel's table; between two, the coefficients are interpolated.
pub const PHASES: usize = 256;
/// The longest kernel.
pub const MAX_TAPS: usize = 128;
/// Behind *now* from which the 32-tap sinc can be read (its half width).
pub const SINC_FROM: f64 = 16.0;
/// Over how many samples the Hermite gives way to the sinc.
const SINC_BLEND: f64 = 8.0;

/// A windowed-sinc kernel's table: `PHASES + 1` rows of `taps` coefficients.
#[derive(Debug)]
pub struct Kernel {
    pub taps: usize,
    table: Vec<f32>,
}

fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term) = (1.0, 1.0);
    let q = x * x / 4.0;
    for k in 1..80 {
        term *= q / (k as f64 * k as f64);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

impl Kernel {
    /// `taps` (even) taps, cut off at `cut` of Nyquist, Kaiser β 9; each row sums to one.
    /// At full band, rows 0 and `PHASES` are exactly the identity.
    pub fn new(taps: usize, cut: f64) -> Kernel {
        let beta = 9.0;
        let half = taps as f64 / 2.0;
        let mut table = vec![0.0f32; (PHASES + 1) * taps];
        let i0b = bessel_i0(beta);
        for p in 0..=PHASES {
            let row = &mut table[p * taps..(p + 1) * taps];
            if cut >= 1.0 && (p == 0 || p == PHASES) {
                row[taps / 2 - 1 + usize::from(p == PHASES)] = 1.0;
                continue;
            }
            let frac = p as f64 / PHASES as f64;
            let mut vals = [0.0f64; MAX_TAPS];
            let mut sum = 0.0;
            for (j, v) in vals.iter_mut().enumerate().take(taps) {
                let t = j as f64 - (half - 1.0) - frac;
                let x = t / half;
                let w = if x.abs() > 1.0 {
                    0.0
                } else {
                    bessel_i0(beta * (1.0 - x * x).sqrt()) / i0b
                };
                let a = std::f64::consts::PI * cut * t;
                let s = if a == 0.0 { 1.0 } else { math::sin(a) / a };
                *v = s * w;
                sum += *v;
            }
            for (r, v) in row.iter_mut().zip(&vals[..taps]) {
                *r = (v / sum) as f32;
            }
        }
        Kernel { taps, table }
    }

    /// The coefficients at `frac` ∈ [0, 1) past a sample into `out[..taps]`; tap `j` weighs
    /// the sample `floor(pos) − (taps/2 − 1) + j`.
    #[inline]
    fn coefs(&self, frac: f64, out: &mut [f32; MAX_TAPS]) {
        let x = frac * PHASES as f64;
        let p = (x as usize).min(PHASES - 1);
        let a = (x - p as f64) as f32;
        let t = self.taps;
        let r0 = &self.table[p * t..p * t + t];
        let r1 = &self.table[(p + 1) * t..(p + 2) * t];
        for ((o, x0), x1) in out[..t].iter_mut().zip(r0).zip(r1) {
            *o = x0 + a * (x1 - x0);
        }
    }
}

/// The three kernels, by cut-off: full band, half, a quarter.
#[derive(Debug)]
pub struct Kernels {
    pub k: [Kernel; 3],
}

/// The kernels, built on first use. Call it off the audio thread first (`Lapsus::new`).
pub fn kernels() -> &'static Kernels {
    static K: OnceLock<Kernels> = OnceLock::new();
    K.get_or_init(|| Kernels {
        k: [
            Kernel::new(32, 1.0),
            Kernel::new(64, 0.5),
            Kernel::new(128, 0.25),
        ],
    })
}

/// The history's memory: two channels of `frames`, zeros. Built off the audio thread.
#[derive(Clone, Debug, Default)]
pub struct Ring {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
}

impl Ring {
    pub fn new(frames: usize) -> Ring {
        Ring {
            l: vec![0.0; frames],
            r: vec![0.0; frames],
        }
    }

    pub fn frames(&self) -> usize {
        self.l.len().min(self.r.len())
    }
}

/// The written past, by stream sample.
#[derive(Clone, Debug, Default)]
pub struct History {
    ring: Ring,
    /// The first stream sample a read may see (a reset, a seek, new memory).
    start: i64,
    /// Silent frames written in a row (bit-exact +0.0 in both channels).
    zeros: usize,
}

impl History {
    pub fn frames(&self) -> usize {
        self.ring.frames()
    }

    /// Nothing before stream sample `n` is heard any more (a seek, a loop's wrap).
    pub fn restart(&mut self, n: i64) {
        self.start = n;
    }

    /// Back to new: zeros everywhere (O(frames), a reset's cost), nothing before `n`.
    pub fn clear(&mut self, n: i64) {
        self.ring.l.iter_mut().for_each(|v| *v = 0.0);
        self.ring.r.iter_mut().for_each(|v| *v = 0.0);
        self.start = n;
        self.zeros = usize::MAX;
    }

    /// New memory, zeros; the old is handed back to be freed off the audio thread. What
    /// was written before `n` is not heard any more.
    pub fn supply(&mut self, ring: Ring, n: i64) -> Ring {
        let old = std::mem::replace(&mut self.ring, ring);
        self.start = n;
        self.zeros = usize::MAX;
        old
    }

    /// Whether everything a read could see now is +0.0: then the unit is as good as new.
    pub fn silent(&self, n: i64) -> bool {
        let reach = (n - self.start).max(0) as usize;
        self.zeros >= reach.min(self.frames())
    }

    /// Write the sample of stream sample `n`.
    #[inline]
    pub fn write(&mut self, n: i64, l: f32, r: f32) {
        let cap = self.frames();
        if cap > 0 {
            let i = n.rem_euclid(cap as i64) as usize;
            self.ring.l[i] = l;
            self.ring.r[i] = r;
        }
        self.zeros = if l.to_bits() == 0 && r.to_bits() == 0 {
            self.zeros.saturating_add(1)
        } else {
            0
        };
    }

    /// `frames` of silence were not written: the ring holds zeros already (at rest).
    pub fn skip(&mut self, frames: usize) {
        self.zeros = self.zeros.saturating_add(frames);
    }

    /// The window a read made at stream sample `now` may see, no earlier than `lo`.
    #[inline]
    fn window(&self, lo: i64, now: i64) -> (i64, i64) {
        let cap = self.frames() as i64;
        (lo.max(self.start).max(now - cap + 1), now)
    }

    /// One sample, zero outside `lo … hi`.
    #[inline]
    fn tap(&self, i: i64, lo: i64, hi: i64) -> (f32, f32) {
        if i < lo || i > hi {
            return (0.0, 0.0);
        }
        let k = i.rem_euclid(self.frames() as i64) as usize;
        (self.ring.l[k], self.ring.r[k])
    }

    /// The sample `delay` (whole) behind `now`, zero before `lo`: the dry path and the null.
    #[inline]
    pub fn at(&self, now: i64, delay: i64, lo: i64) -> (f32, f32) {
        if self.frames() == 0 {
            return (0.0, 0.0);
        }
        let (lo, hi) = self.window(lo, now);
        self.tap(now - delay, lo, hi)
    }

    /// The signal `delay` samples (fractional, ≥ 0) behind stream sample `now`, read at
    /// `speed` (the read's own rate against real time, for the anti-alias), nothing before
    /// `lo`. The fraction is taken from the delay alone, never from `now − delay`, so the
    /// same delay reads with the same coefficients at any point of the song (a render from
    /// bar 5 is bar 5's bits).
    pub fn read(&self, now: i64, delay: f64, lo: i64, speed: f32) -> (f32, f32) {
        if self.frames() == 0 || delay.is_nan() || delay < 0.0 {
            return (0.0, 0.0);
        }
        let (lo, hi) = self.window(lo, now);
        let whole = delay.floor();
        let fd = delay - whole;
        if fd == 0.0 {
            return self.tap(now - whole as i64, lo, hi);
        }
        let i = now - whole as i64 - 1;
        let frac = 1.0 - fd;
        let behind = delay;
        let lin = || {
            let (a, b) = (self.tap(i, lo, hi), self.tap(i + 1, lo, hi));
            let f = frac as f32;
            (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)
        };
        let herm = || {
            let (m, a, b, c) = (
                self.tap(i - 1, lo, hi),
                self.tap(i, lo, hi),
                self.tap(i + 1, lo, hi),
                self.tap(i + 2, lo, hi),
            );
            let f = frac as f32;
            (hermite(m.0, a.0, b.0, c.0, f), hermite(m.1, a.1, b.1, c.1, f))
        };
        let mix = |a: (f32, f32), b: (f32, f32), w: f64| {
            let w = w as f32;
            (a.0 + (b.0 - a.0) * w, a.1 + (b.1 - a.1) * w)
        };
        if behind < 1.0 {
            lin()
        } else if behind < 2.0 {
            mix(lin(), herm(), behind - 1.0)
        } else if behind < SINC_FROM {
            herm()
        } else if behind < SINC_FROM + SINC_BLEND {
            let s = self.sinc(i, frac, lo, hi, 0);
            mix(herm(), s, (behind - SINC_FROM) / SINC_BLEND)
        } else {
            self.sinc_at_speed(i, frac, lo, hi, behind, speed.abs())
        }
    }

    /// The sinc whose cut-off suits `speed`, as far as `behind` lets the kernel reach.
    fn sinc_at_speed(&self, i: i64, frac: f64, lo: i64, hi: i64, behind: f64, speed: f32) -> (f32, f32) {
        let k = kernels();
        let fits = |n: usize| behind >= (k.k[n].taps / 2) as f64;
        let (a, b, w) = if speed <= 1.0 {
            (0, 0, 0.0)
        } else if speed < 2.0 {
            (0, 1, speed - 1.0)
        } else if speed < 4.0 {
            (1, 2, (speed - 2.0) * 0.5)
        } else {
            (2, 2, 0.0)
        };
        // The longest kernel that fits, never past what the speed asks for.
        let a = (0..=a).rev().find(|&n| fits(n)).unwrap_or(0);
        let b = (0..=b).rev().find(|&n| fits(n)).unwrap_or(0);
        let x = self.sinc(i, frac, lo, hi, a);
        if a == b || w == 0.0 {
            return x;
        }
        let y = self.sinc(i, frac, lo, hi, b);
        (x.0 + (y.0 - x.0) * w, x.1 + (y.1 - x.1) * w)
    }

    /// Kernel `which` at `i + frac`, its taps outside `lo … hi` zero.
    fn sinc(&self, i: i64, frac: f64, lo: i64, hi: i64, which: usize) -> (f32, f32) {
        let k = &kernels().k[which];
        let mut c = [0.0f32; MAX_TAPS];
        k.coefs(frac, &mut c);
        let t = k.taps as i64;
        let base = i - (t / 2 - 1);
        let j0 = (lo - base).clamp(0, t);
        let j1 = (hi - base + 1).clamp(0, t);
        if j1 <= j0 {
            return (0.0, 0.0);
        }
        let cap = self.frames() as i64;
        let mut idx = (base + j0).rem_euclid(cap) as usize;
        let (mut sl, mut sr) = (0.0f32, 0.0f32);
        for &cj in &c[j0 as usize..j1 as usize] {
            sl += cj * self.ring.l[idx];
            sr += cj * self.ring.r[idx];
            idx += 1;
            if idx == cap as usize {
                idx = 0;
            }
        }
        (sl, sr)
    }
}
