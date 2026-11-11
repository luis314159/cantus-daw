//! Aula, the convolution reverb (ADR 0112): a room recorded (or made) as an impulse
//! response, heard through a zero-latency partitioned convolver ([`crate::aula_conv`]).
//!
//! The signal path: the input, delayed by the pre-delay, into the convolver; the wet's
//! width (mid/side); then the mix with the dry and the output level. What changes the
//! response itself — stretch, length, decay, the gate, damping, the cuts, reverse, attack —
//! is applied to the response off the audio thread ([`crate::aula_shape`]) and the result
//! swapped in whole.
//!
//! **Nulls**: at mix 0 (the default) and level 0 dB the dry passes untouched, to the bit
//! (the convolver still listens, so raising the mix later brings in the tail of what was
//! played). Width at 100 % touches nothing. Every glide lands exactly on its target.

use crate::aula_conv::{Convolver, State, Supplied};
use crate::aula_kernel::{Kernel, Layout, LEVELS, SIZES, STARTS};
use crate::aula_shape::{shape, Ir, Shape};
use crate::math;
use std::sync::Arc;

/// The longest pre-delay, ms.
pub const MAX_PREDELAY_MS: f32 = 500.0;
/// Samples a glide takes (mix, level, width) and a pre-delay change's crossfade.
const RAMP: u32 = 512;

/// Telemetry slots (`[f32; 64]`).
pub const T_IN_DB: usize = 0;
pub const T_OUT_DB: usize = 1;
/// The response running, seconds.
pub const T_SECONDS: usize = 2;
/// Its paths: 1 mono, 2 stereo, 4 true stereo.
pub const T_PATHS: usize = 3;
/// 1 while a new response is being swapped in.
pub const T_SWAP: usize = 4;

/// The controls the audio thread applies itself.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AulaSettings {
    pub predelay_ms: f32,
    /// 0 … 2 (1 as the response has it, 0 mono).
    pub width: f32,
    /// 0 … 1: the dry and the wet crossfaded.
    pub mix: f32,
    /// The output level, dB.
    pub level_db: f32,
}

impl Default for AulaSettings {
    fn default() -> Self {
        AulaSettings {
            predelay_ms: 0.0,
            width: 1.0,
            mix: 0.0,
            level_db: 0.0,
        }
    }
}

/// The room a response of `taps` gets: up to level 2 whole (1.4 s at 48 kHz, so short
/// responses and their stretches share one), then the largest level's partitions in eights.
/// Stretching within that room never needs a new state.
pub fn capacity_for(taps: usize) -> Layout {
    let top = STARTS[LEVELS - 1];
    let round = if taps <= top {
        top
    } else {
        let eight = 8 * SIZES[LEVELS - 1];
        top + (taps - top).div_ceil(eight) * eight
    };
    Layout::for_taps(round)
}

/// A response shaped and partitioned for `cap` (or for a room of its own when it does not
/// fit `cap`: then the returned layout is that room, and a new [`State`] is needed).
/// Allocates: off the audio thread.
pub fn prepare(src: &Ir, rate: f32, s: &Shape, cap: Option<Layout>) -> (Ir, Kernel) {
    let ir = shape(src, rate, s, true);
    let own = Layout::for_taps(ir.len());
    let cap = match cap {
        Some(c) if c.holds(&own) => c,
        _ => capacity_for(ir.len()),
    };
    let kernel = Kernel::new(ir.format, &ir.channels, cap);
    (ir, kernel)
}

/// A linear glide that lands exactly.
#[derive(Copy, Clone, Debug)]
struct Glide {
    value: f32,
    target: f32,
    step: f32,
    left: u32,
}

impl Glide {
    fn at(v: f32) -> Glide {
        Glide {
            value: v,
            target: v,
            step: 0.0,
            left: 0,
        }
    }
    fn set(&mut self, t: f32) {
        if t != self.target {
            self.target = t;
            self.step = (t - self.value) / RAMP as f32;
            self.left = RAMP;
        }
    }
    #[inline]
    fn next(&mut self) -> f32 {
        if self.left > 0 {
            self.left -= 1;
            self.value = if self.left == 0 {
                self.target
            } else {
                self.value + self.step
            };
        }
        self.value
    }
    fn settled(&self) -> bool {
        self.left == 0
    }
    fn jump(&mut self) {
        self.value = self.target;
        self.left = 0;
    }
}

