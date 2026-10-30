//! Notes, controllers and expressions: the allocator's answers turned into voices on
//! cards (a child of `calor::synth`).

use super::super::voice::x;
use super::super::{Assign, CARDS, MAX_VOICES};
use super::{is_live, note_seed, src, Calor};
use crate::va::math::hash3;
use crate::voices::{Action, PlayMode, MAX_VOICES as ALLOC_MAX};

/// A per-note expression, as the contract names them.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Expression {
    /// Semitones from the key.
    Tuning,
    Pressure,
    Brightness,
    /// −1…1.
    Pan,
    /// Linear gain.
    Volume,
    Vibrato,
    /// The contract's `Expression`, 0…1 (a gain on the note).
    Expr,
}

impl Calor {
    pub fn note_on(&mut self, key: u8, velocity: f32, id: u64) {
        let live = is_live(id);
        if live {
            self.live_count += 1;
        }
        let seed = note_seed(id, self.live_count);
        let held_before = (0..self.polyphony()).any(|v| self.alloc.is_held(v));
        match self.alloc.note_on(key, id, velocity.clamp(0.0, 1.0)) {
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
                self.start(voice, key, id, velocity, seed, live, held_before);
            }
            Action::Glide { voice, key, id } => self.glide(voice, key, id),
            Action::Release { voice } => self.release(voice, 0.0),
            Action::Nothing => {}
        }
    }

    pub fn note_off(&mut self, key: u8, id: u64, velocity: f32) {
        match self.alloc.note_off(key, id) {
            Action::Release { voice } => self.release(voice, velocity),
            Action::Glide { voice, key, id } => self.glide(voice, key, id),
            _ => {}
        }
    }

    /// Stop the note at once: a three-millisecond fade, no release tail.
    pub fn choke(&mut self, key: u8, id: u64) {
        self.note_off(key, id, 0.0);
        let sh = self.shared();
        let mut voices = self.voices;
        for v in voices.iter_mut().filter(|v| v.active && v.id == id) {
            v.fade_out(&sh);
        }
        self.voices = voices;
    }

    pub fn expression(&mut self, id: u64, which: Expression, value: f32) {
        let i = match which {
            Expression::Tuning => x::TUNING,
            Expression::Pressure => x::PRESSURE,
            Expression::Brightness => x::BRIGHTNESS,
            Expression::Pan => x::PAN,
            Expression::Volume => x::VOLUME,
            Expression::Vibrato => x::VIBRATO,
            Expression::Expr => x::EXPRESSION,
        };
        let value = if value.is_finite() {
            value
        } else {
            x::DEFAULT[i]
        };
        for v in self.voices.iter_mut().filter(|v| v.active && v.id == id) {
            // Tuning, pressure and brightness glide to it at the control rate (ADR 0080);
            // a note not yet sounding starts there.
            if i < 3 {
                let fresh = v.fresh();
                crate::expr_glide::set(&mut v.x[i], &mut v.x_to[i], value, fresh);
            } else {
                v.x[i] = value;
            }
        }
    }

    /// Pitch bend in semitones as the processor scales it (±2 at full throw): read as a
    /// fraction of the throw and applied with this instrument's own ranges.
    pub fn pitch_bend(&mut self, semitones: f32) {
        let throw = (semitones / 2.0).clamp(-1.0, 1.0);
        self.globals[src::PB] = throw;
        self.bend = throw
            * if throw >= 0.0 {
                self.s.bend_up
            } else {
                self.s.bend_down
            };
    }

    pub fn mod_wheel(&mut self, v: f32) {
        self.wheel = v.clamp(0.0, 1.0);
        self.globals[src::MW] = self.wheel;
    }

    pub fn pressure(&mut self, v: f32) {
        self.globals[src::AT] = v.clamp(0.0, 1.0);
    }

    pub fn expression_pedal(&mut self, v: f32) {
        self.globals[src::EXPR] = v.clamp(0.0, 1.0);
    }

    pub fn sustain(&mut self, v: f32) {
        self.globals[src::SUS] = v.clamp(0.0, 1.0);
        let mut released = [0usize; ALLOC_MAX];
        let n = self.alloc.set_sustain(v >= 0.5, &mut released);
        for &voice in released.iter().take(n) {
            self.release(voice, 0.0);
        }
    }

    /// Release every voice (transport stop): tails ring.
    pub fn all_notes_off(&mut self) {
        for v in self.voices.iter_mut().filter(|v| v.active && !v.released) {
            v.release(0.0);
        }
        self.alloc.clear();
        self.globals[src::SUS] = 0.0;
    }

    /// Silence at once and forget everything (seek, panic).
    pub fn reset(&mut self) {
        for v in self.voices.iter_mut().chain(self.ghosts.iter_mut()) {
            v.active = false;
        }
        self.alloc.clear();
        self.os.reset();
        self.chorus.reset();
        self.out.reset();
        self.last_pitch = [f32::NAN; MAX_VOICES];
        self.alloc_busy = [false; MAX_VOICES];
        self.clock = 0;
        self.rotate = 0;
        self.live_count = 0;
        self.stolen = 0;
        self.tail = 0;
        self.bend = 0.0;
        self.wheel = 0.0;
        let macros = self.globals;
        self.globals = [0.0; src::COUNT];
        self.globals[src::MACRO1..src::MACRO1 + 8]
            .copy_from_slice(&macros[src::MACRO1..src::MACRO1 + 8]);
    }

    /// Whether any voice sounds on card `c`.
    fn card_busy(&self, c: usize) -> bool {
        self.voices.iter().any(|v| v.active && v.card == c)
    }

    /// The first card a note's stack takes.
    fn pick_card(&mut self, alloc_voice: usize, key: u8, id: u64, live: bool) -> usize {
        let stack = self.s.stack;
        match self.s.assign {
            Assign::Reuse => key as usize % CARDS,
            Assign::Lowest => (alloc_voice * stack) % CARDS,
            Assign::Rotate => {
                let start = if live {
                    let r = self.rotate;
                    self.rotate = self.rotate.wrapping_add(stack as u64);
                    r
                } else {
                    hash3(id, 0xcadd, 0)
                } as usize
                    % CARDS;
                (0..CARDS)
                    .map(|k| (start + k) % CARDS)
                    .find(|c| !self.card_busy(*c))
                    .unwrap_or(start)
            }
        }
    }

    /// Move a sounding voice to a ghost that fades it out.
    fn ghost(&mut self, phys: usize) {
        let sh = self.shared();
        let slot = self
            .ghosts
            .iter()
            .position(|g| !g.active)
            .unwrap_or_else(|| {
                (0..self.ghosts.len())
                    .min_by(|a, b| self.ghosts[*a].fade.total_cmp(&self.ghosts[*b].fade))
                    .unwrap_or(0)
            });
        let mut g = self.voices[phys];
        g.fade_out(&sh);
        self.ghosts[slot] = g;
        self.voices[phys].active = false;
    }

    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        alloc_voice: usize,
        key: u8,
        id: u64,
        velocity: f32,
        seed: u64,
        live: bool,
        held_before: bool,
    ) {
        let stack = self.s.stack;
        let mono = self.s.mode != PlayMode::Poly;
        self.alloc_busy[alloc_voice] = true;
        let base_card = self.pick_card(alloc_voice, key, id, live);
        for j in 0..stack {
            let phys = alloc_voice * stack + j;
            if phys >= MAX_VOICES {
                break;
            }
            let pos = if stack > 1 {
                2.0 * j as f32 / (stack - 1) as f32 - 1.0
            } else {
                0.0
            };
            let v = self.voices[phys];
            let glide_ok = !self.s.glide_legato || held_before;
            // A mono voice retriggers from where it is, as hardware does — unless the
            // envelopes are set to restart from silence.
            if mono && v.active && !self.s.env_reset[0] && !self.s.env_reset[1] {
                let sh = self.shared();
                let mut v = v;
                v.move_to(&sh, key, id, Some(velocity), true);
                if !glide_ok {
                    v.snap();
                }
                self.voices[phys] = v;
                self.last_pitch[phys] = key as f32;
                self.newest = phys;
                continue;
            }
            let from = if mono && v.active {
                Some(v.pitch)
            } else {
                Some(self.last_pitch[phys]).filter(|p| p.is_finite())
            }
            .filter(|_| glide_ok);
            if v.active {
                self.ghost(phys);
            }
            let sh = self.shared();
            let mut nv = self.voices[phys];
            nv.start(
                &sh,
                key,
                velocity,
                id,
                crate::va::math::mix64(seed ^ j as u64),
                (base_card + j) % CARDS,
                pos,
                from,
            );
            self.voices[phys] = nv;
            self.last_pitch[phys] = key as f32;
            self.newest = phys;
        }
    }

    /// Legato: the voice moves to the key without a new attack.
    fn glide(&mut self, alloc_voice: usize, key: u8, id: u64) {
        let stack = self.s.stack;
        let sh = self.shared();
        let mut voices = self.voices;
        for j in 0..stack {
            let phys = alloc_voice * stack + j;
            if phys < MAX_VOICES && voices[phys].active {
                voices[phys].move_to(&sh, key, id, None, false);
            }
        }
        self.voices = voices;
        for j in 0..stack {
            if let Some(p) = self.last_pitch.get_mut(alloc_voice * stack + j) {
                *p = key as f32;
            }
        }
    }

    fn release(&mut self, alloc_voice: usize, velocity: f32) {
        let stack = self.s.stack;
        for j in 0..stack {
            if let Some(v) = self.voices.get_mut(alloc_voice * stack + j) {
                if v.active {
                    v.release(velocity);
                }
            }
        }
    }
}
