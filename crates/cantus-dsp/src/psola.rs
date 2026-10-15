//! **The formant-preserving pitch shifter** of the pitch base (round 28, ADR 0053):
//! pitch-synchronous overlap-add (TD-PSOLA), in two forms that share one engine.
//!
//! - [`Shifter`] — **streaming, fixed latency** ([`Shifter::latency`]: 128 samples at
//!   48 kHz, 2.7 ms, the same time at every rate). What Tonus shifts a voice with.
//! - [`render`] — **offline**: a whole take with a ratio curve, a formant curve and a time
//!   map, on the exact epochs of [`crate::pitch::analyse`]. What the pitch editor
//!   resynthesises a corrected take with.
//!
//! ## How a grain is made
//!
//! A *grain* is a stretch of the input around an **analysis mark** `a`, windowed and added
//! to the output around a **synthesis mark** `s`. Synthesis marks are spaced by the
//! *target* period, so the output's pitch is the target exactly, whatever the input did;
//! each grain keeps the input's waveform — its spectral envelope, its **formants** — so the
//! voice keeps its timbre. Reading a grain faster or slower (rate `f`) moves the formants
//! by `f` without touching the pitch: the *throat* control.
//!
//! The windows are **asymmetric**: a grain falls over one input period (`h = P_a / f`)
//! and rises over `min(P_s, h)` — from the previous synthesis mark, exactly when the
//! previous grain is at its peak, when shifting up. Where the windows sum above one (a
//! shift up) the output is normalised by their sum, so the level never swells; where they
//! dip (a shift down) it is not, because that dip is what makes the longer period. A ratio
//! of 1 reproduces the input. Grains are chosen on a **lattice** one input period apart, anchored on the
//! previous grain's mark, so two overlapping grains are always on the same phase of the
//! waveform; a ratio jump only changes the next spacing — no click.
//!
//! ## The latency, and why it is enough
//!
//! A grain is created when the output reaches the previous synthesis mark, and it may read
//! the input up to the newest sample. Output sample `t` is emitted `L` samples after input
//! sample `t` arrived, so the lattice mark nearest the synthesis mark is usable whenever
//! it is at most `L` ahead of it (less the stretch a faster formant read needs). With
//! `L` = 2.7 ms the nearest mark is always usable for periods up to `2L` (voices above
//! 190 Hz); below, the grain comes from up to half a period earlier — the waveform of the
//! period before, which for a voice is the same shape. The latency is constant: nothing
//! about it depends on the input.
//!
//! Where the shifter is not engaged (no correction, unvoiced) the output is the input
//! delayed by `L`, **bit for bit**; engaging and releasing cross-fade over 5 ms.

use crate::math;
use crate::pitch::F_MIN;
use crate::rest::ZeroRun;

#[path = "psola_offline.rs"]
mod offline;
#[cfg(test)]
#[path = "psola_tests.rs"]
mod tests;

pub use offline::{render, Plan, TimeMap};

/// The latency at 48 kHz, samples (2.7 ms; scaled with the rate).
pub const LATENCY_48K: usize = 128;
/// The engage / release cross-fade, seconds.
pub const FADE_S: f32 = 0.005;
/// The period used for unvoiced input (noise has none): grains 4 ms apart.
pub const NOISE_PERIOD_S: f32 = 0.004;
/// The widest pitch ratio (two octaves either way).
pub const MAX_RATIO: f32 = 4.0;
/// The widest formant read rate (an octave either way).
pub const MAX_FORMANT: f32 = 2.0;
/// Grains alive at once, at most.
const MAX_GRAINS: usize = 24;
const TABLE: usize = 1024;

/// What the shifter does now.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Control {
    /// The input's period, samples; 0 when unvoiced (the grains are then 4 ms apart and
    /// the pitch is left alone).
    pub period: f32,
    /// The pitch ratio (2 is an octave up).
    pub ratio: f32,
    /// The formant read rate (1 keeps the formants; 2^(st/12) moves them `st` semitones).
    pub formant: f32,
    /// Shift (true) or pass the input through, delayed (false).
    pub engage: bool,
}

impl Default for Control {
    fn default() -> Self {
        Control {
            period: 0.0,
            ratio: 1.0,
            formant: 1.0,
            engage: false,
        }
    }
}

/// One grain: marks in input time (samples), half-widths in output samples, a read rate.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct Grain {
    pub s: f64,
    pub a: f64,
    pub hl: f32,
    pub hr: f32,
    pub f: f32,
}

/// The overlap-add engine both forms share: the live grains and the window.
#[derive(Clone, Debug)]
pub(crate) struct Ola {
    grains: [Grain; MAX_GRAINS],
    count: usize,
    table: Vec<f32>,
}

