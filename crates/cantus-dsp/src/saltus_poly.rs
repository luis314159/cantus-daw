//! Saltus's poly engine: a **streaming phase-locked vocoder** that moves every partial of
//! a chord or a mix at once.
//!
//! Frames of `N` (4 096 at 48 kHz, ≈ 85 ms: long enough to keep a guitar chord's low
//! partials apart, [`FRAME_48K`]) every `N/8` samples, Hann windows in and out. In each frame the
//! partials are found (the spectrum's peaks) and **each peak's region is moved whole** to
//! its new frequency, its magnitudes read between bins by a windowed sinc — the
//! stretcher's way (Laroche–Dolson, round 29), so a partial's lobe keeps its shape and
//! level. Phases are **locked** to their peak (identity phase locking): each peak's phase
//! advances by its own true frequency times the ratio, the bins around it keep their
//! analysed relation to it, so a partial stays one coherent lobe. At a transient (the
//! frame's energy up 6 dB on the frame before) the peaks that rose take their analysed
//! phase, so an attack keeps its edge. With the **formants kept**, every moved magnitude is
//! put back under the source's spectral envelope (the true envelope,
//! [`crate::stretch`]'s), moved by the throat.
//!
//! The analysis is shared by both voices; each voice has its own phases and synthesis.
//! The output for sample `t` is complete once the frame that ends `N − 1` samples later has
//! been added: the latency is `N − 1`, fixed.
//!
//! **At rest**: a frame whose window is all silence clears the phases (the next sound
//! starts them from its own, as a new vocoder's first frame does); then, the input window
//! all zeros and every overlap-added sample read out, it is a new vocoder.

// Index loops over spectra read clearer than iterator chains here.
#![allow(clippy::needless_range_loop)]

use super::{balance, Mode, SaltusSettings, Sum};
use crate::aequalis::fft::Fft;
use crate::math;
use crate::rest::ZeroRun;
use crate::stretch::env::Envelope;
use std::f32::consts::PI;

/// The frame at 48 kHz (scaled with the rate to the nearest power of two): 85 ms. At
/// 2 048 (the design's first figure) a minor third at A3 — 220 and 262 Hz, 1.8 bins apart —
/// is one lobe and moves as one: its partials came out 30 cents off and 40 dB down; at
/// 4 096 they are within 0.01 cent and 3 dB, and a real strum's spectrum moves within
/// 0.6 dB. 8 192 resolves a chord better still (its residue −50 dB against −20) but smears
/// a strum's attack over 170 ms ([`Poly::with_frame`] makes one for listening).
pub const FRAME_48K: usize = 4_096;
/// Hops per frame.
const OVERLAP: usize = 8;
/// The bins at the bottom (DC and its lobe) and Nyquist keep their analysed phase.
const DC_BINS: usize = 2;
/// A peak whose magnitude grew by this over a hop, in a transient frame, restarts.
const RISE: f32 = 1.5;
/// A frame is a transient's when its energy is this many times the frame before's.
const TRANSIENT: f32 = 4.0;
/// Taps either side of a fractional bin, and fractions per table.
const SPAN: i64 = 4;
const STEPS: usize = 64;

/// What one voice of the vocoder does this frame.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct PolyVoice {
    /// The interval, semitones.
    pub shift: f32,
    /// The formant factor: 1 keeps the envelope, the pitch ratio lets it follow.
    pub formant: f32,
    pub level: f32,
    pub pan: f32,
}

/// Both voices.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub voices: [PolyVoice; 2],
}

/// The vocoder's plan for settings `s` (Shift or Whammy), the whammy at `whammy` st.
pub fn plan(s: &SaltusSettings, whammy: f32) -> Plan {
    let throat = if s.throat == 0.0 {
        1.0
    } else {
        math::exp2(s.throat / 12.0)
    };
    let mut p = Plan::default();
    for (v, out) in p.voices.iter_mut().enumerate() {
        let vs = &s.voices[v];
        let (shift, level) = match s.mode {
            Mode::Whammy if v == 0 => (whammy, vs.level),
            Mode::Whammy => (0.0, 0.0),
            _ => (vs.semitones + vs.cents * 0.01, vs.level),
        };
        let ratio = math::exp2(shift / 12.0);
        *out = PolyVoice {
            shift,
            formant: if s.formants { throat } else { throat * ratio },
            level,
            pan: vs.pan,
        };
    }
    p
}

