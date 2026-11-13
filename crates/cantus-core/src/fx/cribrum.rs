//! Cribrum, the auto filter (ADR 0130, provisional): its parameter model.
//! Owned by Cribrum; nothing else edits this file.
//!
//! Two filters (the synths' zero-delay-feedback models, `cantus_dsp::va::filter`) moved by
//! three sources summed — an LFO, an envelope follower on the input or a sidechain, and a
//! drawn curve synced to the song — each with its depth onto the cutoff, the resonance and
//! the vowel.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the mix starts at
//! 0 (and the oversampling off, so no latency); the rest waits, set to a plain low-pass, for
//! the mix to be turned up (the insert's starting preset does, ADR 0071).
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `mix` | dry/wet | 0 … 100 % | 0 |
//! | `out` | output gain | −24 … +24 dB | 0 |
//! | `route` | the two filters in series or in parallel | serial / parallel | serial |
//! | `spread` | the right cutoff above the left, the left below | 0 … 2 oct | 0 |
//! | `os` | oversampling of the filters (minimum phase) | 1× / 2× / 4× | 1× |
//! | `a.model`, `b.model` | ZDF model: state-variable, ladder, OTA cascade, diode ladder, Sallen-Key | 0 … 4 | state-variable |
//! | `a.mode`, `b.mode` | low-, band-, high-pass, notch, peak, vowel (where the model gives it) | 0 … 5 | low-pass |
//! | `a.slope`, `b.slope` | 12 or 24 dB/oct | 0 / 1 | 12 |
//! | `a.cut`, `b.cut` | cutoff (in vowel mode, the formants' size: 1 kHz is a voice's) | 20 Hz … 20 kHz | 1 kHz |
//! | `a.res`, `b.res` | resonance; the model self-oscillates from 95 % | 0 … 100 % | 25 % |
//! | `a.drive`, `b.drive` | gain into the model's nonlinearities | 0 … 24 dB | 0 |
//! | `a.vowel`, `b.vowel` | A, E, I, O, U and between (vowel mode) | 0 … 4 | A |
//! | `b.on` | the second filter | off / on | off |
//! | `b.track` | how much of the movement the second filter takes (negative: the other way) | −100 … 100 % | 100 % |
//! | `lfo.sync`, `lfo.div`, `lfo.rate` | the LFO on the song (1/64 … 8/1) or free (0.02 … 20 Hz) | | synced, 1/4, 1 Hz |
//! | `lfo.shape` | sine, triangle, square, saw up, saw down, sample & hold, drift | 0 … 6 | sine |
//! | `lfo.edge` | the soft edge of the stepped shapes | 0 … 100 % | 30 % |
//! | `lfo.phase` | where in its cycle it starts | 0 … 360° | 0 |
//! | `lfo.stereo` | the right channel's LFO ahead of the left | 0 … 180° | 0 |
//! | `env.key` | the follower hears the input or the sidechain | input / sidechain | input |
//! | `env.att`, `env.rel` | the follower's times | 0.1 … 500 ms, 1 … 2 000 ms | 5, 150 ms |
//! | `env.gain` | the follower's sensitivity | −24 … +24 dB | 0 |
//! | `cur.div` | the drawn curve's length, on the song | 1/64 … 8/1 | 1/1 |
//! | `cur.smooth` | the curve's smoothing, so a step never clicks | 0 … 50 ms | 2 ms |
//! | `{lfo,env,cur}.cut` | the source's depth onto the cutoffs | −6 … +6 oct | 0 |
//! | `{lfo,env,cur}.res` | …onto the resonances | −100 … +100 % | 0 |
//! | `{lfo,env,cur}.vowel` | …onto the vowels | −4 … +4 | 0 |
//! | `c{n}.on/x/y/t/s` | the curve's points, n = 0 … 15: used, place, level, tension, shape (the automation editor's six) | | c0 and c1: a fall each cycle |
//!
//! A point's number never changes once it exists (as Fluctus's): removing one frees its
//! number, and no other point is renumbered.

