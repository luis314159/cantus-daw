//! The addressable fields of a pad and of a kit — the "plugin parameter ids" of a drum.
//!
//! The ranges and defaults depend on the pad's voice ([`PadField::info`]): a hi-hat's tone
//! lives around 9 kHz and a snare's around 2 kHz, and offering both the same slider would
//! make one of them unusable. Pulsus I's fifteen fields keep their names and order; Pulsus
//! II's follow them.

use super::{DrumPad, PadVoice, PulsusParams, CHOKE_GROUPS, PULSUS_MODEL};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PadField {
    /// Which voice the pad is, as an index into [`PadVoice::ALL`].
    Voice,
    Level,
    Pan,
    Choke,
    Mute,
    Tune,
    Decay,
    PitchDrop,
    PitchDecay,
    Click,
    Drive,
    NoiseDecay,
    Tone,
    Mix,
    HpCutoff,
    // Pulsus II.
    Model,
    Attack,
    Hold,
    Curve,
    PitchEnv,
    PitchEnvTime,
    Filter,
    Cutoff,
    Resonance,
    FilterEnv,
    TransAttack,
    TransSustain,
    DriveMode,
    Phase,
    VelLevel,
    VelTone,
    VelPitch,
    VelDecay,
    ChokeFade,
    ChokeOnOff,
    Polyphony,
    RoundRobin,
    Pedal,
    Openness,
    OpenDecay,
    Sigh,
    Wires,
    Material,
    Modes,
    Damping,
    Position,
    Hardness,
    Glide,
    Shell,
    BandMix,
    Swell,
    Bursts,
    Spacing,
    Q,
}

/// Display and range metadata for a pad field, for a particular voice.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PadParamInfo {
    pub field: PadField,
    pub label_key: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    pub logarithmic: bool,
    pub stepped: bool,
}

impl PadField {
    pub const ALL: [PadField; 54] = {
        use PadField::*;
        [
            Voice,
            Level,
            Pan,
            Choke,
            Mute,
            Tune,
            Decay,
            PitchDrop,
            PitchDecay,
            Click,
            Drive,
            NoiseDecay,
            Tone,
            Mix,
            HpCutoff,
            Model,
            Attack,
            Hold,
            Curve,
            PitchEnv,
            PitchEnvTime,
            Filter,
            Cutoff,
            Resonance,
            FilterEnv,
            TransAttack,
            TransSustain,
            DriveMode,
            Phase,
            VelLevel,
            VelTone,
            VelPitch,
            VelDecay,
            ChokeFade,
            ChokeOnOff,
            Polyphony,
            RoundRobin,
            Pedal,
            Openness,
            OpenDecay,
            Sigh,
            Wires,
            Material,
            Modes,
            Damping,
            Position,
            Hardness,
            Glide,
            Shell,
            BandMix,
            Swell,
            Bursts,
            Spacing,
            Q,
        ]
    };

    /// Pulsus I's fields: what the inspector shows beside a kit, as it always did.
    pub const CLASSIC: [PadField; 15] = {
        use PadField::*;
        [
            Voice, Level, Pan, Choke, Mute, Tune, Decay, PitchDrop, PitchDecay, Click, Drive,
            NoiseDecay, Tone, Mix, HpCutoff,
        ]
    };

    /// Does this voice have this field at all, whatever its model? The inspector and the
    /// channel rack both ask, so a hi-hat is never offered a pitch envelope it lacks.
    pub fn applies_to(self, voice: PadVoice) -> bool {
        use PadField::*;
        use PadVoice as V;
        match self {
            Voice | Level | Pan | Choke | Mute | Tune | Decay => true,
            Model => voice.models().len() > 1,
            PitchDrop | PitchDecay | Click | Phase | Sigh => voice == V::Kick,
            // Model 1 kits saturate only the kick; the editor says so on the others.
            Drive | DriveMode => true,
            NoiseDecay | Mix => voice == V::Snare,
            Tone => !matches!(voice, V::Sample | V::Tom),
            HpCutoff | Pedal | Openness | OpenDecay | BandMix | Swell => voice == V::HiHat,
            Attack | Hold | Curve => true,
            PitchEnv | PitchEnvTime => voice != V::Kick,
            Filter | Cutoff | Resonance | FilterEnv | TransAttack | TransSustain => true,
            VelLevel | VelTone | VelPitch | VelDecay => true,
            ChokeFade | ChokeOnOff | Polyphony | RoundRobin => true,
            Wires => voice == V::Snare,
            Damping | Position | Hardness => matches!(voice, V::Snare | V::Tom),
            Material | Modes | Glide | Shell => voice == V::Tom,
            Bursts | Spacing | Q => voice == V::Clap,
        }
    }

