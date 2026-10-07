//! Grana — Unda's granular oscillator engine (`research/instruments/fm-additive-
//! granular.md` §4.9, §8.2). Windowed grains from a source — an imported sample, or the
//! oscillator's own table played at the note's pitch — scheduled on the sample clock from
//! the note's start, with every random draw a hash of (note seed, grain number, purpose):
//! the same notes give the same grains at every block size, live or bounced (no product
//! the research found documents that).
//!
//! A fixed pool of [`GRAINS`] per voice and oscillator; when it is full a spawn is
//! skipped (deterministic, click-free) rather than a grain stolen.

use super::modsrc::unit;
use super::table::{hermite, Read, Table, LEVELS, LEVEL_TOP};
use crate::math;
use std::sync::OnceLock;

pub const GRAINS: usize = 32;
const WINDOW_LEN: usize = 256;

/// Grain windows, in `osc{n}.gwin` order.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Window {
    #[default]
    Hann,
    Gauss,
    Tukey,
    /// A fast attack and an exponential decay.
    Expodec,
}

impl Window {
    pub fn from_index(i: f32) -> Window {
        match i.round() as i32 {
            1 => Window::Gauss,
            2 => Window::Tukey,
            3 => Window::Expodec,
            _ => Window::Hann,
        }
    }
}

/// The windows as tables (one extra point, so a read at 1 needs no wrap), made once.
#[allow(clippy::needless_range_loop)] // four tables filled from one index
pub fn windows() -> &'static [[f32; WINDOW_LEN + 1]; 4] {
    static W: OnceLock<[[f32; WINDOW_LEN + 1]; 4]> = OnceLock::new();
    W.get_or_init(|| {
        let mut w = [[0.0; WINDOW_LEN + 1]; 4];
        for i in 0..=WINDOW_LEN {
            let x = i as f32 / WINDOW_LEN as f32;
            let hann = 0.5 - 0.5 * math::cos(std::f32::consts::TAU * x);
            let d = (x - 0.5) / 0.18;
            let gauss = math::exp(-0.5 * d * d) * if i == 0 || i == WINDOW_LEN { 0.0 } else { 1.0 };
            let edge = 0.25;
            let tukey = if x < edge {
                0.5 - 0.5 * math::cos(std::f32::consts::PI * x / edge)
            } else if x > 1.0 - edge {
                0.5 - 0.5 * math::cos(std::f32::consts::PI * (1.0 - x) / edge)
            } else {
                1.0
            };
            let expodec = (1.0 - math::exp(-x * 60.0)) * math::exp(-5.0 * x) * (1.0 - x);
            w[0][i] = hann;
            w[1][i] = gauss;
            w[2][i] = tukey;
            w[3][i] = expodec;
        }
        w
    })
}

/// What grains read.
#[derive(Copy, Clone)]
pub enum Source<'a> {
    /// A sample: channel-interleaved frames, its channel count and rate.
    Sample {
        frames: &'a [f32],
        channels: usize,
        rate: f32,
    },
    /// The oscillator's table, played at the note's pitch.
    Table(&'a Table),
}

/// One control tick's settings for a Grana oscillator (already modulated).
#[derive(Copy, Clone, Debug, Default)]
pub struct GranaControl {
    /// The note's frequency (Hz); a sample's root is C4 (261.6 Hz).
    pub hz: f32,
    pub gain: f32,
    pub pan: f32,
    /// Where in the source, 0…1.
    pub pos: f32,
    /// Grains per second.
    pub density: f32,
    /// Grain length, s.
    pub length: f32,
    /// Position jitter, s (in the table: a fraction of it, ×10 per second).
    pub spray: f32,
    /// Pitch jitter, cents either way.
    pub pitch_rand: f32,
    /// Probability a grain plays backwards.
    pub reverse: f32,
    /// Pan spread of the grains, 0…1.
    pub width: f32,
    pub window: Window,
}

#[derive(Copy, Clone, Debug, Default)]
struct Grain {
    on: bool,
    /// Where it reads: source samples (sample), or cycles (table, the fraction is phase).
    pos: f64,
    inc: f64,
    frame: usize,
    level: usize,
    w: f32,
    dw: f32,
    gl: f32,
    gr: f32,
}

#[derive(Clone, Debug)]
pub struct Grana {
    grains: [Grain; GRAINS],
    /// Samples until the next grain starts.
    next: f64,
    count: u64,
    seed: u64,
    c: GranaControl,
    /// The previous output (what another oscillator's FM reads).
    pub out0: f32,
}

impl Default for Grana {
    fn default() -> Self {
        Grana {
            grains: [Grain::default(); GRAINS],
            next: 0.0,
            count: 0,
            seed: 0,
            c: GranaControl::default(),
            out0: 0.0,
        }
    }
}