impl Ola {
    pub(crate) fn new() -> Self {
        let table = (0..=TABLE)
            .map(|i| {
                let x = i as f64 / TABLE as f64;
                (0.5 * (1.0 - math::cos(core::f64::consts::PI * x))) as f32
            })
            .collect();
        Ola {
            grains: [Grain::default(); MAX_GRAINS],
            count: 0,
            table,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.count = 0;
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The newest grain.
    pub(crate) fn last(&self) -> Option<&Grain> {
        self.count.checked_sub(1).map(|i| &self.grains[i])
    }

    pub(crate) fn push(&mut self, g: Grain) {
        if self.count == MAX_GRAINS {
            self.grains.copy_within(1.., 0);
            self.count -= 1;
        }
        self.grains[self.count] = g;
        self.count += 1;
    }

    /// The raised cosine at `x` (0 … 1), from the table.
    #[inline]
    fn rc(&self, x: f32) -> f32 {
        let p = x.clamp(0.0, 1.0) * TABLE as f32;
        let i = (p as usize).min(TABLE - 1);
        let fr = p - i as f32;
        self.table[i] + (self.table[i + 1] - self.table[i]) * fr
    }

    /// Drop the grains that ended before `t`.
    pub(crate) fn retire(&mut self, t: f64) {
        let mut k = 0;
        for i in 0..self.count {
            let g = self.grains[i];
            if t < g.s + g.hr as f64 {
                self.grains[k] = g;
                k += 1;
            }
        }
        self.count = k;
    }

    /// The sum at output time `t` over `N` channels, normalised where the windows sum
    /// above one; `read(ch, at)` reads channel `ch` of the input at a fractional time.
    /// `None` when no grain is alive.
    #[inline]
    pub(crate) fn sample<const N: usize>(
        &self,
        t: f64,
        mut read: impl FnMut(usize, f64) -> f32,
    ) -> Option<[f32; N]> {
        let mut num = [0.0f32; N];
        let mut den = 0.0f32;
        for g in &self.grains[..self.count] {
            let u = (t - g.s) as f32;
            let w = if u < 0.0 {
                if g.hl <= 0.0 || u <= -g.hl {
                    continue;
                }
                self.rc((u + g.hl) / g.hl)
            } else {
                if u >= g.hr {
                    continue;
                }
                self.rc(1.0 - u / g.hr)
            };
            if w <= 0.0 {
                continue;
            }
            let at = g.a + (u * g.f) as f64;
            for (ch, n) in num.iter_mut().enumerate() {
                *n += w * read(ch, at);
            }
            den += w;
        }
        // Normalised where the windows overlap by more than one (a shift up), never
        // amplified where they dip (a shift down): the dip between two grains is what
        // makes the new, longer period.
        (self.count > 0).then(|| {
            let k = 1.0 / den.max(1.0);
            num.map(|n| n * k)
        })
    }
}

/// Cubic Hermite interpolation of `x` at a fractional index (zero outside).
#[inline]
pub(crate) fn read_at(x: &[f32], at: f64) -> f32 {
    let i = at.floor();
    let t = (at - i) as f32;
    let i = i as i64;
    let get = |k: i64| -> f32 {
        if k >= 0 && (k as usize) < x.len() {
            x[k as usize]
        } else {
            0.0
        }
    };
    hermite(get(i - 1), get(i), get(i + 1), get(i + 2), t)
}

#[inline]
pub(crate) fn hermite(xm1: f32, x0: f32, x1: f32, x2: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}

/// The next analysis mark on the lattice of `prev` (spacing `p`) nearest `s`, no later
/// than `bound`.
#[inline]
pub(crate) fn lattice(prev: f64, p: f64, s: f64, bound: f64) -> f64 {
    let mut k = ((s - prev) / p).round().max(0.0);
    while k > 0.0 && prev + k * p > bound {
        k -= 1.0;
    }
    let mut a = prev + k * p;
    while a > bound {
        a -= p;
    }
    a
}

/// The streaming shifter (see the module docs). Allocates in `new` only.
#[derive(Clone, Debug)]
pub struct Shifter {
    sample_rate: f32,
    latency: usize,
    /// The input, a power-of-two ring per channel, indexed by the sample count.
    ring: [Vec<f32>; 2],
    zero: [ZeroRun; 2],
    /// Input samples written since reset (skips included).
    n: u64,
    ola: Ola,
    /// The wet share, 0 (the dry input, delayed) … 1.
    mix: f32,
    fade_step: f32,
    noise_period: f32,
}

impl Shifter {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8_000.0);
        let latency = ((LATENCY_48K as f32 * sr / 48_000.0).round() as usize).max(1);
        let p_max = sr / F_MIN;
        let len = ((p_max * (MAX_RATIO + 2.0) * MAX_FORMANT) as usize + latency + 16)
            .next_power_of_two();
        Shifter {
            sample_rate: sr,
            latency,
            ring: [vec![0.0; len], vec![0.0; len]],
            zero: [ZeroRun::FULL; 2],
            n: 0,
            ola: Ola::new(),
            mix: 0.0,
            fade_step: 1.0 / (FADE_S * sr).max(1.0),
            noise_period: NOISE_PERIOD_S * sr,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// The fixed delay, samples: the output is the input this much later.
    pub fn latency(&self) -> usize {
        self.latency
    }

    /// The wet share now (0 … 1).
    pub fn mix(&self) -> f32 {
        self.mix
    }

    pub fn reset(&mut self) {
        for r in &mut self.ring {
            r.fill(0.0);
        }
        self.zero = [ZeroRun::FULL; 2];
        self.n = 0;
        self.ola.clear();
        self.mix = 0.0;
    }

    /// Nothing but zeros in the line, no grain alive, fully dry: a new shifter, exactly.
    pub fn at_rest(&self) -> bool {
        let len = self.ring[0].len();
        self.zero[0].covers(len) && self.zero[1].covers(len) && self.mix == 0.0 && self.ola.is_empty()
    }

    /// `frames` samples of silence went by unprocessed, at rest.
    pub fn skip(&mut self, frames: usize) {
        self.n += frames as u64;
    }

    /// One stereo sample in, one out (`latency` samples later).
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32, c: &Control) -> (f32, f32) {
        let mask = self.ring[0].len() - 1;
        let w = self.n as usize & mask;
        self.ring[0][w] = l;
        self.ring[1][w] = r;
        self.zero[0].push(l);
        self.zero[1].push(r);
        self.n += 1;
        // The output's time, in input samples: `latency` behind the newest.
        let t_i = self.n as i64 - 1 - self.latency as i64;
        let dry = if t_i >= 0 {
            let k = t_i as usize & mask;
            (self.ring[0][k], self.ring[1][k])
        } else {
            (0.0, 0.0)
        };
        if c.engage {
            self.mix = (self.mix + self.fade_step).min(1.0);
        } else {
            self.mix = (self.mix - self.fade_step).max(0.0);
        }
        if self.mix == 0.0 {
            self.ola.clear();
            return dry;
        }
        let t = t_i as f64;
        self.grow(t, c);
        self.ola.retire(t);
        let ring = &self.ring;
        let wet = self
            .ola
            .sample::<2>(t, |ch, at| read_ring(&ring[ch], mask, at))
            .unwrap_or([dry.0, dry.1]);
        if self.mix == 1.0 {
            (wet[0], wet[1])
        } else {
            let m = self.mix;
            (dry.0 + (wet[0] - dry.0) * m, dry.1 + (wet[1] - dry.1) * m)
        }
    }

    /// Create grains until one is centred after `t`.
    fn grow(&mut self, t: f64, c: &Control) {
        let voiced = c.period > 1.0;
        let p_a = if voiced { c.period } else { self.noise_period };
        let ratio = if voiced {
            c.ratio.clamp(1.0 / MAX_RATIO, MAX_RATIO)
        } else {
            1.0
        };
        let f = c.formant.clamp(1.0 / MAX_FORMANT, MAX_FORMANT);
        let p_s = p_a / ratio;
        let newest = (self.n - 1) as f64;
        let slack = (newest - 2.0) - t;
        let Some(last) = self.ola.last().copied() else {
            // The first grain: at full weight now, reading the input now (earlier when a
            // faster formant read would overtake the newest sample).
            let hr = p_a / f;
            let early = (hr as f64 * (f as f64 - 1.0) - slack).max(0.0);
            self.ola.push(Grain {
                s: t,
                a: t - early,
                hl: 0.0,
                hr,
                f,
            });
            return;
        };
        if t < last.s {
            return;
        }
        let s = last.s + p_s as f64;
        let hr = p_a / f;
        let hl = p_s.min(hr);
        // `slack`: how far past `t` a grain may read now (Hermite needs two samples past
        // the read point); the bound keeps every read of its life behind the newest input.
        let bound = if f <= 1.0 {
            s + slack - hl as f64 * (1.0 - f as f64)
        } else {
            s + slack + hr as f64 - hr as f64 * f as f64
        };
        let a = lattice(last.a, p_a as f64, s, bound);
        self.ola.push(Grain { s, a, hl, hr, f });
    }
}

/// Hermite read from a ring indexed by absolute sample count.
#[inline]
fn read_ring(ring: &[f32], mask: usize, at: f64) -> f32 {
    let i = at.floor();
    let t = (at - i) as f32;
    let i = i as i64;
    let get = |k: i64| ring[k as usize & mask];
    if i < 1 {
        return 0.0;
    }
    hermite(get(i - 1), get(i), get(i + 1), get(i + 2), t)
}