    /// Does this pad, with its voice **and model**, use the field? What the editor shows.
    pub fn applies(self, pad: &DrumPad) -> bool {
        use PadField::*;
        if !self.applies_to(pad.voice) {
            return false;
        }
        let m = pad.model_index();
        match (pad.voice, self) {
            (PadVoice::Kick, Sigh) => m == 1,
            (PadVoice::Kick, Phase) => m != 1,
            (PadVoice::Snare, Wires | Damping | Position | Hardness) => m == 1,
            (PadVoice::HiHat, HpCutoff | Pedal | Openness | OpenDecay) => m != 2,
            (PadVoice::HiHat, BandMix) => m == 1,
            (PadVoice::HiHat, Swell) => m == 3,
            (PadVoice::Tom, Shell) => pad.material == 0,
            _ => true,
        }
    }

    /// Every field this voice has, in the order an inspector should show them.
    pub fn for_voice(voice: PadVoice) -> impl Iterator<Item = PadField> {
        PadField::ALL
            .into_iter()
            .filter(move |f| f.applies_to(voice))
    }

    pub fn info(self, voice: PadVoice) -> PadParamInfo {
        use PadField::*;
        use PadVoice as V;
        let i = |label_key, min, max, default, unit, logarithmic, stepped| PadParamInfo {
            field: self,
            label_key,
            min,
            max,
            default,
            unit,
            logarithmic,
            stepped,
        };
        let top = (PadVoice::ALL.len() - 1) as f32;
        match self {
            Voice => i("pad.voice", 0.0, top, voice.index() as f32, "", false, true),
            Level => i("pad.level", -24.0, 12.0, 0.0, "dB", false, false),
            Pan => i("pad.pan", -1.0, 1.0, 0.0, "", false, false),
            // Zero is "no group", so the numbers above it are names, not amounts.
            Choke => i("pad.choke", 0.0, CHOKE_GROUPS as f32, 0.0, "", false, true),
            Mute => i("pad.mute", 0.0, 1.0, 0.0, "", false, true),
            Tune => match voice {
                V::Tom => i("pad.tune", -36.0, 48.0, 0.0, "st", false, false),
                _ => i("pad.tune", -24.0, 24.0, 0.0, "st", false, false),
            },
            Decay => match voice {
                V::Kick => i("pad.decay", 0.02, 4.0, 0.36, "s", true, false),
                V::Snare => i("pad.decay", 0.01, 1.5, 0.13, "s", true, false),
                V::HiHat => i("pad.decay", 0.01, 4.0, 0.06, "s", true, false),
                // A sample plays to its end by default; the decay is there to shorten one.
                V::Sample => i("pad.decay", 0.02, 8.0, 4.0, "s", true, false),
                V::Clap => i("pad.decay", 0.05, 2.0, 0.25, "s", true, false),
                V::Tom => i("pad.decay", 0.05, 4.0, 0.6, "s", true, false),
            },
            PitchDrop => i("pad.pitch_drop", 0.0, 48.0, 15.5, "st", false, false),
            PitchDecay => i("pad.pitch_decay", 0.005, 0.4, 0.04, "s", true, false),
            Click => i("pad.click", 0.0, 1.0, 0.35, "", false, false),
            Drive => i("pad.drive", 0.0, 1.0, 0.2, "", false, false),
            NoiseDecay => i("pad.noise_decay", 0.01, 2.0, 0.19, "s", true, false),
            Tone => match voice {
                V::Kick => i("pad.tone", 300.0, 8_000.0, 1_800.0, "Hz", true, false),
                V::HiHat => i("pad.tone", 2_000.0, 16_000.0, 9_500.0, "Hz", true, false),
                V::Clap => i("pad.tone", 400.0, 4_000.0, 1_100.0, "Hz", true, false),
                _ => i("pad.tone", 300.0, 8_000.0, 1_900.0, "Hz", true, false),
            },
            Mix => i("pad.mix", 0.0, 1.0, 0.55, "", false, false),
            HpCutoff => i(
                "pad.hp_cutoff",
                1_000.0,
                16_000.0,
                7_000.0,
                "Hz",
                true,
                false,
            ),
            Model => {
                let top = (voice.models().len() - 1) as f32;
                i("pulsus.f.model", 0.0, top, 0.0, "", false, true)
            }
            Attack => i("pulsus.f.attack", 0.0, 100.0, 0.0, "ms", false, false),
            Hold => i("pulsus.f.hold", 0.0, 500.0, 0.0, "ms", false, false),
            Curve => i("pulsus.f.curve", -1.0, 1.0, 0.0, "", false, false),
            PitchEnv => i("pulsus.f.pitch_env", -48.0, 48.0, 0.0, "st", false, false),
            PitchEnvTime => i(
                "pulsus.f.pitch_env_time",
                5.0,
                400.0,
                50.0,
                "ms",
                true,
                false,
            ),
            Filter => i("pulsus.f.filter", 0.0, 3.0, 0.0, "", false, true),
            Cutoff => i(
                "pulsus.f.cutoff",
                20.0,
                20_000.0,
                1_000.0,
                "Hz",
                true,
                false,
            ),
            Resonance => i("pulsus.f.resonance", 0.0, 1.0, 0.1, "", false, false),
            FilterEnv => i("pulsus.f.filter_env", -48.0, 48.0, 0.0, "st", false, false),
            TransAttack => i(
                "pulsus.f.trans_attack",
                -24.0,
                24.0,
                0.0,
                "dB",
                false,
                false,
            ),
            TransSustain => i(
                "pulsus.f.trans_sustain",
                -24.0,
                24.0,
                0.0,
                "dB",
                false,
                false,
            ),
            DriveMode => i("pulsus.f.drive_mode", 0.0, 3.0, 0.0, "", false, true),
            Phase => i("pulsus.f.phase", 0.0, 90.0, 0.0, "°", false, false),
            VelLevel => i("pulsus.f.vel_level", 0.0, 200.0, 100.0, "%", false, false),
            VelTone => i("pulsus.f.vel_tone", 0.0, 100.0, 50.0, "%", false, false),
            VelPitch => i("pulsus.f.vel_pitch", -12.0, 12.0, 0.0, "st", false, false),
            VelDecay => i("pulsus.f.vel_decay", -100.0, 100.0, 0.0, "%", false, false),
            ChokeFade => i("pulsus.f.choke_fade", 1.0, 250.0, 4.0, "ms", true, false),
            ChokeOnOff => i("pulsus.f.choke_on_off", 0.0, 1.0, 0.0, "", false, true),
            Polyphony => i("pulsus.f.polyphony", 0.0, 8.0, 0.0, "", false, true),
            RoundRobin => i("pulsus.f.round_robin", 0.0, 3.0, 0.0, "", false, true),
            Pedal => i("pulsus.f.pedal", 0.0, 1.0, 0.0, "", false, true),
            Openness => i("pulsus.f.openness", 0.0, 1.0, 0.0, "", false, false),
            OpenDecay => i("pulsus.f.open_decay", 0.1, 3.0, 1.2, "s", true, false),
            Sigh => i("pulsus.f.sigh", 0.0, 1.0, 0.3, "", false, false),
            Wires => i("pulsus.f.wires", 0.0, 1.0, 0.5, "", false, false),
            Material => i("pulsus.f.material", 0.0, 4.0, 0.0, "", false, true),
            Modes => i("pulsus.f.modes", 4.0, 32.0, 12.0, "", false, true),
            Damping => i("pulsus.f.damping", 0.1, 1.0, 0.5, "", false, false),
            Position => i("pulsus.f.position", 0.0, 1.0, 0.3, "", false, false),
            Hardness => i("pulsus.f.hardness", 0.0, 1.0, 0.5, "", false, false),
            Glide => i("pulsus.f.glide", 0.0, 4.0, 1.0, "st", false, false),
            Shell => i("pulsus.f.shell", 0.0, 1.0, 0.5, "", false, false),
            BandMix => i("pulsus.f.band_mix", 0.0, 1.0, 0.5, "", false, false),
            Swell => i("pulsus.f.swell", 0.0, 200.0, 40.0, "ms", false, false),
            Bursts => i("pulsus.f.bursts", 1.0, 6.0, 4.0, "", false, true),
            Spacing => i("pulsus.f.spacing", 3.0, 30.0, 10.0, "ms", false, false),
            Q => i("pulsus.f.q", 0.5, 8.0, 2.0, "", true, false),
        }
    }
}

