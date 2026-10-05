//! Latitudo, stereo control and mid/side (ADR 0026): its parameter model. Owned by
//! Latitudo; nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins'
//! ids hold them). **Every default leaves the signal untouched, to the bit.**
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `width` | the side's level against the mid: 0 is mono, 100 % as it came, 200 % twice as wide | 0 … 200 % |
//! | `mid`, `side` | the mid's and the side's own gain | −24 … +12 dB |
//! | `bal` | balance: the far side is turned down, the near one kept | −100 … +100 % |
//! | `rot` | rotation of the image (positive: toward the right) | −45° … +45° |
//! | `swap` | left and right exchanged | off / on |
//! | `inv_l`, `inv_r` | polarity of the left and of the right input | off / on |
//! | `listen` | hear the stereo result, the mid alone (the mono check) or the side alone | 0 … 2 |
//! | `mono` | **mono below** this frequency: the side is removed under it, phase-coherently; 20 Hz is off | 20 … 1000 Hz |
//! | `mono_slope` | its slope: 6, 12, 24 or 48 dB/oct | 0 … 3 |
//! | `x_lo`, `x_hi` | the crossovers of the width by band | 40 … 2000, 500 … 16 000 Hz |
//! | `w_lo`, `w_mid`, `w_hi` | the width of each band (on top of `width`) | 0 … 200 % |
//! | `m_lo_f`, `m_lo` | the mid's low shelf: frequency and gain | 20 … 2000 Hz, ±12 dB |
//! | `m_hi_f`, `m_hi` | the mid's high shelf | 1 … 20 kHz, ±12 dB |
//! | `s_lo_f`, `s_lo`, `s_hi_f`, `s_hi` | the side's shelves | as the mid's |
//! | `tone_slope` | the shelves' slope: 6 or 12 dB/oct | 0 … 1 |
//! | `haas` | Haas widening: a delay of up to 30 ms; 0 is off | 0 … 30 ms |
//! | `haas_mode` | delay the right, delay the left, or *mono-safe* (the delayed mid added to the side, which leaves the mono sum untouched) | 0 … 2 |
//! | `haas_amt` | how much delayed mid the mono-safe mode adds to the side | 0 … 100 % |
//! | `out` | output gain | −24 … +12 dB |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.stereo";
/// Thirty parameters, almost all at their defaults in a real chain: only what moved is
/// stored.
pub const SPARSE: bool = true;
pub const TAKES_SIDECHAIN: bool = false;

/// `mono` at or below this is off.
pub const MONO_OFF_HZ: f32 = 20.0;

pub const PARAMS: &[EffectParamInfo] = &[
    p("width", "fx.st.width", 0.0, 2.0, 1.0, "%", false, false),
    p("mid", "fx.st.mid", -24.0, 12.0, 0.0, "dB", false, false),
    p("side", "fx.st.side", -24.0, 12.0, 0.0, "dB", false, false),
    p("bal", "fx.st.bal", -1.0, 1.0, 0.0, "%", false, false),
    p("rot", "fx.st.rot", -45.0, 45.0, 0.0, "\u{b0}", false, false),
    p("swap", "fx.st.swap", 0.0, 1.0, 0.0, "", false, true),
    p("inv_l", "fx.st.inv_l", 0.0, 1.0, 0.0, "", false, true),
    p("inv_r", "fx.st.inv_r", 0.0, 1.0, 0.0, "", false, true),
    p("listen", "fx.st.listen", 0.0, 2.0, 0.0, "", false, true),
    p(
        "mono",
        "fx.st.mono",
        MONO_OFF_HZ,
        1_000.0,
        MONO_OFF_HZ,
        "Hz",
        true,
        false,
    ),
    p(
        "mono_slope",
        "fx.st.mono_slope",
        0.0,
        3.0,
        2.0,
        "",
        false,
        true,
    ),
    p(
        "x_lo",
        "fx.st.x_lo",
        40.0,
        2_000.0,
        250.0,
        "Hz",
        true,
        false,
    ),
    p(
        "x_hi",
        "fx.st.x_hi",
        500.0,
        16_000.0,
        4_000.0,
        "Hz",
        true,
        false,
    ),
    p("w_lo", "fx.st.w_lo", 0.0, 2.0, 1.0, "%", false, false),
    p("w_mid", "fx.st.w_mid", 0.0, 2.0, 1.0, "%", false, false),
    p("w_hi", "fx.st.w_hi", 0.0, 2.0, 1.0, "%", false, false),
    p(
        "m_lo_f",
        "fx.st.m_lo_f",
        20.0,
        2_000.0,
        150.0,
        "Hz",
        true,
        false,
    ),
    p("m_lo", "fx.st.m_lo", -12.0, 12.0, 0.0, "dB", false, false),
    p(
        "m_hi_f",
        "fx.st.m_hi_f",
        1_000.0,
        20_000.0,
        8_000.0,
        "Hz",
        true,
        false,
    ),
    p("m_hi", "fx.st.m_hi", -12.0, 12.0, 0.0, "dB", false, false),
    p(
        "s_lo_f",
        "fx.st.s_lo_f",
        20.0,
        2_000.0,
        150.0,
        "Hz",
        true,
        false,
    ),
    p("s_lo", "fx.st.s_lo", -12.0, 12.0, 0.0, "dB", false, false),
    p(
        "s_hi_f",
        "fx.st.s_hi_f",
        1_000.0,
        20_000.0,
        8_000.0,
        "Hz",
        true,
        false,
    ),
    p("s_hi", "fx.st.s_hi", -12.0, 12.0, 0.0, "dB", false, false),
    p(
        "tone_slope",
        "fx.st.tone_slope",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
    p("haas", "fx.st.haas", 0.0, 30.0, 0.0, "ms", false, false),
    p(
        "haas_mode",
        "fx.st.haas_mode",
        0.0,
        2.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "haas_amt",
        "fx.st.haas_amt",
        0.0,
        1.0,
        0.5,
        "%",
        false,
        false,
    ),
    p("out", "fx.st.out", -24.0, 12.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_defaults_in_range() {
        for (i, a) in PARAMS.iter().enumerate() {
            assert!(a.min <= a.default && a.default <= a.max, "{}", a.key);
            assert!(!a.logarithmic || a.min > 0.0, "{} is log from 0", a.key);
            assert!(a.label_key.starts_with("fx.st."), "{}", a.key);
            for b in &PARAMS[i + 1..] {
                assert_ne!(a.key, b.key);
            }
        }
    }

    /// The contract's three keys keep their meaning: whatever was written with them reads
    /// the same.
    #[test]
    fn the_contracts_keys_are_kept() {
        let width = PARAMS.iter().find(|p| p.key == "width").unwrap();
        assert_eq!((width.min, width.max, width.default), (0.0, 2.0, 1.0));
        for k in ["mid", "side"] {
            let p = PARAMS.iter().find(|p| p.key == k).unwrap();
            assert_eq!((p.min, p.max, p.default, p.unit), (-24.0, 12.0, 0.0, "dB"));
        }
    }
}
