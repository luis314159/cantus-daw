//! The tempo map: the tempo and the metre along the song, and named markers (round 29,
//! ADR 0060).
//!
//! **Round 29 contract.** The signatures of the contract are shared by every round-29
//! builder — the warp builder keys its renders on [`TempoMap::span_key`] and places audio
//! with [`TempoMap::seconds_at`], the freeze builder measures render lengths with it, the
//! controllers builder edits the tempo through it — so they are fixed; methods are added,
//! never changed.
//!
//! Positions stay in ticks (ADR 0006): a tempo change never moves a note. Seconds and
//! samples are derived through the map. Before tick 0 (a count-in, a pre-roll) the first
//! tempo and the first metre extend backwards.
//!
//! **The model.** A project without a tempo track has empty `tempo_points` and
//! `metre_changes`: its map is [`TempoMap::constant`] from `tempo_bpm` and
//! `time_signature`, and every lookup gives the `f64`s of [`Ticks::to_seconds`] to the bit
//! (old projects render bit-identically). With a tempo track the first point sits at tick
//! 0 and agrees with `tempo_bpm`; likewise the first metre change with `time_signature`.
//!
//! **A ramp** (ADR 0060) is linear in ticks — the tempo moves from one point's value to the
//! next one's at a constant rate per beat, as Cubase, Studio One and Bitwig draw it — so
//! the time it takes integrates in closed form: over `Δt` ticks from `b₀` at a slope of `k`
//! BPM per tick, `seconds = 60 / (PPQ·k) · ln(1 + k·Δt / b₀)`, and back,
//! `Δt = b₀ / k · (e^{seconds·PPQ·k/60} − 1)` (`cantus_dsp::math`'s `ln_1p` and `exp_m1`,
//! deterministic on every machine).
//!
//! **Metre changes** fall on bar lines; one that does not (an earlier metre edited after
//! it was placed) starts a bar anyway, and the bar before it is cut short — the ruler
//! shows that partial bar and the change can be moved onto a line (ADR 0060 §3).

#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod tests;

pub mod bars;
pub mod edit;
pub mod keep;

use crate::model::{Color, Project};
use crate::time::{Ticks, TimeSignature, MAX_BPM, MIN_BPM, PPQ};
use cantus_dsp::math;
use serde::{Deserialize, Serialize};

pub use bars::BarLines;

/// Inside a ramp, the engine holds the tempo it tells units (synced effects, hosted
/// plug-ins) for one step of this many ticks — a 64th note — and moves it at each step's
/// start, which is a pass boundary: the steps are counted in ticks from the ramp's start,
/// so they fall at the same song positions whatever the blocks (ADR 0060 §4). The engine's
/// own timing (notes, clips, automation, the click) is exact, never stepped.
pub const RAMP_STEP: i64 = PPQ / 16;

/// How far into a song a tempo point, a metre change or a marker may be (review 5): a
/// million beats — days of music. An agent's or a file's point past it is refused or left
/// out (and said): a ramp to a billion beats made a MIDI export loop for hours.
pub const HORIZON: Ticks = Ticks(PPQ * 1_000_000);

/// One point of the tempo track: from `tick` on, the tempo is `bpm`; when `ramp` is set
/// the tempo moves linearly in ticks from this point's `bpm` to the next point's (ADR
/// 0060), otherwise it holds until the next point.
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
    /// The marker's colour, from the track palette; `None` draws it in the theme's accent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

/// One tempo segment, from a point to the next: where it starts, the seconds there, the
/// tempo there and its slope (BPM per tick; 0 while the tempo holds).
#[derive(Copy, Clone, Debug, PartialEq)]
struct Seg {
    tick: f64,
    sec: f64,
    bpm: f64,
    k: f64,
}

