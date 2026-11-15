//! Caelum's way in: the freeze gate, the pre-delay, the **bloom** and the input diffusers.
//!
//! ## Bloom
//!
//! A reverb's density usually arrives at once: the first reflections, then the field. A
//! bloom makes it *arrive slowly* — the sound swells into the space instead of being struck
//! in it. Here it is a pattern of [`TAPS`] taps per side spread over the bloom time after
//! the pre-delay, **sparse at first and dense at the end** (tap *k* at `T·((k+1)/K)^0.6`),
//! their gains rising with time (as `u^1.2`) and normalised to the energy of one tap, so a
//! bloom changes how the reverb's energy arrives, not how much of it there is. The two
//! sides' patterns are jittered apart and their signs scrambled, so the swell is wide. The
//! taps feed the diffusers and the tank, which turn the sparse taps into a field whose
//! density, like its level, rises over the bloom time.
//!
//! The bloom cross-fades with the plain pre-delayed input (equal power), so turning it on
//! or off glides; at a bloom of 0 the taps are not read at all.
//!
//! ## The diffusers
//!
//! Four Schroeder all-passes a side (2.3 to 8.3 ms, 20 ms in all, the two sides' lengths
//! apart) turn each tap into a cluster before the tank. They always run — at a diffusion of
//! 0 they are four plain delays, so the onset never jumps — and the two longest are read
//! a little later by a slow sine (**modulated diffusion**), so even the diffusion's own
//! pattern never repeats.

use super::parts::{phase_inc, Allpass, SineTable};
use crate::math;
use crate::rest::ZeroRun;
use crate::spatium::fdn::glide;
use crate::spatium::model::hash01;

pub const MAX_PREDELAY_MS: f32 = 1_000.0;
pub const MAX_BLOOM_MS: f32 = 4_000.0;
/// Bloom taps a side.
pub const TAPS: usize = 12;
const DIFFUSER_MS: [[f32; 4]; 2] = [[2.3, 3.9, 5.7, 8.3], [2.5, 3.7, 6.1, 7.9]];
/// The diffusers' modulation at full depth, ms.
const DIFF_MOD_MS: f32 = 0.6;
/// The onset the diffusers add, ms (the left side; the right is 20.2 too).
pub const DIFFUSER_TOTAL_MS: f32 = 20.2;

/// The input stage.
#[derive(Clone, Debug)]
pub struct InputStage {
    sr: f32,
    buf: [Vec<f32>; 2],
    mask: usize,
    w: usize,
    zeros: [ZeroRun; 2],
    in_gain: f32,
    in_gain_target: f32,
    predelay: f32,
    predelay_target: f32,
    bloom_t: f32,
    bloom_t_target: f32,
    bloom: f32,
    bloom_target: f32,
    /// The taps: where (0…1 of the bloom time) and how loud.
    tap_u: [[f32; TAPS]; 2],
    tap_g: [[f32; TAPS]; 2],
    diff: [[Allpass; 4]; 2],
    diff_g: f32,
    diff_target: f32,
    mod_depth: f32,
    mod_depth_target: f32,
    lfo: [u32; 2],
    lfo_inc: [u32; 2],
    k_gain: f32,
    k_time: f32,
    table: SineTable,
}