#[derive(Clone, Debug)]
pub struct Aula {
    rate: f32,
    settings: AulaSettings,
    conv: Convolver,
    /// The pre-delay's rings and where the next sample goes.
    pre_l: Vec<f32>,
    pre_r: Vec<f32>,
    pre_mask: usize,
    pre_pos: usize,
    /// The delay heard, the one faded to, how far into the fade, and one waiting.
    delay: usize,
    fade_to: usize,
    fade: u32,
    pending: Option<usize>,
    mix: Glide,
    gain: Glide,
    width: Glide,
    /// Silent input in a row, samples.
    quiet: u64,
    peak_in: f32,
    peak_out: f32,
}

impl Aula {
    /// Allocates (the pre-delay's rings): off the audio thread, as `conv` was made.
    pub fn new(rate: f32, conv: Convolver) -> Aula {
        let max = (MAX_PREDELAY_MS / 1000.0 * rate) as usize + 1;
        let ring = max.next_power_of_two();
        let d = AulaSettings::default();
        Aula {
            rate,
            settings: d,
            conv,
            pre_l: vec![0.0; ring],
            pre_r: vec![0.0; ring],
            pre_mask: ring - 1,
            pre_pos: 0,
            delay: 0,
            fade_to: 0,
            fade: 0,
            pending: None,
            mix: Glide::at(d.mix),
            gain: Glide::at(1.0),
            width: Glide::at(d.width),
            quiet: u64::MAX / 2,
            peak_in: 0.0,
            peak_out: 0.0,
        }
    }

    /// An Aula with a fresh state and `kernel` (allocates).
    pub fn with_kernel(rate: f32, kernel: Arc<Kernel>) -> Aula {
        let state = Box::new(State::new(kernel.cap()));
        Aula::new(rate, Convolver::new(state, kernel))
    }

    pub fn settings(&self) -> &AulaSettings {
        &self.settings
    }

    pub fn convolver(&self) -> &Convolver {
        &self.conv
    }

    /// New settings; the glides move on from where they are. No allocation.
    pub fn set(&mut self, s: &AulaSettings) {
        self.settings = *s;
        self.mix.set(s.mix.clamp(0.0, 1.0));
        self.gain.set(math::db_to_gain(s.level_db));
        self.width.set(s.width.clamp(0.0, 2.0));
        let d = ((s.predelay_ms.clamp(0.0, MAX_PREDELAY_MS) / 1000.0 * self.rate).round() as usize)
            .min(self.pre_mask);
        if self.fade > 0 {
            self.pending = (d != self.fade_to).then_some(d);
        } else if d != self.delay {
            self.fade_to = d;
            self.fade = RAMP;
        }
    }

    /// A new response (see [`Convolver::supply`]).
    pub fn supply(&mut self, kernel: Arc<Kernel>, state: Option<Box<State>>) -> Supplied {
        self.conv.supply(kernel, state)
    }

    pub fn reset(&mut self) {
        self.conv.reset();
        if self.quiet < self.pre_l.len() as u64 {
            self.pre_l.fill(0.0);
            self.pre_r.fill(0.0);
        }
        self.pre_pos = 0;
        if let Some(p) = self.pending.take() {
            self.fade_to = p;
        }
        if self.fade > 0 {
            self.delay = self.fade_to;
            self.fade = 0;
        }
        self.mix.jump();
        self.gain.jump();
        self.width.jump();
        self.quiet = u64::MAX / 2;
        self.peak_in = 0.0;
        self.peak_out = 0.0;
    }

    pub fn latency(&self) -> usize {
        0
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if self.quiet < self.pre_l.len() as u64 {
            return Some("the pre-delay still holds sound");
        }
        if self.fade > 0 || self.pending.is_some() {
            return Some("the pre-delay is moving");
        }
        if !(self.mix.settled() && self.gain.settled() && self.width.settled()) {
            return Some("a glide is still moving");
        }
        self.conv.rest_blocker()
    }