impl Seg {
    /// Seconds from the segment's start to `dt` ticks into it (negative before it: only
    /// the first segment, which holds its tempo backwards).
    #[inline]
    fn seconds(&self, dt: f64) -> f64 {
        if self.k == 0.0 || dt <= 0.0 {
            dt / PPQ as f64 * 60.0 / self.bpm
        } else {
            60.0 / PPQ as f64 / self.k * math::ln_1p(self.k * dt / self.bpm)
        }
    }

    /// Ticks from the segment's start to `s` seconds into it.
    #[inline]
    fn ticks(&self, s: f64) -> f64 {
        if self.k == 0.0 || s <= 0.0 {
            s * self.bpm / 60.0 * PPQ as f64
        } else {
            self.bpm / self.k * math::exp_m1(s * PPQ as f64 * self.k / 60.0)
        }
    }

    #[inline]
    fn bpm_at(&self, tick: f64) -> f64 {
        if self.k == 0.0 || tick <= self.tick {
            self.bpm
        } else {
            self.bpm + self.k * (tick - self.tick)
        }
    }
}

/// One metre segment: where it starts, the 0-based index of the bar starting there and
/// the signature.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Bar {
    tick: i64,
    bar: i64,
    sig: TimeSignature,
}

impl Bar {
    #[inline]
    fn len(&self) -> i64 {
        self.sig.ticks_per_bar().0
    }
}

/// The compiled tempo and metre of a project, shared (`Arc`) with the engine. Lookups never
/// allocate, so the audio thread may call any `&self` method.
#[derive(Clone, Debug, PartialEq)]
pub struct TempoMap {
    /// Sorted by tick, never empty; the first point is at tick 0 and stands for the whole
    /// song before it.
    tempo: Vec<TempoPoint>,
    /// Sorted by tick, never empty; the first change is at tick 0 and stands for the whole
    /// song before it.
    metres: Vec<MetreChange>,
    segs: Vec<Seg>,
    bars: Vec<Bar>,
}

/// A tempo the map can play: finite and within `MIN_BPM..=MAX_BPM`.
pub fn valid_bpm(bpm: f64) -> bool {
    bpm.is_finite() && (MIN_BPM..=MAX_BPM).contains(&bpm)
}

/// A signature the map can count: a numerator of at least 1 and a power-of-two denominator
/// from 1 to 32.
pub fn valid_signature(sig: TimeSignature) -> bool {
    sig.numerator > 0 && matches!(sig.denominator, 1 | 2 | 4 | 8 | 16 | 32)
}

impl TempoMap {
    /// One tempo and one metre for the whole song — every project before round 29.
    pub fn constant(bpm: f64, signature: TimeSignature) -> Self {
        TempoMap::new(bpm, signature, &[], &[])
    }

