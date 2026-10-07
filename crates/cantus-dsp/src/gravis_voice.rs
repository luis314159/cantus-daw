//! Gravis's voice building blocks: the AHDSR, the glide, pitch in hertz and the random
//! numbers seeded by the note. Everything steps once per sample, so nothing here depends
//! on where a block starts.

/// −140 dB: below this an envelope or a tail is exact zero.
use crate::math;

pub const SILENCE: f32 = 1e-7;

/// ln(1000): a segment that falls 60 dB in `n` samples multiplies by `exp(-LN_1000 / n)`.
const LN_1000: f64 = 6.907_755_278_982_137;

/// The per-sample factor of an exponential that falls 60 dB in `ms`.
pub fn coef_60db(ms: f32, sr: f32) -> f32 {
    let n = (ms as f64 * 0.001 * sr as f64).max(1.0);
    math::exp(-LN_1000 / n) as f32
}

/// Hertz of a (fractional) MIDI note against a tuning reference for A4.
#[inline]
pub fn hz(note: f64, a4: f64) -> f64 {
    a4 * math::exp2((note - 69.0) / 12.0)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Stage {
    #[default]
    Idle,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
}

/// Times of an envelope, in milliseconds (decay and release: the time to fall 60 dB).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EnvTimes {
    pub attack_ms: f32,
    pub hold_ms: f32,
    pub decay_ms: f32,
    pub sustain: f32,
    pub release_ms: f32,
}

impl Default for EnvTimes {
    fn default() -> Self {
        EnvTimes {
            attack_ms: 1.0,
            hold_ms: 0.0,
            decay_ms: 300.0,
            sustain: 1.0,
            release_ms: 100.0,
        }
    }
}

/// AHDSR: a linear attack from wherever the value is, a hold, an exponential decay to the
/// sustain level and an exponential release. Reaches exact zero.
#[derive(Copy, Clone, Debug, Default)]
pub struct Env {
    stage: Stage,
    value: f32,
    attack_step: f32,
    hold_n: u32,
    hold_left: u32,
    decay_c: f32,
    sustain: f32,
    release_c: f32,
}

impl Env {
    pub fn set(&mut self, t: &EnvTimes, sr: f32) {
        let a = (t.attack_ms * 0.001 * sr).max(0.0);
        self.attack_step = if a < 1.0 { 1.0 } else { 1.0 / a };
        self.hold_n = (t.hold_ms.max(0.0) * 0.001 * sr) as u32;
        self.decay_c = coef_60db(t.decay_ms, sr);
        self.sustain = t.sustain.clamp(0.0, 1.0);
        self.release_c = coef_60db(t.release_ms, sr);
    }

