//! Arcus's bowed pitch (a child of `arcus`): the correction a bowed note's loop is set by.
//!
//! A bowed string does not sound its free loop's pitch: the stick and slip times the
//! Helmholtz corner's return, and the friction's force flattens it as it grows (Schumacher's
//! flattening; Fides measured +3.1 to −1.2 cents across C2–C6 at its default and a period
//! tracker that moved the loop failed, `docs/round41/fides.md`). So the offset is **measured
//! offline**, on this model, per key × dynamic × contact point, and the loop is set that many
//! cents the other way while the bow is drawn — deterministic, the same at every block size.
//! `tools/arcus/pitch-table.sh` measures it (`cargo run -p cantus-dsp --release --example
//! arcus_pitch_table`) and writes `arcus_pitch_data.rs`; `tests_gate` holds the result
//! within a cent over the grid and between its points.

use super::tables::Instrument;

// Measured data: its cents may read like π, π/2 or √2 — the generated file allows
// `clippy::approx_constant` inside (`#![allow]`, written by `examples/arcus_pitch_table.rs`).
#[path = "arcus_pitch_data.rs"]
mod data;
use super::upper::pitch_data as up; // r42:arcus-upper
                                    // `arcus_pitch_data.rs`'s violin and viola rows are empty and unread: the upper strings'
                                    // table is `upper::pitch_data` (the generator may drop them). // r42:arcus-upper
const _: usize = data::VIOLIN_KEYS
    + data::VIOLA_KEYS
    + data::VIOLIN.len()
    + data::VIOLA.len()
    + data::VIOLIN_LO as usize
    + data::VIOLA_LO as usize; // r42:arcus-upper

/// The dynamics the table is measured at.
pub const DYN: [f64; 9] = [0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0];
/// The contact points (`bow.pos`) it is measured at.
pub const POS: [f64; 5] = [-1.0, -0.5, 0.0, 0.5, 1.0];

/// One instrument's table: cents for each key from `lo`, each dynamic, each contact point
/// (`cents[(k·DYN.len() + d)·POS.len() + p]`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct InstTable {
    pub lo: u8,
    pub keys: usize,
    pub cents: &'static [f32],
}

impl InstTable {
    fn at(&self, k: usize, d: usize, p: usize) -> f64 {
        let i = (k * DYN.len() + d) * POS.len() + p;
        self.cents.get(i).copied().unwrap_or(0.0) as f64
    }
}

/// r42:arcus-p5 — the dynamics a tremolo's table is measured at.
pub const TREM_DYN: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

/// r42:arcus-p5 — one instrument's tremolo table: cents for each key from `lo` and each of
/// [`TREM_DYN`] (`cents[k·TREM_DYN.len() + d]`), as the study's tracker hears the strokes.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TremTable {
    pub lo: u8,
    pub keys: usize,
    pub cents: &'static [f32],
}

/// Every instrument's table.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PitchTable {
    pub tables: [InstTable; 4],
    /// r42:arcus-p5 — the tremolo's: a stroke is short and light, its pitch not a held
    /// note's (measured by `tools/arcus/trem-table.sh`, `tests_p5::write_tremolo_table`).
    pub trem: [TremTable; 4],
}

/// The table measured on this model (`arcus_pitch_data.rs`).
pub const TABLE: PitchTable = PitchTable {
    tables: [
        InstTable {
            lo: data::CELLO_LO,
            keys: data::CELLO_KEYS,
            cents: &data::CELLO,
        },
        InstTable {
            lo: data::BASS_LO,
            keys: data::BASS_KEYS,
            cents: &data::BASS,
        },
        // The violin's and the viola's: `upper::pitch_data` (their voices run at
        // `upper::factor` × the rate; `examples/arcus_upper_pitch.rs`). // r42:arcus-upper
        InstTable {
            lo: up::VIOLIN_LO,     // r42:arcus-upper
            keys: up::VIOLIN_KEYS, // r42:arcus-upper
            cents: &up::VIOLIN,    // r42:arcus-upper
        },
        InstTable {
            lo: up::VIOLA_LO,     // r42:arcus-upper
            keys: up::VIOLA_KEYS, // r42:arcus-upper
            cents: &up::VIOLA,    // r42:arcus-upper
        },
    ],
    trem: [
        TremTable {
            lo: trem::CELLO_LO,
            keys: trem::CELLO_KEYS,
            cents: &trem::CELLO,
        },
        TremTable {
            lo: trem::BASS_LO,
            keys: trem::BASS_KEYS,
            cents: &trem::BASS,
        },
        TremTable {
            lo: trem::VIOLIN_LO,
            keys: trem::VIOLIN_KEYS,
            cents: &trem::VIOLIN,
        },
        TremTable {
            lo: trem::VIOLA_LO,
            keys: trem::VIOLA_KEYS,
            cents: &trem::VIOLA,
        },
    ],
};

