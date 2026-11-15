//! Caelum's tank: a feedback delay network of 8 or 16 modulated lines, an all-pass in
//! every line, a three-band absorption fitted to the decay, and the shimmer — pitch
//! shifters that take part of the tank's energy out, move it, and put the same energy
//! back.
//!
//! ## The loop, one sample
//!
//! ```text
//!   line i ─ modulated read (all-pass interpolated) ─ all-pass ─ absorption ─┬─ outputs (2 rows)
//!                                                                            │
//!     ┌─ hold/governor trim ─ shimmer exchange (4 channels) ─────────────────┘
//!     └─ signs ─ Hadamard ─ permutation ─ + input (2 rows) ─ write line i
//! ```
//!
//! - **The matrix** is `P·H·D`: signs, the Walsh–Hadamard transform (normalised, so
//!   orthogonal), a permutation — lossless, and not an involution (the Hadamard alone
//!   sends everything back where it came from two passes later).
//! - **The all-pass in every line** (3–12 ms, gain up to 0.6 with Diffusion) multiplies the
//!   echoes per pass without a loss: a huge tank's long lines would otherwise read as
//!   separate echoes. Its length counts in the loop's length for the decay.
//! - **The absorption** of each line is Jot's: the loss per pass proportional to the
//!   loop's length, so every mode decays alike — a gain for the middle and two first-order
//!   shelves (a high one at the damping frequency, a low one at the bass frequency), each
//!   exact at its end of the spectrum, monotonic between. A line `L` samples long loses
//!   `60·L / (fs·T)` dB a pass in each band's own `T`.
//! - **Infinite** holds the middle (a gain of one) and keeps the shelves as they would be
//!   at the longest finite decay, so damping still darkens an endless tail; a **hold**
//!   gives back the little the modulation's interpolation loses (at most 0.5 % a pass),
//!   and never lifts the energy above the highest it reached.
//! - **Freeze** cross-fades each line from its absorbed output to its raw output (a loop
//!   that keeps everything), mutes the shimmer, and holds the level it had 150 ms after
//!   closing — the input is gated outside the tank.
//! - **The governor** is the tank's last guard: above a ceiling (the wet signal at
//!   −3 dBFS RMS) the loop gain comes down by the square root of the excess. It never
//!   acts on a normal tail; it is what makes an infinite tank fed for minutes plateau.
//!
//! ## The shimmer's energy
//!
//! Four channels (voice A on two rows of the matrix, voice B on two others), each a row
//! `c` of the Hadamard matrix, so the four are orthonormal. For each, the component
//! `s = c·y` of the lines' outputs is replaced by `√(1−a)·s + √a·z`, `z` the shifted,
//! filtered `s`. The shifter's normaliser makes `z`'s power `s`'s, and `z` is uncorrelated
//! with `s` (it is another pitch, read from an older part of the line), so the tank's
//! energy after the exchange equals the energy before — at any amount, at infinite decay:
//! **the shimmer moves energy, it does not make it**. The tone filters on `z` only take.

// The DSP here walks parallel arrays (lines, channels) with one index; that reads clearer
// than zipping several iterators.
#![allow(clippy::needless_range_loop)]

use super::parts::{one_pole_g, phase_inc, OnePole, SineTable, Svf2, SvfCoef};
use super::shift::Shifter;
use crate::math;
use crate::rest::ZeroRun;
use crate::spatium::fdn::glide;
use crate::spatium::model::hash01;

pub const MAX_LINES: usize = 16;
/// Shimmer channels: voice A on two rows, voice B on two.
pub const CHANNELS: usize = 4;
/// The longest line, ms (at the largest size, with its spread and jitter).
pub const MAX_LINE_MS: f32 = 520.0;
/// The deepest modulation, ms.
pub const MAX_DEPTH_MS: f32 = 4.0;
/// The decay the shelves keep at infinite, s.
pub const INFINITE_SHELF_S: f32 = 60.0;
/// A frozen loop's gain per pass.
pub const FREEZE_GAIN: f32 = 0.999_99;
/// The hold's largest trim per pass.
const HOLD_STEP: f32 = 0.005;

/// How many lines the tank runs.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Lines {
    /// 8 lines: half the cost, a little less dense.
    Eight,
    /// 16 lines.
    #[default]
    Sixteen,
}

impl Lines {
    pub fn count(self) -> usize {
        match self {
            Lines::Eight => 8,
            Lines::Sixteen => 16,
        }
    }

    pub fn from_index(i: usize) -> Lines {
        if i == 0 {
            Lines::Eight
        } else {
            Lines::Sixteen
        }
    }
}

/// Signs and permutations of the two matrices (`P·H·D`).
const SIGNS16: [f32; 16] = [
    1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0, -1.0,
];
const PERM16: [usize; 16] = [9, 4, 14, 1, 11, 6, 0, 13, 3, 15, 7, 10, 2, 12, 5, 8];
const SIGNS8: [f32; 8] = [1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0];
const PERM8: [usize; 8] = [5, 2, 7, 0, 6, 3, 1, 4];
/// Rows of the Hadamard matrix: the two inputs, the two outputs, the four shimmer channels.
const IN_ROWS: [usize; 2] = [3, 5];
const OUT_ROWS: [usize; 2] = [6, 9];
const OUT_ROWS8: [usize; 2] = [6, 1];
const SHIM_ROWS16: [usize; CHANNELS] = [10, 12, 7, 15];
const SHIM_ROWS8: [usize; CHANNELS] = [2, 4, 7, 3];

