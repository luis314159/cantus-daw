//! Lamina, the modelled electric piano: its DSP. Owned by this instrument's builder
//! (ADR 0023).
//!
//! ```text
//!  key ─ hammer pulse ─ tine + tone bar (5 modes) ─┐            (or reed, 3 modes)
//!                        ... one voice per key ...  ├ pickup each (flux or capacitance, dΦ/dt)
//!                                                   ┴ Σ ─ tone (bass, treble) ─ preamp (ADAA) ─ speaker? ─ tremolo (pan or amplitude) ─ gain ─ L/R
//! ```
//!
//! - **Two engines** ([`geometry::Engine`]): a tine with its tone bar and an
//!   electromagnetic pickup (Dens), a reed with a tip mass and an electrostatic pickup
//!   (Lingua). Geometry in millimetres ([`geometry`]); the voice in [`voice`].
//! - **Voices** come from the shared allocator (`crate::voices`): poly with stealing (a
//!   stolen voice fades out over 3 ms before its new strike). A key struck again while it
//!   still rings is struck *on the moving tine*: the new voice takes the old one's state.
//! - **The sustain pedal is continuous**: the allocator keeps the bookkeeping (a released
//!   key is held while the pedal is past its lift point), and every released voice's damper
//!   sits at `engagement(pedal)` — fully on the tine below the lift point's zone, fully off
//!   above it, and partly on in between: half-pedalling, where the modes decay at a rate
//!   between free and damped.
//! - **Keyboard curves**: hardness, decay and level each have eight anchors (C1…C8),
//!   interpolated per key.
//!
//! Deterministic (the per-key spread and age come from the key; the noises are seeded by
//! the note's id), block-size independent (everything steps per sample; the tremolo's phase
//! is counted from the start of the stream), exactly silent when the notes are over.

// Parallel per-wheel / per-mode arrays read clearest by index (as in `spatium_*`).
#![allow(clippy::needless_range_loop)]

#[path = "lamina_geometry.rs"]
pub mod geometry;
#[path = "lamina_voice.rs"]
pub mod voice;

#[cfg(test)]
#[path = "lamina_tests.rs"]
mod tests;

use crate::organum::filters::{Bq, Clipper, Glide};
use crate::voices::{Action, Allocator, MAX_VOICES};
use geometry::Engine;
use voice::{HammerShape, Strike, Voice};

/// The keyboard.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Range {
    /// E1–E7, the stage piano's 73.
    #[default]
    Keys73,
    /// A1–C7, the reed piano's 64.
    Keys64,
    /// A0–C8.
    Keys88,
}

impl Range {
    pub fn from_index(i: f32) -> Range {
        match i.round() as i32 {
            1 => Range::Keys64,
            2 => Range::Keys88,
            _ => Range::Keys73,
        }
    }

    /// Lowest and highest MIDI key.
    pub fn keys(self) -> (u8, u8) {
        match self {
            Range::Keys73 => (28, 100),
            Range::Keys64 => (33, 96),
            Range::Keys88 => (21, 108),
        }
    }
}

/// The tremolo's kind.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Trem {
    /// The engine's own: pan for the tine (the suitcase's), amplitude for the reed.
    #[default]
    Auto,
    Pan,
    Amplitude,
}

impl Trem {
    pub fn from_index(i: f32) -> Trem {
        match i.round() as i32 {
            1 => Trem::Pan,
            2 => Trem::Amplitude,
            _ => Trem::Auto,
        }
    }
}

/// Keys of the keyboard curves' anchors: C1, C2 … C8.
pub const CURVE_KEYS: [u8; 8] = [24, 36, 48, 60, 72, 84, 96, 108];

/// A keyboard curve's value at `key`, linear between anchors.
pub fn curve_at(anchors: &[f32; 8], key: u8) -> f32 {
    let k = key as f32;
    if k <= CURVE_KEYS[0] as f32 {
        return anchors[0];
    }
    for i in 0..7 {
        let (a, b) = (CURVE_KEYS[i] as f32, CURVE_KEYS[i + 1] as f32);
        if k <= b {
            let f = (k - a) / (b - a);
            return anchors[i] + (anchors[i + 1] - anchors[i]) * f;
        }
    }
    anchors[7]
}

