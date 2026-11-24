//! Fides — the resonator instrument (round 41, ADR 0150): its DSP. An exciter (pluck, pick,
//! a bow by a friction curve, a mallet, noise, a live input) into one or two resonators —
//! a waveguide string, tube or pipe ([`string`]), or a bank of modes: a bar, a marimba's
//! tuned bar, a tine, a plate, a membrane, a drumhead, a bell, a glass, a pan ([`modal`]) —
//! in parallel or in series, then a body ([`body`]) and the output stage.
//!
//! **Determinism.** Every voice's control step (its modulation, its pitch, its resonators'
//! design) runs on a grid of [`CTRL`] samples counted from the start of the stream, never
//! from the start of a block, and once at the note's start; between steps the waveguide's
//! lengths glide sample by sample. Every random number is a hash of the instance's seed,
//! the note's key and its id. So a render is the same at every block size, offline and in
//! real time, and with the voices on the pool's threads ([`Fides::process_par`], each
//! voice into its own lane, the lanes summed in voice order — ADR 0038).
//!
//! **Rest.** A voice ends when its exciter is done and its resonators have gone still
//! (a waveguide whose every wave passed the junction below −160 dB; modes below −180 dB),
//! their state zeroed; the body and the output filters zero theirs the same way. Then
//! [`Fides::at_rest`] is exact: nothing but zeros comes out, and the unit is as a new one.

#[path = "fides_body.rs"]
pub mod body;
#[path = "fides_bow2.rs"]
pub mod bow2;
#[path = "fides_exciter.rs"]
pub mod exciter;
#[path = "fides_modal.rs"]
pub mod modal;
#[path = "fides_render.rs"]
mod render;
#[path = "fides_string.rs"]
pub mod string;
#[path = "fides_tables.rs"]
pub mod tables;
#[cfg(test)]
#[path = "fides_tests.rs"]
mod tests;
#[path = "fides_voice.rs"]
pub mod voice;

use crate::lamina::voice::HammerShape;
use crate::modmatrix::{ModMatrix, ModRoute};
use crate::par::Lanes;
use crate::voices::{Action, Allocator, PlayMode};
use body::{Body, BodySettings};
use exciter::{ExciterSettings, NoiseSettings, Rng};
use tables::ResKind;
use voice::Voice;

/// The control grid, in samples from the start of the stream.
pub const CTRL: usize = 32;
/// The most voices one instance plays (`poly`'s maximum).
pub const MAX_VOICES: usize = 32;
/// Voices fading out after they were taken by a new note.
pub const GHOSTS: usize = 4;
/// The render's chunk: voices into lanes this long, then the body.
pub const CHUNK: usize = 256;

/// One resonator's settings.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ResSettings {
    pub kind: ResKind,
    pub decay_s: f32,
    pub material: f32,
    pub decay_key: f32,
    pub inharm: f32,
    pub listen: f32,
    pub modes: usize,
    pub tune_st: f32,
    pub fine_ct: f32,
    pub keytrack: f32,
    pub level_db: f32,
}

impl Default for ResSettings {
    fn default() -> Self {
        ResSettings {
            kind: ResKind::String,
            decay_s: 4.0,
            material: 0.45,
            decay_key: 0.3,
            inharm: 0.0,
            listen: 0.37,
            modes: 32,
            tune_st: 0.0,
            fine_ct: 0.0,
            keytrack: 1.0,
            level_db: 0.0,
        }
    }
}

/// Everything the unit sets (`cantus_engine::inst::fides::settings`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FidesSettings {
    /// The model a song was made with (ADR 0059's rule): 1, round 41's; 2, round 42's bow
    /// (`bow2`: Arcus's junction, its pitch held by the fundamental it hears). Only a
    /// waveguide string drawn by the bow differs; everything else is model 1's to the bit.
    pub model: u8,
    pub poly: usize,
    pub mode: PlayMode,
    pub retrig: bool,
    pub glide_ms: f32,
    pub bend_st: f32,
    pub transpose_st: f32,
    pub fine_ct: f32,
    pub a4: f32,
    pub velocity: f32,
    pub release_s: f32,
    pub vib_depth_ct: f32,
    pub vib_rate_hz: f32,
    pub vib_delay_ms: f32,
    pub trem_depth: f32,
    pub trem_rate_hz: f32,
    pub seed: u32,
    pub exc: ExciterSettings,
    pub noise: NoiseSettings,
    pub mpe_press: f32,
    pub mpe_slide: f32,
    pub a: ResSettings,
    pub b_on: bool,
    pub b: ResSettings,
    pub serial: bool,
    pub mix: f32,
    pub body: BodySettings,
    pub width: f32,
    pub lowcut_hz: f32,
    pub highcut_hz: f32,
    pub gain_db: f32,
}

