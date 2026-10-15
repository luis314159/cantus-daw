//! Anulus, the ring modulator (ADR 0055): its parameter model.
//! Owned by Anulus; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the mix starts at
//! 0; the carrier waits at A 440, a sine.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `freq` | the carrier's frequency | 1 … 5 000 Hz | 440 Hz |
//! | `wave` | sine, triangle, square, band-limited (`cantus_dsp::anulus::Wave`) | 0 … 2 | sine |
//! | `lfo` | the LFO's swing of the frequency either way | 0 … 24 st | 0 |
//! | `sync` | the LFO follows the song (`div`) or runs free (`rate`) | 0 / 1 | free |
//! | `div` | the synced LFO | 8/1 … 1/32 | 1/1 |
//! | `rate` | the free LFO | 0.02 … 20 Hz | 0.5 Hz |
//! | `shape` | the LFO's shape (the family's six) | 0 … 5 | sine |
//! | `spread` | the right carrier's phase ahead of the left | 0 … 180° | 0 |
//! | `mix` | equal-gain crossfade; 100 % is the product alone | 0 … 100 % | 0 |
//! | `out` | output gain | −24 … +24 dB | 0 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.anulus";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022). Not this
/// round: the carrier is set by its frequency (shown as a note), not by played notes.
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key (`modfx.*`: the family's
/// words, kept in Tremor's locale files).
pub const CHOICES: &[(&str, &str)] = &[
    ("wave", "anulus.wave"),
    ("sync", "modfx.sync"),
    ("div", "modfx.div"),
    ("shape", "modfx.shape"),
];

pub const PARAMS: &[EffectParamInfo] = &[
    p(
        "freq",
        "fx.anulus.freq",
        1.0,
        5_000.0,
        440.0,
        "Hz",
        true,
        false,
    ),
    p("wave", "fx.anulus.wave", 0.0, 2.0, 0.0, "", false, true),
    p("lfo", "fx.anulus.lfo", 0.0, 24.0, 0.0, "st", false, false),
    p("sync", "modfx.p.sync", 0.0, 1.0, 0.0, "", false, true),
    p("div", "modfx.p.div", 0.0, 16.0, 3.0, "", false, true),
    p("rate", "modfx.p.rate", 0.02, 20.0, 0.5, "Hz", true, false),
    p("shape", "modfx.p.shape", 0.0, 5.0, 0.0, "", false, true),
    p(
        "spread",
        "modfx.p.spread",
        0.0,
        180.0,
        0.0,
        "\u{b0}",
        false,
        false,
    ),
    p("mix", "fx.anulus.mix", 0.0, 1.0, 0.0, "%", false, false),
    p("out", "fx.anulus.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}
