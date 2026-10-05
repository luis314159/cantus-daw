//! Culmen, the mastering maximizer (ADR 0026): its parameter model. Owned by Culmen;
//! nothing else edits this file.
//!
//! ## The shape of it
//!
//! ```text
//! in ─ band stage (up to three bands) ─ full band ─ × drive ─ brickwall (true peak) ─ Δ / 1:1 / match ─ dither ─ out
//! ```
//!
//! - **The band stage**: up to three bands (`lo`, `mid`, `hi`) split at `x1` and `x2`,
//!   with zero-latency dynamic shelves, a minimum-phase split or a linear-phase split
//!   (`xmode`). With two bands the split is at `x1` and the bands are `lo` and `hi`; with
//!   one there is no band stage at all.
//! - **The full band** (`all`): the same dynamics on the whole mix, after the bands —
//!   FL Maximus's *master* band.
//! - Each of the four has a **two-sided curve**: a section above `thr` and one below
//!   `thr_lo`, each with its own ratio and range. A ratio above 1 squeezes, below 1 it
//!   stretches, on either side (Multiplex's rule, the same code):
//!
//!   | Section | ratio > 1 | ratio < 1 |
//!   |---|---|---|
//!   | above `thr` | downward compression | upward expansion |
//!   | below `thr_lo` | upward compression | downward expansion |
//!
//! - **The drive** (`gain`) pushes the result into the **brickwall**: Terminus's
//!   look-ahead, true-peak limiter, its five styles as the release character, its
//!   per-stage stereo link.
//! - **What you hear**: the output, or the delta (everything Culmen changes); at full
//!   level, with the drive undone (1:1), or matched to the input's loudness. Then dither.
//!
//! Keys are forever once shipped; band keys are `{band}.{field}` with the band one of
//! [`BANDS`]. Everything is stored sparsely: a new Culmen stores nothing, and at every
//! default it touches no sample — the signal comes out delayed by the brickwall's
//! look-ahead, exactly, while it stays under the ceiling.

use crate::effect::{p, EffectParamInfo};
use std::sync::OnceLock;

pub const NAME_KEY: &str = "effect.maximizer";
/// Seventy-odd parameters, most never touched: store only what moved.
pub const SPARSE: bool = true;
pub const TAKES_SIDECHAIN: bool = false;

/// The bands' key prefixes, in the order the DSP numbers them: low, mid, high, full.
pub const BANDS: [&str; 4] = ["lo", "mid", "hi", "all"];
/// The index of the full band in [`BANDS`].
pub const FULL: usize = 3;

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