/// Everything Lamina is set to, in real units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LaminaSettings {
    pub engine: Engine,
    pub range: Range,
    pub a4_hz: f32,
    /// Per-key detune, seeded from the key, cents (±).
    pub spread_ct: f32,
    /// Hammer tip hardness 0…1, and how much velocity adds to it.
    pub hardness: f32,
    pub hardness_vel: f32,
    pub force_db: f32,
    /// Strike point, fraction of the length from the clamp.
    pub strike: f32,
    pub hammer_noise_db: f32,
    /// Fundamental decay to −60 dB at C4, s.
    pub tine_decay_s: f32,
    pub couple: f32,
    pub bell_db: f32,
    pub bell_decay_s: f32,
    pub tine_gap_mm: f32,
    pub tine_align_mm: f32,
    pub reed_decay_s: f32,
    pub overtones_db: f32,
    pub reed_gap_mm: f32,
    pub reed_align_mm: f32,
    pub pickup_db: f32,
    pub damper_ms: f32,
    /// 0: every mode damps alike; 1: the upper ones much faster.
    pub damper_tone: f32,
    pub damper_noise_db: f32,
    pub release_noise_db: f32,
    /// Where on its travel (0…1) the pedal lifts the dampers.
    pub pedal_lift: f32,
    pub drive: f32,
    pub bass_db: f32,
    pub treble_db: f32,
    pub speaker: bool,
    pub trem: Trem,
    pub trem_rate_hz: f32,
    pub trem_depth: f32,
    /// Level range across velocity, dB.
    pub dynamics_db: f32,
    /// −1…1: negative harder to play loud, positive easier.
    pub vel_curve: f32,
    pub polyphony: usize,
    /// 0…1: more spread, noise and misalignment, key by key.
    pub age: f32,
    /// Keyboard curves: hardness offset (±), decay multiplier (×), level (dB).
    pub curve_hard: [f32; 8],
    pub curve_decay: [f32; 8],
    pub curve_level: [f32; 8],
    pub gain_db: f32,
}

impl Default for LaminaSettings {
    fn default() -> Self {
        LaminaSettings {
            engine: Engine::Tine,
            range: Range::Keys73,
            a4_hz: 440.0,
            spread_ct: 2.0,
            hardness: 0.5,
            hardness_vel: 0.6,
            force_db: 0.0,
            strike: 0.08,
            hammer_noise_db: -24.0,
            tine_decay_s: 8.0,
            couple: 0.5,
            bell_db: -12.0,
            bell_decay_s: 0.25,
            tine_gap_mm: 1.6,
            tine_align_mm: 0.6,
            reed_decay_s: 3.0,
            overtones_db: -10.0,
            reed_gap_mm: 1.0,
            reed_align_mm: 0.6,
            pickup_db: 0.0,
            damper_ms: 120.0,
            damper_tone: 0.5,
            damper_noise_db: -30.0,
            release_noise_db: -36.0,
            pedal_lift: 0.35,
            drive: 0.2,
            bass_db: 0.0,
            treble_db: 0.0,
            speaker: false,
            trem: Trem::Auto,
            trem_rate_hz: 5.5,
            trem_depth: 0.0,
            dynamics_db: 40.0,
            vel_curve: 0.0,
            polyphony: 32,
            age: 0.2,
            curve_hard: [0.0; 8],
            curve_decay: [1.0; 8],
            curve_level: [0.0; 8],
            gain_db: 0.0,
        }
    }
}

/// How far the dampers rest on the tines for a pedal position (0 up … 1 down): on below
/// the lift point's zone, off above it, a smooth partial contact across the 20 % of
/// travel around it.
pub fn engagement(pedal: f32, lift: f32) -> f32 {
    let x = ((pedal - (lift - 0.1)) / 0.2).clamp(0.0, 1.0);
    1.0 - x * x * (3.0 - 2.0 * x)
}

/// The velocity (0…1) after the curve.
pub fn shaped_velocity(v: f32, curve: f32) -> f32 {
    v.clamp(0.0, 1.0)
        .powf(2f32.powf(-1.5 * curve.clamp(-1.0, 1.0)))
}

