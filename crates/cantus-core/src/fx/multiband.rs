//! Multiplex, the multiband dynamics: its parameter model. Owned by the multiband; nothing
//! else edits this file.
//!
//! ## The shape of it
//!
//! Up to [`MB_BANDS`] bands that cover the spectrum edge to edge, each with a **stable
//! number** (`b0` … `b7`): a band keeps its number for life, so an automation lane or a
//! script that names `b3.thr` names the same band forever, whatever is added or removed
//! around it (ADR 0017's rule, the same one Aequalis keeps).
//!
//! A band's place is its `from`: the frequency it starts at. The used bands, sorted by
//! `from`, tile the spectrum — each one runs from its own `from` to the next band's — so
//! the crossovers are exactly the `from` of every band but the lowest (whose `from` is
//! kept but means nothing while it is the lowest). Adding a band splits one; removing one
//! gives its range to the band below it.
//!
//! Each band has **two-sided** dynamics, one section above an upper threshold and one
//! below a lower one, each with its own ratio and range. A ratio above 1 squeezes (it
//! compresses), below 1 it stretches (it expands), on either side:
//!
//! | Section | ratio > 1 | ratio < 1 |
//! |---|---|---|
//! | above `thr` | downward compression | upward expansion |
//! | below `thr_lo` | upward compression | downward expansion (a gate, at a low ratio) |
//!
//! That is the Ableton / OTT / Multiplicity model the benchmark's users ask for
//! (`research/fabfilter/pro-mb.md` §7 and §9 P0-5) in two numbers per side.
//!
//! Keys are forever once shipped. Everything here is stored sparsely: a new Multiplex
//! stores nothing and is a wire.

use crate::effect::{p, EffectParamInfo};
use std::sync::OnceLock;

pub const NAME_KEY: &str = "effect.multiband";
/// A band has twenty-odd fields and most are never touched: store only what moved.
pub const SPARSE: bool = true;
/// A band's detector can listen to the slot's sidechain ([`crate::EffectSlot::sidechain`]).
pub const TAKES_SIDECHAIN: bool = true;

/// How many bands Multiplex has. Eight bands is seven crossovers.
pub const MB_BANDS: usize = 8;

/// One field of a band: key, label key, min, max, default, unit, logarithmic, stepped.
pub type MbField = (
    &'static str,
    &'static str,
    f32,
    f32,
    f32,
    &'static str,
    bool,
    bool,
);

/// The fields of one band. Keys are `b{n}.{field}`.
pub const MB_BAND_FIELDS: &[MbField] = &[
    // key, label, min, max, default, unit, log, stepped
    // The band exists (and so has a place in the spectrum).
    ("used", "fx.mb.used", 0.0, 1.0, 0.0, "", false, true),
    // Where the band starts. The lowest band's is kept but not used.
    (
        "from",
        "fx.mb.from",
        20.0,
        20_000.0,
        1_000.0,
        "Hz",
        true,
        false,
    ),
    // Off: the band keeps its place and does nothing (no dynamics, no gain).
    ("on", "fx.mb.on", 0.0, 1.0, 1.0, "", false, true),
    ("solo", "fx.mb.solo", 0.0, 1.0, 0.0, "", false, true),
    ("mute", "fx.mb.mute", 0.0, 1.0, 0.0, "", false, true),
    // The upper section: what happens above `thr`.
    ("thr", "fx.mb.thr", -80.0, 0.0, -24.0, "dB", false, false),
    ("ratio", "fx.mb.ratio", 0.1, 100.0, 2.0, ":1", true, false),
    ("range", "fx.mb.range", 0.0, 60.0, 12.0, "dB", false, false),
    // The lower section: what happens below `thr_lo`. Off (1:1) until asked for.
    (
        "thr_lo",
        "fx.mb.thr_lo",
        -90.0,
        0.0,
        -60.0,
        "dB",
        false,
        false,
    ),
    (
        "ratio_lo",
        "fx.mb.ratio_lo",
        0.1,
        100.0,
        1.0,
        ":1",
        true,
        false,
    ),
    (
        "range_lo",
        "fx.mb.range_lo",
        0.0,
        60.0,
        12.0,
        "dB",
        false,
        false,
    ),
    ("knee", "fx.mb.knee", 0.0, 24.0, 6.0, "dB", false, false),
    ("att", "fx.mb.att", 0.02, 250.0, 10.0, "ms", true, false),
    ("rel", "fx.mb.rel", 5.0, 5_000.0, 150.0, "ms", true, false),
    // Lookahead; the device's latency is the largest of its bands'.
    ("la", "fx.mb.la", 0.0, 20.0, 0.0, "ms", false, false),
    // The band's own static gain (make-up).
    ("gain", "fx.mb.gain", -24.0, 24.0, 0.0, "dB", false, false),
    // 0 stereo (linked), 1 mid only, 2 side only.
    ("place", "fx.mb.place", 0.0, 2.0, 0.0, "", false, true),
    // 0 the band's own input, 1 the slot's sidechain.
    ("key", "fx.mb.key", 0.0, 1.0, 0.0, "", false, true),
    // 0 the detector hears the band's own range, 1 the free range `key_lo`…`key_hi`.
    ("key_free", "fx.mb.key_free", 0.0, 1.0, 0.0, "", false, true),
    (
        "key_lo",
        "fx.mb.key_lo",
        20.0,
        20_000.0,
        200.0,
        "Hz",
        true,
        false,
    ),
    (
        "key_hi",
        "fx.mb.key_hi",
        20.0,
        20_000.0,
        4_000.0,
        "Hz",
        true,
        false,
    ),
];

