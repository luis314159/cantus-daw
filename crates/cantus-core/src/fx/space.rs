//! Spatium, the reverb: its parameter model. Owned by the reverb.
//!
//! Keys are forever once shipped (automation and saved projects hold them), so they are
//! short and plain:
//!
//! - **The macros**: `style`, `space`, `decay` (seconds — the mid-band RT60), `predelay`
//!   (ms) with `sync` (the note value it was set from, 0 for none), `distance`,
//!   `character`, `bright`, `width`, `mix`, `freeze`.
//! - **Early reflections**: `er` (a level offset in dB; −30 is off) and `er_size`.
//! - **Ducking**: `duck` (the range, dB; 0 is off), `duck_thr`, `duck_att`, `duck_rel`.
//!   The key is the slot's sidechain when one is routed, the input otherwise.
//! - **The decay curve**: six bands `d{n}.{used,on,shape,freq,mult,q}` — a band's
//!   "gain" is a decay multiplier (×0.25 … ×4) — and `deq_on` to bypass the curve.
//! - **The post EQ**: six bands `e{n}.{used,on,shape,freq,gain,q,slope,place}`, `peq_on`,
//!   and `peq_auto` (gain compensation).
//!
//! A band's number never changes: removing band 2 frees `d2.*` and renumbers nothing.
//! Sparse: a new reverb stores nothing, and the file holds only what moved.

use crate::effect::{p, EffectParamInfo};
use std::sync::OnceLock;

pub const NAME_KEY: &str = "effect.space";
pub const SPARSE: bool = true;
pub const TAKES_SIDECHAIN: bool = true;

/// Bands on the decay curve and on the post EQ.
pub const DECAY_BANDS: usize = 6;
pub const POST_BANDS: usize = 6;

/// One field of a band: key, label key, min, max, default, unit, logarithmic, stepped.
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

/// A decay band. `shape`: 0 bell, 1 low shelf, 2 high shelf, 3 notch.
pub const DECAY_FIELDS: &[BandField] = &[
    ("used", "fx.space.band.used", 0.0, 1.0, 0.0, "", false, true),
    ("on", "fx.space.band.on", 0.0, 1.0, 1.0, "", false, true),
    (
        "shape",
        "fx.space.band.shape",
        0.0,
        3.0,
        0.0,
        "",
        false,
        true,
    ),
    (
        "freq",
        "fx.space.band.freq",
        20.0,
        20_000.0,
        1_000.0,
        "Hz",
        true,
        false,
    ),
    (
        "mult",
        "fx.space.band.mult",
        0.25,
        4.0,
        1.0,
        "x",
        true,
        false,
    ),
    ("q", "fx.space.band.q", 0.1, 10.0, 0.7, "", true, false),
];

/// A post-EQ band. `shape`: 0 bell, 1 low shelf, 2 high shelf, 3 low cut, 4 high cut.
/// `place`: 0 stereo, 1 left, 2 right, 3 mid, 4 side.
pub const POST_FIELDS: &[BandField] = &[
    ("used", "fx.space.band.used", 0.0, 1.0, 0.0, "", false, true),
    ("on", "fx.space.band.on", 0.0, 1.0, 1.0, "", false, true),
    (
        "shape",
        "fx.space.band.shape",
        0.0,
        4.0,
        0.0,
        "",
        false,
        true,
    ),
    (
        "freq",
        "fx.space.band.freq",
        20.0,
        20_000.0,
        1_000.0,
        "Hz",
        true,
        false,
    ),
    (
        "gain",
        "fx.space.band.gain",
        -18.0,
        18.0,
        0.0,
        "dB",
        false,
        false,
    ),
    ("q", "fx.space.band.q", 0.1, 10.0, 0.7, "", true, false),
    (
        "slope",
        "fx.space.band.slope",
        6.0,
        96.0,
        24.0,
        "dB/oct",
        false,
        true,
    ),
    (
        "place",
        "fx.space.band.place",
        0.0,
        4.0,
        0.0,
        "",
        false,
        true,
    ),
];

/// The key of one field of one decay band: `d2.mult`.
pub fn decay_key(band: usize, field: &str) -> String {
    format!("d{band}.{field}")
}

/// The key of one field of one post-EQ band: `e2.gain`.
pub fn post_key(band: usize, field: &str) -> String {
    format!("e{band}.{field}")
}