use crate::effect::{p, EffectParamInfo};
use std::sync::OnceLock;

pub const NAME_KEY: &str = "effect.cribrum";
/// The curve's eighty point parameters are rarely all used: store only what moved.
pub const SPARSE: bool = true;
/// The envelope follower may hear a sidechain (`env.key`).
pub const TAKES_SIDECHAIN: bool = true;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key; a point's shape by its
/// field (`c.s`, the exported plug-in's group of points).
pub const CHOICES: &[(&str, &str)] = &[
    ("route", "cribrum.route"),
    ("os", "cribrum.os"),
    ("a.model", "cribrum.model"),
    ("b.model", "cribrum.model"),
    ("a.mode", "cribrum.mode"),
    ("b.mode", "cribrum.mode"),
    ("a.slope", "cribrum.slope"),
    ("b.slope", "cribrum.slope"),
    ("b.on", "cribrum.onoff"),
    ("lfo.sync", "modfx.sync"),
    ("lfo.div", "cribrum.div"),
    ("lfo.shape", "cribrum.shape"),
    ("env.key", "cribrum.key"),
    ("cur.div", "cribrum.div"),
    ("c.s", "fluctus.shape"),
];

/// How many points the curve may hold.
pub const POINTS: usize = 16;
/// The synced lengths (`cribrum.div.{i}`): the modulation family's seventeen, 8/1 … 1/32,
/// then 1/32 triplet and 1/64.
pub const DIVISIONS: usize = 19;
/// 1/4 and 1/1 in that list.
pub const DIV_QUARTER: f32 = 8.0;
pub const DIV_WHOLE: f32 = 3.0;

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
    ("on", "fx.cribrum.c.on", 0.0, 1.0, 0.0, "", false, true),
    ("x", "fx.cribrum.c.x", 0.0, 1.0, 0.0, "%", false, false),
    ("y", "fx.cribrum.c.y", 0.0, 1.0, 1.0, "%", false, false),
    ("t", "fx.cribrum.c.t", -1.0, 1.0, 0.0, "", false, false),
    ("s", "fx.cribrum.c.s", 0.0, 5.0, 0.0, "", false, true),
];

/// The default curve: full at the start of each cycle, falling fast (a negative tension)
/// to nothing by nine tenths of it, then back up — a pluck on every cycle.
const DEFAULT_POINTS: [(usize, [f32; 5]); 2] = [
    (0, [1.0, 0.0, 1.0, -0.6, 0.0]),
    (1, [1.0, 0.9, 0.0, 0.0, 0.0]),
];

/// One filter's parameters, under its prefix (`a` or `b`).
macro_rules! filter_params {
    ($pre:literal) => {
        [
            p(
                concat!($pre, ".model"),
                "fx.cribrum.model",
                0.0,
                4.0,
                0.0,
                "",
                false,
                true,
            ),
            p(
                concat!($pre, ".mode"),
                "fx.cribrum.mode",
                0.0,
                5.0,
                0.0,
                "",
                false,
                true,
            ),
            p(
                concat!($pre, ".slope"),
                "fx.cribrum.slope",
                0.0,
                1.0,
                0.0,
                "",
                false,
                true,
            ),
            p(
                concat!($pre, ".cut"),
                "fx.cribrum.cut",
                20.0,
                20_000.0,
                1_000.0,
                "Hz",
                true,
                false,
            ),
            p(
                concat!($pre, ".res"),
                "fx.cribrum.res",
                0.0,
                1.0,
                0.25,
                "%",
                false,
                false,
            ),
            p(
                concat!($pre, ".drive"),
                "fx.cribrum.drive",
                0.0,
                24.0,
                0.0,
                "dB",
                false,
                false,
            ),
            p(
                concat!($pre, ".vowel"),
                "fx.cribrum.vowel",
                0.0,
                4.0,
                0.0,
                "",
                false,
                false,
            ),
        ]
    };
}

