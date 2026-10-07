//! Gravis, the bass instrument: its DSP (ADR 0023; `research/instruments/bass.md` §8.1;
//! `docs/instruments/gravis.md`).
//!
//! One instrument, three engines — **808**, **Synth**, **String** — behind one voice:
//!
//! - a mono / legato voice from [`crate::voices::Allocator`] (note stack, last / low / high
//!   priority, the pedal), with glide by time or by rate in three shapes, a **legato
//!   window** so notes that only touch in the piano roll still slide (the snapshot sends
//!   note-offs before note-ons at one tick), and a declick crossfade on a retrigger;
//! - a **sub** derived from the main oscillator's phase, never filtered or driven;
//! - a **fundamental-locked drive** with key tracking (`gravis_out.rs`);
//! - a subsonic high-pass, stereo width above a mono crossover, a kick duck the host
//!   drives ahead of the kick, an oversampled clipper and the output gain.
//!
//! Everything steps once per sample from the start of the stream, so a render is the same
//! at every block size, and live and offline. Randomness is seeded by the note.

#[path = "gravis_808.rs"]
pub mod e808;
#[path = "gravis_filter.rs"]
pub mod filter;
#[path = "gravis_out.rs"]
pub mod out;
#[path = "gravis_string.rs"]
pub mod string;
#[path = "gravis_synth.rs"]
pub mod synth;
#[cfg(test)]
#[path = "gravis_tests.rs"]
mod tests;
#[path = "gravis_voice.rs"]
pub mod voice;

use crate::voices::{Action, Allocator, PlayMode, Priority, MAX_VOICES};
use e808::{Settings808, Voice808, HARMONICS};
use filter::DiodeCal;
use out::{Clipper, Drive, DriveSettings, Duck, DuckSettings, MonoBelow, Subsonic};
use string::{StringEngine, StringSettings, MAX_STRINGS};
use synth::{Accent, PhaseStart, SynthSettings, SynthVoice};
use voice::{hz, seed, Env, EnvTimes, Glide, GlideShape, Smooth};

/// Which engine makes the tone.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Engine {
    #[default]
    E808,
    Synth,
    String,
}

impl Engine {
    pub fn from_index(i: f32) -> Self {
        match i.round() as i32 {
            1 => Engine::Synth,
            2 => Engine::String,
            _ => Engine::E808,
        }
    }
}

/// How notes share the voice.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum VoiceMode {
    /// A note played while another is held (or within the legato window) glides without
    /// a new attack.
    #[default]
    Legato,
    /// Every note attacks; overlapping notes still glide if the glide says so.
    Mono,
    /// The String engine: one voice per string (a chord rings); the others play Legato.
    Strings,
}

impl VoiceMode {
    pub fn from_index(i: f32) -> Self {
        match i.round() as i32 {
            1 => VoiceMode::Mono,
            2 => VoiceMode::Strings,
            _ => VoiceMode::Legato,
        }
    }
}

/// When the pitch glides.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum GlideTrigger {
    #[default]
    LegatoOnly,
    Always,
    Off,
}

impl GlideTrigger {
    pub fn from_index(i: f32) -> Self {
        match i.round() as i32 {
            1 => GlideTrigger::Always,
            2 => GlideTrigger::Off,
            _ => GlideTrigger::LegatoOnly,
        }
    }
}

/// How a note's oscillator phase starts (808 and Synth).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum PhaseMode {
    #[default]
    Reset,
    Random,
    Free,
}

impl PhaseMode {
    pub fn from_index(i: f32) -> Self {
        match i.round() as i32 {
            1 => PhaseMode::Random,
            2 => PhaseMode::Free,
            _ => PhaseMode::Reset,
        }
    }
}

/// Everything Gravis does, in real units. The engine unit builds it from the spec.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GravisSettings {
    pub engine: Engine,
    pub mode: VoiceMode,
    pub priority: Priority,
    pub glide_ms: f32,
    pub glide_trigger: GlideTrigger,
    /// `glide_ms` is per octave (constant rate) rather than per glide.
    pub glide_rate: bool,
    pub glide_shape: GlideShape,
    pub legato_window_ms: f32,
    pub declick_ms: f32,
    pub bend_range_st: f32,
    /// Octave and transpose, semitones.
    pub transpose_st: f32,
    pub fine_ct: f32,
    pub a4: f32,
    pub phase: PhaseMode,
    /// 0…1: how much velocity changes the level.
    pub velocity: f32,
    pub lfo_hz: f32,
    pub lfo_shape: u8,
    pub e808: Settings808,
    pub synth: SynthSettings,
    pub string: StringSettings,
    pub sub_level: f32,
    /// Octaves below: 1 or 2.
    pub sub_octave: u8,
    pub sub_wave: u8,
    pub drive: DriveSettings,
    pub subsonic_hz: f32,
    pub clip: bool,
    pub duck_on: bool,
    pub duck: DuckSettings,
    /// 0 off, 1 the song's scale, 2 its root and fifth.
    pub key_snap: u8,
    pub gain_db: f32,
    pub mono_below_hz: f32,
}

