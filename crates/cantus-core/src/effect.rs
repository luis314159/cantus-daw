//! Native insert effects: their *parameters*, not their DSP (see `cantus-dsp`).
//!
//! Effects use a generic parameter model — a kind plus `key → f32` — so the UI, the command
//! layer and the AI bridge treat every effect (and, later, every plugin) the same way.

pub use crate::external::{ExternalPlugin, PluginFormat, PluginState};
use crate::fx;
use crate::model::{EffectId, StripId, TrackId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectKind {
    Delay,
    Reverb,
    /// Aequalis, the parametric EQ: [`EQ_BANDS`] bands addressed as `b{n}.{field}`.
    Eq,
    /// Comprimo, the compressor. Can be keyed from another strip ([`EffectSlot::sidechain`]).
    Compressor,
    /// Terminus, the limiter (`fx::limiter`).
    Limiter,
    /// Spatium, the reverb (`fx::space`). The older `Reverb` stays for the projects that
    /// use it.
    Space,
    /// Multiplex, the multiband dynamics (`fx::multiband`).
    Multiband,
    /// Sibilus, the de-esser (`fx::deesser`).
    DeEsser,
    /// Porta, the gate and expander (`fx::gate`).
    Gate,
    /// Fervor, saturation and colour: tape, tube, overdrive, a drawn waveshaper
    /// (`fx::saturator`, ADR 0026).
    Saturator,
    /// Latitudo, stereo control: width, mid/side, mono bass, balance (`fx::stereo`).
    Stereo,
    /// Culmen, the mastering maximizer: multiband dynamics into a limiter
    /// (`fx::maximizer`), FL's Maximus to our bar.
    Maximizer,
    /// A plug-in made to a market standard (CLAP, VST3), hosted (ADR 0019). What it is and
    /// its parameters are in [`EffectSpec::external`]; `params` stays empty.
    External,
    /// Concentus, the chorus (Juno, Dimension) (`fx::concentus`, round 28).
    Chorus,
    /// Flexus, the through-zero flanger (`fx::flexus`, round 28).
    Flanger,
    /// Phasis, the phaser (`fx::phasis`, round 28).
    Phaser,
    /// Tremor, tremolo and auto-pan (`fx::tremor`, round 28).
    Tremolo,
    /// Vibrans, the vibrato (`fx::vibrans`, round 28).
    Vibrato,
    /// Anulus, the ring modulator (`fx::anulus`, round 28).
    RingMod,
    /// Translatio, the frequency shifter (`fx::translatio`, round 28).
    FreqShift,
    /// Fluctus, the LFO Tool-style volume shaper (`fx::fluctus`, round 28).
    VolumeShaper,
    /// Tonsor, the clipper, with true peak (`fx::tonsor`, round 28).
    Clipper,
    /// Impetus, the transient processor (`fx::impetus`, round 28).
    Transient,
    /// Discrimen, the transient splitter (`fx::discrimen`, round 28).
    TransientSplit,
    /// Speculum, the analyser (Wave Candy / SPAN class) (`fx::speculum`, round 28).
    Analyzer,
    /// Tonus, the real-time auto-tuner (`fx::tonus`, round 28).
    AutoTune,
}

pub use fx::compressor::COMPRESSOR_PARAMS;
pub use fx::eq::{eq_key, BandField, EQ_BANDS, EQ_BAND_FIELDS};

/// Display/range metadata for one effect parameter.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EffectParamInfo {
    pub key: &'static str,
    pub label_key: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    pub logarithmic: bool,
    pub stepped: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn p(
    key: &'static str,
    label_key: &'static str,
    min: f32,
    max: f32,
    default: f32,
    unit: &'static str,
    logarithmic: bool,
    stepped: bool,
) -> EffectParamInfo {
    EffectParamInfo {
        key,
        label_key,
        min,
        max,
        default,
        unit,
        logarithmic,
        stepped,
    }
}

pub const DELAY_PARAMS: &[EffectParamInfo] = &[
    p(
        "time_ms",
        "fx.delay.time",
        1.0,
        2000.0,
        375.0,
        "ms",
        true,
        false,
    ),
    p(
        "feedback",
        "fx.delay.feedback",
        0.0,
        0.95,
        0.4,
        "",
        false,
        false,
    ),
    p(
        "tone_hz",
        "fx.delay.tone",
        200.0,
        20_000.0,
        6_000.0,
        "Hz",
        true,
        false,
    ),
    p(
        "ping_pong",
        "fx.delay.ping_pong",
        0.0,
        1.0,
        1.0,
        "",
        false,
        true,
    ),
    p("mix", "fx.mix", 0.0, 1.0, 0.3, "", false, false),
];

pub const REVERB_PARAMS: &[EffectParamInfo] = &[
    p("size", "fx.reverb.size", 0.0, 1.0, 0.7, "", false, false),
    p(
        "damping",
        "fx.reverb.damping",
        0.0,
        1.0,
        0.4,
        "",
        false,
        false,
    ),
    p("width", "fx.reverb.width", 0.0, 1.0, 1.0, "", false, false),
    p(
        "predelay_ms",
        "fx.reverb.predelay",
        0.0,
        100.0,
        10.0,
        "ms",
        false,
        false,
    ),
    p("mix", "fx.mix", 0.0, 1.0, 0.25, "", false, false),
];

impl EffectKind {
    pub const ALL: [EffectKind; 25] = [
        EffectKind::Eq,
        EffectKind::Compressor,
        EffectKind::Limiter,
        EffectKind::Space,
        EffectKind::Multiband,
        EffectKind::DeEsser,
        EffectKind::Gate,
        EffectKind::Saturator,
        EffectKind::Stereo,
        EffectKind::Maximizer,
        EffectKind::Delay,
        EffectKind::Reverb,
        EffectKind::Chorus,
        EffectKind::Flanger,
        EffectKind::Phaser,
        EffectKind::Tremolo,
        EffectKind::Vibrato,
        EffectKind::RingMod,
        EffectKind::FreqShift,
        EffectKind::VolumeShaper,
        EffectKind::Clipper,
        EffectKind::Transient,
        EffectKind::TransientSplit,
        EffectKind::Analyzer,
        EffectKind::AutoTune,
    ];

    pub fn name_key(self) -> &'static str {
        match self {
            EffectKind::Delay => "effect.delay",
            EffectKind::Reverb => "effect.reverb",
            EffectKind::Eq => fx::eq::NAME_KEY,
            EffectKind::Compressor => fx::compressor::NAME_KEY,
            EffectKind::Limiter => fx::limiter::NAME_KEY,
            EffectKind::Space => fx::space::NAME_KEY,
            EffectKind::Multiband => fx::multiband::NAME_KEY,
            EffectKind::DeEsser => fx::deesser::NAME_KEY,
            EffectKind::Gate => fx::gate::NAME_KEY,
            EffectKind::Saturator => fx::saturator::NAME_KEY,
            EffectKind::Stereo => fx::stereo::NAME_KEY,
            EffectKind::Maximizer => fx::maximizer::NAME_KEY,
            EffectKind::External => crate::external::NAME_KEY,
            EffectKind::Chorus => fx::concentus::NAME_KEY,
            EffectKind::Flanger => fx::flexus::NAME_KEY,
            EffectKind::Phaser => fx::phasis::NAME_KEY,
            EffectKind::Tremolo => fx::tremor::NAME_KEY,
            EffectKind::Vibrato => fx::vibrans::NAME_KEY,
            EffectKind::RingMod => fx::anulus::NAME_KEY,
            EffectKind::FreqShift => fx::translatio::NAME_KEY,
            EffectKind::VolumeShaper => fx::fluctus::NAME_KEY,
            EffectKind::Clipper => fx::tonsor::NAME_KEY,
            EffectKind::Transient => fx::impetus::NAME_KEY,
            EffectKind::TransientSplit => fx::discrimen::NAME_KEY,
            EffectKind::Analyzer => fx::speculum::NAME_KEY,
            EffectKind::AutoTune => fx::tonus::NAME_KEY,
        }
    }

    pub fn params(self) -> &'static [EffectParamInfo] {
        match self {
            EffectKind::Delay => DELAY_PARAMS,
            EffectKind::Reverb => REVERB_PARAMS,
            EffectKind::Eq => fx::eq::params(),
            EffectKind::Compressor => fx::compressor::params(),
            EffectKind::Limiter => fx::limiter::params(),
            EffectKind::Space => fx::space::params(),
            EffectKind::Multiband => fx::multiband::params(),
            EffectKind::DeEsser => fx::deesser::params(),
            EffectKind::Gate => fx::gate::params(),
            EffectKind::Saturator => fx::saturator::params(),
            EffectKind::Stereo => fx::stereo::params(),
            EffectKind::Maximizer => fx::maximizer::params(),
            // The plug-in's own, by its ids, in `EffectSpec::external`.
            EffectKind::External => &[],
            EffectKind::Chorus => fx::concentus::params(),
            EffectKind::Flanger => fx::flexus::params(),
            EffectKind::Phaser => fx::phasis::params(),
            EffectKind::Tremolo => fx::tremor::params(),
            EffectKind::Vibrato => fx::vibrans::params(),
            EffectKind::RingMod => fx::anulus::params(),
            EffectKind::FreqShift => fx::translatio::params(),
            EffectKind::VolumeShaper => fx::fluctus::params(),
            EffectKind::Clipper => fx::tonsor::params(),
            EffectKind::Transient => fx::impetus::params(),
            EffectKind::TransientSplit => fx::discrimen::params(),
            EffectKind::Analyzer => fx::speculum::params(),
            EffectKind::AutoTune => fx::tonus::params(),
        }
    }

    /// A kind with a large parameter space stores only what differs from the defaults,
    /// so an EQ with two bands is a few lines in the project file and not three hundred.
    pub fn sparse(self) -> bool {
        match self {
            EffectKind::Delay | EffectKind::Reverb => false,
            EffectKind::Eq => fx::eq::SPARSE,
            EffectKind::Compressor => fx::compressor::SPARSE,
            EffectKind::Limiter => fx::limiter::SPARSE,
            EffectKind::Space => fx::space::SPARSE,
            EffectKind::Multiband => fx::multiband::SPARSE,
            EffectKind::DeEsser => fx::deesser::SPARSE,
            EffectKind::Gate => fx::gate::SPARSE,
            EffectKind::Saturator => fx::saturator::SPARSE,
            EffectKind::Stereo => fx::stereo::SPARSE,
            EffectKind::Maximizer => fx::maximizer::SPARSE,
            EffectKind::External => true,
            EffectKind::Chorus => fx::concentus::SPARSE,
            EffectKind::Flanger => fx::flexus::SPARSE,
            EffectKind::Phaser => fx::phasis::SPARSE,
            EffectKind::Tremolo => fx::tremor::SPARSE,
            EffectKind::Vibrato => fx::vibrans::SPARSE,
            EffectKind::RingMod => fx::anulus::SPARSE,
            EffectKind::FreqShift => fx::translatio::SPARSE,
            EffectKind::VolumeShaper => fx::fluctus::SPARSE,
            EffectKind::Clipper => fx::tonsor::SPARSE,
            EffectKind::Transient => fx::impetus::SPARSE,
            EffectKind::TransientSplit => fx::discrimen::SPARSE,
            EffectKind::Analyzer => fx::speculum::SPARSE,
            EffectKind::AutoTune => fx::tonus::SPARSE,
        }
    }

    /// A kind with an editor of its own, rather than a row of knobs per parameter.
    pub fn has_editor(self) -> bool {
        !matches!(self, EffectKind::Delay | EffectKind::Reverb)
    }

    /// A kind whose detector can listen to another strip.
    pub fn takes_sidechain(self) -> bool {
        match self {
            EffectKind::Delay | EffectKind::Reverb => false,
            EffectKind::Eq => fx::eq::TAKES_SIDECHAIN,
            EffectKind::Compressor => fx::compressor::TAKES_SIDECHAIN,
            EffectKind::Limiter => fx::limiter::TAKES_SIDECHAIN,
            EffectKind::Space => fx::space::TAKES_SIDECHAIN,
            EffectKind::Multiband => fx::multiband::TAKES_SIDECHAIN,
            EffectKind::DeEsser => fx::deesser::TAKES_SIDECHAIN,
            EffectKind::Gate => fx::gate::TAKES_SIDECHAIN,
            EffectKind::Saturator => fx::saturator::TAKES_SIDECHAIN,
            EffectKind::Stereo => fx::stereo::TAKES_SIDECHAIN,
            EffectKind::Maximizer => fx::maximizer::TAKES_SIDECHAIN,
            // Per plug-in, not per kind: `EffectSpec::takes_sidechain`.
            EffectKind::External => false,
            EffectKind::Chorus => fx::concentus::TAKES_SIDECHAIN,
            EffectKind::Flanger => fx::flexus::TAKES_SIDECHAIN,
            EffectKind::Phaser => fx::phasis::TAKES_SIDECHAIN,
            EffectKind::Tremolo => fx::tremor::TAKES_SIDECHAIN,
            EffectKind::Vibrato => fx::vibrans::TAKES_SIDECHAIN,
            EffectKind::RingMod => fx::anulus::TAKES_SIDECHAIN,
            EffectKind::FreqShift => fx::translatio::TAKES_SIDECHAIN,
            EffectKind::VolumeShaper => fx::fluctus::TAKES_SIDECHAIN,
            EffectKind::Clipper => fx::tonsor::TAKES_SIDECHAIN,
            EffectKind::Transient => fx::impetus::TAKES_SIDECHAIN,
            EffectKind::TransientSplit => fx::discrimen::TAKES_SIDECHAIN,
            EffectKind::Analyzer => fx::speculum::TAKES_SIDECHAIN,
            EffectKind::AutoTune => fx::tonus::TAKES_SIDECHAIN,
        }
    }

    /// A kind that can hear a track's notes ([`EffectSlot::midi_from`]).
    pub fn takes_notes(self) -> bool {
        match self {
            EffectKind::Gate => fx::gate::TAKES_NOTES,
            EffectKind::Chorus => fx::concentus::TAKES_NOTES,
            EffectKind::Flanger => fx::flexus::TAKES_NOTES,
            EffectKind::Phaser => fx::phasis::TAKES_NOTES,
            EffectKind::Tremolo => fx::tremor::TAKES_NOTES,
            EffectKind::Vibrato => fx::vibrans::TAKES_NOTES,
            EffectKind::RingMod => fx::anulus::TAKES_NOTES,
            EffectKind::FreqShift => fx::translatio::TAKES_NOTES,
            EffectKind::VolumeShaper => fx::fluctus::TAKES_NOTES,
            EffectKind::Clipper => fx::tonsor::TAKES_NOTES,
            EffectKind::Transient => fx::impetus::TAKES_NOTES,
            EffectKind::TransientSplit => fx::discrimen::TAKES_NOTES,
            EffectKind::Analyzer => fx::speculum::TAKES_NOTES,
            EffectKind::AutoTune => fx::tonus::TAKES_NOTES,
            _ => false,
        }
    }

    /// The locale prefix of a stepped parameter's choices (`{prefix}.{i}`), by key, for
    /// the kinds that keep that table in their own module (round 28 on): what the
    /// exported plug-in shows a host (`cantus_plugins::params`). Empty for the older
    /// kinds, whose prefixes `cantus_plugins::params` lists itself.
    pub fn choices(self) -> &'static [(&'static str, &'static str)] {
        match self {
            EffectKind::Chorus => fx::concentus::CHOICES,
            EffectKind::Flanger => fx::flexus::CHOICES,
            EffectKind::Phaser => fx::phasis::CHOICES,
            EffectKind::Tremolo => fx::tremor::CHOICES,
            EffectKind::Vibrato => fx::vibrans::CHOICES,
            EffectKind::RingMod => fx::anulus::CHOICES,
            EffectKind::FreqShift => fx::translatio::CHOICES,
            EffectKind::VolumeShaper => fx::fluctus::CHOICES,
            EffectKind::Clipper => fx::tonsor::CHOICES,
            EffectKind::Transient => fx::impetus::CHOICES,
            EffectKind::TransientSplit => fx::discrimen::CHOICES,
            EffectKind::Analyzer => fx::speculum::CHOICES,
            EffectKind::AutoTune => fx::tonus::CHOICES,
            _ => &[],
        }
    }

    pub fn param(self, key: &str) -> Option<&'static EffectParamInfo> {
        self.params().iter().find(|i| i.key == key)
    }
}

