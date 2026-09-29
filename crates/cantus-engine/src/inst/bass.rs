//! Gravis — the bass instrument (808, modelled electric, synth): the engine's unit. Owned
//! by this instrument's builder (ADR 0023). The DSP is `cantus_dsp::gravis`; this reads the
//! spec into its settings, runs the modulation matrix and reports telemetry.
//!
//! **Modulation** is evaluated on a fixed grid of [`CTRL`] samples counted from the start
//! of the stream, never from the start of a block: a block is cut at the grid points, the
//! sources are read (envelopes, the LFO, the note, the controllers, the macros), the routes
//! add `shape(source) · depth · via` of the target's travel (in its knob's taper, so a
//! cutoff moves in octaves) to its value, and the settings are rebuilt. With no live route
//! the settings are only rebuilt by a snapshot.
//!
//! **Host hooks** this unit offers and the processor does not call yet (the wiring is the
//! coordinator's, `docs/instruments/gravis.md` § Host hooks): [`Unit::kick_in`] (the kick
//! duck, driven from the kick track's note events ahead of the kick) and
//! [`Unit::set_song_key`] (snapping notes to the song's key).

use super::{Controller, InstrumentUnit, NoteExpression, NoteOn, Telemetry, TELEMETRY_LEN};
use cantus_core::inst::bass::{self as model, ix, COUNT, PARAMS};
use cantus_core::inst::modulation::{MACRO_KEYS, SLOTS};
use cantus_core::NativeSpec;
use cantus_dsp::gravis::e808::Settings808;
use cantus_dsp::gravis::filter::FilterKind;
use cantus_dsp::gravis::out::{DriveSettings, DriveType, DuckSettings};
use cantus_dsp::gravis::string::{open_strings, Fingering, StringSettings, StringType, Technique};
use cantus_dsp::gravis::synth::{SynthSettings, Wave};
use cantus_dsp::gravis::voice::{EnvTimes, GlideShape};
use cantus_dsp::gravis::{Engine, GlideTrigger, Gravis, GravisSettings, PhaseMode, VoiceMode};
use cantus_dsp::modmatrix::{ModCurve, ModMatrix, ModRoute};
use cantus_dsp::voices::Priority;

/// The modulation grid, in samples from the start of the stream.
pub const CTRL: u64 = 32;

/// Telemetry slots (from the frame's `TEL_OWN`).
pub mod tel {
    /// 0 808 · 1 Synth · 2 String.
    pub const ENGINE: usize = 8;
    /// The pitch now, a fractional MIDI note.
    pub const PITCH: usize = 9;
    /// Where a glide is heading.
    pub const TARGET: usize = 10;
    /// The fundamental now, Hz.
    pub const F0: usize = 11;
    /// The amplitude envelope, 0…1.
    pub const ENV: usize = 12;
    /// The synth's cutoff now, Hz.
    pub const CUTOFF: usize = 13;
    /// The accent sweep's charge, 0…1.
    pub const SWEEP: usize = 14;
    /// The string sounding (−1 none) and its fret.
    pub const STRING: usize = 15;
    pub const FRET: usize = 16;
    /// Milliseconds since the note began.
    pub const AGE_MS: usize = 17;
    /// The drive's gain for the note now, dB.
    pub const DRIVE_DB: usize = 18;
    /// The duck's gain now, dB.
    pub const DUCK_DB: usize = 19;
    /// The block's output peak, linear.
    pub const PEAK: usize = 20;
    /// The note's velocity.
    pub const VELOCITY: usize = 21;
}

/// The sources this unit produces, in the order of `model::SOURCES`.
const N_SOURCES: usize = 18;

/// Every number of the spec, by index of [`PARAMS`].
pub fn values(spec: &NativeSpec) -> [f32; COUNT] {
    let mut v = [0.0; COUNT];
    for (o, info) in v.iter_mut().zip(PARAMS) {
        *o = spec.get(info.key);
    }
    v
}

/// The DSP's settings from a spec (the editor draws with these too).
pub fn settings(spec: &NativeSpec) -> GravisSettings {
    settings_from(&values(spec))
}