/// The true envelope's computer: scratch only (every call starts from the spectrum), so a
/// copy is a new one.
struct Env {
    e: Envelope,
    n: usize,
    sr: f32,
}

impl Env {
    fn new(n: usize, sr: f32) -> Self {
        Env {
            e: Envelope::new(n, sr),
            n,
            sr,
        }
    }
}

impl Clone for Env {
    fn clone(&self) -> Self {
        Env::new(self.n, self.sr)
    }
}

impl std::fmt::Debug for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Env({})", self.n)
    }
}

/// One channel's analysis.
#[derive(Clone, Debug)]
struct Ana {
    mag: Vec<f32>,
    mag0: Vec<f32>,
    phase: Vec<f32>,
    prev: Vec<f32>,
    omega: Vec<f32>,
    /// The peaks found, the first `n_peaks` of a list as long as the spectrum (a clone
    /// keeps its length, where it would not keep a reserved capacity).
    peaks: Vec<usize>,
    n_peaks: usize,
    owner: Vec<usize>,
    env: Vec<f32>,
}

impl Ana {
    fn new(bins: usize) -> Self {
        Ana {
            mag: vec![0.0; bins],
            mag0: vec![0.0; bins],
            phase: vec![0.0; bins],
            prev: vec![0.0; bins],
            omega: vec![0.0; bins],
            peaks: vec![0; bins],
            n_peaks: 0,
            owner: (0..bins).collect(),
            env: vec![1.0; bins],
        }
    }

    fn clear(&mut self) {
        self.mag.fill(0.0);
        self.mag0.fill(0.0);
        self.phase.fill(0.0);
        self.prev.fill(0.0);
        self.omega.fill(0.0);
        self.n_peaks = 0;
        for (k, o) in self.owner.iter_mut().enumerate() {
            *o = k;
        }
        self.env.fill(1.0);
    }
}

/// The streaming vocoder. Allocates in `new` only.
#[derive(Clone, Debug)]
pub struct Poly {
    sr: f32,
    n: usize,
    h: usize,
    fft: Fft,
    win: Vec<f32>,
    scale: f32,
    weights: Vec<[f32; 8]>,
    ring: [Vec<f32>; 2],
    zero: [ZeroRun; 2],
    acc: [Vec<f32>; 2],
    count: u64,
    /// The overlap-added samples before this index may be non-zero.
    dirty_end: u64,
    ana: [Ana; 2],
    /// Each voice's phases, per channel.
    psi: [[Vec<f32>; 2]; 2],
    live: [bool; 2],
    env: Env,
    re: Vec<f32>,
    im: Vec<f32>,
    energy0: f32,
    fresh: bool,
    shifts: [Option<f32>; 2],
}

impl Poly {
    /// The frame length at `sample_rate`.
    pub fn frame_at(sample_rate: f32) -> usize {
        ((FRAME_48K as f32 * sample_rate.max(8_000.0) / 48_000.0).round() as usize)
            .next_power_of_two()
            .clamp(256, 16_384)
    }

    /// The latency at `sample_rate`, samples.
    pub fn latency_at(sample_rate: f32) -> usize {
        Self::frame_at(sample_rate) - 1
    }

    pub fn new(sample_rate: f32) -> Self {
        Self::with_frame(sample_rate, Self::frame_at(sample_rate))
    }

