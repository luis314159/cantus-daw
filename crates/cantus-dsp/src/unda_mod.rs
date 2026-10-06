//! Unda's modulation sources that live in the voice: the DAHDSR envelope with curved
//! segments, the LFO with drawable shapes, and the seeded random numbers everything
//! random draws from. All of it steps per sample or on the control grid, counted from the
//! note's start, so nothing depends on where a block begins.

use crate::math;
use std::f32::consts::TAU;

/// SplitMix64: a counter-based hash, so a random value is a function of what it is for
/// (seed, note, lane, purpose), never of how many were drawn before.
#[inline]
pub fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Uniform in [0, 1) from a hash of `parts`.
#[inline]
pub fn unit(parts: &[u64]) -> f32 {
    let h = parts.iter().fold(0x5EED_u64, |h, p| mix(h ^ p));
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// A segment's shape, `(e^{kt} − 1)/(e^k − 1)`: `0` is a straight line; negative bends it
/// to move fast first (a natural decay), positive to move slowly first. `e^{kt}` is kept
/// by multiplying by `e^{k·dt}` each sample, so a segment costs no `exp` per sample; the
/// segment's end is snapped exactly.
#[derive(Copy, Clone, Debug, Default)]
struct Curve {
    inv_denom: f32,
    /// `e^{kt}` now, and its factor per sample.
    e: f32,
    r: f32,
}

impl Curve {
    fn new(c: f32, t: f32, dt: f32) -> Curve {
        let k = 6.0 * c.clamp(-1.0, 1.0);
        if k.abs() < 1e-4 {
            return Curve::default();
        }
        Curve {
            inv_denom: 1.0 / (math::exp(k) - 1.0),
            e: math::exp(k * t),
            r: math::exp(k * dt),
        }
    }
    /// Step once and return the shape at the new `t`.
    #[inline]
    fn step(&mut self, t: f32) -> f32 {
        if self.inv_denom == 0.0 {
            t
        } else {
            self.e *= self.r;
            (self.e - 1.0) * self.inv_denom
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct EnvSettings {
    pub delay: f32,
    pub attack: f32,
    pub hold: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub a_curve: f32,
    pub d_curve: f32,
    pub r_curve: f32,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Idle,
    Delay,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
}

/// A DAHDSR envelope, 0…1, stepped once a sample and exact at the segment ends: an
/// attack lands on 1, a decay on the sustain, a release on exactly 0.
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub stage: Stage,
    /// Progress through the stage, 0…1.
    t: f32,
    /// Progress per sample.
    dt: f32,
    /// Where the current segment starts (attack and release start from the level they
    /// interrupt).
    from: f32,
    level: f32,
    curve: Curve,
    s: EnvSettings,
    sr: f32,
}

impl Env {
    pub fn level(&self) -> f32 {
        self.level
    }

    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    /// Start (or restart from the current level).
    pub fn trigger(&mut self, s: &EnvSettings, sr: f32) {
        self.s = *s;
        self.sr = sr;
        self.from = self.level;
        self.enter(if s.delay > 0.0 {
            Stage::Delay
        } else {
            Stage::Attack
        });
    }

    pub fn release(&mut self) {
        if !matches!(self.stage, Stage::Idle | Stage::Release) {
            self.from = self.level;
            self.enter(Stage::Release);
        }
    }

    /// Silence at once.
    pub fn kill(&mut self) {
        self.stage = Stage::Idle;
        self.level = 0.0;
    }

    /// New times, sustain and curves (modulated): the progress through the stage is kept.
    pub fn update(&mut self, s: &EnvSettings) {
        if *s != self.s {
            self.s = *s;
            self.shape();
        }
    }

    fn enter(&mut self, stage: Stage) {
        self.stage = stage;
        self.t = 0.0;
        self.shape();
        if stage == Stage::Sustain {
            self.level = self.s.sustain;
        }
    }

    fn shape(&mut self) {
        let len = match self.stage {
            Stage::Delay => self.s.delay,
            Stage::Attack => self.s.attack,
            Stage::Hold => self.s.hold,
            Stage::Decay => self.s.decay,
            Stage::Release => self.s.release,
            _ => 0.0,
        };
        self.dt = 1.0 / (len * self.sr).max(1.0);
        let c = match self.stage {
            Stage::Attack => self.s.a_curve,
            Stage::Decay => self.s.d_curve,
            Stage::Release => self.s.r_curve,
            _ => 0.0,
        };
        self.curve = Curve::new(c, self.t, self.dt);
    }

    /// One sample.
    #[inline]
    pub fn next(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => return 0.0,
            Stage::Sustain => {
                self.level = self.s.sustain;
                return self.level;
            }
            _ => {}
        }
        self.t += self.dt;
        let done = self.t >= 1.0;
        let t = self.t.min(1.0);
        match self.stage {
            Stage::Delay => {
                if done {
                    self.enter(Stage::Attack);
                }
            }
            Stage::Attack => {
                self.level = self.from + (1.0 - self.from) * self.curve.step(t);
                if done {
                    self.level = 1.0;
                    self.enter(if self.s.hold > 0.0 {
                        Stage::Hold
                    } else {
                        Stage::Decay
                    });
                }
            }
            Stage::Hold => {
                if done {
                    self.enter(Stage::Decay);
                }
            }
            Stage::Decay => {
                self.level = 1.0 + (self.s.sustain - 1.0) * self.curve.step(t);
                if done {
                    self.enter(Stage::Sustain);
                }
            }
            Stage::Release => {
                self.level = self.from * (1.0 - self.curve.step(t));
                if done {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
            _ => {}
        }
        self.level
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum LfoWave {
    #[default]
    Sine,
    Triangle,
    SawUp,
    SawDown,
    Square,
    SampleHold,
    SmoothRandom,
    Drawn,
}

impl LfoWave {
    pub fn from_index(i: f32) -> LfoWave {
        use LfoWave::*;
        [
            Sine,
            Triangle,
            SawUp,
            SawDown,
            Square,
            SampleHold,
            SmoothRandom,
            Drawn,
        ][(i.round().max(0.0) as usize).min(7)]
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum LfoMode {
    /// Restarts at its phase with every note.
    #[default]
    Retrig,
    /// Runs on for the whole instrument, whatever the notes.
    Free,
    /// Runs one cycle from the note and holds its end.
    Envelope,
}

impl LfoMode {
    pub fn from_index(i: f32) -> LfoMode {
        match i.round() as i32 {
            1 => LfoMode::Free,
            2 => LfoMode::Envelope,
            _ => LfoMode::Retrig,
        }
    }
}

/// A drawn shape: points (x, y) in 0…1, straight lines between, wrapping.
#[derive(Copy, Clone, Debug)]
pub struct Shape {
    pub points: [(f32, f32); 16],
    pub len: usize,
}

impl Default for Shape {
    /// A triangle: what "Drawn" plays before anything is drawn.
    fn default() -> Self {
        let mut points = [(0.0, 0.0); 16];
        points[0] = (0.0, 0.5);
        points[1] = (0.25, 1.0);
        points[2] = (0.75, 0.0);
        points[3] = (1.0, 0.5);
        Shape { points, len: 4 }
    }
}

impl Shape {
    /// −1…1 at phase `x` in [0, 1).
    pub fn at(&self, x: f32) -> f32 {
        let p = &self.points[..self.len.max(1)];
        if p.len() == 1 {
            return 2.0 * p[0].1 - 1.0;
        }
        let i = p.iter().position(|q| q.0 > x).unwrap_or(p.len());
        let (a, b) = if i == 0 {
            let last = p[p.len() - 1];
            ((last.0 - 1.0, last.1), p[0])
        } else if i == p.len() {
            (p[i - 1], (p[0].0 + 1.0, p[0].1))
        } else {
            (p[i - 1], p[i])
        };
        let w = b.0 - a.0;
        let t = if w > 1e-6 {
            ((x - a.0) / w).clamp(0.0, 1.0)
        } else {
            1.0
        };
        2.0 * (a.1 + (b.1 - a.1) * t) - 1.0
    }
}

/// A classic shape's value, −1…1, at phase `x`; `r0`/`r1` are this cycle's and the
/// next cycle's random values (for the random shapes).
#[inline]
pub fn wave(w: LfoWave, x: f32, r0: f32, r1: f32, shape: &Shape) -> f32 {
    match w {
        LfoWave::Sine => math::sin(TAU * x),
        LfoWave::Triangle => 1.0 - 4.0 * ((x + 0.25).fract() - 0.5).abs(),
        LfoWave::SawUp => 2.0 * x - 1.0,
        LfoWave::SawDown => 1.0 - 2.0 * x,
        LfoWave::Square => {
            if x < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        LfoWave::SampleHold => r0,
        LfoWave::SmoothRandom => {
            let s = 0.5 - 0.5 * math::cos(std::f32::consts::PI * x);
            r0 + (r1 - r0) * s
        }
        LfoWave::Drawn => shape.at(x),
    }
}
