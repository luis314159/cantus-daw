//! Aequalis's parameter model: the band table and the parameter list built from it.
//! Owned by the EQ; nothing else edits this file.
//!
//! Keys are forever once shipped: an automation lane or a saved project that names
//! `b3.gain` or `mode` must go on naming the same thing. New fields are added, never
//! renamed; a field's default is what an old file that lacks it meant.

use crate::effect::{p, EffectParamInfo};
use std::sync::OnceLock;

pub const NAME_KEY: &str = "effect.eq";
/// Most of the 400-odd parameters are never touched: store only what moved.
pub const SPARSE: bool = true;
/// A dynamic band can listen to another strip (`EffectSlot::sidechain`), and the editor
/// draws that strip's spectrum, its collisions with this one, and matches to it.
pub const TAKES_SIDECHAIN: bool = true;

/// How many bands Aequalis has. A band's number never changes once it exists, so an
/// automation lane or a script that names `b3.gain` names the same band forever.
pub const EQ_BANDS: usize = 24;

/// One field of an EQ band: key, label key, min, max, default, unit, logarithmic, stepped.
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

/// The fields of one EQ band, in the order the editor lists them. Keys are `b{n}.{field}`.
pub const EQ_BAND_FIELDS: &[BandField] = &[
    // key, label, min, max, default, unit, log, stepped
    ("used", "fx.eq.used", 0.0, 1.0, 0.0, "", false, true),
    ("on", "fx.eq.on", 0.0, 1.0, 1.0, "", false, true),
    ("shape", "fx.eq.shape", 0.0, 8.0, 0.0, "", false, true),
    (
        "freq",
        "fx.eq.freq",
        10.0,
        30_000.0,
        1_000.0,
        "Hz",
        true,
        false,
    ),
    ("gain", "fx.eq.gain", -30.0, 30.0, 0.0, "dB", false, false),
    ("q", "fx.eq.q", 0.025, 40.0, 1.0, "", true, false),
    (
        "slope",
        "fx.eq.slope",
        6.0,
        96.0,
        12.0,
        "dB/oct",
        false,
        true,
    ),
    ("place", "fx.eq.place", 0.0, 4.0, 0.0, "", false, true),
    ("dyn", "fx.eq.dyn", 0.0, 1.0, 0.0, "", false, true),
    ("thr", "fx.eq.thr", -80.0, 0.0, -24.0, "dB", false, false),
    (
        "range",
        "fx.eq.range",
        -30.0,
        30.0,
        -6.0,
        "dB",
        false,
        false,
    ),
    ("ratio", "fx.eq.ratio", 1.0, 20.0, 4.0, ":1", true, false),
    ("att", "fx.eq.att", 0.1, 300.0, 10.0, "ms", true, false),
    ("rel", "fx.eq.rel", 5.0, 3_000.0, 120.0, "ms", true, false),
    // Added in round 16. 0 = the band moves while the level is over the threshold, 1 = while
    // it is under it.
    ("dir", "fx.eq.dir", 0.0, 1.0, 0.0, "", false, true),
    // 0 = the detector hears the band's own signal, 1 = the sidechain key.
    ("key", "fx.eq.key", 0.0, 1.0, 0.0, "", false, true),
    // Gain–Q interaction (bells): the band narrows as its gain grows, as an analog one does.
    ("gq", "fx.eq.gq", 0.0, 1.0, 0.0, "", false, true),
];

/// The parameters that are not a band's: output gain, processing mode and resolution, auto
/// gain, the band being auditioned (0 = none, n = band n − 1), and the gain scale.
pub const EQ_GLOBALS: &[EffectParamInfo] = &[
    p("out", "fx.eq.out", -24.0, 24.0, 0.0, "dB", false, false),
    p("mode", "fx.eq.mode", 0.0, 2.0, 0.0, "", false, true),
    p("lp_res", "fx.eq.lp_res", 0.0, 3.0, 1.0, "", false, true),
    p("auto", "fx.eq.auto", 0.0, 1.0, 0.0, "", false, true),
    p("solo", "fx.eq.solo", 0.0, 24.0, 0.0, "", false, true),
    p("scale", "fx.eq.scale", 0.0, 2.0, 1.0, "%", false, false),
];

/// The key of one field of one band: `b3.gain`.
pub fn eq_key(band: usize, field: &str) -> String {
    format!("b{band}.{field}")
}

/// Aequalis's parameters: every field of every band, then the globals. Built once and
/// kept for the life of the program, so the parameter model stays `&'static` like the
/// hand-written tables above.
pub fn params() -> &'static [EffectParamInfo] {
    static PARAMS: OnceLock<Vec<EffectParamInfo>> = OnceLock::new();
    PARAMS.get_or_init(|| {
        let mut v = Vec::with_capacity(EQ_BANDS * EQ_BAND_FIELDS.len() + EQ_GLOBALS.len());
        for band in 0..EQ_BANDS {
            for (key, label, min, max, default, unit, log, stepped) in EQ_BAND_FIELDS {
                let key: &'static str = Box::leak(eq_key(band, key).into_boxed_str());
                v.push(p(key, label, *min, *max, *default, unit, *log, *stepped));
            }
        }
        v.extend_from_slice(EQ_GLOBALS);
        v
    })
}