/// The DSP's settings from a value array.
pub fn settings_from(v: &[f32; COUNT]) -> GravisSettings {
    let round = |i: usize| v[i].round();
    let on = |i: usize| v[i] >= 0.5;
    let strings = round(ix::STR_STRINGS).clamp(4.0, 6.0) as usize;
    GravisSettings {
        engine: Engine::from_index(v[ix::ENGINE]),
        mode: VoiceMode::from_index(v[ix::MODE]),
        priority: Priority::from_index(v[ix::PRIORITY]),
        glide_ms: v[ix::GLIDE],
        glide_trigger: GlideTrigger::from_index(v[ix::GLIDE_TRIGGER]),
        glide_rate: on(ix::GLIDE_RATE),
        glide_shape: GlideShape::from_index(v[ix::GLIDE_SHAPE]),
        legato_window_ms: v[ix::LEGATO_WINDOW],
        declick_ms: v[ix::DECLICK],
        bend_range_st: v[ix::BEND_RANGE],
        transpose_st: 12.0 * round(ix::OCTAVE) + round(ix::TRANSPOSE),
        fine_ct: v[ix::FINE],
        a4: v[ix::A4],
        phase: PhaseMode::from_index(v[ix::PHASE]),
        velocity: v[ix::VELOCITY],
        lfo_hz: v[ix::LFO_RATE],
        lfo_shape: round(ix::LFO_SHAPE) as u8,
        e808: Settings808 {
            shape: v[ix::E808_SHAPE],
            pitch_st: v[ix::E808_PITCH],
            pitch_ms: v[ix::E808_PITCH_TIME],
            knock: v[ix::E808_KNOCK],
            click: v[ix::E808_CLICK],
            click_hz: v[ix::E808_CLICK_TONE],
            env: EnvTimes {
                attack_ms: v[ix::E808_ATTACK],
                hold_ms: v[ix::E808_HOLD],
                decay_ms: v[ix::E808_DECAY],
                sustain: v[ix::E808_SUSTAIN],
                release_ms: v[ix::E808_RELEASE],
            },
            sigh: v[ix::E808_SIGH],
            harmonics: v[ix::E808_HARMONICS],
            tilt_db: v[ix::E808_TILT],
            even: v[ix::E808_EVEN],
        },
        synth: SynthSettings {
            wave: Wave::from_index(v[ix::SYN_WAVE]),
            pw: v[ix::SYN_PW],
            unison: round(ix::SYN_UNISON).clamp(1.0, 7.0) as usize,
            detune: v[ix::SYN_DETUNE],
            spread: v[ix::SYN_SPREAD],
            width: v[ix::SYN_WIDTH],
            osc2: v[ix::SYN_OSC2],
            osc2_ratio: ((round(ix::SYN_OSC2_SEMI) + v[ix::SYN_OSC2_FINE] * 0.01) as f64 / 12.0)
                .exp2(),
            filter: FilterKind::from_index(v[ix::SYN_FILTER]),
            cutoff: v[ix::SYN_CUTOFF],
            reso: v[ix::SYN_RESO],
            bass_comp: v[ix::SYN_BASS_COMP],
            drive: v[ix::SYN_DRIVE],
            env_oct: v[ix::SYN_ENV],
            key_track: v[ix::SYN_KEY_TRACK],
            fenv: EnvTimes {
                attack_ms: v[ix::SYN_F_ATTACK],
                hold_ms: 0.0,
                decay_ms: v[ix::SYN_F_DECAY],
                sustain: v[ix::SYN_F_SUSTAIN],
                release_ms: v[ix::SYN_F_RELEASE],
            },
            aenv: EnvTimes {
                attack_ms: v[ix::SYN_ATTACK],
                hold_ms: 0.0,
                decay_ms: v[ix::SYN_DECAY],
                sustain: v[ix::SYN_SUSTAIN],
                release_ms: v[ix::SYN_RELEASE],
            },
            accent: v[ix::SYN_ACCENT],
            accent_sweep: on(ix::SYN_ACCENT_SWEEP),
            accent_vel: round(ix::SYN_ACCENT_VEL) / 127.0,
        },
        string: StringSettings {
            strings,
            open: open_strings(strings, round(ix::STR_TUNING) as usize),
            scale_mm: v[ix::STR_SCALE],
            frets: round(ix::STR_FRETS) as usize,
            fretless: on(ix::STR_FRETLESS),
            kind: StringType::from_index(v[ix::STR_TYPE]),
            stiffness: v[ix::STR_STIFFNESS],
            sustain_s: v[ix::STR_SUSTAIN],
            bright: v[ix::STR_BRIGHT],
            age: v[ix::STR_AGE],
            action_mm: v[ix::STR_ACTION],
            buzz: v[ix::STR_BUZZ],
            technique: Technique::from_index(v[ix::STR_TECHNIQUE]),
            pluck_mm: v[ix::STR_PLUCK],
            hardness: v[ix::STR_HARDNESS],
            pop_vel: round(ix::STR_POP_VEL) / 127.0,
            mute: v[ix::STR_MUTE],
            release_ms: v[ix::STR_RELEASE],
            noise: v[ix::STR_NOISE],
            fingering: Fingering::from_index(v[ix::STR_FINGERING]),
            pu_mm: [v[ix::STR_PU1], v[ix::STR_PU2]],
            pu_level: [v[ix::STR_PU1_LEVEL], v[ix::STR_PU2_LEVEL]],
            humbucker: on(ix::STR_PU_TYPE),
            pu_height_mm: v[ix::STR_PU_HEIGHT],
            piezo: v[ix::STR_PIEZO],
            tone: v[ix::STR_TONE],
            humanize: v[ix::STR_HUMANIZE],
        },
        sub_level: v[ix::SUB_LEVEL],
        sub_octave: round(ix::SUB_OCTAVE).clamp(1.0, 2.0) as u8,
        sub_wave: round(ix::SUB_WAVE) as u8,
        drive: DriveSettings {
            amount: v[ix::DRIVE],
            kind: DriveType::from_index(v[ix::DRIVE_TYPE]),
            keytrack_db: v[ix::DRIVE_KEYTRACK],
            ref_note: round(ix::DRIVE_REF),
            lock: on(ix::DRIVE_LOCK),
        },
        subsonic_hz: v[ix::SUBSONIC],
        clip: on(ix::CLIP),
        duck_on: round(ix::DUCK_SOURCE) >= 1.0,
        duck: DuckSettings {
            depth_db: v[ix::DUCK_DEPTH],
            pre_ms: v[ix::DUCK_PRE],
            hold_ms: v[ix::DUCK_HOLD],
            release_ms: v[ix::DUCK_RELEASE],
            shape: round(ix::DUCK_SHAPE) as u8,
        },
        key_snap: round(ix::KEY_SNAP) as u8,
        gain_db: v[ix::GAIN],
        mono_below_hz: v[ix::SYN_MONO_BELOW],
    }
}