/// r42:arcus-p5 — the measured tremolo table (`arcus_trem_pitch_data.rs`).
#[path = "arcus_trem_pitch_data.rs"]
mod trem;

/// Where `x` falls on `grid` (ascending): the cell's first index and the fraction into it.
fn cell(grid: &[f64], x: f64) -> (usize, f64) {
    let last = grid.len() - 1;
    if x <= grid[0] {
        return (0, 0.0);
    }
    if x >= grid[last] {
        return (last - 1, 1.0);
    }
    let mut i = 0;
    while i + 1 < last && x > grid[i + 1] {
        i += 1;
    }
    (i, (x - grid[i]) / (grid[i + 1] - grid[i]))
}

impl PitchTable {
    /// The cents a bowed note of `inst` at `semis` (MIDI, fractional), dynamic `d` and
    /// contact point `pos` sounds from its loop — interpolated between the measured keys,
    /// dynamics and points (keys outside the table take its end).
    pub fn cents(&self, inst: Instrument, semis: f64, d: f64, pos: f64) -> f64 {
        let t = &self.tables[inst as usize];
        if t.keys == 0 {
            return 0.0;
        }
        let kx = (semis - t.lo as f64).clamp(0.0, (t.keys - 1) as f64);
        let k0 = (kx.floor() as usize).min(t.keys.saturating_sub(2));
        let fk = if t.keys < 2 { 0.0 } else { kx - k0 as f64 };
        let k1 = (k0 + 1).min(t.keys - 1);
        let (d0, fd) = cell(&DYN, d);
        let (p0, fp) = cell(&POS, pos);
        let mut acc = 0.0;
        for (ki, wk) in [(k0, 1.0 - fk), (k1, fk)] {
            for (di, wd) in [(d0, 1.0 - fd), (d0 + 1, fd)] {
                for (pi, wp) in [(p0, 1.0 - fp), (p0 + 1, fp)] {
                    let w = wk * wd * wp;
                    if w != 0.0 {
                        acc += w * t.at(ki, di, pi);
                    }
                }
            }
        }
        acc
    }
}

impl PitchTable {
    /// r42:arcus-p5 — the cents a tremolo of `inst` at `semis` and dynamic `d` sounds from
    /// its loop (interpolated between the measured keys and dynamics; none measured: 0).
    pub fn trem_cents(&self, inst: Instrument, semis: f64, d: f64) -> f64 {
        let t = &self.trem[inst as usize];
        if t.keys == 0 {
            return 0.0;
        }
        let kx = (semis - t.lo as f64).clamp(0.0, (t.keys - 1) as f64);
        let k0 = (kx.floor() as usize).min(t.keys.saturating_sub(2));
        let fk = if t.keys < 2 { 0.0 } else { kx - k0 as f64 };
        let k1 = (k0 + 1).min(t.keys - 1);
        let (d0, fd) = cell(&TREM_DYN, d);
        let at =
            |k: usize, d: usize| t.cents.get(k * TREM_DYN.len() + d).copied().unwrap_or(0.0) as f64;
        let mut acc = 0.0;
        for (ki, wk) in [(k0, 1.0 - fk), (k1, fk)] {
            for (di, wd) in [(d0, 1.0 - fd), (d0 + 1, fd)] {
                let w = wk * wd;
                if w != 0.0 {
                    acc += w * at(ki, di);
                }
            }
        }
        acc
    }
}
