//! One sounding zone — a *layer* — and what shapes it: the DAHDSR envelope, the state-variable
//! filter, the two LFOs. A note starts one layer per zone it matches (velocity layers
//! crossfading, microphones, a release sample later), all tied to the note's voice slot.
//!
//! Everything advances per sample or on a control grid of [`CONTROL`] samples counted from
//! the layer's own start, never from a block boundary, so a render is the same at every
//! block size (ADR 0023 rule 1).

use super::play::Playhead;
use crate::math;

/// Samples between control updates (pitch, filter, modulation).
pub const CONTROL: u32 = 32;
/// Level below which a released layer is over (−140 dB): the tail is gated to exact zero.
pub const FLOOR: f32 = 1e-7;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Stage {
    #[default]
    Off,
    Delay,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
}

/// Envelope times in samples and the sustain level (0…1).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EnvShape {
    pub delay: u32,
    pub attack: f32,
    pub hold: u32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl Default for EnvShape {
    fn default() -> Self {
        EnvShape {
            delay: 0,
            attack: 0.0,
            hold: 0,
            decay: 48_000.0,
            sustain: 1.0,
            release: 12_000.0,
        }
    }
}

/// A delay–attack–hold–decay–sustain–release envelope. Attack is linear; decay and release
/// fall exponentially, each time being the time to fall 60 dB of the way.
#[derive(Copy, Clone, Debug, Default)]
pub struct Env {
    pub stage: Stage,
    pub level: f32,
    left: u32,
    hold_after: u32,
    inc: f32,
    sustain: f32,
    decay_coef: f32,
    release_coef: f32,
}

/// The coefficient that falls 60 dB in `samples`.
fn fall(samples: f32) -> f32 {
    if samples <= 1.0 {
        0.0
    } else {
        math::exp(-6.907_755 / samples)
    }
}

impl Env {
    pub fn start(&mut self, s: &EnvShape) {
        self.sustain = s.sustain.clamp(0.0, 1.0);
        self.decay_coef = fall(s.decay);
        self.release_coef = fall(s.release);
        self.level = 0.0;
        self.inc = if s.attack > 4.8 { 1.0 / s.attack } else { 1.0 };
        self.stage = Stage::Delay;
        self.left = s.delay;
        self.hold_after = s.hold;
        if self.left == 0 {
            self.enter_attack();
        }
    }

    fn enter_attack(&mut self) {
        self.stage = Stage::Attack;
        if self.inc >= 1.0 {
            self.level = 1.0;
            self.enter_hold();
        }
    }

    fn enter_hold(&mut self) {
        self.stage = Stage::Hold;
        self.left = self.hold_after;
        if self.left == 0 {
            self.stage = Stage::Decay;
        }
    }

    /// New release time while releasing (a changed knob); the rest keeps its course.
    pub fn set_release(&mut self, samples: f32) {
        self.release_coef = fall(samples);
    }

