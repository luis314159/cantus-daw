//! Effects that sound on insert (ADR 0071, BACKLOG §3b option B, the owner's decision of
//! 2026-10-16).
//!
//! Every parameter's **default** stays the value ADR 0018's null rule asks for — a unit at
//! its defaults touches no sample — so the family's tests are unchanged. What changes is
//! what an insert *loads*: a musical **starting preset**, one of the unit's own factory
//! presets (Concentus's *Juno I*, …), so a chorus inserted on a pad is heard as a chorus
//! at once, as in FL Studio and Ableton. Units that already sound at their defaults, that
//! start flat by design, that must not surprise (a limiter) or that only measure take
//! none; [`start_of`] says which and why, for every kind. The application can turn it off
//! (*Insert effects at their neutral settings*).

use crate::effect::{EffectKind, EffectSpec};

/// A unit's starting preset: the factory preset's label key and the values it sets (the
/// rest at their defaults). The same values as the factory list the editors show (a test
/// in the application holds the two together).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StartingPreset {
    pub label_key: &'static str,
    pub params: &'static [(&'static str, f32)],
}

/// Why a unit starts at its defaults.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Neutral {
    /// Flat by design: the curve or the image is what the user makes (an EQ, a stereo
    /// utility) — as FL's Parametric EQ 2, Pro-Q and every utility start.
    Flat,
    /// Its defaults already do the job audibly (a compressor at −18 dB 4:1, a reverb 25 %
    /// wet, a gate at −40 dB, a tuner correcting to the chromatic scale).
    Sounds,
    /// A safety at the end of a chain: it must not change anything until it is asked to
    /// (a limiter, a maximizer, a clipper at 0 dBFS catch only what goes over).
    MustNotSurprise,
    /// It only measures (the analyser passes its signal through untouched).
    Measures,
    /// A hosted plug-in: its own initial state is its maker's.
    Hosted,
}

/// What an insert of a kind starts with.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Start {
    Preset(StartingPreset),
    Neutral(Neutral),
}

