//! Fragor, the distortion and fuzz pedals (ADR 0128): its parameter model. Owned by Fragor;
//! nothing else edits this file.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the board starts
//! empty (`s1` … `s3` hold nothing) with the gate off, so nothing but the gains runs, and
//! those are 0 dB. The pedals' own knobs default to noon (the fuzz's battery fresh) — they
//! do nothing until their pedal is on the board.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `s1`, `s2`, `s3` | what each slot of the chain holds, in signal order: none, or a pedal ([`MODELS`]) | 0 … 5 | none |
//! | `od.on`, `op.on`, `si.on`, `ge.on`, `mf.on` | each pedal's footswitch | 0 / 1 | on |
//! | `od.drive`, `od.tone`, `od.level` | Green overdrive: Drive, Tone, Level | 0 … 100 % | 50 % |
//! | `op.dist`, `op.filter`, `op.vol` | Op-amp distortion: Distortion, Filter, Volume | 0 … 100 % | 50 % |
//! | `si.dist`, `si.tone`, `si.level` | Silicon distortion: Dist, Tone, Level | 0 … 100 % | 50 % |
//! | `ge.fuzz`, `ge.bias`, `ge.vol` | Germanium fuzz: Fuzz, Bias (the battery: 100 % fresh), Volume | 0 … 100 % | 50, 100, 50 % |
//! | `mf.sustain`, `mf.tone`, `mf.vol` | Triangle muff: Sustain, Tone, Volume | 0 … 100 % | 50 % |
//! | `gate` | the noise gate's threshold, before the pedals; −90 is off | −90 … −20 dB | off |
//! | `in` | the level into the pedals | −24 … 24 dB | 0 |
//! | `mix` | dry/wet, the dry path delayed to match | 0 … 100 % | 100 % |
//! | `mono` | one circuit for both channels (their sum), as a pedal is | 0 / 1 | stereo |
//! | `os` | oversampling: 2×, 4×, 8× | 0 … 2 | 4× |
//! | `out` | output gain | −24 … 24 dB | 0 |

use crate::effect::{p, EffectParamInfo};

pub const NAME_KEY: &str = "effect.fragor";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;

/// The pedals, in the order of the slots' values 1 … 5 (0 is an empty slot); the key
/// prefix of each one's parameters.
pub const MODELS: [&str; 5] = ["od", "op", "si", "ge", "mf"];
/// Each pedal's three knobs, in the DSP's order (gain, tone, level).
pub const KNOBS: [[&str; 3]; 5] = [
    ["od.drive", "od.tone", "od.level"],
    ["op.dist", "op.filter", "op.vol"],
    ["si.dist", "si.tone", "si.level"],
    ["ge.fuzz", "ge.bias", "ge.vol"],
    ["mf.sustain", "mf.tone", "mf.vol"],
];
/// Each pedal's footswitch.
pub const SWITCHES: [&str; 5] = ["od.on", "op.on", "si.on", "ge.on", "mf.on"];
/// The slots, in signal order.
pub const SLOTS: [&str; 3] = ["s1", "s2", "s3"];
/// The gate's threshold at or below which it is off.
pub const GATE_OFF_DB: f32 = -90.0;

/// The locale prefix of each stepped parameter's choices, by key: choice `i` is
/// `{prefix}.{i}` in the locale files.
pub const CHOICES: &[(&str, &str)] = &[
    ("s1", "fragor.model"),
    ("s2", "fragor.model"),
    ("s3", "fragor.model"),
    ("od.on", "fragor.onoff"),
    ("op.on", "fragor.onoff"),
    ("si.on", "fragor.onoff"),
    ("ge.on", "fragor.onoff"),
    ("mf.on", "fragor.onoff"),
    ("mono", "fragor.chan"),
    ("os", "fragor.os"),
];

const fn knob(key: &'static str, label: &'static str, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, 1.0, default, "%", false, false)
}

const fn switch(key: &'static str, label: &'static str) -> EffectParamInfo {
    p(key, label, 0.0, 1.0, 1.0, "", false, true)
}

const fn slot(key: &'static str, label: &'static str) -> EffectParamInfo {
    p(key, label, 0.0, 5.0, 0.0, "", false, true)
}

pub const PARAMS: &[EffectParamInfo] = &[
    slot("s1", "fx.fragor.s1"),
    slot("s2", "fx.fragor.s2"),
    slot("s3", "fx.fragor.s3"),
    switch("od.on", "fx.fragor.od.on"),
    knob("od.drive", "fx.fragor.od.drive", 0.5),
    knob("od.tone", "fx.fragor.od.tone", 0.5),
    knob("od.level", "fx.fragor.od.level", 0.5),
    switch("op.on", "fx.fragor.op.on"),
    knob("op.dist", "fx.fragor.op.dist", 0.5),
    knob("op.filter", "fx.fragor.op.filter", 0.5),
    knob("op.vol", "fx.fragor.op.vol", 0.5),
    switch("si.on", "fx.fragor.si.on"),
    knob("si.dist", "fx.fragor.si.dist", 0.5),
    knob("si.tone", "fx.fragor.si.tone", 0.5),
    knob("si.level", "fx.fragor.si.level", 0.5),
    switch("ge.on", "fx.fragor.ge.on"),
    knob("ge.fuzz", "fx.fragor.ge.fuzz", 0.5),
    knob("ge.bias", "fx.fragor.ge.bias", 1.0),
    knob("ge.vol", "fx.fragor.ge.vol", 0.5),
    switch("mf.on", "fx.fragor.mf.on"),
    knob("mf.sustain", "fx.fragor.mf.sustain", 0.5),
    knob("mf.tone", "fx.fragor.mf.tone", 0.5),
    knob("mf.vol", "fx.fragor.mf.vol", 0.5),
    p(
        "gate",
        "fx.fragor.gate",
        GATE_OFF_DB,
        -20.0,
        GATE_OFF_DB,
        "dB",
        false,
        false,
    ),
    p("in", "fx.fragor.in", -24.0, 24.0, 0.0, "dB", false, false),
    p("mix", "fx.fragor.mix", 0.0, 1.0, 1.0, "%", false, false),
    p("mono", "fx.fragor.mono", 0.0, 1.0, 0.0, "", false, true),
    p("os", "fx.fragor.os", 0.0, 2.0, 1.0, "", false, true),
    p("out", "fx.fragor.out", -24.0, 24.0, 0.0, "dB", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key once; the contract's `out` kept; the board empty and the gate off by
    /// default (the null).
    #[test]
    fn the_table_is_whole_and_its_defaults_are_a_null() {
        let mut keys: Vec<&str> = PARAMS.iter().map(|p| p.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), PARAMS.len());
        let get = |k: &str| PARAMS.iter().find(|p| p.key == k).unwrap();
        assert_eq!(get("out").default, 0.0);
        assert_eq!(get("in").default, 0.0);
        assert_eq!(get("gate").default, GATE_OFF_DB);
        for s in SLOTS {
            assert_eq!(get(s).default, 0.0);
            assert_eq!(get(s).max as usize, MODELS.len());
        }
        for (m, ks) in KNOBS.iter().enumerate() {
            for k in ks {
                assert!(k.starts_with(MODELS[m]));
                assert!(get(k).min == 0.0 && get(k).max == 1.0);
            }
            assert!(SWITCHES[m].starts_with(MODELS[m]));
        }
        for (k, _) in CHOICES {
            assert!(get(k).stepped, "{k}");
        }
    }
}
