//! Instrument specifications: the *parameters* of a sound, not the DSP that makes it.
//!
//! The engine (`cantus-engine`) turns an [`InstrumentSpec`] into a running instrument.
//! Keeping the spec here means the project format, undo/redo and the AI layer can all
//! reason about sounds without linking the DSP crate.

use crate::model::Pitch;
use crate::pads::{KitField, Lanes, PadField, PadId, PulsusParams};
use serde::{Deserialize, Serialize};

/// Oscillator waveform. Stored by name so project files stay readable.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Waveform {
    Sine,
    Triangle,
    Saw,
    Square,
}

impl Waveform {
    pub const ALL: [Waveform; 4] = [
        Waveform::Sine,
        Waveform::Triangle,
        Waveform::Saw,
        Waveform::Square,
    ];

    pub fn index(self) -> usize {
        match self {
            Waveform::Sine => 0,
            Waveform::Triangle => 1,
            Waveform::Saw => 2,
            Waveform::Square => 3,
        }
    }

    pub fn from_index(i: usize) -> Waveform {
        Waveform::ALL[i.min(3)]
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Waveform::Sine => "wave.sine",
            Waveform::Triangle => "wave.triangle",
            Waveform::Saw => "wave.saw",
            Waveform::Square => "wave.square",
        }
    }
}

/// Parameters of **Firmus**, the built-in subtractive polysynth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirmusParams {
    pub osc1_wave: Waveform,
    pub osc2_wave: Waveform,
    /// Coarse tuning of oscillator 2 in semitones.
    pub osc2_semitones: f32,
    /// Fine tuning of oscillator 2 in cents.
    pub osc2_detune_cents: f32,
    /// 0.0 = only osc 1, 1.0 = only osc 2.
    pub osc_mix: f32,
    /// Base low-pass cutoff in Hz.
    pub cutoff_hz: f32,
    /// 0.0 ..= 1.0
    pub resonance: f32,
    /// Filter envelope depth in octaves (−4 ..= 4).
    pub filter_env_octaves: f32,
    /// Key tracking of the cutoff: 0 = none, 1 = cutoff follows the note fully.
    pub key_track: f32,
    pub amp_attack_s: f32,
    pub amp_decay_s: f32,
    pub amp_sustain: f32,
    pub amp_release_s: f32,
    pub filter_attack_s: f32,
    pub filter_decay_s: f32,
    pub filter_sustain: f32,
    pub filter_release_s: f32,
    /// Output level in dB.
    pub gain_db: f32,
}

impl Default for FirmusParams {
    fn default() -> Self {
        FirmusParams {
            osc1_wave: Waveform::Saw,
            osc2_wave: Waveform::Saw,
            osc2_semitones: 0.0,
            osc2_detune_cents: 8.0,
            osc_mix: 0.5,
            cutoff_hz: 2_200.0,
            resonance: 0.25,
            filter_env_octaves: 2.0,
            key_track: 0.5,
            amp_attack_s: 0.005,
            amp_decay_s: 0.15,
            amp_sustain: 0.8,
            amp_release_s: 0.25,
            filter_attack_s: 0.01,
            filter_decay_s: 0.3,
            filter_sustain: 0.3,
            filter_release_s: 0.3,
            gain_db: -6.0,
        }
    }
}

/// Addressable parameter of Firmus. This is the "plugin parameter id" of the built-in
/// synth: commands, automation (later) and the AI bridge all speak in these.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FirmusParam {
    Osc1Wave,
    Osc2Wave,
    Osc2Semitones,
    Osc2Detune,
    OscMix,
    Cutoff,
    Resonance,
    FilterEnvAmount,
    KeyTrack,
    AmpAttack,
    AmpDecay,
    AmpSustain,
    AmpRelease,
    FilterAttack,
    FilterDecay,
    FilterSustain,
    FilterRelease,
    Gain,
}

/// Display/range metadata for a parameter.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ParamInfo {
    pub param: FirmusParam,
    pub label_key: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    /// Use a logarithmic slider (frequencies, times).
    pub logarithmic: bool,
    /// Integer-valued (waveform selectors, semitones).
    pub stepped: bool,
}