    pub fn skip(&mut self, frames: usize) {
        self.pre_pos = (self.pre_pos + frames) & self.pre_mask;
        self.conv.skip(frames);
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        const CHUNK: usize = 256;
        let n = l.len().min(r.len());
        match (0..n).rev().find(|&i| l[i] != 0.0 || r[i] != 0.0) {
            Some(i) => self.quiet = (n - 1 - i) as u64,
            None => self.quiet = self.quiet.saturating_add(n as u64),
        }
        self.peak_in = peak(&l[..n]).max(peak(&r[..n]));
        let mut pk_out = 0.0f32;
        let mut at = 0;
        while at < n {
            let m = (n - at).min(CHUNK);
            let (dl, dr) = (&mut l[at..at + m], &mut r[at..at + m]);
            let mut wl = [0.0f32; CHUNK];
            let mut wr = [0.0f32; CHUNK];
            self.predelay(dl, dr, &mut wl[..m], &mut wr[..m]);
            self.conv.process(&mut wl[..m], &mut wr[..m]);
            let untouched = self.mix.settled()
                && self.mix.value == 0.0
                && self.gain.settled()
                && self.gain.value == 1.0;
            if !untouched {
                let still_width = self.width.settled() && self.width.value == 1.0;
                for i in 0..m {
                    let (mut yl, mut yr) = (wl[i], wr[i]);
                    let w = self.width.next();
                    if !still_width {
                        let mid = 0.5 * (yl + yr);
                        let side = 0.5 * (yl - yr) * w;
                        yl = mid + side;
                        yr = mid - side;
                    }
                    let mix = self.mix.next();
                    let g = self.gain.next();
                    dl[i] = (dl[i] * (1.0 - mix) + yl * mix) * g;
                    dr[i] = (dr[i] * (1.0 - mix) + yr * mix) * g;
                }
            }
            pk_out = pk_out.max(peak(dl)).max(peak(dr));
            at += m;
        }
        self.peak_out = pk_out;
    }

    /// The pre-delay: `x` written into the rings, the delayed signal into `w`.
    fn predelay(&mut self, xl: &[f32], xr: &[f32], wl: &mut [f32], wr: &mut [f32]) {
        let mask = self.pre_mask;
        for i in 0..xl.len() {
            let p = self.pre_pos;
            self.pre_l[p] = xl[i];
            self.pre_r[p] = xr[i];
            let read = |d: usize, ring: &[f32]| ring[p.wrapping_sub(d) & mask];
            if self.fade > 0 {
                let a = 1.0 - self.fade as f32 / RAMP as f32;
                let (d0, d1) = (self.delay, self.fade_to);
                wl[i] = read(d0, &self.pre_l) * (1.0 - a) + read(d1, &self.pre_l) * a;
                wr[i] = read(d0, &self.pre_r) * (1.0 - a) + read(d1, &self.pre_r) * a;
                self.fade -= 1;
                if self.fade == 0 {
                    self.delay = self.fade_to;
                    if let Some(d) = self.pending.take() {
                        if d != self.delay {
                            self.fade_to = d;
                            self.fade = RAMP;
                        }
                    }
                }
            } else if self.delay == 0 {
                wl[i] = xl[i];
                wr[i] = xr[i];
            } else {
                wl[i] = read(self.delay, &self.pre_l);
                wr[i] = read(self.delay, &self.pre_r);
            }
            self.pre_pos = (p + 1) & mask;
        }
    }

    pub fn telemetry(&self, out: &mut [f32; 64]) {
        let db = |v: f32| {
            if v > 1e-9 {
                20.0 * math::log10(v)
            } else {
                -120.0
            }
        };
        let k = self.conv.kernel();
        out[T_IN_DB] = db(self.peak_in);
        out[T_OUT_DB] = db(self.peak_out);
        out[T_SECONDS] = k.taps() as f32 / self.rate.max(1.0);
        out[T_PATHS] = k.format().paths() as f32;
        out[T_SWAP] = if self.conv.swapping() { 1.0 } else { 0.0 };
    }
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

#[cfg(test)]
#[path = "aula_tests.rs"]
mod tests;
