//! Lamina — the modelled electric piano: its parameter model. Owned by this instrument's
//! builder (ADR 0023).
//!
//! Values in the units of the instrument: the pickup's gap and alignment in millimetres,
//! decays in seconds, the damper in milliseconds, levels in dB, the tremolo in Hz. Shares
//! (hardness, coupling, drive…) are fractions shown as %. Keys are forever once shipped.
//!
//! | Group | Keys |
//! |---|---|
//! | Model | `engine` (0 tine, 1 reed), `range` (0 73, 1 64, 2 88 keys), `a4` (Hz), `spread` (ct), `age` |
//! | Hammer | `hard`, `hard.vel`, `force` (dB), `strike` (of the length), `hammer.noise` (dB) |
//! | Tine | `tine.decay` (s at C4), `tine.couple`, `tine.bell` (dB), `tine.bell_decay` (s), `tine.gap` `tine.align` (mm) |
//! | Reed | `reed.decay` (s at C4), `reed.overtones` (dB), `reed.gap` `reed.align` (mm) |
//! | Pickup | `pickup.level` (dB) |
//! | Damper | `damper.time` (ms), `damper.tone`, `damper.noise` `release.noise` (dB), `pedal.lift` |
//! | Preamp | `drive`, `bass` `treble` (dB), `output` (0 direct, 1 speaker) |
//! | Tremolo | `trem.mode` (0 the engine's, 1 pan, 2 amplitude), `trem.rate` (Hz), `trem.depth` |
//! | Playing | `dynamics` (dB), `vel.curve`, `polyphony` (voices) |
//! | Key curves | `curve.hard.1…8` (±), `curve.decay.1…8` (×), `curve.level.1…8` (dB): anchors at C1…C8 |
//! | Output | `gain` (dB) |

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.epiano";
pub const ICON: TrackIcon = TrackIcon::Keys;
/// Offered in the instrument picker once it is built.
pub const READY: bool = true;

/// The keyboard curves' anchor keys, by curve.
pub const CURVE_HARD: [&str; 8] = [
    "curve.hard.1",
    "curve.hard.2",
    "curve.hard.3",
    "curve.hard.4",
    "curve.hard.5",
    "curve.hard.6",
    "curve.hard.7",
    "curve.hard.8",
];
pub const CURVE_DECAY: [&str; 8] = [
    "curve.decay.1",
    "curve.decay.2",
    "curve.decay.3",
    "curve.decay.4",
    "curve.decay.5",
    "curve.decay.6",
    "curve.decay.7",
    "curve.decay.8",
];
pub const CURVE_LEVEL: [&str; 8] = [
    "curve.level.1",
    "curve.level.2",
    "curve.level.3",
    "curve.level.4",
    "curve.level.5",
    "curve.level.6",
    "curve.level.7",
    "curve.level.8",
];

const fn switch(key: &'static str, label: &'static str, max: f32, default: f32) -> InstParamInfo {
    p(key, label, 0.0, max, default, "", false, true)
}

const fn hard(key: &'static str) -> InstParamInfo {
    p(key, "epiano.curve_hard", -0.5, 0.5, 0.0, "%", false, false)
}
const fn decay(key: &'static str) -> InstParamInfo {
    p(
        key,
        "epiano.curve_decay",
        0.25,
        4.0,
        1.0,
        "\u{d7}",
        true,
        false,
    )
}
const fn level(key: &'static str) -> InstParamInfo {
    p(
        key,
        "epiano.curve_level",
        -12.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    )
}