impl FirmusParam {
    pub const ALL: [FirmusParam; 18] = [
        FirmusParam::Osc1Wave,
        FirmusParam::Osc2Wave,
        FirmusParam::Osc2Semitones,
        FirmusParam::Osc2Detune,
        FirmusParam::OscMix,
        FirmusParam::Cutoff,
        FirmusParam::Resonance,
        FirmusParam::FilterEnvAmount,
        FirmusParam::KeyTrack,
        FirmusParam::AmpAttack,
        FirmusParam::AmpDecay,
        FirmusParam::AmpSustain,
        FirmusParam::AmpRelease,
        FirmusParam::FilterAttack,
        FirmusParam::FilterDecay,
        FirmusParam::FilterSustain,
        FirmusParam::FilterRelease,
        FirmusParam::Gain,
    ];

    pub fn info(self) -> ParamInfo {
        use FirmusParam::*;
        let d = FirmusParams::default();
        let i = |label_key, min, max, default, unit, logarithmic, stepped| ParamInfo {
            param: self,
            label_key,
            min,
            max,
            default,
            unit,
            logarithmic,
            stepped,
        };
        match self {
            Osc1Wave => i(
                "param.osc1_wave",
                0.0,
                3.0,
                d.osc1_wave.index() as f32,
                "",
                false,
                true,
            ),
            Osc2Wave => i(
                "param.osc2_wave",
                0.0,
                3.0,
                d.osc2_wave.index() as f32,
                "",
                false,
                true,
            ),
            Osc2Semitones => i(
                "param.osc2_semitones",
                -24.0,
                24.0,
                d.osc2_semitones,
                "st",
                false,
                true,
            ),
            Osc2Detune => i(
                "param.osc2_detune",
                -50.0,
                50.0,
                d.osc2_detune_cents,
                "ct",
                false,
                false,
            ),
            OscMix => i("param.osc_mix", 0.0, 1.0, d.osc_mix, "", false, false),
            Cutoff => i(
                "param.cutoff",
                20.0,
                20_000.0,
                d.cutoff_hz,
                "Hz",
                true,
                false,
            ),
            Resonance => i("param.resonance", 0.0, 1.0, d.resonance, "", false, false),
            FilterEnvAmount => i(
                "param.filter_env",
                -4.0,
                4.0,
                d.filter_env_octaves,
                "oct",
                false,
                false,
            ),
            KeyTrack => i("param.key_track", 0.0, 1.0, d.key_track, "", false, false),
            AmpAttack => i(
                "param.amp_attack",
                0.001,
                5.0,
                d.amp_attack_s,
                "s",
                true,
                false,
            ),
            AmpDecay => i(
                "param.amp_decay",
                0.001,
                5.0,
                d.amp_decay_s,
                "s",
                true,
                false,
            ),
            AmpSustain => i(
                "param.amp_sustain",
                0.0,
                1.0,
                d.amp_sustain,
                "",
                false,
                false,
            ),
            AmpRelease => i(
                "param.amp_release",
                0.001,
                10.0,
                d.amp_release_s,
                "s",
                true,
                false,
            ),
            FilterAttack => i(
                "param.filter_attack",
                0.001,
                5.0,
                d.filter_attack_s,
                "s",
                true,
                false,
            ),
            FilterDecay => i(
                "param.filter_decay",
                0.001,
                5.0,
                d.filter_decay_s,
                "s",
                true,
                false,
            ),
            FilterSustain => i(
                "param.filter_sustain",
                0.0,
                1.0,
                d.filter_sustain,
                "",
                false,
                false,
            ),
            FilterRelease => i(
                "param.filter_release",
                0.001,
                10.0,
                d.filter_release_s,
                "s",
                true,
                false,
            ),
            Gain => i("param.gain", -36.0, 12.0, d.gain_db, "dB", false, false),
        }
    }

