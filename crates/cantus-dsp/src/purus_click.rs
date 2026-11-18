//! Purus's de-click and de-crackle: the audio editor's click detector and rebuilding
//! (ADR 0096 §5a, `denoise::declick`), run as a stream.
//!
//! **Shared, not copied.** The model's fit (`ar_fit_into`: Welch-windowed autocorrelation,
//! Levinson–Durbin, order 32), the block's prediction errors and robust scale
//! (`block_errors`: the median absolute error / 0.6745, floored at a thousandth of the
//! block's level), the threshold (`threshold`: 12 to 4 robust scales with the sensitivity),
//! the merging distance (`MERGE`, 24 samples), the widening (`widen`) and the least-squares
//! autoregressive rebuilding (`rebuild`, Vaseghi, Godsill & Rayner) are the editor's own
//! functions, made to work in scratch allocated here once.
//!
//! **The stream.** Each channel keeps its input and its repaired output in rings. Every
//! [`BLOCK`] samples the block that ended [`AHEAD`] samples ago is judged: the model is
//! fitted to it and the block before it, the suspects marked and merged, and each click
//! rebuilt from up to [`BEFORE`] samples before it (already repaired) and [`AFTER`] after
//! (the look-ahead). A sample is final once its block is judged: **the latency,
//! `BLOCK + AHEAD − 1` samples (767, 16 ms at 48 kHz)**, fixed.
//!
//! **What is a click.** Suspects merged into a stretch no longer than [`MAX_GAP`] (2 ms at
//! 48 kHz; a longer one is a drum hit or a consonant, left alone), whose largest error
//! passes the sensitivity's threshold — the editor's judgement.
//!
//! **Crackle** — the dense, small impulses of a worn record — is not rebuilt: it is found in
//! the high band and each impulse *subtracted* (`purus_crackle.rs`), after the clicks, in
//! the same window: no latency of its own.
//!
//! At most [`MAX_REPAIRS`] clicks a block a channel are rebuilt (the CPU is bounded);
//! a block with none is handed on bit for bit.

use super::crackle::Crackle;
use crate::denoise::declick::{self, ArScratch, GapScratch, MERGE, ORDER};

/// The block judged at once, samples.
pub const BLOCK: usize = 512;
/// How far past the block the stream looks before judging it, samples.
pub const AHEAD: usize = 256;
/// The context before a block (the model of a rebuilding), samples.
pub const BEFORE: usize = 1024;
/// The context after a click its rebuilding is fitted to, samples.
pub const AFTER: usize = 192;
/// The longest stretch rebuilt as a click, samples.
pub const MAX_GAP: usize = 96;
/// With the de-crackle on, stretches of at most this many marks are left to it.
pub const IMPULSE_LEN: usize = 4;
/// With the de-crackle on, a click's peak error must be at least this share of the block's
/// level.
pub const CLICK_OVER: f64 = 1.5;
/// With the de-crackle on, the clicks' threshold is raised by this many robust scales.
pub const CLICK_RAISE: f64 = 6.0;
/// The most clicks rebuilt a block a channel.
pub const MAX_REPAIRS: usize = 32;
/// The window judged each block.
const WINDOW: usize = BEFORE + BLOCK + AHEAD;
const RING: usize = 4096;
/// Samples under this count toward the drain (−180 dBFS).
pub const QUIET: f32 = 1e-9;

/// The module's latency, samples (fixed).
pub const LATENCY: usize = BLOCK + AHEAD - 1;

/// The de-click module's settings.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ClickSettings {
    pub on: bool,
    /// 0–1: the clicks found (the editor's sensitivity: 12 robust scales at 0, 4 at 1).
    pub sensitivity: f32,
    /// 0–1: the de-crackle (0 off; at 1 an impulse 5 robust scales above the high band is
    /// taken, at its lowest 9).
    pub crackle: f32,
    pub mix: f32,
    pub out_db: f32,
    pub delta: bool,
}

impl Default for ClickSettings {
    fn default() -> Self {
        ClickSettings {
            on: false,
            sensitivity: 0.5,
            crackle: 0.0,
            mix: 1.0,
            out_db: 0.0,
            delta: false,
        }
    }
}

