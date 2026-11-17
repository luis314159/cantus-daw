//! Saltus, the pitch unit — octaver, pitch shifter and harmoniser, whammy (ADR 0132)
//! — behind the engine's plug-in contract. Owned by the saltus builder.
//!
//! [`settings`] reads the parameters (`cantus_core::fx::saltus`) into the DSP's settings
//! (`cantus_dsp::saltus`); the editor draws from the same reading. The notes of the slot's
//! MIDI source (ADR 0022) arrive through [`FxUnit::note`], to the sample: the chord or the
//! melody the harmony sings, and the notes that press the whammy's pedal. **The latency is
//! fixed by the settings** — 128 samples at 48 kHz for the mono shifter, 4 095 for the poly
//! engine, none for the analog octave — and reported to delay compensation; a change of mode
//! changes it as a parameter does, never the audio.
//!
//! ## Telemetry
//!
//! | Index | What |
//! |---|---|
//! | 0 | sample rate, Hz |
//! | 1 | latency, samples |
//! | 2 | the engine running: 0 the mono shifter, 1 the poly engine, 2 the divider |
//! | 3 | the note heard, MIDI against the reference (meaningful when flag 1) |
//! | 4, 5, 6 | each voice's shift now, semitones (meaningful when its flag; the clean octave's three) |
//! | 7 | the whammy's pedal as followed, 0 … 1 |
//! | 8 | the input level, dBFS |
//! | 9, 10, 11 | the analog octave's levels: one down, two down, one up (linear) |
//! | 12 | the pitch classes held on MIDI (bit 0 = C) |
//! | 13 | flags: 1 a note heard, 2/4/8 voice 1/2/3 sounding |
//! | 14 | the whammy's target, 0 … 1 (the toe while a note plays it) |

use super::{FxNote, FxUnit, Telemetry, TELEMETRY_LEN};
use cantus_core::EffectSpec;
use cantus_dsp::saltus::{
    Engine, HarmonySource, Mode, NoteEvent, OctaveStyle, Path, Saltus, SaltusSettings,
    VoiceSettings,
};
use cantus_dsp::tonus::Scale;

pub const T_SAMPLE_RATE: usize = 0;
pub const T_LATENCY: usize = 1;
pub const T_PATH: usize = 2;
pub const T_NOTE: usize = 3;
pub const T_SHIFT: usize = 4;
pub const T_PEDAL: usize = 7;
pub const T_LEVEL: usize = 8;
pub const T_OCTAVES: usize = 9;
pub const T_HELD: usize = 12;
pub const T_FLAGS: usize = 13;
pub const T_TARGET: usize = 14;
pub const FLAG_NOTE: u32 = 1;
/// Voice `i` sounding: `FLAG_VOICE << i`.
pub const FLAG_VOICE: u32 = 2;
/// The voices the telemetry carries (two, or the clean octave's three).
pub const VOICES: usize = 3;

const _: () = assert!(T_TARGET < TELEMETRY_LEN);

/// Each voice's keys, voice 1 then voice 2: interval, cents, degrees, level, balance.
pub const VOICE_KEYS: [[&str; 5]; 2] = [
    ["st1", "ct1", "deg1", "lvl1", "pan1"],
    ["st2", "ct2", "deg2", "lvl2", "pan2"],
];

/// Saltus's parameters as the DSP wants them.
pub fn settings(spec: &EffectSpec) -> SaltusSettings {
    let idx = |k: &str| spec.get(k).round().max(0.0) as usize;
    let voice = |[st, ct, deg, lvl, pan]: [&str; 5]| VoiceSettings {
        semitones: spec.get(st).round(),
        cents: spec.get(ct),
        degree: spec.get(deg).round() as i32,
        level: spec.get(lvl),
        pan: spec.get(pan),
    };
    SaltusSettings {
        mode: Mode::from_index(idx("mode")),
        engine: Engine::from_index(idx("engine")),
        octave_style: OctaveStyle::from_index(idx("ostyle")),
        harmony_source: HarmonySource::from_index(idx("hsrc")),
        voices: [voice(VOICE_KEYS[0]), voice(VOICE_KEYS[1])],
        dry: spec.get("dry"),
        key: idx("key").min(11) as u8,
        scale: Scale::from_index(idx("scale")),
        formants: spec.get("formant") >= 0.5,
        throat: spec.get("throat"),
        pedal: spec.get("pedal"),
        heel: spec.get("heel").round(),
        toe: spec.get("toe").round(),
        glide_ms: spec.get("glide"),
        whammy_notes: spec.get("wsrc") >= 0.5,
        sub1: spec.get("sub1"),
        sub2: spec.get("sub2"),
        up1: spec.get("up1"),
        a4: spec.get("a4"),
        out_db: spec.get("out"),
    }
}

/// The telemetry of a running Saltus.
pub fn encode(d: &Saltus) -> Telemetry {
    let mut t = [0.0; TELEMETRY_LEN];
    t[T_SAMPLE_RATE] = d.sample_rate();
    t[T_LATENCY] = d.latency() as f32;
    t[T_PATH] = match d.path() {
        Path::Psola => 0.0,
        Path::Poly => 1.0,
        Path::Divider => 2.0,
    };
    let mut flags = 0u32;
    if let Some(n) = d.detected() {
        t[T_NOTE] = n;
        flags |= FLAG_NOTE;
    }
    for i in 0..VOICES {
        if let Some(s) = d.voice_shift(i) {
            t[T_SHIFT + i] = s;
            flags |= FLAG_VOICE << i;
        }
    }
    t[T_PEDAL] = d.pedal_now();
    t[T_LEVEL] = d.level_db();
    t[T_OCTAVES..T_OCTAVES + 3].copy_from_slice(&d.octave_levels());
    t[T_HELD] = d.held_classes() as f32;
    t[T_FLAGS] = flags as f32;
    t[T_TARGET] = d.pedal_target();
    t
}

