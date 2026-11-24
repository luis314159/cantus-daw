//! An Arcus voice at `os ×` the rate (round 42, `arcus-upper`; a child of `arcus::voice`):
//! the violin's and the viola's voices run their string, bow, pluck and pitch loop at a
//! multiple of the rate and come back through a half-band cascade (`upper::Decimator`).
//! At `os = 1` (the cello, the double bass) nothing here runs: [`Voice::render`] is the
//! cello's own render, the same instructions on the same numbers.

use super::super::upper::{factor_with, Decimator, MAX_OS};
use super::*;

/// The scratch a voice renders into at `os ×` the rate, and its way back.
#[derive(Clone, Debug)]
pub struct Rate {
    /// The factor the voice runs at now.
    pub os: usize,
    /// The rate the instance runs at (the voice's own is `os ×` it).
    base_sr: f32,
    dec: Decimator,
    hi_l: Vec<f32>,
    hi_r: Vec<f32>,
    /// Base-rate samples the way back still rings after the voice ended.
    pub drain: usize,
}

impl Rate {
    /// Allocates: never on the audio thread.
    pub fn new(sample_rate: f32) -> Self {
        Rate {
            os: 1,
            base_sr: sample_rate,
            dec: Decimator::new(),
            hi_l: vec![0.0; MAX_OS * super::super::CHUNK],
            hi_r: vec![0.0; MAX_OS * super::super::CHUNK],
            drain: 0,
        }
    }

    pub fn reset(&mut self) {
        self.dec.reset();
        self.drain = 0;
    }

    /// The scale on a white noise's level that keeps its density in the audio band the same
    /// at every factor (`√os`; exactly 1 at `os = 1`).
    pub fn noise_scale(&self) -> f32 {
        match self.os {
            1 => 1.0,
            os => (os as f32).sqrt(),
        }
    }
}

impl Voice {
    /// What the voice reads while it plays at its own rate: the instance's, with the rate and
    /// the stream's clock in the voice's samples.
    pub(super) fn at_rate<'a>(&self, sh: &Shared<'a>) -> Shared<'a> {
        let os = self.rate.os;
        Shared {
            s: sh.s,
            sr: sh.sr * os as f32,
            ctl: sh.ctl,
            tuning: sh.tuning,
            hammer: sh.hammer,
            hammer_area: sh.hammer_area,
            table: sh.table,
            clock: sh.clock * os as u64,
        }
    }

    /// A new note's factor (`keep` false: the voice was cleared): its string and its pluck
    /// set to the rate when the factor changes, its way back emptied. No allocation.
    pub(super) fn enter_rate(&mut self, sh: &Shared) {
        let os = factor_with(sh.s.instrument, sh.s.oversample);
        if os != self.rate.os || self.rate.base_sr != sh.sr {
            self.rate.os = os;
            self.rate.base_sr = sh.sr;
            let sr = sh.sr * os as f32;
            self.string.set_rate(sr);
            self.pluck = Exciter::new(sr);
        }
        self.rate.reset();
    }

    /// The factor a note of these settings would play at, and whether a ringing string can
    /// be kept for it (the same factor).
    pub(super) fn same_rate(&self, sh: &Shared) -> bool {
        factor_with(sh.s.instrument, sh.s.oversample) == self.rate.os
    }

    /// Render the voice **added** into `l`/`r` (mono: the same into both), from the
    /// stream's sample `sh.clock` (at the rate). At `os > 1` the string runs a control span
    /// at a time (the grid's `CTRL` samples at the rate, `CTRL · os` at the voice's: its
    /// control steps as often in time as the cello's),
    /// where alone a voice can end; its way back then rings out for [`TAIL`] samples from
    /// that span's end, so what is heard never depends on where the blocks were cut.
    pub fn render(&mut self, sh: &Shared, l: &mut [f32], r: &mut [f32]) {
        if self.rate.os == 1 {
            self.render_core(sh, l, r);
            return;
        }
        let os = self.rate.os;
        let n = l.len().min(r.len());
        let grid = super::super::CTRL as u64;
        let mut hi_l = std::mem::take(&mut self.rate.hi_l);
        let mut hi_r = std::mem::take(&mut self.rate.hi_r);
        let mut done = 0;
        let mut peak = 0f32;
        let mut low = [0.0f32; super::super::CHUNK];
        while done < n && (self.active || self.rate.drain > 0) {
            let at = sh.clock + done as u64;
            let k = if self.active {
                ((grid - at % grid) as usize).min(n - done)
            } else {
                self.rate.drain.min(n - done)
            };
            let (a, b) = (&mut hi_l[..k * os], &mut hi_r[..k * os]);
            a.fill(0.0);
            if self.active {
                b.fill(0.0);
                let base = Shared { clock: at, ..*sh };
                let hi = self.at_rate(&base);
                self.render_core(&hi, a, b);
                if !self.active {
                    self.rate.drain = TAIL;
                }
            } else {
                self.rate.drain -= k;
            }
            low[..k].fill(0.0);
            let p = self.rate.dec.run(os, a, &mut low[..k]);
            peak = peak.max(p);
            for j in 0..k {
                l[done + j] += low[j];
                r[done + j] += low[j];
            }
            done += k;
        }
        self.rate.hi_l = hi_l;
        self.rate.hi_r = hi_r;
        if !self.active && self.rate.drain == 0 {
            self.rate.reset();
        }
        self.peak = peak;
    }
}

/// The base-rate samples the way back takes to ring out (its two stages' memory, with a
/// margin): after them its state is exact zeros.
pub const TAIL: usize = 64;