    pub fn release(&mut self) {
        if self.stage != Stage::Off {
            self.stage = Stage::Release;
        }
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        match self.stage {
            Stage::Off => return 0.0,
            Stage::Delay => {
                self.left -= 1;
                if self.left == 0 {
                    self.enter_attack();
                }
                return self.level;
            }
            Stage::Attack => {
                self.level += self.inc;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.enter_hold();
                }
            }
            Stage::Hold => {
                self.left -= 1;
                if self.left == 0 {
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level = self.sustain + (self.level - self.sustain) * self.decay_coef;
                if (self.level - self.sustain).abs() < 1e-6 {
                    self.level = self.sustain;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {}
            Stage::Release => {
                self.level *= self.release_coef;
                if self.level < FLOOR {
                    self.level = 0.0;
                    self.stage = Stage::Off;
                }
            }
        }
        self.level
    }
}

/// A state-variable filter (topology-preserving transform), 12 dB per octave.
#[derive(Copy, Clone, Debug, Default)]
pub struct Svf {
    ic1: [f32; 2],
    ic2: [f32; 2],
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
    /// 1 low-pass, 2 band-pass, 3 high-pass.
    pub mode: u8,
}

impl Svf {
    pub fn reset(&mut self) {
        self.ic1 = [0.0; 2];
        self.ic2 = [0.0; 2];
    }

    /// `cutoff` in Hz, `reso` 0…1.
    pub fn set(&mut self, cutoff: f32, reso: f32, fs: f32) {
        let fc = cutoff.clamp(10.0, 0.45 * fs);
        let g = math::tan(std::f32::consts::PI * fc / fs);
        self.k = 2.0 - 1.96 * reso.clamp(0.0, 1.0);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    #[inline]
    pub fn process(&mut self, x: f32, c: usize) -> f32 {
        let v3 = x - self.ic2[c];
        let v1 = self.a1 * self.ic1[c] + self.a2 * v3;
        let v2 = self.ic2[c] + self.a2 * self.ic1[c] + self.a3 * v3;
        self.ic1[c] = 2.0 * v1 - self.ic1[c];
        self.ic2[c] = 2.0 * v2 - self.ic2[c];
        // Flush what has decayed below hearing so an idle filter holds exact zeros.
        if self.ic1[c].abs() < 1e-20 {
            self.ic1[c] = 0.0;
        }
        if self.ic2[c].abs() < 1e-20 {
            self.ic2[c] = 0.0;
        }
        match self.mode {
            2 => v1,
            3 => x - self.k * v1 - v2,
            _ => v2,
        }
    }
}

/// An LFO's value (−1…1) at `phase` (cycles) for a shape: sine, triangle, square, saw.
#[inline]
pub fn lfo_value(shape: u8, phase: f64) -> f32 {
    let p = phase.fract() as f32;
    match shape {
        1 => 1.0 - 4.0 * (p - 0.5).abs(),
        2 => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        3 => 2.0 * p - 1.0,
        _ => math::sin(std::f32::consts::TAU * p),
    }
}

/// Voice slots and layers carry these; kept for the editor and the tests.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct LayerInfo {
    pub zone: u16,
    pub group: u8,
    pub position: f64,
    pub level: f32,
}

/// The per-layer reader and shapers; the engine drives them.
#[derive(Clone, Debug, Default)]
pub struct Layer {
    pub active: bool,
    /// The allocator's voice slot this layer belongs to.
    pub slot: usize,
    pub id: u64,
    pub key: u8,
    pub zone: u16,
    pub group: u8,
    pub offby: u8,
    pub off_normal: bool,
    /// Plays to its end whatever the note does.
    pub one_shot: bool,
    /// Held keys have not let it go yet.
    pub released: bool,
    pub play: Playhead,
    /// Pitch in semitones before modulation: key offset, tunes, transpose.
    pub semis: f64,
    /// Source rate over output rate.
    pub rate_ratio: f64,
    pub ratio: f64,
    /// Glide: semitones still to travel, and per control step.
    pub glide: f64,
    pub glide_step: f64,
    /// Loops only while held.
    pub sustain_loop: bool,
    /// Static level: zone, group, instrument, velocity, crossfade, release decay.
    pub amp: f32,
    /// −1 … 1 before modulation.
    pub pan: f32,
    /// `amp` × pan law × modulated pan, per side; refreshed each control step.
    pub gain: [f32; 2],
    /// No control step has run yet (the first one sets levels without a ramp).
    pub first: bool,
    /// Modulated gain, ramped per sample towards `gain_to` across a control step.
    pub gmod: f32,
    pub gmod_to: f32,
    pub gstep: f32,
    pub env: Env,
    pub env2: Env,
    pub filter: Svf,
    pub lfo: [f64; 2],
    /// Samples until the next control update.
    pub ctl: u32,
    /// Samples since the layer started.
    pub age: u64,
    /// Start order, for stealing the oldest.
    pub serial: u64,
    /// Steal/choke fade: current gain and its per-sample step (0 when not fading).
    pub fade: f32,
    pub fade_step: f32,
    pub vel: f32,
    pub relvel: f32,
    pub rnd: f32,
}

impl Layer {
    /// Fade out over `samples` (a steal or a choke), then stop.
    pub fn fade_out(&mut self, samples: f32) {
        if self.fade_step == 0.0 {
            self.fade = 1.0;
        }
        self.fade_step = -(1.0 / samples.max(1.0));
    }

    pub fn info(&self) -> LayerInfo {
        LayerInfo {
            zone: self.zone,
            group: self.group,
            position: self.play.file_position(),
            level: self.env.level * self.fade.max(if self.fade_step == 0.0 { 1.0 } else { 0.0 }),
        }
    }
}
