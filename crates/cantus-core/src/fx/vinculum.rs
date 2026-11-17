//! Vinculum, the bus compressor (ADR 0142, provisional): its parameter model. Owned by the
//! wave-B builder; the DSP is `cantus_dsp::vinculum`, whose report
//! (`docs/round-fx/vinculum.md`) says why each model is built as it is.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the threshold at
//! its top (+24 dBFS, over any signal), no make-up, no colour; `out` at 0 dB is not applied.
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `model` | VCA (a console bus compressor, feed-forward), Opto (a photocell, feedback), Vari-mu (a remote-cutoff valve, feedback) | 0 … 2 |
//! | `thr` | the threshold (the opto's and the vari-mu's output level where they begin) | −50 … +24 dB |
//! | `ratio` | the VCA's ratio: 2, 4, 10 : 1 | 0 … 2 |
//! | `att` | the VCA's attack: 0.1, 0.3, 1, 3, 10, 30 ms | 0 … 5 |
//! | `rel` | the VCA's release: 0.1, 0.3, 0.6, 1.2 s, Auto (programme dependent) | 0 … 4 |
//! | `char` | the opto's and the vari-mu's Compress or Limit | 0 … 1 |
//! | `time` | the vari-mu's six time positions | 0 … 5 |
//! | `hpf` | the side chain's high-pass (20 Hz is off) | 20 … 500 Hz |
//! | `makeup` | the compressed path's gain | 0 … 24 dB |
//! | `colour` | the vari-mu's valve colour (odd harmonics, at 2×) | 0 … 100 % |
//! | `mix` | dry to compressed (parallel compression) | 0 … 100 % |
//! | `out` | the output level | −24 … +24 dB |
//!
//! The key: the slot's sidechain when one is routed (another strip, `TAKES_SIDECHAIN`), its
//! own input otherwise, through the high-pass either way.

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.vinculum";
pub const SPARSE: bool = false;
/// The classic bus compressor's external key.
pub const TAKES_SIDECHAIN: bool = true;
pub const TAKES_NOTES: bool = false;

/// The VCA's panel, in `ratio`'s, `att`'s and `rel`'s order (`rel` 4 is Auto).
pub const RATIOS: [f32; 3] = [2.0, 4.0, 10.0];
pub const ATTACKS_MS: [f32; 6] = [0.1, 0.3, 1.0, 3.0, 10.0, 30.0];
pub const RELEASES_MS: [f32; 4] = [100.0, 300.0, 600.0, 1200.0];
/// `rel`'s Auto position.
pub const REL_AUTO: f32 = 4.0;
/// `hpf` at or under this is off.
pub const HPF_OFF_HZ: f32 = 20.0;

/// The locale prefix of each stepped parameter's choices, by key (ADR 0020).
pub const CHOICES: &[(&str, &str)] = &[
    ("model", "fx.vinculum.model"),
    ("ratio", "fx.vinculum.ratio"),
    ("att", "fx.vinculum.att"),
    ("rel", "fx.vinculum.rel"),
    ("char", "fx.vinculum.char"),
    ("time", "fx.vinculum.time"),
];

const fn step(key: &'static str, label: &'static str, max: f32, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, max, default, "", false, true)
}

pub const PARAMS: &[EffectParamInfo] = &[
    // A host's list and the faceplate's order: the circuit, then the level, then the time.
    step("model", "fx.vinculum.model", 2.0, 0.0),
    p(
        "thr",
        "fx.vinculum.thr",
        -50.0,
        24.0,
        24.0,
        "dB",
        false,
        false,
    ),
    step("ratio", "fx.vinculum.ratio", 2.0, 1.0),
    step("att", "fx.vinculum.att", 5.0, 4.0),
    step("rel", "fx.vinculum.rel", 4.0, 1.0),
    step("char", "fx.vinculum.char", 1.0, 0.0),
    step("time", "fx.vinculum.time", 5.0, 1.0),
    p(
        "hpf",
        "fx.vinculum.hpf",
        HPF_OFF_HZ,
        500.0,
        HPF_OFF_HZ,
        "Hz",
        true,
        false,
    ),
    p(
        "makeup",
        "fx.vinculum.makeup",
        0.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
    p(
        "colour",
        "fx.vinculum.colour",
        0.0,
        1.0,
        0.0,
        "%",
        false,
        false,
    ),
    p("mix", "fx.vinculum.mix", 0.0, 1.0, 1.0, "%", false, false),
    p(
        "out",
        "fx.vinculum.out",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
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
        let get = |k: &str| PARAMS.iter().find(|p| p.key == k).unwrap();
        assert_eq!(get("ratio").max as usize + 1, RATIOS.len());
        assert_eq!(get("att").max as usize + 1, ATTACKS_MS.len());
        assert_eq!(get("rel").max, REL_AUTO);
        assert_eq!(
            get("thr").default,
            get("thr").max,
            "the null: over any signal"
        );
        assert_eq!(get("makeup").default, 0.0);
        assert_eq!(get("colour").default, 0.0);
    }
}
