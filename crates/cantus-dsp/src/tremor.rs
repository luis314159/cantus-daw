//! Tremor: tremolo, auto-pan and the harmonic tremolo (ADR 0055).
//!
//! - **Tremolo**: the level follows the LFO between full and `1 − depth` (linear gain), each
//!   channel at its own phase when the stereo spread is opened.
//! - **Auto-pan**: the LFO is a pan position, `depth` wide, under the **equal-power law**
//!   (ADR 0012), normalised to unity at the centre — a centred moment is the input itself,
//!   and the power of a mono source stays constant as it moves.
//! - **Harmonic** (the 1959 brown amps' "vibrato"): the signal split at a crossover, the low
//!   and the high halves modulated in opposite phase — a moving tone more than a moving
//!   level.
//!
//! The LFO is the family's (`modfx_lfo`): synced to the song position with swing, or free
//! from the start of playback; every shape has a soft edge so a square never clicks.
//! **Nulls**: at depth 0 (the default) every sample passes untouched, in every mode; the
//! output gain at 0 dB touches nothing. Exact rest: the unit holds no audio but the
//! crossover's two states, and its clock counts what [`Tremor::skip`] is told.

use crate::modfx_clock::{peak_db, peak_of, tel, Clock, Transport};
use crate::modfx_lfo::{Lfo, LfoSettings};
use crate::modfx_line::{Glide, Lp2, Lp2Coef};

/// What the LFO moves.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Tremolo,
    Pan,
    Harmonic,
}

impl Mode {
    pub fn from_index(i: f32) -> Mode {
        match i.round() as i32 {
            1 => Mode::Pan,
            2 => Mode::Harmonic,
            _ => Mode::Tremolo,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TremorSettings {
    pub mode: Mode,
    /// 0 … 1: how far the level (or the pan) moves.
    pub depth: f32,
    pub lfo: LfoSettings,
    /// The right channel's LFO ahead of the left, in cycles (0 … ½).
    pub spread: f32,
    /// The harmonic mode's crossover, Hz.
    pub xover_hz: f32,
    pub out_db: f32,
}

impl Default for TremorSettings {
    fn default() -> Self {
        TremorSettings {
            mode: Mode::Tremolo,
            depth: 0.0,
            lfo: LfoSettings::default(),
            spread: 0.0,
            xover_hz: 700.0,
            out_db: 0.0,
        }
    }
}

/// Glides land within this of their target (−120 dB of a unity gain).
const SNAP: f32 = 1e-6;
/// Crossover states below this are flushed to zero on silence.
const SETTLE: f32 = 1e-20;
/// Silent input samples in a row before the crossover is flushed.
const SETTLE_RUN: u32 = 64;

/// The equal-power pan gains of position `p` (−1 … 1), unity at the centre.
#[inline]
pub fn pan_gains(p: f32) -> (f32, f32) {
    let theta = (p.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    let (s, c) = crate::math::sin_cos(theta);
    (c * std::f32::consts::SQRT_2, s * std::f32::consts::SQRT_2)
}

#[derive(Clone, Debug)]
pub struct Tremor {
    sr: f32,
    set: TremorSettings,
    clock: Clock,
    lfo: Lfo,
    depth: Glide,
    out: Glide,
    spread: Glide,
    xover: Lp2Coef,
    lp: [Lp2; 2],
    /// Silent input samples in a row.
    zrun: u32,
    /// What the editor reads.
    tel: [f32; 16],
    blocks: u32,
}

impl Tremor {
    pub fn new(sample_rate: f32) -> Tremor {
        let set = TremorSettings::default();
        Tremor {
            sr: sample_rate,
            set,
            clock: Clock::new(),
            lfo: Lfo::new(sample_rate),
            depth: Glide::new(0.0, 10.0, sample_rate, SNAP),
            out: Glide::new(1.0, 10.0, sample_rate, SNAP),
            spread: Glide::new(0.0, 30.0, sample_rate, SNAP),
            xover: Lp2Coef::new(set.xover_hz, sample_rate),
            lp: [Lp2::default(); 2],
            zrun: 0,
            tel: [0.0; 16],
            blocks: 0,
        }
    }

    pub fn settings(&self) -> &TremorSettings {
        &self.set
    }

    /// New settings; changes glide.
    pub fn set(&mut self, s: &TremorSettings) {
        if s.xover_hz != self.set.xover_hz {
            self.xover = Lp2Coef::new(s.xover_hz.clamp(20.0, 0.45 * self.sr), self.sr);
        }
        self.lfo.set(&s.lfo, &self.clock);
        self.depth.target = s.depth.clamp(0.0, 1.0);
        self.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db)
        };
        self.spread.target = s.spread.clamp(0.0, 0.5);
        self.set = *s;
    }

    /// Where the song is, before the next call.
    pub fn set_transport(&mut self, t: &Transport) {
        self.clock.set(t);
        self.lfo.follow(&self.clock);
    }

    /// Back to the start of a stream, the glides on their targets.
    pub fn reset(&mut self) {
        self.clock.reset();
        self.lfo.reset();
        self.depth.jump(self.depth.target);
        self.out.jump(self.out.target);
        self.spread.jump(self.spread.target);
        self.lp = [Lp2::default(); 2];
        self.zrun = 0;
        self.tel = [0.0; 16];
        self.blocks = 0;
    }

    pub fn latency(&self) -> usize {
        0
    }

    /// Exact rest: no glide moving and the crossover holding nothing.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if !self.depth.still() || !self.out.still() || !self.spread.still() {
            return Some("a glide is moving");
        }
        if !self.lp.iter().all(Lp2::is_zero) {
            return Some("the crossover rings");
        }
        None
    }

