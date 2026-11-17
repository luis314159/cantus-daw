//! Purus, the real-time repair unit (ADR 0145, provisional): its parameter model. Owned by
//! the wave-C builder; the DSP is `cantus_dsp::purus`, whose report
//! (`docs/round-fx/purus.md`) says why it is built as it is.
//!
//! Three modules in a chain — de-hum, de-click (with de-crackle), de-reverb — each with its
//! switch, its mix, its output level and a delta (listen to what it takes out). Keys are
//! forever once shipped (automation, saved projects and the exported plug-ins' ids hold
//! them). **Every default leaves the signal untouched, to the bit**: every module off. A
//! module switched on starts from its measured settings (its mix at 1).
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `hon` | the de-hum | off, on |
//! | `hmode` | where the fundamental comes from: fixed, followed (±2 Hz), found (50 or 60, then followed) | 0 … 2 |
//! | `hhz` | the fundamental, or the nominal the tracker centres on | 40 … 70 Hz |
//! | `hharm` | the harmonics taken out, the fundamental counted | 1 … 16 |
//! | `hdepth`, `hwidth` | each notch's depth and −3 dB width | 0 … 80 dB, 0.5 … 20 Hz |
//! | `hgentle` | subtract the learned hum (held under a bass note, 50 ms ahead) instead of notching | off, on |
//! | `hmix`, `hout`, `hdelta` | the de-hum's mix, output and delta | 0 … 100 %, −24 … +24 dB, off/on |
//! | `con` | the de-click | off, on |
//! | `csens` | the clicks found — capped at [`SENSITIVITY_MAX`] (above it the repair harms) | 0 … 0.7 |
//! | `crackle` | the de-crackle (0: off) | 0 … 100 % |
//! | `cmix`, `cout`, `cdelta` | the de-click's mix, output and delta | |
//! | `von` | the de-reverb | off, on |
//! | `vamount` | the most any bin is turned down | 0 … 30 dB |
//! | `vdecay` | the decay set, or estimated from the signal's free decays | 0 … 1 |
//! | `vt60` | the room's decay (T60) | 0.1 … 10 s |
//! | `vdirect` | the direct sound the model assumes, against the room's (higher: a closer microphone) | −10 … +20 dB |
//! | `vsmooth` | the gains' smoothing | 0 … 100 % |
//! | `vkeep` | the early reflections kept | 5 … 200 ms |
//! | `vmix`, `vout`, `vdelta` | the de-reverb's mix, output and delta | |
//! | `out` | the unit's output level | −24 … +24 dB |
//!
//! **Latency** is the sum of the modules that are on (`cantus_dsp::purus::latency_for`):
//! the gentle de-hum's 50 ms, the de-click's 767 samples, the de-reverb's `N − 1` (1 023
//! samples: 21.3 ms at 48 kHz, 23.2 ms at 44.1 kHz). The engine compensates it.

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.purus";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
pub const TAKES_NOTES: bool = false;

/// The de-click's sensitivity at its top: the DSP's own scale runs to 1, but past about
/// 0.7 the repair harms the music more than the clicks do (the DSP report's finding, the
/// editor's own declick alike). The knob stops here.
pub const SENSITIVITY_MAX: f32 = 0.7;

/// The de-reverb's Direct sound at its default: +6 dB, dialogue first (a dry voice keeps
/// its level within 1 dB). The owner may move it; it is this one constant (far-microphone
/// presets set 0 dB).
pub const DIRECT_DB_DEFAULT: f32 = 6.0;

/// The locale prefix of each stepped parameter's choices, by key (ADR 0020).
pub const CHOICES: &[(&str, &str)] = &[
    ("hon", "fx.purus.onoff"),
    ("hmode", "fx.purus.hmode"),
    ("hgentle", "fx.purus.onoff"),
    ("hdelta", "fx.purus.onoff"),
    ("con", "fx.purus.onoff"),
    ("cdelta", "fx.purus.onoff"),
    ("von", "fx.purus.onoff"),
    ("vdecay", "fx.purus.vdecay"),
    ("vdelta", "fx.purus.onoff"),
];

const fn switch(key: &'static str, label: &'static str, max: f32, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, max, default, "", false, true)
}

const fn amount(key: &'static str, label: &'static str, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, 1.0, default, "%", false, false)
}

const fn level(key: &'static str, label: &'static str) -> EffectParamInfo {
    p(key, label, -24.0, 24.0, 0.0, "dB", false, false)
}

pub const PARAMS: &[EffectParamInfo] = &[
    // A host's list: the three modules in the signal's order, each its switch first.
    switch("hon", "fx.purus.hon", 1.0, 0.0),
    switch("hmode", "fx.purus.hmode", 2.0, 0.0),
    p("hhz", "fx.purus.hhz", 40.0, 70.0, 50.0, "Hz", false, false),
    p("hharm", "fx.purus.hharm", 1.0, 16.0, 8.0, "", false, true),
    p("hdepth", "fx.purus.hdepth", 0.0, 80.0, 40.0, "dB", false, false),
    p("hwidth", "fx.purus.hwidth", 0.5, 20.0, 2.0, "Hz", true, false),
    switch("hgentle", "fx.purus.hgentle", 1.0, 0.0),
    amount("hmix", "fx.purus.hmix", 1.0),
    level("hout", "fx.purus.hout"),
    switch("hdelta", "fx.purus.hdelta", 1.0, 0.0),
    switch("con", "fx.purus.con", 1.0, 0.0),
    p(
        "csens",
        "fx.purus.csens",
        0.0,
        SENSITIVITY_MAX,
        0.5,
        "",
        false,
        false,
    ),
    amount("crackle", "fx.purus.crackle", 0.0),
    amount("cmix", "fx.purus.cmix", 1.0),
    level("cout", "fx.purus.cout"),
    switch("cdelta", "fx.purus.cdelta", 1.0, 0.0),
    switch("von", "fx.purus.von", 1.0, 0.0),
    p(
        "vamount",
        "fx.purus.vamount",
        0.0,
        30.0,
        12.0,
        "dB",
        false,
        false,
    ),
    switch("vdecay", "fx.purus.vdecay", 1.0, 0.0),
    p("vt60", "fx.purus.vt60", 0.1, 10.0, 0.8, "s", true, false),
    p(
        "vdirect",
        "fx.purus.vdirect",
        -10.0,
        20.0,
        DIRECT_DB_DEFAULT,
        "dB",
        false,
        false,
    ),
    amount("vsmooth", "fx.purus.vsmooth", 0.5),
    p("vkeep", "fx.purus.vkeep", 5.0, 200.0, 50.0, "ms", true, false),
    amount("vmix", "fx.purus.vmix", 1.0),
    level("vout", "fx.purus.vout"),
    switch("vdelta", "fx.purus.vdelta", 1.0, 0.0),
    level("out", "fx.purus.out"),
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
        // The null: every module off.
        for k in ["hon", "con", "von"] {
            let info = PARAMS.iter().find(|p| p.key == k).unwrap();
            assert_eq!(info.default, 0.0, "{k}");
        }
        let sens = PARAMS.iter().find(|p| p.key == "csens").unwrap();
        assert_eq!(sens.max, SENSITIVITY_MAX);
        let direct = PARAMS.iter().find(|p| p.key == "vdirect").unwrap();
        assert_eq!(direct.default, DIRECT_DB_DEFAULT);
    }
}
