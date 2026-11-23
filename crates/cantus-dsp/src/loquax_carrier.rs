//! Loquax's built-in carrier: a polyphonic synthesiser played by the notes the engine
//! routes to the unit (and the robot's drone), made to be vocoded — bright and dense.
//!
//! Each voice is a blend of a saw, a pulse (two saws a width apart, so its DC is zero) and
//! white noise, with up to seven unison copies detuned and spread across the stereo field,
//! and an attack/release envelope. The saw and the pulse are band-limited by the two-sample
//! polynomial BLEP (Välimäki and Huovilainen's "polyBLEP" residual, a published textbook
//! method): the vocoder's bands would make any alias audible as a whistle.
//!
//! **Deterministic**: each unison copy starts at a phase hashed from its key and index;
//! the noise is a hash of the voice's key and its own sample count; voice stealing takes
//! the oldest voice by a serial that restarts once every voice is silent. **Rest**: no
//! voice sounds — then the synthesiser is exactly a new one.

use crate::math;
use crate::modfx_clock::{hash64, random_bipolar};

pub const MAX_VOICES: usize = 16;
pub const MAX_UNISON: usize = 7;
/// The drone's slot, after the played voices.
const DRONE: usize = MAX_VOICES;
const SEED: u64 = 0x10C0_A7C5_2026;
/// An envelope released under this ends its voice (−80 dB).
const END: f32 = 1e-4;

/// The synthesiser's settings.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SynthParams {
    /// The blend: saw, pulse and noise levels (0 … 1 each).
    pub saw: f32,
    pub pulse: f32,
    /// The pulse's width (0.05 … 0.5).
    pub width: f32,
    pub noise: f32,
    /// Copies per voice (1 … 7).
    pub unison: usize,
    /// The outermost copies' detune, cents.
    pub detune: f32,
    /// How far the copies spread across the stereo field (0 … 1).
    pub spread: f32,
    /// Octaves added to every note (−3 … 3).
    pub octave: i32,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// The synthesiser's level, dB.
    pub level_db: f32,
}

impl Default for SynthParams {
    /// The formant-friendly default: a saw with a breath of noise, three copies 12 cents
    /// apart and half spread — every harmonic present, so every band has something to
    /// shape.
    fn default() -> Self {
        SynthParams {
            saw: 1.0,
            pulse: 0.0,
            width: 0.3,
            noise: 0.04,
            unison: 3,
            detune: 12.0,
            spread: 0.5,
            octave: 0,
            attack_ms: 5.0,
            release_ms: 180.0,
            level_db: -6.0,
        }
    }
}

/// A note for the carrier, at a frame of the block being processed.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NoteEvent {
    /// The frame within the block it lands on.
    pub frame: u32,
    pub kind: NoteKind,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum NoteKind {
    /// A key pressed, velocity 0 … 1 (0 is a release, as MIDI has it).
    On {
        key: u8,
        velocity: f32,
    },
    Off {
        key: u8,
    },
    /// Every key released (the engine's panic, a transport stop).
    AllOff,
}

#[inline]
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        2.0 * t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

