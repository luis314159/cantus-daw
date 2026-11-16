//! Fractus, the glitch sequencer and grain cloud (ADR 0140, provisional): its parameter
//! model and its painted lanes. Owned by the wave-A builder; the DSP is
//! `cantus_dsp::fractus`, whose report (`docs/round-fx/fractus.md`) says why it is built as
//! it is.
//!
//! Keys are forever once shipped. **Every default leaves the signal untouched, to the bit**:
//! nothing painted and the cloud's level at 0 — the output is the input.
//!
//! **The lanes** ([`Pattern`], [`crate::EffectSpec::fractus`]): ten, one per module, in
//! the DSP's order (repeat, reverse, tape stop, shuffle, pitch jump, crush, filter sweep,
//! gate, throw, cloud). A lane is a list of **blocks** — a start step and the steps it
//! holds, each with its amount (the module's mix for that event) and its probability —
//! sorted, apart, inside the 256 steps of the longest cycle. Only the lanes that hold a
//! block are written; an empty pattern is `None` and not written at all.
//! `Command::SetFractusLane` replaces one lane, its inverse the lane it replaced.
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `step`, `bars`, `swing` | the grid: 1/4 … 1/64 (triplets too), 1 … 4 bars, odd steps late | 0 … 6, 1 … 4, 0 … 100 % |
//! | `xfade` | the joins | 0.5 … 10 ms |
//! | `trig`, `quant`, `key` | painted steps, held notes or both; a note waits for the next line; the first lane's key | 0 … 2, 0 … 1, 0 … 127 |
//! | `seed` | every roll's seed | 1 … 9 999 |
//! | `mix`, `out` | the processed signal against the dry; the output level | 0 … 100 %, −24 … +24 dB |
//! | `rsize`, `rramp`, `rpitch`, `rdecay`, `rduty` | repeat: the slice, the stutter build's halvings, pitch and level a repeat, how much sounds | note, 0 … 3, ±12 st, −24 … 0 dB, 5 … 100 % |
//! | `vslice` | reverse: the slice (0: the grid's step) | 0 … 14 |
//! | `tkind`, `ttime`, `tcurve` | tape: stop or start, its time (0: the event), its curve | 0 … 1, 0 … 14, 0 … 100 % |
//! | `sslice`, `srange`, `srev` | shuffle: the slice, how far back, the chance of a reversed slice | note, 1 … 8, 0 … 100 % |
//! | `psemi`, `pwin`, `pstyle` | pitch jump: the interval, the window, clean or the classic warble | ±24 st, 20 … 100 ms, 0 … 1 |
//! | `kbits`, `krate` | crush: bits and the held rate | 1 … 24, 100 Hz … 48 kHz |
//! | `fkind`, `ffrom`, `fto`, `fres`, `fcurve` | filter sweep: low, high or band pass, from and to, resonance, bend | 0 … 2, 20 Hz … 20 kHz, 0 … 100 %, −1 … 1 |
//! | `grate`, `gduty`, `gdepth` | gate: the pulse (0: shut for the event), open share, depth | 0 … 14, 5 … 100 %, 0 … 100 % |
//! | `wtime`, `wfb`, `wtone`, `wpp`, `wlvl` | throw: the echo's time, feedback, tone, ping-pong, level | note, 0 … 95 %, 200 Hz … 20 kHz, 0 … 1, 0 … 100 % |
//! | `clevel`, `cpos`, `cspray`, `csize`, `cdens` | cloud: level beside the chain (0: off), how far back, spray, a grain's size, grains a second | 0 … 100 %, 0 … 2 000 ms, 0 … 1 000 ms, 5 … 500 ms, 1 … 200 Hz |
//! | `cpitch`, `crand`, `cscale`, `cspread`, `crev`, `cwin`, `cfreeze` | cloud: transposition, random, scale, spread, reversed grains, window, freeze | ±24 st, 0 … 24 st, 0 … 6, 0 … 100 %, 0 … 100 %, 0 … 4, 0 … 1 |
//!
//! "note" is an index into the DSP's fourteen note values (1/2 … 1/128,
//! `cantus_dsp::fractus::NOTE_BEATS`); the `vslice`, `ttime` and `grate` indices start at 1
//! for those values and keep 0 for their own meaning.

use crate::effect::{p, EffectParamInfo};
use serde::{Deserialize, Serialize};

pub const NAME_KEY: &str = "effect.fractus";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Held notes play the modules (`trig`): the notes of the track the slot listens to.
pub const TAKES_NOTES: bool = true;

