//! The instrument: settings, voice allocation onto cards, ghosts for stolen voices, the
//! controllers, the render loop (voices at the oversampled rate, one decimator for their
//! sum) and what it reports.

use super::chorus::{Chorus, OutputStage};
use super::settings::{src, CalorSettings};
use super::voice::{Shared, Voice};
use super::{Quality, CARDS, GHOSTS, MAX_VOICES};
use crate::comprimo::oversample::Oversampler;
use crate::math;
use crate::modmatrix::{ModMatrix, ModRoute};
use crate::va::math::{hash3, mix64};
use crate::va::{card_trims, CardTrims, FilterCoefs};
use crate::voices::{Allocator, PlayMode};

/// Frames rendered at a time inside a block (the oversampled buffers hold four times it).
const CHUNK: usize = 64;
/// Live input's note ids sit at the top of the range, one per key; they repeat.
const LIVE_ID_BASE: u64 = u64::MAX - 256;
/// Frames the decimator and the chorus need to empty after the last voice.
const TAIL: usize = 512;

/// Telemetry layout (from slot 8; 0–2 are the frame's).
pub mod tel {
    /// Sixteen card levels.
    pub const CARD_LEVEL: usize = 8;
    /// Sixteen card keys (MIDI + 1; 0 idle).
    pub const CARD_KEY: usize = 24;
    /// Peak level into the filter (linear).
    pub const IN_PEAK: usize = 40;
    /// The newest voice's cutoff, Hz.
    pub const CUTOFF: usize = 41;
    pub const ENV1: usize = 42;
    pub const ENV2: usize = 43;
    pub const FACTOR: usize = 44;
    /// Nanoseconds per voice per base sample.
    pub const CPU_NS: usize = 45;
    pub const LATENCY: usize = 46;
    pub const NEWEST_CARD: usize = 47;
}

pub struct Calor {
    fs: f32,
    s: CalorSettings,
    cutoff_log2: f32,
    coefs: FilterCoefs,
    trims: [CardTrims; CARDS],
    matrix: ModMatrix,
    routes_shape_filter: bool,
    routes_env: bool,
    globals: [f32; src::COUNT],
    bend: f32,
    wheel: f32,
    alloc: Allocator,
    voices: [Voice; MAX_VOICES],
    ghosts: [Voice; GHOSTS],
    /// Each physical voice's last pitch: where its next note glides from.
    last_pitch: [f32; MAX_VOICES],
    /// Allocator voices with a stack still sounding.
    alloc_busy: [bool; MAX_VOICES],
    os: Oversampler,
    factor: usize,
    buf_l: [f32; CHUNK * 4],
    buf_r: [f32; CHUNK * 4],
    out_l: [f32; CHUNK],
    out_r: [f32; CHUNK],
    chorus: Chorus,
    out: OutputStage,
    clock: u64,
    rotate: u64,
    live_count: u64,
    stolen: u32,
    tail: usize,
    in_peak: f32,
    newest: usize,
    cpu_ns: f32,
}

impl std::fmt::Debug for Calor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Calor")
            .field("fs", &self.fs)
            .field("active", &self.active_voices())
            .finish()
    }
}

impl Calor {
    /// Allocates (the chorus line): build it off the audio thread.
    pub fn new(sample_rate: f32) -> Self {
        let s = CalorSettings::default();
        let mut c = Calor {
            fs: sample_rate,
            cutoff_log2: math::log2(s.cutoff),
            coefs: FilterCoefs::new(&s.filter),
            trims: [CardTrims::default(); CARDS],
            matrix: ModMatrix::default(),
            routes_shape_filter: false,
            routes_env: false,
            globals: [0.0; src::COUNT],
            bend: 0.0,
            wheel: 0.0,
            alloc: Allocator::new(s.voices),
            voices: [Voice::default(); MAX_VOICES],
            ghosts: [Voice::default(); GHOSTS],
            last_pitch: [f32::NAN; MAX_VOICES],
            alloc_busy: [false; MAX_VOICES],
            os: Oversampler::new(),
            factor: 1,
            buf_l: [0.0; CHUNK * 4],
            buf_r: [0.0; CHUNK * 4],
            out_l: [0.0; CHUNK],
            out_r: [0.0; CHUNK],
            chorus: Chorus::new(sample_rate),
            out: OutputStage::new(sample_rate),
            clock: 0,
            rotate: 0,
            live_count: 0,
            stolen: 0,
            tail: 0,
            in_peak: 0.0,
            newest: 0,
            cpu_ns: 0.0,
            s: s.clone(),
        };
        c.s.quality = Quality::Draft; // so `set` sees a change and builds the factor
        c.set(&s);
        c
    }

