//! Modulus's envelopes: the DX7's rate/level envelope driven by the published measurements,
//! a time/level breakpoint envelope with curves, a sustain point and a loop, and the small
//! ones (pitch envelope, modulation ADSR, LFO). All advance by a whole control period at a
//! time, on the sample grid counted from the stream's start.
//!
//! **The DX envelope** (research §4.4, from the MSFA wiki's measurements of the DX7): a level
//! lives in log steps of 6.02/256 dB (about 0.0235 dB); a DX level `L` (0…99) is
//! `64·(so(L) >> 1)` steps, where `so` is the DX output-level scaling (a table below 20,
//! `28 + L` above); full scale is 3840 steps and the floor 16, which is silence. A rate `R`
//! becomes `qrate = R·41/64` (plus keyboard rate scaling), and a falling level moves at
//! `0.2819 · 2^(qrate>>2) · (1 + (qrate&3)/4)` dB/s — linear in dB. A rising level first
//! jumps to 1716 steps (about 40 dB above the floor) and then moves at that speed times
//! `(4352 − level)/256` rounded down, which is the DX's concave attack.

use crate::modulus::{DxEnvSettings, TimeEnvSettings};

/// dB per DX level step.
pub const DX_STEP_DB: f32 = 6.020_6 / 256.0;
/// Full scale and floor, in steps.
pub const DX_FULL: f32 = 3840.0;
pub const DX_FLOOR: f32 = 16.0;
const DX_ATTACK_JUMP: f32 = 1716.0;
const DX_ATTACK_TOP: f32 = 4352.0;

/// The DX7's output-level scaling below 20; `28 + L` from 20 up.
const SO_LOW: [u8; 20] = [
    0, 5, 9, 13, 17, 20, 23, 25, 27, 29, 31, 33, 35, 37, 39, 41, 42, 43, 45, 46,
];

pub fn dx_scale_level(l: u8) -> u32 {
    let l = l.min(99);
    if l < 20 {
        SO_LOW[l as usize] as u32
    } else {
        28 + l as u32
    }
}

/// A DX envelope level (0…99) in steps, before the operator's own level is added.
pub fn dx_env_steps(l: u8) -> f32 {
    (64 * (dx_scale_level(l) >> 1)) as f32 - (64.0 * 63.0 - DX_FULL)
}

/// A DX output level (0…99) as dB relative to 99 — how an imported operator's level is kept.
pub fn dx_output_level_db(ol: u8) -> f32 {
    32.0 * (dx_scale_level(ol) as f32 - 127.0) * DX_STEP_DB
}

/// The speed of a DX rate, in steps per second (falling; rising multiplies it).
pub fn dx_speed(qrate: u32) -> f32 {
    let q = qrate.min(63);
    let db_s = 0.2819 * (1u32 << (q >> 2)) as f32 * (1.0 + 0.25 * (q & 3) as f32);
    db_s / DX_STEP_DB
}

/// The DX's `qrate` for a rate 0…99 and a rate-scaling delta.
pub fn dx_qrate(rate: u8, delta: u32) -> u32 {
    ((rate.min(99) as u32 * 41) >> 6) + delta
}

/// Keyboard rate scaling (0…7): the qrate added at a MIDI note.
pub fn dx_rate_scaling(note: u8, sens: u8) -> u32 {
    let x = (note as i32 / 3 - 7).clamp(0, 31) as u32;
    (sens.min(7) as u32 * x) >> 3
}

/// Amplitude of a level in steps (the floor and below are silence).
#[inline]
pub fn dx_amp(steps: f32) -> f32 {
    if steps <= DX_FLOOR {
        0.0
    } else {
        ((steps - DX_FULL) * (1.0 / 256.0)).exp2()
    }
}

/// A running DX envelope.
#[derive(Copy, Clone, Debug, Default)]
pub struct DxEnv {
    pub level: f32,
    /// 0…2: going to L1…L3; 3: holding L3; 4: releasing to L4; 5: done.
    pub stage: u8,
    /// Added to every target, in steps (the operator's level, key scaling, velocity).
    offset: f32,
    qdelta: u32,
}

