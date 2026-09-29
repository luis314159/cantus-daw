//! Organum — the tonewheel organ and rotary speaker: its parameter model. Owned by this
//! instrument's builder (ADR 0023).
//!
//! Every value is in the unit an organist reads: drawbar positions 0–8 (3 dB a step),
//! dB, ms, Hz, rpm, seconds, cm and degrees. Switches are stepped numbers (0/1, or an
//! index into a short list the editor names). Keys are forever once shipped.
//!
//! | Group | Keys |
//! |---|---|
//! | Drawbars | `u1…u9` upper 16′…1′, `l1…l9` lower, `p1` `p2` pedal 16′ 8′ |
//! | Splits | `split.upper` (first key of the upper manual), `split.pedal` (first key above the pedals); 0 = none |
//! | Percussion | `perc`, `perc.harm` (0 2nd, 1 3rd), `perc.decay` (0 fast, 1 slow), `perc.vol` (0 normal, 1 soft), `perc.fast` `perc.slow` (ms to −60 dB), `perc.soft` `perc.drop` (dB), `perc.steal` |
//! | Vibrato | `vib` (V1 V2 V3 C1 C2 C3), `vib.upper`, `vib.lower`, `vib.rate` (Hz) |
//! | Tone | `click` `click.rel` `leak` `leak.bar` `bright` (dB), `tuning` (0 gear, 1 equal), `a4` (Hz) |
//! | Amp | `swell` `drive` (fractions shown as %), `lvl.upper` `lvl.lower` `lvl.pedal` (dB) |
//! | Rotary | `rot.out` (0 rotary, 1 direct), `rot.speed` (0 stop, 1 slow, 2 fast), `rot.horn_slow` `rot.horn_fast` `rot.drum_slow` `rot.drum_fast` (rpm), `rot.horn_up` `rot.horn_down` `rot.drum_up` `rot.drum_down` (s), `rot.balance` (dB), `rot.xover` (Hz), `rot.cab` (0 closed, 1 open), `rot.dist` (cm), `rot.spread` `rot.brake` (°) |
//! | Output | `gain` (dB) |

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.organ";
pub const ICON: TrackIcon = TrackIcon::Keys;
/// Offered in the instrument picker once it is built.
pub const READY: bool = true;

/// The upper manual's drawbars, 16′ first.
pub const UPPER: [&str; 9] = ["u1", "u2", "u3", "u4", "u5", "u6", "u7", "u8", "u9"];
pub const LOWER: [&str; 9] = ["l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8", "l9"];
/// The pedal's two drawbars, 16′ and 8′.
pub const PEDAL: [&str; 2] = ["p1", "p2"];

/// The vibrato switch's positions, by the index `vib` stores.
pub const VIB_MODES: [&str; 6] = ["V1", "V2", "V3", "C1", "C2", "C3"];

const fn bar(key: &'static str, label: &'static str, default: f32) -> InstParamInfo {
    p(key, label, 0.0, 8.0, default, "", false, true)
}

const fn switch(key: &'static str, label: &'static str, max: f32, default: f32) -> InstParamInfo {
    p(key, label, 0.0, max, default, "", false, true)
}

