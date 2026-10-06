//! The instrument: a pool of voices behind the shared allocator, the events that reach it,
//! and the loop that ticks the control grid and renders.

use super::guard::{graph, Graph};
use super::tables::{tables, Tables};
use super::voice::{Ctx, Start, Voice};
use super::{source, DxEnvSettings, Patch, TimeEnvSettings, CONTROL, MAX_VOICES, OPS};
use crate::math;
use crate::voices::{Action, Allocator};
use std::sync::Arc;

/// Telemetry slots Modulus fills (the frame owns 0…7).
pub mod tel {
    /// The widest Carson bandwidth among the sounding voices, Hz.
    pub const BANDWIDTH: usize = 8;
    /// The newest voice's oversampling factor.
    pub const FACTOR: usize = 9;
    /// The smallest index scale the guard is applying (1 = none).
    pub const TAME: usize = 10;
    pub const KEY: usize = 11;
    pub const VELOCITY: usize = 12;
    /// Seconds since the newest voice began.
    pub const ELAPSED: usize = 13;
    pub const RELEASED: usize = 14;
    /// The host rate.
    pub const RATE: usize = 15;
    /// The newest voice's operator amplitudes A…H.
    pub const AMP: usize = 16;
    /// … their frequencies, Hz.
    pub const HZ: usize = 24;
    /// … their envelope stages.
    pub const STAGE: usize = 32;
}

/// Bake the easy envelope macros (−1…1: attack, decay, sustain, release) into a
/// breakpoint envelope: times ×8 or ÷8 at the ends, the sustain level up to double or
/// down to nothing. What the synth plays, and what "Bake" writes.
pub fn baked_time(s: &TimeEnvSettings, m: [f32; 4]) -> TimeEnvSettings {
    let mut e = *s;
    if m == [0.0; 4] {
        return e;
    }
    for k in 0..e.n {
        let x = if k == 0 {
            m[0]
        } else if e.sus > 0 && k >= e.sus {
            m[3]
        } else {
            m[1]
        };
        e.t[k] *= math::exp2(3.0 * x);
    }
    if e.sus > 0 {
        let i = e.sus - 1;
        e.l[i] = (e.l[i] * (1.0 + m[2])).clamp(0.0, 1.0);
    }
    e
}

/// The same for a DX envelope: rates move by up to 30, the third level by up to 30.
pub fn baked_dx(d: &DxEnvSettings, m: [f32; 4]) -> DxEnvSettings {
    let mut e = *d;
    let shift = |v: u8, by: f32| (v as f32 + by).round().clamp(0.0, 99.0) as u8;
    e.rates[0] = shift(e.rates[0], -30.0 * m[0]);
    e.rates[1] = shift(e.rates[1], -30.0 * m[1]);
    e.rates[2] = shift(e.rates[2], -30.0 * m[1]);
    e.rates[3] = shift(e.rates[3], -30.0 * m[3]);
    e.levels[2] = shift(e.levels[2], 30.0 * m[2]);
    e
}

#[derive(Debug)]
pub struct Modulus {
    fs: f32,
    tables: Arc<Tables>,
    /// What plays: the patch with the envelope macros baked in.
    patch: Box<Patch>,
    graph: Graph,
    voices: Vec<Voice>,
    ticked: Vec<u64>,
    alloc: Allocator,
    unison: usize,
    globals: [f32; source::COUNT],
    /// Pitch bend as the controller sent it (±2 st full scale).
    bend_raw: f32,
    clock: u64,
    last_pitch: Option<f32>,
    released: [usize; MAX_VOICES],
    stolen: u32,
}

impl Modulus {
    /// Allocates: never on the audio thread.
    pub fn new(sample_rate: f32) -> Self {
        let p = Patch::default();
        let mut m = Modulus {
            fs: sample_rate,
            tables: tables(),
            graph: graph(&p),
            patch: Box::new(p),
            voices: vec![Voice::default(); MAX_VOICES],
            ticked: vec![0; MAX_VOICES],
            alloc: Allocator::new(16),
            unison: 1,
            globals: [0.0; source::COUNT],
            bend_raw: 0.0,
            clock: 0,
            last_pitch: None,
            released: [0; MAX_VOICES],
            stolen: 0,
        };
        m.set_patch(&Patch::default());
        m
    }