/// The macros, the reflections, the ducking and the switches.
const MAIN: &[EffectParamInfo] = &[
    p("style", "fx.space.style", 0.0, 3.0, 0.0, "", false, true),
    p("space", "fx.space.space", 0.0, 1.0, 0.5, "", false, false),
    p("decay", "fx.space.decay", 0.1, 30.0, 1.6, "s", true, false),
    p(
        "predelay",
        "fx.space.predelay",
        0.0,
        500.0,
        8.0,
        "ms",
        false,
        false,
    ),
    p("sync", "fx.space.sync", 0.0, 11.0, 0.0, "", false, true),
    p(
        "distance",
        "fx.space.distance",
        0.0,
        1.0,
        0.35,
        "%",
        false,
        false,
    ),
    p(
        "character",
        "fx.space.character",
        0.0,
        1.0,
        0.3,
        "%",
        false,
        false,
    ),
    p(
        "bright",
        "fx.space.bright",
        0.0,
        1.0,
        0.45,
        "%",
        false,
        false,
    ),
    p("width", "fx.space.width", 0.0, 1.2, 1.0, "%", false, false),
    p("mix", "fx.space.mix", 0.0, 1.0, 0.25, "%", false, false),
    p("freeze", "fx.space.freeze", 0.0, 1.0, 0.0, "", false, true),
    p("er", "fx.space.er", -30.0, 12.0, 0.0, "dB", false, false),
    p(
        "er_size",
        "fx.space.er_size",
        0.25,
        2.0,
        1.0,
        "x",
        true,
        false,
    ),
    p("duck", "fx.space.duck", 0.0, 48.0, 0.0, "dB", false, false),
    p(
        "duck_thr",
        "fx.space.duck_thr",
        -60.0,
        0.0,
        -30.0,
        "dB",
        false,
        false,
    ),
    p(
        "duck_att",
        "fx.space.duck_att",
        0.5,
        200.0,
        10.0,
        "ms",
        true,
        false,
    ),
    p(
        "duck_rel",
        "fx.space.duck_rel",
        20.0,
        3_000.0,
        400.0,
        "ms",
        true,
        false,
    ),
    p("deq_on", "fx.space.deq_on", 0.0, 1.0, 1.0, "", false, true),
    p("peq_on", "fx.space.peq_on", 0.0, 1.0, 1.0, "", false, true),
    p(
        "peq_auto",
        "fx.space.peq_auto",
        0.0,
        1.0,
        1.0,
        "",
        false,
        true,
    ),
];

/// Every parameter: the main table, then the decay bands, then the post-EQ bands. Built
/// once and kept, so the model stays `&'static` like the hand-written tables.
pub fn params() -> &'static [EffectParamInfo] {
    static PARAMS: OnceLock<Vec<EffectParamInfo>> = OnceLock::new();
    PARAMS.get_or_init(|| {
        let mut v = MAIN.to_vec();
        for band in 0..DECAY_BANDS {
            for (key, label, min, max, default, unit, log, stepped) in DECAY_FIELDS {
                let key: &'static str = Box::leak(decay_key(band, key).into_boxed_str());
                v.push(p(key, label, *min, *max, *default, unit, *log, *stepped));
            }
        }
        for band in 0..POST_BANDS {
            for (key, label, min, max, default, unit, log, stepped) in POST_FIELDS {
                let key: &'static str = Box::leak(post_key(band, key).into_boxed_str());
                v.push(p(key, label, *min, *max, *default, unit, *log, *stepped));
            }
        }
        v
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EffectKind, EffectSpec};

    #[test]
    fn a_new_reverb_stores_nothing_and_reads_its_defaults() {
        let s = EffectSpec::new(EffectKind::Space);
        assert!(s.params.is_empty());
        assert_eq!(s.get("decay"), 1.6);
        assert_eq!(s.get("d5.mult"), 1.0);
        assert_eq!(s.get("e0.slope"), 24.0);
        assert_eq!(
            params().len(),
            MAIN.len() + DECAY_BANDS * DECAY_FIELDS.len() + POST_BANDS * POST_FIELDS.len()
        );
        let mut keys: Vec<&str> = params().iter().map(|i| i.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), params().len(), "every key is unique");
    }
}