/// One source's three depths, under its prefix.
macro_rules! depths {
    ($pre:literal) => {
        [
            p(
                concat!($pre, ".cut"),
                "fx.cribrum.depth.cut",
                -6.0,
                6.0,
                0.0,
                "oct",
                false,
                false,
            ),
            p(
                concat!($pre, ".res"),
                "fx.cribrum.depth.res",
                -1.0,
                1.0,
                0.0,
                "%",
                false,
                false,
            ),
            p(
                concat!($pre, ".vowel"),
                "fx.cribrum.depth.vowel",
                -4.0,
                4.0,
                0.0,
                "",
                false,
                false,
            ),
        ]
    };
}

const GLOBAL: [EffectParamInfo; 5] = [
    p("mix", "fx.cribrum.mix", 0.0, 1.0, 0.0, "%", false, false),
    p(
        "out",
        "fx.cribrum.out",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p("route", "fx.cribrum.route", 0.0, 1.0, 0.0, "", false, true),
    p(
        "spread",
        "fx.cribrum.spread",
        0.0,
        2.0,
        0.0,
        "oct",
        false,
        false,
    ),
    p("os", "fx.cribrum.os", 0.0, 2.0, 0.0, "", false, true),
];

const FILTER_A: [EffectParamInfo; 7] = filter_params!("a");
const FILTER_B: [EffectParamInfo; 7] = filter_params!("b");

const FILTER_B_MORE: [EffectParamInfo; 2] = [
    p("b.on", "fx.cribrum.b.on", 0.0, 1.0, 0.0, "", false, true),
    p(
        "b.track",
        "fx.cribrum.b.track",
        -1.0,
        1.0,
        1.0,
        "%",
        false,
        false,
    ),
];

const LFO: [EffectParamInfo; 7] = [
    p("lfo.sync", "modfx.p.sync", 0.0, 1.0, 1.0, "", false, true),
    p(
        "lfo.div",
        "modfx.p.div",
        0.0,
        (DIVISIONS - 1) as f32,
        DIV_QUARTER,
        "",
        false,
        true,
    ),
    p(
        "lfo.rate",
        "modfx.p.rate",
        0.02,
        20.0,
        1.0,
        "Hz",
        true,
        false,
    ),
    p("lfo.shape", "modfx.p.shape", 0.0, 6.0, 0.0, "", false, true),
    p("lfo.edge", "modfx.p.edge", 0.0, 1.0, 0.3, "%", false, false),
    p(
        "lfo.phase",
        "fx.cribrum.lfo.phase",
        0.0,
        360.0,
        0.0,
        "\u{b0}",
        false,
        false,
    ),
    p(
        "lfo.stereo",
        "fx.cribrum.lfo.stereo",
        0.0,
        180.0,
        0.0,
        "\u{b0}",
        false,
        false,
    ),
];
const LFO_DEPTHS: [EffectParamInfo; 3] = depths!("lfo");

const ENV: [EffectParamInfo; 4] = [
    p(
        "env.key",
        "fx.cribrum.env.key",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "env.att",
        "fx.cribrum.env.att",
        0.1,
        500.0,
        5.0,
        "ms",
        true,
        false,
    ),
    p(
        "env.rel",
        "fx.cribrum.env.rel",
        1.0,
        2_000.0,
        150.0,
        "ms",
        true,
        false,
    ),
    p(
        "env.gain",
        "fx.cribrum.env.gain",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
];
const ENV_DEPTHS: [EffectParamInfo; 3] = depths!("env");

const CURVE: [EffectParamInfo; 2] = [
    p(
        "cur.div",
        "fx.cribrum.cur.div",
        0.0,
        (DIVISIONS - 1) as f32,
        DIV_WHOLE,
        "",
        false,
        true,
    ),
    p(
        "cur.smooth",
        "fx.cribrum.cur.smooth",
        0.0,
        50.0,
        2.0,
        "ms",
        false,
        false,
    ),
];
const CURVE_DEPTHS: [EffectParamInfo; 3] = depths!("cur");

/// The parameters that are not a point's, in the table's order.
pub fn globals() -> impl Iterator<Item = &'static EffectParamInfo> {
    GLOBAL
        .iter()
        .chain(FILTER_A.iter())
        .chain(FILTER_B.iter())
        .chain(FILTER_B_MORE.iter())
        .chain(LFO.iter())
        .chain(LFO_DEPTHS.iter())
        .chain(ENV.iter())
        .chain(ENV_DEPTHS.iter())
        .chain(CURVE.iter())
        .chain(CURVE_DEPTHS.iter())
}

/// How many parameters are not a point's.
pub const GLOBALS: usize = 5 + 7 + 7 + 2 + 7 + 3 + 4 + 3 + 2 + 3;

/// The key of one field of one point: `c3.y`.
pub fn point_key(point: usize, field: &str) -> String {
    format!("c{point}.{field}")
}

/// The five keys of every point, `&'static`, in point order: `[on, x, y, t, s]`. What the
/// engine reads the curve with, without building a string.
pub fn point_keys() -> &'static [[&'static str; 5]; POINTS] {
    static KEYS: OnceLock<[[&'static str; 5]; POINTS]> = OnceLock::new();
    KEYS.get_or_init(|| {
        let all = params();
        std::array::from_fn(|n| std::array::from_fn(|f| all[GLOBALS + n * 5 + f].key))
    })
}

/// Cribrum's parameters: the globals, then every field of every point. Built once and kept
/// for the life of the program, so the model stays `&'static` like the hand-written tables.
pub fn params() -> &'static [EffectParamInfo] {
    static PARAMS: OnceLock<Vec<EffectParamInfo>> = OnceLock::new();
    PARAMS.get_or_init(|| {
        let mut v = Vec::with_capacity(GLOBALS + POINTS * POINT_FIELDS.len());
        v.extend(globals().copied());
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
        assert_eq!(globals().count(), GLOBALS);
        assert_eq!(all.len(), GLOBALS + POINTS * 5);
        for (i, a) in all.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            for b in &all[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
    }

    /// The null rule's keys: nothing is mixed in, nothing added to the gain, no movement,
    /// and the oversampling (which would add latency) off.
    #[test]
    fn the_defaults_are_a_null() {
        let kind = crate::EffectKind::AutoFilter;
        assert_eq!(kind.param("mix").unwrap().default, 0.0);
        assert_eq!(kind.param("out").unwrap().default, 0.0);
        assert_eq!(kind.param("os").unwrap().default, 0.0);
        for src in ["lfo", "env", "cur"] {
            for d in ["cut", "res", "vowel"] {
                assert_eq!(kind.param(&format!("{src}.{d}")).unwrap().default, 0.0);
            }
        }
    }

    #[test]
    fn point_keys_name_their_fields_and_the_pluck_is_drawn() {
        let k = point_keys();
        assert_eq!(k[0], ["c0.on", "c0.x", "c0.y", "c0.t", "c0.s"]);
        assert_eq!(k[15][4], "c15.s");
        let kind = crate::EffectKind::AutoFilter;
        assert_eq!(kind.param("c0.on").unwrap().default, 1.0);
        assert_eq!(kind.param("c1.x").unwrap().default, 0.9);
        assert_eq!(kind.param("c2.on").unwrap().default, 0.0);
    }

    /// Every choice list but the points' names a stepped key of the table.
    #[test]
    fn every_choice_list_names_a_stepped_key() {
        for (key, _) in CHOICES {
            if *key == "c.s" {
                continue;
            }
            let info = params().iter().find(|p| p.key == *key).expect(key);
            assert!(info.stepped, "{key}");
        }
    }
}
