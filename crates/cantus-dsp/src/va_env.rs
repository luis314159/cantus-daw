//! Envelopes with a model: an RC charge (the analog default), ADS (decay doubles as
//! release), Digital (straight lines), One-shot (attack then decay, the key ignored).
//!
//! The RC model charges a capacitor towards a target *past* the peak (the overshoot that
//! makes an analog attack convex and snappy), then discharges exponentially towards the
//! sustain level and towards zero, each target set a little beyond the level it stops at,
//! so every stage ends in finite time — and the release lands on exact zero.
//!
//! **Retriggering never jumps**: a new gate starts the attack from the level the envelope
//! is at, as hardware does. (A "reset" is the voice's business: it fades the old sound out
//! over a few milliseconds and starts a fresh one.)

use super::math::exp;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum EnvModel {
    #[default]
    Rc,
    Ads,
    Digital,
    OneShot,
}

impl EnvModel {
    pub fn from_index(i: f32) -> EnvModel {
        match i.round() as i32 {
            1 => EnvModel::Ads,
            2 => EnvModel::Digital,
            3 => EnvModel::OneShot,
            _ => EnvModel::Rc,
        }
    }
}

/// An envelope's settings, in seconds and a level.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EnvSettings {
    pub attack: f32,
    pub decay: f32,
    /// 0…1.
    pub sustain: f32,
    pub release: f32,
    pub model: EnvModel,
}

impl Default for EnvSettings {
    fn default() -> Self {
        EnvSettings {
            attack: 0.005,
            decay: 0.3,
            sustain: 0.7,
            release: 0.3,
            model: EnvModel::Rc,
        }
    }
}

/// How far past the peak the attack aims: the curve's convexity.
const ATTACK_OVERSHOOT: f32 = 0.3;
/// How far past its end a discharge aims (−60 dB of the step, so "time" is time to
/// −60 dB).
const DISCHARGE_OVERSHOOT: f32 = 0.001;
/// Shortest stage, seconds (a shorter one clicks).
pub const MIN_TIME: f32 = 0.0005;

/// Per-sample coefficients for one rate and one time scale.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct EnvCoefs {
    model: EnvModel,
    /// RC: the fraction of the way to the target each sample; Digital: a step.
    a: f32,
    d: f32,
    r: f32,
    sustain: f32,
    /// Digital release: samples it lasts (the step depends on where it starts).
    r_samples: f32,
}

/// `ln((1 + o) / o)` for the two overshoots: how many time constants a stage lasts.
const LN_ATTACK: f32 = 1.466_337_1; // ln(1.3 / 0.3)
const LN_DISCHARGE: f32 = 6.908_755; // ln(1.001 / 0.001)

fn rc_coef(time: f32, sample_rate: f32, time_constants: f32) -> f32 {
    let n = time.max(MIN_TIME) * sample_rate;
    1.0 - exp(-time_constants / n)
}

impl EnvCoefs {
    /// `time_scale` multiplies every time (a card's calibration, key scaling).
    pub fn new(s: &EnvSettings, sample_rate: f32, time_scale: f32) -> Self {
        let ts = time_scale.max(0.01);
        let (a, d, r) = (s.attack * ts, s.decay * ts, s.release * ts);
        let sustain = match s.model {
            EnvModel::OneShot => 0.0,
            _ => s.sustain.clamp(0.0, 1.0),
        };
        let release = match s.model {
            EnvModel::Ads => d,
            EnvModel::OneShot => d,
            _ => r,
        };
        match s.model {
            EnvModel::Digital => EnvCoefs {
                model: s.model,
                a: 1.0 / (a.max(MIN_TIME) * sample_rate),
                d: (1.0 - sustain) / (d.max(MIN_TIME) * sample_rate),
                r: 0.0,
                sustain,
                r_samples: release.max(MIN_TIME) * sample_rate,
            },
            _ => EnvCoefs {
                model: s.model,
                a: rc_coef(a, sample_rate, LN_ATTACK),
                d: rc_coef(d, sample_rate, LN_DISCHARGE),
                r: rc_coef(release, sample_rate, LN_DISCHARGE),
                sustain,
                r_samples: 0.0,
            },
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum EnvStage {
    #[default]
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct VaEnv {
    pub stage: EnvStage,
    pub level: f32,
    /// Digital release: the step, fixed when the key is let go.
    step: f32,
}

impl VaEnv {
    /// A new gate: the attack, from the level it is at.
    pub fn gate_on(&mut self) {
        self.stage = EnvStage::Attack;
    }

    /// The key let go (a one-shot ignores it).
    pub fn gate_off(&mut self, c: &EnvCoefs) {
        if c.model == EnvModel::OneShot || self.stage == EnvStage::Idle {
            return;
        }
        self.stage = EnvStage::Release;
        self.step = self.level / c.r_samples.max(1.0);
    }

    pub fn reset(&mut self) {
        *self = VaEnv::default();
    }

    pub fn is_idle(&self) -> bool {
        self.stage == EnvStage::Idle
    }

    #[inline]
    pub fn tick(&mut self, c: &EnvCoefs) -> f32 {
        let digital = c.model == EnvModel::Digital;
        match self.stage {
            EnvStage::Idle => return 0.0,
            EnvStage::Attack => {
                if digital {
                    self.level += c.a;
                } else {
                    self.level += (1.0 + ATTACK_OVERSHOOT - self.level) * c.a;
                }
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = EnvStage::Decay;
                }
            }
            EnvStage::Decay => {
                let s = c.sustain;
                if digital {
                    self.level -= c.d;
                } else {
                    let target = s - DISCHARGE_OVERSHOOT * (1.0 - s);
                    self.level += (target - self.level) * c.d;
                }
                if self.level <= s {
                    self.level = s;
                    self.stage = if s <= 0.0 {
                        self.level = 0.0;
                        EnvStage::Idle
                    } else {
                        EnvStage::Sustain
                    };
                }
            }
            EnvStage::Sustain => {
                // Follow a sustain level that moves (a knob, modulation) smoothly.
                self.level += (c.sustain - self.level) * c.d.max(1e-3);
                if c.sustain <= 0.0 && self.level < 1e-6 {
                    self.level = 0.0;
                    self.stage = EnvStage::Idle;
                }
            }
            EnvStage::Release => {
                if digital {
                    self.level -= self.step;
                } else {
                    self.level += (-DISCHARGE_OVERSHOOT - self.level) * c.r;
                }
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = EnvStage::Idle;
                }
            }
        }
        self.level
    }
}
