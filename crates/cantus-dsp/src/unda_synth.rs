//! The instrument: voices and their allocator, the control grid, the free-running LFOs,
//! the voice sum at the oversampled rate and the one decimator.

use super::decim::Decimator;
use super::modsrc::{self, unit, LfoMode};
use super::voice::{Globals, Voice};
use super::{factory, source, target, Settings, CONTROL, SOURCES};
use crate::par::Lanes;
use crate::voices::{Action, Allocator, PlayMode, MAX_VOICES as ALLOC_MAX};

/// The most voices Unda plays at once.
pub const MAX_VOICES: usize = 32;
/// The largest chunk the voices render at once, at the oversampled rate.
const CHUNK: usize = CONTROL * 4;
/// Below this the decimator's state is let go, so an idle instrument is exactly silent.
const GATE: f32 = 1e-7;

/// What the editor draws, refreshed with every block.
#[derive(Copy, Clone, Debug, Default)]
pub struct Meter {
    pub voices: usize,
    pub stolen: u32,
    /// The newest voice's table positions (0…1), envelope levels, LFO values and
    /// filter cutoffs (Hz, modulated).
    pub pos: [f32; 3],
    pub env: [f32; 4],
    pub lfo: [f32; 8],
    pub cutoff: [f32; 2],
    pub res: [f32; 2],
    pub peak: [f32; 2],
    /// Unison lanes running (a CPU cue), summed over voices and oscillators.
    pub lanes: usize,
}

#[derive(Copy, Clone, Debug, Default)]
struct FreeLfo {
    phase: f32,
    cycle: u64,
    r0: f32,
    r1: f32,
}

#[derive(Debug)]
pub struct Unda {
    sr: f32,
    s: Box<Settings>,
    voices: Vec<Voice>,
    alloc: Allocator,
    /// Samples since the stream began: the control grid counts from here.
    clock: u64,
    ages: u64,
    decim: Decimator,
    acc_l: [f32; CHUNK],
    acc_r: [f32; CHUNK],
    /// Global sources: macros, wheel, aftertouch, bend, expression, sustain.
    globals: [f32; SOURCES],
    free: [FreeLfo; 8],
    free_val: [f32; 8],
    last_pitch: Option<f32>,
    stolen: u32,
    peak: [f32; 2],
    released: [usize; ALLOC_MAX],
    /// The parallel render's lanes (one span per voice at 4×), the piece each voice ended
    /// in, and which voices a span rendered (ADR 0038, `unda_par.rs`).
    lanes: Lanes,
    fin: [usize; MAX_VOICES],
    used: [bool; MAX_VOICES],
}