    /// A map from a project's points; `bpm` and `signature` are the values at tick 0, used
    /// when the lists are empty or do not start at tick 0. The lists are sorted; of two
    /// entries at one tick the later wins; entries before tick 0 are dropped (the first
    /// tempo and metre extend backwards); a tempo out of range is clamped, a signature that
    /// cannot be counted is dropped; a ramp on the last point holds (it has no target).
    pub fn new(
        bpm: f64,
        signature: TimeSignature,
        tempo: &[TempoPoint],
        metres: &[MetreChange],
    ) -> Self {
        let clamp = |b: f64| {
            if b.is_finite() {
                b.clamp(MIN_BPM, MAX_BPM)
            } else {
                120.0
            }
        };
        let mut pts: Vec<TempoPoint> = tempo
            .iter()
            .copied()
            .filter(|p| p.tick.0 >= 0)
            .map(|p| TempoPoint {
                bpm: clamp(p.bpm),
                ..p
            })
            .collect();
        pts.sort_by_key(|p| p.tick);
        pts.reverse();
        pts.dedup_by_key(|p| p.tick);
        pts.reverse();
        if pts.first().is_none_or(|p| p.tick.0 != 0) {
            pts.insert(
                0,
                TempoPoint {
                    tick: Ticks::ZERO,
                    bpm,
                    ramp: false,
                },
            );
        }
        if let Some(last) = pts.last_mut() {
            last.ramp = false;
        }
        let mut segs = Vec::with_capacity(pts.len());
        let mut sec = 0.0;
        for (i, p) in pts.iter().enumerate() {
            let tick = p.tick.0 as f64;
            let k = match pts.get(i + 1) {
                Some(n) if p.ramp => (n.bpm - p.bpm) / (n.tick.0 - p.tick.0) as f64,
                _ => 0.0,
            };
            let seg = Seg {
                tick,
                sec,
                bpm: p.bpm,
                k,
            };
            if let Some(n) = pts.get(i + 1) {
                let len = seg.seconds(n.tick.0 as f64 - tick);
                sec = if i == 0 { len } else { sec + len };
            }
            segs.push(seg);
        }

        let mut ms: Vec<MetreChange> = metres
            .iter()
            .copied()
            .filter(|m| m.tick.0 >= 0 && valid_signature(m.signature))
            .collect();
        ms.sort_by_key(|m| m.tick);
        ms.reverse();
        ms.dedup_by_key(|m| m.tick);
        ms.reverse();
        if ms.first().is_none_or(|m| m.tick.0 != 0) {
            ms.insert(
                0,
                MetreChange {
                    tick: Ticks::ZERO,
                    signature,
                },
            );
        }
        let mut bars: Vec<Bar> = Vec::with_capacity(ms.len());
        for m in &ms {
            let bar = match bars.last() {
                // A partial bar before an off-line change counts as a bar.
                Some(b) => b.bar + (m.tick.0 - b.tick + b.len() - 1).div_euclid(b.len()),
                None => 0,
            };
            bars.push(Bar {
                tick: m.tick.0,
                bar,
                sig: m.signature,
            });
        }
        TempoMap {
            tempo: pts,
            metres: ms,
            segs,
            bars,
        }
    }

    /// Whether the tempo never changes along the song (the metre may).
    pub fn is_constant(&self) -> bool {
        self.segs.len() == 1
    }

    /// Whether the metre never changes along the song.
    pub fn is_one_metre(&self) -> bool {
        self.bars.len() == 1
    }

    /// The tempo points, in order (the first at tick 0).
    pub fn points(&self) -> &[TempoPoint] {
        &self.tempo
    }

    /// The metre changes, in order (the first at tick 0).
    pub fn metres(&self) -> &[MetreChange] {
        &self.metres
    }

    /// The segment holding `tick` (the first for anything before tick 0).
    #[inline]
    fn seg_of(&self, tick: f64) -> usize {
        self.segs
            .partition_point(|s| s.tick <= tick)
            .saturating_sub(1)
    }

    /// The tempo at `tick` (inside a ramp, the ramp's value there).
    pub fn bpm_at(&self, tick: f64) -> f64 {
        self.segs[self.seg_of(tick)].bpm_at(tick)
    }

    /// The tempo held at `tick` and the ticks `[from, to)` it holds over (`from` is
    /// `-inf` for the first segment, `to` is `+inf` for the last) — `None` inside a ramp.
    /// Over such a stretch the old constant-tempo arithmetic is exact, and the engine keeps
    /// it there to the bit.
    pub fn held_at(&self, tick: f64) -> Option<(f64, f64, f64)> {
        let i = self.seg_of(tick);
        let s = &self.segs[i];
        if tick < s.tick {
            // Before tick 0: the first tempo holds backwards (up to a ramp's start).
            let to = if s.k == 0.0 {
                self.segs.get(1).map_or(f64::INFINITY, |n| n.tick)
            } else {
                s.tick
            };
            return Some((s.bpm, f64::NEG_INFINITY, to));
        }
        if s.k != 0.0 {
            return None;
        }
        let from = if i == 0 { f64::NEG_INFINITY } else { s.tick };
        let to = self.segs.get(i + 1).map_or(f64::INFINITY, |n| n.tick);
        Some((s.bpm, from, to))
    }

