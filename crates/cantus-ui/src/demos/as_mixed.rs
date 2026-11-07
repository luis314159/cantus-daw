//! The demos keep the presets they were mixed on (round 37, ADR 0101 §7).

/// A preset's key and the values to set back on it.
pub type Restore = (&'static str, &'static [(&'static str, Option<f32>)]);

/// The values the demos were mixed with, for the presets the library's second delivery
/// changed (round 37, ADR 0101: levels fitted, heavy voices lightened): a demo is a mix, so
/// it keeps the sound it was balanced on — `None` is the parameter's default. Generated
/// from the presets' history; a preset retuned later does not move a finished song.
pub const AS_MIXED: &[Restore] = &[
    ("bass.preset.fretless", &[("gain", None)]),
    ("bass.preset.p_fingers", &[("gain", None)]),
    ("bass.preset.slap", &[("gain", None)]),
    ("bass.preset.square_sub", &[("gain", None)]),
    ("bass.preset.sub_sine", &[("gain", None)]),
    ("bass.preset.trap_long", &[("gain", None)]),
    (
        "calor.preset.glass_pluck",
        &[("gain", None), ("mix.osc1", None), ("mix.osc2", Some(0.5))],
    ),
    ("calor.preset.string_machine", &[("gain", None)]),
    ("calor.preset.sync_lead", &[("gain", None)]),
    (
        "calor.preset.unison_saw",
        &[("gain", None), ("voice.stack", Some(6.0))],
    ),
    (
        "calor.preset.velvet_pad",
        &[("gain", None), ("voice.stack", Some(3.0))],
    ),
    ("comp.preset.bass", &[("makeup", None)]),
    ("comp.preset.parallel", &[("makeup", Some(12.0))]),
    ("comp.preset.pump", &[("makeup", None)]),
    ("epiano.preset.reed_growl", &[("gain", None)]),
    ("epiano.preset.suitcase", &[("gain", None)]),
    ("epiano.preset.worn", &[("gain", None)]),
    ("eq.preset.kick", &[("out", None)]),
    ("eq.preset.vocal", &[("out", None)]),
    ("fm.preset.bell", &[("gain", None)]),
    ("fm.preset.brass", &[("gain", None)]),
    ("fm.preset.flute", &[("gain", None)]),
    ("fm.preset.marimba", &[("gain", None)]),
    ("fm.preset.pluck", &[("gain", None)]),
    ("mb.preset.glue", &[("out", None)]),
    ("mb.preset.low_end", &[("out", None)]),
    ("organ.preset.soul", &[("gain", None)]),
    ("preset.anulus.stereo_shimmer", &[("out", None)]),
    ("preset.concentus.juno_two", &[("out", None)]),
    ("preset.concentus.string_ensemble", &[("out", None)]),
    ("preset.concentus.vocal_double", &[("out", None)]),
    ("preset.discrimen.snappy", &[("out", None)]),
    ("preset.flexus.synced_sweep", &[("out", None)]),
    ("preset.fluctus.pump_quarters", &[("out", None)]),
    ("preset.impetus.kick", &[("out", None)]),
    ("preset.impetus.snap", &[("out", None)]),
    ("preset.phasis.phase90_script", &[("out", None)]),
    ("preset.translatio.barber_up", &[("out", None)]),
    ("preset.tremor.wide_pan", &[("out", None)]),
    (
        "smp.preset.bells",
        &[("amp.release", Some(1800.0)), ("gain", Some(-3.0))],
    ),
    ("smp.preset.choir", &[("gain", Some(-3.0))]),
    ("smp.preset.nylon", &[("gain", Some(-5.0))]),
    ("smp.preset.pad", &[("gain", None)]),
    ("unda.preset.digital_bells", &[("gain", Some(-3.0))]),
    ("unda.preset.formant_talker", &[("gain", Some(-4.0))]),
    (
        "unda.preset.glass_choir",
        &[
            ("gain", Some(-4.0)),
            ("osc1.uni", Some(5.0)),
            ("osc2.uni", Some(3.0)),
        ],
    ),
    ("unda.preset.hyper_saw", &[("gain", Some(-6.0))]),
    ("unda.preset.noise_sweep", &[("gain", Some(-8.0))]),
    ("unda.preset.pluck_keys", &[("gain", None)]),
    ("unda.preset.sub_drop", &[("gain", Some(-2.0))]),
    (
        "unda.preset.warp_pad",
        &[
            ("env1.r", Some(2.5)),
            ("gain", Some(-5.0)),
            ("osc1.level", None),
            ("osc1.uni", Some(7.0)),
            ("osc1.warp2", Some(7.0)),
            ("osc1.warp2.amt", Some(0.2)),
        ],
    ),
];
/// The values to set back on a preset `key` a demo loads, each `None` read from `fresh` (the
/// unit's defaults).
pub fn restore(key: &str, fresh: impl Fn(&str) -> f32) -> Vec<(&'static str, f32)> {
    AS_MIXED
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, values)| {
            values
                .iter()
                .map(|(p, v)| (*p, v.unwrap_or_else(|| fresh(p))))
                .collect()
        })
        .unwrap_or_default()
}
