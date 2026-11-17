//! Lapsus, the time shaper (ADR 0129): tape stop, half-time, stutter, reverse,
//! scratch and gates, drawn as curves over cycles synced to the song — its parameter model
//! and its two banks of patterns. Owned by Lapsus; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: slot 1 of each
//! factory bank is *Through* (no delay; full level), and the mix at 100 % of a signal that
//! is its own input is that input.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `tslot` | the **time** pattern playing (automation switches it; notes may override it) | 1 … 36 | 1 (*Through*) |
//! | `vslot` | the **volume** pattern playing, chosen apart from the time one | 1 … 36 | 1 (*Through*) |
//! | `mix` | the shaped signal against the dry | 0 … 100 % | 100 % |
//! | `xfade` | the crossfade where the time curve jumps (a repeat, the cycle's end) | 0 … 20 ms | 2 ms |
//! | `smooth` | the volume curve's glide, so a gate never clicks | 0 … 20 ms | 1 ms |
//! | `notes` | what a note does: nothing, switch the pattern, or play it while held | off / switch / hold | hold |
//! | `retrig` | a note starts its pattern from the beginning, there | off / on | off |
//! | `tkey` | the note of time pattern 1 (36 notes up from it) | 0 … 92 | 60 (C4) |
//! | `vkey` | the note of volume pattern 1 | 0 … 92 | 24 (C1) |
//! | `ahead` | lookahead: the time curve may read the future this far (the latency it costs) | 0 … 250 ms | 0 |
//! | `out` | output gain | −24 … +24 dB | 0 |
//!
//! **The banks are content, not parameters** (as Gross Beat's): thirty-six **time** patterns
//! and thirty-six **volume** patterns, each a length and a drawn curve, played in any
//! combination (`EffectSpec::lapsus`, [`Bank`]). A spec with no bank plays the factory banks
//! ([`factory`]); a bank holds only the slots that differ from them. A host sees eleven
//! parameters, not six thousand point coordinates.
//!
//! **A time curve's value is the delay** behind the live signal, in pattern lengths: 0 is
//! the signal as it arrives, a line falling at 45° is the tape stopped, half that slope is
//! half speed, steeper is reverse. **Nothing before the cycle's start is heard**: a curve
//! that reaches further back reads silence, so a cycle never depends on what played before
//! it — a render from bar 5 is bar 5 of the whole render, whatever the input. A volume
//! curve's value is the level, 0 … 1.

use crate::effect::{p, EffectParamInfo};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[path = "lapsus_bank.rs"]
mod bank;