impl DxEnv {
    /// Start (or restart from the current level when `retrigger`).
    pub fn start(&mut self, s: &DxEnvSettings, offset: f32, qdelta: u32, retrigger: bool) {
        self.offset = offset;
        self.qdelta = qdelta;
        if !retrigger {
            self.level = self.target(s, 3).max(DX_FLOOR);
        }
        self.stage = 0;
    }

    fn target(&self, s: &DxEnvSettings, i: usize) -> f32 {
        (dx_env_steps(s.levels[i]) + self.offset).max(DX_FLOOR)
    }

    pub fn release(&mut self) {
        if self.stage < 4 {
            self.stage = 4;
        }
    }

    pub fn done(&self) -> bool {
        self.stage >= 5
    }

    /// Advance by `dt` seconds; returns the amplitude at the end.
    pub fn advance(&mut self, s: &DxEnvSettings, dt: f32) -> f32 {
        let mut left = dt;
        while left > 0.0 && self.stage != 3 && self.stage < 5 {
            let i = if self.stage == 4 {
                3
            } else {
                self.stage as usize
            };
            let target = self.target(s, i);
            let speed = dx_speed(dx_qrate(s.rates[i], self.qdelta));
            if (target - self.level).abs() < 1e-3 {
                self.level = target;
                self.next_stage();
                continue;
            }
            if target > self.level {
                if self.level < DX_ATTACK_JUMP {
                    self.level = DX_ATTACK_JUMP.min(target);
                    continue;
                }
                // Constant speed until the next 256-step boundary or the target.
                let factor = ((DX_ATTACK_TOP - self.level) / 256.0 - 1e-4)
                    .floor()
                    .max(1.0);
                let v = speed * factor;
                let boundary = DX_ATTACK_TOP - 256.0 * factor;
                let stop = target.min(if boundary > self.level + 1e-3 {
                    boundary
                } else {
                    target
                });
                let need = (stop - self.level) / v;
                if need <= left {
                    self.level = stop;
                    left -= need;
                } else {
                    self.level += v * left;
                    left = 0.0;
                }
            } else {
                let need = (self.level - target) / speed;
                if need <= left {
                    self.level = target;
                    left -= need;
                } else {
                    self.level -= speed * left;
                    left = 0.0;
                }
            }
        }
        dx_amp(self.level)
    }

    fn next_stage(&mut self) {
        self.stage = match self.stage {
            0 | 1 => self.stage + 1,
            2 => 3,
            _ => 5,
        };
    }
}

/// How a breakpoint segment bends: `x^(2^(−3c))` — 0 is a straight line, positive moves
/// fast first (an exponential-like decay), negative slow first.
#[inline]
pub fn bend(x: f32, c: f32) -> f32 {
    if c == 0.0 {
        x
    } else {
        x.max(0.0).powf((-3.0 * c).exp2())
    }
}

/// A running breakpoint envelope.
#[derive(Copy, Clone, Debug, Default)]
pub struct TimeEnv {
    pub level: f32,
    /// The segment running (0-based: segment *k* goes to point *k + 1*); `n` = done.
    pub stage: usize,
    t: f32,
    from: f32,
    /// Time scale for this note (key and velocity scaling, the easy macros).
    scale_attack: f32,
    scale_decay: f32,
    scale_release: f32,
    released: bool,
}

impl TimeEnv {
    pub fn start(&mut self, s: &TimeEnvSettings, scales: [f32; 3], retrigger: bool) {
        if !retrigger {
            self.level = s.l0;
        }
        self.from = self.level;
        self.stage = 0;
        self.t = 0.0;
        self.released = false;
        [self.scale_attack, self.scale_decay, self.scale_release] = scales;
    }

    pub fn release(&mut self, s: &TimeEnvSettings) {
        if self.released {
            return;
        }
        self.released = true;
        if s.sus > 0 && self.stage < s.n {
            self.stage = s.sus.min(s.n);
            self.from = self.level;
            self.t = 0.0;
        }
    }

    pub fn done(&self, s: &TimeEnvSettings) -> bool {
        self.stage >= s.n
    }

