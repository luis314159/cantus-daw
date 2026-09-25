//! Porta, the gate and expander: its parameter model. Owned by the gate.
//!
//! Every value is in the unit a musician reads — dB, ms, Hz — never a percentage. Keys are
//! forever once shipped (automation and saved projects hold them):
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `thr` | the level at which the gate **opens** | −80 … 0 dB |
//! | `hyst` | how far below `thr` it **closes** (hysteresis): the close threshold is `thr − hyst` | 0 … 24 dB |
//! | `ratio` | expansion below the threshold (above it, in the Upward style); 100 is ∞, a gate | 1 … 100 |
//! | `range` | the most it takes away (Upward: adds, up to 24 dB); 100 is ∞, silence | 0 … 100 dB |
//! | `knee` | a soft knee around the threshold | 0 … 30 dB |
//! | `att` | the time to open fully | 0.01 … 1000 ms |
//! | `hold` | the time it stays fully open after the key falls below the close threshold | 0 … 500 ms |
//! | `rel` | the time to close fully | 1 … 5000 ms |
//! | `look` | lookahead: how early the detector hears; any value above 0 costs a fixed 10 ms of latency | 0 … 10 ms |
//! | `style` | Classic, Clean, Vocal, Guitar, Upward, Ducking (`cantus_dsp::porta::Style`) | 0 … 5 |
//! | `hpf`, `lpf` | the detector's high- and low-pass; 20 Hz and 20 kHz are off | Hz |
//! | `key_slope` | their slope | 12 … 48 dB/oct |
//! | `listen` | hear the gate (0), the key (1), or what the gate removes (2) | 0 … 2 |
//! | `trig` | what opens it: the key's level (0) or the notes of the slot's track (1, [`crate::EffectSlot::midi_from`]) | 0 … 1 |
//! | `vel` | with notes: how much softer the softest note opens than the hardest (0 is every note fully) | 0 … 60 dB |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.gate";
pub const SPARSE: bool = false;
/// Its detector can listen to another strip ([`crate::EffectSlot::sidechain`]): the Ducking
/// style is keyed that way, and so is a snare gate opened by the snare's top mic.
pub const TAKES_SIDECHAIN: bool = true;
/// It can be opened by a track's notes instead of a level ([`crate::EffectSlot::midi_from`],
/// `trig` = 1): a gate played from a MIDI part (ADR 0022).
pub const TAKES_NOTES: bool = true;

pub const PARAMS: &[EffectParamInfo] = &[
    p("thr", "fx.gate.thr", -80.0, 0.0, -40.0, "dB", false, false),
    p("hyst", "fx.gate.hyst", 0.0, 24.0, 4.0, "dB", false, false),
    p(
        "ratio",
        "fx.gate.ratio",
        1.0,
        100.0,
        100.0,
        ":1",
        true,
        false,
    ),
    p(
        "range",
        "fx.gate.range",
        0.0,
        100.0,
        40.0,
        "dB",
        false,
        false,
    ),
    p("knee", "fx.gate.knee", 0.0, 30.0, 0.0, "dB", false, false),
    p("att", "fx.gate.att", 0.01, 1_000.0, 1.0, "ms", true, false),
    p("hold", "fx.gate.hold", 0.0, 500.0, 20.0, "ms", false, false),
    p("rel", "fx.gate.rel", 1.0, 5_000.0, 150.0, "ms", true, false),
    p("look", "fx.gate.look", 0.0, 10.0, 0.0, "ms", false, false),
    p("style", "fx.gate.style", 0.0, 5.0, 0.0, "", false, true),
    p(
        "hpf",
        "fx.gate.hpf",
        20.0,
        10_000.0,
        20.0,
        "Hz",
        true,
        false,
    ),
    p(
        "lpf",
        "fx.gate.lpf",
        100.0,
        20_000.0,
        20_000.0,
        "Hz",
        true,
        false,
    ),
    p(
        "key_slope",
        "fx.gate.key_slope",
        12.0,
        48.0,
        24.0,
        "dB/oct",
        false,
        true,
    ),
    p("listen", "fx.gate.listen", 0.0, 2.0, 0.0, "", false, true),
    p("trig", "fx.gate.trig", 0.0, 1.0, 0.0, "", false, true),
    p("vel", "fx.gate.vel", 0.0, 60.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_defaults_in_range() {
        for (i, a) in PARAMS.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            assert!(!a.logarithmic || a.min > 0.0, "{} is log from 0", a.key);
            for b in &PARAMS[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
    }
}