impl Default for FidesSettings {
    fn default() -> Self {
        FidesSettings {
            model: 2,
            poly: 16,
            mode: PlayMode::Poly,
            retrig: true,
            glide_ms: 0.0,
            bend_st: 2.0,
            transpose_st: 0.0,
            fine_ct: 0.0,
            a4: 440.0,
            velocity: 0.7,
            release_s: 0.4,
            vib_depth_ct: 0.0,
            vib_rate_hz: 5.5,
            vib_delay_ms: 300.0,
            trem_depth: 0.0,
            trem_rate_hz: 5.0,
            seed: 0,
            exc: ExciterSettings::default(),
            noise: NoiseSettings::default(),
            mpe_press: 0.5,
            mpe_slide: 0.5,
            a: ResSettings::default(),
            b_on: false,
            b: ResSettings {
                kind: ResKind::Tube,
                decay_s: 1.0,
                ..ResSettings::default()
            },
            serial: false,
            mix: 0.5,
            body: BodySettings::default(),
            width: 0.4,
            lowcut_hz: 10.0,
            highcut_hz: 20_000.0,
            gain_db: 0.0,
        }
    }
}

/// What a modulation route can move, in the unit's own units (the engine scales a
/// route's depth into them: a knob's travel in its taper).
pub mod target {
    /// Semitones.
    pub const PITCH: u16 = 0;
    /// dB.
    pub const EXC_LEVEL: u16 = 1;
    /// Octaves.
    pub const EXC_CONTACT: u16 = 2;
    /// Octaves.
    pub const EXC_TONE: u16 = 3;
    pub const EXC_NOISE: u16 = 4;
    pub const EXC_POS: u16 = 5;
    pub const BOW_PRESSURE: u16 = 6;
    pub const BOW_SPEED: u16 = 7;
    pub const NOISE_LEVEL: u16 = 8;
    /// Octaves.
    pub const NOISE_COLOR: u16 = 9;
    /// Octaves of T60.
    pub const A_DECAY: u16 = 10;
    pub const A_MATERIAL: u16 = 11;
    /// Semitones.
    pub const A_TUNE: u16 = 12;
    /// dB.
    pub const A_LEVEL: u16 = 13;
    pub const B_DECAY: u16 = 14;
    pub const B_MATERIAL: u16 = 15;
    pub const B_TUNE: u16 = 16;
    pub const B_LEVEL: u16 = 17;
    pub const MIX: u16 = 18;
    /// Cents.
    pub const VIB_DEPTH: u16 = 19;
    /// −1 … 1.
    pub const PAN: u16 = 20;
    /// dB, the voice's.
    pub const GAIN: u16 = 21;
    /// The instance's (global sources only): the body's mix and the tremolo's depth.
    pub const BODY_MIX: u16 = 22;
    pub const TREM_DEPTH: u16 = 23;
    pub const COUNT: usize = 24;
    /// The first target the voices do not read.
    pub const FIRST_GLOBAL: u16 = BODY_MIX;
}

/// The voice-scope sources, in the core's `SOURCES` order: env1, lfo1, vel, key, rnd,
/// mpe.bend, mpe.press, mpe.slide. The globals follow: macros 1…8, mw, at, pb, expr, sus.
pub const VOICE_SOURCES: usize = 8;
pub const GLOBAL_SOURCES: usize = 13;
pub const SOURCES: usize = VOICE_SOURCES + GLOBAL_SOURCES;