impl PadField {
    pub fn get(self, pad: &DrumPad) -> f32 {
        use PadField::*;
        match self {
            Voice => pad.voice.index() as f32,
            Level => pad.level_db,
            Pan => pad.pan,
            Choke => pad.choke as f32,
            Mute => pad.muted as u8 as f32,
            Tune => pad.tune,
            Decay => pad.decay,
            PitchDrop => pad.pitch_drop,
            PitchDecay => pad.pitch_decay,
            Click => pad.click,
            Drive => pad.drive,
            NoiseDecay => pad.noise_decay,
            Tone => pad.tone,
            Mix => pad.mix,
            HpCutoff => pad.hp_cutoff,
            Model => pad.model as f32,
            Attack => pad.attack_ms,
            Hold => pad.hold_ms,
            Curve => pad.curve,
            PitchEnv => pad.pitch_env,
            PitchEnvTime => pad.pitch_env_ms,
            Filter => pad.filter as f32,
            Cutoff => pad.cutoff,
            Resonance => pad.resonance,
            FilterEnv => pad.filter_env,
            TransAttack => pad.trans_attack,
            TransSustain => pad.trans_sustain,
            DriveMode => pad.drive_mode as f32,
            Phase => pad.phase,
            VelLevel => pad.vel_level,
            VelTone => pad.vel_tone,
            VelPitch => pad.vel_pitch,
            VelDecay => pad.vel_decay,
            ChokeFade => pad.choke_fade_ms,
            ChokeOnOff => pad.choke_on_off as u8 as f32,
            Polyphony => pad.polyphony as f32,
            RoundRobin => pad.round_robin.index() as f32,
            Pedal => pad.pedal as u8 as f32,
            Openness => pad.openness,
            OpenDecay => pad.open_decay,
            Sigh => pad.sigh,
            Wires => pad.wires,
            Material => pad.material as f32,
            Modes => pad.modes as f32,
            Damping => pad.damping,
            Position => pad.position,
            Hardness => pad.hardness,
            Glide => pad.glide,
            Shell => pad.shell,
            BandMix => pad.band_mix,
            Swell => pad.swell_ms,
            Bursts => pad.bursts as f32,
            Spacing => pad.spacing_ms,
            Q => pad.q,
        }
    }

