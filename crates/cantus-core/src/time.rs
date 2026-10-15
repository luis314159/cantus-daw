//! Musical time.
//!
//! Every position and duration in a Cantus project is stored in **ticks** at a fixed
//! resolution of [`PPQ`] ticks per quarter note. Seconds are derived at playback time from
//! the tempo, never stored — so changing the tempo never moves a note (ADR 0006).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// Pulses per quarter note. 960 is divisible by 2, 3, 4, 5, 6, 8, 10, 12, 15, 16, 32, 64,
/// so triplets, quintuplets and 1/64 notes are all exact integers.
pub const PPQ: i64 = 960;

/// A musical position or duration in ticks (see [`PPQ`]).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Ticks(pub i64);

impl Ticks {
    /// Whether it is zero (serde's `skip_serializing_if`).
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub const ZERO: Ticks = Ticks(0);
    pub const QUARTER: Ticks = Ticks(PPQ);
    pub const EIGHTH: Ticks = Ticks(PPQ / 2);
    pub const SIXTEENTH: Ticks = Ticks(PPQ / 4);

    pub const fn new(ticks: i64) -> Self {
        Ticks(ticks)
    }

    pub const fn beats(beats: i64) -> Self {
        Ticks(beats * PPQ)
    }

    pub fn from_beats_f64(beats: f64) -> Self {
        Ticks((beats * PPQ as f64).round() as i64)
    }

    pub fn as_beats_f64(self) -> f64 {
        self.0 as f64 / PPQ as f64
    }

    pub const fn raw(self) -> i64 {
        self.0
    }

    pub fn max(self, other: Ticks) -> Ticks {
        Ticks(self.0.max(other.0))
    }

    pub fn min(self, other: Ticks) -> Ticks {
        Ticks(self.0.min(other.0))
    }

    pub fn clamp(self, lo: Ticks, hi: Ticks) -> Ticks {
        Ticks(self.0.clamp(lo.0, hi.0))
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Snap to the nearest multiple of `grid` (grid must be > 0).
    pub fn snap(self, grid: Ticks) -> Ticks {
        debug_assert!(grid.0 > 0);
        let g = grid.0;
        Ticks(((self.0 as f64 / g as f64).round() as i64) * g)
    }

    /// Snap *down* to the previous multiple of `grid`.
    pub fn snap_floor(self, grid: Ticks) -> Ticks {
        debug_assert!(grid.0 > 0);
        Ticks(self.0.div_euclid(grid.0) * grid.0)
    }

    /// Length in seconds of `self` at a constant tempo.
    pub fn to_seconds(self, bpm: f64) -> f64 {
        self.as_beats_f64() * 60.0 / bpm
    }

    /// Ticks corresponding to `seconds` at a constant tempo.
    pub fn from_seconds(seconds: f64, bpm: f64) -> Ticks {
        Ticks::from_beats_f64(seconds * bpm / 60.0)
    }

    /// Exact sample position at a constant tempo (fractional samples preserved).
    pub fn to_samples_f64(self, bpm: f64, sample_rate: f64) -> f64 {
        self.to_seconds(bpm) * sample_rate
    }
}

impl fmt::Display for Ticks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}t", self.0)
    }
}

impl Add for Ticks {
    type Output = Ticks;
    fn add(self, rhs: Ticks) -> Ticks {
        Ticks(self.0 + rhs.0)
    }
}
impl Sub for Ticks {
    type Output = Ticks;
    fn sub(self, rhs: Ticks) -> Ticks {
        Ticks(self.0 - rhs.0)
    }
}
impl AddAssign for Ticks {
    fn add_assign(&mut self, rhs: Ticks) {
        self.0 += rhs.0;
    }
}
impl SubAssign for Ticks {
    fn sub_assign(&mut self, rhs: Ticks) {
        self.0 -= rhs.0;
    }
}
impl Mul<i64> for Ticks {
    type Output = Ticks;
    fn mul(self, rhs: i64) -> Ticks {
        Ticks(self.0 * rhs)
    }
}
impl Div<i64> for Ticks {
    type Output = Ticks;
    fn div(self, rhs: i64) -> Ticks {
        Ticks(self.0 / rhs)
    }
}
impl Neg for Ticks {
    type Output = Ticks;
    fn neg(self) -> Ticks {
        Ticks(-self.0)
    }
}