/// Entry `j` of row `r` of the Sylvester–Hadamard matrix (±1).
pub fn hadamard_entry(r: usize, j: usize) -> f32 {
    if (r & j).count_ones().is_multiple_of(2) {
        1.0
    } else {
        -1.0
    }
}

/// In-place fast Walsh–Hadamard transform of the first `n` values (8 or 16), normalised.
#[inline]
pub fn hadamard(v: &mut [f32; MAX_LINES], n: usize) {
    let mut h = 1;
    while h < n {
        let mut i = 0;
        while i < n {
            for j in i..i + h {
                let a = v[j];
                let b = v[j + h];
                v[j] = a + b;
                v[j + h] = a - b;
            }
            i += 2 * h;
        }
        h *= 2;
    }
    let k = if n == 16 { 0.25 } else { 0.353_553_38 };
    for x in v.iter_mut().take(n) {
        *x *= k;
    }
}

/// The matrix `P·H·D` applied to the first `n` values.
#[inline]
pub fn mix_matrix(v: &[f32; MAX_LINES], n: usize, out: &mut [f32; MAX_LINES]) {
    let (signs, perm): (&[f32], &[usize]) = if n == 16 {
        (&SIGNS16, &PERM16)
    } else {
        (&SIGNS8, &PERM8)
    };
    let mut t = [0.0f32; MAX_LINES];
    for i in 0..n {
        t[i] = v[i] * signs[i];
    }
    hadamard(&mut t, n);
    for i in 0..n {
        out[i] = t[perm[i]];
    }
}

fn is_prime(n: u32) -> bool {
    if n < 2 {
        return false;
    }
    if n.is_multiple_of(2) {
        return n == 2;
    }
    let mut d = 3;
    while d * d <= n {
        if n.is_multiple_of(d) {
            return false;
        }
        d += 2;
    }
    true
}

/// The nearest prime to `target` not in `taken`.
fn nearest_prime(target: u32, taken: &[f32]) -> u32 {
    for step in 0..100_000u32 {
        let free = |n: u32| is_prime(n) && !taken.iter().any(|x| *x as u32 == n);
        if free(target + step) {
            return target + step;
        }
        if step < target && free(target - step) {
            return target - step;
        }
    }
    target
}

/// The mean line length at a size (0…1), ms: 15 ms (a small chamber) to 320 ms (a space
/// no building holds), geometric.
pub fn mean_line_ms(size: f32) -> f32 {
    15.0 * math::powf(320.0f32 / 15.0, size.clamp(0.0, 1.0))
}

/// The lines' lengths for a size, samples (distinct primes): spread over 1.2 octaves
/// around the mean, each jittered by ±4 % so no two sizes share a pattern.
pub fn line_lengths(size: f32, n: usize, sample_rate: f32) -> [f32; MAX_LINES] {
    let mean = mean_line_ms(size);
    let mut out = [0.0f32; MAX_LINES];
    for i in 0..n {
        let x = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.5 };
        let spread = math::powf(2f32, (x - 0.5) * 1.2);
        let jitter = 1.0 + 0.08 * (hash01(i as u32, 211) - 0.5);
        let ms = (mean * spread * jitter).min(MAX_LINE_MS);
        let target = ((ms * 0.001 * sample_rate) as u32).max(16);
        out[i] = nearest_prime(target, &out[..i]) as f32;
    }
    out
}

/// The in-loop all-passes' lengths, samples: 3 to 12 ms, fixed (a length that moved
/// would click), distinct primes.
pub fn allpass_lengths(sample_rate: f32) -> [usize; MAX_LINES] {
    let mut f = [0.0f32; MAX_LINES];
    let mut out = [0usize; MAX_LINES];
    for i in 0..MAX_LINES {
        let ms = 3.0 + 9.0 * hash01(i as u32, 307);
        let p = nearest_prime(((ms * 0.001 * sample_rate) as u32).max(7), &f[..i]);
        f[i] = p as f32;
        out[i] = p as usize;
    }
    out
}

/// What the decay is set to.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Decay {
    /// The middle band's RT60, s.
    pub t60_s: f32,
    /// The middle held for good (the shelves kept as at [`INFINITE_SHELF_S`]).
    pub infinite: bool,
    /// The high shelf's corner, Hz, and the highs' decay as a fraction of the middle's.
    pub high_hz: f32,
    pub high_ratio: f32,
    /// The low shelf's corner, Hz, and the lows' decay as a multiple of the middle's.
    pub low_hz: f32,
    pub low_ratio: f32,
}

/// The tank's design: the lengths and each line's absorption.
#[derive(Clone, Debug, PartialEq)]
pub struct TankDesign {
    pub sample_rate: f32,
    pub lines: usize,
    /// Each line's delay, samples (distinct primes).
    pub lengths: [f32; MAX_LINES],
    /// Each line's whole loop: the delay and its all-pass, samples.
    pub loop_len: [f32; MAX_LINES],
    /// The middle's gain a pass, the high shelf's gain at Nyquist, the low shelf's at DC.
    pub g_mid: [f32; MAX_LINES],
    pub hi: [f32; MAX_LINES],
    pub lo: [f32; MAX_LINES],
    pub high_hz: f32,
    pub low_hz: f32,
}