/// An effect's kind and parameter values. Unknown keys are ignored on load; missing keys
/// read as their default, so adding a parameter never breaks an old file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectSpec {
    pub kind: EffectKind,
    pub params: BTreeMap<String, f32>,
    /// The hosted plug-in, for [`EffectKind::External`]; `None` for every native kind, and
    /// then not written, so a native effect's JSON is what it always was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<Box<ExternalPlugin>>,
}

impl EffectSpec {
    pub fn new(kind: EffectKind) -> Self {
        let params = if kind.sparse() {
            BTreeMap::new()
        } else {
            kind.params()
                .iter()
                .map(|i| (i.key.to_string(), i.default))
                .collect()
        };
        EffectSpec {
            kind,
            params,
            external: None,
        }
    }

    /// A hosted plug-in's spec.
    pub fn external(plugin: ExternalPlugin) -> Self {
        EffectSpec {
            kind: EffectKind::External,
            params: BTreeMap::new(),
            external: Some(Box::new(plugin)),
        }
    }

    /// Whether this effect can be keyed from another strip: by kind for the native ones,
    /// by what the scan found for a hosted plug-in (a second audio input).
    pub fn takes_sidechain(&self) -> bool {
        self.kind.takes_sidechain() || self.external.as_ref().is_some_and(|e| e.sidechain)
    }