/// Lanes, one per module, in the DSP's order.
pub const LANES: usize = 10;
/// Steps in the longest cycle (4 bars of 1/64).
pub const MAX_STEPS: usize = 256;
/// The modules' keys, in lane order (`fx.fractus.lane.{key}` names them).
pub const LANE_KEYS: [&str; LANES] = [
    "repeat", "reverse", "tape", "shuffle", "pitch", "crush", "filter", "gate", "throw", "cloud",
];

/// The locale prefix of each stepped parameter's choices, by key (ADR 0020).
pub const CHOICES: &[(&str, &str)] = &[
    ("step", "fx.fractus.step"),
    ("trig", "fx.fractus.trig"),
    ("quant", "fx.fractus.quant"),
    ("rsize", "fx.fractus.note"),
    ("vslice", "fx.fractus.slice"),
    ("tkind", "fx.fractus.tkind"),
    ("ttime", "fx.fractus.ttime"),
    ("sslice", "fx.fractus.note"),
    ("pstyle", "fx.fractus.pstyle"),
    ("fkind", "fx.fractus.fkind"),
    ("grate", "fx.fractus.grate"),
    ("wtime", "fx.fractus.note"),
    ("wpp", "fx.fractus.wpp"),
    ("cscale", "fx.fractus.cscale"),
    ("cwin", "fx.fractus.cwin"),
    ("cfreeze", "fx.fractus.cfreeze"),
];

const fn amount(key: &'static str, label: &'static str, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, 1.0, default, "%", false, false)
}

const fn stepped(
    key: &'static str,
    label: &'static str,
    lo: f32,
    hi: f32,
    d: f32,
) -> EffectParamInfo {
    p(key, label, lo, hi, d, "", false, true)
}

const fn ms(key: &'static str, label: &'static str, lo: f32, hi: f32, d: f32) -> EffectParamInfo {
    p(key, label, lo, hi, d, "ms", true, false)
}

const fn hz(key: &'static str, label: &'static str, lo: f32, hi: f32, d: f32) -> EffectParamInfo {
    p(key, label, lo, hi, d, "Hz", true, false)
}

/// The last note value's index (`NOTE_BEATS`: fourteen).
const NOTE_MAX: f32 = 13.0;

