//! Phasis's Uni-Vibe style (fx3:arca — ADR 0133): four phase stages that a lamp sweeps
//! through light-dependent resistors, after Darabundit, Wedelich & Bischoff, "Digital grey
//! box model of the Uni-Vibe effects pedal" (DAFx-19), whose Table 1 gives the values used
//! here.
//!
//! - **The stages are not tuned together** (the classic phaser's are): each has its own
//!   capacitor — 15 nF, 220 nF, 470 pF, 4.7 nF — and its own cell, whose measured
//!   resistance runs from 12.7k / 6.86k / 7.69k / 6.22k (the lamp bright) to 2.79M /
//!   2.59M / 3.32M / 4.16M (dark). Each stage is a first-order all-pass at
//!   `1/(2π·R·C)`: the notches fall unevenly and move unevenly — the throb.
//! - **The lamp** is an incandescent filament driven by the LFO: it heats and cools with a
//!   thermal lag, its light grows faster than its current. **The cells** (CdS) follow the
//!   light quickly as it brightens and slowly as it fades (their well-known asymmetry; the
//!   paper measures the curves' asymmetry but gives no time constants: those used here
//!   are typical of CdS cells, marked so in ADR 0133): the sweep rises quickly and falls
//!   slowly, more so as the intensity grows.
//! - A cell's resistance follows its light on a log scale (a CdS cell's power law): `ln R`
//!   moves between `ln R_min` and `ln R_max` with the light.
//!
//! The lamp and the cells are a state the LFO drives, independent of the audio: a skip
//! steps them sample by sample (a few multiplications each), so a render after silence is
//! the same bits as one that played it.

use crate::math;
use crate::modfx_line::Allpass1;

pub const STAGES: usize = 4;
/// The phasing capacitors (F) and the cells' measured extremes (Ω), stage by stage.
pub const CAPS: [f32; STAGES] = [15e-9, 220e-9, 470e-12, 4.7e-9];
pub const R_MIN: [f32; STAGES] = [12.7e3, 6.86e3, 7.69e3, 6.22e3];
pub const R_MAX: [f32; STAGES] = [2.79e6, 2.59e6, 3.32e6, 4.16e6];
/// The paper's measured mean resistance in use, stage by stage (what the test holds the
/// sweep's log-mean to, within a factor of two).
pub const R_MEAN: [f32; STAGES] = [405e3, 233e3, 290e3, 240e3];

/// The filament's thermal time constant and the cells' rise and fall (s): typical values.
const T_LAMP: f32 = 0.012;
const T_RISE: f32 = 0.004;
const T_FALL: f32 = 0.060;
/// The lamp never goes quite dark (its bias current).
const BIAS: f32 = 0.08;

/// One lamp and its four cells (one per channel: the right's LFO may run ahead).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Lamp {
    heat: f32,
    /// The cells' light, 0 … 1.
    pub light: f32,
}

#[derive(Clone, Debug)]
pub struct Vibe {
    sr: f32,
    k_lamp: f32,
    k_rise: f32,
    k_fall: f32,
    pub lamps: [Lamp; 2],
    ap: [[Allpass1; STAGES]; 2],
    ln_min: [f32; STAGES],
    ln_span: [f32; STAGES],
}

fn coef(sr: f32, tau: f32) -> f32 {
    1.0 - math::exp(-1.0 / (tau * sr))
}

impl Vibe {
    pub fn new(sample_rate: f32) -> Vibe {
        let mut ln_min = [0.0; STAGES];
        let mut ln_span = [0.0; STAGES];
        for k in 0..STAGES {
            ln_min[k] = math::ln(R_MIN[k]);
            ln_span[k] = math::ln(R_MAX[k]) - ln_min[k];
        }
        let mut v = Vibe {
            sr: sample_rate,
            k_lamp: coef(sample_rate, T_LAMP),
            k_rise: coef(sample_rate, T_RISE),
            k_fall: coef(sample_rate, T_FALL),
            lamps: [Lamp::default(); 2],
            ap: [[Allpass1::default(); STAGES]; 2],
            ln_min,
            ln_span,
        };
        v.reset();
        v
    }

    /// Lamps at their bias, stages silent.
    pub fn reset(&mut self) {
        let rest = BIAS * BIAS;
        self.lamps = [Lamp {
            heat: rest,
            light: rest,
        }; 2];
        self.ap = [[Allpass1::default(); STAGES]; 2];
    }

    /// The lamp of channel `ch` one sample on, driven by the LFO's `m` (−1 … 1) at
    /// `intensity` (0 … 1).
    #[inline]
    pub fn drive(&mut self, ch: usize, m: f32, intensity: f32) {
        let i = BIAS + (1.0 - BIAS) * intensity.clamp(0.0, 1.0) * 0.5 * (1.0 + m);
        let l = &mut self.lamps[ch];
        // Power in the filament ∝ I²; the light it gives grows faster still.
        l.heat += (i * i - l.heat) * self.k_lamp;
        let target = l.heat * l.heat.sqrt();
        let k = if target > l.light {
            self.k_rise
        } else {
            self.k_fall
        };
        l.light += (target - l.light) * k;
    }

    /// Stage `k`'s cell resistance for channel `ch` now (Ω).
    pub fn resistance(&self, ch: usize, k: usize) -> f32 {
        let g = self.lamps[ch].light.clamp(0.0, 1.0);
        math::exp(self.ln_min[k] + self.ln_span[k] * (1.0 - g))
    }

    /// Stage `k`'s frequency for channel `ch` now (Hz).
    pub fn stage_hz(&self, ch: usize, k: usize) -> f32 {
        1.0 / (std::f32::consts::TAU * self.resistance(ch, k) * CAPS[k])
    }

    /// One sample through channel `ch`'s four stages.
    #[inline]
    pub fn run(&mut self, ch: usize, x: f32) -> f32 {
        let top = 0.45 * self.sr;
        let mut w = x;
        for k in 0..STAGES {
            let hz = self.stage_hz(ch, k).min(top);
            let g = math::tan(std::f32::consts::PI * hz / self.sr);
            w = self.ap[ch][k].run(w, g / (1.0 + g));
        }
        w
    }

    /// Every stage silent (exactly).
    pub fn quiet(&self) -> bool {
        self.ap.iter().flatten().all(|a| a.state().to_bits() == 0)
    }

    /// Below `floor` everywhere: set to zero.
    pub fn settle(&mut self, floor: f32) -> bool {
        if self.ap.iter().flatten().all(|a| a.state().abs() < floor) {
            for a in self.ap.iter_mut().flatten() {
                a.settle(floor);
            }
            true
        } else {
            false
        }
    }
}
