//! Flexus, the through-zero flanger (ADR 0055): its parameter model.
//! Owned by Flexus; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the mix starts at
//! 0; the rest waits, set to a slow classic sweep, for the mix to be turned up.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `mode` | what moves the sweep: LFO, envelope, manual (`cantus_dsp::flexus::Mode`) | 0 … 2 | LFO |
//! | `tz` | through zero: the dry delayed by 5 ms (the latency), the wet crossing it | 0 / 1 | off |
//! | `mix` | equal-gain crossfade, both full at 50 % | 0 … 100 % | 0 |
//! | `delay` | the sweep's shortest delay (plain mode) | 0.05 … 5 ms | 0.5 ms |
//! | `depth` | how far the sweep reaches | 0 … 10 ms | 4 ms |
//! | `fb` | feedback, either polarity | −95 … 95 % | 0 |
//! | `invert` | the wet upside down | 0 / 1 | off |
//! | `sync` | the LFO follows the song (`div`) or runs free (`rate`) | 0 / 1 | free |
//! | `div` | the synced sweep | 8/1 … 1/32 | 2/1 |
//! | `rate` | the free sweep | 0.02 … 10 Hz | 0.25 Hz |
//! | `shape` | sine or triangle | 0 / 1 | triangle |
//! | `spread` | the right channel's LFO ahead of the left | 0 … 180° | 0 |
//! | `manual` | the sweep's position in manual mode | 0 … 100 % | 50 % |
//! | `att`, `rel` | the envelope's attack and release | 0.5 … 100 ms, 10 … 2 000 ms | 5, 200 ms |
//! | `sens` | the envelope's gain into the sweep | 0 … 48 dB | 24 dB |
//! | `out` | output gain | −24 … +24 dB | 0 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.flexus";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key (`modfx.*`: the family's
/// words, kept in Tremor's locale files).
pub const CHOICES: &[(&str, &str)] = &[
    ("mode", "flexus.mode"),
    ("tz", "flexus.tz"),
    ("invert", "flexus.invert"),
    ("sync", "modfx.sync"),
    ("div", "modfx.div"),
    ("shape", "modfx.shape"),
];

pub const PARAMS: &[EffectParamInfo] = &[
    p("mode", "fx.flexus.mode", 0.0, 2.0, 0.0, "", false, true),
    p("tz", "fx.flexus.tz", 0.0, 1.0, 0.0, "", false, true),
    p("mix", "fx.flexus.mix", 0.0, 1.0, 0.0, "%", false, false),
    p("delay", "fx.flexus.delay", 0.05, 5.0, 0.5, "ms", true, false),
    p("depth", "fx.flexus.depth", 0.0, 10.0, 4.0, "ms", false, false),
    p("fb", "fx.flexus.fb", -0.95, 0.95, 0.0, "%", false, false),
    p("invert", "fx.flexus.invert", 0.0, 1.0, 0.0, "", false, true),
    p("sync", "modfx.p.sync", 0.0, 1.0, 0.0, "", false, true),
    p("div", "modfx.p.div", 0.0, 16.0, 2.0, "", false, true),
    p("rate", "modfx.p.rate", 0.02, 10.0, 0.25, "Hz", true, false),
    p("shape", "modfx.p.shape", 0.0, 1.0, 1.0, "", false, true),
    p("spread", "modfx.p.spread", 0.0, 180.0, 0.0, "\u{b0}", false, false),
    p("manual", "fx.flexus.manual", 0.0, 1.0, 0.5, "%", false, false),
    p("att", "fx.flexus.att", 0.5, 100.0, 5.0, "ms", true, false),
    p("rel", "fx.flexus.rel", 10.0, 2_000.0, 200.0, "ms", true, false),
    p("sens", "fx.flexus.sens", 0.0, 48.0, 24.0, "dB", false, false),
    p("out", "fx.flexus.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}
