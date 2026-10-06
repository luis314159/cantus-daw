//! Culmen, the mastering maximizer: up to three bands of two-sided dynamics, a full band,
//! a drive into a look-ahead true-peak brickwall, delta and level-matched listening, and
//! dither — FL Maximus's idea, built from the family's tested parts.
//!
//! ## The signal path
//!
//! ```text
//! in ─┬─ band stage (Multiplex) ─ full band (Multiplex, one band) ─ × drive ─ brickwall (Terminus) ─┬─ Δ? ─ 1:1 / match ─ dither ─ out
//!     └─ delay (band latency) ───────────────────────────────────── × drive ─ delay (brickwall) ────┘
//! ```
//!
//! Nothing is written twice: the bands are a [`Multiplex`] (its dynamic shelves,
//! Linkwitz–Riley split or linear-phase split, its two-sided gain computer); the full band
//! is a one-band `Multiplex` in its dynamic mode (a pure gain, a wire while it does not
//! move); the brickwall is a [`Terminus`] with its own gain at 0 dB and its monitor,
//! delta and dither off — those are Culmen's, because they have to cover every stage.
//!
//! ## What the null tests rely on
//!
//! - At every default the bands and the full band are at 1:1 — a wire to the bit — the
//!   drive is exactly 1, and the brickwall with nothing to limit is an exact delay: the
//!   output is the input delayed by [`Culmen::latency`], to the bit.
//! - The reference path (for the delta) is the same multiplication by the same drive at
//!   the same sample, through delays as long as the stages', so with nothing to do the
//!   delta is exactly zero.
//! - The block is cut into pieces of at most [`GRID`] samples that end on the 16-sample
//!   grid and on the 10 ms columns, both counted from the start of the stream; the stages
//!   are block-size independent, the per-sample work is per sample, and the telemetry is
//!   read only on the grid — so neither the output nor the telemetry depends on how the
//!   host cuts its blocks.

use crate::math;
use crate::multiplex::{self, BandSettings, Mode, Slope};
use crate::terminus::loudness;
use crate::terminus::{self, LimSettings, Monitor, Shaping, DITHER_BITS};

/// The bands' indices: low, mid, high, and the full band.
pub const LO: usize = 0;
pub const MID: usize = 1;
pub const HI: usize = 2;
pub const FULL: usize = 3;

/// The pieces a block is cut into: the 16-sample grid of the family's DSPs.
pub const GRID: usize = 16;
/// The narrowest a band may be between the crossovers: a third of an octave.
pub const MIN_GAP: f32 = 1.259_921;
/// How far under the ceiling the brickwall aims with true peak on, dB. Terminus holds its
/// own corpus 0.019 dB under; band-processed drums driven 18 dB into its clipper measured
/// 0.032 dB over by the 32× reference, so Culmen asks for this much more.
pub const TP_MARGIN_DB: f32 = 0.05;

/// Everything Culmen is.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MaxSettings {
    /// Low, mid, high and full band, as Multiplex reads a band (`used`, `from_hz`, the
    /// lookahead and the key fields are Culmen's to set, not the caller's).
    pub bands: [BandSettings; 4],
    /// 1: no band stage; 2: low and high split at `x1`; 3: low, mid and high.
    pub count: usize,
    pub x1_hz: f32,
    pub x2_hz: f32,
    pub mode: Mode,
    pub slope: Slope,
    /// The drive into the brickwall, dB.
    pub drive_db: f32,
    /// The brickwall. Its gain, monitor, delta and dither are ignored: Culmen's own are
    /// below.
    pub limiter: LimSettings,
    /// Hear what Culmen changes: the driven input, delayed, minus the output.
    pub delta: bool,
    pub monitor: Monitor,
    /// Index into [`DITHER_BITS`]; 0 is off.
    pub dither: usize,
    pub shaping: Shaping,
}

/// A band at 1:1, on, doing nothing.
pub fn idle_band() -> BandSettings {
    BandSettings {
        used: true,
        on: true,
        thr_db: -12.0,
        ratio: 1.0,
        thr_lo_db: -40.0,
        ratio_lo: 1.0,
        ..BandSettings::default()
    }
}

