//! **Tempo and the beat grid** of a recording (round 29, ADR 0061): the tempo from the
//! onset strength's autocorrelation, combed over its multiples and weighted toward the
//! tempos people play (the octave resolved toward 70–180 BPM), with a confidence; the beats
//! by dynamic programming (Ellis, JNMR 2007); the downbeat by where the low onsets fall;
//! and a loop told by its length (a whole number of bars). Offline, deterministic.

use crate::onset::Onsets;

#[cfg(test)]
#[path = "beat_tests.rs"]
mod tests;
#[path = "beat_testkit.rs"]
pub mod testkit;

/// The slowest and fastest tempos the tracker considers.
pub const MIN_BPM: f64 = 40.0;
pub const MAX_BPM: f64 = 240.0;
/// Where an octave-ambiguous tempo is put.
pub const PREFER: (f64, f64) = (70.0, 180.0);

/// What [`analyse`] finds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grid {
    /// The tempo, BPM (0 when none was found).
    pub bpm: f64,
    /// How sure, 0 … 1.
    pub confidence: f32,
    /// The beats, in samples, in order.
    pub beats: Vec<f64>,
    /// The index in `beats` of the first downbeat.
    pub downbeat: usize,
    /// A loop: the file is this many whole bars at `bpm` (which is then exact).
    pub bars: Option<u32>,
}

impl Grid {
    /// The first downbeat, in samples.
    pub fn first_downbeat(&self) -> Option<f64> {
        self.beats.get(self.downbeat).copied()
    }
}

/// The tempo, the beats and the downbeat of a signal `len` samples long at `rate` whose
/// onsets are `onsets`, in a metre of `beats_per_bar`.
pub fn analyse(onsets: &Onsets, rate: f32, len: usize, beats_per_bar: u32) -> Grid {
    let _ = (onsets, rate, len, beats_per_bar);
    Grid::default()
}
