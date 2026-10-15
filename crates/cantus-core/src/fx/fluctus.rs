//! Fluctus, the volume shaper (LFO Tool / Kickstart) (ADR 0055): its parameter model.
//! Owned by Fluctus; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the depth starts
//! at 0; the curve waits, drawn as the classic sidechain pump on the quarters, for the depth
//! to be turned up.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `depth` | how far the level follows the curve | 0 … 100 % | 0 |
//! | `div` | the curve's length, synced to the song (`cantus_dsp::modfx_clock::DIVISIONS`) | 4/1 … 1/16 | 1/4 |
//! | `offset` | where in the song's cycle the curve starts | 0 … 100 % | 0 |
//! | `smooth` | a glide on the level, so a step never clicks | 0 … 50 ms | 2 ms |
//! | `out` | output gain | −24 … +24 dB | 0 |
//! | `p{n}.on/x/y/t/s` | the drawn curve's points, n = 0 … 15: used, where in the cycle, the level, the tension and the shape (the automation editor's six: curve, S-curve, hold, stairs, pulse, wave) of the segment that starts there | | p0 and p1 used: the pump |
//!
//! A point's number never changes once it exists (as Fervor's and Aequalis's): removing one
//! frees its number, and no other point is renumbered.

use crate::effect::{p, EffectParamInfo};
use std::sync::OnceLock;

pub const NAME_KEY: &str = "effect.fluctus";
/// The curve's eighty point parameters are rarely all used: store only what moved.
pub const SPARSE: bool = true;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key; a point's shape by its
/// field (`p.s`, the exported plug-in's group of points).
pub const CHOICES: &[(&str, &str)] = &[("div", "modfx.div"), ("p.s", "fluctus.shape")];

/// How many points the curve may hold.
pub const POINTS: usize = 16;

/// A point field: (key, label key, min, max, default, unit, logarithmic, stepped).
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

/// Every point's fields, at the defaults of an unused point.
pub const POINT_FIELDS: &[PointField] = &[
    ("on", "fx.fluctus.p.on", 0.0, 1.0, 0.0, "", false, true),
    ("x", "fx.fluctus.p.x", 0.0, 1.0, 0.0, "%", false, false),
    ("y", "fx.fluctus.p.y", 0.0, 1.0, 1.0, "%", false, false),
    ("t", "fx.fluctus.p.t", -1.0, 1.0, 0.0, "", false, false),
    ("s", "fx.fluctus.p.s", 0.0, 5.0, 0.0, "", false, true),
];

/// The default curve, the pump: the level at nothing on the beat, rising fast (a negative
/// tension: quick, then easing) to full by half the cycle, held there.
const DEFAULT_POINTS: [(usize, [f32; 5]); 2] = [
    (0, [1.0, 0.0, 0.0, -0.6, 0.0]),
    (1, [1.0, 0.5, 1.0, 0.0, 2.0]),
];

/// The parameters that are not a point's.
pub const GLOBALS: &[EffectParamInfo] = &[
    p(
        "depth",
        "fx.fluctus.depth",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p("div", "fx.fluctus.div", 1.0, 14.0, 8.0, "", false, true),
    p(
        "offset",
        "fx.fluctus.offset",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p(
        "smooth",
        "fx.fluctus.smooth",
        0.0,
        50.0,
        2.0,
        "ms",
        false,
        false,
    ),
    p(
        "out",
        "fx.fluctus.out",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
];

/// The key of one field of one point: `p3.y`.
pub fn point_key(point: usize, field: &str) -> String {
    format!("p{point}.{field}")
}

/// The five keys of every point, `&'static`, in point order: `[on, x, y, t, s]`. What the
/// engine reads the curve with, without building a string.
pub fn point_keys() -> &'static [[&'static str; 5]; POINTS] {
    static KEYS: OnceLock<[[&'static str; 5]; POINTS]> = OnceLock::new();
    KEYS.get_or_init(|| {
        let all = params();
        std::array::from_fn(|n| std::array::from_fn(|f| all[GLOBALS.len() + n * 5 + f].key))
    })
}

/// Fluctus's parameters: the globals, then every field of every point. Built once and kept
/// for the life of the program, so the model stays `&'static` like the hand-written tables.
pub fn params() -> &'static [EffectParamInfo] {
    static PARAMS: OnceLock<Vec<EffectParamInfo>> = OnceLock::new();
    PARAMS.get_or_init(|| {
        let mut v = Vec::with_capacity(GLOBALS.len() + POINTS * POINT_FIELDS.len());
        v.extend_from_slice(GLOBALS);
        for n in 0..POINTS {
            let own = DEFAULT_POINTS.iter().find(|(i, _)| *i == n).map(|(_, d)| d);
            for (f, (key, label, min, max, default, unit, log, stepped)) in
                POINT_FIELDS.iter().enumerate()
            {
                let key: &'static str = Box::leak(point_key(n, key).into_boxed_str());
                let default = own.map_or(*default, |d| d[f]);
                v.push(p(key, label, *min, *max, default, unit, *log, *stepped));
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
        assert_eq!(all.len(), GLOBALS.len() + POINTS * 5);
        for (i, a) in all.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            for b in &all[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
    }

    #[test]
    fn point_keys_name_their_fields_and_the_pump_is_drawn() {
        let k = point_keys();
        assert_eq!(k[0], ["p0.on", "p0.x", "p0.y", "p0.t", "p0.s"]);
        assert_eq!(k[15][4], "p15.s");
        let kind = crate::EffectKind::VolumeShaper;
        assert_eq!(kind.param("p0.on").unwrap().default, 1.0);
        assert_eq!(kind.param("p1.x").unwrap().default, 0.5);
        assert_eq!(kind.param("p2.on").unwrap().default, 0.0);
    }
}
