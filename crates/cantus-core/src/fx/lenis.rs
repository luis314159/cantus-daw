//! Lenis, the resonance suppressor (ADR 0143, provisional): its parameter model. Owned by
//! the wave-B builder; the DSP is `cantus_dsp::lenis`, whose report
//! (`docs/round-fx/lenis.md`) says how a resonance is told from a note and a loud region.
//!
//! Keys are forever once shipped. **Every default leaves the signal untouched**: at depth 0
//! the output is the input delayed by the latency, to the bit (the DSP's dry ring; the host
//! aligns it like any latent insert); `out` at 0 dB is not applied.
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `depth` | the share of each resonance's bump taken away | 0 … 100 % |
//! | `sharp` | narrow rings only (100 %) … broad build-ups too (0 %) | 0 … 100 % |
//! | `select` | how far a peak must stand out before it counts | 0 … 100 % |
//! | `att`, `rel` | the reduction's attack and release | 0.5 … 200 ms, 5 … 2 000 ms |
//! | `lo`, `hi` | the focus's limits (a third of an octave's fade outside) | 20 … 20 000 Hz |
//! | `b{n}on`, `b{n}hz`, `b{n}amt`, `b{n}w` (n 0 … 5) | the focus curve's six bands: on, centre, amount (−100 % left alone … +100 % twice), width at half height | 0 … 1, 20 … 20 000 Hz, −1 … +1, 0.1 … 4 oct |
//! | `chan` | Linked, Independent, Mid/Side | 0 … 2 |
//! | `curve` | Soft, Hard | 0 … 1 |
//! | `qual` | Fast (5.3 ms), Standard (10.6 ms), Precise (21.3 ms): the frame and so the latency | 0 … 2 |
//! | `delta` | hear what is taken away | 0 … 1 |
//! | `mix` | dry to processed | 0 … 100 % |
//! | `out` | the output level | −24 … +24 dB |
//!
//! A `qual` change resets the unit (a gap of one latency) and the engine follows the new
//! latency with its delay compensation: two outputs delayed differently cannot crossfade.

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.lenis";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
pub const TAKES_NOTES: bool = false;

/// The focus curve's bands.
pub const BANDS: usize = 6;

/// The locale prefix of each stepped parameter's choices, by key (ADR 0020).
pub const CHOICES: &[(&str, &str)] = &[
    ("chan", "fx.lenis.chan"),
    ("curve", "fx.lenis.curve"),
    ("qual", "fx.lenis.qual"),
    ("delta", "fx.lenis.delta"),
    ("b0on", "fx.lenis.bon"),
    ("b1on", "fx.lenis.bon"),
    ("b2on", "fx.lenis.bon"),
    ("b3on", "fx.lenis.bon"),
    ("b4on", "fx.lenis.bon"),
    ("b5on", "fx.lenis.bon"),
];

const fn amount(key: &'static str, label: &'static str, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, 1.0, default, "%", false, false)
}

const fn step(key: &'static str, label: &'static str, max: f32, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, max, default, "", false, true)
}

const fn hz(key: &'static str, label: &'static str, default: f32) -> EffectParamInfo {
    p(key, label, 20.0, 20_000.0, default, "Hz", true, false)
}

/// One focus band's four keys. Their centres start spread over the band (an octave and a
/// half apart from 150 Hz), so a band switched on lands somewhere useful.
macro_rules! band {
    ($n:literal, $hz:expr) => {
        [
            step(concat!("b", $n, "on"), "fx.lenis.bon", 1.0, 0.0),
            hz(concat!("b", $n, "hz"), "fx.lenis.bhz", $hz),
            p(
                concat!("b", $n, "amt"),
                "fx.lenis.bamt",
                -1.0,
                1.0,
                0.0,
                "%",
                false,
                false,
            ),
            p(
                concat!("b", $n, "w"),
                "fx.lenis.bw",
                0.1,
                4.0,
                1.0,
                "oct",
                true,
                false,
            ),
        ]
    };
}

const B0: [EffectParamInfo; 4] = band!("0", 150.0);
const B1: [EffectParamInfo; 4] = band!("1", 400.0);
const B2: [EffectParamInfo; 4] = band!("2", 1_000.0);
const B3: [EffectParamInfo; 4] = band!("3", 2_500.0);
const B4: [EffectParamInfo; 4] = band!("4", 5_000.0);
const B5: [EffectParamInfo; 4] = band!("5", 10_000.0);

pub const PARAMS: &[EffectParamInfo] = &[
    // A host's list: the three that matter most, then time, the focus, the switches.
    amount("depth", "fx.lenis.depth", 0.0),
    amount("sharp", "fx.lenis.sharp", 0.5),
    amount("select", "fx.lenis.select", 0.5),
    p("att", "fx.lenis.att", 0.5, 200.0, 5.0, "ms", true, false),
    p("rel", "fx.lenis.rel", 5.0, 2_000.0, 80.0, "ms", true, false),
    hz("lo", "fx.lenis.lo", 20.0),
    hz("hi", "fx.lenis.hi", 20_000.0),
    step("chan", "fx.lenis.chan", 2.0, 0.0),
    step("curve", "fx.lenis.curve", 1.0, 0.0),
    step("qual", "fx.lenis.qual", 2.0, 1.0),
    step("delta", "fx.lenis.delta", 1.0, 0.0),
    amount("mix", "fx.lenis.mix", 1.0),
    p("out", "fx.lenis.out", -24.0, 24.0, 0.0, "dB", false, false),
    B0[0],
    B0[1],
    B0[2],
    B0[3],
    B1[0],
    B1[1],
    B1[2],
    B1[3],
    B2[0],
    B2[1],
    B2[2],
    B2[3],
    B3[0],
    B3[1],
    B3[2],
    B3[3],
    B4[0],
    B4[1],
    B4[2],
    B4[3],
    B5[0],
    B5[1],
    B5[2],
    B5[3],
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

/// A band's key: `band_key(2, "hz")` is `b2hz`.
pub fn band_key(band: usize, what: &str) -> &'static str {
    let i = band.min(BANDS - 1) * 4
        + match what {
            "on" => 0,
            "hz" => 1,
            "amt" => 2,
            _ => 3,
        };
    PARAMS[13 + i].key
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
        assert_eq!(PARAMS.len(), 13 + 4 * BANDS);
        assert_eq!(band_key(0, "on"), "b0on");
        assert_eq!(band_key(5, "w"), "b5w");
        assert_eq!(band_key(3, "amt"), "b3amt");
        let depth = PARAMS.iter().find(|p| p.key == "depth").unwrap();
        assert_eq!(depth.default, 0.0, "the null: depth 0");
    }
}
