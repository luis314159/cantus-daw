//! Caelum, the shimmer reverb (ADR 0138): its parameter model. Owned by the
//! wave-A builder; the DSP is `cantus_dsp::caelum`, whose report (`docs/round-fx/caelum.md`)
//! says why it is built as it is.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the mix at 0 —
//! the dry at unity and nothing else — over a musical space, so turning the mix up is
//! heard at once. `out` at 0 dB is not applied.
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `size` | the tank's lines, 15 → 320 ms on average | 0 … 100 % |
//! | `decay` | the middle band's RT60 | 0.3 … 60 s |
//! | `inf` | infinite: the tail held at the level it has; what comes in layers under a soft ceiling (a tail of the set decay's level) | 0 … 1 |
//! | `isus` | Sustain: infinite's wash once nothing more comes in — from carried away by the shimmer's climb (0) to held for ever, its tone kept (100 %, the default) | 0 … 100 % |
//! | `pre`, `psync` | the pre-delay, or a note value at the song's tempo (0: free) | 0 … 1 000 ms, 0 … 8 |
//! | `diff` | the diffusion | 0 … 100 % |
//! | `hhz`, `hdec` | the high damping's corner and the highs' decay against the middle's | 1 … 20 kHz, 0.1 … 1 × |
//! | `lhz`, `ldec` | the low damping's corner and the lows' decay against the middle's | 50 … 1 000 Hz, 0.25 … 4 × |
//! | `mdepth`, `mrate` | the lines' modulation | 0 … 100 %, 0.02 … 10 Hz |
//! | `shim` | the shimmer's amount | 0 … 100 % |
//! | `pa`, `pb`, `pblvl` | the two voices' intervals and the second's level | −36 … +36 st, 0 … 100 % |
//! | `slo`, `shi` | the tone in the shimmer's loop (20 Hz and 20 kHz are off) | 20 … 2 000 Hz, 1 … 20 kHz |
//! | `bloom` | the swell's time (0: off) | 0 … 4 000 ms |
//! | `freeze` | the tank holds what it has and takes nothing | 0 … 1 |
//! | `duck`, `dthr`, `datt`, `drel` | ducking: range (0: off), threshold, attack, release | 0 … 48 dB, −60 … 0 dB, 0.1 … 500 ms, 10 … 5 000 ms |
//! | `lcut`, `hcut` | the wet's tone (20 Hz and 20 kHz are off) | 20 … 1 000 Hz, 1 … 20 kHz |
//! | `width` | mono … as made … wider | 0 … 150 % |
//! | `lines` | eight lines (lighter) or sixteen | 0 … 1 |
//! | `mix` | dry to wet (the dry at unity to half way) | 0 … 100 % |
//! | `out` | the output level | −24 … +24 dB |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.caelum";
pub const SPARSE: bool = false;
/// Ducking can listen to another strip (the key), as Spatium's and the compressors' do.
pub const TAKES_SIDECHAIN: bool = true;
pub const TAKES_NOTES: bool = false;

/// The pre-delay's note values, in `psync`'s order from 1 (0 is free), in quarter notes.
pub const SYNC_BEATS: [f32; 8] = [0.125, 0.25, 1.0 / 3.0, 0.5, 0.75, 1.0, 1.5, 2.0];

/// The locale prefix of each stepped parameter's choices, by key (ADR 0020).
pub const CHOICES: &[(&str, &str)] = &[
    ("inf", "fx.caelum.inf"),
    ("psync", "fx.caelum.psync"),
    ("freeze", "fx.caelum.freeze"),
    ("lines", "fx.caelum.lines"),
];

const fn amount(key: &'static str, label: &'static str, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, 1.0, default, "%", false, false)
}

const fn switch(key: &'static str, label: &'static str, max: f32, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, max, default, "", false, true)
}

const fn hz(key: &'static str, label: &'static str, lo: f32, hi: f32, d: f32) -> EffectParamInfo {
    p(key, label, lo, hi, d, "Hz", true, false)
}

const fn ms(key: &'static str, label: &'static str, lo: f32, hi: f32, d: f32) -> EffectParamInfo {
    p(key, label, lo, hi, d, "ms", true, false)
}