/// The gain a pass of `len` samples loses in a decay of `t60` s.
fn pass_gain(len: f32, t60: f32, fs: f32) -> f64 {
    math::powf(10f64, -3.0 * len as f64 / (fs as f64 * t60.max(1e-3) as f64))
}

impl TankDesign {
    pub fn new(
        sample_rate: f32,
        lines: usize,
        size: f32,
        decay: &Decay,
        ap: &[usize; MAX_LINES],
    ) -> Self {
        let lengths = line_lengths(size, lines, sample_rate);
        let mut d = TankDesign {
            sample_rate,
            lines,
            lengths,
            loop_len: [0.0; MAX_LINES],
            g_mid: [1.0; MAX_LINES],
            hi: [1.0; MAX_LINES],
            lo: [1.0; MAX_LINES],
            high_hz: decay.high_hz.clamp(200.0, 0.45 * sample_rate),
            low_hz: decay.low_hz.clamp(20.0, 2_000.0),
        };
        let hr = decay.high_ratio.clamp(0.02, 1.0);
        let lr = decay.low_ratio.clamp(0.1, 4.0);
        for i in 0..lines {
            let l = lengths[i] + ap[i] as f32;
            d.loop_len[i] = l;
            if decay.infinite {
                let t = INFINITE_SHELF_S;
                let mid = pass_gain(l, t, sample_rate);
                d.g_mid[i] = 1.0;
                d.hi[i] = (pass_gain(l, t * hr, sample_rate) / mid) as f32;
                d.lo[i] = ((pass_gain(l, t * lr, sample_rate) / mid) as f32).min(1.0);
            } else {
                let t = decay.t60_s;
                let mid = pass_gain(l, t, sample_rate);
                d.g_mid[i] = mid as f32;
                d.hi[i] = (pass_gain(l, t * hr, sample_rate) / mid) as f32;
                d.lo[i] = (pass_gain(l, t * lr, sample_rate) / mid) as f32;
            }
        }
        d
    }

    /// Line `i`'s gain a pass at `hz` (linear): the middle's and the two shelves'.
    pub fn pass_gain(&self, i: usize, hz: f32) -> f64 {
        let fs = self.sample_rate as f64;
        let warp = |f: f64| math::tan(std::f64::consts::PI * f.min(0.4999 * fs) / fs);
        let w = warp(hz as f64);
        let oh = w / warp(self.high_hz as f64);
        let ol = w / warp(self.low_hz as f64);
        let h = self.hi[i] as f64;
        let l = self.lo[i] as f64;
        let hs = ((1.0 + h * h * oh * oh) / (1.0 + oh * oh)).sqrt();
        let ls = ((l * l + ol * ol) / (1.0 + ol * ol)).sqrt();
        self.g_mid[i] as f64 * hs * ls
    }

    /// Line `i`'s own decay at `hz`, s (`f32::INFINITY` for a lossless one).
    pub fn line_t60(&self, i: usize, hz: f32) -> f32 {
        let db = 20.0 * math::log10(self.pass_gain(i, hz).max(1e-30));
        if db >= -1e-9 {
            return f32::INFINITY;
        }
        (-60.0 * self.loop_len[i] as f64 / (self.sample_rate as f64 * db)) as f32
    }

    /// The decay the tank achieves at `hz`, s: from the mean loss per sample over the
    /// lines, which is how its modes decay on average.
    pub fn achieved_t60(&self, hz: f32) -> f32 {
        let mut per_sample = 0.0f64;
        for i in 0..self.lines {
            per_sample +=
                20.0 * math::log10(self.pass_gain(i, hz).max(1e-30)) / self.loop_len[i] as f64;
        }
        per_sample /= self.lines as f64;
        if per_sample >= -1e-12 {
            return f32::INFINITY;
        }
        (-60.0 / (self.sample_rate as f64 * per_sample)) as f32
    }

    /// The mean loop length, samples.
    pub fn mean_loop(&self) -> f32 {
        self.loop_len[..self.lines].iter().sum::<f32>() / self.lines as f32
    }
}

/// A tank write under this, for as long as the longest line, means the tank holds nothing
/// above −120 dB.
pub const SETTLE: f32 = 1e-6;

/// The shimmer as the tank takes it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Shimmer {
    /// 0…1: how much of each channel's energy goes through its shifter each pass.
    pub amount: f32,
    /// Voice A's and voice B's shifts, semitones.
    pub a_st: f32,
    pub b_st: f32,
    /// Voice B's share of the amount, 0…1 (0: voice A alone).
    pub b_level: f32,
    /// The tone in the loop: a low cut and a high cut on what the shifters give back, Hz
    /// (at or below 20 and at or above 20 000 they are off).
    pub low_hz: f32,
    pub high_hz: f32,
}

impl Default for Shimmer {
    fn default() -> Self {
        Shimmer {
            amount: 0.0,
            a_st: 12.0,
            b_st: 7.0,
            b_level: 0.0,
            low_hz: 20.0,
            high_hz: 20_000.0,
        }
    }
}