#[derive(Debug)]
pub struct Unit {
    g: Gravis,
    sample_rate: f32,
    base: [f32; COUNT],
    matrix: ModMatrix,
    /// mw, at, pb (−1…1), expr.
    controllers: [f32; 4],
    macros: [f32; 8],
    t: u64,
    peak: f32,
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        Unit {
            g: Gravis::new(sample_rate),
            sample_rate,
            base: values(&NativeSpec::new(cantus_core::InstrumentKind::Bass)),
            matrix: ModMatrix::default(),
            controllers: [0.0; 4],
            macros: [0.0; 8],
            t: 0,
            peak: 0.0,
        }
    }

    /// **Host hook**: a kick on the duck's source track starts `ahead` samples from now
    /// (at most the duck's pre time). See `docs/instruments/gravis.md` § Host hooks.
    pub fn kick_in(&mut self, ahead: u32) {
        self.g.kick_in(ahead);
    }

    /// **Host hook**: the song's key — its root (0 = C) and the pitch classes of its scale as
    /// a bit mask (bit 0 = the root) — or `None`.
    pub fn set_song_key(&mut self, key: Option<(u8, u16)>) {
        self.g.set_song_key(key);
    }

    /// The DSP, for tests and the editor's previews.
    pub fn dsp(&self) -> &Gravis {
        &self.g
    }

    /// Read the sources and apply every route: the settings for the next `CTRL` samples.
    fn modulate(&mut self) {
        let dsp = self.g.mod_sources();
        let mut src = [0.0f32; N_SOURCES];
        src[..6].copy_from_slice(&dsp);
        src[6..10].copy_from_slice(&self.controllers);
        src[10..18].copy_from_slice(&self.macros);
        let mut off = [0.0f32; COUNT];
        self.matrix.apply(&src, &mut off);
        let mut v = self.base;
        for (i, o) in off.iter().enumerate() {
            if *o != 0.0 {
                // The depth is a fraction of the knob's travel, in the knob's taper: a
                // logarithmic control (a cutoff, a time) is modulated in octaves.
                let info = &PARAMS[i];
                let x = if info.logarithmic {
                    let span = (info.max / info.min).ln();
                    let pos = (v[i] / info.min).ln() / span + o;
                    info.min * (pos.clamp(0.0, 1.0) * span).exp()
                } else {
                    (v[i] + o * (info.max - info.min)).clamp(info.min, info.max)
                };
                v[i] = if info.stepped { x.round() } else { x };
            }
        }
        self.g.set(&settings_from(&v));
    }
}

