//! Fervor, saturation and colour (ADR 0026): its parameter model. Owned by Fervor;
//! nothing else edits this file.
//!
//! Keys are forever once shipped: automation and saved projects hold them. **Every default
//! is a null**: with nothing moved, Fervor returns its input to the bit.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `model` | Tape, Tube, Drive, Fuzz, Clip, Fold, Curve (`cantus_dsp::fervor::Model`) | 0 … 6 | Tape |
//! | `drive` | input gain into the curve; at 0 dB every model is a wire (see the DSP) | 0 … 36 dB | 0 |
//! | `boost` | ×100: 40 dB more drive (Blood Overdrive's switch) | 0 / 1 | off |
//! | `char` | the model's own character (tape: high-end emphasis; tube: asymmetry; drive: low end spared; fuzz: gating; clip: knee; fold: sine ↔ triangle) | 0 … 100 % | 50 % |
//! | `bias` | an offset into the curve: even harmonics | −100 … 100 % | 0 |
//! | `lo_cut` | high-pass before the curve (tightens the low end); 20 Hz is off | 20 … 2 000 Hz | off |
//! | `hi_cut` | low-pass after the curve (tames the fizz); 20 kHz is off | 1 … 20 kHz | off |
//! | `tone` | tilt after the curve, around 1 kHz | −12 … 12 dB | 0 |
//! | `os` | oversampling: 1×, 2×, 4×, 8× | 0 … 3 | 1× |
//! | `phase` | the oversampling filters: linear (latency) or minimum phase | 0 / 1 | linear |
//! | `bits` | bit depth after the curve; 24 is off | 1 … 24 | off |
//! | `rate` | sample-and-hold rate after the curve; 50 kHz is off | 100 … 50 000 Hz | off |
//! | `chan` | what is coloured: both channels, the mid, or the side | 0 … 2 | stereo |
//! | `mix` | dry/wet, the dry path delayed to match | 0 … 100 % | 100 % |
//! | `auto` | level-match: the curve's loudness at −18 dBFS RMS taken back out | 0 / 1 | on |
//! | `out` | output gain | −24 … 24 dB | 0 |
//! | `c.sym` | the drawn curve is odd-symmetric (only x ≥ 0 is drawn, mirrored) | 0 / 1 | on |
//! | `c.lo`, `c.hi` | the drawn curve's ends: its value at x = −1 and x = 1 | −1 … 1 | −1, 1 |
//! | `c.t` | tension of the curve's last segment | −1 … 1 | 0 |
//! | `p{n}.on/x/y/t` | the drawn curve's points, n = 0 … 15: used, where, tension of the segment that ends there | | unused |
//!
//! A point's number never changes once it exists (as Aequalis's bands): removing one frees
//! its number, and no other point is renumbered.

use crate::effect::{p, EffectParamInfo};
use std::sync::OnceLock;

pub const NAME_KEY: &str = "effect.saturator";
/// The drawn curve's 64 point parameters are rarely all used: store only what moved.
pub const SPARSE: bool = true;
pub const TAKES_SIDECHAIN: bool = false;

/// How many points the drawn curve can have, besides its two ends.
pub const POINTS: usize = 16;

/// The models, by the `model` parameter's index.
pub const MODELS: usize = 7;

/// One field of a drawn point: key, label key, min, max, default, unit, logarithmic, stepped.
pub type PointField = (
    &'static str,
    &'static str,
    f32,
    f32,
    f32,
    &'static str,
    bool,
    bool,
);

/// The fields of one point of the drawn curve. Keys are `p{n}.{field}`.
pub const POINT_FIELDS: &[PointField] = &[
    ("on", "fx.sat.p.on", 0.0, 1.0, 0.0, "", false, true),
    ("x", "fx.sat.p.x", -1.0, 1.0, 0.0, "", false, false),
    ("y", "fx.sat.p.y", -1.0, 1.0, 0.0, "", false, false),
    ("t", "fx.sat.p.t", -1.0, 1.0, 0.0, "", false, false),
];