    /// Set the field, clamping to its legal range. Returns the value actually stored, so
    /// the command bus records what happened rather than what was asked for.
    ///
    /// Exactly one field changes. A first version of `Voice` also moved the decay to the
    /// new voice's default, and the undo fuzz caught it within a minute: the inverse
    /// restores the *voice*, not the decay it dragged along. A value outside the new
    /// voice's range is clamped by the engine and shown pinned, which is honest.
    pub fn set(self, pad: &mut DrumPad, value: f32) -> f32 {
        use PadField::*;
        let info = self.info(pad.voice);
        let v = if value.is_finite() {
            value.clamp(info.min, info.max)
        } else {
            info.default
        };
        let v = if info.stepped { v.round() } else { v };
        match self {
            Voice => pad.voice = PadVoice::from_index(v as usize),
            Level => pad.level_db = v,
            Pan => pad.pan = v,
            Choke => pad.choke = v as u8,
            Mute => pad.muted = v >= 0.5,
            Tune => pad.tune = v,
            Decay => pad.decay = v,
            PitchDrop => pad.pitch_drop = v,
            PitchDecay => pad.pitch_decay = v,
            Click => pad.click = v,
            Drive => pad.drive = v,
            NoiseDecay => pad.noise_decay = v,
            Tone => pad.tone = v,
            Mix => pad.mix = v,
            HpCutoff => pad.hp_cutoff = v,
            Model => pad.model = v as u8,
            Attack => pad.attack_ms = v,
            Hold => pad.hold_ms = v,
            Curve => pad.curve = v,
            PitchEnv => pad.pitch_env = v,
            PitchEnvTime => pad.pitch_env_ms = v,
            Filter => pad.filter = v as u8,
            Cutoff => pad.cutoff = v,
            Resonance => pad.resonance = v,
            FilterEnv => pad.filter_env = v,
            TransAttack => pad.trans_attack = v,
            TransSustain => pad.trans_sustain = v,
            DriveMode => pad.drive_mode = v as u8,
            Phase => pad.phase = v,
            VelLevel => pad.vel_level = v,
            VelTone => pad.vel_tone = v,
            VelPitch => pad.vel_pitch = v,
            VelDecay => pad.vel_decay = v,
            ChokeFade => pad.choke_fade_ms = v,
            ChokeOnOff => pad.choke_on_off = v >= 0.5,
            Polyphony => pad.polyphony = v as u8,
            RoundRobin => pad.round_robin = super::RoundRobin::from_index(v as usize),
            Pedal => pad.pedal = v >= 0.5,
            Openness => pad.openness = v,
            OpenDecay => pad.open_decay = v,
            Sigh => pad.sigh = v,
            Wires => pad.wires = v,
            Material => pad.material = v as u8,
            Modes => pad.modes = v as u8,
            Damping => pad.damping = v,
            Position => pad.position = v,
            Hardness => pad.hardness = v,
            Glide => pad.glide = v,
            Shell => pad.shell = v,
            BandMix => pad.band_mix = v,
            Swell => pad.swell_ms = v,
            Bursts => pad.bursts = v as u8,
            Spacing => pad.spacing_ms = v,
            Q => pad.q = v,
        }
        v
    }