/// The fields of one band. Keys are `{band}.{field}`. Every default leaves the band
/// doing nothing (both ratios 1:1, no gain).
pub const BAND_FIELDS: &[BandField] = &[
    // Off: the band keeps its place and does nothing.
    ("on", "fx.max.on", 0.0, 1.0, 1.0, "", false, true),
    // Hear this band alone (not the full band's: it has nothing to be heard apart from).
    ("solo", "fx.max.solo", 0.0, 1.0, 0.0, "", false, true),
    // The upper section: what happens above `thr`.
    ("thr", "fx.max.thr", -60.0, 0.0, -12.0, "dB", false, false),
    ("ratio", "fx.max.ratio", 0.1, 100.0, 1.0, ":1", true, false),
    ("range", "fx.max.range", 0.0, 60.0, 12.0, "dB", false, false),
    // The lower section: what happens below `thr_lo`.
    (
        "thr_lo",
        "fx.max.thr_lo",
        -90.0,
        0.0,
        -40.0,
        "dB",
        false,
        false,
    ),
    (
        "ratio_lo",
        "fx.max.ratio_lo",
        0.1,
        100.0,
        1.0,
        ":1",
        true,
        false,
    ),
    (
        "range_lo",
        "fx.max.range_lo",
        0.0,
        60.0,
        12.0,
        "dB",
        false,
        false,
    ),
    ("knee", "fx.max.knee", 0.0, 24.0, 6.0, "dB", false, false),
    ("att", "fx.max.att", 0.02, 250.0, 10.0, "ms", true, false),
    ("rel", "fx.max.rel", 5.0, 5_000.0, 150.0, "ms", true, false),
    // The band's own gain, after its dynamics.
    (
        "gain",
        "fx.max.band_gain",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
    // 0 stereo (linked), 1 mid only, 2 side only.
    ("place", "fx.max.place", 0.0, 2.0, 0.0, "", false, true),
];

/// The index of a field in [`BAND_FIELDS`].
pub fn field_index(field: &str) -> Option<usize> {
    BAND_FIELDS.iter().position(|f| f.0 == field)
}

/// Whether a band has this field: the full band has no solo.
pub fn has_field(band: usize, field: usize) -> bool {
    !(band == FULL && BAND_FIELDS[field].0 == "solo")
}

/// The device-wide parameters.
pub const GLOBALS: &[EffectParamInfo] = &[
    // The drive into the ceiling: what *Learn* sets.
    p("gain", "fx.max.gain", -12.0, 30.0, 0.0, "dB", false, false),
    p(
        "ceiling",
        "fx.max.ceiling",
        -30.0,
        0.0,
        -1.0,
        "dB",
        false,
        false,
    ),
    // The brickwall's release character: Terminus's styles (Clean, Punch, Loud, Bus, Safe).
    p("style", "fx.max.style", 0.0, 4.0, 0.0, "", false, true),
    p("look", "fx.max.look", 0.0, 5.0, 2.0, "ms", false, false),
    p(
        "lim_att",
        "fx.max.lim_att",
        0.1,
        10_000.0,
        10.0,
        "ms",
        true,
        false,
    ),
    p(
        "lim_rel",
        "fx.max.lim_rel",
        1.0,
        10_000.0,
        200.0,
        "ms",
        true,
        false,
    ),
    // Stereo link of the brickwall's two stages: transients, and the release.
    p("link_t", "fx.max.link_t", 0.0, 1.0, 0.75, "%", false, false),
    p("link_r", "fx.max.link_r", 0.0, 1.0, 1.0, "%", false, false),
    p(
        "lim_knee",
        "fx.max.lim_knee",
        0.0,
        12.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p("tp", "fx.max.tp", 0.0, 1.0, 1.0, "", false, true),
    p("clip", "fx.max.clip", 0.0, 1.0, 0.0, "%", false, false),
    p("dc", "fx.max.dc", 0.0, 1.0, 0.0, "", false, true),
    // How many bands: 1 is no band stage (the full band alone), 2 splits at `x1`, 3 at
    // `x1` and `x2`.
    p("bands", "fx.max.bands", 1.0, 3.0, 3.0, "", false, true),
    p("x1", "fx.max.x1", 20.0, 20_000.0, 150.0, "Hz", true, false),
    p(
        "x2",
        "fx.max.x2",
        20.0,
        20_000.0,
        5_000.0,
        "Hz",
        true,
        false,
    ),
    // 0 dynamic (no split: zero latency, a wire while nothing moves), 1 minimum-phase
    // split, 2 linear-phase split (latency).
    p("xmode", "fx.max.xmode", 0.0, 2.0, 0.0, "", false, true),
    p(
        "slope",
        "fx.max.slope",
        6.0,
        48.0,
        24.0,
        "dB/oct",
        false,
        true,
    ),
    // Hear what Culmen changes (the input, driven and delayed, minus the output).
    p("delta", "fx.max.delta", 0.0, 1.0, 0.0, "", false, true),
    // 0 the output, 1 the output with the drive undone, 2 at the input's loudness.
    p("monitor", "fx.max.monitor", 0.0, 2.0, 0.0, "", false, true),
    // Off, 24, 22, 20, 18, 16 bits.
    p("dither", "fx.max.dither", 0.0, 5.0, 0.0, "", false, true),
    // Flat, basic, optimized.
    p("shape", "fx.max.shape", 0.0, 2.0, 0.0, "", false, true),
    // The loudness the meters, the verdict and *Learn* aim at.
    p(
        "target",
        "fx.max.target",
        -36.0,
        -5.0,
        -14.0,
        "LUFS",
        false,
        false,
    ),
];

/// The key of one field of one band: `mid.thr`. Allocates; for the UI and for tests. The
/// audio thread reads keys with [`parse_key`] and names them with [`band_key`], neither
/// of which does.
pub fn max_key(band: usize, field: &str) -> String {
    format!("{}.{field}", BANDS[band.min(FULL)])
}

/// The same key as a `&'static str`, from the table built once with the parameters.
pub fn band_key(band: usize, field: usize) -> &'static str {
    keys()[band.min(FULL)][field.min(BAND_FIELDS.len() - 1)]
}

/// `mid.thr` → `(1, index of "thr")`. No allocation.
pub fn parse_key(key: &str) -> Option<(usize, usize)> {
    let (head, field) = key.split_once('.')?;
    let band = BANDS.iter().position(|b| *b == head)?;
    let f = field_index(field)?;
    has_field(band, f).then_some((band, f))
}

fn keys() -> &'static Vec<Vec<&'static str>> {
    static KEYS: OnceLock<Vec<Vec<&'static str>>> = OnceLock::new();
    KEYS.get_or_init(|| {
        (0..BANDS.len())
            .map(|b| {
                BAND_FIELDS
                    .iter()
                    .map(|f| &*Box::leak(max_key(b, f.0).into_boxed_str()))
                    .collect()
            })
            .collect()
    })
}

/// Culmen's parameters: the device-wide ones, then every field of every band. Built once
/// and kept for the life of the program, so the model stays `&'static` like the others.
pub fn params() -> &'static [EffectParamInfo] {
    static PARAMS: OnceLock<Vec<EffectParamInfo>> = OnceLock::new();
    PARAMS.get_or_init(|| {
        let mut v = Vec::with_capacity(GLOBALS.len() + BANDS.len() * BAND_FIELDS.len());
        v.extend_from_slice(GLOBALS);
        for band in 0..BANDS.len() {
            for (i, (_, label, min, max, default, unit, log, stepped)) in
                BAND_FIELDS.iter().enumerate()
            {
                if has_field(band, i) {
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
        }
        v
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EffectKind, EffectSpec};

    #[test]
    fn a_new_culmen_stores_nothing_and_its_keys_are_stable() {
        let mut s = EffectSpec::new(EffectKind::Maximizer);
        assert!(s.params.is_empty(), "a new Culmen writes nothing");
        assert_eq!(
            EffectKind::Maximizer.params().len(),
            GLOBALS.len() + BANDS.len() * BAND_FIELDS.len() - 1
        );
        assert_eq!(s.get("lo.ratio"), 1.0, "every band starts at 1:1");
        assert_eq!(s.get("all.ratio_lo"), 1.0);
        assert_eq!(s.get("ceiling"), -1.0);
        assert_eq!(s.set("mid.thr", 12.0), Some(0.0));
        assert!(
            s.set("all.solo", 1.0).is_none(),
            "the full band has no solo"
        );
        assert!(s.set("top.thr", -10.0).is_none());
        assert_eq!(s.set("slope", 23.6), Some(24.0));
        assert_eq!(s.params.len(), 2);
        assert!(EffectKind::Maximizer.sparse());
        assert!(EffectKind::Maximizer.has_editor());
    }

    #[test]
    fn keys_parse_back_without_formatting() {
        for b in 0..BANDS.len() {
            for (i, f) in BAND_FIELDS.iter().enumerate() {
                let k = band_key(b, i);
                assert_eq!(k, max_key(b, f.0));
                if has_field(b, i) {
                    assert_eq!(parse_key(k), Some((b, i)));
                } else {
                    assert_eq!(parse_key(k), None);
                }
            }
        }
        assert_eq!(parse_key("gain"), None);
        assert_eq!(parse_key("lo.nope"), None);
    }

    #[test]
    fn every_default_is_in_range_and_every_key_unique() {
        let ps = params();
        for (i, a) in ps.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            assert!(!a.logarithmic || a.min > 0.0, "{} is log from 0", a.key);
            for b in &ps[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
    }
}