/// A number in −1…1 from a key and a salt, the same every time.
fn key_hash(key: u8, salt: u64) -> f32 {
    let mut z = (key as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

/// What the editor draws.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct LaminaMeter {
    pub voices: usize,
    pub stolen: u32,
    /// The last struck key and its velocity.
    pub key: u8,
    pub velocity: f32,
    /// Its tip now, and its swing over the last block, mm.
    pub x: f32,
    pub swing_lo: f32,
    pub swing_hi: f32,
    pub pedal: f32,
    pub engagement: f32,
    /// The tremolo's position, −1…1.
    pub trem: f32,
    pub peak_l: f32,
    pub peak_r: f32,
}

/// Mono sum to the preamp: a C4 at full velocity reaches the preamp's knee.
const LEVEL: f32 = 0.25;
/// After the preamp: its ceiling well under full scale.
const OUT: f32 = 0.7;
const SILENCE: f32 = 1e-7;

#[derive(Debug)]
pub struct Lamina {
    sr: f32,
    s: LaminaSettings,
    configured: bool,
    shape: Box<HammerShape>,
    voices: Box<[Voice]>,
    alloc: Allocator,
    pedal: f32,
    bend: f32,
    stolen: u32,
    last: usize,
    released: [usize; MAX_VOICES],
    bass: Bq,
    treble: Bq,
    clip: Clipper,
    spk: [Bq; 3],
    drive: Glide,
    post: Glide,
    out: Glide,
    expr: Glide,
    depth: Glide,
    trem_phase: u32,
    trem_inc: u32,
    quiet: u32,
    sleep_after: u32,
    asleep: bool,
    meter: LaminaMeter,
}

impl Lamina {
    /// Allocates: build it off the audio thread.
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let glide = |v: f32| Glide::new(v, ((0.01 * sr) as u32).max(1));
        let mut l = Lamina {
            sr,
            s: LaminaSettings::default(),
            configured: false,
            shape: Box::new(HammerShape::new()),
            voices: (0..MAX_VOICES).map(|_| Voice::new(sr)).collect(),
            alloc: Allocator::new(32),
            pedal: 0.0,
            bend: 1.0,
            stolen: 0,
            last: 0,
            released: [0; MAX_VOICES],
            bass: Bq::identity(),
            treble: Bq::identity(),
            clip: Clipper::new(0.15),
            spk: [Bq::identity(); 3],
            drive: glide(1.0),
            post: glide(1.0),
            out: glide(1.0),
            expr: glide(1.0),
            depth: glide(0.0),
            trem_phase: 0,
            trem_inc: 0,
            quiet: 0,
            sleep_after: ((0.1 * sr) as u32).max(1),
            asleep: true,
            meter: LaminaMeter::default(),
        };
        l.spk[0].set_highpass(sr, 110.0, 0.7);
        l.spk[1].set_peak(sr, 1_500.0, 4.0, 1.0);
        l.spk[2].set_lowpass(sr, 5_500.0, 0.7);
        let s = LaminaSettings::default();
        l.set(&s);
        l.configured = false;
        l
    }

    pub fn settings(&self) -> &LaminaSettings {
        &self.s
    }

    pub fn set(&mut self, s: &LaminaSettings) {
        let first = !self.configured;
        self.s = *s;
        let sr = self.sr;
        self.alloc.set_polyphony(s.polyphony.clamp(1, MAX_VOICES));
        self.bass.set_low_shelf(sr, 150.0, s.bass_db);
        self.treble.set_high_shelf(sr, 3_000.0, s.treble_db);
        let g = 10f32.powf(1.5 * s.drive.clamp(0.0, 1.0));
        self.drive.set(g);
        self.post.set(OUT / (self.clip.slope() * g.sqrt()));
        self.out.set(10f32.powf(s.gain_db / 20.0));
        self.depth.set(s.trem_depth.clamp(0.0, 1.0));
        self.trem_inc =
            (s.trem_rate_hz.max(0.0) as f64 / sr as f64 * 4_294_967_296.0).round() as u32;
        // The pedal's damping follows a new lift point at once.
        let e = engagement(self.pedal, s.pedal_lift);
        for v in self.voices.iter_mut().filter(|v| v.active && !v.key_down) {
            v.set_damper(e, 0.5);
        }
        if first {
            for gl in [
                &mut self.drive,
                &mut self.post,
                &mut self.out,
                &mut self.depth,
            ] {
                let t = gl.target();
                gl.snap(t);
            }
            self.configured = true;
        }
    }

    /// The strike a key gets under settings `s` (per-note modulation passes its own).
    pub fn strike_for(&self, s: &LaminaSettings, key: u8, id: u64, velocity: f32) -> Strike {
        let (lo, hi) = s.range.keys();
        let mut k = key;
        while k < lo {
            k += 12;
        }
        while k > hi {
            k -= 12;
        }
        let age = s.age.clamp(0.0, 1.0);
        let spread = s.spread_ct * (1.0 + age) * key_hash(k, 1);
        let hz = s.a4_hz * 2f32.powf((k as f32 - 69.0 + spread / 100.0) / 12.0) * self.bend;
        let v = shaped_velocity(velocity, s.vel_curve);
        let hardness = (s.hardness + s.hardness_vel * (v - 0.5) * 0.6 + curve_at(&s.curve_hard, k))
            .clamp(0.0, 1.0);
        let key_decay = (261.63 / hz).powf(0.5).clamp(0.2, 4.0);
        let age_decay = 1.0 - 0.25 * age * (0.5 + 0.5 * key_hash(k, 2));
        let (decay, bell, gap, align) = match s.engine {
            Engine::Tine => (s.tine_decay_s, s.bell_db, s.tine_gap_mm, s.tine_align_mm),
            Engine::Reed => (
                s.reed_decay_s,
                s.overtones_db,
                s.reed_gap_mm,
                s.reed_align_mm,
            ),
        };
        let misalign = 0.3 * age * key_hash(k, 3);
        Strike {
            key: k,
            id,
            engine: s.engine,
            hz,
            velocity: v,
            hardness,
            dynamics_db: s.dynamics_db,
            strike: s.strike,
            decay_s: decay * curve_at(&s.curve_decay, k) * key_decay * age_decay,
            couple: s.couple,
            bell_db: bell,
            bell_decay_s: s.bell_decay_s,
            gap_mm: gap,
            align_mm: align + misalign,
            level_db: s.force_db + s.pickup_db + curve_at(&s.curve_level, k),
            hammer_noise_db: s.hammer_noise_db + 6.0 * age,
            damper_ms: s.damper_ms,
            damper_tone: s.damper_tone,
            release_noise_db: s.release_noise_db,
            damper_noise_db: s.damper_noise_db,
            mode_detune: [
                0.01 * (0.3 + age) * key_hash(k, 4),
                0.015 * (0.3 + age) * key_hash(k, 5),
                0.02 * (0.3 + age) * key_hash(k, 6),
            ],
        }
    }

    /// A key goes down. `per_note`: the settings this note is struck with, when modulation
    /// moves them for it (velocity, key); otherwise the instrument's.
    pub fn note_on(&mut self, key: u8, id: u64, velocity: f32, per_note: Option<&LaminaSettings>) {
        let s = *per_note.unwrap_or(&self.s);
        let strike = self.strike_for(&s, key, id, velocity);
        self.asleep = false;
        self.quiet = 0;
        if let Action::Start { voice, stolen, .. } = self.alloc.note_on(key, id, velocity) {
            self.last = voice;
            self.meter.key = strike.key;
            self.meter.velocity = strike.velocity;
            if stolen {
                self.stolen += 1;
                self.voices[voice].start(&strike, &self.shape, true);
                return;
            }
            // Struck again while it rings: the hammer meets the moving tine.
            let ringing =
                self.voices.iter().enumerate().position(|(i, v)| {
                    i != voice && v.active && v.key == strike.key && !v.key_down
                });
            if let Some(u) = ringing {
                let old = std::mem::take(&mut self.voices[u]);
                self.voices[u] = Voice::new(self.sr);
                self.alloc.voice_finished(u);
                self.voices[voice].take_state(&old);
                self.voices[voice].restrike(&strike, &self.shape);
            } else {
                self.voices[voice].start(&strike, &self.shape, false);
            }
        }
    }

    pub fn note_off(&mut self, key: u8, id: u64, release_velocity: f32) {
        let e = engagement(self.pedal, self.s.pedal_lift);
        if let Some(v) = self
            .voices
            .iter_mut()
            .find(|v| v.active && v.key_down && v.id == id)
        {
            v.key_up(e, release_velocity);
        }
        let _ = self.alloc.note_off(key, id);
    }

    /// The sustain pedal, 0 (up) … 1 (down); anything between is half-pedalling.
    pub fn set_pedal(&mut self, pedal: f32) {
        self.pedal = pedal.clamp(0.0, 1.0);
        let e = engagement(self.pedal, self.s.pedal_lift);
        for v in self.voices.iter_mut().filter(|v| v.active && !v.key_down) {
            v.set_damper(e, 0.5);
        }
        let _ = self
            .alloc
            .set_sustain(self.pedal >= self.s.pedal_lift, &mut self.released);
    }

    pub fn pedal(&self) -> f32 {
        self.pedal
    }

    /// Pitch bend, semitones: every ringing tine and every new one.
    pub fn set_bend(&mut self, semitones: f32) {
        let b = 2f32.powf(semitones / 12.0);
        let ratio = b / self.bend;
        self.bend = b;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.retune(ratio);
        }
    }

    /// Expression pedal, 0…1.
    pub fn set_expression(&mut self, e: f32) {
        self.expr.set(e.clamp(0.0, 1.0));
    }

    pub fn all_notes_off(&mut self) {
        let e = engagement(self.pedal, self.s.pedal_lift);
        for v in self.voices.iter_mut().filter(|v| v.active && v.key_down) {
            v.key_up(e, 0.5);
        }
        for i in 0..MAX_VOICES {
            if self.voices[i].active {
                let (k, id) = (self.voices[i].key, self.voices[i].id);
                let _ = self.alloc.note_off(k, id);
            }
        }
    }

    pub fn reset(&mut self) {
        for v in self.voices.iter_mut() {
            v.clear();
        }
        self.alloc.clear();
        self.pedal = 0.0;
        self.bass.reset();
        self.treble.reset();
        self.clip.reset();
        for b in &mut self.spk {
            b.reset();
        }
        self.trem_phase = 0;
        self.asleep = true;
        self.quiet = 0;
        self.meter = LaminaMeter::default();
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    pub fn polyphony(&self) -> usize {
        self.alloc.polyphony()
    }

    pub fn meter(&self) -> LaminaMeter {
        let mut m = self.meter;
        m.voices = self.active_voices();
        m.stolen = self.stolen;
        let v = &self.voices[self.last];
        if v.active {
            m.x = v.x;
            m.swing_lo = v.swing_lo;
            m.swing_hi = v.swing_hi;
        }
        m.pedal = self.pedal;
        m.engagement = engagement(self.pedal, self.s.pedal_lift);
        m
    }

    /// The voice (for tests and the editor).
    pub fn voice(&self, i: usize) -> &Voice {
        &self.voices[i]
    }

    fn idle_tick(&mut self) {
        for g in [
            &mut self.drive,
            &mut self.post,
            &mut self.out,
            &mut self.expr,
            &mut self.depth,
        ] {
            g.next();
        }
    }

    /// Render into `left`/`right` (added). Returns whether anything but zeros was written.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        let n = left.len().min(right.len());
        {
            let v = &mut self.voices[self.last];
            v.swing_lo = v.x;
            v.swing_hi = v.x;
        }
        if self.asleep && self.voices.iter().all(|v| !v.active) {
            self.trem_phase = self
                .trem_phase
                .wrapping_add(self.trem_inc.wrapping_mul(n as u32));
            for _ in 0..n {
                self.idle_tick();
            }
            self.meter.peak_l = 0.0;
            self.meter.peak_r = 0.0;
            return false;
        }
        let mut wrote = false;
        let pan = match self.s.trem {
            Trem::Pan => true,
            Trem::Amplitude => false,
            Trem::Auto => self.s.engine == Engine::Tine,
        };
        for i in 0..n {
            let mut mono = 0.0;
            let mut any = false;
            for (vi, v) in self.voices.iter_mut().enumerate() {
                if v.active {
                    if v.tick(&self.shape, &mut mono) {
                        any = true;
                    } else {
                        self.alloc.voice_finished(vi);
                    }
                }
            }
            let x = mono * LEVEL * self.expr.next();
            let x = self.treble.run(self.bass.run(x));
            let mut y = self.clip.run(x * self.drive.next()) * self.post.next();
            if self.s.speaker {
                for b in &mut self.spk {
                    y = b.run(y);
                }
            }
            self.trem_phase = self.trem_phase.wrapping_add(self.trem_inc);
            let lfo = (self.trem_phase as f32 * (std::f32::consts::TAU / 4_294_967_296.0)).sin();
            let d = self.depth.next();
            let (l, r) = if d == 0.0 {
                (y, y)
            } else if pan {
                let a = std::f32::consts::FRAC_PI_4 * (1.0 + d * lfo);
                (
                    y * a.cos() * std::f32::consts::SQRT_2,
                    y * a.sin() * std::f32::consts::SQRT_2,
                )
            } else {
                let g = 1.0 - d * (0.5 + 0.5 * lfo);
                (y * g, y * g)
            };
            let o = self.out.next();
            let (l, r) = (l * o, r * o);
            self.meter.trem = lfo;
            self.meter.peak_l = (self.meter.peak_l * 0.9999).max(l.abs());
            self.meter.peak_r = (self.meter.peak_r * 0.9999).max(r.abs());
            if !any && l.abs() < SILENCE && r.abs() < SILENCE {
                self.quiet += 1;
                if self.quiet >= self.sleep_after {
                    self.asleep = true;
                    self.quiet = 0;
                    self.bass.reset();
                    self.treble.reset();
                    self.clip.reset();
                    for b in &mut self.spk {
                        b.reset();
                    }
                    continue;
                }
            } else {
                self.quiet = 0;
            }
            if self.asleep {
                continue;
            }
            if l != 0.0 || r != 0.0 {
                left[i] += l;
                right[i] += r;
                wrote = true;
            }
        }
        wrote
    }
}