impl ClickSettings {
    pub fn sane(&self) -> ClickSettings {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        ClickSettings {
            sensitivity: f(self.sensitivity, 0.0, 1.0, 0.5),
            crackle: f(self.crackle, 0.0, 1.0, 0.0),
            mix: f(self.mix, 0.0, 1.0, 1.0),
            out_db: f(self.out_db, -24.0, 24.0, 0.0),
            ..*self
        }
    }
}

/// The de-click module.
#[derive(Clone, Debug)]
pub struct Declick {
    s: ClickSettings,
    ring_in: [Vec<f32>; 2],
    ring_out: [Vec<f32>; 2],
    /// The sample count (the rings' position) and the block grid.
    t: usize,
    phase: usize,
    win: Vec<f32>,
    e: Vec<f64>,
    mag: Vec<f64>,
    a: Vec<f64>,
    ar: ArScratch,
    gap: GapScratch,
    crackle: Crackle,
    hp: Vec<f64>,
    click_floor: f64,
    /// Input samples in a row under [`QUIET`].
    quiet: usize,
    /// Clicks rebuilt in the last block (both channels) and in all.
    last: usize,
    total: u64,
}

impl Default for Declick {
    fn default() -> Self {
        Self::new(48_000.0)
    }
}

impl Declick {
    pub fn new(sample_rate: f32) -> Self {
        let crackle = Crackle::new(sample_rate);
        assert!(crackle.reach() < AHEAD.min(BEFORE));
        Declick {
            s: ClickSettings::default(),
            ring_in: [vec![0.0; RING], vec![0.0; RING]],
            ring_out: [vec![0.0; RING], vec![0.0; RING]],
            t: 0,
            phase: 0,
            win: vec![0.0; WINDOW],
            e: vec![0.0; BLOCK],
            mag: vec![0.0; BLOCK],
            a: vec![0.0; ORDER + 1],
            ar: ArScratch::new(BEFORE + BLOCK, ORDER),
            gap: GapScratch::new(MAX_GAP, BEFORE, AFTER),
            crackle,
            hp: vec![0.0; WINDOW],
            click_floor: 0.0,
            quiet: usize::MAX,
            last: 0,
            total: 0,
        }
    }

    pub fn set(&mut self, s: &ClickSettings) {
        self.s = s.sane();
    }

    pub fn settings(&self) -> &ClickSettings {
        &self.s
    }

    /// Clicks rebuilt in the last block judged (both channels), and since the reset.
    pub fn repaired(&self) -> (usize, u64) {
        (self.last, self.total)
    }

    pub fn reset(&mut self) {
        for c in 0..2 {
            self.ring_in[c].fill(0.0);
            self.ring_out[c].fill(0.0);
        }
        self.t = 0;
        self.phase = 0;
        self.quiet = usize::MAX;
        self.last = 0;
        self.total = 0;
    }

    /// Whether everything still to come out is under [`QUIET`] (the unit's drain asks).
    pub fn drained(&self) -> bool {
        self.quiet >= WINDOW + LATENCY
    }