/// Telemetry slots (from the frame's own, 8).
pub mod tel {
    pub const EXCITER: usize = 8;
    pub const RES_A: usize = 9;
    pub const RES_B: usize = 10;
    pub const KEY: usize = 11;
    pub const F0: usize = 12;
    pub const LEVEL: usize = 13;
    pub const BOW: usize = 14;
    pub const PRESSURE: usize = 15;
    pub const POS: usize = 16;
    pub const PEAK: usize = 17;
    pub const CPU_NS: usize = 18;
    pub const BODY: usize = 19;
    pub const ALIVE: usize = 20;
    /// The newest voice's first modes' amplitudes (A's), and their ratios.
    pub const MODES: usize = 24;
    pub const MODE_COUNT: usize = 16;
    pub const RATIOS: usize = 40;
}

/// The instrument.
#[derive(Clone, Debug)]
pub struct Fides {
    sr: f32,
    s: FidesSettings,
    /// r42:tuning — each key's offset in semitones from 12-TET at A440 (ADR 0155).
    tuning: [f32; 128],
    voices: Vec<Voice>,
    alloc: Allocator,
    matrix: ModMatrix,
    globals: [f32; GLOBAL_SOURCES],
    /// The pitch bend now, −1 … 1, and whether the sustain pedal is down.
    bend: f32,
    sustain: bool,
    clock: u64,
    lanes: Lanes,
    used: [bool; MAX_VOICES + GHOSTS],
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    body: Body,
    out: body::Output,
    hammer: HammerShape,
    hammer_area: f32,
    stolen: u32,
    newest: usize,
    peak: f32,
    cpu_ns: f32,
    /// The instance-wide targets' values now (the body's mix, the tremolo's depth).
    global_mods: [f32; 2],
}

impl Fides {
    /// Build an instance (allocates: never on the audio thread).
    pub fn new(sample_rate: f32) -> Self {
        let hammer = HammerShape::new();
        let hammer_area = hammer.shape.iter().sum::<f32>() / hammer.shape.len() as f32;
        let s = FidesSettings::default();
        let mut alloc = Allocator::new(s.poly);
        alloc.set_mode(s.mode, Default::default());
        let mut body = Body::new(sample_rate);
        body.set(&s.body);
        let mut out = body::Output::new(sample_rate);
        out.set(&s);
        Fides {
            sr: sample_rate,
            s,
            tuning: [0.0; 128], // r42:tuning
            voices: (0..MAX_VOICES + GHOSTS)
                .map(|_| Voice::new(sample_rate))
                .collect(),
            alloc,
            matrix: ModMatrix::default(),
            globals: [0.0; GLOBAL_SOURCES],
            bend: 0.0,
            sustain: false,
            clock: 0,
            lanes: Lanes::new(MAX_VOICES + GHOSTS, CHUNK),
            used: [false; MAX_VOICES + GHOSTS],
            buf_l: vec![0.0; CHUNK],
            buf_r: vec![0.0; CHUNK],
            body,
            out,
            hammer,
            hammer_area,
            stolen: 0,
            newest: 0,
            peak: 0.0,
            cpu_ns: 0.0,
            global_mods: [0.0; 2],
        }
    }

