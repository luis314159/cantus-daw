//! Phasis: the phaser (ADR 0055).
//!
//! A cascade of first-order allpass stages (TPT, Zavalishin's topology-preserving form) all
//! tuned to one frequency that the LFO sweeps between a low and a high bound on a log
//! scale; the cascade mixed with the dry makes a notch for every two stages, and the
//! notches move. **2 … 12 stages**, **feedback** of either polarity from the cascade's
//! output (one sample round the loop, as the pedals' "colour" switch and resonance do),
//! the right channel's LFO apart from the left's for a stereo sweep, the family's synced or
//! free LFO.
//!
//! The classic pedals are factory presets: the MXR Phase 90 (four stages, the "script"
//! version without feedback and the "block" version with it) and the Electro-Harmonix
//! Small Stone (four stages, its colour switch's resonance). **Nulls**: at mix 0 (the
//! default) the dry passes untouched.

use crate::modfx_clock::{peak_db, peak_of, tel, Clock, Transport};
use crate::modfx_lfo::{Lfo, LfoSettings, Rate, Shape};
use crate::modfx_line::{Allpass1, Glide};

/// Most stages.
pub const MAX_STAGES: usize = 12;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PhasisSettings {
    /// 2 … 12.
    pub stages: usize,
    /// 0 … 1, the equal-gain crossfade (both full at ½: the classic phaser).
    pub mix: f32,
    /// The sweep's bounds, Hz.
    pub lo_hz: f32,
    pub hi_hz: f32,
    /// −0.95 … 0.95.
    pub feedback: f32,
    pub lfo: LfoSettings,
    /// The right channel's LFO ahead of the left, cycles (0 … ½).
    pub spread: f32,
    pub out_db: f32,
}

impl Default for PhasisSettings {
    fn default() -> Self {
        PhasisSettings {
            stages: 4,
            mix: 0.0,
            lo_hz: 200.0,
            hi_hz: 2_000.0,
            feedback: 0.0,
            lfo: LfoSettings {
                rate: Rate::Hz(0.5),
                shape: Shape::Sine,
                ..LfoSettings::default()
            },
            spread: 0.0,
            out_db: 0.0,
        }
    }
}

const SNAP: f32 = 1e-6;
const SETTLE: f32 = 1e-20;
const SETTLE_RUN: u32 = 64;

/// Phasis's own telemetry: the stages' frequency now, left and right (Hz).
pub const T_FC_L: usize = tel::OWN;
pub const T_FC_R: usize = tel::OWN + 1;

/// The stages' frequency for a sweep value `m` (−1 … 1) between `lo` and `hi`, log.
#[inline]
pub fn sweep_hz(lo: f32, hi: f32, m: f32) -> f32 {
    let u = 0.5 * (m + 1.0);
    lo * crate::math::exp2(crate::math::log2(hi / lo) * u)
}

#[derive(Clone, Debug)]
pub struct Phasis {
    sr: f32,
    set: PhasisSettings,
    clock: Clock,
    lfo: Lfo,
    ap: [[Allpass1; MAX_STAGES]; 2],
    /// The cascade's last output, for the feedback.
    back: [f32; 2],
    /// log2 of the bounds, glided so a moved bound never clicks.
    lo: Glide,
    hi: Glide,
    feedback: Glide,
    mix: Glide,
    out: Glide,
    spread: Glide,
    zrun: u32,
    tel: [f32; 16],
    blocks: u32,
}

impl Phasis {
    pub fn new(sample_rate: f32) -> Phasis {
        let mut p = Phasis {
            sr: sample_rate,
            set: PhasisSettings::default(),
            clock: Clock::new(),
            lfo: Lfo::new(sample_rate),
            ap: [[Allpass1::default(); MAX_STAGES]; 2],
            back: [0.0; 2],
            lo: Glide::new(0.0, 30.0, sample_rate, 1e-5),
            hi: Glide::new(0.0, 30.0, sample_rate, 1e-5),
            feedback: Glide::new(0.0, 20.0, sample_rate, SNAP),
            mix: Glide::new(0.0, 10.0, sample_rate, SNAP),
            out: Glide::new(1.0, 10.0, sample_rate, SNAP),
            spread: Glide::new(0.0, 30.0, sample_rate, SNAP),
            zrun: 0,
            tel: [0.0; 16],
            blocks: 0,
        };
        let s = p.set;
        p.set(&s);
        p.reset();
        p
    }

    pub fn settings(&self) -> &PhasisSettings {
        &self.set
    }