/// The tank's running state.
#[derive(Clone, Debug)]
pub struct Tank {
    sr: f32,
    n: usize,
    size: usize,
    mask: usize,
    buf: Vec<f32>,
    w: usize,
    len: [f32; MAX_LINES],
    len_target: [f32; MAX_LINES],
    k_len: f32,
    interp: [f32; MAX_LINES],
    lfo: [u32; MAX_LINES],
    lfo_inc: [u32; MAX_LINES],
    depth: f32,
    depth_target: f32,
    depth_scale: [f32; MAX_LINES],
    ap_size: usize,
    ap_mask: usize,
    ap_buf: Vec<f32>,
    ap_w: usize,
    ap_len: [usize; MAX_LINES],
    diff: f32,
    diff_target: f32,
    hs: [OnePole; MAX_LINES],
    ls: [OnePole; MAX_LINES],
    g_hi: f32,
    g_lo: f32,
    design: TankDesign,
    freeze: f32,
    freeze_target: f32,
    k_gain: f32,
    infinite: bool,
    /// The hold and the governor: the lines' energy (smoothed), the level held, samples
    /// frozen, the ceiling, the trim last applied.
    energy: f32,
    energy_ref: f32,
    k_energy: f32,
    frozen_for: u32,
    ceiling: f32,
    trim: f32,
    b: [[f32; MAX_LINES]; 2],
    c: [[f32; MAX_LINES]; 2],
    rows: [[f32; MAX_LINES]; CHANNELS],
    shifters: [Shifter; CHANNELS],
    amount: [f32; CHANNELS],
    amount_target: [f32; CHANNELS],
    /// `√(1 − a) − 1` and `√a` for each channel's amount now.
    keep: [f32; CHANNELS],
    give: [f32; CHANNELS],
    sh_hp: [OnePole; CHANNELS],
    sh_lp: [Svf2; CHANNELS],
    sh_hp_g: f32,
    sh_lp_c: SvfCoef,
    sh_hp_on: bool,
    sh_lp_on: bool,
    shimmer: Shimmer,
    /// What the shimmer gave back last sample (the meter's).
    sh_power: f32,
    zeros: [ZeroRun; MAX_LINES],
    ap_zeros: [ZeroRun; MAX_LINES],
    quiet: usize,
    table: SineTable,
    configured: bool,
}

impl Tank {
    pub fn new(sample_rate: f32) -> Self {
        let fs = sample_rate;
        let need = (fs * (MAX_LINE_MS + MAX_DEPTH_MS + 2.0) * 0.001) as usize + 8;
        let size = need.next_power_of_two();
        let ap_len = allpass_lengths(fs);
        let ap_size = (ap_len.iter().max().copied().unwrap_or(1) + 2).next_power_of_two();
        let mut depth_scale = [0.0; MAX_LINES];
        let mut lfo = [0u32; MAX_LINES];
        for i in 0..MAX_LINES {
            depth_scale[i] = 0.6 + 0.4 * hash01(i as u32, 71);
            lfo[i] = (i as u32).wrapping_mul(0x9E37_79B9);
        }
        let design = TankDesign::new(
            fs,
            16,
            0.5,
            &Decay {
                t60_s: 2.0,
                infinite: false,
                high_hz: 6_000.0,
                high_ratio: 0.5,
                low_hz: 250.0,
                low_ratio: 1.0,
            },
            &ap_len,
        );
        let mut t = Tank {
            sr: fs,
            n: 16,
            size,
            mask: size - 1,
            buf: vec![0.0; size * MAX_LINES],
            w: 0,
            len: [1000.0; MAX_LINES],
            len_target: [1000.0; MAX_LINES],
            k_len: 1.0 - math::exp(-1.0 / (0.08 * fs)),
            interp: [0.0; MAX_LINES],
            lfo,
            lfo_inc: [0; MAX_LINES],
            depth: 0.0,
            depth_target: 0.0,
            depth_scale,
            ap_size,
            ap_mask: ap_size - 1,
            ap_buf: vec![0.0; ap_size * MAX_LINES],
            ap_w: 0,
            ap_len,
            diff: 0.0,
            diff_target: 0.0,
            hs: [OnePole::default(); MAX_LINES],
            ls: [OnePole::default(); MAX_LINES],
            g_hi: 0.5,
            g_lo: 0.5,
            design,
            freeze: 0.0,
            freeze_target: 0.0,
            k_gain: 1.0 - math::exp(-1.0 / (0.03 * fs)),
            infinite: false,
            energy: 0.0,
            energy_ref: 0.0,
            k_energy: 1.0 - math::exp(-1.0 / (0.1 * fs)),
            frozen_for: 0,
            ceiling: f32::MAX,
            trim: 1.0,
            b: [[0.0; MAX_LINES]; 2],
            c: [[0.0; MAX_LINES]; 2],
            rows: [[0.0; MAX_LINES]; CHANNELS],
            shifters: [
                Shifter::with_origin(fs, 0),
                Shifter::with_origin(fs, 0x5555_0000),
                Shifter::with_origin(fs, 0x2AAA_0000),
                Shifter::with_origin(fs, 0x8000_0000),
            ],
            amount: [0.0; CHANNELS],
            amount_target: [0.0; CHANNELS],
            keep: [0.0; CHANNELS],
            give: [0.0; CHANNELS],
            sh_hp: [OnePole::default(); CHANNELS],
            sh_lp: [Svf2::default(); CHANNELS],
            sh_hp_g: 0.0,
            sh_lp_c: SvfCoef::new(10_000.0, fs),
            sh_hp_on: false,
            sh_lp_on: false,
            shimmer: Shimmer::default(),
            sh_power: 0.0,
            zeros: [ZeroRun::FULL; MAX_LINES],
            ap_zeros: [ZeroRun::FULL; MAX_LINES],
            quiet: usize::MAX / 2,
            table: SineTable::new(),
            configured: false,
        };
        t.set_rows(16);
        t
    }