pub const PARAMS: &[EffectParamInfo] = &[
    // The grid and the whole first: a host's list and the panel's first row.
    stepped("step", "fx.fractus.step", 0.0, 6.0, 3.0),
    stepped("bars", "fx.fractus.bars", 1.0, 4.0, 1.0),
    amount("swing", "fx.fractus.swing", 0.0),
    amount("mix", "fx.fractus.mix", 1.0),
    p(
        "out",
        "fx.fractus.out",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
    amount("clevel", "fx.fractus.clevel", 0.0),
    stepped("trig", "fx.fractus.trig", 0.0, 2.0, 0.0),
    stepped("quant", "fx.fractus.quant", 0.0, 1.0, 1.0),
    stepped("key", "fx.fractus.key", 0.0, 127.0, 48.0),
    stepped("seed", "fx.fractus.seed", 1.0, 9_999.0, 1.0),
    p(
        "xfade",
        "fx.fractus.xfade",
        0.5,
        10.0,
        2.0,
        "ms",
        false,
        false,
    ),
    // Repeat (1/32).
    stepped("rsize", "fx.fractus.rsize", 0.0, NOTE_MAX, 10.0),
    stepped("rramp", "fx.fractus.rramp", 0.0, 3.0, 0.0),
    p(
        "rpitch",
        "fx.fractus.rpitch",
        -12.0,
        12.0,
        0.0,
        "st",
        false,
        false,
    ),
    p(
        "rdecay",
        "fx.fractus.rdecay",
        -24.0,
        0.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "rduty",
        "fx.fractus.rduty",
        0.05,
        1.0,
        1.0,
        "%",
        false,
        false,
    ),
    // Reverse (the grid's step).
    stepped("vslice", "fx.fractus.vslice", 0.0, NOTE_MAX + 1.0, 0.0),
    // Tape.
    stepped("tkind", "fx.fractus.tkind", 0.0, 1.0, 0.0),
    stepped("ttime", "fx.fractus.ttime", 0.0, NOTE_MAX + 1.0, 0.0),
    amount("tcurve", "fx.fractus.tcurve", 0.5),
    // Shuffle (1/16).
    stepped("sslice", "fx.fractus.sslice", 0.0, NOTE_MAX, 8.0),
    stepped("srange", "fx.fractus.srange", 1.0, 8.0, 4.0),
    amount("srev", "fx.fractus.srev", 0.0),
    // Pitch jump.
    p(
        "psemi",
        "fx.fractus.psemi",
        -24.0,
        24.0,
        12.0,
        "st",
        false,
        true,
    ),
    p(
        "pwin",
        "fx.fractus.pwin",
        20.0,
        100.0,
        50.0,
        "ms",
        false,
        false,
    ),
    stepped("pstyle", "fx.fractus.pstyle", 0.0, 1.0, 0.0),
    // Crush.
    stepped("kbits", "fx.fractus.kbits", 1.0, 24.0, 6.0),
    hz("krate", "fx.fractus.krate", 100.0, 48_000.0, 6_000.0),
    // Filter sweep.
    stepped("fkind", "fx.fractus.fkind", 0.0, 2.0, 0.0),
    hz("ffrom", "fx.fractus.ffrom", 20.0, 20_000.0, 16_000.0),
    hz("fto", "fx.fractus.fto", 20.0, 20_000.0, 300.0),
    amount("fres", "fx.fractus.fres", 0.4),
    p(
        "fcurve",
        "fx.fractus.fcurve",
        -1.0,
        1.0,
        0.0,
        "",
        false,
        false,
    ),
    // Gate (1/32: index 10 + 1).
    stepped("grate", "fx.fractus.grate", 0.0, NOTE_MAX + 1.0, 11.0),
    p(
        "gduty",
        "fx.fractus.gduty",
        0.05,
        1.0,
        0.5,
        "%",
        false,
        false,
    ),
    amount("gdepth", "fx.fractus.gdepth", 1.0),
    // Throw (1/8 dotted).
    stepped("wtime", "fx.fractus.wtime", 0.0, NOTE_MAX, 3.0),
    p("wfb", "fx.fractus.wfb", 0.0, 0.95, 0.45, "%", false, false),
    hz("wtone", "fx.fractus.wtone", 200.0, 20_000.0, 5_000.0),
    stepped("wpp", "fx.fractus.wpp", 0.0, 1.0, 1.0),
    amount("wlvl", "fx.fractus.wlvl", 0.8),
    // The cloud.
    p(
        "cpos",
        "fx.fractus.cpos",
        0.0,
        2_000.0,
        120.0,
        "ms",
        false,
        false,
    ),
    p(
        "cspray",
        "fx.fractus.cspray",
        0.0,
        1_000.0,
        60.0,
        "ms",
        false,
        false,
    ),
    ms("csize", "fx.fractus.csize", 5.0, 500.0, 90.0),
    hz("cdens", "fx.fractus.cdens", 1.0, 200.0, 24.0),
    p(
        "cpitch",
        "fx.fractus.cpitch",
        -24.0,
        24.0,
        0.0,
        "st",
        false,
        false,
    ),
    p(
        "crand",
        "fx.fractus.crand",
        0.0,
        24.0,
        0.0,
        "st",
        false,
        false,
    ),
    stepped("cscale", "fx.fractus.cscale", 0.0, 6.0, 1.0),
    amount("cspread", "fx.fractus.cspread", 0.5),
    amount("crev", "fx.fractus.crev", 0.0),
    stepped("cwin", "fx.fractus.cwin", 0.0, 4.0, 0.0),
    stepped("cfreeze", "fx.fractus.cfreeze", 0.0, 1.0, 0.0),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

/// One painted event on a lane: a start at `at` and the steps it holds, `len` in all.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub at: u16,
    pub len: u16,
    /// The module's mix for the event, 0 … 1.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub amount: f32,
    /// The chance the event plays, 0 … 1.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub prob: f32,
}

fn one() -> f32 {
    1.0
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

impl Block {
    pub fn new(at: u16, len: u16) -> Block {
        Block {
            at,
            len,
            amount: 1.0,
            prob: 1.0,
        }
    }

    /// The step after its last.
    pub fn end(&self) -> usize {
        usize::from(self.at) + usize::from(self.len)
    }
}

/// The lanes that hold blocks, by lane index (0 … 9).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub lanes: std::collections::BTreeMap<u8, Vec<Block>>,
}

/// `blocks` as a lane may hold them: inside the cycle's 256 steps, at least a step long,
/// amounts and probabilities in 0 … 1 (a value that is not a number is 1), sorted by
/// start, and a block that overlaps the one before cut to begin where it ends (dropped
/// when nothing is left of it).
pub fn sanitize(blocks: &[Block]) -> Vec<Block> {
    let mut v: Vec<Block> = blocks
        .iter()
        .filter(|b| usize::from(b.at) < MAX_STEPS && b.len > 0)
        .map(|b| Block {
            at: b.at,
            len: b.len.min((MAX_STEPS - usize::from(b.at)) as u16),
            amount: unit(b.amount),
            prob: unit(b.prob),
        })
        .collect();
    v.sort_by_key(|b| b.at);
    let mut out: Vec<Block> = Vec::with_capacity(v.len());
    for mut b in v {
        if let Some(last) = out.last() {
            let end = last.end();
            if usize::from(b.at) < end {
                let cut = end - usize::from(b.at);
                if cut >= usize::from(b.len) {
                    continue;
                }
                b.len -= cut as u16;
                b.at = end as u16;
            }
        }
        out.push(b);
    }
    out
}

fn unit(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

impl Pattern {
    /// Lane `lane`'s blocks (empty when it holds none).
    pub fn lane(&self, lane: usize) -> &[Block] {
        u8::try_from(lane)
            .ok()
            .and_then(|l| self.lanes.get(&l))
            .map_or(&[], Vec::as_slice)
    }

    pub fn is_empty(&self) -> bool {
        self.lanes.values().all(Vec::is_empty)
    }

    /// Replace lane `lane` (sanitized; an empty lane is not kept) and return what it held.
    pub fn replace(&mut self, lane: u8, blocks: &[Block]) -> Vec<Block> {
        let clean = sanitize(blocks);
        let old = if clean.is_empty() {
            self.lanes.remove(&lane)
        } else {
            self.lanes.insert(lane, clean)
        };
        old.unwrap_or_default()
    }

    /// Every lane sanitized, the empty and the unknown dropped: what a hand-edited file
    /// becomes on load.
    pub fn sanitized(&self) -> Pattern {
        Pattern {
            lanes: self
                .lanes
                .iter()
                .filter(|(l, _)| usize::from(**l) < LANES)
                .map(|(l, b)| (*l, sanitize(b)))
                .filter(|(_, b)| !b.is_empty())
                .collect(),
        }
    }
}

/// Patterns a command would refuse (a hand edit), repaired on load: a pattern on a slot
/// that is not a Fractus taken off, every lane sanitized, an empty pattern `None`. How many
/// were changed. Never to the audio thread.
pub fn repair_project(project: &mut crate::model::Project) -> usize {
    let mut changed = 0;
    for e in project.all_effects_mut() {
        if e.spec.kind != crate::EffectKind::Glitch {
            if e.spec.fractus.take().is_some() {
                changed += 1;
            }
            continue;
        }
        if let Some(p) = e.spec.fractus.as_deref() {
            let clean = p.sanitized();
            if clean != *p {
                changed += 1;
            }
            e.spec.fractus = (!clean.is_empty()).then(|| Box::new(clean));
        }
    }
    changed
}

/// The starting spec (ADR 0071): the last beat of each bar of sixteenths repeats a
/// thirty-second, the rest of the bar untouched.
pub fn last_beat_stutter() -> crate::EffectSpec {
    let mut spec = crate::EffectSpec::new(crate::EffectKind::Glitch);
    let mut p = Pattern::default();
    p.replace(0, &[Block::new(12, 4)]);
    spec.fractus = Some(Box::new(p));
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_defaults_in_range_and_choices_name_stepped_keys() {
        for (i, a) in PARAMS.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            for b in &PARAMS[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
        for (k, _) in CHOICES {
            let info = PARAMS.iter().find(|p| p.key == *k).expect(k);
            assert!(info.stepped, "{k}");
        }
        let cl = PARAMS.iter().find(|p| p.key == "clevel").unwrap();
        assert_eq!(cl.default, 0.0, "the null: no cloud");
    }

    #[test]
    fn a_lane_is_sanitized_into_sorted_apart_blocks() {
        let b = |at, len| Block::new(at, len);
        let mut wild = b(10, 4);
        wild.amount = f32::NAN;
        wild.prob = 3.0;
        let s = sanitize(&[b(4, 8), wild, b(0, 0), b(300, 2), b(250, 20), b(5, 2)]);
        // (4, 8) covers 4..12; (5, 2) inside it is dropped; (10, 4) is cut to 12..14.
        assert_eq!(s.len(), 3, "{s:?}");
        assert_eq!((s[0].at, s[0].len), (4, 8));
        assert_eq!(
            (s[1].at, s[1].len, s[1].amount, s[1].prob),
            (12, 2, 1.0, 1.0)
        );
        assert_eq!((s[2].at, s[2].len), (250, 6));
        let mut p = Pattern::default();
        assert!(p.replace(3, &[b(0, 4)]).is_empty());
        assert_eq!(p.replace(3, &[]), vec![b(0, 4)]);
        assert!(p.is_empty() && p.lanes.is_empty());
        let json = serde_json::to_string(&Pattern {
            lanes: [(2, vec![b(1, 2)])].into_iter().collect(),
        })
        .unwrap();
        assert_eq!(json, r#"{"lanes":{"2":[{"at":1,"len":2}]}}"#);
    }
}