pub const PARAMS: &[EffectParamInfo] = &[
    // A host's list and the display's first row: the space, then the sky.
    amount("mix", "fx.caelum.mix", 0.0),
    p("decay", "fx.caelum.decay", 0.3, 60.0, 4.0, "s", true, false),
    amount("size", "fx.caelum.size", 0.6),
    amount("shim", "fx.caelum.shim", 0.0),
    p("pa", "fx.caelum.pa", -36.0, 36.0, 12.0, "st", false, true),
    p("pb", "fx.caelum.pb", -36.0, 36.0, 7.0, "st", false, true),
    amount("pblvl", "fx.caelum.pblvl", 0.0),
    switch("inf", "fx.caelum.inf", 1.0, 0.0),
    switch("freeze", "fx.caelum.freeze", 1.0, 0.0),
    p(
        "bloom",
        "fx.caelum.bloom",
        0.0,
        4_000.0,
        0.0,
        "ms",
        false,
        false,
    ),
    p(
        "pre",
        "fx.caelum.pre",
        0.0,
        1_000.0,
        20.0,
        "ms",
        false,
        false,
    ),
    switch("psync", "fx.caelum.psync", 8.0, 0.0),
    amount("diff", "fx.caelum.diff", 0.75),
    hz("hhz", "fx.caelum.hhz", 1_000.0, 20_000.0, 6_000.0),
    p(
        "hdec",
        "fx.caelum.hdec",
        0.1,
        1.0,
        0.5,
        "\u{d7}",
        false,
        false,
    ),
    hz("lhz", "fx.caelum.lhz", 50.0, 1_000.0, 250.0),
    p(
        "ldec",
        "fx.caelum.ldec",
        0.25,
        4.0,
        1.0,
        "\u{d7}",
        true,
        false,
    ),
    amount("mdepth", "fx.caelum.mdepth", 0.3),
    hz("mrate", "fx.caelum.mrate", 0.02, 10.0, 0.4),
    hz("slo", "fx.caelum.slo", 20.0, 2_000.0, 300.0),
    hz("shi", "fx.caelum.shi", 1_000.0, 20_000.0, 9_000.0),
    amount("isus", "fx.caelum.isus", 1.0),
    p("duck", "fx.caelum.duck", 0.0, 48.0, 0.0, "dB", false, false),
    p(
        "dthr",
        "fx.caelum.dthr",
        -60.0,
        0.0,
        -30.0,
        "dB",
        false,
        false,
    ),
    ms("datt", "fx.caelum.datt", 0.1, 500.0, 10.0),
    ms("drel", "fx.caelum.drel", 10.0, 5_000.0, 400.0),
    hz("lcut", "fx.caelum.lcut", 20.0, 1_000.0, 20.0),
    hz("hcut", "fx.caelum.hcut", 1_000.0, 20_000.0, 20_000.0),
    p("width", "fx.caelum.width", 0.0, 1.5, 1.0, "%", false, false),
    switch("lines", "fx.caelum.lines", 1.0, 1.0),
    p("out", "fx.caelum.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

/// The keys wave A's first Caelum held and the amendment of ADR 0138 retired (never
/// released): `ishim` (infinite with the shimmer: 0 fades, 1 sustains) and `ilevel`
/// (infinite 8 dB lower, or as loud).
pub const RETIRED: [&str; 2] = ["ishim", "ilevel"];

/// A Caelum spec written before the amendment, made today's: `ishim` becomes the Sustain
/// it chose (fades 0 %, sustains 100 %); `ilevel` is dropped (engaging Infinite no longer
/// moves the level, so neither choice has a meaning). `true` when it changed.
pub fn repair_spec(spec: &mut crate::EffectSpec) -> bool {
    if spec.kind != crate::EffectKind::Shimmer {
        return false;
    }
    let mut changed = false;
    if let Some(v) = spec.params.remove("ishim") {
        spec.params
            .insert("isus".to_string(), if v >= 0.5 { 1.0 } else { 0.0 });
        changed = true;
    }
    changed |= spec.params.remove("ilevel").is_some();
    changed
}

/// [`repair_spec`] over every insert of a project and every pedal on a board. The number
/// of specs changed.
pub fn repair_project(project: &mut crate::model::Project) -> usize {
    let mut changed = 0;
    for e in project.all_effects_mut() {
        if repair_spec(&mut e.spec) {
            changed += 1;
        }
        if let Some(board) = e.spec.board.as_deref_mut() {
            for pedal in &mut board.pedals {
                if repair_spec(&mut pedal.spec) {
                    changed += 1;
                }
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_defaults_in_range_and_choices_name_stepped_keys() {
        for (i, a) in PARAMS.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            for b in &PARAMS[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
        for (k, _) in CHOICES {
            let info = PARAMS.iter().find(|p| p.key == *k).expect(k);
            assert!(info.stepped, "{k}");
        }
        let sync = PARAMS.iter().find(|p| p.key == "psync").unwrap();
        assert_eq!(sync.max as usize, SYNC_BEATS.len());
        let mix = PARAMS.iter().find(|p| p.key == "mix").unwrap();
        assert_eq!(mix.default, 0.0, "the null: the mix at 0");
        let sus = PARAMS.iter().find(|p| p.key == "isus").unwrap();
        assert_eq!(sus.default, 1.0, "infinite means infinite");
        for k in RETIRED {
            assert!(PARAMS.iter().all(|p| p.key != k), "{k} is retired");
        }
    }

    #[test]
    fn a_retired_key_becomes_today_s() {
        let mut s = crate::EffectSpec::new(crate::EffectKind::Shimmer);
        s.params.insert("ishim".into(), 0.0);
        s.params.insert("ilevel".into(), 1.0);
        assert!(repair_spec(&mut s));
        assert_eq!(s.params.get("isus"), Some(&0.0), "Fades is Sustain 0 %");
        assert!(!s.params.contains_key("ishim") && !s.params.contains_key("ilevel"));
        let mut s = crate::EffectSpec::new(crate::EffectKind::Shimmer);
        s.params.insert("ishim".into(), 1.0);
        assert!(repair_spec(&mut s));
        assert_eq!(
            s.params.get("isus"),
            Some(&1.0),
            "Sustains is Sustain 100 %"
        );
        assert!(!repair_spec(&mut s), "once");
        let mut other = crate::EffectSpec::new(crate::EffectKind::Glitch);
        other.params.insert("ishim".into(), 1.0);
        assert!(!repair_spec(&mut other), "another kind's keys are its own");
    }
}