    fn set_rows(&mut self, n: usize) {
        let k = 1.0 / (n as f32).sqrt();
        let out_rows = if n == 16 { OUT_ROWS } else { OUT_ROWS8 };
        let shim = if n == 16 { SHIM_ROWS16 } else { SHIM_ROWS8 };
        for i in 0..MAX_LINES {
            let on = i < n;
            for s in 0..2 {
                self.b[s][i] = if on { k * hadamard_entry(IN_ROWS[s], i) } else { 0.0 };
                self.c[s][i] = if on { k * hadamard_entry(out_rows[s], i) } else { 0.0 };
            }
            for (ch, r) in shim.iter().enumerate() {
                self.rows[ch][i] = if on { k * hadamard_entry(*r, i) } else { 0.0 };
            }
        }
    }

    pub fn lines(&self) -> usize {
        self.n
    }

    pub fn design(&self) -> &TankDesign {
        &self.design
    }

    /// The longest delay the lines hold, samples.
    pub fn capacity(&self) -> usize {
        self.size - 8
    }
}

impl Tank {
    /// Every buffer and state cleared; the counts (write positions, phases) kept.
    pub fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.ap_buf.iter_mut().for_each(|x| *x = 0.0);
        self.zeros = [ZeroRun::FULL; MAX_LINES];
        self.ap_zeros = [ZeroRun::FULL; MAX_LINES];
        for s in &mut self.shifters {
            s.clear();
        }
        self.flush_states();
        self.quiet = usize::MAX / 2;
    }

    /// The small states: what a drain clears every sample.
    fn flush_states(&mut self) {
        self.interp = [0.0; MAX_LINES];
        for f in self.hs.iter_mut().chain(self.ls.iter_mut()) {
            f.clear();
        }
        for f in &mut self.sh_hp {
            f.clear();
        }
        for f in &mut self.sh_lp {
            f.clear();
        }
        for s in &mut self.shifters {
            s.p_in = 0.0;
            s.p_out = 0.0;
        }
        self.energy = 0.0;
        self.energy_ref = 0.0;
        self.frozen_for = 0;
        self.trim = 1.0;
        self.sh_power = 0.0;
    }

    /// As new: cleared, the counts back to their start, every glide landed.
    pub fn reset(&mut self) {
        self.clear();
        self.w = 0;
        self.ap_w = 0;
        for i in 0..MAX_LINES {
            self.lfo[i] = (i as u32).wrapping_mul(0x9E37_79B9);
        }
        for s in &mut self.shifters {
            s.reset();
        }
        self.len = self.len_target;
        self.depth = self.depth_target;
        self.diff = self.diff_target;
        self.freeze = self.freeze_target;
        self.amount = self.amount_target;
        self.update_amounts();
    }

    fn update_amounts(&mut self) {
        for ch in 0..CHANNELS {
            let a = self.amount[ch].clamp(0.0, 1.0);
            self.keep[ch] = (1.0 - a).sqrt() - 1.0;
            self.give[ch] = a.sqrt();
        }
    }

    /// A new design. The lengths glide to theirs; a change in the number of lines clears
    /// the tank (it is a quality choice, like an oversampling factor).
    pub fn set_design(&mut self, d: &TankDesign) {
        let cap = self.capacity() as f32;
        let restart = d.lines != self.n;
        if restart {
            self.n = d.lines;
            self.set_rows(d.lines);
            self.clear();
        }
        for i in 0..MAX_LINES {
            self.len_target[i] = if i < d.lines {
                d.lengths[i].clamp(16.0, cap - 4.0)
            } else {
                1000.0f32.min(cap - 4.0)
            };
        }
        if !self.configured || restart {
            self.len = self.len_target;
            self.configured = true;
        }
        self.g_hi = one_pole_g(d.high_hz, self.sr);
        self.g_lo = one_pole_g(d.low_hz, self.sr);
        self.design = d.clone();
    }

    /// The lines' modulation: a depth in samples and a rate in Hz (each line its own,
    /// between 0.6 and 1.4 times it).
    pub fn set_modulation(&mut self, depth_samples: f32, rate_hz: f32) {
        self.depth_target = depth_samples.max(0.0);
        for i in 0..MAX_LINES {
            let r = rate_hz as f64 * (0.6 + 0.8 * hash01(i as u32, 97) as f64);
            self.lfo_inc[i] = phase_inc(r, self.sr);
        }
    }

    /// The in-loop all-passes' gain (0…0.6).
    pub fn set_diffusion(&mut self, g: f32) {
        self.diff_target = g.clamp(0.0, 0.6);
    }

    pub fn set_freeze(&mut self, on: bool) {
        self.freeze_target = if on { 1.0 } else { 0.0 };
        self.apply_shimmer();
    }

    pub fn set_infinite(&mut self, on: bool) {
        self.infinite = on;
    }

    /// The governor's ceiling: the lines' summed energy (smoothed) above which the loop
    /// gain comes down.
    pub fn set_ceiling(&mut self, energy: f32) {
        self.ceiling = energy.max(1e-12);
    }

    pub fn set_shimmer(&mut self, s: &Shimmer) {
        self.shimmer = *s;
        self.apply_shimmer();
    }

    fn apply_shimmer(&mut self) {
        let s = self.shimmer;
        // With 8 lines four channels are half the tank: half the amount moves the same
        // share of its energy a pass.
        let scale = if self.n == 16 { 1.0 } else { 0.5 };
        let frozen = self.freeze_target > 0.0;
        let a = if frozen { 0.0 } else { s.amount.clamp(0.0, 1.0) * scale };
        let b = s.b_level.clamp(0.0, 1.0);
        self.amount_target = [a, a, a * b, a * b];
        self.shifters[0].set_semitones(s.a_st);
        self.shifters[1].set_semitones(s.a_st);
        self.shifters[2].set_semitones(s.b_st);
        self.shifters[3].set_semitones(s.b_st);
        self.sh_hp_on = s.low_hz > 20.0;
        self.sh_hp_g = one_pole_g(s.low_hz.max(20.0), self.sr);
        self.sh_lp_on = s.high_hz < 20_000.0;
        self.sh_lp_c = SvfCoef::new(s.high_hz.min(20_000.0), self.sr);
    }

    pub fn freeze_amount(&self) -> f32 {
        self.freeze
    }

    /// The trim the hold and the governor applied last sample (1: none).
    pub fn trim(&self) -> f32 {
        self.trim
    }

    /// The lines' summed energy, smoothed over 100 ms.
    pub fn energy(&self) -> f32 {
        self.energy
    }

    /// The power the shimmer gave back last sample.
    pub fn shimmer_power(&self) -> f32 {
        self.sh_power
    }

    /// The mean line length now, samples.
    pub fn mean_length(&self) -> f32 {
        self.len[..self.n].iter().sum::<f32>() / self.n as f32
    }

    pub fn depth(&self) -> f32 {
        self.depth
    }

    /// Every write has been under [`SETTLE`] for as long as the longest line: the tank
    /// holds nothing above −120 dB.
    pub fn quiet_long(&self) -> bool {
        self.quiet >= self.size
    }

    /// The hold and the governor: the trim for this sample, from the lines' energy.
    fn hold(&mut self, e: f32) -> f32 {
        self.energy += (e - self.energy) * self.k_energy;
        if self.energy < 1e-30 {
            self.energy = 0.0;
        }
        let mut t = 1.0f32;
        if self.freeze >= 1.0 {
            // An empty frozen tank counts nothing: it is at rest, as a new one.
            if self.energy > 0.0 {
                self.frozen_for = self.frozen_for.saturating_add(1);
            }
            if self.energy_ref == 0.0 {
                if self.frozen_for as f32 >= 0.15 * self.sr {
                    self.energy_ref = self.energy.max(1e-30);
                }
            } else if self.energy > 1e-24 {
                t = (1.0 + 0.5 * (self.energy_ref / self.energy - 1.0))
                    .clamp(1.0 - HOLD_STEP, 1.0 + HOLD_STEP);
            }
        } else if self.infinite && self.freeze == 0.0 {
            if self.energy > self.energy_ref {
                self.energy_ref = self.energy;
            } else if self.energy > 1e-24 {
                t = (1.0 + 0.5 * (self.energy_ref / self.energy - 1.0)).clamp(1.0, 1.0 + HOLD_STEP);
            }
        } else if self.frozen_for != 0 || self.energy_ref != 0.0 {
            self.frozen_for = 0;
            self.energy_ref = 0.0;
        }
        if self.energy > self.ceiling {
            t *= (self.ceiling / self.energy).sqrt().max(0.5);
        }
        self.trim = t;
        t
    }
}