    /// A vocoder with frames of `n` (a power of two, 256 … 16 384): to listen to another
    /// trade between a chord's resolution and an attack's smear. The unit uses [`Self::new`].
    pub fn with_frame(sample_rate: f32, n: usize) -> Self {
        let sr = sample_rate.max(8_000.0);
        let n = n.next_power_of_two().clamp(256, 16_384);
        let h = n / OVERLAP;
        let bins = n / 2 + 1;
        let win: Vec<f32> = (0..n)
            .map(|i| {
                (0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / n as f64)) as f32
            })
            .collect();
        let ola: f32 = (0..OVERLAP).map(|j| win[j * h] * win[j * h]).sum();
        let chan = || [vec![0.0f32; bins], vec![0.0f32; bins]];
        Poly {
            sr,
            n,
            h,
            fft: Fft::new(n),
            scale: 1.0 / (n as f32 * ola),
            win,
            weights: weights(),
            ring: [vec![0.0; n], vec![0.0; n]],
            zero: [ZeroRun::FULL; 2],
            acc: [vec![0.0; 2 * n], vec![0.0; 2 * n]],
            count: 0,
            dirty_end: 0,
            ana: [Ana::new(bins), Ana::new(bins)],
            psi: [chan(), chan()],
            live: [false; 2],
            env: Env::new(n, sr),
            re: vec![0.0; n],
            im: vec![0.0; n],
            energy0: 0.0,
            fresh: true,
            shifts: [None; 2],
        }
    }

    pub fn latency(&self) -> usize {
        self.n - 1
    }

    pub fn frame_len(&self) -> usize {
        self.n
    }

    pub fn reset(&mut self) {
        for c in 0..2 {
            self.ring[c].fill(0.0);
            self.acc[c].fill(0.0);
            self.ana[c].clear();
            for v in 0..2 {
                self.psi[v][c].fill(0.0);
            }
        }
        self.zero = [ZeroRun::FULL; 2];
        self.count = 0;
        self.dirty_end = 0;
        self.live = [false; 2];
        self.energy0 = 0.0;
        self.fresh = true;
        self.shifts = [None; 2];
    }

    pub fn at_rest(&self) -> bool {
        self.fresh
            && self.zero[0].covers(self.n)
            && self.zero[1].covers(self.n)
            && self.dirty_end + self.n as u64 <= self.count + 1
    }

    pub fn skip(&mut self, frames: usize) {
        self.count += frames as u64;
    }

    /// The interval voice `i` sounds at, while it does.
    pub fn shift(&self, i: usize) -> Option<f32> {
        self.shifts.get(i).copied().flatten()
    }

    /// One stereo sample in; the voices' sum `latency` samples later.
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32, plan: &Plan) -> Sum {
        let n = self.n;
        let w = self.count as usize & (n - 1);
        self.ring[0][w] = l;
        self.ring[1][w] = r;
        self.zero[0].push(l);
        self.zero[1].push(r);
        self.count += 1;
        if self.count.is_multiple_of(self.h as u64) {
            self.frame(plan);
        }
        let mut sum = Sum::default();
        if self.count >= n as u64 {
            let j = (self.count - n as u64) as usize & (2 * n - 1);
            let (a, b) = (self.acc[0][j], self.acc[1][j]);
            self.acc[0][j] = 0.0;
            self.acc[1][j] = 0.0;
            if self.dirty_end > self.count - n as u64 {
                sum.add(a, b, 1.0, 0.0);
            }
        }
        sum
    }

    /// The frame over the newest `N` samples.
    fn frame(&mut self, plan: &Plan) {
        let n = self.n;
        if self.zero[0].covers(n) && self.zero[1].covers(n) {
            // Silence: the phases start again from the next sound's own.
            if !self.fresh {
                for c in 0..2 {
                    self.ana[c].clear();
                    for v in 0..2 {
                        self.psi[v][c].fill(0.0);
                    }
                }
                self.live = [false; 2];
                self.energy0 = 0.0;
                self.fresh = true;
            }
            self.shifts = [None; 2];
            return;
        }
        let needs_env = plan
            .voices
            .iter()
            .any(|v| v.level > 0.0 && v.formant != math::exp2(v.shift / 12.0));
        let mut energy = 0.0f32;
        for c in 0..2 {
            energy += self.analyse(c, needs_env);
        }
        let transient = !self.fresh && energy > TRANSIENT * self.energy0;
        for v in 0..2 {
            let pv = plan.voices[v];
            if pv.level <= 0.0 {
                self.live[v] = false;
                self.shifts[v] = None;
                continue;
            }
            let all = self.fresh || !self.live[v];
            let ratio = math::exp2(pv.shift / 12.0);
            let (gl, gr) = balance(pv.pan);
            for c in 0..2 {
                self.advance(v, c, ratio, all, transient);
                self.synthesise(v, c, ratio, pv.formant, needs_env);
                let g = pv.level * if c == 0 { gl } else { gr };
                self.add(c, g);
            }
            self.live[v] = true;
            self.shifts[v] = Some(pv.shift);
        }
        for c in 0..2 {
            let a = &mut self.ana[c];
            a.mag0.copy_from_slice(&a.mag);
        }
        self.energy0 = energy;
        self.fresh = false;
        self.dirty_end = self.count;
    }

    /// Channel `c`'s spectrum, its true frequencies, its peaks and (if asked) its envelope;
    /// the frame's energy.
    fn analyse(&mut self, c: usize, needs_env: bool) -> f32 {
        let n = self.n;
        let mask = n - 1;
        let start = self.count as usize; // the oldest sample of the window, mod n
        for i in 0..n {
            self.re[i] = self.ring[c][(start + i) & mask] * self.win[i];
            self.im[i] = 0.0;
        }
        self.fft.run(&mut self.re, &mut self.im, false);
        let h = self.h as f32;
        let a = &mut self.ana[c];
        let bins = a.mag.len();
        let mut e = 0.0f32;
        for k in 0..bins {
            let (re, im) = (self.re[k], self.im[k]);
            a.mag[k] = math::hypot(re, im);
            a.phase[k] = math::atan2(im, re);
            e += a.mag[k] * a.mag[k];
            let wk = 2.0 * PI * k as f32 / n as f32;
            let d = a.phase[k] - a.prev[k] - wk * h;
            a.omega[k] = wk + princarg(d) / h;
        }
        a.prev.copy_from_slice(&a.phase);
        a.n_peaks = find_peaks(&a.mag, &mut a.peaks, &mut a.owner);
        if needs_env {
            self.env.e.compute(&a.mag, &mut a.env);
        }
        e
    }

    /// Voice `v`'s phases on channel `c` advanced to this frame.
    fn advance(&mut self, v: usize, c: usize, ratio: f32, all: bool, transient: bool) {
        let a = &self.ana[c];
        let psi = &mut self.psi[v][c];
        let step = self.h as f32 * ratio;
        let bins = a.mag.len();
        if all {
            psi.copy_from_slice(&a.phase);
            return;
        }
        let rose = |k: usize| transient && a.mag[k] > RISE * a.mag0[k];
        if a.n_peaks == 0 {
            for k in 0..bins {
                psi[k] = if rose(k) {
                    a.phase[k]
                } else {
                    princarg(psi[k] + step * a.omega[k])
                };
            }
        } else {
            for &p in &a.peaks[..a.n_peaks] {
                psi[p] = if rose(p) {
                    a.phase[p]
                } else {
                    princarg(psi[p] + step * a.omega[p])
                };
            }
            for k in 0..bins {
                let p = a.owner[k];
                if p != k {
                    psi[k] = princarg(psi[p] + a.phase[k] - a.phase[p]);
                }
            }
        }
        for k in (0..DC_BINS.min(bins)).chain([bins - 1]) {
            psi[k] = a.phase[k];
        }
    }

    /// Voice `v`'s spectrum on channel `c`, into `re`/`im` (the full, conjugate-symmetric
    /// spectrum): each peak's region moved by the ratio.
    fn synthesise(&mut self, v: usize, c: usize, ratio: f32, formant: f32, env: bool) {
        let n = self.n;
        let half = n / 2;
        let a = &self.ana[c];
        let psi = &self.psi[v][c];
        self.re.fill(0.0);
        self.im.fill(0.0);
        let unmoved = ratio == 1.0 && (!env || formant == 1.0);
        if unmoved {
            for m in 0..=half {
                let (s, co) = math::sin_cos(psi[m]);
                self.re[m] = a.mag[m] * co;
                self.im[m] = a.mag[m] * s;
            }
        } else {
            let e = &a.env;
            let env_at = |x: f32| -> f32 {
                let x = x.clamp(0.0, half as f32);
                let i = (x.floor() as usize).min(half);
                let j = (i + 1).min(half);
                let t = x - i as f32;
                e[i] + (e[j] - e[i]) * t
            };
            let to_bin = n as f32 / (2.0 * PI);
            for k in 0..DC_BINS.min(half) {
                let (s, co) = math::sin_cos(psi[k]);
                self.re[k] = a.mag[k] * co;
                self.im[k] = a.mag[k] * s;
            }
            let mut lo = DC_BINS;
            while lo < half {
                let p = a.owner[lo];
                let mut hi = lo + 1;
                while hi < half && a.owner[hi] == p {
                    hi += 1;
                }
                let pf = p as f32;
                let true_bin = (a.omega[p] * to_bin).clamp(pf - 0.5, pf + 0.5);
                let d = true_bin * (ratio - 1.0);
                let first = ((lo as f32 + d).ceil().max(DC_BINS as f32)) as usize;
                let last = ((hi as f32 + d).ceil().max(0.0) as usize).min(half);
                for m in first..last {
                    let q = m as f32 - d;
                    if q < 0.0 || q > half as f32 {
                        continue;
                    }
                    let gain = if env {
                        (env_at(m as f32 / formant) / env_at(q).max(1e-12)).clamp(1.0 / 32.0, 32.0)
                    } else {
                        1.0
                    };
                    let mag = between(&self.weights, &a.mag, q) * gain;
                    let i = (q.round() as usize).clamp(lo, hi - 1);
                    let (s, co) = math::sin_cos(psi[i]);
                    self.re[m] += mag * co;
                    self.im[m] += mag * s;
                }
                lo = hi;
            }
        }
        self.im[0] = 0.0;
        self.im[half] = 0.0;
        for m in 1..half {
            self.re[n - m] = self.re[m];
            self.im[n - m] = -self.im[m];
        }
        self.fft.run(&mut self.re, &mut self.im, true);
    }

    /// The synthesised frame, windowed, into channel `c`'s overlap-add at gain `g`.
    fn add(&mut self, c: usize, g: f32) {
        let n = self.n;
        let mask = 2 * n - 1;
        // The window's first sample; before the stream's start nothing is written (those
        // cells are a later time's, never read for this one).
        let start = self.count as i64 - n as i64;
        let skip = (-start).max(0) as usize;
        let k = self.scale * g;
        let acc = &mut self.acc[c];
        for i in skip..n {
            acc[(start + i as i64) as usize & mask] += self.re[i] * self.win[i] * k;
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }
}