/// A time signature such as 4/4 or 7/8.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeSignature {
    pub numerator: u8,
    pub denominator: u8,
}

impl Default for TimeSignature {
    fn default() -> Self {
        TimeSignature {
            numerator: 4,
            denominator: 4,
        }
    }
}

impl TimeSignature {
    pub const fn new(numerator: u8, denominator: u8) -> Self {
        TimeSignature {
            numerator,
            denominator,
        }
    }

    /// Ticks in one beat of this signature (a "beat" is one `1/denominator` note).
    pub fn ticks_per_beat(self) -> Ticks {
        Ticks(PPQ * 4 / self.denominator.max(1) as i64)
    }

    pub fn ticks_per_bar(self) -> Ticks {
        self.ticks_per_beat() * self.numerator.max(1) as i64
    }

    /// Bar/beat/tick display, 1-based, e.g. `(3, 2, 480)` for bar 3, beat 2, 480 ticks in.
    pub fn bbt(self, pos: Ticks) -> (i64, i64, i64) {
        let bar_len = self.ticks_per_bar().0;
        let beat_len = self.ticks_per_beat().0;
        let bar = pos.0.div_euclid(bar_len);
        let in_bar = pos.0.rem_euclid(bar_len);
        let beat = in_bar / beat_len;
        let tick = in_bar % beat_len;
        (bar + 1, beat + 1, tick)
    }
}

impl fmt::Display for TimeSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}

/// Tempo in beats per minute, validated to a sane range.
pub const MIN_BPM: f64 = 20.0;
pub const MAX_BPM: f64 = 999.0;

pub fn clamp_bpm(bpm: f64) -> f64 {
    if bpm.is_finite() {
        bpm.clamp(MIN_BPM, MAX_BPM)
    } else {
        120.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beats_roundtrip() {
        assert_eq!(Ticks::beats(3), Ticks(2880));
        assert_eq!(Ticks::beats(3).as_beats_f64(), 3.0);
        assert_eq!(Ticks::from_beats_f64(0.5), Ticks::EIGHTH);
    }

    #[test]
    fn seconds_at_120_bpm() {
        assert!((Ticks::beats(2).to_seconds(120.0) - 1.0).abs() < 1e-12);
        assert_eq!(Ticks::from_seconds(1.0, 120.0), Ticks::beats(2));
        assert!((Ticks::beats(1).to_samples_f64(120.0, 48_000.0) - 24_000.0).abs() < 1e-9);
    }

    #[test]
    fn snapping() {
        assert_eq!(Ticks(1000).snap(Ticks::QUARTER), Ticks(960));
        assert_eq!(Ticks(1500).snap(Ticks::QUARTER), Ticks(1920));
        assert_eq!(Ticks(1500).snap_floor(Ticks::QUARTER), Ticks(960));
        assert_eq!(Ticks(-10).snap_floor(Ticks::QUARTER), Ticks(-960));
    }

    #[test]
    fn bbt_display() {
        let ts = TimeSignature::default();
        assert_eq!(ts.ticks_per_bar(), Ticks::beats(4));
        assert_eq!(ts.bbt(Ticks::ZERO), (1, 1, 0));
        assert_eq!(ts.bbt(Ticks::beats(5) + Ticks(10)), (2, 2, 10));
        let seven8 = TimeSignature::new(7, 8);
        assert_eq!(seven8.ticks_per_beat(), Ticks::EIGHTH);
        assert_eq!(seven8.ticks_per_bar(), Ticks(7 * 480));
    }
}