impl InputStage {
    pub fn new(sample_rate: f32) -> Self {
        let fs = sample_rate;
        let need = (fs * (MAX_PREDELAY_MS + MAX_BLOOM_MS + 20.0) * 0.001) as usize + 8;
        let size = need.next_power_of_two();
        let room = (DIFF_MOD_MS * 2.0 * 0.001 * fs) as usize + 2;
        let mk = |side: usize, k: usize| {
            let len = (DIFFUSER_MS[side][k] * 0.001 * fs) as usize;
            Allpass::new(len, if k >= 2 { room } else { 0 })
        };
        let mut tap_u = [[0.0f32; TAPS]; 2];
        let mut tap_g = [[0.0f32; TAPS]; 2];
        for side in 0..2 {
            let mut e = 0.0f32;
            for k in 0..TAPS {
                let base = math::powf((k + 1) as f32 / TAPS as f32, 0.6);
                let jitter = 1.0 + 0.08 * (hash01(k as u32 + 40 * side as u32, 601) - 0.5);
                let u = (base * jitter).min(1.0);
                let sign = if hash01(k as u32 + 40 * side as u32, 607) < 0.5 {
                    -1.0
                } else {
                    1.0
                };
                tap_u[side][k] = u;
                tap_g[side][k] = sign * math::powf(u, 1.2);
                e += tap_g[side][k] * tap_g[side][k];
            }
            let k = 1.0 / e.sqrt();
            for g in &mut tap_g[side] {
                *g *= k;
            }
        }
        InputStage {
            sr: fs,
            buf: [vec![0.0; size], vec![0.0; size]],
            mask: size - 1,
            w: 0,
            zeros: [ZeroRun::FULL; 2],
            in_gain: 1.0,
            in_gain_target: 1.0,
            predelay: 0.0,
            predelay_target: 0.0,
            bloom_t: 0.0,
            bloom_t_target: 0.0,
            bloom: 0.0,
            bloom_target: 0.0,
            tap_u,
            tap_g,
            diff: [
                [mk(0, 0), mk(0, 1), mk(0, 2), mk(0, 3)],
                [mk(1, 0), mk(1, 1), mk(1, 2), mk(1, 3)],
            ],
            diff_g: 0.0,
            diff_target: 0.0,
            mod_depth: 0.0,
            mod_depth_target: 0.0,
            lfo: [0, 0x4000_0000],
            lfo_inc: [phase_inc(0.71, fs), phase_inc(0.93, fs)],
            k_gain: 1.0 - math::exp(-1.0 / (0.02 * fs)),
            k_time: 1.0 - math::exp(-1.0 / (0.08 * fs)),
            table: SineTable::new(),
        }
    }

    /// The pre-delay (ms), the bloom time (ms; 0 is off), the diffusers' gain (0…0.75),
    /// their modulation (0…1), and the freeze gate.
    pub fn set(&mut self, predelay_ms: f32, bloom_ms: f32, diff: f32, depth: f32, gate: bool) {
        let ms = self.sr * 0.001;
        self.predelay_target = predelay_ms.clamp(0.0, MAX_PREDELAY_MS) * ms;
        let b = bloom_ms.clamp(0.0, MAX_BLOOM_MS);
        if b > 0.0 {
            self.bloom_t_target = b * ms;
            self.bloom_target = 1.0;
        } else {
            self.bloom_target = 0.0;
        }
        self.diff_target = diff.clamp(0.0, 0.75);
        self.mod_depth_target = depth.clamp(0.0, 1.0) * DIFF_MOD_MS * ms;
        self.in_gain_target = if gate { 0.0 } else { 1.0 };
    }

    /// The first set: everything lands at once.
    pub fn snap(&mut self) {
        self.in_gain = self.in_gain_target;
        self.predelay = self.predelay_target;
        self.bloom_t = self.bloom_t_target;
        self.bloom = self.bloom_target;
        self.diff_g = self.diff_target;
        self.mod_depth = self.mod_depth_target;
    }

    /// How far back the stage reads now, samples.
    pub fn span(&self) -> usize {
        let pd = self.predelay.max(self.predelay_target);
        let bl = if self.bloom > 0.0 || self.bloom_target > 0.0 {
            self.bloom_t.max(self.bloom_t_target)
        } else {
            0.0
        };
        (pd + bl) as usize + 4
    }

    /// The bloom's share now, 0…1.
    pub fn bloom(&self) -> f32 {
        self.bloom
    }

    pub fn predelay_samples(&self) -> f32 {
        self.predelay
    }

    #[inline]
    fn read(&self, side: usize, d: f32) -> f32 {
        let b = &self.buf[side];
        let nf = d.floor();
        let f = d - nf;
        let n = nf as usize;
        let a = b[self.w.wrapping_sub(n) & self.mask];
        if f == 0.0 {
            a
        } else {
            let c = b[self.w.wrapping_sub(n + 1) & self.mask];
            a + (c - a) * f
        }
    }
}