    /// `frames` of silence skipped at rest.
    pub fn skip(&mut self, frames: usize) {
        self.clock.skip(frames);
        self.zrun = self.zrun.saturating_add(frames as u32);
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        let harmonic = self.set.mode == Mode::Harmonic;
        let (mut ml, mut mr) = (0.0f32, 0.0f32);
        for i in 0..n {
            let depth = self.depth.next();
            let out = self.out.next();
            let spread = self.spread.next();
            let (mut x, mut y) = (l[i], r[i]);
            // The crossover runs whenever the mode needs it, so a depth that comes back
            // finds it in step.
            let lows = if harmonic {
                // After a run of silent input, what is left of the crossover's ring is
                // flushed — per sample, so where it happens does not depend on the blocks.
                if x.to_bits() == 0 && y.to_bits() == 0 {
                    self.zrun = self.zrun.saturating_add(1);
                    if self.zrun >= SETTLE_RUN {
                        for lp in &mut self.lp {
                            lp.settle(SETTLE);
                        }
                    }
                } else {
                    self.zrun = 0;
                }
                (
                    self.lp[0].run(x, &self.xover),
                    self.lp[1].run(y, &self.xover),
                )
            } else {
                (0.0, 0.0)
            };
            if depth > 0.0 {
                ml = self.lfo.value(&self.clock, i);
                mr = if spread == 0.0 {
                    ml
                } else {
                    self.lfo.value_offset(&self.clock, i, f64::from(spread))
                };
                match self.set.mode {
                    Mode::Tremolo => {
                        x *= 1.0 - depth * (0.5 - 0.5 * ml);
                        y *= 1.0 - depth * (0.5 - 0.5 * mr);
                    }
                    Mode::Pan => {
                        let (gl, _) = pan_gains(depth * ml);
                        let (_, gr) = pan_gains(depth * mr);
                        x *= gl;
                        y *= gr;
                    }
                    Mode::Harmonic => {
                        let (lo_x, lo_y) = lows;
                        let lo = |m: f32| 1.0 - depth * (0.5 - 0.5 * m);
                        let hi = |m: f32| 1.0 - depth * (0.5 + 0.5 * m);
                        x = lo_x * lo(ml) + (x - lo_x) * hi(ml);
                        y = lo_y * lo(mr) + (y - lo_y) * hi(mr);
                    }
                }
            }
            if out != 1.0 {
                x *= out;
                y *= out;
            }
            l[i] = x;
            r[i] = y;
        }
        self.clock.advance(n);
        self.blocks = (self.blocks + 1) % (1 << 24);
        let cycles = self.lfo.cycles(&self.clock, 0);
        let (pl, _) = self.lfo.phase_at(cycles);
        let (pr, _) = self.lfo.phase_at(cycles + f64::from(self.spread.value));
        self.tel[tel::SAMPLE_RATE] = self.sr;
        self.tel[tel::TEMPO] = self.clock.tempo() as f32;
        self.tel[tel::PLAYING] = if self.clock.playing() { 1.0 } else { 0.0 };
        self.tel[tel::PHASE_L] = pl as f32;
        self.tel[tel::PHASE_R] = pr as f32;
        self.tel[tel::MOD_L] = ml;
        self.tel[tel::MOD_R] = mr;
        self.tel[tel::IN_PEAK] = peak_db(in_peak);
        self.tel[tel::OUT_PEAK] = peak_db(peak_of(&l[..n], &r[..n]));
        self.tel[tel::BLOCKS] = self.blocks as f32;
    }

    /// The editor's numbers (the family's header, [`tel`]).
    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..16].copy_from_slice(&self.tel);
    }
}

#[cfg(test)]
#[path = "tremor_tests.rs"]
mod tests;