    pub fn settings(&self) -> &CalorSettings {
        &self.s
    }

    /// How many notes can sound at once with the current stack.
    pub fn polyphony(&self) -> usize {
        match self.s.mode {
            PlayMode::Poly => self.s.voices.clamp(1, MAX_VOICES / self.s.stack.max(1)),
            _ => 1,
        }
    }

    /// New settings (on the audio thread: no allocation).
    pub fn set(&mut self, s: &CalorSettings) {
        let quality_changed = s.quality != self.s.quality;
        let layout_changed = s.stack != self.s.stack || s.mode != self.s.mode;
        self.s.clone_from(s);
        self.s.stack = s.stack.clamp(1, 8);
        self.cutoff_log2 = math::log2(s.cutoff.max(1.0));
        self.coefs = FilterCoefs::new(&s.filter);
        let v = s.card_variance();
        for (c, t) in self.trims.iter_mut().enumerate() {
            *t = card_trims(s.seed, c, s.rolls[c], &v);
        }
        self.alloc.set_mode(s.mode, s.priority);
        self.alloc.set_polyphony(self.polyphony());
        if layout_changed {
            // Voices are laid out note × stack: a new layout releases what sounds.
            for v in self.voices.iter_mut().filter(|v| v.active) {
                v.release(0.0);
            }
            self.alloc.clear();
        }
        if quality_changed {
            self.factor = s.quality.factor();
            self.os.set_factor(self.factor);
            let sh = self.shared();
            let mut voices = self.voices;
            for v in voices.iter_mut().filter(|v| v.active) {
                v.rebuild(&sh);
            }
            self.voices = voices;
        }
    }

    /// The matrix routes, resolved by the caller to [`src`] and [`super::Target`] indices
    /// with depths in the targets' units.
    pub fn set_routes(&mut self, routes: &[ModRoute]) {
        use super::Target as T;
        self.matrix.clear();
        for r in routes {
            self.matrix.push(*r);
        }
        let hits = |ts: &[T]| {
            self.matrix
                .routes()
                .iter()
                .any(|r| ts.iter().any(|t| r.dst == *t as u16))
        };
        self.routes_shape_filter = hits(&[T::Reso, T::VcfDrive, T::Morph, T::BassComp]);
        self.routes_env = hits(&[
            T::Env1A,
            T::Env1D,
            T::Env1S,
            T::Env1R,
            T::Env2A,
            T::Env2D,
            T::Env2S,
            T::Env2R,
        ]);
    }

    pub fn set_macros(&mut self, macros: &[f32; 8]) {
        self.globals[src::MACRO1..src::MACRO1 + 8].copy_from_slice(macros);
    }

    fn shared(&self) -> Shared<'_> {
        Shared {
            s: &self.s,
            fs: self.fs,
            fs_os: self.fs * self.factor as f32,
            factor: self.factor,
            solve: self.s.quality.solve(),
            matrix: &self.matrix,
            globals: &self.globals,
            bend: self.bend,
            wheel: self.wheel,
            trims: &self.trims,
            coefs: self.coefs,
            cutoff_log2: self.cutoff_log2,
            routes_shape_filter: self.routes_shape_filter,
            routes_env: self.routes_env,
            clock: self.clock,
        }
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Samples between a note and its sound: the BLEP line and the decimator's delay.
    pub fn latency(&self) -> usize {
        match self.factor {
            1 => 2,
            2 => 17,
            _ => 19,
        }
    }

    /// The trims of each card, as they are now (the editor draws the same function).
    pub fn trims(&self) -> &[CardTrims; CARDS] {
        &self.trims
    }
}

#[path = "calor_notes.rs"]
mod notes;
pub use notes::Expression;
#[path = "calor_render.rs"]
mod render;

/// Whether a note id is live input's (one per key, repeating).
fn is_live(id: u64) -> bool {
    id >= LIVE_ID_BASE
}

/// A note's seed: its id (stable for a clip's note) — or, for live input, its id and how
/// many live notes came before.
fn note_seed(id: u64, live_count: u64) -> u64 {
    if is_live(id) {
        mix64(id ^ live_count.wrapping_mul(0x9e37_79b9_7f4a_7c15))
    } else {
        hash3(id, 0xca10, 0)
    }
}
