//! Sibilus, the de-esser: its parameter model. Owned by the de-esser; nothing else edits
//! this file.
//!
//! Keys are stable once shipped: automation lanes, presets and saved projects hold them.
//! The stepped ones are choices, by index:
//!
//! | Key | Choices |
//! |---|---|
//! | `mode` | 0 single vocal, 1 allround |
//! | `proc` | 0 wide band, 1 split above the band's low edge, 2 split: the band only |
//! | `stereo` | 0 L/R, 1 M/S, 2 mid only, 3 side only |
//! | `listen` | 0 the output, 1 the trigger (what the detector hears), 2 what is removed |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.deesser";
pub const SPARSE: bool = false;
/// The detector can listen to another strip ([`crate::EffectSlot::sidechain`]): the clean
/// vocal keying the distorted copy.
pub const TAKES_SIDECHAIN: bool = true;

pub const PARAMS: &[EffectParamInfo] = &[
    p("mode", "fx.ds.mode", 0.0, 1.0, 0.0, "", false, true),
    p("proc", "fx.ds.proc", 0.0, 2.0, 0.0, "", false, true),
    p("thr", "fx.ds.thr", -60.0, 0.0, -30.0, "dB", false, false),
    p("range", "fx.ds.range", 0.0, 30.0, 6.0, "dB", false, false),
    p(
        "lo", "fx.ds.lo", 1_000.0, 20_000.0, 5_000.0, "Hz", true, false,
    ),
    p(
        "hi", "fx.ds.hi", 1_000.0, 20_000.0, 12_000.0, "Hz", true, false,
    ),
    p("att", "fx.ds.att", 0.1, 50.0, 1.0, "ms", true, false),
    p("rel", "fx.ds.rel", 5.0, 500.0, 40.0, "ms", true, false),
    // Whole milliseconds: a lookahead is a latency, and the host compensates each value.
    p("look", "fx.ds.look", 0.0, 15.0, 5.0, "ms", false, true),
    p("link", "fx.ds.link", 0.0, 1.0, 1.0, "%", false, false),
    p("stereo", "fx.ds.stereo", 0.0, 3.0, 0.0, "", false, true),
    p("listen", "fx.ds.listen", 0.0, 2.0, 0.0, "", false, true),
    p("sens", "fx.ds.sens", 0.0, 1.0, 0.5, "%", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}