impl Grana {
    /// A note starts: the first grain now.
    pub fn start(&mut self, seed: u64) {
        self.grains = [Grain::default(); GRAINS];
        self.next = 0.0;
        self.count = 0;
        self.seed = seed;
        self.out0 = 0.0;
    }

    pub fn control(&mut self, c: &GranaControl) {
        self.c = *c;
    }

    /// Grains sounding now.
    pub fn running(&self) -> usize {
        self.grains.iter().filter(|g| g.on).count()
    }

    fn spawn(&mut self, src: &Source, sr: f32) {
        let n = self.count;
        self.count += 1;
        let r = |purpose: u64| unit(&[self.seed, 0x6A, n, purpose]);
        let c = self.c;
        let Some(slot) = self.grains.iter().position(|g| !g.on) else {
            return; // full: skip, deterministically
        };
        let cents = c.pitch_rand * (2.0 * r(1) - 1.0);
        let bend = math::exp2(cents / 1200.0) as f64;
        let reverse = r(2) < c.reverse;
        let len = (c.length * sr).max(8.0);
        let pan = (c.pan + c.width * (2.0 * r(3) - 1.0)).clamp(-1.0, 1.0);
        let th = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
        let overlap = (c.density * c.length).max(1.0);
        let g = c.gain / overlap.sqrt();
        let mut grain = Grain {
            on: true,
            w: 0.0,
            dw: 1.0 / len,
            gl: g * math::cos(th) * std::f32::consts::SQRT_2,
            gr: g * math::sin(th) * std::f32::consts::SQRT_2,
            ..Grain::default()
        };
        match src {
            Source::Sample {
                frames,
                channels,
                rate,
            } => {
                let total = frames.len() / channels.max(&1);
                let ratio = (*rate / sr) as f64 * (c.hz / 261.63) as f64 * bend;
                let spray = c.spray * rate * (2.0 * r(4) - 1.0);
                let start = (c.pos * total as f32 + spray)
                    .clamp(0.0, total.saturating_sub(1) as f32) as f64;
                grain.inc = if reverse { -ratio } else { ratio };
                grain.pos = start;
            }
            Source::Table(t) => {
                let frames = t.frames().max(1);
                let spray = c.spray * 10.0 * (2.0 * r(4) - 1.0);
                let fp = ((c.pos + spray) * (frames - 1) as f32).clamp(0.0, (frames - 1) as f32);
                grain.frame = fp.round() as usize;
                let hz = c.hz * bend as f32;
                grain.inc = (hz / sr) as f64 * if reverse { -1.0 } else { 1.0 };
                grain.pos = r(5) as f64;
                let x = math::log2((hz * LEVEL_TOP[0] as f32 / (0.5 * sr)).max(1e-9));
                grain.level = (x.ceil().max(0.0) as usize).min(LEVELS - 1);
            }
        }
        self.grains[slot] = grain;
    }

    /// One sample at `sr`: (left, right).
    #[inline]
    pub fn tick(&mut self, src: &Source, sr: f32) -> (f32, f32) {
        self.next -= 1.0;
        if self.next <= 0.0 {
            self.spawn(src, sr);
            let jitter = 0.75 + 0.5 * unit(&[self.seed, 0x7B, self.count]);
            self.next += (sr / self.c.density.max(0.1)) as f64 * jitter as f64;
        }
        let win = &windows()[self.c.window as usize];
        let (mut l, mut r) = (0.0, 0.0);
        for g in self.grains.iter_mut().filter(|g| g.on) {
            let x = g.w * WINDOW_LEN as f32;
            let i = (x as usize).min(WINDOW_LEN - 1);
            let w = win[i] + (win[i + 1] - win[i]) * (x - i as f32);
            let y = match src {
                Source::Sample {
                    frames, channels, ..
                } => {
                    let ch = (*channels).max(1);
                    let total = frames.len() / ch;
                    let at = |k: isize| frames[(k.clamp(0, total as isize - 1) as usize) * ch];
                    let p = g.pos.floor();
                    let t = (g.pos - p) as f32;
                    let k = p as isize;
                    hermite(at(k - 1), at(k), at(k + 1), at(k + 2), t)
                }
                Source::Table(t) => {
                    let phi = g.pos.rem_euclid(1.0) as f32;
                    t.apply(t.base(g.level, g.frame), &Read::new(g.level, phi))
                }
            };
            let v = y * w;
            l += v * g.gl;
            r += v * g.gr;
            g.pos += g.inc;
            g.w += g.dw;
            if g.w >= 1.0 {
                g.on = false;
            }
        }
        self.out0 = l + r;
        (l, r)
    }
}