    pub fn settings(&self) -> &FidesSettings {
        &self.s
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// New settings (audio thread, no allocation): the voices take them at their next
    /// control step.
    /// r42:tuning — the project's tuning (ADR 0155), for the notes struck from now on (a
    /// ringing resonator keeps its pitch); the same table again changes nothing.
    pub fn set_tuning(&mut self, table: &[f32; 128]) {
        if !crate::tuning::same(&self.tuning, table) {
            self.tuning = *table;
        }
    }

    pub fn set(&mut self, s: &FidesSettings) {
        self.s = *s;
        self.s.poly = s.poly.clamp(1, MAX_VOICES);
        self.alloc.set_polyphony(self.s.poly);
        self.alloc.set_mode(self.s.mode, Default::default());
        self.body.set(&s.body);
        self.out.set(&self.s);
        // A modulated mix is applied again over the new one at the next chunk.
        self.global_mods[0] = 0.0;
    }

    /// The modulation routes (`dst` a [`target`], `depth` in its units).
    pub fn set_routes(&mut self, routes: &[ModRoute]) {
        self.matrix.clear();
        for r in routes {
            self.matrix.push(*r);
        }
    }

    pub fn routes(&self) -> &[ModRoute] {
        self.matrix.routes()
    }

    pub fn set_macros(&mut self, m: &[f32; 8]) {
        self.globals[..8].copy_from_slice(m);
    }

    /// The mod wheel, channel pressure, expression (0 … 1).
    pub fn set_controller(&mut self, which: usize, value: f32) {
        // 8 mw · 9 at · 11 expr
        if let Some(g) = self.globals.get_mut(which) {
            *g = value;
        }
    }

    /// The pitch bend in semitones as the processor sends it (±2 for plain MIDI): the
    /// instrument's own range scales it.
    pub fn pitch_bend(&mut self, semitones: f32) {
        self.bend = (semitones / 2.0).clamp(-1.0, 1.0);
        self.globals[10] = self.bend;
    }

    pub fn sustain(&mut self, down: bool) {
        self.globals[12] = if down { 1.0 } else { 0.0 };
        if down == self.sustain {
            return;
        }
        self.sustain = down;
        let mut released = [0usize; crate::voices::MAX_VOICES];
        let n = self.alloc.set_sustain(down, &mut released);
        for v in released.iter().take(n) {
            if let Some(v) = self.voices.get_mut(*v) {
                v.release(&self.s);
            }
        }
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active()).count()
    }

    pub fn polyphony(&self) -> usize {
        self.alloc.polyphony()
    }

    /// The voices, for the editor's pictures and tests.
    pub fn voices(&self) -> &[Voice] {
        &self.voices
    }

    pub fn newest(&self) -> &Voice {
        &self.voices[self.newest.min(MAX_VOICES - 1)]
    }

    /// The seeded random generator of a note.
    fn rng(&self, key: u8, id: u64) -> Rng {
        Rng::of(self.s.seed, key, id)
    }

    pub fn note_on(&mut self, key: u8, velocity: f32, id: u64) {
        match self.alloc.note_on(key, id, velocity) {
            Action::Start {
                voice,
                key,
                id,
                velocity,
                stolen,
            } => self.start(voice, key, id, velocity, stolen),
            Action::Glide { voice, key, id } => {
                let s = self.s;
                let tune = crate::tuning::at(&self.tuning, key); // r42:tuning
                if let Some(v) = self.voices.get_mut(voice) {
                    v.tune_next(tune);
                    v.glide_to(&s, key, id);
                }
            }
            Action::Release { voice } => {
                let s = self.s;
                if let Some(v) = self.voices.get_mut(voice) {
                    v.release(&s);
                }
            }
            Action::Nothing => {}
        }
    }

    fn start(&mut self, voice: usize, key: u8, id: u64, velocity: f32, stolen: bool) {
        if voice >= MAX_VOICES {
            return;
        }
        if stolen {
            // The taken voice fades out in a ghost slot; the note starts clean.
            let ghost = (MAX_VOICES..MAX_VOICES + GHOSTS)
                .find(|&g| !self.voices[g].active())
                .unwrap_or_else(|| {
                    (MAX_VOICES..MAX_VOICES + GHOSTS)
                        .min_by(|&x, &y| self.voices[x].fade().total_cmp(&self.voices[y].fade()))
                        .unwrap_or(MAX_VOICES)
                });
            self.voices[ghost].clear();
            self.voices.swap(voice, ghost);
            self.voices[ghost].fade_out(self.sr);
            self.voices[voice].clear();
            self.stolen += 1;
        }
        let mut keep = false;
        // Struck again while ringing: the new note takes the ringing resonators over, and
        // the old voice gives its slot back (a bar is one bar).
        if self.s.retrig {
            let other = (0..MAX_VOICES)
                .find(|&u| u != voice && self.voices[u].active() && self.voices[u].key() == key);
            if let Some(u) = other {
                let (a, b) = two(&mut self.voices, voice, u);
                a.take_resonators(b);
                self.alloc.voice_finished(u);
                keep = true;
            }
        }
        let rng = self.rng(key, id);
        let sh = voice::Shared {
            s: &self.s,
            sr: self.sr,
            matrix: &self.matrix,
            globals: &self.globals,
            bend_st: self.bend * self.s.bend_st,
            hammer: &self.hammer,
            hammer_area: self.hammer_area,
            clock: self.clock,
            input: None,
        };
        self.voices[voice].tune_next(crate::tuning::at(&self.tuning, key)); // r42:tuning
        self.voices[voice].start(&sh, key, id, velocity, rng, keep);
        self.newest = voice;
    }