impl Tank {
    /// One sample: the stereo input in, the stereo late field out. While `drain`, every
    /// write is +0.0 and the small states are cleared (the caller drains a tank that holds
    /// nothing above −120 dB and hears nothing, so it comes to rest exactly).
    #[inline]
    pub fn tick(&mut self, in_l: f32, in_r: f32, drain: bool) -> (f32, f32) {
        let n = self.n;
        let size = self.size;
        let mask = self.mask;
        let w = self.w;
        let ap_w = self.ap_w;
        glide(&mut self.depth, self.depth_target, self.k_gain, 1e-4);
        glide(&mut self.diff, self.diff_target, self.k_gain, 1e-6);
        let g = self.diff;
        let mut y = [0.0f32; MAX_LINES];
        let mut peak = 0.0f32;
        for i in 0..n {
            glide(&mut self.len[i], self.len_target[i], self.k_len, 1e-3);
            let mut d = self.len[i];
            let ph = self.lfo[i];
            self.lfo[i] = ph.wrapping_add(self.lfo_inc[i]);
            if self.depth > 0.0 {
                let dep = (self.depth * self.depth_scale[i])
                    .min(0.45 * d - 2.0)
                    .max(0.0);
                d += dep * self.table.sin(ph);
            }
            // The all-pass interpolator: lossless at every frequency, so the modulation
            // costs the decay nothing.
            let nf = (d - 0.5).floor();
            let frac = d - nf;
            let k = nf as usize;
            let base = i * size;
            let x0 = self.buf[base + (w.wrapping_sub(k) & mask)];
            let x1 = self.buf[base + (w.wrapping_sub(k + 1) & mask)];
            let out = if frac == 1.0 {
                x1
            } else {
                let eta = (1.0 - frac) / (1.0 + frac);
                eta * (x0 - self.interp[i]) + x1
            };
            let out = if out.abs() < 1e-20 { 0.0 } else { out };
            self.interp[i] = out;
            // The line's own all-pass.
            let ab = i * self.ap_size;
            let dd = self.ap_buf[ab + (ap_w.wrapping_sub(self.ap_len[i]) & self.ap_mask)];
            let v = out + g * dd;
            let v = if v.abs() < 1e-20 || drain { 0.0 } else { v };
            peak = peak.max(v.abs());
            self.ap_buf[ab + ap_w] = v;
            y[i] = dd - g * v;
        }
        // Absorption: the high shelf, the low shelf, the middle's gain.
        let mut e = 0.0f32;
        let mut v = [0.0f32; MAX_LINES];
        for i in 0..n {
            let x = y[i];
            e += x * x;
            let lp = self.hs[i].lp(x, self.g_hi);
            let a = lp + self.design.hi[i] * (x - lp);
            let lp2 = self.ls[i].lp(a, self.g_lo);
            let b = self.design.lo[i] * lp2 + (a - lp2);
            v[i] = b * self.design.g_mid[i];
        }
        glide(&mut self.freeze, self.freeze_target, self.k_gain, 1e-5);
        if self.freeze > 0.0 {
            let f = self.freeze;
            for i in 0..n {
                v[i] += (y[i] * FREEZE_GAIN - v[i]) * f;
            }
        }
        let mut ol = 0.0f32;
        let mut or = 0.0f32;
        for i in 0..n {
            ol += self.c[0][i] * v[i];
            or += self.c[1][i] * v[i];
        }
        self.exchange(&mut v, drain);
        let t = self.hold(e);
        if t != 1.0 {
            for x in v.iter_mut().take(n) {
                *x *= t;
            }
        }
        let mut m = [0.0f32; MAX_LINES];
        mix_matrix(&v, n, &mut m);
        for i in 0..n {
            // `+ 0.0` turns a −0.0 into +0.0 (zero is +0.0, bit for bit — ADR 0031 §5.1).
            let x = if drain {
                0.0
            } else {
                m[i] + self.b[0][i] * in_l + self.b[1][i] * in_r + 0.0
            };
            peak = peak.max(x.abs());
            self.buf[i * size + w] = x;
        }
        self.quiet = if peak < SETTLE {
            self.quiet.saturating_add(1)
        } else {
            0
        };
        if drain {
            self.flush_states();
        }
        self.w = (w + 1) & mask;
        self.ap_w = (ap_w + 1) & self.ap_mask;
        (ol, or)
    }