    pub fn get(self, p: &FirmusParams) -> f32 {
        use FirmusParam::*;
        match self {
            Osc1Wave => p.osc1_wave.index() as f32,
            Osc2Wave => p.osc2_wave.index() as f32,
            Osc2Semitones => p.osc2_semitones,
            Osc2Detune => p.osc2_detune_cents,
            OscMix => p.osc_mix,
            Cutoff => p.cutoff_hz,
            Resonance => p.resonance,
            FilterEnvAmount => p.filter_env_octaves,
            KeyTrack => p.key_track,
            AmpAttack => p.amp_attack_s,
            AmpDecay => p.amp_decay_s,
            AmpSustain => p.amp_sustain,
            AmpRelease => p.amp_release_s,
            FilterAttack => p.filter_attack_s,
            FilterDecay => p.filter_decay_s,
            FilterSustain => p.filter_sustain,
            FilterRelease => p.filter_release_s,
            Gain => p.gain_db,
        }
    }

    /// Set the parameter, clamping to its legal range. Returns the value actually stored.
    pub fn set(self, p: &mut FirmusParams, value: f32) -> f32 {
        use FirmusParam::*;
        let info = self.info();
        let v = if value.is_finite() {
            value.clamp(info.min, info.max)
        } else {
            info.default
        };
        let v = if info.stepped { v.round() } else { v };
        match self {
            Osc1Wave => p.osc1_wave = Waveform::from_index(v as usize),
            Osc2Wave => p.osc2_wave = Waveform::from_index(v as usize),
            Osc2Semitones => p.osc2_semitones = v,
            Osc2Detune => p.osc2_detune_cents = v,
            OscMix => p.osc_mix = v,
            Cutoff => p.cutoff_hz = v,
            Resonance => p.resonance = v,
            FilterEnvAmount => p.filter_env_octaves = v,
            KeyTrack => p.key_track = v,
            AmpAttack => p.amp_attack_s = v,
            AmpDecay => p.amp_decay_s = v,
            AmpSustain => p.amp_sustain = v,
            AmpRelease => p.amp_release_s = v,
            FilterAttack => p.filter_attack_s = v,
            FilterDecay => p.filter_decay_s = v,
            FilterSustain => p.filter_sustain = v,
            FilterRelease => p.filter_release_s = v,
            Gain => p.gain_db = v,
        }
        v
    }
}

/// One addressable parameter of *whatever* instrument a track hosts.
///
/// `SetInstrumentParam` used to carry a [`FirmusParam`] directly, which quietly said
/// "every instrument is Firmus". It is this now, so a second instrument does not need a
/// second command — and neither will a third. The `From` impls keep every existing call
/// site reading the way it did.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InstrumentParam {
    Firmus(FirmusParam),
    /// A field of one pad of a kit, addressed by the pad's **identity** ([`PadId`]), so an
    /// edit, an automation lane or an undo step keeps its pad when a lower one is added.
    Pulsus {
        pad: PadId,
        field: PadField,
    },
    /// A kit-wide field of Pulsus II: its model, seed and hi-hat pedal.
    PulsusKit(KitField),
}

impl From<FirmusParam> for InstrumentParam {
    fn from(p: FirmusParam) -> Self {
        InstrumentParam::Firmus(p)
    }
}

/// Which instrument a track hosts. New instruments are added as variants; plugin hosting
/// will add a `Plugin { format, id, state }` variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InstrumentSpec {
    Firmus(FirmusParams),
    /// **Pulsus**, the synthesized kit. Its arrival is what schema 6 is: a schema-5 build
    /// cannot read a project containing one, and the version number is what makes it say
    /// so instead of failing in some more confusing way.
    Pulsus(PulsusParams),
    /// A native instrument whose model is `key → f32` plus text (ADR 0023): Unda, Calor,
    /// Modulus, Exemplar, Gravis, Lamina, Organum. Its arrival is schema 16.
    Native(crate::inst::NativeSpec),
}

impl Default for InstrumentSpec {
    fn default() -> Self {
        InstrumentSpec::Firmus(FirmusParams::default())
    }
}

impl InstrumentSpec {
    pub fn firmus(&self) -> Option<&FirmusParams> {
        match self {
            InstrumentSpec::Firmus(p) => Some(p),
            _ => None,
        }
    }

    pub fn firmus_mut(&mut self) -> Option<&mut FirmusParams> {
        match self {
            InstrumentSpec::Firmus(p) => Some(p),
            _ => None,
        }
    }