/// A band-limited saw at phase `t` (0 … 1) moving `dt` per sample.
#[inline]
fn saw(t: f32, dt: f32) -> f32 {
    2.0 * t - 1.0 - blep(t, dt)
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Voice {
    on: bool,
    key: u8,
    vel: f32,
    /// The note, semitones (MIDI numbering, before the octave).
    note: f32,
    env: f32,
    released: bool,
    phase: [f32; MAX_UNISON],
    dt: [f32; MAX_UNISON],
    pan: [(f32, f32); MAX_UNISON],
    noise_n: i64,
    serial: u32,
}

/// The polyphonic carrier.
#[derive(Clone, Debug)]
pub struct Synth {
    sr: f32,
    p: SynthParams,
    voices: [Voice; MAX_VOICES + 1],
    /// The velocities of the keys held (0: up), whether or not the voices run.
    held: [f32; 128],
    /// r42:tuning — the strip's tuning (ADR 0155): each key's carrier at its pitch.
    pub tuning: [f32; 128],
    serial: u32,
    ka: f32,
    kr: f32,
    gain: f32,
}

impl Synth {
    pub fn new(sr: f32) -> Synth {
        let mut s = Synth {
            sr,
            p: SynthParams::default(),
            voices: [Voice::default(); MAX_VOICES + 1],
            held: [0.0; 128],
            tuning: [0.0; 128], // r42:tuning
            serial: 0,
            ka: 1.0,
            kr: 1.0,
            gain: 1.0,
        };
        s.set(&SynthParams::default());
        s
    }

    pub fn set(&mut self, p: &SynthParams) {
        let mut p = *p;
        p.unison = p.unison.clamp(1, MAX_UNISON);
        p.width = p.width.clamp(0.05, 0.5);
        p.octave = p.octave.clamp(-3, 3);
        self.ka = crate::dynfx_follow::coef(p.attack_ms.clamp(0.0, 5_000.0), self.sr);
        self.kr = crate::dynfx_follow::coef(p.release_ms.clamp(1.0, 10_000.0), self.sr);
        self.gain = math::db_to_gain(p.level_db.clamp(-48.0, 12.0));
        let changed = p.unison != self.p.unison
            || p.detune != self.p.detune
            || p.spread != self.p.spread
            || p.octave != self.p.octave;
        self.p = p;
        if changed {
            for i in 0..=MAX_VOICES {
                if self.voices[i].on {
                    let note = self.voices[i].note;
                    self.tune(i, note);
                }
            }
        }
    }

    /// Voice `i`'s copies' increments and pans for `note`.
    fn tune(&mut self, i: usize, note: f32) {
        let p = self.p;
        let n = p.unison;
        let v = &mut self.voices[i];
        v.note = note;
        let base = note + 12.0 * p.octave as f32 - 69.0;
        for u in 0..MAX_UNISON {
            let pos = if n > 1 {
                2.0 * u as f32 / (n - 1) as f32 - 1.0
            } else {
                0.0
            };
            let semis = base + pos * p.detune.clamp(0.0, 100.0) / 100.0;
            let hz = 440.0 * math::exp2(semis / 12.0);
            v.dt[u] = (hz / self.sr).clamp(1e-6, 0.45);
            let pan = (pos * p.spread.clamp(0.0, 1.0)).clamp(-1.0, 1.0);
            let norm = 1.0 / (n as f32).sqrt();
            v.pan[u] = (
                (0.5 * (1.0 - pan)).sqrt() * norm,
                (0.5 * (1.0 + pan)).sqrt() * norm,
            );
        }
    }

    fn start(&mut self, i: usize, key: u8, note: f32, vel: f32) {
        self.serial = self.serial.wrapping_add(1);
        let fresh = !self.voices[i].on;
        let v = &mut self.voices[i];
        if fresh {
            *v = Voice::default();
            for u in 0..MAX_UNISON {
                let h = hash64(SEED ^ (u64::from(key) << 8) ^ u as u64);
                v.phase[u] = (h >> 40) as f32 / (1u64 << 24) as f32;
            }
        }
        v.on = true;
        v.key = key;
        v.vel = vel.clamp(0.0, 1.0);
        v.released = false;
        v.serial = self.serial;
        self.tune(i, note);
    }

    /// A note event. `running`: the voices sound (the unit is engaged); either way the
    /// held keys are kept, so engaging starts the chord already held.
    pub fn note(&mut self, kind: NoteKind, running: bool) {
        match kind {
            NoteKind::On { key, velocity } if velocity > 0.0 => {
                let key = key.min(127);
                self.held[key as usize] = velocity.min(1.0);
                if running {
                    self.press(key, velocity);
                }
            }
            NoteKind::On { key, .. } | NoteKind::Off { key } => {
                let key = key.min(127);
                self.held[key as usize] = 0.0;
                for v in self.voices[..MAX_VOICES].iter_mut() {
                    if v.on && v.key == key {
                        v.released = true;
                    }
                }
            }
            NoteKind::AllOff => {
                self.held = [0.0; 128];
                for v in self.voices[..MAX_VOICES].iter_mut() {
                    v.released = true;
                }
            }
        }
    }

    fn press(&mut self, key: u8, vel: f32) {
        let played = &self.voices[..MAX_VOICES];
        let slot = played
            .iter()
            .position(|v| v.on && v.key == key)
            .or_else(|| played.iter().position(|v| !v.on))
            .unwrap_or_else(|| {
                // The oldest voice, released ones first.
                (0..MAX_VOICES)
                    .min_by_key(|&i| (!played[i].released, played[i].serial))
                    .unwrap_or(0)
            });
        // r42:tuning — the key's tuned pitch (`+ 0.0` by default).
        let note = f32::from(key) + crate::tuning::at(&self.tuning, key);
        self.start(slot, key, note, vel);
    }

    /// Start a voice for every key held (the unit has just engaged).
    pub fn start_held(&mut self) {
        for k in 0..128u8 {
            let v = self.held[k as usize];
            if v > 0.0 {
                self.press(k, v);
            }
        }
    }

    pub fn any_held(&self) -> bool {
        self.held.iter().any(|v| *v > 0.0)
    }

    /// The robot's drone: on at `note` (MIDI numbering, fractional), or released.
    pub fn drone(&mut self, on: bool, note: f32) {
        let v = self.voices[DRONE];
        if on {
            if !v.on || v.released || v.note != note {
                self.start(DRONE, 255, note, 0.8);
            }
        } else if v.on {
            self.voices[DRONE].released = true;
        }
    }

    /// Every voice silenced at once (a reset, a disengage); the held keys stay.
    pub fn silence(&mut self) {
        self.voices = [Voice::default(); MAX_VOICES + 1];
        self.serial = 0;
    }

    pub fn reset(&mut self) {
        self.silence();
        self.held = [0.0; 128];
    }

    /// Voices sounding (the drone included).
    pub fn sounding(&self) -> usize {
        self.voices.iter().filter(|v| v.on).count()
    }

    /// No voice sounds: exactly a new synthesiser (the held keys apart).
    pub fn at_rest(&self) -> bool {
        self.voices.iter().all(|v| !v.on) && self.serial == 0
    }

    /// One stereo sample.
    #[inline]
    pub fn tick(&mut self) -> (f32, f32) {
        let p = self.p;
        let (mut l, mut r) = (0.0f32, 0.0f32);
        let mut any = false;
        for v in self.voices.iter_mut() {
            if !v.on {
                continue;
            }
            if v.released {
                v.env -= v.env * self.kr;
                if v.env < END {
                    *v = Voice::default();
                    continue;
                }
            } else if v.env != 1.0 {
                let e = v.env + (1.0 - v.env) * self.ka;
                v.env = if (1.0 - e) < 1e-6 { 1.0 } else { e };
            }
            any = true;
            let amp = v.env * (0.35 + 0.65 * v.vel);
            let noise = if p.noise > 0.0 {
                v.noise_n += 1;
                p.noise * random_bipolar(SEED ^ u64::from(v.key), v.noise_n)
            } else {
                0.0
            };
            for u in 0..p.unison {
                let (t, dt) = (v.phase[u], v.dt[u]);
                let mut x = p.saw * saw(t, dt);
                if p.pulse > 0.0 {
                    let mut t2 = t + p.width;
                    if t2 >= 1.0 {
                        t2 -= 1.0;
                    }
                    x += p.pulse * 0.5 * (saw(t, dt) - saw(t2, dt));
                }
                let x = (x + noise) * amp;
                l += x * v.pan[u].0;
                r += x * v.pan[u].1;
                let mut t = t + dt;
                if t >= 1.0 {
                    t -= 1.0;
                }
                v.phase[u] = t;
            }
        }
        if !any {
            self.serial = 0;
        }
        (l * self.gain, r * self.gain)
    }
}