impl Default for MaxSettings {
    fn default() -> Self {
        MaxSettings {
            bands: [idle_band(); 4],
            count: 3,
            x1_hz: 150.0,
            x2_hz: 5_000.0,
            mode: Mode::Dynamic,
            slope: Slope::S24,
            drive_db: 0.0,
            limiter: LimSettings::default(),
            delta: false,
            monitor: Monitor::Normal,
            dither: 0,
            shaping: Shaping::Flat,
        }
    }
}

impl MaxSettings {
    /// The crossovers in force: `x1` kept under the top, `x2` at least a third of an
    /// octave above `x1`.
    pub fn crossovers(&self) -> (f32, f32) {
        let x1 = self.x1_hz.clamp(20.0, 20_000.0 / MIN_GAP);
        let x2 = self.x2_hz.clamp(x1 * MIN_GAP, 20_000.0);
        (x1, x2)
    }

    /// The band stage as Multiplex is told it: bands numbered as Culmen numbers them
    /// (0 low, 1 mid, 2 high), so Multiplex's meters read by the same numbers.
    pub fn band_stage(&self) -> multiplex::Settings {
        let mut m = multiplex::Settings {
            mode: self.mode,
            slope: self.slope,
            ..multiplex::Settings::default()
        };
        let (x1, x2) = self.crossovers();
        let place = |b: usize, from: f32| {
            let mut s = self.bands[b];
            s.used = true;
            s.from_hz = from;
            s.mute = false;
            s.lookahead_ms = 0.0;
            s.key_ext = false;
            s.key_free = false;
            s
        };
        match self.count {
            3 => {
                m.bands[LO] = place(LO, 20.0);
                m.bands[MID] = place(MID, x1);
                m.bands[HI] = place(HI, x2);
            }
            2 => {
                m.bands[LO] = place(LO, 20.0);
                m.bands[HI] = place(HI, x1);
            }
            _ => {}
        }
        m
    }

    /// The full band as a one-band Multiplex: dynamic (a pure gain), never split.
    pub fn full_stage(&self) -> multiplex::Settings {
        let mut m = multiplex::Settings::default();
        let mut b = self.bands[FULL];
        b.used = true;
        b.from_hz = 20.0;
        b.solo = false;
        b.mute = false;
        b.lookahead_ms = 0.0;
        b.key_ext = false;
        b.key_free = false;
        m.bands[0] = b;
        m
    }

    /// The brickwall as Terminus is told it: its own gain, monitor, delta and dither off.
    pub fn brickwall(&self) -> LimSettings {
        let margin = if self.limiter.true_peak {
            TP_MARGIN_DB
        } else {
            0.0
        };
        LimSettings {
            gain_db: 0.0,
            ceiling_db: self.limiter.ceiling_db - margin,
            monitor: Monitor::Normal,
            delta: false,
            dither: 0,
            ..self.limiter
        }
    }

    /// Whether band `b` is part of the band stage at this count.
    pub fn band_in_use(&self, b: usize) -> bool {
        match b {
            LO | HI => self.count >= 2,
            MID => self.count >= 3,
            _ => true,
        }
    }
}

/// The whole delay of a set of settings at a sample rate, without building a Culmen: the
/// band stage's (linear-phase split) plus the brickwall's.
pub fn latency_of(s: &MaxSettings, sample_rate: f32, linear_latency: usize) -> usize {
    let bands = if s.count >= 2 && s.mode == Mode::Linear {
        linear_latency
    } else {
        0
    };
    bands + terminus::latency_of(&s.brickwall(), sample_rate)
}

#[path = "culmen_engine.rs"]
mod engine;
#[path = "culmen_telemetry.rs"]
pub mod telemetry;

pub use engine::Culmen;

#[cfg(test)]
#[path = "culmen_tests.rs"]
mod tests;

/// dB of a linear magnitude, −160 for silence.
#[inline]
pub(crate) fn db(lin: f32) -> f32 {
    if lin > 1e-8 {
        20.0 * math::log10(lin)
    } else {
        -160.0
    }
}

/// What the meters and the match use: re-exported so the editor needs one module.
pub use loudness::lufs;

/// Dither bits for an index.
pub fn dither_bits(i: usize) -> u32 {
    DITHER_BITS[i.min(DITHER_BITS.len() - 1)]
}