    pub fn sample_rate(&self) -> f32 {
        self.fs
    }

    /// Take a new patch. No allocation: the patch is copied into the one already held.
    pub fn set_patch(&mut self, p: &Patch) {
        self.patch.as_mut().clone_from(p);
        let m = p.env_macro;
        for o in &mut self.patch.ops {
            o.time = baked_time(&o.time, m);
            o.dxenv = baked_dx(&o.dxenv, m);
        }
        self.graph = graph(&self.patch);
        let unison = p.unison.clamp(1, 8);
        if unison != self.unison {
            // The voice-to-note mapping changes: start clean.
            self.reset();
            self.unison = unison;
        }
        self.alloc
            .set_polyphony(p.voices.clamp(1, MAX_VOICES / unison));
        self.alloc.set_mode(p.mode, p.priority);
        for (i, v) in p.macros.iter().enumerate() {
            self.globals[source::MACRO + i] = *v;
        }
    }

    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    fn bend(&self) -> f32 {
        self.bend_raw / 2.0 * self.patch.pb_range
    }

    fn start_voice(&mut self, voice: usize, key: u8, id: u64, velocity: f32) {
        let u = self.unison;
        let from = if self.patch.glide > 0.0 {
            self.last_pitch
        } else {
            None
        };
        let span = CONTROL - (self.clock % CONTROL as u64) as usize;
        let bend = self.bend();
        for copy in 0..u {
            let v = voice * u + copy;
            let stolen = self.voices[v].active;
            let c = Ctx {
                p: &self.patch,
                g: &self.graph,
                t: &self.tables,
                fs: self.fs,
                clock: self.clock,
                globals: &self.globals,
                bend,
            };
            let st = Start {
                key,
                id,
                velocity,
                copy,
                copies: u,
                stolen,
                from,
            };
            self.voices[v].start(st, &c);
            self.voices[v].tick(span, &c);
            self.ticked[v] = self.clock + span as u64;
        }
    }

    fn act(&mut self, a: Action, relvel: f32) {
        let u = self.unison;
        match a {
            Action::Start {
                voice,
                key,
                id,
                velocity,
                ..
            } => {
                self.start_voice(voice, key, id, velocity);
                self.last_pitch = Some(key as f32);
            }
            Action::Glide { voice, key, id } => {
                for v in voice * u..voice * u + u {
                    self.voices[v].glide_to(key, id, &self.patch);
                }
                self.last_pitch = Some(key as f32);
            }
            Action::Release { voice } => {
                for v in voice * u..voice * u + u {
                    self.voices[v].release(relvel, &self.patch);
                }
            }
            Action::Nothing => {}
        }
    }

    pub fn note_on(&mut self, key: u8, id: u64, velocity: f32) {
        let a = self.alloc.note_on(key, id, velocity);
        self.stolen += self.alloc.take_stolen();
        self.act(a, 0.0);
    }

    pub fn note_off(&mut self, key: u8, id: u64, velocity: f32) {
        let a = self.alloc.note_off(key, id);
        self.act(a, velocity);
    }

    /// A per-note expression: 0 tuning (st), 1 pressure, 2 slide, 3 pan, 4 volume.
    pub fn expression(&mut self, id: u64, which: usize, value: f32) {
        for v in self.voices.iter_mut().filter(|v| v.active && v.id == id) {
            v.expression(which, value);
        }
    }

    /// Pitch bend in semitones at a ±2 range (the processor's scale).
    pub fn set_bend(&mut self, semitones: f32) {
        self.bend_raw = semitones;
        self.globals[source::PB] = (semitones / 2.0).clamp(-1.0, 1.0);
    }