impl InstrumentUnit for Unit {
    fn configure(&mut self, spec: &NativeSpec, _assets: &super::Assets) {
        self.base = values(spec);
        for (m, key) in MACRO_KEYS.iter().enumerate() {
            self.macros[m] = spec.get(key);
        }
        self.matrix.clear();
        for keys in SLOTS.iter() {
            let (Some(src), Some(dst)) = (spec.text(keys.src), spec.text(keys.dst)) else {
                continue;
            };
            if spec.get(keys.on) < 0.5 {
                continue;
            }
            let (Some(s), Some(d)) = (
                model::SOURCES.iter().position(|t| *t == src),
                model::index(dst),
            ) else {
                continue;
            };
            let via = spec
                .text(keys.via)
                .and_then(|v| model::SOURCES.iter().position(|t| *t == v))
                .map(|i| i as u16);
            self.matrix.push(ModRoute {
                src: s as u16,
                dst: d as u16,
                depth: spec.get(keys.amt),
                via,
                curve: ModCurve::from_index(spec.get(keys.curve)),
            });
        }
        if self.matrix.is_empty() {
            self.g.set(&settings_from(&self.base));
        } else {
            self.modulate();
        }
    }

    fn note_on(&mut self, note: NoteOn) {
        self.g.note_on(note.key, note.id, note.velocity);
    }

    fn note_off(&mut self, key: u8, id: u64, _velocity: f32) {
        self.g.note_off(key, id);
    }

    fn choke(&mut self, key: u8, id: u64) {
        self.g.choke(key, id);
    }

    fn expression(&mut self, id: u64, expression: NoteExpression, value: f32) {
        if expression == NoteExpression::Tuning {
            self.g.set_note_tuning(id, value);
        }
    }

    fn controller(&mut self, controller: Controller, value: f32) {
        match controller {
            Controller::PitchBend => {
                self.g.set_bend(value);
                self.controllers[2] = (value / 2.0).clamp(-1.0, 1.0);
            }
            Controller::ModWheel => self.controllers[0] = value,
            Controller::Pressure => self.controllers[1] = value,
            Controller::Expression => self.controllers[3] = value,
            Controller::Sustain => self.g.set_sustain(value >= 0.5),
            Controller::Cc(_) => {}
        }
    }

    fn all_notes_off(&mut self) {
        self.g.all_notes_off();
    }

    fn reset(&mut self) {
        self.g.reset();
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        let n = left.len().min(right.len());
        let mut wrote = false;
        if self.matrix.is_empty() {
            wrote = self.g.process(&mut left[..n], &mut right[..n]);
            self.t += n as u64;
        } else {
            let mut at = 0;
            while at < n {
                if self.t.is_multiple_of(CTRL) {
                    self.modulate();
                }
                let k = ((CTRL - self.t % CTRL) as usize).min(n - at);
                wrote |= self
                    .g
                    .process(&mut left[at..at + k], &mut right[at..at + k]);
                at += k;
                self.t += k as u64;
            }
        }
        self.peak = if wrote {
            left[..n]
                .iter()
                .chain(&right[..n])
                .fold(0f32, |m, x| m.max(x.abs()))
        } else {
            0.0
        };
        wrote
    }

    fn active_voices(&self) -> usize {
        self.g.active_voices()
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        let s = self.g.settings();
        let r = self.g.readout();
        t[0] = self.g.active_voices() as f32;
        t[1] = if s.engine == Engine::String && s.mode == VoiceMode::Strings {
            s.string.strings as f32
        } else {
            1.0
        };
        t[tel::ENGINE] = match s.engine {
            Engine::E808 => 0.0,
            Engine::Synth => 1.0,
            Engine::String => 2.0,
        };
        t[tel::PITCH] = r.pitch;
        t[tel::TARGET] = r.target;
        t[tel::F0] = r.f0;
        t[tel::ENV] = r.env1;
        t[tel::CUTOFF] = r.cutoff;
        t[tel::SWEEP] = r.sweep;
        t[tel::STRING] = r.string;
        t[tel::FRET] = r.fret;
        t[tel::AGE_MS] = r.age as f32 * 1000.0 / self.sample_rate;
        t[tel::DRIVE_DB] = r.drive_db;
        t[tel::DUCK_DB] = r.duck_db;
        t[tel::PEAK] = self.peak;
        t[tel::VELOCITY] = r.velocity;
        t
    }
}