    pub fn pulsus(&self) -> Option<&PulsusParams> {
        match self {
            InstrumentSpec::Pulsus(p) => Some(p),
            _ => None,
        }
    }

    pub fn pulsus_mut(&mut self) -> Option<&mut PulsusParams> {
        match self {
            InstrumentSpec::Pulsus(p) => Some(p),
            _ => None,
        }
    }

    pub fn native(&self) -> Option<&crate::inst::NativeSpec> {
        match self {
            InstrumentSpec::Native(n) => Some(n),
            _ => None,
        }
    }

    pub fn native_mut(&mut self) -> Option<&mut crate::inst::NativeSpec> {
        match self {
            InstrumentSpec::Native(n) => Some(n),
            _ => None,
        }
    }

    pub fn name_key(&self) -> &'static str {
        match self {
            InstrumentSpec::Firmus(_) => "instrument.firmus",
            InstrumentSpec::Pulsus(_) => "instrument.pulsus",
            InstrumentSpec::Native(n) => n.kind.name_key(),
        }
    }

    /// What this instrument calls a pitch, when it has a name of its own for it.
    ///
    /// `None` means "ask the keyboard": the caller falls back to
    /// [`pitch_name`](crate::pitch_name). It is deliberately not "C4" for a kit — a lane
    /// called `F#2` when it plays a closed hat is a lane the musician has to translate.
    pub fn lane_name(&self, pitch: Pitch) -> Option<&str> {
        match self {
            InstrumentSpec::Firmus(_) | InstrumentSpec::Native(_) => None,
            InstrumentSpec::Pulsus(p) => p
                .pads
                .iter()
                .find(|pad| pad.pitch == pitch)
                .map(|pad| pad.name.as_str()),
        }
    }

    /// The rows a note editor should draw for this instrument.
    pub fn lanes(&self) -> Lanes<'_> {
        match self {
            InstrumentSpec::Firmus(_) | InstrumentSpec::Native(_) => Lanes::Chromatic,
            InstrumentSpec::Pulsus(p) => Lanes::Pads(&p.pads),
        }
    }

    /// Set a parameter, clamping to its range. Returns the value actually stored, or
    /// `None` when the parameter does not belong to this instrument — which is what a
    /// project file edited by hand, or an AI tool call, can ask for.
    pub fn set_param(&mut self, param: InstrumentParam, value: f32) -> Option<f32> {
        match (self, param) {
            (InstrumentSpec::Firmus(p), InstrumentParam::Firmus(f)) => Some(f.set(p, value)),
            (InstrumentSpec::Pulsus(p), InstrumentParam::Pulsus { pad, field }) => {
                let pad = p.by_id_mut(pad)?;
                Some(field.set(pad, value))
            }
            (InstrumentSpec::Pulsus(p), InstrumentParam::PulsusKit(field)) => {
                Some(field.set(p, value))
            }
            _ => None,
        }
    }

    pub fn param(&self, param: InstrumentParam) -> Option<f32> {
        match (self, param) {
            (InstrumentSpec::Firmus(p), InstrumentParam::Firmus(f)) => Some(f.get(p)),
            (InstrumentSpec::Pulsus(p), InstrumentParam::Pulsus { pad, field }) => {
                Some(field.get(p.by_id(pad)?))
            }
            (InstrumentSpec::Pulsus(p), InstrumentParam::PulsusKit(field)) => Some(field.get(p)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_param_roundtrips_and_clamps() {
        let mut p = FirmusParams::default();
        for param in FirmusParam::ALL {
            let info = param.info();
            assert!(
                info.min <= info.default && info.default <= info.max,
                "{param:?}"
            );
            let stored = param.set(&mut p, info.max + 1000.0);
            assert_eq!(stored, info.max);
            assert_eq!(param.get(&p), info.max);
            let stored = param.set(&mut p, f32::NAN);
            assert_eq!(stored, info.default);
        }
    }

    #[test]
    fn waveform_index() {
        for w in Waveform::ALL {
            assert_eq!(Waveform::from_index(w.index()), w);
        }
    }
}