impl InputStage {
    /// One sample in: the late field's input out.
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32, drain: bool) -> (f32, f32) {
        let (kg, kt) = (self.k_gain, self.k_time);
        glide(&mut self.in_gain, self.in_gain_target, kg, 1e-6);
        let g = self.in_gain;
        let w = self.w;
        // `+ 0.0`: a −0.0 in is written as +0.0 (ADR 0031 §5.1).
        self.buf[0][w] = l * g + 0.0;
        self.buf[1][w] = r * g + 0.0;
        glide(&mut self.predelay, self.predelay_target, kt, 1e-3);
        glide(&mut self.bloom, self.bloom_target, kg * 0.25, 1e-6);
        glide(&mut self.bloom_t, self.bloom_t_target, kt, 1e-3);
        glide(&mut self.diff_g, self.diff_target, kg, 1e-6);
        glide(&mut self.mod_depth, self.mod_depth_target, kg, 1e-4);
        let pd = self.predelay;
        let mut x = [self.read(0, pd), self.read(1, pd)];
        let b = self.bloom;
        if b > 0.0 {
            let (dry_k, bloom_k) = ((1.0 - b).sqrt(), b.sqrt());
            for (side, xs) in x.iter_mut().enumerate() {
                let mut acc = 0.0f32;
                for k in 0..TAPS {
                    let d = pd + self.bloom_t * self.tap_u[side][k];
                    acc += self.tap_g[side][k] * self.read(side, d);
                }
                *xs = *xs * dry_k + acc * bloom_k;
            }
        }
        let dg = self.diff_g;
        for side in 0..2 {
            let ph = self.lfo[side];
            self.lfo[side] = ph.wrapping_add(self.lfo_inc[side]);
            let m = if self.mod_depth > 0.0 {
                self.mod_depth * (1.0 + self.table.sin(ph))
            } else {
                0.0
            };
            let mut v = x[side];
            for (k, a) in self.diff[side].iter_mut().enumerate() {
                v = a.tick(v, dg, if k >= 2 { m } else { 0.0 }, drain);
            }
            x[side] = v;
        }
        self.w = (w + 1) & self.mask;
        (x[0], x[1])
    }

    /// After a block of `n` ticks: the rings' runs of zeros.
    pub fn wrote(&mut self, n: usize) {
        for c in 0..2 {
            self.zeros[c].wrote(&self.buf[c], self.w, n);
        }
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        let span = self.span();
        if !self.zeros.iter().all(|z| z.covers(span)) {
            return Some("pre-delay");
        }
        if !self.diff.iter().flatten().all(|a| a.is_zero()) {
            return Some("diffusers");
        }
        if self.in_gain != self.in_gain_target
            || self.predelay != self.predelay_target
            || self.bloom != self.bloom_target
            || (self.bloom_target > 0.0 && self.bloom_t != self.bloom_t_target)
            || self.diff_g != self.diff_target
            || self.mod_depth != self.mod_depth_target
        {
            return Some("input glides");
        }
        None
    }

    /// `n` samples of silence not run, at rest.
    pub fn skip(&mut self, n: usize) {
        self.w = (self.w + n) & self.mask;
        for a in self.diff.iter_mut().flatten() {
            a.skip(n);
        }
        for side in 0..2 {
            self.lfo[side] = self.lfo[side].wrapping_add(self.lfo_inc[side].wrapping_mul(n as u32));
        }
    }

    /// As new: cleared, the counts back to their start, every glide landed.
    pub fn reset(&mut self) {
        for b in &mut self.buf {
            b.iter_mut().for_each(|x| *x = 0.0);
        }
        self.zeros = [ZeroRun::FULL; 2];
        self.w = 0;
        for a in self.diff.iter_mut().flatten() {
            a.clear();
            a.rewind();
        }
        self.lfo = [0, 0x4000_0000];
        self.snap();
    }
}