    /// The tick of the tempo point at or before `tick` (tick 0 for anything before it): where
    /// the segment holding `tick` starts.
    pub fn point_at_or_before(&self, tick: f64) -> f64 {
        self.segs[self.seg_of(tick)].tick
    }

    /// Whether `tick` lies inside a ramp (from its start, before its end).
    pub fn in_ramp(&self, tick: f64) -> bool {
        tick >= 0.0 && self.segs[self.seg_of(tick)].k != 0.0
    }

    /// The tempo's rate of change at `tick`, in BPM per tick (0 outside ramps).
    pub fn slope_at(&self, tick: f64) -> f64 {
        if tick < 0.0 {
            return 0.0;
        }
        self.segs[self.seg_of(tick)].k
    }

    /// Seconds from tick 0 to `tick` (negative before it).
    pub fn seconds_at(&self, tick: f64) -> f64 {
        let i = self.seg_of(tick);
        let s = &self.segs[i];
        let local = s.seconds(tick - s.tick);
        if i == 0 {
            local
        } else {
            s.sec + local
        }
    }

    /// The tick heard `seconds` after tick 0 (the inverse of [`TempoMap::seconds_at`]).
    pub fn tick_at_seconds(&self, seconds: f64) -> f64 {
        let i = self
            .segs
            .partition_point(|s| s.sec <= seconds)
            .saturating_sub(1);
        let s = &self.segs[i];
        if i == 0 {
            s.ticks(seconds)
        } else {
            s.tick + s.ticks(seconds - s.sec)
        }
    }

    /// Exact sample position of `tick` from tick 0 at `sample_rate` (fractions kept).
    pub fn sample_at(&self, tick: f64, sample_rate: f64) -> f64 {
        self.seconds_at(tick) * sample_rate
    }

    /// The tick at `sample` samples from tick 0 at `sample_rate`.
    pub fn tick_at_sample(&self, sample: f64, sample_rate: f64) -> f64 {
        self.tick_at_seconds(sample / sample_rate)
    }

    /// Seconds from `from` to `to` (negative when `to` is earlier). Inside one segment
    /// that holds its tempo, `(to − from).to_seconds(bpm)` to the bit.
    pub fn seconds_between(&self, from: Ticks, to: Ticks) -> f64 {
        let (a, b) = (from.0 as f64, to.0 as f64);
        let (i, j) = (self.seg_of(a), self.seg_of(b));
        if i == j && (self.segs[i].k == 0.0 || (a <= 0.0 && b <= 0.0)) {
            return (to - from).to_seconds(self.segs[i].bpm);
        }
        self.seconds_at(b) - self.seconds_at(a)
    }

    /// [`TempoMap::seconds_between`] for fractional ticks.
    pub fn seconds_between_f64(&self, from: f64, to: f64) -> f64 {
        let (i, j) = (self.seg_of(from), self.seg_of(to));
        if i == j && self.segs[i].k == 0.0 {
            return (to - from) / PPQ as f64 * 60.0 / self.segs[i].bpm;
        }
        self.seconds_at(to) - self.seconds_at(from)
    }

    /// The tick `seconds` after `from` (before it when negative). Inside one segment that
    /// holds its tempo, `from + seconds · bpm / 60 · PPQ`.
    pub fn tick_after_seconds(&self, from: f64, seconds: f64) -> f64 {
        let i = self.seg_of(from);
        let s = &self.segs[i];
        if s.k == 0.0 {
            let to = from + seconds * s.bpm / 60.0 * PPQ as f64;
            if self.seg_of(to) == i {
                return to;
            }
        }
        self.tick_at_seconds(self.seconds_at(from) + seconds)
    }

    /// The first tick after `tick` where the tempo stops being what it is at `tick` (a
    /// point, or a ramp's start or end) — where a pass that wants one tempo must end.
    pub fn next_change_after(&self, tick: f64) -> Option<f64> {
        let i = self.segs.partition_point(|s| s.tick <= tick);
        match self.segs.get(i) {
            // Before tick 0 a held first tempo goes on through 0: no change there (review 1).
            Some(s) if i == 0 && s.k == 0.0 => self.segs.get(1).map(|s| s.tick),
            other => other.map(|s| s.tick),
        }
    }

