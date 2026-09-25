//! Comprimo's parameter model. Owned by the compressor; nothing else edits this file.
//!
//! Keys are forever: a project, a preset or an automation lane that names `thr` names the
//! threshold for good. The thirteen keys of the first release (`thr` … `listen`) keep
//! their meaning; the ranges only widened. What came after is below them, grouped:
//!
//! - **time and output** — `look` (lookahead, and the latency), `auto_gain`, `dry`,
//!   `mix` (0–200 %), `in_gain`, `out_gain`, `os` (oversampling: off, 2×, 4×);
//! - **the engine** — `custom` switches the detector from the style's own to the five
//!   `det_*` numbers the engine drawer edits;
//! - **the key** — `key_gain`, `key_eq` (the EQ's bypass) and six EQ bands addressed as
//!   `sc{n}.{field}`, whose numbers never change (the EQ's rule, ADR 0017).
//!
//! Sparse, like the EQ: a new compressor stores nothing, and a file holds only what moved.
//! A file from before it was sparse holds every key and loads as it was.

use crate::effect::{p, EffectParamInfo};
use std::sync::OnceLock;

pub const NAME_KEY: &str = "effect.compressor";
/// Six key-EQ bands of seven fields each are rarely touched: store only what moved.
pub const SPARSE: bool = true;
/// Its detector can listen to another strip ([`crate::EffectSlot::sidechain`]).
pub const TAKES_SIDECHAIN: bool = true;

/// Bands of the key's EQ.
pub const KEY_BANDS: usize = 6;

