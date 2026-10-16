//! The tempo map: the tempo and the metre along the song, and named markers (round 29,
//! ADR 0060).
//!
//! **Round 29 contract.** The signatures below are shared by every round-29 builder — the
//! warp builder keys its renders on [`TempoMap::span_key`] and places audio with
//! [`TempoMap::seconds_at`], the freeze builder measures render lengths with it, the
//! controllers builder edits the tempo through it — so they are fixed: a builder that needs
//! a new method *adds* one and says so. The **tempo** builder owns this file and replaces
//! the bodies: today they read only the map's first tempo and first metre, which is exactly
//! what a project without a tempo track is, and gives the same `f64`s as
//! [`Ticks::to_seconds`] (a constant-tempo project must render bit-identically once the
//! map is real).
//!
//! Positions stay in ticks (ADR 0006): a tempo change never moves a note. Seconds and
//! samples are derived through the map. Before tick 0 (a count-in, a pre-roll) the first
//! tempo and the first metre extend backwards.

use crate::model::Project;
use crate::time::{Ticks, TimeSignature, PPQ};
use serde::{Deserialize, Serialize};

/// One point of the tempo track: from `tick` on, the tempo is `bpm`; when `ramp` is set
/// the tempo moves continuously from this point's `bpm` to the next point's (the shape is
/// ADR 0060's decision), otherwise it holds until the next point.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TempoPoint {
    pub tick: Ticks,
    pub bpm: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ramp: bool,
}

/// A metre change: from `tick` (which must fall on a bar line of the metre before it) the
/// time signature is `signature`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetreChange {
    pub tick: Ticks,
    pub signature: TimeSignature,
}

/// A stable marker id (never an index: commands, maps and exports name markers by it).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct MarkerId(pub u64);

/// A named marker on the timeline (jump to it, loop or export between two of them).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub id: MarkerId,
    pub tick: Ticks,
    pub name: String,
}

/// The compiled tempo and metre of a project, shared (`Arc`) with the engine. Lookups never
/// allocate, so the audio thread may call any `&self` method.
#[derive(Clone, Debug, PartialEq)]
pub struct TempoMap {
    /// Sorted by tick, never empty; the first point stands for the whole song before it.
    tempo: Vec<TempoPoint>,
    /// Sorted by tick, never empty; the first change stands for the whole song before it.
    metres: Vec<MetreChange>,
}

impl TempoMap {
    /// One tempo and one metre for the whole song — every project before round 29.
    pub fn constant(bpm: f64, signature: TimeSignature) -> Self {
        TempoMap {
            tempo: vec![TempoPoint {
                tick: Ticks::ZERO,
                bpm,
                ramp: false,
            }],
            metres: vec![MetreChange {
                tick: Ticks::ZERO,
                signature,
            }],
        }
    }

    /// A map from a project's points; `bpm` and `signature` are the values at tick 0 used
    /// when the lists are empty. The tempo builder validates and sorts.
    pub fn new(
        bpm: f64,
        signature: TimeSignature,
        tempo: &[TempoPoint],
        metres: &[MetreChange],
    ) -> Self {
        let _ = (tempo, metres);
        TempoMap::constant(bpm, signature)
    }

    /// Whether the tempo never changes along the song (the metre may).
    pub fn is_constant(&self) -> bool {
        self.tempo.len() == 1
    }

    /// The tempo points, in order.
    pub fn points(&self) -> &[TempoPoint] {
        &self.tempo
    }

    /// The metre changes, in order.
    pub fn metres(&self) -> &[MetreChange] {
        &self.metres
    }

    /// The tempo at `tick` (inside a ramp, the ramp's value there).
    pub fn bpm_at(&self, tick: f64) -> f64 {
        let _ = tick;
        self.tempo[0].bpm
    }

    /// Seconds from tick 0 to `tick` (negative before it).
    pub fn seconds_at(&self, tick: f64) -> f64 {
        tick / PPQ as f64 * 60.0 / self.tempo[0].bpm
    }

    /// The tick heard `seconds` after tick 0 (the inverse of [`TempoMap::seconds_at`]).
    pub fn tick_at_seconds(&self, seconds: f64) -> f64 {
        seconds * self.tempo[0].bpm / 60.0 * PPQ as f64
    }

    /// Exact sample position of `tick` from tick 0 at `sample_rate` (fractions kept).
    pub fn sample_at(&self, tick: f64, sample_rate: f64) -> f64 {
        self.seconds_at(tick) * sample_rate
    }

    /// The tick at `sample` samples from tick 0 at `sample_rate`.
    pub fn tick_at_sample(&self, sample: f64, sample_rate: f64) -> f64 {
        self.tick_at_seconds(sample / sample_rate)
    }