/// A telemetry block, read back.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Reading {
    pub sample_rate: f32,
    pub latency: usize,
    /// 0 the mono shifter, 1 the poly engine, 2 the divider.
    pub path: u8,
    /// The note heard, MIDI.
    pub note: Option<f32>,
    /// Each voice's shift, semitones, while it sounds.
    pub shifts: [Option<f32>; VOICES],
    pub pedal: f32,
    pub target: f32,
    pub level_db: f32,
    pub octaves: [f32; 3],
    pub held: u16,
}

pub fn read(t: &Telemetry) -> Reading {
    let flags = t[T_FLAGS].max(0.0) as u32;
    let mut shifts = [None; VOICES];
    for (i, s) in shifts.iter_mut().enumerate() {
        if flags & (FLAG_VOICE << i) != 0 {
            *s = Some(t[T_SHIFT + i]);
        }
    }
    Reading {
        sample_rate: t[T_SAMPLE_RATE],
        latency: t[T_LATENCY].max(0.0) as usize,
        path: t[T_PATH].clamp(0.0, 2.0) as u8,
        note: (flags & FLAG_NOTE != 0).then_some(t[T_NOTE]),
        shifts,
        pedal: t[T_PEDAL],
        target: t[T_TARGET],
        level_db: t[T_LEVEL],
        octaves: [t[T_OCTAVES], t[T_OCTAVES + 1], t[T_OCTAVES + 2]],
        held: t[T_HELD].max(0.0) as u16,
    }
}

#[derive(Clone, Debug)]
pub struct Unit {
    dsp: Saltus,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            dsp: Saltus::new(sample_rate),
        }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        let s = settings(spec);
        if s != *self.dsp.settings() {
            self.dsp.set(&s);
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right);
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn at_rest(&self) -> bool {
        self.dsp.at_rest()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn telemetry(&self) -> Telemetry {
        encode(&self.dsp)
    }

    fn note(&mut self, offset: usize, note: FxNote) {
        self.dsp.note(
            offset,
            match note {
                FxNote::On { pitch, .. } => NoteEvent::On(pitch),
                FxNote::Off { pitch } => NoteEvent::Off(pitch),
                FxNote::AllOff => NoteEvent::AllOff,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    /// The defaults of the table read as the DSP's defaults: the null rule holds through
    /// the mapping.
    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let spec = EffectSpec::new(EffectKind::PitchShift);
        assert_eq!(settings(&spec), SaltusSettings::default());
    }

    /// Every key of the table reaches the DSP: moved alone, the settings change.
    #[test]
    fn every_key_reaches_the_dsp() {
        let d = settings(&EffectSpec::new(EffectKind::PitchShift));
        for info in cantus_core::fx::saltus::PARAMS {
            let mut spec = EffectSpec::new(EffectKind::PitchShift);
            let v = if info.default == info.max {
                info.min
            } else {
                info.max
            };
            spec.set(info.key, v).unwrap();
            assert_ne!(settings(&spec), d, "{} does not reach the DSP", info.key);
        }
    }

    /// At its defaults the unit is the input delayed by its latency, to the bit.
    #[test]
    fn the_defaults_are_the_input_delayed() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::PitchShift));
        let lat = u.latency();
        assert_eq!(lat, 128);
        let x: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
        let (mut l, mut r) = (x.clone(), x.clone());
        u.process(&mut l, &mut r, None);
        assert!(l[..lat].iter().chain(&r[..lat]).all(|v| *v == 0.0));
        for ch in [&l, &r] {
            assert!(ch[lat..]
                .iter()
                .zip(&x)
                .all(|(a, b)| a.to_bits() == b.to_bits()));
        }
    }

    /// The latency follows the mode, the engine and the octave's style.
    #[test]
    fn the_latency_is_fixed_by_the_settings() {
        let mut u = Unit::new(48_000.0);
        let mut spec = EffectSpec::new(EffectKind::PitchShift);
        for (k, v, want) in [
            ("engine", 1.0, 4_095),
            ("mode", 3.0, 4_095),
            ("mode", 2.0, 0),
            ("ostyle", 1.0, 128),
            ("mode", 1.0, 128),
        ] {
            spec.set(k, v).unwrap();
            u.configure(&spec);
            assert_eq!(u.latency(), want, "{k} = {v}");
        }
    }

    #[test]
    fn telemetry_reads_back() {
        let mut u = Unit::new(48_000.0);
        let mut spec = EffectSpec::new(EffectKind::PitchShift);
        spec.set("mode", 3.0).unwrap();
        spec.set("pedal", 0.5).unwrap();
        u.configure(&spec);
        u.note(
            0,
            FxNote::On {
                pitch: 64,
                velocity: 90,
            },
        );
        let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
        u.process(&mut l, &mut r, None);
        let t = read(&u.telemetry());
        assert_eq!((t.sample_rate, t.latency, t.path), (48_000.0, 128, 0));
        assert_eq!((t.pedal, t.target), (0.5, 0.5));
        assert_eq!(t.held, 1 << 4);
    }
}