pub const PARAMS: &[InstParamInfo] = &[
    bar("u1", "organ.u1", 8.0),
    bar("u2", "organ.u2", 8.0),
    bar("u3", "organ.u3", 8.0),
    bar("u4", "organ.u4", 0.0),
    bar("u5", "organ.u5", 0.0),
    bar("u6", "organ.u6", 0.0),
    bar("u7", "organ.u7", 0.0),
    bar("u8", "organ.u8", 0.0),
    bar("u9", "organ.u9", 0.0),
    bar("l1", "organ.l1", 0.0),
    bar("l2", "organ.l2", 0.0),
    bar("l3", "organ.l3", 8.0),
    bar("l4", "organ.l4", 8.0),
    bar("l5", "organ.l5", 0.0),
    bar("l6", "organ.l6", 0.0),
    bar("l7", "organ.l7", 0.0),
    bar("l8", "organ.l8", 0.0),
    bar("l9", "organ.l9", 0.0),
    bar("p1", "organ.p1", 8.0),
    bar("p2", "organ.p2", 4.0),
    p(
        "split.upper",
        "organ.split_upper",
        0.0,
        127.0,
        0.0,
        "key",
        false,
        true,
    ),
    p(
        "split.pedal",
        "organ.split_pedal",
        0.0,
        127.0,
        0.0,
        "key",
        false,
        true,
    ),
    switch("perc", "organ.perc", 1.0, 0.0),
    switch("perc.harm", "organ.perc_harm", 1.0, 1.0),
    switch("perc.decay", "organ.perc_decay", 1.0, 0.0),
    switch("perc.vol", "organ.perc_vol", 1.0, 0.0),
    p(
        "perc.fast",
        "organ.perc_fast",
        50.0,
        3_000.0,
        1_000.0,
        "ms",
        true,
        false,
    ),
    p(
        "perc.slow",
        "organ.perc_slow",
        200.0,
        8_000.0,
        4_000.0,
        "ms",
        true,
        false,
    ),
    p(
        "perc.soft",
        "organ.perc_soft",
        -12.0,
        0.0,
        -6.0,
        "dB",
        false,
        false,
    ),
    p(
        "perc.drop",
        "organ.perc_drop",
        -6.0,
        0.0,
        -3.0,
        "dB",
        false,
        false,
    ),
    switch("perc.steal", "organ.perc_steal", 1.0, 1.0),
    switch("vib", "organ.vib", 5.0, 5.0),
    switch("vib.upper", "organ.vib_upper", 1.0, 1.0),
    switch("vib.lower", "organ.vib_lower", 1.0, 0.0),
    p(
        "vib.rate",
        "organ.vib_rate",
        4.0,
        9.0,
        7.0,
        "Hz",
        false,
        false,
    ),
    p("click", "organ.click", -60.0, 6.0, -6.0, "dB", false, false),
    p(
        "click.rel",
        "organ.click_rel",
        -60.0,
        6.0,
        -12.0,
        "dB",
        false,
        false,
    ),
    p(
        "leak",
        "organ.leak",
        -80.0,
        -20.0,
        -40.0,
        "dB",
        false,
        false,
    ),
    p(
        "leak.bar",
        "organ.leak_bar",
        -80.0,
        -20.0,
        -60.0,
        "dB",
        false,
        false,
    ),
    p("bright", "organ.bright", -6.0, 6.0, 0.0, "dB", false, false),
    switch("tuning", "organ.tuning", 1.0, 0.0),
    p("a4", "organ.a4", 415.0, 466.0, 440.0, "Hz", false, false),
    p("swell", "organ.swell", 0.0, 1.0, 1.0, "%", false, false),
    p("drive", "organ.drive", 0.0, 1.0, 0.2, "%", false, false),
    p(
        "lvl.upper",
        "organ.lvl_upper",
        -40.0,
        6.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "lvl.lower",
        "organ.lvl_lower",
        -40.0,
        6.0,
        -3.0,
        "dB",
        false,
        false,
    ),
    p(
        "lvl.pedal",
        "organ.lvl_pedal",
        -40.0,
        6.0,
        -3.0,
        "dB",
        false,
        false,
    ),
    switch("rot.out", "organ.rot_out", 1.0, 0.0),
    switch("rot.speed", "organ.rot_speed", 2.0, 1.0),
    p(
        "rot.horn_slow",
        "organ.horn_slow",
        20.0,
        80.0,
        40.0,
        "rpm",
        false,
        false,
    ),
    p(
        "rot.horn_fast",
        "organ.horn_fast",
        300.0,
        480.0,
        420.0,
        "rpm",
        false,
        false,
    ),
    p(
        "rot.drum_slow",
        "organ.drum_slow",
        20.0,
        80.0,
        36.0,
        "rpm",
        false,
        false,
    ),
    p(
        "rot.drum_fast",
        "organ.drum_fast",
        250.0,
        450.0,
        357.0,
        "rpm",
        false,
        false,
    ),
    p(
        "rot.horn_up",
        "organ.horn_up",
        0.05,
        2.0,
        0.16,
        "s",
        true,
        false,
    ),
    p(
        "rot.horn_down",
        "organ.horn_down",
        0.05,
        3.0,
        0.32,
        "s",
        true,
        false,
    ),
    p(
        "rot.drum_up",
        "organ.drum_up",
        0.5,
        10.0,
        4.1,
        "s",
        true,
        false,
    ),
    p(
        "rot.drum_down",
        "organ.drum_down",
        0.5,
        6.0,
        1.4,
        "s",
        true,
        false,
    ),
    p(
        "rot.balance",
        "organ.rot_balance",
        -12.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "rot.xover",
        "organ.rot_xover",
        500.0,
        1_200.0,
        800.0,
        "Hz",
        true,
        false,
    ),
    switch("rot.cab", "organ.rot_cab", 1.0, 0.0),
    p(
        "rot.dist",
        "organ.rot_dist",
        10.0,
        200.0,
        60.0,
        "cm",
        true,
        false,
    ),
    p(
        "rot.spread",
        "organ.rot_spread",
        0.0,
        180.0,
        120.0,
        "\u{b0}",
        false,
        false,
    ),
    p(
        "rot.brake",
        "organ.rot_brake",
        0.0,
        359.0,
        90.0,
        "\u{b0}",
        false,
        false,
    ),
    p("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

/// The modulation sources it produces (tags from `modulation::SOURCES`): the macros and
/// the channel's controllers. The organ has no voices, so no per-note sources.
pub const SOURCES: &[&str] = &[
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8", "mw", "expr",
    "at", "sus",
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
    fn the_drawbar_tables_name_real_keys() {
        for k in UPPER.iter().chain(&LOWER).chain(&PEDAL) {
            let i = param(k).unwrap();
            assert!(i.stepped && i.max == 8.0, "{k}");
        }
        assert_eq!(param("gain").unwrap().default, 0.0);
    }
}