    /// Start (or restart) the attack from the present value.
    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
        self.hold_left = self.hold_n;
    }

    pub fn gate_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    /// The legato window: a note arrived just after the release began, so the envelope
    /// goes back to its decay and sustain without a new attack.
    pub fn resume(&mut self) {
        if self.stage == Stage::Release || self.stage == Stage::Idle {
            self.stage = Stage::Decay;
        }
    }

    pub fn reset(&mut self) {
        self.stage = Stage::Idle;
        self.value = 0.0;
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn is_active(&self) -> bool {
        self.stage != Stage::Idle
    }

    pub fn is_releasing(&self) -> bool {
        self.stage == Stage::Release
    }

    /// Whether it holds exact zero from now on until the next gate (idle, or sustaining at
    /// zero).
    pub fn is_silent(&self) -> bool {
        self.value == 0.0 && matches!(self.stage, Stage::Idle | Stage::Sustain | Stage::Release)
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.value += self.attack_step;
                if self.value >= 1.0 {
                    self.value = 1.0;
                    self.stage = if self.hold_left > 0 {
                        Stage::Hold
                    } else {
                        Stage::Decay
                    };
                }
            }
            Stage::Hold => {
                self.hold_left = self.hold_left.saturating_sub(1);
                if self.hold_left == 0 {
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.value = self.sustain + (self.value - self.sustain) * self.decay_c;
                if (self.value - self.sustain).abs() < 1e-6 {
                    self.value = self.sustain;
                    self.stage = Stage::Sustain;
                }
                if self.value < SILENCE {
                    self.value = 0.0;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {
                // A lowered sustain level is approached, never jumped to.
                if self.value != self.sustain {
                    self.value = self.sustain + (self.value - self.sustain) * self.decay_c;
                    if (self.value - self.sustain).abs() < 1e-6 {
                        self.value = self.sustain;
                    }
                }
            }
            Stage::Release => {
                self.value *= self.release_c;
                if self.value < SILENCE {
                    self.value = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.value
    }
}

/// How a glide moves between two pitches.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum GlideShape {
    /// An RC lag on the pitch (the 303 and most analog monos): 99 % of the way at the glide
    /// time.
    #[default]
    Exponential,
    /// Straight in semitones.
    Linear,
    /// Smoothstep: eases out and in.
    S,
}

impl GlideShape {
    pub fn from_index(i: f32) -> Self {
        match i.round() as i32 {
            1 => GlideShape::Linear,
            2 => GlideShape::S,
            _ => GlideShape::Exponential,
        }
    }
}

/// ln(100): the exponential reaches 99 % of the interval at the glide time.
const LN_100: f64 = 4.605_170_185_988_092;

/// A pitch in semitones that jumps or glides, one step per sample.
#[derive(Copy, Clone, Debug, Default)]
pub struct Glide {
    cur: f64,
    from: f64,
    to: f64,
    pos: f64,
    step: f64,
    alpha: f64,
    shape: GlideShape,
    moving: bool,
}

impl Glide {
    pub fn jump(&mut self, st: f64) {
        self.cur = st;
        self.from = st;
        self.to = st;
        self.moving = false;
    }

    /// Glide from where it is now to `to` over `samples`.
    pub fn start(&mut self, to: f64, samples: f64, shape: GlideShape) {
        if samples < 1.0 || (to - self.cur).abs() < 1e-9 {
            self.jump(to);
            return;
        }
        self.from = self.cur;
        self.to = to;
        self.pos = 0.0;
        self.step = 1.0 / samples;
        self.alpha = 1.0 - math::exp(-LN_100 / samples);
        self.shape = shape;
        self.moving = true;
    }

    #[inline]
    pub fn next(&mut self) -> f64 {
        if !self.moving {
            return self.cur;
        }
        match self.shape {
            GlideShape::Exponential => {
                self.cur += (self.to - self.cur) * self.alpha;
                if (self.to - self.cur).abs() < 1e-5 {
                    self.cur = self.to;
                    self.moving = false;
                }
            }
            GlideShape::Linear | GlideShape::S => {
                self.pos += self.step;
                if self.pos >= 1.0 {
                    self.cur = self.to;
                    self.moving = false;
                } else {
                    let x = if self.shape == GlideShape::S {
                        self.pos * self.pos * (3.0 - 2.0 * self.pos)
                    } else {
                        self.pos
                    };
                    self.cur = self.from + (self.to - self.from) * x;
                }
            }
        }
        self.cur
    }

    pub fn value(&self) -> f64 {
        self.cur
    }

    pub fn target(&self) -> f64 {
        self.to
    }

    pub fn moving(&self) -> bool {
        self.moving
    }
}

/// SplitMix64: a well-mixed seed from a note's id and key, so a note's randomness depends
/// on the note alone — never on a global counter or on what other notes did.
pub fn seed(id: u64, key: u8, salt: u64) -> u64 {
    let mut z = id
        .wrapping_add((key as u64) << 56)
        .wrapping_add(salt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    let z = z ^ (z >> 31);
    if z == 0 {
        0x2545_F491_4F6C_DD1D
    } else {
        z
    }
}

/// Xorshift64*: white noise in −1…1 from a seeded state.
#[derive(Copy, Clone, Debug)]
pub struct Rng(pub u64);

impl Rng {
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in −1…1.
    #[inline]
    pub fn bipolar(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

impl Default for Rng {
    fn default() -> Self {
        Rng(0x2545_F491_4F6C_DD1D)
    }
}

/// A one-pole smoother stepping per sample, snapped to its target when within `1e-6` of it
/// so a settled control is exactly its value.
#[derive(Copy, Clone, Debug, Default)]
pub struct Smooth {
    pub value: f32,
    pub target: f32,
    c: f32,
}

impl Smooth {
    pub fn new(ms: f32, sr: f32) -> Self {
        let n = (ms * 0.001 * sr).max(1.0);
        Smooth {
            value: 0.0,
            target: 0.0,
            c: math::exp(-1.0 / n),
        }
    }

    pub fn snap(&mut self, v: f32) {
        self.value = v;
        self.target = v;
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        if self.value != self.target {
            self.value = self.target + (self.value - self.target) * self.c;
            if (self.value - self.target).abs() <= 1e-6 * self.target.abs().max(1.0) {
                self.value = self.target;
            }
        }
        self.value
    }

    pub fn settled(&self) -> bool {
        self.value == self.target
    }
}