/// The parameters that are not a point's.
pub const GLOBALS: &[EffectParamInfo] = &[
    p("model", "fx.sat.model", 0.0, 6.0, 0.0, "", false, true),
    p("drive", "fx.sat.drive", 0.0, 36.0, 0.0, "dB", false, false),
    p("boost", "fx.sat.boost", 0.0, 1.0, 0.0, "", false, true),
    p("char", "fx.sat.char", 0.0, 1.0, 0.5, "%", false, false),
    p("bias", "fx.sat.bias", -1.0, 1.0, 0.0, "%", false, false),
    p(
        "lo_cut",
        "fx.sat.lo_cut",
        20.0,
        2_000.0,
        20.0,
        "Hz",
        true,
        false,
    ),
    p(
        "hi_cut",
        "fx.sat.hi_cut",
        1_000.0,
        20_000.0,
        20_000.0,
        "Hz",
        true,
        false,
    ),
    p("tone", "fx.sat.tone", -12.0, 12.0, 0.0, "dB", false, false),
    p("os", "fx.sat.os", 0.0, 3.0, 0.0, "", false, true),
    p("phase", "fx.sat.phase", 0.0, 1.0, 0.0, "", false, true),
    p("bits", "fx.sat.bits", 1.0, 24.0, 24.0, "", false, false),
    p(
        "rate",
        "fx.sat.rate",
        100.0,
        50_000.0,
        50_000.0,
        "Hz",
        true,
        false,
    ),
    p("chan", "fx.sat.chan", 0.0, 2.0, 0.0, "", false, true),
    p("mix", "fx.sat.mix", 0.0, 1.0, 1.0, "%", false, false),
    p("auto", "fx.sat.auto", 0.0, 1.0, 1.0, "", false, true),
    p("out", "fx.sat.out", -24.0, 24.0, 0.0, "dB", false, false),
    p("c.sym", "fx.sat.c.sym", 0.0, 1.0, 1.0, "", false, true),
    p("c.lo", "fx.sat.c.lo", -1.0, 1.0, -1.0, "", false, false),
    p("c.hi", "fx.sat.c.hi", -1.0, 1.0, 1.0, "", false, false),
    p("c.t", "fx.sat.c.t", -1.0, 1.0, 0.0, "", false, false),
];

/// The key of one field of one point: `p3.y`.
pub fn point_key(point: usize, field: &str) -> String {
    format!("p{point}.{field}")
}

/// The four keys of every point, `&'static`, in point order: `[on, x, y, t]`. What the
/// engine reads the curve with, without building a string.
pub fn point_keys() -> &'static [[&'static str; 4]; POINTS] {
    static KEYS: OnceLock<[[&'static str; 4]; POINTS]> = OnceLock::new();
    KEYS.get_or_init(|| {
        let all = params();
        std::array::from_fn(|n| std::array::from_fn(|f| all[GLOBALS.len() + n * 4 + f].key))
    })
}

/// Fervor's parameters: the globals, then every field of every point. Built once and kept
/// for the life of the program, so the model stays `&'static` like the hand-written tables.
pub fn params() -> &'static [EffectParamInfo] {
    static PARAMS: OnceLock<Vec<EffectParamInfo>> = OnceLock::new();
    PARAMS.get_or_init(|| {
        let mut v = Vec::with_capacity(GLOBALS.len() + POINTS * POINT_FIELDS.len());
        v.extend_from_slice(GLOBALS);
        for n in 0..POINTS {
            for (key, label, min, max, default, unit, log, stepped) in POINT_FIELDS {
                let key: &'static str = Box::leak(point_key(n, key).into_boxed_str());
                v.push(p(key, label, *min, *max, *default, unit, *log, *stepped));
            }
        }
        v
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_defaults_in_range() {
        let all = params();
        assert_eq!(all.len(), GLOBALS.len() + POINTS * 4);
        for (i, a) in all.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            assert!(!a.logarithmic || a.min > 0.0, "{} is log from 0", a.key);
            for b in &all[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
    }

    #[test]
    fn point_keys_name_their_fields() {
        let k = point_keys();
        assert_eq!(k[0], ["p0.on", "p0.x", "p0.y", "p0.t"]);
        assert_eq!(k[15][3], "p15.t");
    }
}
