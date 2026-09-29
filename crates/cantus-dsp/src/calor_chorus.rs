//! Calor's output stage: a bucket-brigade-style stereo chorus (I, II, I+II), a DC block,
//! the output gain and a soft knee that keeps the instrument under 0 dBFS.
//!
//! The chorus is one delay line read by two taps swept in opposite directions by a
//! triangle — the classic two-phase ensemble — with a darker wet path (a bucket brigade's
//! limited bandwidth). Its sweep runs on the stream's clock, so it is the same at every
//! block size.

use crate::va::math::flush;
use crate::va::{math, OnePole};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ChorusMode {
    #[default]
    Off,
    One,
    Two,
    Both,
}

impl ChorusMode {
    pub fn from_index(i: f32) -> ChorusMode {
        match i.round() as i32 {
            1 => ChorusMode::One,
            2 => ChorusMode::Two,
            3 => ChorusMode::Both,
            _ => ChorusMode::Off,
        }
    }

    /// (rate Hz, centre ms, depth ms).
    fn sweep(self) -> (f32, f32, f32) {
        match self {
            ChorusMode::Off => (0.0, 0.0, 0.0),
            ChorusMode::One => (0.52, 3.3, 1.5),
            ChorusMode::Two => (0.86, 3.3, 1.9),
            ChorusMode::Both => (8.0, 3.3, 0.22),
        }
    }
}

/// Longest delay the chorus reads, ms.
const MAX_MS: f32 = 8.0;

#[derive(Clone, Debug)]
pub struct Chorus {
    buf: Vec<f32>,
    mask: usize,
    pos: usize,
    wet_lp: [OnePole; 2],
    g_lp: f32,
    fs: f32,
    was_on: bool,
}

impl Chorus {
    /// Allocates its line: build it off the audio thread.
    pub fn new(sample_rate: f32) -> Self {
        let need = (MAX_MS * 0.001 * sample_rate) as usize + 4;
        let len = need.next_power_of_two();
        Chorus {
            buf: vec![0.0; len],
            mask: len - 1,
            pos: 0,
            wet_lp: [OnePole::default(); 2],
            g_lp: OnePole::coef(math::tan_pi((7000.0 / sample_rate).min(0.45))),
            fs: sample_rate,
            was_on: false,
        }
    }

    pub fn reset(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.pos = 0;
        self.wet_lp = [OnePole::default(); 2];
    }

    /// Whether its line and filters hold nothing (so its output is exactly its input).
    pub fn is_silent(&self) -> bool {
        !self.was_on
            || (self.wet_lp.iter().all(OnePole::is_silent) && self.buf.iter().all(|x| *x == 0.0))
    }

    #[inline]
    fn read(&self, delay: f32) -> f32 {
        let d = delay.max(1.0);
        let i = d as usize;
        let f = d - i as f32;
        let a = self.buf[(self.pos.wrapping_sub(i)) & self.mask];
        let b = self.buf[(self.pos.wrapping_sub(i + 1)) & self.mask];
        a + (b - a) * f
    }

    /// Process in place; `clock` is the stream position (base-rate samples) of the first
    /// frame.
    pub fn process(
        &mut self,
        mode: ChorusMode,
        mix: f32,
        l: &mut [f32],
        r: &mut [f32],
        clock: u64,
    ) {
        if mode == ChorusMode::Off {
            self.was_on = false;
            return;
        }
        if !self.was_on {
            self.reset();
            self.was_on = true;
        }
        let (rate, centre, depth) = mode.sweep();
        let ms = self.fs * 0.001;
        let mix = mix.clamp(0.0, 1.0);
        for (i, (xl, xr)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
            let t = (clock + i as u64) as f64 * rate as f64 / self.fs as f64;
            let p = (t - t as u64 as f64) as f32;
            let tri = if p < 0.5 {
                4.0 * p - 1.0
            } else {
                3.0 - 4.0 * p
            };
            self.pos = (self.pos + 1) & self.mask;
            self.buf[self.pos] = 0.5 * (*xl + *xr);
            let wl = self.read((centre + depth * tri) * ms);
            let wr = self.read((centre - depth * tri) * ms);
            let wl = self.wet_lp[0].lp_c(wl, self.g_lp);
            let wr = self.wet_lp[1].lp_c(wr, self.g_lp);
            *xl = flush(*xl + (wl - *xl) * mix);
            *xr = flush(*xr + (wr - *xr) * mix);
        }
    }
}

/// The instance's last stage: DC block, gain, and a knee that is the identity up to
/// −6 dBFS and bends every sample above it towards (never to) 0 dBFS.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OutputStage {
    dc: [OnePole; 2],
    g_dc: f32,
}

/// Where the knee starts.
pub const KNEE: f32 = 0.5;

impl OutputStage {
    pub fn new(sample_rate: f32) -> Self {
        OutputStage {
            dc: [OnePole::default(); 2],
            g_dc: OnePole::coef(math::tan_pi(5.0 / sample_rate)),
        }
    }

    pub fn reset(&mut self) {
        self.dc = [OnePole::default(); 2];
    }

    pub fn is_silent(&self) -> bool {
        self.dc.iter().all(OnePole::is_silent)
    }

    /// The knee alone.
    #[inline]
    pub fn knee(x: f32) -> f32 {
        let a = x.abs();
        if a <= KNEE {
            return x;
        }
        let d = (a - KNEE) / (1.0 - KNEE);
        let y = (KNEE + (1.0 - KNEE) * d / (1.0 + d)).min(0.999_99);
        if x > 0.0 {
            y
        } else {
            -y
        }
    }

    #[inline]
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], gain: f32) {
        for (xl, xr) in l.iter_mut().zip(r.iter_mut()) {
            *xl = Self::knee(self.dc[0].hp_c(*xl, self.g_dc) * gain);
            *xr = Self::knee(self.dc[1].hp_c(*xr, self.g_dc) * gain);
        }
    }
}