    /// The shimmer's exchange: each channel's component `s` replaced by `√(1−a)·s + √a·z`.
    #[inline]
    fn exchange(&mut self, v: &mut [f32; MAX_LINES], drain: bool) {
        let n = self.n;
        let mut pw = 0.0f32;
        for ch in 0..CHANNELS {
            let target = self.amount_target[ch];
            if self.amount[ch] != target {
                glide(&mut self.amount[ch], target, self.k_gain, 1e-6);
                let a = self.amount[ch];
                self.keep[ch] = (1.0 - a).sqrt() - 1.0;
                self.give[ch] = a.sqrt();
                if a == 0.0 {
                    // Landed at nothing: the channel starts clean when it comes back.
                    self.shifters[ch].clear();
                    self.sh_hp[ch].clear();
                    self.sh_lp[ch].clear();
                }
            }
            if self.amount[ch] == 0.0 {
                self.shifters[ch].skip(1);
                continue;
            }
            let row = &self.rows[ch];
            let mut s = 0.0f32;
            for i in 0..n {
                s += row[i] * v[i];
            }
            let mut z = self.shifters[ch].tick(if drain { 0.0 } else { s }, &self.table);
            if self.sh_hp_on {
                let lp = self.sh_hp[ch].lp(z, self.sh_hp_g);
                z -= lp;
            }
            if self.sh_lp_on {
                z = self.sh_lp[ch].run(z, &self.sh_lp_c).0;
            }
            let give = self.give[ch];
            let delta = self.keep[ch] * s + give * z;
            pw += give * give * z * z;
            for i in 0..n {
                v[i] += row[i] * delta;
            }
        }
        self.sh_power = pw;
    }

    /// After a block of `n` ticks: every ring's run of zeros (one comparison a ring while
    /// sound passes).
    pub fn wrote(&mut self, n: usize) {
        let size = self.size;
        let aps = self.ap_size;
        for i in 0..self.n {
            self.zeros[i].wrote(&self.buf[i * size..(i + 1) * size], self.w, n);
            self.ap_zeros[i].wrote(&self.ap_buf[i * aps..(i + 1) * aps], self.ap_w, n);
        }
        for s in &mut self.shifters {
            s.wrote(n);
        }
    }

