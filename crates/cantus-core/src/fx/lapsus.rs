//! Lapsus, the time shaper (ADR 0132, provisional): tape stop, half-time, stutter, reverse,
//! scratch and gates, drawn as two curves over a cycle synced to the song — its parameter
//! model and its bank of patterns. Owned by Lapsus; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: slot 1 of the
//! factory bank is *Through* (no delay, full level), and the mix at 100 % of a signal that
//! is its own input is that input.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `slot` | the pattern playing (automation switches it; notes may override it) | 1 … 36 | 1 (*Through*) |
//! | `mix` | the shaped signal against the dry | 0 … 100 % | 100 % |
//! | `xfade` | the crossfade where the time curve jumps (a repeat, the cycle's end) | 0 … 20 ms | 2 ms |
//! | `smooth` | the volume curve's glide, so a gate never clicks | 0 … 20 ms | 1 ms |
//! | `notes` | what a note does: nothing, switch the slot, or play it while held | off / switch / hold | hold |
//! | `retrig` | a note starts its pattern from the beginning, there | off / on | off |
//! | `key` | the note of slot 1 (36 notes up from it) | 0 … 92 | 48 (C3) |
//! | `ahead` | lookahead: the time curve may read the future this far (the latency it costs) | 0 … 250 ms | 0 |
//! | `out` | output gain | −24 … +24 dB | 0 |
//!
//! **The bank** is not parameters: thirty-six patterns, each a length and two drawn curves
//! (`EffectSpec::lapsus`, [`Bank`]). A spec with no bank plays the factory bank
//! ([`factory`]); a bank holds only the slots that differ from it. Patterns are content, as
//! a clip's notes are: a host sees the slot, not three thousand point coordinates.
//!
//! **The time curve's value is the delay** behind the live signal, in pattern lengths: 0 is
//! the signal as it arrives, a line falling at 45° is the tape stopped, half that slope is
//! half speed, steeper is reverse. **Nothing before the cycle's start is heard**: a curve
//! that reaches further back reads silence, so a cycle never depends on what played before
//! it — a render from bar 5 is bar 5 of the whole render, whatever the input.

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
/// pattern, as Gross Beat's keys do.
pub const TAKES_NOTES: bool = true;
/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files. What the exported plug-in shows a host for them
/// (ADR 0020, `cantus_plugins::params`).
pub const CHOICES: &[(&str, &str)] = &[("slot", "lapsus.slot"), ("notes", "lapsus.notes")];

/// Patterns in the bank (Gross Beat's thirty-six).
pub const SLOTS: usize = 36;
/// Points a curve may hold: a pencil's steps at 1/32 over two bars.
pub const MAX_POINTS: usize = 64;
/// A pattern's lengths, in quarter notes: ¼, ½, 1, 2 and 4 bars of 4/4.
pub const LENGTHS: [f64; 5] = [1.0, 2.0, 4.0, 8.0, 16.0];
/// The index of one bar in [`LENGTHS`].
pub const LEN_BAR: u8 = 2;
/// How far the time curve's value may go: ahead (with `ahead`) and back, in lengths.
pub const TIME_RANGE: (f32, f32) = (-1.0, 1.0);

/// What a note does (`notes`).
pub const NOTES_OFF: f32 = 0.0;
pub const NOTES_SWITCH: f32 = 1.0;
pub const NOTES_HOLD: f32 = 2.0;