#[inline]
fn princarg(x: f32) -> f32 {
    x - 2.0 * PI * (x / (2.0 * PI)).round()
}

/// The spectrum's peaks (a bin above its two neighbours each side, over −100 dB of the
/// frame's top), and each bin's owner: the peak whose region it is in (regions meet at the
/// lowest bin between two peaks). No peak: every bin its own.
fn find_peaks(mag: &[f32], peaks: &mut [usize], owner: &mut [usize]) -> usize {
    let mut count = 0;
    let top = mag.iter().fold(0.0f32, |m, v| m.max(*v));
    let floor = top * 1e-5;
    let h = mag.len();
    for k in 2..h.saturating_sub(2) {
        let v = mag[k];
        if v > floor && v > mag[k - 1] && v >= mag[k + 1] && v > mag[k - 2] && v >= mag[k + 2] {
            peaks[count] = k;
            count += 1;
        }
    }
    if count == 0 {
        for (k, o) in owner.iter_mut().enumerate() {
            *o = k;
        }
        return 0;
    }
    let peaks = &peaks[..count];
    let mut start = 0;
    for w in 0..peaks.len() {
        let p = peaks[w];
        let end = match peaks.get(w + 1) {
            Some(&q) => {
                let mut lowest = p;
                for b in p..=q {
                    if mag[b] < mag[lowest] {
                        lowest = b;
                    }
                }
                lowest + 1
            }
            None => h,
        };
        for o in &mut owner[start..end] {
            *o = p;
        }
        start = end;
    }
    count
}