pub const NAME_KEY: &str = "effect.lapsus";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// It hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022): they choose the
/// patterns, as Gross Beat's keys do — a range of 36 notes for each bank.
pub const TAKES_NOTES: bool = true;
/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`).
pub const CHOICES: &[(&str, &str)] = &[
    ("tslot", "lapsus.tslot"),
    ("vslot", "lapsus.vslot"),
    ("notes", "lapsus.notes"),
];

/// Patterns in each bank (Gross Beat's thirty-six).
pub const SLOTS: usize = 36;
/// Points a curve may hold: a pencil's steps at 1/32 over two bars.
pub const MAX_POINTS: usize = 64;
/// A pattern's lengths, in quarter notes: ¼, ½, 1, 2 and 4 bars of 4/4.
pub const LENGTHS: [f64; 5] = [1.0, 2.0, 4.0, 8.0, 16.0];
/// The index of one bar in [`LENGTHS`].
pub const LEN_BAR: u8 = 2;
/// How far a time curve's value may go: ahead (with `ahead`) and back, in lengths.
pub const TIME_RANGE: (f32, f32) = (-1.0, 1.0);

/// What a note does (`notes`).
pub const NOTES_OFF: f32 = 0.0;
pub const NOTES_SWITCH: f32 = 1.0;
pub const NOTES_HOLD: f32 = 2.0;

pub const PARAMS: &[EffectParamInfo] = &[
    p("tslot", "fx.lapsus.tslot", 0.0, 35.0, 0.0, "", false, true),
    p("vslot", "fx.lapsus.vslot", 0.0, 35.0, 0.0, "", false, true),
    p("mix", "fx.lapsus.mix", 0.0, 1.0, 1.0, "%", false, false),
    p(
        "xfade",
        "fx.lapsus.xfade",
        0.0,
        20.0,
        2.0,
        "ms",
        false,
        false,
    ),
    p(
        "smooth",
        "fx.lapsus.smooth",
        0.0,
        20.0,
        1.0,
        "ms",
        false,
        false,
    ),
    p("notes", "fx.lapsus.notes", 0.0, 2.0, 2.0, "", false, true),
    p("retrig", "fx.lapsus.retrig", 0.0, 1.0, 0.0, "", false, true),
    p("tkey", "fx.lapsus.tkey", 0.0, 92.0, 60.0, "", false, true),
    p("vkey", "fx.lapsus.vkey", 0.0, 92.0, 24.0, "", false, true),
    p(
        "ahead",
        "fx.lapsus.ahead",
        0.0,
        250.0,
        0.0,
        "ms",
        false,
        false,
    ),
    p("out", "fx.lapsus.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

/// The two banks.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lane {
    /// Where the music is read from.
    Time,
    /// How loud it is.
    Vol,
}

impl Lane {
    pub const ALL: [Lane; 2] = [Lane::Time, Lane::Vol];

    /// The range a point's value may take.
    pub fn range(self) -> (f32, f32) {
        match self {
            Lane::Time => TIME_RANGE,
            Lane::Vol => (0.0, 1.0),
        }
    }

    /// The parameter that chooses the lane's pattern, and the one that names its first note.
    pub fn keys(self) -> (&'static str, &'static str) {
        match self {
            Lane::Time => ("tslot", "tkey"),
            Lane::Vol => ("vslot", "vkey"),
        }
    }
}

/// One point of a curve: where in the cycle (`x`, 0 … 1), the value (`y`: the delay in
/// pattern lengths for a time curve, the level 0 … 1 for a volume curve), and the segment
/// that starts here — its tension (`t`, −1 … 1) and its shape (`s`, the automation
/// editor's six: curve, S-curve, hold, stairs, pulse, wave). Fluctus's model (ADR 0055).
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub t: f32,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub s: u8,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Point {
        Point { x, y, t: 0.0, s: 0 }
    }
    /// The segment from here bends with `t`.
    pub const fn bend(self, t: f32) -> Point {
        Point { t, ..self }
    }
    /// The segment from here has shape `s` (2: hold).
    pub const fn shape(self, s: u8) -> Point {
        Point { s, ..self }
    }
}

/// The shapes a segment may take, by index (the automation editor's `Shape::ALL`).
pub const SHAPE_CURVE: u8 = 0;
pub const SHAPE_S: u8 = 1;
pub const SHAPE_HOLD: u8 = 2;

/// One pattern: its length (an index of [`LENGTHS`]) and its curve. An empty curve is
/// flat: no delay, or full level. The curve wraps, as Fluctus's does: the last point runs
/// to the first one a cycle later.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub len: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<Point>,
    /// A name the user gave; `None` shows the factory pattern's name, or the slot's number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Pattern {
    /// Quarter notes in one cycle.
    pub fn beats(&self) -> f64 {
        LENGTHS[(self.len as usize).min(LENGTHS.len() - 1)]
    }

    /// Clamped into `lane`'s ranges, with at most [`MAX_POINTS`] points, sorted by place:
    /// what a pattern from a file or a paste is made into before anything plays it.
    pub fn sanitized(&self, lane: Lane) -> Pattern {
        let (lo, hi) = lane.range();
        let mut points: Vec<Point> = self
            .points
            .iter()
            .take(MAX_POINTS)
            .map(|q| Point {
                x: finite_or(q.x, 0.0).clamp(0.0, 1.0),
                y: finite_or(q.y, 0.0).clamp(lo, hi),
                t: finite_or(q.t, 0.0).clamp(-1.0, 1.0),
                s: q.s.min(5),
            })
            .collect();
        points.sort_by(|a, b| a.x.total_cmp(&b.x));
        Pattern {
            len: self.len.min(LENGTHS.len() as u8 - 1),
            points,
            name: self.name.clone(),
        }
    }
}

fn finite_or(v: f32, d: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        d
    }
}

/// The slots that differ from the factory banks, by bank and slot (0 … 35). Empty: the
/// factory banks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bank {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub time: BTreeMap<u8, Pattern>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vol: BTreeMap<u8, Pattern>,
}

impl Bank {
    /// One bank's own slots.
    pub fn lane(&self, lane: Lane) -> &BTreeMap<u8, Pattern> {
        match lane {
            Lane::Time => &self.time,
            Lane::Vol => &self.vol,
        }
    }

    pub fn lane_mut(&mut self, lane: Lane) -> &mut BTreeMap<u8, Pattern> {
        match lane {
            Lane::Time => &mut self.time,
            Lane::Vol => &mut self.vol,
        }
    }

    /// The pattern in `slot` (0 … 35) of `lane`: this bank's own, or the factory's.
    pub fn pattern(&self, lane: Lane, slot: usize) -> &Pattern {
        let slot = slot.min(SLOTS - 1);
        self.lane(lane)
            .get(&(slot as u8))
            .unwrap_or_else(|| &factory(lane)[slot])
    }

    /// Nothing differs from the factory.
    pub fn is_empty(&self) -> bool {
        self.time.is_empty() && self.vol.is_empty()
    }
}

/// The pattern in `slot` of `lane` of a spec's bank (the factory's where it has none).
pub fn pattern_of(bank: Option<&Bank>, lane: Lane, slot: usize) -> &Pattern {
    match bank {
        Some(b) => b.pattern(lane, slot),
        None => &factory(lane)[slot.min(SLOTS - 1)],
    }
}

/// A factory bank: thirty-six patterns in six rows. Forever once shipped, as a default
/// is: a project that never edited a slot plays these.
pub fn factory(lane: Lane) -> &'static [Pattern; SLOTS] {
    static TIME: OnceLock<[Pattern; SLOTS]> = OnceLock::new();
    static VOL: OnceLock<[Pattern; SLOTS]> = OnceLock::new();
    match lane {
        Lane::Time => TIME.get_or_init(bank::time),
        Lane::Vol => VOL.get_or_init(bank::vol),
    }
}

/// The locale key of a factory pattern's name.
pub fn slot_name_key(lane: Lane, slot: usize) -> String {
    match lane {
        Lane::Time => format!("lapsus.tslot.{slot}"),
        Lane::Vol => format!("lapsus.vslot.{slot}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The null rule: every default is the value that leaves the signal untouched.
    #[test]
    fn the_defaults_are_a_null() {
        let get = |k: &str| PARAMS.iter().find(|i| i.key == k).unwrap().default;
        assert_eq!((get("tslot"), get("vslot")), (0.0, 0.0));
        assert_eq!(get("mix"), 1.0);
        assert_eq!(get("ahead"), 0.0);
        assert_eq!(get("out"), 0.0);
        for lane in Lane::ALL {
            assert!(
                factory(lane)[0].points.is_empty(),
                "{lane:?} slot 1 is Through"
            );
        }
    }

    #[test]
    fn keys_are_unique_and_defaults_in_range() {
        for (i, a) in PARAMS.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            for b in &PARAMS[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
        assert!(
            PARAMS.iter().any(|i| i.key == "out"),
            "the contract's key stays"
        );
    }

    #[test]
    fn the_two_note_ranges_do_not_overlap_by_default() {
        let get = |k: &str| PARAMS.iter().find(|i| i.key == k).unwrap().default as i32;
        let (t, v) = (get("tkey"), get("vkey"));
        assert!(v + SLOTS as i32 <= t || t + SLOTS as i32 <= v);
    }

    #[test]
    fn the_factory_banks_are_within_the_model() {
        for lane in Lane::ALL {
            for (i, p) in factory(lane).iter().enumerate() {
                assert_eq!(
                    p,
                    &p.sanitized(lane),
                    "{lane:?} {i}: as sanitizing makes it"
                );
                // Within a bar, so a bank that is the factory's needs one bar of memory.
                assert!(p.beats() <= 4.0, "{lane:?} {i}");
            }
            // Every pattern but Through moves something.
            for (i, p) in factory(lane).iter().enumerate().skip(1) {
                let flat = match lane {
                    Lane::Time => p.points.iter().all(|q| q.y == 0.0),
                    Lane::Vol => p.points.iter().all(|q| q.y == 1.0),
                };
                assert!(!flat, "{lane:?} {i} changes the sound");
            }
        }
    }

    #[test]
    fn a_bank_holds_only_what_differs() {
        let mut b = Bank::default();
        assert_eq!(b.pattern(Lane::Time, 5), &factory(Lane::Time)[5]);
        let mine = Pattern {
            len: 0,
            points: vec![Point::new(0.0, 0.5)],
            name: None,
        };
        b.vol.insert(5, mine.clone());
        assert_eq!(b.pattern(Lane::Vol, 5), &mine);
        assert_eq!(b.pattern(Lane::Time, 5), &factory(Lane::Time)[5]);
        assert_eq!(pattern_of(None, Lane::Vol, 99), &factory(Lane::Vol)[35]);
        let json = serde_json::to_string(&b).unwrap();
        let back: Bank = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }
}