    /// A global source's value (wheel, pressure, expression).
    pub fn set_source(&mut self, index: usize, value: f32) {
        if let Some(s) = self.globals.get_mut(index) {
            *s = value;
        }
    }

    pub fn set_sustain(&mut self, down: bool) {
        self.globals[source::SUS] = if down { 1.0 } else { 0.0 };
        let mut released = self.released;
        let n = self.alloc.set_sustain(down, &mut released);
        for &voice in &released[..n] {
            self.act(Action::Release { voice }, 0.0);
        }
    }

    /// Release everything (tails ring); the allocator forgets its notes, so a voice still
    /// ringing is simply taken over by the next note that needs it.
    pub fn all_notes_off(&mut self) {
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.release(0.0, &self.patch);
        }
        self.alloc.clear();
        self.globals[source::SUS] = 0.0;
    }

    pub fn reset(&mut self) {
        for v in &mut self.voices {
            v.active = false;
        }
        self.alloc.clear();
        self.last_pitch = None;
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Voices stolen since the last reset.
    pub fn stolen(&self) -> u32 {
        self.stolen
    }

    pub fn polyphony(&self) -> usize {
        self.alloc.polyphony()
    }

    /// Render into `left`/`right` (added). `false` when nothing sounded (the buffers are
    /// untouched then).
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        let len = left.len().min(right.len());
        if !self.voices.iter().any(|v| v.active) {
            self.clock += len as u64;
            return false;
        }
        let bend = self.bend();
        let mut i = 0;
        while i < len {
            let pos = (self.clock % CONTROL as u64) as usize;
            let run = (len - i).min(CONTROL - pos);
            let c = Ctx {
                p: &self.patch,
                g: &self.graph,
                t: &self.tables,
                fs: self.fs,
                clock: self.clock,
                globals: &self.globals,
                bend,
            };
            for (n, v) in self.voices.iter_mut().enumerate() {
                if !v.active {
                    continue;
                }
                if pos == 0 && self.ticked[n] <= self.clock {
                    self.ticked[n] = self.clock + CONTROL as u64;
                    if !v.tick(CONTROL, &c) {
                        continue;
                    }
                }
                v.render(&c, &mut left[i..i + run], &mut right[i..i + run]);
            }
            i += run;
            self.clock += run as u64;
        }
        // Tell the allocator about notes whose every copy has ended.
        let u = self.unison;
        for voice in 0..MAX_VOICES / u {
            let any = (voice * u..voice * u + u).any(|v| self.voices[v].active);
            if !any && !self.alloc.is_held(voice) {
                self.alloc.voice_finished(voice);
            }
        }
        true
    }

    /// Fill Modulus's own telemetry slots (from 8).
    pub fn telemetry(&self, out: &mut [f32]) {
        if out.len() < 40 {
            return;
        }
        out[tel::RATE] = self.fs;
        out[tel::TAME] = 1.0;
        let mut newest: Option<&Voice> = None;
        for v in self.voices.iter().filter(|v| v.active) {
            out[tel::BANDWIDTH] = out[tel::BANDWIDTH].max(v.bandwidth);
            out[tel::TAME] = out[tel::TAME].min(v.tame);
            if newest.is_none_or(|n| v.started >= n.started) {
                newest = Some(v);
            }
        }
        let Some(v) = newest else {
            return;
        };
        out[tel::FACTOR] = v.factor as f32;
        out[tel::KEY] = v.key as f32;
        out[tel::VELOCITY] = v.vel;
        out[tel::ELAPSED] = v.elapsed;
        out[tel::RELEASED] = if v.released { 1.0 } else { 0.0 };
        for o in 0..OPS {
            let op = &v.ops[o];
            out[tel::AMP + o] = if self.graph.needed[o] { op.amp } else { 0.0 };
            out[tel::HZ + o] = op.hz;
            out[tel::STAGE + o] = if self.patch.ops[o].dx {
                op.dx.stage as f32
            } else {
                op.time.stage as f32
            };
        }
    }
}