    /// The start of the tempo step holding `tick`: inside a ramp the last multiple of
    /// [`RAMP_STEP`] ticks from the ramp's start at or before it; elsewhere `tick` itself.
    /// The tempo a unit is told for a pass is [`TempoMap::bpm_at`] this tick.
    pub fn step_start(&self, tick: f64) -> f64 {
        let s = &self.segs[self.seg_of(tick)];
        if s.k == 0.0 || tick <= s.tick {
            return tick;
        }
        let n = ((tick - s.tick) / RAMP_STEP as f64).floor();
        s.tick + n * RAMP_STEP as f64
    }

    /// The first tempo step boundary after `tick`: the next point, or inside a ramp the
    /// next [`RAMP_STEP`] from the ramp's start — where a pass that tells units one tempo
    /// must end.
    pub fn next_step_after(&self, tick: f64) -> Option<f64> {
        let s = &self.segs[self.seg_of(tick)];
        let next = self.next_change_after(tick);
        if s.k == 0.0 || tick < s.tick {
            return next;
        }
        let n = ((tick - s.tick) / RAMP_STEP as f64).floor() + 1.0;
        let knot = s.tick + n * RAMP_STEP as f64;
        Some(next.map_or(knot, |x| x.min(knot)))
    }
}

impl TempoMap {
    /// A stable hash (FNV-1a, the same on every build) of everything in the map that
    /// decides timing between `from` and `to`: a render that depends on the tempo over a
    /// span (a warped clip, a frozen strip) keys itself on it. It is the tempo function
    /// over the span, measured from `from`: the tempo (and slope) in force at `from`, then
    /// each point inside the span by its distance from `from` — so a span that lies inside
    /// one held tempo keys on that tempo alone (moving it along the song keeps its key), and
    /// points outside the span never change it.
    pub fn span_key(&self, from: Ticks, to: Ticks) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |bits: u64| {
            for byte in bits.to_le_bytes() {
                h ^= byte as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        let a = from.0 as f64;
        let i = self.seg_of(a);
        let s = &self.segs[i];
        eat(s.bpm_at(a).to_bits());
        if s.k != 0.0 && a >= s.tick {
            eat(s.k.to_bits());
        }
        // Every segment starting inside the span — the first too, for a span that starts
        // before tick 0 (review 1).
        let next = if a < s.tick { i } else { i + 1 };
        for s in self.segs[next..]
            .iter()
            .take_while(|s| s.tick < to.0 as f64)
        {
            eat(((s.tick - a) as i64) as u64);
            eat(s.bpm.to_bits());
            eat(s.k.to_bits());
        }
        h
    }
}

impl Project {
    /// The project's tempo map, compiled from its tempo, its metre, its tempo track's
    /// points and its metre changes.
    pub fn tempo_map(&self) -> TempoMap {
        TempoMap::new(
            self.tempo_bpm,
            self.time_signature,
            &self.tempo_points,
            &self.metre_changes,
        )
    }

    /// Whether the project has a tempo track (points of its own), even one that holds.
    pub fn has_tempo_track(&self) -> bool {
        !self.tempo_points.is_empty()
    }

    /// The marker with this id.
    pub fn marker(&self, id: MarkerId) -> Option<&Marker> {
        self.markers.iter().find(|m| m.id == id)
    }

    /// The markers before and after `tick`: the last at or before it and the first after
    /// it (by position, then id) — what *loop between markers* and the marker jumps read.
    pub fn markers_around(&self, tick: Ticks) -> (Option<&Marker>, Option<&Marker>) {
        let before = self.markers.iter().rev().find(|m| m.tick <= tick);
        let after = self.markers.iter().find(|m| m.tick > tick);
        (before, after)
    }
}