const fn preset(label_key: &'static str, params: &'static [(&'static str, f32)]) -> Start {
    Start::Preset(StartingPreset { label_key, params })
}

/// The decision for each kind (ADR 0071's table).
pub fn start_of(kind: EffectKind) -> Start {
    use EffectKind as K;
    use Neutral::*;
    match kind {
        K::Eq | K::Stereo => Start::Neutral(Flat),
        K::Compressor | K::Space | K::DeEsser | K::Gate | K::Delay | K::Reverb | K::AutoTune => {
            Start::Neutral(Sounds)
        }
        K::Limiter | K::Maximizer | K::Clipper => Start::Neutral(MustNotSurprise),
        K::Analyzer => Start::Neutral(Measures),
        K::External => Start::Neutral(Hosted),
        K::Multiband => preset(
            "mb.preset.glue",
            &[
                ("b0.used", 1.0),
                ("b0.from", 20.0),
                ("b0.thr", -26.0),
                ("b0.ratio", 1.8),
                ("b0.range", 6.0),
                ("b0.knee", 12.0),
                ("b0.att", 30.0),
                ("b0.rel", 250.0),
                ("b1.used", 1.0),
                ("b1.from", 120.0),
                ("b1.thr", -28.0),
                ("b1.ratio", 1.5),
                ("b1.range", 6.0),
                ("b1.knee", 12.0),
                ("b1.att", 25.0),
                ("b1.rel", 200.0),
                ("b2.used", 1.0),
                ("b2.from", 2_000.0),
                ("b2.thr", -30.0),
                ("b2.ratio", 1.5),
                ("b2.range", 6.0),
                ("b2.knee", 12.0),
                ("b2.att", 15.0),
                ("b2.rel", 150.0),
                ("b3.used", 1.0),
                ("b3.from", 8_000.0),
                ("b3.thr", -34.0),
                ("b3.ratio", 1.5),
                ("b3.range", 4.0),
                ("b3.knee", 12.0),
                ("b3.att", 10.0),
                ("b3.rel", 120.0),
            ],
        ),
        K::Saturator => preset(
            "preset.sat.tape_bus",
            &[("model", 0.0), ("drive", 7.0), ("char", 0.5), ("os", 2.0)],
        ),
        K::Chorus => preset("preset.concentus.juno_one", &[("mode", 0.0), ("mix", 0.5)]),
        K::Flanger => preset(
            "preset.flexus.jet",
            &[("mix", 0.5), ("fb", 0.75), ("rate", 0.15), ("depth", 6.0)],
        ),
        K::Phaser => preset(
            "preset.phasis.phase90_script",
            &[("mix", 0.5), ("lo", 220.0), ("hi", 1_600.0), ("rate", 0.6)],
        ),
        K::Tremolo => preset(
            "preset.tremor.surf",
            &[
                ("depth", 0.65),
                ("sync", 0.0),
                ("rate", 6.2),
                ("shape", 1.0),
            ],
        ),
        K::Vibrato => preset(
            "preset.vibrans.singer",
            &[("depth", 30.0), ("rate", 5.5), ("rise", 350.0)],
        ),
        K::RingMod => preset("preset.anulus.bell", &[("freq", 587.0), ("mix", 0.6)]),
        K::FreqShift => preset(
            "preset.translatio.slow_phase",
            &[("shift", 0.7), ("mix", 0.5)],
        ),
        K::VolumeShaper => preset(
            "preset.fluctus.pump_quarters",
            &[
                ("depth", 1.0),
                ("div", 8.0),
                ("smooth", 2.0),
                ("p0.on", 1.0),
                ("p0.x", 0.0),
                ("p0.y", 0.0),
                ("p0.t", -0.6),
                ("p0.s", 0.0),
                ("p1.on", 1.0),
                ("p1.x", 0.5),
                ("p1.y", 1.0),
                ("p1.t", 0.0),
                ("p1.s", 2.0),
            ],
        ),
        K::Transient => preset("preset.impetus.punch", &[("attack", 0.6), ("speed", 12.0)]),
        K::TransientSplit => preset(
            "preset.discrimen.more_attack",
            &[("t.gain", 5.0), ("mode", 1.0)],
        ),
    }
}

/// A kind's starting preset, if it has one.
pub fn starting_preset(kind: EffectKind) -> Option<StartingPreset> {
    match start_of(kind) {
        Start::Preset(p) => Some(p),
        Start::Neutral(_) => None,
    }
}

/// Load `kind`'s starting preset into `spec` (its values over the defaults it holds). A
/// key the kind does not have is left out (none does: a test holds every key to its
/// kind). Returns the preset's label key, or `None` for a unit that starts neutral.
pub fn apply(spec: &mut EffectSpec) -> Option<&'static str> {
    let p = starting_preset(spec.kind)?;
    for (k, v) in p.params {
        let _ = spec.set(k, *v);
    }
    Some(p.label_key)
}

/// A fresh spec of `kind` as an insert starts it: its starting preset over its defaults.
pub fn starting_spec(kind: EffectKind) -> EffectSpec {
    let mut s = EffectSpec::new(kind);
    apply(&mut s);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind is decided — a preset, or a reason to start neutral — and each preset
    /// names only keys its kind has, in range, and differs from the defaults (it sounds).
    #[test]
    fn every_unit_has_a_starting_preset_or_a_reason_for_none() {
        for kind in EffectKind::ALL {
            match start_of(kind) {
                Start::Preset(p) => {
                    let infos = kind.params();
                    for (k, v) in p.params {
                        let info = infos
                            .iter()
                            .find(|i| i.key == *k)
                            .unwrap_or_else(|| panic!("{kind:?}: no key {k}"));
                        assert!(
                            (info.min..=info.max).contains(v),
                            "{kind:?}: {k} = {v} out of range"
                        );
                    }
                    let s = starting_spec(kind);
                    assert_ne!(
                        s,
                        EffectSpec::new(kind),
                        "{kind:?}: the preset changes nothing"
                    );
                    for (k, v) in p.params {
                        assert_eq!(s.get(k), *v, "{kind:?}: {k}");
                    }
                }
                Start::Neutral(_) => assert_eq!(starting_spec(kind), EffectSpec::new(kind)),
            }
        }
        assert_eq!(
            start_of(EffectKind::External),
            Start::Neutral(Neutral::Hosted)
        );
    }

    /// The units that must not surprise are the safeties at the end of a chain.
    #[test]
    fn a_limiter_never_surprises() {
        for k in [
            EffectKind::Limiter,
            EffectKind::Maximizer,
            EffectKind::Clipper,
        ] {
            assert_eq!(start_of(k), Start::Neutral(Neutral::MustNotSurprise));
        }
    }
}