impl Default for GravisSettings {
    fn default() -> Self {
        GravisSettings {
            engine: Engine::E808,
            mode: VoiceMode::Legato,
            priority: Priority::Last,
            glide_ms: 60.0,
            glide_trigger: GlideTrigger::LegatoOnly,
            glide_rate: false,
            glide_shape: GlideShape::Exponential,
            legato_window_ms: 5.0,
            declick_ms: 1.5,
            bend_range_st: 2.0,
            transpose_st: 0.0,
            fine_ct: 0.0,
            a4: 440.0,
            phase: PhaseMode::Reset,
            velocity: 0.6,
            lfo_hz: 2.0,
            lfo_shape: 0,
            e808: Settings808::default(),
            synth: SynthSettings::default(),
            string: StringSettings::default(),
            sub_level: 0.0,
            sub_octave: 1,
            sub_wave: 0,
            drive: DriveSettings::default(),
            subsonic_hz: 0.0,
            clip: false,
            duck_on: false,
            duck: DuckSettings::default(),
            key_snap: 0,
            gain_db: 0.0,
            mono_below_hz: 120.0,
        }
    }
}

/// What the editor draws and the modulation matrix reads, refreshed every sample.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Readout {
    /// The pitch now (fractional MIDI note, glide and bend included).
    pub pitch: f32,
    /// The pitch a glide is heading for.
    pub target: f32,
    /// The fundamental now, Hz (the 808's envelopes included).
    pub f0: f32,
    /// Amplitude envelope, 0…1.
    pub env1: f32,
    /// Modulation envelope: the synth's filter envelope, the 808's pitch envelope.
    pub env2: f32,
    /// −1…1.
    pub lfo: f32,
    pub velocity: f32,
    /// −1…1 from C4.
    pub key: f32,
    /// −1…1, seeded by the note.
    pub rnd: f32,
    /// The synth's cutoff now, Hz.
    pub cutoff: f32,
    /// The accent sweep's charge, 0…1.
    pub sweep: f32,
    /// The string sounding (−1: none) and its fret.
    pub string: f32,
    pub fret: f32,
    /// Samples since the note started.
    pub age: u32,
    /// The drive's gain for the note now, dB (0 when off).
    pub drive_db: f32,
    /// The duck's gain now, dB.
    pub duck_db: f32,
}

/// Grid (samples from the start of the stream) the string follows the pitch on.
const STRING_GRID: u64 = 16;

/// The instrument.
#[derive(Debug)]
pub struct Gravis {
    sr: f32,
    s: GravisSettings,
    harm: [f32; HARMONICS + 1],
    cal: DiodeCal,
    alloc: Allocator,
    glide: Glide,
    bend: Smooth,
    /// Per-note tuning of the sounding note (a note expression), semitones.
    expr_tuning: f32,
    note_id: u64,
    note_key: u8,
    vel: f32,
    rnd: f32,
    has_pitch: bool,
    /// Held notes by id: (id, velocity, the key as played after snapping), for a mono
    /// voice returning to one and for matching a note-off to its snapped key.
    held: [(u64, f32, u8); 16],
    v808: [Voice808; 2],
    syn: [SynthVoice; 2],
    fade: f32,
    fade_step: f32,
    fade_hz: f64,
    accent: Accent,
    strings: StringEngine,
    /// Strings mode: the note each string was plucked at, before the bend.
    string_base: [f64; MAX_STRINGS],
    string_pitch: f64,
    /// The String engine's sub: a phase at the string's pitch and an envelope.
    ssub_phase: u32,
    ssub_wraps: u32,
    ssub_env: Env,
    drive: Drive,
    subsonic: Subsonic,
    mono_below: MonoBelow,
    clip: [Clipper; 2],
    duck: Duck,
    gain: Smooth,
    window_left: u32,
    t: u64,
    lfo_phase: u32,
    lfo_inc: u32,
    song_key: Option<(u8, u16)>,
    released: [usize; MAX_VOICES],
    readout: Readout,
    configured: bool,
    /// Samples the output has been below −140 dB with every voice silent.
    tail_quiet: u32,
}

const NO_ID: u64 = u64::MAX;