    /// Seconds from `from` to `to` (negative when `to` is earlier).
    pub fn seconds_between(&self, from: Ticks, to: Ticks) -> f64 {
        (to - from).to_seconds(self.tempo[0].bpm)
    }

    /// The first tick after `tick` where the tempo stops being what it is at `tick` (a
    /// point, or a ramp's start or end) — where a pass that wants one tempo must end.
    pub fn next_change_after(&self, tick: f64) -> Option<f64> {
        let _ = tick;
        None
    }

    /// The time signature in force at `tick`.
    pub fn signature_at(&self, tick: Ticks) -> TimeSignature {
        let _ = tick;
        self.metres[0].signature
    }

    /// The start of the bar that holds `tick`.
    pub fn bar_start(&self, tick: Ticks) -> Ticks {
        tick.snap_floor(self.metres[0].signature.ticks_per_bar())
    }

    /// Bar/beat/tick, 1-based, counted through every metre change.
    pub fn bbt(&self, tick: Ticks) -> (i64, i64, i64) {
        self.metres[0].signature.bbt(tick)
    }

    /// The bar lines from `from` (inclusive) to `to` (exclusive), in order.
    pub fn bar_lines(&self, from: Ticks, to: Ticks) -> impl Iterator<Item = Ticks> + '_ {
        let bar = self.metres[0].signature.ticks_per_bar();
        let first = if from == from.snap_floor(bar) {
            from
        } else {
            from.snap_floor(bar) + bar
        };
        (0..)
            .map(move |i| first + bar * i)
            .take_while(move |t| *t < to)
    }

    /// A stable hash (FNV-1a, the same on every build) of everything in the map that
    /// decides timing between `from` and `to`: a render that depends on the tempo over a
    /// span (a warped clip, a frozen strip) keys itself on it.
    pub fn span_key(&self, from: Ticks, to: Ticks) -> u64 {
        let _ = (from, to);
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in self.tempo[0].bpm.to_bits().to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h
    }
}

impl Project {
    /// The project's tempo map, compiled from its tempo and metre (round 29: the tempo
    /// builder adds the tempo track's points and metre changes).
    pub fn tempo_map(&self) -> TempoMap {
        TempoMap::new(self.tempo_bpm, self.time_signature, &[], &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_constant_map_gives_the_constant_tempo_arithmetic_to_the_bit() {
        for bpm in [20.0, 97.3, 120.0, 140.0, 999.0] {
            let map = TempoMap::constant(bpm, TimeSignature::default());
            for t in [-7_680, -1, 0, 1, 959, 960, 123_457, 9_600_000] {
                let tick = Ticks(t);
                assert_eq!(
                    map.seconds_at(t as f64).to_bits(),
                    tick.to_seconds(bpm).to_bits()
                );
                assert_eq!(
                    map.sample_at(t as f64, 48_000.0).to_bits(),
                    tick.to_samples_f64(bpm, 48_000.0).to_bits()
                );
            }
        }
    }

    #[test]
    fn seconds_and_ticks_are_inverses() {
        let map = TempoMap::constant(132.0, TimeSignature::default());
        for t in [0.0, 1.0, 480.0, 96_000.5] {
            let back = map.tick_at_seconds(map.seconds_at(t));
            assert!((back - t).abs() < 1e-6, "{t} → {back}");
        }
    }

    #[test]
    fn bars_of_a_constant_metre() {
        let map = TempoMap::constant(120.0, TimeSignature::new(3, 4));
        let lines: Vec<Ticks> = map.bar_lines(Ticks(100), Ticks::beats(10)).collect();
        assert_eq!(
            lines,
            vec![Ticks::beats(3), Ticks::beats(6), Ticks::beats(9)]
        );
        assert_eq!(map.bar_start(Ticks::beats(4)), Ticks::beats(3));
        assert_eq!(map.bbt(Ticks::beats(4)), (2, 2, 0));
        assert_eq!(
            map.bar_lines(Ticks::ZERO, Ticks::beats(3))
                .collect::<Vec<_>>(),
            vec![Ticks::ZERO]
        );
    }

    #[test]
    fn a_project_without_a_tempo_track_is_constant() {
        let p = Project::empty("t");
        let map = p.tempo_map();
        assert!(map.is_constant());
        assert_eq!(map.bpm_at(12_345.0), p.tempo_bpm);
        assert_eq!(map.signature_at(Ticks::beats(99)), p.time_signature);
        assert_eq!(map.next_change_after(0.0), None);
    }
}