    pub fn note_off(&mut self, key: u8, id: u64) {
        if let Action::Release { voice } = self.alloc.note_off(key, id) {
            let s = self.s;
            if let Some(v) = self.voices.get_mut(voice) {
                v.release(&s);
            }
        }
    }

    /// Stop the note at once (a choke): it fades out as a taken voice does.
    pub fn choke(&mut self, key: u8, id: u64) {
        let found = (0..MAX_VOICES).find(|&u| {
            self.voices[u].active() && self.voices[u].id() == id && self.voices[u].key() == key
        });
        if let Some(u) = found {
            self.voices[u].fade_out(self.sr);
            let _ = self.alloc.note_off(key, id);
        }
    }

    /// A per-note expression: `0` tuning (semitones), `1` pressure, `2` slide (0 … 1).
    pub fn expression(&mut self, id: u64, which: u8, value: f32) {
        for v in self.voices.iter_mut().take(MAX_VOICES) {
            if v.active() && v.id() == id {
                v.expression(which, value);
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        // The pedal up, then every held note let go: the voices ring out on their own and
        // give their slots back when they end.
        self.sustain(false);
        for v in 0..MAX_VOICES {
            if self.voices[v].active() && self.alloc.is_held(v) {
                let (key, id) = (self.voices[v].key(), self.voices[v].id());
                let _ = self.alloc.note_off(key, id);
            }
        }
        let s = self.s;
        for v in self.voices.iter_mut() {
            v.release(&s);
        }
        // A mono voice's note stack is forgotten too: its voice rings out in a ghost slot
        // (as it is, no fade), so the next note starts on a free voice.
        if self.s.mode != PlayMode::Poly {
            for v in 0..MAX_VOICES {
                if !self.voices[v].active() {
                    continue;
                }
                match (MAX_VOICES..MAX_VOICES + GHOSTS).find(|&g| !self.voices[g].active()) {
                    Some(g) => self.voices.swap(v, g),
                    None => self.voices[v].fade_out(self.sr),
                }
            }
            self.alloc.clear();
        }
    }

    pub fn reset(&mut self) {
        for v in self.voices.iter_mut() {
            v.clear();
        }
        self.alloc.clear();
        self.body.reset();
        self.out.reset();
        self.clock = 0;
        self.sustain = false;
        self.bend = 0.0;
        self.globals[8..].fill(0.0);
        self.stolen = 0;
        self.peak = 0.0;
    }

    /// Exactly at rest (see the module's notes): no voice, the body and the output still,
    /// the played controllers home and no route reading a source that moves on its own.
    pub fn at_rest(&self) -> bool {
        self.voices.iter().all(|v| !v.active())
            && self.alloc.is_clear()
            && self.body.is_still()
            && self.out.is_still()
            && self.globals[8..].iter().all(|g| *g == 0.0)
    }

    /// The engine skipped `frames` samples at rest.
    pub fn skip(&mut self, frames: usize) {
        self.clock += frames as u64;
    }

    pub fn take_stolen(&mut self) -> u32 {
        std::mem::take(&mut self.stolen)
    }
}

/// Two distinct elements of a slice, mutably.
fn two<T>(v: &mut [T], a: usize, b: usize) -> (&mut T, &mut T) {
    assert!(a != b);
    if a < b {
        let (x, y) = v.split_at_mut(b);
        (&mut x[a], &mut y[0])
    } else {
        let (x, y) = v.split_at_mut(a);
        (&mut y[0], &mut x[b])
    }
}