/// The device-wide parameters.
pub const MB_GLOBALS: &[EffectParamInfo] = &[
    // 0 dynamic (no split: zero latency, a wire while nothing moves), 1 minimum-phase split,
    // 2 linear-phase split (latency).
    p("mode", "fx.mb.mode", 0.0, 2.0, 0.0, "", false, true),
    // Crossover slope, dB/octave: 6, 12, 24 or 48 (anything else rounds to the nearest).
    p(
        "slope",
        "fx.mb.slope",
        6.0,
        48.0,
        24.0,
        "dB/oct",
        false,
        true,
    ),
    p("out", "fx.mb.out", -24.0, 24.0, 0.0, "dB", false, false),
    // 0 the output, 1 the delta (what the dynamics change), 2 the detectors' key.
    p("listen", "fx.mb.listen", 0.0, 2.0, 0.0, "", false, true),
];

/// The key of one field of one band: `b3.thr`. Allocates; for the UI and for tests. The
/// audio thread reads keys with [`parse_key`] and names them with [`band_key`], neither of
/// which does.
pub fn mb_key(band: usize, field: &str) -> String {
    format!("b{band}.{field}")
}

/// The same key as a `&'static str`, from the table built once with the parameters — so
/// the audio thread can look a band field up without formatting a string.
pub fn band_key(band: usize, field: usize) -> &'static str {
    keys()[band.min(MB_BANDS - 1)][field.min(MB_BAND_FIELDS.len() - 1)]
}

/// The index of a field in [`MB_BAND_FIELDS`].
pub fn field_index(field: &str) -> Option<usize> {
    MB_BAND_FIELDS.iter().position(|f| f.0 == field)
}

/// `b3.thr` → `(3, index of "thr")`. No allocation.
pub fn parse_key(key: &str) -> Option<(usize, usize)> {
    let rest = key.strip_prefix('b')?;
    let (n, field) = rest.split_once('.')?;
    let band: usize = n.parse().ok()?;
    if band >= MB_BANDS {
        return None;
    }
    Some((band, field_index(field)?))
}

fn keys() -> &'static Vec<Vec<&'static str>> {
    static KEYS: OnceLock<Vec<Vec<&'static str>>> = OnceLock::new();
    KEYS.get_or_init(|| {
        (0..MB_BANDS)
            .map(|b| {
                MB_BAND_FIELDS
                    .iter()
                    .map(|f| &*Box::leak(mb_key(b, f.0).into_boxed_str()))
                    .collect()
            })
            .collect()
    })
}

/// Multiplex's parameters: every field of every band, then the device-wide ones. Built once
/// and kept for the life of the program, so the model stays `&'static` like the others.
pub fn params() -> &'static [EffectParamInfo] {
    static PARAMS: OnceLock<Vec<EffectParamInfo>> = OnceLock::new();
    PARAMS.get_or_init(|| {
        let mut v = Vec::with_capacity(MB_BANDS * MB_BAND_FIELDS.len() + MB_GLOBALS.len());
        for band in 0..MB_BANDS {
            for (i, (_, label, min, max, default, unit, log, stepped)) in
                MB_BAND_FIELDS.iter().enumerate()
            {
                v.push(p(
                    band_key(band, i),
                    label,
                    *min,
                    *max,
                    *default,
                    unit,
                    *log,
                    *stepped,
                ));
            }
        }
        v.extend_from_slice(MB_GLOBALS);
        v
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EffectKind, EffectSpec};

    #[test]
    fn bands_are_named_stably_and_a_new_one_stores_nothing() {
        let mut s = EffectSpec::new(EffectKind::Multiband);
        assert!(s.params.is_empty(), "a new Multiplex writes nothing");
        assert_eq!(
            EffectKind::Multiband.params().len(),
            MB_BANDS * MB_BAND_FIELDS.len() + MB_GLOBALS.len()
        );
        assert_eq!(s.get("b7.ratio_lo"), 1.0, "the lower section starts off");
        assert_eq!(s.set("b2.thr", 12.0), Some(0.0));
        assert!(s.set("b8.thr", -10.0).is_none(), "eight bands, 0 to 7");
        assert_eq!(s.set("slope", 23.6), Some(24.0));
        assert_eq!(s.params.len(), 2);
        assert!(EffectKind::Multiband.takes_sidechain());
        assert!(EffectKind::Multiband.sparse());
    }

    #[test]
    fn keys_parse_back_without_formatting() {
        for b in 0..MB_BANDS {
            for (i, f) in MB_BAND_FIELDS.iter().enumerate() {
                let k = band_key(b, i);
                assert_eq!(k, mb_key(b, f.0));
                assert_eq!(parse_key(k), Some((b, i)));
            }
        }
        assert_eq!(parse_key("b9.thr"), None);
        assert_eq!(parse_key("b1.nope"), None);
        assert_eq!(parse_key("out"), None);
    }
}