impl Gravis {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8_000.0);
        let mut g = Gravis {
            sr,
            s: GravisSettings::default(),
            harm: [0.0; HARMONICS + 1],
            cal: DiodeCal::compute(),
            alloc: Allocator::new(1),
            glide: Glide::default(),
            bend: Smooth::new(4.0, sr),
            expr_tuning: 0.0,
            note_id: NO_ID,
            note_key: 60,
            vel: 0.0,
            rnd: 0.0,
            has_pitch: false,
            held: [(NO_ID, 0.0, 0); 16],
            v808: [Voice808::default(); 2],
            syn: [SynthVoice::default(); 2],
            fade: 0.0,
            fade_step: 0.0,
            fade_hz: 0.0,
            accent: Accent::new(sr),
            strings: StringEngine::new(sr),
            string_base: [0.0; MAX_STRINGS],
            string_pitch: 0.0,
            ssub_phase: 0,
            ssub_wraps: 0,
            ssub_env: Env::default(),
            drive: Drive::new(),
            subsonic: Subsonic::default(),
            mono_below: MonoBelow::default(),
            clip: [Clipper::new(), Clipper::new()],
            duck: Duck::default(),
            gain: Smooth::new(10.0, sr),
            window_left: 0,
            t: 0,
            lfo_phase: 0,
            lfo_inc: 0,
            song_key: None,
            released: [0; MAX_VOICES],
            readout: Readout::default(),
            configured: false,
            tail_quiet: 0,
        };
        g.set(&GravisSettings::default());
        g.configured = false;
        g
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &GravisSettings {
        &self.s
    }

    /// What the editor draws.
    pub fn readout(&self) -> &Readout {
        &self.readout
    }

    /// The modulation sources Gravis produces, now: `[env1, env2, lfo1, vel, key, rnd]`.
    /// Computed from the state (not the readout), so they are the same whether or not an
    /// idle stretch was skipped.
    pub fn mod_sources(&self) -> [f32; 6] {
        let (env1, env2) = match self.s.engine {
            Engine::E808 => (self.v808[0].env.value(), 0.0),
            Engine::Synth => {
                let v = &self.syn[0];
                if v.is_active() {
                    (v.aenv.value(), v.fenv.value())
                } else {
                    (0.0, 0.0)
                }
            }
            Engine::String => (self.ssub_env.value(), 0.0),
        };
        [
            env1,
            env2,
            self.lfo_value(),
            self.vel,
            (self.note_key as f32 - 60.0) / 60.0,
            self.rnd,
        ]
    }

    /// New settings (a snapshot, or the modulation's tick). Sounding voices follow.
    pub fn set(&mut self, s: &GravisSettings) {
        let prev = self.s;
        let first = !self.configured;
        if !first && s.engine != prev.engine {
            self.cut_voices();
        }
        if !first && s.mode != prev.mode {
            self.all_notes_off();
        }
        self.s = *s;
        self.harm = s.e808.harmonic_levels();
        let mode = match s.mode {
            VoiceMode::Mono => PlayMode::Mono,
            _ => PlayMode::Legato,
        };
        self.alloc.set_mode(mode, s.priority);
        let gain = 10f32.powf(s.gain_db / 20.0);
        if first {
            self.gain.snap(gain);
        } else {
            self.gain.target = gain;
        }
        self.lfo_inc = (s.lfo_hz.max(0.0) as f64 / self.sr as f64 * 4_294_967_296.0) as u32;
        let sr = self.sr;
        if s.e808.env != prev.e808.env {
            for v in &mut self.v808 {
                v.retime(&s.e808, sr);
            }
        }
        if s.synth.aenv != prev.synth.aenv || s.synth.fenv != prev.synth.fenv {
            for v in &mut self.syn {
                v.retime(&s.synth, sr);
            }
        }
        if s.string != prev.string || s.a4 != prev.a4 {
            self.strings.retune_all(&s.string, s.a4 as f64);
        }
        self.ssub_env.set(
            &EnvTimes {
                attack_ms: 3.0,
                hold_ms: 0.0,
                decay_ms: 10.0,
                sustain: 1.0,
                release_ms: s.string.release_ms,
            },
            sr,
        );
        self.configured = true;
    }

    /// The song's key, from the host: its root (0 = C) and a mask of the pitch classes in
    /// its scale (bit 0 = the root). `None` when the song has no key.
    pub fn set_song_key(&mut self, key: Option<(u8, u16)>) {
        self.song_key = key;
    }

    /// The host says a kick starts `ahead` samples from now (see `DuckSettings`).
    pub fn kick_in(&mut self, ahead: u32) {
        if self.s.duck_on {
            self.duck.kick_in(ahead);
        }
    }

    /// Pitch bend, in the semitones the host scales plain MIDI to (±2 at full travel).
    pub fn set_bend(&mut self, st: f32) {
        self.bend.target = st * self.s.bend_range_st / 2.0;
    }

    /// A note expression's tuning (MPE bend), semitones, for the note with this id.
    pub fn set_note_tuning(&mut self, id: u64, st: f32) {
        if id == self.note_id {
            self.expr_tuning = st.clamp(-48.0, 48.0);
        }
    }

    pub fn set_sustain(&mut self, down: bool) {
        if self.strings_mode() {
            return;
        }
        let n = self.alloc.set_sustain(down, &mut self.released);
        if self.released[..n].contains(&0) {
            self.release_voice();
        }
    }

    fn strings_mode(&self) -> bool {
        self.s.mode == VoiceMode::Strings && self.s.engine == Engine::String
    }

    fn note_of(&self, key: u8) -> f64 {
        key as f64 + self.s.transpose_st as f64
    }

    /// Move a key into the song's scale (or onto its root and fifth).
    fn snap_key(&self, key: u8) -> u8 {
        let (Some((root, mask)), 1..) = (self.song_key, self.s.key_snap) else {
            return key;
        };
        let mask = if self.s.key_snap == 2 {
            1 | (1 << 7)
        } else {
            mask | 1
        };
        for d in [0i32, -1, 1, -2, 2, -3, 3, -4, 4, -5, 5, -6, 6] {
            let k = key as i32 + d;
            let pc = (k - root as i32).rem_euclid(12);
            if (0..=127).contains(&k) && mask & (1 << pc) != 0 {
                return k as u8;
            }
        }
        key
    }

    fn remember(&mut self, id: u64, vel: f32, key: u8) {
        let slot = self
            .held
            .iter()
            .position(|h| h.0 == NO_ID)
            .unwrap_or(self.held.len() - 1);
        self.held[slot] = (id, vel, key);
    }

    /// Forget a note; returns the key it was played as.
    fn forget(&mut self, id: u64, key: u8) -> u8 {
        match self.held.iter().position(|h| h.0 == id) {
            Some(i) => {
                let k = self.held[i].2;
                self.held[i] = (NO_ID, 0.0, 0);
                k
            }
            None => key,
        }
    }

    fn vel_of(&self, id: u64) -> f32 {
        self.held
            .iter()
            .find(|h| h.0 == id)
            .map(|h| h.1)
            .unwrap_or(self.vel)
    }

    fn glide_samples(&self, from: f64, to: f64) -> f64 {
        let ms = if self.s.glide_rate {
            self.s.glide_ms as f64 * (to - from).abs() / 12.0
        } else {
            self.s.glide_ms as f64
        };
        ms * 0.001 * self.sr as f64
    }

    /// The pitch now plus what bends and tunes it (the string is tuned to this).
    fn extra(&self) -> f64 {
        (self.bend.value + self.expr_tuning + self.s.fine_ct * 0.01) as f64
    }

    /// Whether the voice is still making sound (for the legato window).
    fn sounding(&self) -> bool {
        match self.s.engine {
            Engine::E808 => self.v808[0].is_active(),
            Engine::Synth => self.syn[0].is_active(),
            Engine::String => self
                .strings
                .current
                .is_some_and(|c| self.strings.is_active(c)),
        }
    }

    pub fn note_on(&mut self, key: u8, id: u64, velocity: f32) {
        let key = self.snap_key(key);
        let vel = velocity.clamp(0.0, 1.0);
        self.remember(id, vel, key);
        let a4 = self.s.a4 as f64;
        if self.strings_mode() {
            let note = self.note_of(key);
            let pitch = note + self.extra();
            let s = self.s.string;
            let i = self
                .strings
                .poly_on(&s, key, id, pitch, vel, seed(id, key, 1), a4);
            self.string_base[i] = note;
            self.note_key = key;
            self.note_id = id;
            self.vel = vel;
            self.glide.jump(note);
            self.has_pitch = true;
            return;
        }
        let was_held = self.alloc.is_held(0);
        let in_window = self.window_left > 0 && self.sounding();
        match self.alloc.note_on(key, id, vel) {
            Action::Glide { key, id, .. } => {
                let v = self.vel_of(id);
                self.legato_to(key, id, v);
            }
            Action::Start {
                key, id, velocity, ..
            } => {
                if in_window && self.s.mode == VoiceMode::Legato {
                    self.legato_to(key, id, velocity);
                } else {
                    let glide = match self.s.glide_trigger {
                        GlideTrigger::Always => self.has_pitch,
                        GlideTrigger::LegatoOnly => self.has_pitch && (was_held || in_window),
                        GlideTrigger::Off => false,
                    };
                    self.start_note(key, id, velocity, glide);
                }
            }
            Action::Release { .. } | Action::Nothing => {}
        }
        self.window_left = 0;
    }

    pub fn note_off(&mut self, key: u8, id: u64) {
        let key = self.forget(id, key);
        if self.strings_mode() {
            let s = self.s.string;
            self.strings.poly_off(&s, key, id, self.s.a4 as f64);
            return;
        }
        match self.alloc.note_off(key, id) {
            Action::Glide { key, id, .. } => {
                let v = self.vel_of(id);
                if self.s.mode == VoiceMode::Mono {
                    let glide = self.s.glide_trigger != GlideTrigger::Off;
                    self.start_note(key, id, v, glide);
                } else {
                    self.legato_to(key, id, v);
                }
            }
            Action::Release { .. } => self.release_voice(),
            _ => {}
        }
    }

    /// A choke: the note stops at once (a quick release).
    pub fn choke(&mut self, key: u8, id: u64) {
        self.note_off(key, id);
    }

    /// Move the sounding voice to another note without a new attack.
    fn legato_to(&mut self, key: u8, id: u64, vel: f32) {
        let note = self.note_of(key);
        let a4 = self.s.a4 as f64;
        self.note_key = key;
        self.note_id = id;
        self.expr_tuning = 0.0;
        let glide = self.s.glide_trigger != GlideTrigger::Off;
        if glide {
            let n = self.glide_samples(self.glide.value(), note);
            self.glide.start(note, n, self.s.glide_shape);
        } else {
            self.glide.jump(note);
        }
        match self.s.engine {
            Engine::E808 => self.v808[0].env.resume(),
            Engine::Synth => {
                self.syn[0].aenv.resume();
                self.syn[0].fenv.resume();
                // An accent under a slide is still an accent: never swallowed.
                let accented = vel >= self.s.synth.accent_vel;
                if accented {
                    self.accent.trigger();
                }
                self.syn[0].accented = accented;
            }
            Engine::String => {
                let s = self.s.string;
                let cur = self.strings.current;
                let i = self.strings.choose(&s, note, cur);
                if Some(i) == cur {
                    self.strings.resume(&s, i, a4);
                    if !glide || self.s.glide_ms < 3.0 {
                        let pitch = note + self.extra();
                        self.strings.hammer(&s, i, pitch, a4);
                        self.string_pitch = pitch;
                    }
                } else {
                    if let Some(c) = cur {
                        self.strings.release(&s, c, a4);
                    }
                    let pitch = note + self.extra();
                    self.strings
                        .pluck(&s, i, pitch, vel, seed(id, key, 1), a4, true);
                    self.strings.current = Some(i);
                    self.glide.jump(note);
                    self.string_pitch = pitch;
                }
                self.ssub_env.resume();
            }
        }
        self.has_pitch = true;
    }

    /// Attack a note (the first, or a retrigger in Mono mode).
    fn start_note(&mut self, key: u8, id: u64, vel: f32, glide: bool) {
        let note = self.note_of(key);
        let s = self.s;
        let sr = self.sr;
        let a4 = s.a4 as f64;
        let amp = (1.0 - s.velocity) + s.velocity * vel;
        let sd = seed(id, key, 0);
        self.rnd = voice::Rng(sd).bipolar();
        let from = self.glide.value();
        let old_hz = hz(from + self.extra(), a4);
        self.expr_tuning = 0.0;
        if glide && self.has_pitch {
            let n = self.glide_samples(from, note);
            self.glide.start(note, n, s.glide_shape);
        } else {
            self.glide.jump(note);
        }
        self.note_key = key;
        self.note_id = id;
        self.vel = vel;
        self.has_pitch = true;
        let declick = (s.declick_ms * 0.001 * sr).round();
        match s.engine {
            Engine::E808 => {
                if self.v808[0].is_active() && declick >= 1.0 {
                    self.v808[1] = self.v808[0];
                    self.fade = 1.0;
                    self.fade_step = 1.0 / declick;
                    self.fade_hz = old_hz;
                }
                let phase = match s.phase {
                    PhaseMode::Reset => Some(0),
                    PhaseMode::Random => Some((sd >> 32) as u32),
                    PhaseMode::Free => None,
                };
                self.v808[0].start(&s.e808, sr, amp, phase, sd);
            }
            Engine::Synth => {
                if self.syn[0].is_active() && declick >= 1.0 {
                    self.syn[1] = self.syn[0];
                    self.fade = 1.0;
                    self.fade_step = 1.0 / declick;
                    self.fade_hz = old_hz;
                }
                let accented = vel >= s.synth.accent_vel;
                if accented {
                    self.accent.trigger();
                }
                let start = match s.phase {
                    PhaseMode::Reset => PhaseStart::Reset,
                    PhaseMode::Random => PhaseStart::Random(sd),
                    PhaseMode::Free => PhaseStart::Free,
                };
                self.syn[0].start(&s.synth, sr, amp, accented, start);
            }
            Engine::String => {
                let cur = self.strings.current;
                let i = self.strings.choose(&s.string, note, None);
                if let Some(c) = cur.filter(|c| *c != i) {
                    self.strings.release(&s.string, c, a4);
                }
                // A slide into the note is plucked where it starts.
                let start = if glide && self.glide.moving() {
                    from.max(s.string.open[i] as f64)
                } else {
                    note
                };
                let pitch = start + self.extra();
                self.strings
                    .pluck(&s.string, i, pitch, vel, seed(id, key, 1), a4, false);
                self.strings.current = Some(i);
                self.string_pitch = pitch;
                self.ssub_phase = 0;
                self.ssub_wraps = 0;
                self.ssub_env.gate_on();
            }
        }
    }

    fn release_voice(&mut self) {
        let a4 = self.s.a4 as f64;
        match self.s.engine {
            Engine::E808 => self.v808[0].env.gate_off(),
            Engine::Synth => {
                self.syn[0].aenv.gate_off();
                self.syn[0].fenv.gate_off();
            }
            Engine::String => {
                if let Some(c) = self.strings.current {
                    let s = self.s.string;
                    self.strings.release(&s, c, a4);
                }
                self.ssub_env.gate_off();
            }
        }
        self.window_left = (self.s.legato_window_ms * 0.001 * self.sr) as u32;
    }

    /// Release everything (transport stop, loop wrap); tails ring.
    pub fn all_notes_off(&mut self) {
        self.alloc.clear();
        self.held = [(NO_ID, 0.0, 0); 16];
        for v in &mut self.v808 {
            v.env.gate_off();
        }
        for v in &mut self.syn {
            v.aenv.gate_off();
            v.fenv.gate_off();
        }
        let s = self.s.string;
        self.strings.release_all(&s, self.s.a4 as f64);
        self.ssub_env.gate_off();
        self.window_left = 0;
    }

    /// Stop every voice at once (the engine changed).
    fn cut_voices(&mut self) {
        self.alloc.clear();
        self.held = [(NO_ID, 0.0, 0); 16];
        self.v808 = [Voice808::default(); 2];
        self.syn = [SynthVoice::default(); 2];
        self.fade = 0.0;
        self.accent.reset();
        self.strings.reset();
        self.ssub_env.reset();
        self.window_left = 0;
    }

    /// Silence at once and forget every tail (seek, panic).
    pub fn reset(&mut self) {
        self.cut_voices();
        self.drive.reset();
        self.subsonic.reset();
        self.mono_below.reset();
        for c in &mut self.clip {
            c.reset();
        }
        self.duck.reset();
        self.bend.snap(0.0);
        self.gain.snap(self.gain.target);
        self.expr_tuning = 0.0;
        self.has_pitch = false;
        self.tail_quiet = 0;
    }

    /// At rest (ADR 0031 §5.1): idle (every voice ended, every stage after it empty, every
    /// glide landed — what lets a block be skipped), no note held, the legato window shut,
    /// no bend or note tuning, and nothing that would make the next note differ from a new
    /// unit's: no glide from the last pitch, no oscillator phase carried over. The String
    /// engine is never at rest — it picks the next string from where the hand last was, as
    /// a bassist does, and a new unit's hand is nowhere. The LFO is not part of the sound
    /// here (only of the modulation the host unit reads, which checks it).
    pub fn at_rest(&self) -> bool {
        self.s.engine != Engine::String
            && self.s.phase != PhaseMode::Free
            && self.idle()
            // One voice; its allocator is never told the voice ended, and a released slot
            // gives the next note the same voice as a free one (Gravis ignores `stolen`).
            && self.alloc.is_quiet()
            && self.held.iter().all(|h| h.0 == NO_ID)
            && self.window_left == 0
            && self.bend.value == 0.0
            && self.bend.target == 0.0
            && self.expr_tuning == 0.0
            && (self.s.glide_trigger != GlideTrigger::Always || !self.has_pitch)
    }

    /// Samples skipped while at rest (ADR 0031 §5.1): what an idle block does.
    pub fn skip(&mut self, frames: usize) {
        self.t += frames as u64;
        self.lfo_phase = self
            .lfo_phase
            .wrapping_add(self.lfo_inc.wrapping_mul(frames as u32));
        self.window_left = self.window_left.saturating_sub(frames as u32);
    }

    /// Voices sounding: the 808 or synth voice (and one fading out), or the strings.
    pub fn active_voices(&self) -> usize {
        match self.s.engine {
            Engine::E808 => self.v808[0].is_active() as usize + (self.fade > 0.0) as usize,
            Engine::Synth => self.syn[0].is_active() as usize + (self.fade > 0.0) as usize,
            Engine::String => self.strings.active(),
        }
    }

    /// Whether every sample from now is exact zero until the next note (so a block can be
    /// skipped without changing any state it would have changed).
    fn idle(&self) -> bool {
        // A voice held at zero sustain still runs (its phase and envelopes move), so only
        // a voice that has ended counts.
        let voices = match self.s.engine {
            Engine::E808 => {
                !self.v808[0].is_active() && self.v808[0].is_silent() && self.fade == 0.0
            }
            Engine::Synth => !self.syn[0].is_active() && self.fade == 0.0,
            Engine::String => self.strings.is_silent() && self.ssub_env.is_silent(),
        };
        voices
            && self.accent.env == 0.0
            && self.accent.sweep == 0.0
            && self.drive.is_zero()
            && self.subsonic.is_zero()
            && self.mono_below.is_zero()
            && self.clip.iter().all(Clipper::is_zero)
            && self.duck.is_idle()
            && self.gain.settled()
            && self.bend.settled()
            && !self.glide.moving()
    }

    /// Render one segment, **added** into `left`/`right`. Returns whether anything was
    /// written (`false` only when it was exact zero).
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        let n = left.len().min(right.len());
        if self.idle() {
            self.t += n as u64;
            self.lfo_phase = self
                .lfo_phase
                .wrapping_add(self.lfo_inc.wrapping_mul(n as u32));
            self.window_left = self.window_left.saturating_sub(n as u32);
            return false;
        }
        let mut wrote = false;
        for i in 0..n {
            let (a, b) = self.sample();
            if a != 0.0 || b != 0.0 {
                wrote = true;
            }
            left[i] += a;
            right[i] += b;
        }
        wrote
    }

    /// The LFO's value now, −1…1.
    #[inline]
    fn lfo_value(&self) -> f32 {
        let t = self.lfo_phase as f64 / 4_294_967_296.0;
        (match self.s.lfo_shape {
            1 => {
                if t < 0.5 {
                    4.0 * t - 1.0
                } else {
                    3.0 - 4.0 * t
                }
            }
            2 => {
                if t < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            3 => 1.0 - 2.0 * t,
            _ => (std::f64::consts::TAU * t).sin(),
        }) as f32
    }

    /// The strings follow the pitch on a fixed grid: a slide fret by fret (or smoothly on a
    /// fretless), a bend smoothly.
    fn follow_strings(&mut self, base: f64, extra: f64) {
        let a4 = self.s.a4 as f64;
        let s = self.s.string;
        if self.strings_mode() {
            for i in 0..MAX_STRINGS {
                if self.strings.is_active(i) {
                    let want = self.string_base[i] + extra;
                    if (self.strings.note_of(i) - want).abs() > 1e-9 {
                        self.strings.set_pitch(&s, i, want, a4, STRING_GRID as u32);
                    }
                }
            }
            return;
        }
        let Some(i) = self.strings.current else {
            return;
        };
        let pitch = base + extra;
        if (pitch - self.string_pitch).abs() < 1e-9 {
            return;
        }
        if self.glide.moving() && !s.fretless {
            let open = s.open[i] as f64;
            let fretted = open + (base - open).round() + extra;
            if (fretted - self.strings.note_of(i)).abs() > 1e-9 {
                self.strings
                    .slide_to(&s, i, fretted, a4, STRING_GRID as u32);
            }
        } else {
            self.strings.slide_to(&s, i, pitch, a4, STRING_GRID as u32);
        }
        self.string_pitch = pitch;
    }

    #[inline]
    fn sample(&mut self) -> (f32, f32) {
        let sr = self.sr;
        let bend = self.bend.next();
        let base = self.glide.next();
        let extra = (bend + self.expr_tuning + self.s.fine_ct * 0.01) as f64;
        let pitch = base + extra;
        let a4 = self.s.a4 as f64;
        let base_hz = hz(pitch, a4);
        let sub_div = if self.s.sub_octave >= 2 { 4 } else { 2 };
        let sub_wave = self.s.sub_wave as u32;
        self.readout.lfo = self.lfo_value();
        self.lfo_phase = self.lfo_phase.wrapping_add(self.lfo_inc);
        let (mut main, mut side, mut sub, f0);
        match self.s.engine {
            Engine::E808 => {
                let s = &self.s.e808;
                let (m, sb) = self.v808[0].next(s, &self.harm, base_hz, sr, sub_div, sub_wave);
                main = m;
                sub = sb;
                side = 0.0;
                if self.fade > 0.0 {
                    let (fm, fs) =
                        self.v808[1].next(s, &self.harm, self.fade_hz, sr, sub_div, sub_wave);
                    main += fm * self.fade;
                    sub += fs * self.fade;
                    self.fade = (self.fade - self.fade_step).max(0.0);
                }
                f0 = self.v808[0].f0;
                self.readout.env1 = self.v808[0].env.value();
                self.readout.age = self.v808[0].age;
            }
            Engine::Synth => {
                let s = &self.s.synth;
                let acc = self.accent.next(s.accent, s.accent_sweep);
                let (m, sd, sb) =
                    self.syn[0].next(s, base_hz, pitch, sr, sub_div, sub_wave, acc, &self.cal);
                main = m;
                side = sd;
                sub = sb;
                if self.fade > 0.0 {
                    let (fm, fsd, fs) = self.syn[1].next(
                        s,
                        self.fade_hz,
                        pitch,
                        sr,
                        sub_div,
                        sub_wave,
                        acc,
                        &self.cal,
                    );
                    main += fm * self.fade;
                    side += fsd * self.fade;
                    sub += fs * self.fade;
                    self.fade = (self.fade - self.fade_step).max(0.0);
                }
                f0 = self.syn[0].f0;
                self.readout.env1 = self.syn[0].aenv.value();
                self.readout.env2 = self.syn[0].fenv.value();
                self.readout.cutoff = self.syn[0].cutoff_now;
                self.readout.sweep = self.accent.sweep;
            }
            Engine::String => {
                if self.t.is_multiple_of(STRING_GRID) {
                    self.follow_strings(base, extra);
                }
                main = self.strings.next(&self.s.string);
                side = 0.0;
                f0 = match self.strings.current {
                    Some(i) if !self.strings_mode() => self.strings.f0(i),
                    _ => base_hz,
                };
                // The sub: a phase at the string's pitch, octaves down, with its own
                // envelope (the string has none).
                let env = self.ssub_env.next();
                sub = 0.0;
                if env > 0.0 {
                    const SCALE: f64 = 4_294_967_296.0;
                    let inc_f = (f0 / sr as f64).clamp(0.0, 0.49);
                    let t = self.ssub_phase as f64 / SCALE;
                    let st = (t + (self.ssub_wraps % sub_div) as f64) / sub_div as f64;
                    let w = match sub_wave {
                        1 => {
                            if st < 0.5 {
                                4.0 * st - 1.0
                            } else {
                                3.0 - 4.0 * st
                            }
                        }
                        2 => e808::square(st, inc_f / sub_div as f64),
                        _ => (std::f64::consts::TAU * st).sin(),
                    };
                    let (nx, wrapped) = self
                        .ssub_phase
                        .overflowing_add((inc_f * SCALE).round() as u32);
                    self.ssub_phase = nx;
                    if wrapped {
                        self.ssub_wraps = self.ssub_wraps.wrapping_add(1);
                    }
                    sub = w as f32
                        * env
                        * 0.5
                        * ((1.0 - self.s.velocity) + self.s.velocity * self.vel);
                }
                self.readout.env1 = env;
                if let Some(i) = self.strings.current {
                    let p = self.strings.played(i);
                    self.readout.string = i as f32;
                    self.readout.fret = p.fret;
                } else {
                    self.readout.string = -1.0;
                }
            }
        }
        self.readout.pitch = pitch as f32;
        self.readout.target = (self.glide.target() + extra) as f32;
        self.readout.f0 = f0 as f32;
        self.readout.velocity = self.vel;
        self.readout.key = ((self.note_key as f32) - 60.0) / 60.0;
        self.readout.rnd = self.rnd;

        // The output stage.
        let srd = sr as f64;
        let driven = self.drive.process(main, &self.s.drive, f0, pitch, srd);
        self.readout.drive_db = if self.s.drive.amount > 0.0 {
            self.s.drive.gain_db(pitch) as f32
        } else {
            0.0
        };
        let mut mid = driven;
        if self.s.sub_level > 0.0 {
            mid += sub * self.s.sub_level;
        }
        mid = self.subsonic.process(mid, self.s.subsonic_hz, srd);
        let side = if side != 0.0 || !self.mono_below.is_zero() {
            self.mono_below.process(side, self.s.mono_below_hz, srd)
        } else {
            0.0
        };
        let duck = if self.s.duck_on || !self.duck.is_idle() {
            self.duck.next(&self.s.duck, srd)
        } else {
            1.0
        };
        self.readout.duck_db = self.duck.db() as f32;
        let g = self.gain.next() * duck;
        let (mut a, mut b) = if side == 0.0 {
            let m = mid * g;
            (m, m)
        } else {
            ((mid + side) * g, (mid - side) * g)
        };
        if self.s.clip || !self.clip[0].is_zero() || !self.clip[1].is_zero() {
            // Mono (every engine but a wide synth): one clipper for both sides.
            let mono = a == b && self.clip[1].is_zero();
            a = self.clip[0].process(a, self.s.clip);
            b = if mono {
                a
            } else {
                self.clip[1].process(b, self.s.clip)
            };
        }
        if !a.is_finite() || !b.is_finite() {
            // Never let a NaN out; start clean.
            self.reset();
            a = 0.0;
            b = 0.0;
        }
        // The tail gate: once every voice is silent and the output has stayed under −140 dB
        // for 50 ms, the filters' last residue is zeroed so the track can go idle.
        let voices_silent = match self.s.engine {
            Engine::E808 => self.v808[0].is_silent() && self.fade == 0.0,
            Engine::Synth => self.syn[0].is_silent() && self.fade == 0.0,
            Engine::String => self.strings.active() == 0 && self.ssub_env.is_silent(),
        };
        if voices_silent && a.abs() < voice::SILENCE && b.abs() < voice::SILENCE {
            self.tail_quiet += 1;
            if self.tail_quiet as f32 > 0.05 * sr {
                self.drive.reset();
                self.subsonic.reset();
                self.mono_below.reset();
                for c in &mut self.clip {
                    c.reset();
                }
                self.accent.reset();
                self.strings.zero_filters();
                self.tail_quiet = 0;
            }
        } else {
            self.tail_quiet = 0;
        }
        self.window_left = self.window_left.saturating_sub(1);
        self.t += 1;
        (a, b)
    }
}