    /// One stereo sample in; out, the repaired sample and the input, both `LATENCY` ago.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> ((f32, f32), (f32, f32)) {
        let mask = RING - 1;
        let at = self.t & mask;
        self.ring_in[0][at] = l;
        self.ring_in[1][at] = r;
        if l.abs() < QUIET && r.abs() < QUIET {
            self.quiet = self.quiet.saturating_add(1);
        } else {
            self.quiet = 0;
        }
        self.phase += 1;
        if self.phase == BLOCK {
            self.phase = 0;
            self.last = 0;
            for c in 0..2 {
                self.judge(c);
            }
        }
        let back = self.t.wrapping_sub(LATENCY) & mask;
        let wet = (self.ring_out[0][back], self.ring_out[1][back]);
        let dry = (self.ring_in[0][back], self.ring_in[1][back]);
        self.t = self.t.wrapping_add(1);
        wet_dry(wet, dry)
    }

    /// Judge the block that ended `AHEAD` samples ago on channel `c`, and hand it on.
    fn judge(&mut self, c: usize) {
        let mask = RING - 1;
        // The window: the context (already repaired), the block and the look-ahead (as
        // they came in).
        let w0 = self.t.wrapping_add(1).wrapping_sub(WINDOW);
        let mut sound = false;
        for i in 0..WINDOW {
            let at = w0.wrapping_add(i) & mask;
            let v = if i < BEFORE {
                self.ring_out[c][at]
            } else {
                self.ring_in[c][at]
            };
            sound |= v != 0.0;
            self.win[i] = v;
        }
        if sound && self.s.on {
            self.repair();
        }
        for i in BEFORE..BEFORE + BLOCK {
            let at = w0.wrapping_add(i) & mask;
            self.ring_out[c][at] = self.win[i];
        }
    }

    fn repair(&mut self) {
        let mut done = 0;
        // The crackle first, subtracted: the small impulses out of the way, cleanly, before
        // the clicks' model is fitted.
        if self.s.crackle > 0.0 {
            let kc = 9.0 - 4.0 * self.s.crackle as f64;
            done += self.crackle.run(
                &mut self.win,
                BEFORE,
                BEFORE + BLOCK,
                kc,
                &mut self.hp,
                &mut self.mag,
            );
        }
        // Then the editor's pass: clicks at the sensitivity's threshold — raised by
        // [`CLICK_RAISE`] with the de-crackle on (what is smaller was the de-crackle's).
        let k = declick::threshold(self.s.sensitivity);
        let k = if self.s.crackle > 0.0 {
            k + CLICK_RAISE
        } else {
            k
        };
        done += self.pass(k);
        self.last += done;
        self.total += done as u64;
    }

    /// One pass of finding and rebuilding at `k` robust scales; how many were rebuilt.
    fn pass(&mut self, k: f64) -> usize {
        let (b0, b1) = (BEFORE, BEFORE + BLOCK);
        declick::ar_fit_into(&self.win[..b1], ORDER, &mut self.a, &mut self.ar);
        let sigma = declick::block_errors(&self.win, b0, b1, &self.a, &mut self.e, &mut self.mag);
        if sigma <= 1e-9 {
            return 0;
        }
        let mark = k * sigma;
        // With the de-crackle on, a click must also stand out of the music itself: a peak
        // error of at least [`CLICK_OVER`] of the block's level (what is smaller was the
        // de-crackle's, and rebuilding it costs more than it is).
        self.click_floor = if self.s.crackle > 0.0 {
            let n = (b1 - b0) as f64;
            let e2: f64 = self.win[b0..b1]
                .iter()
                .map(|v| (*v as f64) * (*v as f64))
                .sum();
            CLICK_OVER * (e2 / n).sqrt()
        } else {
            0.0
        };
        let mut done = 0;
        // Runs of suspects, merged as the editor merges them: (first, last + 1).
        let mut run: Option<(usize, usize)> = None;
        for i in 0..=BLOCK {
            let at = b0 + i;
            let hit = i < BLOCK && self.e[i].abs() > mark;
            if hit {
                run = match run {
                    Some((a, b)) if at <= b + MERGE => Some((a, at + 1)),
                    Some(prev) => {
                        done += self.rebuild_run(prev);
                        Some((at, at + 1))
                    }
                    None => Some((at, at + 1)),
                };
            }
            if i == BLOCK {
                if let Some(prev) = run.take() {
                    done += self.rebuild_run(prev);
                }
            }
            if done >= MAX_REPAIRS {
                break;
            }
        }
        done
    }

    /// Rebuild one run of suspects as the editor does (widened, the least-squares
    /// autoregressive rebuilding); 1 if it was rebuilt.
    fn rebuild_run(&mut self, (a, b): (usize, usize)) -> usize {
        // With the de-crackle on, an impulse of a few samples was its to take (subtracted,
        // not rebuilt); what is left of one is not rebuilt either.
        if self.s.crackle > 0.0 {
            let peak = (a..b).fold(0.0f64, |m, t| m.max(self.e[t - BEFORE].abs()));
            if b - a <= IMPULSE_LEN || peak < self.click_floor {
                return 0;
            }
        }
        let (from, to) = declick::widen(a, b, BEFORE, WINDOW);
        if to - from > MAX_GAP {
            return 0;
        }
        match declick::rebuild(&mut self.win, from, to, BEFORE, AFTER, &mut self.gap) {
            Ok(()) => 1,
            Err(_) => 0,
        }
    }
}

#[inline]
fn wet_dry(wet: (f32, f32), dry: (f32, f32)) -> ((f32, f32), (f32, f32)) {
    (wet, dry)
}
