//! Terminus, the limiter: its parameter model. Owned by the limiter; nothing else edits
//! this file.
//!
//! Keys are stable once shipped: automation and saved projects hold them. The DSP reads
//! them in `cantus_engine::fx::limiter::settings`; the meaning of each is in
//! `cantus_dsp::terminus::LimSettings`.
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `gain` | drive into the ceiling | 0 … 30 dB |
//! | `ceiling` | the most the output reaches (dBTP with `tp`) | −30 … 0 dB |
//! | `style` | Clean, Punch, Loud, Bus, Safe | 0 … 4 |
//! | `look` | look-ahead of the transient stage | 0 … 5 ms |
//! | `att`, `rel` | the release stage's attack and release | 0.1 ms … 10 s, 1 ms … 10 s |
//! | `link_t`, `link_r` | stereo link of each stage | 0 … 100 % |
//! | `knee` | soft knee under the ceiling | 0 … 12 dB |
//! | `tp` | true-peak limiting | off / on |
//! | `clip` | the clipper in front: how much of each peak it takes | 0 … 100 % |
//! | `dc` | DC filter | off / on |
//! | `monitor` | normal, 1:1 (gain undone), match (measured loudness) | 0 … 2 |
//! | `delta` | hear what is taken away | off / on |
//! | `dither` | off, 24, 22, 20, 18, 16 bits | 0 … 5 |
//! | `shape` | noise shaping: flat, basic, optimized | 0 … 2 |
//! | `target` | the loudness the display and the pass/fail aim at | −36 … −5 LUFS |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.limiter";
pub const SPARSE: bool = false;
/// The detector can listen to another strip — stem mastering: every stem limited by the
/// reduction the full mix needs ([`crate::EffectSlot::sidechain`]).
pub const TAKES_SIDECHAIN: bool = true;

pub const PARAMS: &[EffectParamInfo] = &[
    p("gain", "fx.lim.gain", 0.0, 30.0, 0.0, "dB", false, false),
    p(
        "ceiling",
        "fx.lim.ceiling",
        -30.0,
        0.0,
        -1.0,
        "dB",
        false,
        false,
    ),
    p("style", "fx.lim.style", 0.0, 4.0, 0.0, "", false, true),
    p("look", "fx.lim.look", 0.0, 5.0, 2.0, "ms", false, false),
    p("att", "fx.lim.att", 0.1, 10_000.0, 10.0, "ms", true, false),
    p("rel", "fx.lim.rel", 1.0, 10_000.0, 200.0, "ms", true, false),
    p("link_t", "fx.lim.link_t", 0.0, 1.0, 0.75, "%", false, false),
    p("link_r", "fx.lim.link_r", 0.0, 1.0, 1.0, "%", false, false),
    p("knee", "fx.lim.knee", 0.0, 12.0, 0.0, "dB", false, false),
    p("tp", "fx.lim.tp", 0.0, 1.0, 1.0, "", false, true),
    p("clip", "fx.lim.clip", 0.0, 1.0, 0.0, "%", false, false),
    p("dc", "fx.lim.dc", 0.0, 1.0, 0.0, "", false, true),
    p("monitor", "fx.lim.monitor", 0.0, 2.0, 0.0, "", false, true),
    p("delta", "fx.lim.delta", 0.0, 1.0, 0.0, "", false, true),
    p("dither", "fx.lim.dither", 0.0, 5.0, 0.0, "", false, true),
    p("shape", "fx.lim.shape", 0.0, 2.0, 0.0, "", false, true),
    p(
        "target",
        "fx.lim.target",
        -36.0,
        -5.0,
        -14.0,
        "LUFS",
        false,
        false,
    ),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}