    /// The names of a stepped field's values, as label keys, where they are names rather
    /// than numbers.
    pub fn choices(self, pad: &DrumPad) -> Option<&'static [&'static str]> {
        use PadField::*;
        Some(match self {
            Model => pad.voice.models(),
            Filter => &[
                "pulsus.filter.off",
                "pulsus.filter.lp",
                "pulsus.filter.hp",
                "pulsus.filter.bp",
            ],
            DriveMode => &[
                "pulsus.drive.soft",
                "pulsus.drive.tape",
                "pulsus.drive.clip",
                "pulsus.drive.raw",
            ],
            RoundRobin => &[
                "pulsus.rr.off",
                "pulsus.rr.cycle",
                "pulsus.rr.random",
                "pulsus.rr.no_repeat",
            ],
            Material => &[
                "pulsus.material.membrane",
                "pulsus.material.bar",
                "pulsus.material.wood",
                "pulsus.material.plate",
                "pulsus.material.bell",
            ],
            _ => return None,
        })
    }
}

/// A kit-wide field of Pulsus II, addressed by `InstrumentParam::PulsusKit`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KitField {
    /// The model (1 = Pulsus I, 2 = Pulsus II). Upgrading is setting it.
    Version,
    Seed,
    HatCc,
    HatInvert,
}

impl KitField {
    pub const ALL: [KitField; 4] = [
        KitField::Version,
        KitField::Seed,
        KitField::HatCc,
        KitField::HatInvert,
    ];

    /// `(label key, min, max, default)`; every kit field is stepped.
    pub fn range(self) -> (&'static str, f32, f32, f32) {
        match self {
            KitField::Version => ("pulsus.kit.version", 1.0, PULSUS_MODEL as f32, 2.0),
            KitField::Seed => ("pulsus.kit.seed", 0.0, 65_535.0, 1.0),
            KitField::HatCc => ("pulsus.kit.hat_cc", 0.0, 127.0, 4.0),
            KitField::HatInvert => ("pulsus.kit.hat_invert", 0.0, 1.0, 0.0),
        }
    }

    pub fn get(self, kit: &PulsusParams) -> f32 {
        match self {
            KitField::Version => kit.version as f32,
            KitField::Seed => kit.kit.seed as f32,
            KitField::HatCc => kit.kit.hat_cc as f32,
            KitField::HatInvert => kit.kit.hat_invert as u8 as f32,
        }
    }

    /// Clamp, round and store; returns what was stored.
    pub fn set(self, kit: &mut PulsusParams, value: f32) -> f32 {
        let (_, min, max, default) = self.range();
        let v = if value.is_finite() {
            value.clamp(min, max).round()
        } else {
            default
        };
        match self {
            KitField::Version => kit.version = v as u8,
            KitField::Seed => kit.kit.seed = v as u32,
            KitField::HatCc => kit.kit.hat_cc = v as u8,
            KitField::HatInvert => kit.kit.hat_invert = v >= 0.5,
        }
        v
    }
}