    pub fn get(&self, key: &str) -> f32 {
        match self.params.get(key) {
            Some(v) => *v,
            None => self.kind.param(key).map(|i| i.default).unwrap_or(0.0),
        }
    }

    /// Set a parameter, clamping to its range. `None` for a key the effect does not have.
    pub fn set(&mut self, key: &str, value: f32) -> Option<f32> {
        let info = self.kind.param(key)?;
        let v = if value.is_finite() {
            value.clamp(info.min, info.max)
        } else {
            info.default
        };
        let v = if info.stepped { v.round() } else { v };
        self.params.insert(key.to_string(), v);
        Some(v)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectSlot {
    pub id: EffectId,
    pub bypass: bool,
    pub spec: EffectSpec,
    /// The slot's own dry/wet, 0…1, outside the effect: at 1 the effect's output is what
    /// leaves the slot, at 0 the slot is a wire. FL has this knob on every slot, and it
    /// is what makes any effect a parallel one without a second strip. Unity is skipped
    /// entirely on the audio thread, so an old file renders exactly as it did.
    #[serde(default = "unit_mix")]
    pub mix: f32,
    /// The strip whose post-fader output this effect's detector listens to, instead of its
    /// own input — a sidechain, chosen inside the effect with no send to set up. A route
    /// like an output or a send: it orders the strips and it cannot close a loop.
    #[serde(default)]
    pub sidechain: Option<StripId>,
    /// The track whose notes this effect hears — a gate opened by a MIDI part, chosen
    /// inside the effect (ADR 0022). Not a route for audio: it orders nothing, and a track
    /// that is removed simply stops sending (undoing the removal brings it back). The
    /// track's own mute silences its instrument, not its notes. `None` (the default, and
    /// every file before schema 14) sends nothing, and the effect renders as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi_from: Option<TrackId>,
}

fn unit_mix() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_clamping() {
        for kind in EffectKind::ALL {
            let mut s = EffectSpec::new(kind);
            for info in kind.params() {
                assert_eq!(s.get(info.key), info.default);
                assert_eq!(s.set(info.key, info.max + 1.0), Some(info.max));
                assert_eq!(s.set(info.key, f32::NAN), Some(info.default));
            }
            assert!(s.set("nope", 1.0).is_none());
        }
    }

    #[test]
    fn the_eq_names_its_bands_stably_and_stores_only_what_moved() {
        let mut s = EffectSpec::new(EffectKind::Eq);
        assert!(s.params.is_empty(), "a new EQ writes nothing");
        assert_eq!(
            EffectKind::Eq.params().len(),
            EQ_BANDS * EQ_BAND_FIELDS.len() + fx::eq::EQ_GLOBALS.len()
        );
        assert_eq!(s.get("b23.freq"), 1_000.0);
        assert_eq!(s.set("b3.gain", 40.0), Some(30.0));
        assert_eq!(
            s.set("b3.shape", 2.4),
            Some(2.0),
            "a shape is a whole number"
        );
        assert!(
            s.set("b24.gain", 1.0).is_none(),
            "there are 24 bands, 0 to 23"
        );
        assert_eq!(s.params.len(), 2);
    }

    #[test]
    fn missing_key_reads_default() {
        let mut s = EffectSpec::new(EffectKind::Delay);
        s.params.remove("mix");
        assert_eq!(s.get("mix"), 0.3);
        let json = serde_json::to_string(&s).unwrap();
        let back: EffectSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