    pub fn set(&mut self, s: &PhasisSettings) {
        let top = 0.45 * self.sr;
        let lo = s.lo_hz.clamp(10.0, top);
        let hi = s.hi_hz.clamp(10.0, top);
        self.lo.target = crate::math::log2(lo.min(hi));
        self.hi.target = crate::math::log2(hi.max(lo));
        self.feedback.target = s.feedback.clamp(-0.95, 0.95);
        self.mix.target = s.mix.clamp(0.0, 1.0);
        self.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db)
        };
        self.spread.target = s.spread.clamp(0.0, 0.5);
        self.lfo.set(&s.lfo, &self.clock);
        self.set = *s;
        self.set.stages = s.stages.clamp(2, MAX_STAGES);
    }

    pub fn set_transport(&mut self, t: &Transport) {
        self.clock.set(t);
        self.lfo.follow(&self.clock);
    }

    pub fn reset(&mut self) {
        self.clock.reset();
        self.lfo.reset();
        self.ap = [[Allpass1::default(); MAX_STAGES]; 2];
        self.back = [0.0; 2];
        for g in [
            &mut self.lo,
            &mut self.hi,
            &mut self.feedback,
            &mut self.mix,
            &mut self.out,
            &mut self.spread,
        ] {
            g.jump(g.target);
        }
        self.zrun = 0;
        self.tel = [0.0; 16];
        self.blocks = 0;
    }

    pub fn latency(&self) -> usize {
        0
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if [
            &self.lo,
            &self.hi,
            &self.feedback,
            &self.mix,
            &self.out,
            &self.spread,
        ]
        .iter()
        .any(|g| !g.still())
        {
            return Some("a glide is moving");
        }
        if self.back.iter().any(|v| v.to_bits() != 0)
            || self
                .ap
                .iter()
                .flatten()
                .any(|a| a.state().to_bits() != 0)
        {
            return Some("a stage rings");
        }
        None
    }

    pub fn skip(&mut self, frames: usize) {
        self.clock.skip(frames);
        self.zrun = self.zrun.saturating_add(frames as u32);
    }

    /// The coefficient `g/(1 + g)` of a stage at `log2(hz)`.
    #[inline]
    fn coef(&self, log_hz: f32) -> f32 {
        let hz = crate::math::exp2(log_hz).min(0.45 * self.sr);
        let g = crate::math::tan(std::f32::consts::PI * hz / self.sr);
        g / (1.0 + g)
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        let stages = self.set.stages;
        let (mut ml, mut mr) = (0.0f32, 0.0f32);
        let (mut fl, mut fr) = (0.0f32, 0.0f32);
        for i in 0..n {
            let (x, y) = (l[i], r[i]);
            if x.to_bits() == 0 && y.to_bits() == 0 {
                self.zrun = self.zrun.saturating_add(1);
            } else {
                self.zrun = 0;
            }
            let lo = self.lo.next();
            let hi = self.hi.next();
            let fb = self.feedback.next();
            let (gd, gw) = crate::concentus::mix_gains(self.mix.next());
            let out = self.out.next();
            let spread = self.spread.next();
            ml = self.lfo.value(&self.clock, i);
            mr = if spread == 0.0 {
                ml
            } else {
                self.lfo.value_offset(&self.clock, i, f64::from(spread))
            };
            // The sweep on a log scale: lo … hi as the LFO goes −1 … 1.
            fl = lo + (hi - lo) * 0.5 * (ml + 1.0);
            fr = lo + (hi - lo) * 0.5 * (mr + 1.0);
            let (hl, hr) = (self.coef(fl), self.coef(fr));
            if self.zrun >= SETTLE_RUN {
                let quiet = self.back.iter().all(|v| v.abs() < SETTLE)
                    && self.ap.iter().flatten().all(|a| a.state().abs() < SETTLE);
                if quiet {
                    self.back = [0.0; 2];
                    for a in self.ap.iter_mut().flatten() {
                        a.settle(SETTLE);
                    }
                }
            }
            let mut wl = x + fb * self.back[0];
            let mut wr = y + fb * self.back[1];
            for k in 0..stages {
                wl = self.ap[0][k].run(wl, hl);
                wr = self.ap[1][k].run(wr, hr);
            }
            self.back = [wl, wr];
            let (mut ox, mut oy) = if gw == 0.0 {
                (x, y)
            } else {
                (x * gd + wl * gw, y * gd + wr * gw)
            };
            if out != 1.0 {
                ox *= out;
                oy *= out;
            }
            l[i] = ox;
            r[i] = oy;
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
        self.tel[T_FC_L] = crate::math::exp2(fl);
        self.tel[T_FC_R] = crate::math::exp2(fr);
    }

    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..16].copy_from_slice(&self.tel);
    }
}

#[cfg(test)]
#[path = "phasis_tests.rs"]
mod tests;