/// The interpolation weights: for `STEPS + 1` fractions `f` of a bin, eight taps at
/// `k − f` — a Hann-windowed sinc, each row summing to 1.
fn weights() -> Vec<[f32; 8]> {
    (0..=STEPS)
        .map(|i| {
            let f = i as f64 / STEPS as f64;
            let mut row = [0.0f64; 8];
            for (j, w) in row.iter_mut().enumerate() {
                let x = (j as i64 + 1 - SPAN) as f64 - f;
                let sinc = if x.abs() < 1e-12 {
                    1.0
                } else {
                    let a = std::f64::consts::PI * x;
                    math::sin(a) / a
                };
                let win = 0.5 + 0.5 * math::cos(std::f64::consts::PI * x / (SPAN as f64 + 1.0));
                *w = sinc * win;
            }
            let sum: f64 = row.iter().sum();
            let mut out = [0.0f32; 8];
            for (o, w) in out.iter_mut().zip(row) {
                *o = (w / sum) as f32;
            }
            out
        })
        .collect()
}

/// The magnitude spectrum `v` between its bins, at `q`, mirrored at DC and Nyquist.
#[inline]
fn between(weights: &[[f32; 8]], v: &[f32], q: f32) -> f32 {
    let half = (v.len() - 1) as i64;
    let i = q.floor() as i64;
    let f = q - i as f32;
    let row = &weights[((f * STEPS as f32).round() as usize).min(STEPS)];
    let at = |k: i64| -> f32 {
        let k = k.abs();
        let k = if k > half { 2 * half - k } else { k };
        v[k.clamp(0, half) as usize]
    };
    let mut acc = 0.0f32;
    for (j, w) in row.iter().enumerate() {
        acc += w * at(i + j as i64 + 1 - SPAN);
    }
    acc.max(0.0)
}