    fn seg_time(&self, s: &TimeEnvSettings, k: usize) -> f32 {
        let scale = if k == 0 {
            self.scale_attack
        } else if s.sus > 0 && k >= s.sus {
            self.scale_release
        } else {
            self.scale_decay
        };
        s.t[k] * scale
    }

    /// Advance by `dt` seconds; returns the level (0…1) at the end.
    pub fn advance(&mut self, s: &TimeEnvSettings, dt: f32) -> f32 {
        let mut left = dt;
        let mut guard = 0;
        while left > 0.0 && self.stage < s.n {
            guard += 1;
            if guard > 64 {
                break; // zero-length loops cannot spin
            }
            if !self.released && s.sus > 0 && self.stage == s.sus {
                if s.lp > 0 && s.lp < s.sus {
                    self.stage = s.lp;
                    self.from = self.level;
                    self.t = 0.0;
                    continue;
                }
                return self.level; // holding
            }
            let k = self.stage;
            let dur = self.seg_time(s, k);
            let to = s.l[k];
            if dur <= 0.0 {
                self.level = to;
                self.stage += 1;
                self.from = to;
                self.t = 0.0;
                continue;
            }
            let step = left.min(dur - self.t);
            self.t += step;
            left -= step;
            let x = (self.t / dur).min(1.0);
            self.level = self.from + (to - self.from) * bend(x, s.c[k]);
            if self.t >= dur {
                self.level = to;
                self.stage += 1;
                self.from = to;
                self.t = 0.0;
            }
        }
        self.level
    }
}

/// Linear attack, then exponential-looking decay and release: the modulation envelope.
#[derive(Copy, Clone, Debug, Default)]
pub struct ModAdsr {
    pub level: f32,
    stage: u8,
}

impl ModAdsr {
    pub fn start(&mut self) {
        self.stage = 0;
    }
    pub fn release(&mut self) {
        self.stage = 3;
    }
    pub fn advance(&mut self, a: f32, d: f32, s: f32, r: f32, dt: f32) -> f32 {
        match self.stage {
            0 => {
                self.level += if a > 0.0 { dt / a } else { 1.0 };
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = 1;
                }
            }
            1 | 2 => {
                let k = if d > 0.0 { (-dt * 4.6 / d).exp() } else { 0.0 };
                self.level = s + (self.level - s) * k;
            }
            _ => {
                let k = if r > 0.0 { (-dt * 4.6 / r).exp() } else { 0.0 };
                self.level *= k;
                if self.level < 1e-6 {
                    self.level = 0.0;
                }
            }
        }
        self.level
    }
}

/// A per-voice LFO: sine, triangle, saw down, saw up, square, sample and hold.
#[derive(Copy, Clone, Debug, Default)]
pub struct Lfo {
    /// Cycles, 0…1.
    pub phase: f64,
    pub held: f32,
    /// Seconds since the note began (for the delay).
    pub age: f32,
    pub rng: u32,
}

impl Lfo {
    /// Start a note: the phase, and the first held value, from the note's own seed.
    pub fn start(&mut self, phase: f64, seed: u32) {
        self.phase = phase;
        self.age = 0.0;
        self.rng = seed | 1;
        self.hold_next();
    }

    fn hold_next(&mut self) {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.held = (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0;
    }

    pub fn advance(&mut self, rate: f32, wave: u8, delay: f32, dt: f32) -> f32 {
        let before = self.phase;
        self.phase += rate as f64 * dt as f64;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
        }
        if wave == 5 && self.phase < before {
            self.hold_next();
        }
        self.age += dt;
        let x = self.phase as f32;
        let v = match wave {
            1 => 1.0 - 4.0 * (x - 0.5).abs(),
            2 => 1.0 - 2.0 * x,
            3 => 2.0 * x - 1.0,
            4 => {
                if x < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            5 => self.held,
            _ => crate::modulus::fast_sin_cycles(x),
        };
        let fade = if delay > 0.0 {
            (self.age / delay).min(1.0)
        } else {
            1.0
        };
        v * fade
    }
}