impl Unda {
    /// Allocates the voices and makes sure the factory tables exist: never on the audio
    /// thread.
    pub fn new(sample_rate: f32) -> Self {
        let _ = factory::tables();
        let _ = factory::sub_table();
        let _ = super::grana::windows();
        Unda {
            sr: sample_rate,
            s: Box::default(),
            voices: (0..MAX_VOICES)
                .map(|_| Voice::new(sample_rate * 2.0))
                .collect(),
            alloc: Allocator::new(16),
            clock: 0,
            ages: 0,
            decim: Decimator::new(),
            acc_l: [0.0; CHUNK],
            acc_r: [0.0; CHUNK],
            globals: [0.0; SOURCES],
            free: [FreeLfo::default(); 8],
            free_val: [0.0; 8],
            last_pitch: None,
            stolen: 0,
            peak: [0.0; 2],
            released: [0; ALLOC_MAX],
            lanes: Lanes::new(MAX_VOICES, par::SPAN * 4),
            fin: [0; MAX_VOICES],
            used: [false; MAX_VOICES],
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &Settings {
        &self.s
    }

    /// New settings. Allocation-free: the settings are copied into the box held.
    pub fn set(&mut self, s: &Settings) {
        (*self.s).clone_from(s);
        self.alloc.set_polyphony(s.polyphony.min(MAX_VOICES));
        self.alloc.set_mode(s.mode, s.priority);
    }

    pub fn note_on(&mut self, key: u8, id: u64, velocity: f32) {
        let action = self.alloc.note_on(key, id, velocity);
        self.act(action);
    }

    pub fn note_off(&mut self, key: u8, id: u64, velocity: f32) {
        let action = self.alloc.note_off(key, id);
        if let Action::Release { voice } = action {
            self.voices[voice].release(velocity);
        } else {
            self.act(action);
        }
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Start {
                voice,
                key,
                id,
                velocity,
                stolen,
            } => {
                if stolen {
                    self.stolen += 1;
                }
                let from = if self.s.glide_s > 0.0 {
                    let legato = self.s.mode != PlayMode::Poly && stolen;
                    if self.s.glide_always || legato {
                        self.last_pitch
                    } else {
                        None
                    }
                } else {
                    None
                };
                self.ages += 1;
                let (s, sr) = (&*self.s, self.sr);
                let v = &mut self.voices[voice];
                v.start(s, key, id, velocity, stolen, from, self.ages, sr);
                let g = Globals {
                    sr: self.sr,
                    oversample: s.oversample,
                    src: &self.globals,
                    free_lfo: &self.free_val,
                };
                v.control(s, &g, true);
                self.last_pitch = Some(key as f32);
            }
            Action::Glide { voice, key, id } => {
                self.voices[voice].glide_to(&self.s, key, id, self.sr);
                self.last_pitch = Some(key as f32);
            }
            Action::Release { voice } => self.voices[voice].release(0.0),
            Action::Nothing => {}
        }
    }

    /// Per-note expressions, by note id: tuning (st), pressure, slide, volume, pan.
    ///
    /// Tuning, pressure and slide glide to each new value at the control rate
    /// (`expr_glide`, ADR 0080); a note that has not sounded yet takes its first values at
    /// once, its first samples already bent.
    pub fn expression(&mut self, id: u64, which: usize, value: f32) {
        use crate::expr_glide::set;
        let s = &*self.s;
        let g = Globals {
            sr: self.sr,
            oversample: s.oversample,
            src: &self.globals,
            free_lfo: &self.free_val,
        };
        for v in self.voices.iter_mut().filter(|v| v.active && v.id == id) {
            let fresh = v.fresh();
            match which {
                0 => set(&mut v.tuning, &mut v.expr_to[0], value, fresh),
                1 => set(
                    &mut v.pressure,
                    &mut v.expr_to[1],
                    value.clamp(0.0, 1.0),
                    fresh,
                ),
                2 => set(
                    &mut v.slide,
                    &mut v.expr_to[2],
                    value.clamp(0.0, 1.0),
                    fresh,
                ),
                3 => v.volume = value.clamp(0.0, 4.0),
                4 => v.pan = value.clamp(-1.0, 1.0),
                _ => {}
            }
            if fresh && which < 3 {
                v.control(s, &g, true);
            }
        }
    }

    /// A global source (a macro, the wheel, aftertouch, bend −1…1, expression).
    pub fn set_global(&mut self, index: usize, value: f32) {
        if index < SOURCES {
            self.globals[index] = value;
        }
    }

    /// The sustain pedal.
    pub fn sustain(&mut self, down: bool) {
        self.globals[source::SUS] = if down { 1.0 } else { 0.0 };
        let n = self.alloc.set_sustain(down, &mut self.released);
        let released = self.released;
        for &v in &released[..n] {
            self.voices[v].release(0.0);
        }
    }

    pub fn all_notes_off(&mut self) {
        let mut out = [0; ALLOC_MAX];
        self.alloc.set_sustain(false, &mut out);
        for v in self.voices.iter_mut() {
            if v.active && !v.releasing() {
                v.release(0.0);
                let _ = self.alloc.note_off(v.key, v.id);
            }
        }
        self.globals[source::SUS] = 0.0;
    }

    pub fn reset(&mut self) {
        for v in &mut self.voices {
            v.kill();
        }
        self.alloc.clear();
        self.decim.clear();
        self.clock = 0;
        self.free = [FreeLfo::default(); 8];
        self.free_val = [0.0; 8];
        self.last_pitch = None;
        self.peak = [0.0; 2];
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// At rest (ADR 0031 §5.1): no voice, no note held, the decimator empty, the wheels and
    /// pedals home, no free-running LFO in use (its phase follows the history of its rate,
    /// which a new unit has not lived), and, with glide on every note, no last pitch to
    /// glide from. The control grid counts samples, so skipping it is exact.
    pub fn at_rest(&self) -> bool {
        let s = &*self.s;
        let free_lfo =
            (0..8).any(|l| s.lfo[l].mode == LfoMode::Free && s.used_sources[source::LFO + l]);
        !self.voices.iter().any(|v| v.active)
            && self.alloc.is_clear()
            && self.decim.peak() == 0.0
            && self.globals[..source::MACRO].iter().all(|v| *v == 0.0)
            && self.globals[source::MW..].iter().all(|v| *v == 0.0)
            && !free_lfo
            && (s.glide_s <= 0.0 || !s.glide_always || self.last_pitch.is_none())
    }

    /// Samples skipped while at rest (ADR 0031 §5.1): the control grid's clock moves on.
    pub fn skip(&mut self, frames: usize) {
        self.clock += frames as u64;
    }

    /// The free-running LFOs, one tick of `n` samples.
    fn tick_free(&mut self, n: usize) {
        for l in 0..8 {
            let ls = &self.s.lfo[l];
            if ls.mode != LfoMode::Free || !self.s.used_sources[source::LFO + l] {
                continue;
            }
            let rate = self.s.base[target::LFO_RATE + l];
            let f = &mut self.free[l];
            f.phase += rate * n as f32 / self.sr;
            while f.phase >= 1.0 {
                f.phase -= 1.0;
                f.cycle += 1;
                f.r0 = f.r1;
                f.r1 = 2.0 * unit(&[self.s.seed, 7, l as u64, f.cycle + 1]) - 1.0;
            }
            let x = (f.phase + ls.phase).fract();
            self.free_val[l] = modsrc::wave(ls.wave, x, f.r0, f.r1, &ls.shape);
        }
    }

    fn control_all(&mut self) {
        self.tick_free(CONTROL);
        let s = &*self.s;
        let g = Globals {
            sr: self.sr,
            oversample: s.oversample,
            src: &self.globals,
            free_lfo: &self.free_val,
        };
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.control(s, &g, false);
        }
    }

    /// Render one segment, added into `left`/`right`. Returns whether anything was written.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        let n = left.len().min(right.len());
        let q = self.s.oversample.clamp(1, 4);
        let mut wrote = false;
        let mut i = 0;
        self.peak = [self.peak[0] * 0.9, self.peak[1] * 0.9];
        while i < n {
            let on_grid = self.clock.is_multiple_of(CONTROL as u64);
            if on_grid {
                self.control_all();
            }
            let len = (CONTROL - (self.clock % CONTROL as u64) as usize).min(n - i);
            let any = self.voices.iter().any(|v| v.active);
            if !any && on_grid && self.decim.peak() < GATE {
                self.decim.clear();
            }
            if any || self.decim.peak() > 0.0 {
                self.acc_l[..len * q].iter_mut().for_each(|x| *x = 0.0);
                self.acc_r[..len * q].iter_mut().for_each(|x| *x = 0.0);
                let s = &*self.s;
                for (vi, v) in self.voices.iter_mut().enumerate() {
                    if v.active
                        && !v.render(
                            s,
                            q,
                            len,
                            &mut self.acc_l[..len * q],
                            &mut self.acc_r[..len * q],
                        )
                    {
                        self.alloc.voice_finished(vi);
                    }
                }
                for j in 0..len {
                    let (l, r) = self
                        .decim
                        .tick(q, &self.acc_l[j * q..], &self.acc_r[j * q..]);
                    if l != 0.0 || r != 0.0 {
                        wrote = true;
                        left[i + j] += l;
                        right[i + j] += r;
                        self.peak[0] = self.peak[0].max(l.abs());
                        self.peak[1] = self.peak[1].max(r.abs());
                    }
                }
            }
            self.clock += len as u64;
            i += len;
        }
        wrote
    }

    /// What the editor draws.
    pub fn meter(&self) -> Meter {
        let mut m = Meter {
            voices: self.active_voices(),
            stolen: self.stolen,
            peak: self.peak,
            ..Meter::default()
        };
        let newest = self
            .voices
            .iter()
            .filter(|v| v.active)
            .max_by_key(|v| v.age);
        m.lanes = m.voices
            * (0..3)
                .filter(|o| self.s.osc[*o].on)
                .map(|o| self.s.osc[o].unison)
                .sum::<usize>();
        if let Some(v) = newest {
            for o in 0..3 {
                m.pos[o] = v.val[target::OSC + o * target::OSC_N + target::POS];
            }
            for e in 0..4 {
                m.env[e] = v.env[e].level();
            }
            for l in 0..8 {
                m.lfo[l] = v.src[source::LFO + l];
            }
            for f in 0..2 {
                m.cutoff[f] = v.val[target::FLT + f * target::FLT_N + target::CUTOFF];
                m.res[f] = v.val[target::FLT + f * target::FLT_N + target::RES];
            }
        }
        m
    }
}

#[path = "unda_par.rs"]
mod par;