pub const PARAMS: &[InstParamInfo] = &[
    switch("engine", "epiano.engine", 1.0, 0.0),
    switch("range", "epiano.range", 2.0, 0.0),
    p("a4", "epiano.a4", 430.0, 450.0, 440.0, "Hz", false, false),
    p(
        "spread",
        "epiano.spread",
        0.0,
        10.0,
        2.0,
        "ct",
        false,
        false,
    ),
    p("age", "epiano.age", 0.0, 1.0, 0.2, "%", false, false),
    p("hard", "epiano.hard", 0.0, 1.0, 0.5, "%", false, false),
    p(
        "hard.vel",
        "epiano.hard_vel",
        0.0,
        1.0,
        0.6,
        "%",
        false,
        false,
    ),
    p("force", "epiano.force", -24.0, 6.0, 0.0, "dB", false, false),
    p(
        "strike",
        "epiano.strike",
        0.02,
        0.2,
        0.08,
        "%",
        false,
        false,
    ),
    p(
        "hammer.noise",
        "epiano.hammer_noise",
        -60.0,
        0.0,
        -24.0,
        "dB",
        false,
        false,
    ),
    p(
        "tine.decay",
        "epiano.tine_decay",
        0.5,
        30.0,
        8.0,
        "s",
        true,
        false,
    ),
    p(
        "tine.couple",
        "epiano.tine_couple",
        0.0,
        1.0,
        0.5,
        "%",
        false,
        false,
    ),
    p(
        "tine.bell",
        "epiano.tine_bell",
        -40.0,
        0.0,
        -12.0,
        "dB",
        false,
        false,
    ),
    p(
        "tine.bell_decay",
        "epiano.tine_bell_decay",
        0.02,
        2.0,
        0.25,
        "s",
        true,
        false,
    ),
    p(
        "tine.gap",
        "epiano.tine_gap",
        0.3,
        5.0,
        1.6,
        "mm",
        true,
        false,
    ),
    p(
        "tine.align",
        "epiano.tine_align",
        -2.0,
        2.0,
        0.6,
        "mm",
        false,
        false,
    ),
    p(
        "reed.decay",
        "epiano.reed_decay",
        0.3,
        10.0,
        3.0,
        "s",
        true,
        false,
    ),
    p(
        "reed.overtones",
        "epiano.reed_overtones",
        -40.0,
        0.0,
        -10.0,
        "dB",
        false,
        false,
    ),
    p(
        "reed.gap",
        "epiano.reed_gap",
        0.3,
        5.0,
        1.0,
        "mm",
        true,
        false,
    ),
    p(
        "reed.align",
        "epiano.reed_align",
        -2.0,
        2.0,
        0.6,
        "mm",
        false,
        false,
    ),
    p(
        "pickup.level",
        "epiano.pickup_level",
        -24.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "damper.time",
        "epiano.damper_time",
        20.0,
        1_000.0,
        120.0,
        "ms",
        true,
        false,
    ),
    p(
        "damper.tone",
        "epiano.damper_tone",
        0.0,
        1.0,
        0.5,
        "%",
        false,
        false,
    ),
    p(
        "damper.noise",
        "epiano.damper_noise",
        -60.0,
        0.0,
        -30.0,
        "dB",
        false,
        false,
    ),
    p(
        "release.noise",
        "epiano.release_noise",
        -60.0,
        0.0,
        -36.0,
        "dB",
        false,
        false,
    ),
    p(
        "pedal.lift",
        "epiano.pedal_lift",
        0.1,
        0.9,
        0.35,
        "%",
        false,
        false,
    ),
    p("drive", "epiano.drive", 0.0, 1.0, 0.2, "%", false, false),
    p("bass", "epiano.bass", -12.0, 12.0, 0.0, "dB", false, false),
    p(
        "treble",
        "epiano.treble",
        -12.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    ),
    switch("output", "epiano.output", 1.0, 0.0),
    switch("trem.mode", "epiano.trem_mode", 2.0, 0.0),
    p(
        "trem.rate",
        "epiano.trem_rate",
        0.5,
        12.0,
        5.5,
        "Hz",
        true,
        false,
    ),
    p(
        "trem.depth",
        "epiano.trem_depth",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "dynamics",
        "epiano.dynamics",
        20.0,
        60.0,
        40.0,
        "dB",
        false,
        false,
    ),
    p(
        "vel.curve",
        "epiano.vel_curve",
        -1.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "polyphony",
        "epiano.polyphony",
        8.0,
        64.0,
        32.0,
        "voices",
        false,
        true,
    ),
    hard("curve.hard.1"),
    hard("curve.hard.2"),
    hard("curve.hard.3"),
    hard("curve.hard.4"),
    hard("curve.hard.5"),
    hard("curve.hard.6"),
    hard("curve.hard.7"),
    hard("curve.hard.8"),
    decay("curve.decay.1"),
    decay("curve.decay.2"),
    decay("curve.decay.3"),
    decay("curve.decay.4"),
    decay("curve.decay.5"),
    decay("curve.decay.6"),
    decay("curve.decay.7"),
    decay("curve.decay.8"),
    level("curve.level.1"),
    level("curve.level.2"),
    level("curve.level.3"),
    level("curve.level.4"),
    level("curve.level.5"),
    level("curve.level.6"),
    level("curve.level.7"),
    level("curve.level.8"),
    p("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

/// The modulation sources it produces (tags from `modulation::SOURCES`).
pub const SOURCES: &[&str] = &[
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8", "vel", "key",
    "mw", "at", "pb", "expr", "sus",
];

pub fn param(key: &str) -> Option<&'static InstParamInfo> {
    find(PARAMS, key).or_else(|| super::modulation::param(key))
}

pub fn is_text_key(key: &str) -> bool {
    super::modulation::is_text_key(key)
}

pub fn is_asset_key(key: &str) -> bool {
    let _ = key;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_tables_name_real_keys() {
        for k in CURVE_HARD.iter().chain(&CURVE_DECAY).chain(&CURVE_LEVEL) {
            assert!(param(k).is_some(), "{k}");
        }
        assert_eq!(param("tine.gap").unwrap().unit, "mm");
    }
}