pub const PARAMS: &[EffectParamInfo] = &[
    p("slot", "fx.lapsus.slot", 0.0, 35.0, 0.0, "", false, true),
    p("mix", "fx.lapsus.mix", 0.0, 1.0, 1.0, "%", false, false),
    p("xfade", "fx.lapsus.xfade", 0.0, 20.0, 2.0, "ms", false, false),
    p("smooth", "fx.lapsus.smooth", 0.0, 20.0, 1.0, "ms", false, false),
    p("notes", "fx.lapsus.notes", 0.0, 2.0, 2.0, "", false, true),
    p("retrig", "fx.lapsus.retrig", 0.0, 1.0, 0.0, "", false, true),
    p("key", "fx.lapsus.key", 0.0, 92.0, 48.0, "", false, true),
    p("ahead", "fx.lapsus.ahead", 0.0, 250.0, 0.0, "ms", false, false),
    p("out", "fx.lapsus.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

/// One point of a curve: where in the cycle (`x`, 0 … 1), the value (`y`: the delay in
/// pattern lengths for the time curve, the level 0 … 1 for the volume), and the segment
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

/// One pattern: its length (an index of [`LENGTHS`]) and its two curves. An empty curve is
/// flat: no delay, full level. Both wrap, as Fluctus's does: the last point runs to the
/// first one a cycle later.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub len: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub time: Vec<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vol: Vec<Point>,
    /// A name the user gave; `None` shows the factory pattern's name, or the slot's number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Pattern {
    /// Quarter notes in one cycle.
    pub fn beats(&self) -> f64 {
        LENGTHS[(self.len as usize).min(LENGTHS.len() - 1)]
    }

    /// Clamped into the model's ranges, with at most [`MAX_POINTS`] per curve: what a
    /// pattern from a file or a paste is made into before anything plays it.
    pub fn sanitized(&self) -> Pattern {
        let fix = |v: &[Point], lo: f32, hi: f32| -> Vec<Point> {
            v.iter()
                .take(MAX_POINTS)
                .map(|q| Point {
                    x: finite_or(q.x, 0.0).clamp(0.0, 1.0),
                    y: finite_or(q.y, 0.0).clamp(lo, hi),
                    t: finite_or(q.t, 0.0).clamp(-1.0, 1.0),
                    s: q.s.min(5),
                })
                .collect()
        };
        Pattern {
            len: self.len.min(LENGTHS.len() as u8 - 1),
            time: fix(&self.time, TIME_RANGE.0, TIME_RANGE.1),
            vol: fix(&self.vol, 0.0, 1.0),
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

/// The slots that differ from the factory bank, by slot (0 … 35). Empty: the factory bank.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bank {
    #[serde(default)]
    pub slots: BTreeMap<u8, Pattern>,
}

impl Bank {
    /// The pattern in `slot` (0 … 35): this bank's own, or the factory's.
    pub fn pattern(&self, slot: usize) -> &Pattern {
        let slot = slot.min(SLOTS - 1);
        self.slots
            .get(&(slot as u8))
            .unwrap_or_else(|| &factory()[slot])
    }
}

/// The pattern in `slot` of a spec's bank (the factory's where it has none).
pub fn pattern_of(bank: Option<&Bank>, slot: usize) -> &Pattern {
    match bank {
        Some(b) => b.pattern(slot),
        None => &factory()[slot.min(SLOTS - 1)],
    }
}

/// The factory bank: thirty-six patterns in six rows — stops and starts, speed, repeats,
/// reverse, scratch, volume. Forever once shipped, as a default is: a project that never
/// edited a slot plays these.
pub fn factory() -> &'static [Pattern; SLOTS] {
    static BANK: OnceLock<[Pattern; SLOTS]> = OnceLock::new();
    BANK.get_or_init(bank::factory)
}

/// The locale key of a factory pattern's name.
pub fn slot_name_key(slot: usize) -> String {
    format!("lapsus.slot.{slot}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The null rule: every default is the value that leaves the signal untouched.
    #[test]
    fn the_defaults_are_a_null() {
        let get = |k: &str| PARAMS.iter().find(|i| i.key == k).unwrap().default;
        assert_eq!(get("slot"), 0.0);
        assert_eq!(get("mix"), 1.0);
        assert_eq!(get("ahead"), 0.0);
        assert_eq!(get("out"), 0.0);
        let through = &factory()[0];
        assert!(through.time.is_empty() && through.vol.is_empty());
    }

    #[test]
    fn keys_are_unique_and_defaults_in_range() {
        for (i, a) in PARAMS.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            for b in &PARAMS[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
        assert!(PARAMS.iter().any(|i| i.key == "out"), "the contract's key stays");
    }

    #[test]
    fn the_factory_bank_is_within_the_model() {
        for (i, p) in factory().iter().enumerate() {
            assert_eq!(p, &p.sanitized(), "slot {i} is already what sanitizing makes it");
            assert!(p.time.len() <= MAX_POINTS && p.vol.len() <= MAX_POINTS);
            for c in [&p.time, &p.vol] {
                assert!(c.windows(2).all(|w| w[0].x <= w[1].x), "slot {i}: sorted");
            }
            // Within a bar, so a bank that is the factory's needs one bar of memory.
            assert!(p.beats() <= 4.0, "slot {i}");
        }
    }

    #[test]
    fn a_bank_holds_only_what_differs() {
        let mut b = Bank::default();
        assert_eq!(b.pattern(5), &factory()[5]);
        let mine = Pattern {
            len: 0,
            time: vec![Point::new(0.0, 0.5)],
            ..Pattern::default()
        };
        b.slots.insert(5, mine.clone());
        assert_eq!(b.pattern(5), &mine);
        assert_eq!(pattern_of(None, 99), &factory()[35]);
        let json = serde_json::to_string(&b).unwrap();
        let back: Bank = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }
}