    /// At rest (ADR 0031 §5.1): every ring and state +0.0, every glide landed, nothing
    /// frozen — silence in gives silence out, and only the counts move.
    pub fn rest_blocker(&self) -> Option<&'static str> {
        use crate::rest::{all_zero, z};
        let n = self.n;
        if !self.zeros[..n].iter().all(|r| r.covers(self.size)) {
            return Some("tank lines");
        }
        if !self.ap_zeros[..n].iter().all(|r| r.covers(self.ap_size)) {
            return Some("tank all-passes");
        }
        if !all_zero(&self.interp) || !self.hs.iter().chain(&self.ls).all(|f| f.is_zero()) {
            return Some("tank filters");
        }
        if !self.shifters.iter().all(|s| s.is_zero())
            || !self.sh_hp.iter().all(|f| f.is_zero())
            || !self.sh_lp.iter().all(|f| f.is_zero())
        {
            return Some("shimmer");
        }
        if self.len != self.len_target
            || self.depth != self.depth_target
            || self.diff != self.diff_target
            || self.amount != self.amount_target
        {
            return Some("tank glides");
        }
        if self.freeze != self.freeze_target || self.frozen_for != 0 {
            return Some("freeze");
        }
        if !z(self.energy) || !z(self.energy_ref) {
            return Some("hold");
        }
        None
    }

    /// `n` samples of silence not run, at rest: the write positions, the modulation's
    /// phases and the shifters' heads move as `n` ticks would move them.
    pub fn skip(&mut self, n: usize) {
        self.w = (self.w + n) & self.mask;
        self.ap_w = (self.ap_w + n) & self.ap_mask;
        for i in 0..self.n {
            self.lfo[i] = self.lfo[i].wrapping_add(self.lfo_inc[i].wrapping_mul(n as u32));
        }
        for s in &mut self.shifters {
            s.skip(n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_matrices_are_orthogonal_and_not_involutions() {
        for n in [8usize, 16] {
            for r in 0..n {
                let mut e = [0.0f32; MAX_LINES];
                e[r] = 1.0;
                let mut once = [0.0f32; MAX_LINES];
                mix_matrix(&e, n, &mut once);
                let p: f32 = once.iter().map(|x| x * x).sum();
                assert!((p - 1.0).abs() < 1e-6, "{n}: row {r} power {p}");
                let mut twice = [0.0f32; MAX_LINES];
                mix_matrix(&once, n, &mut twice);
                assert!(twice[r].abs() < 0.9, "{n}: row {r} returns {}", twice[r]);
            }
            let mut sorted: Vec<usize> = if n == 16 { PERM16.to_vec() } else { PERM8.to_vec() };
            sorted.sort();
            assert!(sorted.iter().enumerate().all(|(i, p)| i == *p));
        }
    }

    #[test]
    fn the_lengths_are_distinct_primes_and_fit() {
        for n in [8usize, 16] {
            for size in [0.0f32, 0.5, 1.0] {
                let l = line_lengths(size, n, 48_000.0);
                for i in 0..n {
                    assert!(is_prime(l[i] as u32));
                    assert!(l[i] <= MAX_LINE_MS * 48.0 + 1.0);
                    for j in 0..i {
                        assert_ne!(l[i], l[j]);
                    }
                }
            }
        }
        let ap = allpass_lengths(48_000.0);
        for i in 0..MAX_LINES {
            assert!(is_prime(ap[i] as u32));
        }
    }

    /// The design's every line loses `60·L/(fs·T)` dB a pass at both ends of the spectrum
    /// and in the middle (far from both corners), for finite decays.
    #[test]
    fn every_line_decays_at_its_band_s_rate() {
        let ap = allpass_lengths(48_000.0);
        let decay = Decay {
            t60_s: 3.0,
            infinite: false,
            high_hz: 8_000.0,
            high_ratio: 0.4,
            low_hz: 150.0,
            low_ratio: 1.5,
        };
        let d = TankDesign::new(48_000.0, 16, 0.6, &decay, &ap);
        for i in 0..16 {
            let mid = d.line_t60(i, 1_100.0);
            let top = d.line_t60(i, 23_900.0);
            let bottom = d.line_t60(i, 3.0);
            assert!((mid / 3.0 - 1.0).abs() < 0.06, "line {i}: middle {mid}");
            assert!((top / 1.2 - 1.0).abs() < 0.03, "line {i}: top {top}");
            assert!((bottom / 4.5 - 1.0).abs() < 0.03, "line {i}: bottom {bottom}");
        }
    }

    #[test]
    fn infinite_holds_the_middle_and_never_gains() {
        let ap = allpass_lengths(48_000.0);
        let decay = Decay {
            t60_s: 3.0,
            infinite: true,
            high_hz: 5_000.0,
            high_ratio: 0.3,
            low_hz: 200.0,
            low_ratio: 3.0,
        };
        let d = TankDesign::new(48_000.0, 16, 1.0, &decay, &ap);
        for i in 0..16 {
            for k in 0..400 {
                let hz = 5.0 + 60.0 * k as f32;
                assert!(d.pass_gain(i, hz) <= 1.0 + 1e-9, "line {i} gains at {hz}");
            }
        }
        assert!(d.achieved_t60(1_000.0) > 200.0);
    }
}