/// The parameters that are not per band: key, label, min, max, default, unit, log, stepped.
const MAIN: &[EffectParamInfo] = &[
    // The first release's thirteen.
    p("thr", "fx.comp.thr", -80.0, 0.0, -18.0, "dB", false, false),
    p("ratio", "fx.comp.ratio", 1.0, 100.0, 4.0, ":1", true, false),
    p("knee", "fx.comp.knee", 0.0, 72.0, 6.0, "dB", false, false),
    p(
        "range",
        "fx.comp.range",
        0.0,
        60.0,
        60.0,
        "dB",
        false,
        false,
    ),
    p("att", "fx.comp.att", 0.005, 250.0, 10.0, "ms", true, false),
    p("rel", "fx.comp.rel", 5.0, 5_000.0, 120.0, "ms", true, false),
    p(
        "auto_rel",
        "fx.comp.auto_rel",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p("hold", "fx.comp.hold", 0.0, 500.0, 0.0, "ms", false, false),
    p(
        "makeup",
        "fx.comp.makeup",
        -36.0,
        36.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p("style", "fx.comp.style", 0.0, 8.0, 0.0, "", false, true),
    p("link", "fx.comp.link", 0.0, 1.0, 1.0, "%", false, false),
    p(
        "key_hpf",
        "fx.comp.key_hpf",
        20.0,
        500.0,
        20.0,
        "Hz",
        true,
        false,
    ),
    p("listen", "fx.comp.listen", 0.0, 4.0, 0.0, "", false, true),
    // Time and output.
    p("look", "fx.comp.look", 0.0, 20.0, 0.0, "ms", false, false),
    p(
        "auto_gain",
        "fx.comp.auto_gain",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p("dry", "fx.comp.dry", -60.0, 12.0, -60.0, "dB", false, false),
    p("mix", "fx.comp.mix", 0.0, 2.0, 1.0, "%", false, false),
    p(
        "in_gain",
        "fx.comp.in_gain",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "out_gain",
        "fx.comp.out_gain",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p("os", "fx.comp.os", 0.0, 2.0, 0.0, "", false, true),
    // The engine drawer.
    p("custom", "fx.comp.custom", 0.0, 1.0, 0.0, "", false, true),
    p(
        "det_rms",
        "fx.comp.det_rms",
        0.1,
        300.0,
        0.1,
        "ms",
        true,
        false,
    ),
    p("det_fb", "fx.comp.det_fb", 0.0, 1.0, 0.0, "%", false, false),
    p(
        "det_prog",
        "fx.comp.det_prog",
        0.0,
        4.0,
        0.0,
        "",
        false,
        false,
    ),
    p(
        "det_dual",
        "fx.comp.det_dual",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "det_curve",
        "fx.comp.det_curve",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    // The key.
    p(
        "key_gain",
        "fx.comp.key_gain",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p("key_eq", "fx.comp.key_eq", 0.0, 1.0, 1.0, "", false, true),
];

/// One field of a key-EQ band: key, label, min, max, default, unit, log, stepped.
pub type BandField = (
    &'static str,
    &'static str,
    f32,
    f32,
    f32,
    &'static str,
    bool,
    bool,
);

/// The fields of one key-EQ band. Keys are `sc{n}.{field}`; shapes and slopes are
/// Aequalis's (`cantus_dsp::EqShape`).
pub const KEY_BAND_FIELDS: &[BandField] = &[
    ("used", "fx.comp.sc.used", 0.0, 1.0, 0.0, "", false, true),
    ("on", "fx.comp.sc.on", 0.0, 1.0, 1.0, "", false, true),
    ("shape", "fx.comp.sc.shape", 0.0, 8.0, 0.0, "", false, true),
    (
        "freq",
        "fx.comp.sc.freq",
        20.0,
        20_000.0,
        1_000.0,
        "Hz",
        true,
        false,
    ),
    (
        "gain",
        "fx.comp.sc.gain",
        -30.0,
        30.0,
        0.0,
        "dB",
        false,
        false,
    ),
    ("q", "fx.comp.sc.q", 0.1, 18.0, 1.0, "", true, false),
    (
        "slope",
        "fx.comp.sc.slope",
        6.0,
        96.0,
        12.0,
        "dB/oct",
        false,
        true,
    ),
];

/// The key of one field of one key-EQ band: `sc2.freq`.
pub fn key_band_key(band: usize, field: &str) -> String {
    format!("sc{band}.{field}")
}

/// Comprimo's parameters: the main list, then every field of every key-EQ band. Built
/// once and kept for the life of the program.
pub fn params() -> &'static [EffectParamInfo] {
    static PARAMS: OnceLock<Vec<EffectParamInfo>> = OnceLock::new();
    PARAMS.get_or_init(|| {
        let mut v = MAIN.to_vec();
        for band in 0..KEY_BANDS {
            for (key, label, min, max, default, unit, log, stepped) in KEY_BAND_FIELDS {
                let key: &'static str = Box::leak(key_band_key(band, key).into_boxed_str());
                v.push(p(key, label, *min, *max, *default, unit, *log, *stepped));
            }
        }
        v
    })
}

/// The first release's parameter table, kept under its old name for the code that reads
/// it; the full list is [`params`].
pub const COMPRESSOR_PARAMS: &[EffectParamInfo] = MAIN;

#[cfg(test)]
mod tests {
    use super::*;

    /// The first release's keys are all still here, with their defaults, and their ranges
    /// only grew — so every project that stored them opens as it sounded.
    #[test]
    fn the_first_keys_are_kept() {
        let first = [
            ("thr", -80.0, 0.0, -18.0),
            ("ratio", 1.0, 100.0, 4.0),
            ("knee", 0.0, 36.0, 6.0),
            ("range", 0.0, 60.0, 60.0),
            ("att", 0.05, 250.0, 10.0),
            ("rel", 5.0, 5_000.0, 120.0),
            ("auto_rel", 0.0, 1.0, 0.0),
            ("hold", 0.0, 500.0, 0.0),
            ("makeup", -12.0, 36.0, 0.0),
            ("style", 0.0, 4.0, 0.0),
            ("link", 0.0, 1.0, 1.0),
            ("key_hpf", 20.0, 500.0, 20.0),
            ("listen", 0.0, 2.0, 0.0),
        ];
        for (key, min, max, default) in first {
            let info = params().iter().find(|i| i.key == key).expect(key);
            assert!(info.min <= min && info.max >= max, "{key} narrowed");
            assert_eq!(info.default, default, "{key} moved its default");
        }
        assert_eq!(
            params().len(),
            MAIN.len() + KEY_BANDS * KEY_BAND_FIELDS.len()
        );
        let mut keys: Vec<&str> = params().iter().map(|i| i.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), params().len(), "a key is used twice");
    }
}
