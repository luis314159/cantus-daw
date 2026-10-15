//! Translatio: the frequency shifter (ADR 0055), Bode-style.
//!
//! Every partial moves by the same number of hertz — not by a ratio, as a pitch shifter
//! would — so harmonics stop being harmonic: a few hertz make a slow phasing, tens make a
//! detuned, bell-like shimmer, hundreds a different instrument. Single sideband: the
//! signal's analytic pair from the family's Hilbert design (`modfx_hilbert`: two allpass
//! chains 90° apart, the coefficients designed for the sample rate — 69 dB of sideband
//! rejection over 20 Hz … 20 kHz at 44.1 and 48 kHz) multiplied by a quadrature carrier:
//!
//! - **up**: `I·cos φ − Q·sin φ` — every partial `f` to `f + shift`;
//! - **down**: `I·cos φ + Q·sin φ` — to `f − shift`;
//! - **both**: up on the left, down on the right, as the Bode shifter's two outputs.
//!
//! **Feedback** sends the shifted output back into the input (each pass shifts again: the
//! barber-pole spirals), **an LFO** swings the shift, synced or free. The shift can be
//! negative (up and down swap). **Nulls**: at mix 0 (the default) the dry passes untouched;
//! no latency (the allpasses only turn the phase).

use crate::modfx_clock::{peak_db, peak_of, tel, Clock, Transport};
use crate::modfx_hilbert::{design, Hilbert};
use crate::modfx_lfo::{Lfo, LfoSettings, Rate, Shape};
use crate::modfx_line::{Glide, OnePole};

/// The feedback path's band, Hz: inside the Hilbert pair's 90° band.
const FB_LOW_HZ: f32 = 25.0;
const FB_HIGH_HZ: f32 = 16_000.0;

/// Which sideband comes out (`translatio.side.{i}`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Side {
    #[default]
    Up,
    Down,
    /// Up on the left, down on the right.
    Both,
}

impl Side {
    pub fn from_index(i: f32) -> Side {
        match i.round() as i32 {
            1 => Side::Down,
            2 => Side::Both,
            _ => Side::Up,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TranslatioSettings {
    /// The shift, Hz (negative: the other way).
    pub shift_hz: f32,
    pub side: Side,
    /// 0 … 0.95: the shifted output back into the input.
    pub feedback: f32,
    /// The LFO's swing of the shift either way, Hz.
    pub lfo_hz: f32,
    pub lfo: LfoSettings,
    /// 0 … 1, the equal-gain crossfade.
    pub mix: f32,
    pub out_db: f32,
}

impl Default for TranslatioSettings {
    fn default() -> Self {
        TranslatioSettings {
            shift_hz: 10.0,
            side: Side::Up,
            feedback: 0.0,
            lfo_hz: 0.0,
            lfo: LfoSettings {
                rate: Rate::Hz(f64::from(0.2f32)),
                shape: Shape::Sine,
                ..LfoSettings::default()
            },
            mix: 0.0,
            out_db: 0.0,
        }
    }
}

const SNAP: f32 = 1e-6;
const SETTLE: f32 = 1e-20;
const SETTLE_RUN: u32 = 64;

/// Translatio's own telemetry: the shift now (Hz), and the design's sections and its
/// worst sideband rejection over the band (dB).
pub const T_SHIFT: usize = tel::OWN;
pub const T_SECTIONS: usize = tel::OWN + 1;
pub const T_REJECTION: usize = tel::OWN + 2;

#[derive(Clone, Debug)]
pub struct Translatio {
    sr: f32,
    set: TranslatioSettings,
    clock: Clock,
    lfo: Lfo,
    hilbert: [Hilbert; 2],
    sections: usize,
    rejection_db: f32,
    /// The carrier's phase, cycles (0 … 1).
    phase: f64,
    /// The last output of each channel's shifter, for the feedback.
    back: [f32; 2],
    /// The feedback path's band: a high-pass and a low-pass, one pole each.
    fb_hp: [OnePole; 2],
    fb_lp: [OnePole; 2],
    g_hp: f32,
    g_lp: f32,
    shift: Glide,
    feedback: Glide,
    mix: Glide,
    out: Glide,
    zrun: u32,
    tel: [f32; 16],
    blocks: u32,
}

impl Translatio {
    /// Designs the Hilbert pair for the sample rate: build it off the audio thread.
    pub fn new(sample_rate: f32) -> Translatio {
        let d = design(sample_rate);
        let mut t = Translatio {
            sr: sample_rate,
            set: TranslatioSettings::default(),
            clock: Clock::new(),
            lfo: Lfo::new(sample_rate),
            hilbert: [Hilbert::new(&d), Hilbert::new(&d)],
            sections: d.n,
            rejection_db: d.rejection_db as f32,
            phase: 0.0,
            back: [0.0; 2],
            fb_hp: [OnePole::default(); 2],
            fb_lp: [OnePole::default(); 2],
            g_hp: OnePole::coef(FB_LOW_HZ, sample_rate),
            g_lp: OnePole::coef(FB_HIGH_HZ.min(0.4 * sample_rate), sample_rate),
            shift: Glide::new(0.0, 20.0, sample_rate, 1e-4),
            feedback: Glide::new(0.0, 20.0, sample_rate, SNAP),
            mix: Glide::new(0.0, 10.0, sample_rate, SNAP),
            out: Glide::new(1.0, 10.0, sample_rate, SNAP),
            zrun: 0,
            tel: [0.0; 16],
            blocks: 0,
        };
        let s = t.set;
        t.set(&s);
        t.reset();
        t
    }

    pub fn settings(&self) -> &TranslatioSettings {
        &self.set
    }

    /// The Hilbert design's sections and its worst rejection over 20 Hz … 20 kHz, dB.
    pub fn design_quality(&self) -> (usize, f32) {
        (self.sections, self.rejection_db)
    }

    pub fn set(&mut self, s: &TranslatioSettings) {
        let top = 0.45 * self.sr;
        self.shift.target = s.shift_hz.clamp(-top, top);
        self.feedback.target = s.feedback.clamp(0.0, 0.95);
        self.mix.target = s.mix.clamp(0.0, 1.0);
        self.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db)
        };
        self.lfo.set(&s.lfo, &self.clock);
        self.set = *s;
    }

    pub fn set_transport(&mut self, t: &Transport) {
        self.clock.set(t);
        self.lfo.follow(&self.clock);
    }

    pub fn reset(&mut self) {
        self.clock.reset();
        self.lfo.reset();
        for h in &mut self.hilbert {
            h.reset();
        }
        self.phase = 0.0;
        self.back = [0.0; 2];
        self.fb_hp = [OnePole::default(); 2];
        self.fb_lp = [OnePole::default(); 2];
        for g in [&mut self.shift, &mut self.feedback, &mut self.mix, &mut self.out] {
            g.jump(g.target);
        }
        self.zrun = 0;
        self.tel = [0.0; 16];
        self.blocks = 0;
    }

    pub fn latency(&self) -> usize {
        0
    }

    /// At rest: the glides landed, the allpass chains and the feedback holding nothing.
    /// The carrier's phase is not audio state (it multiplies silence); `skip` keeps it.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if [&self.shift, &self.feedback, &self.mix, &self.out]
            .iter()
            .any(|g| !g.still())
        {
            return Some("a glide is moving");
        }
        if !self.hilbert.iter().all(Hilbert::is_zero)
            || self.back.iter().any(|v| v.to_bits() != 0)
            || !self.fb_hp.iter().chain(&self.fb_lp).all(OnePole::is_zero)
        {
            return Some("the allpass chains ring");
        }
        None
    }

    /// The shift `i` samples into the next call, Hz, with the LFO's swing.
    #[inline]
    fn shift_at(&self, base: f32, i: usize) -> f32 {
        let swing = self.set.lfo_hz;
        let s = if swing == 0.0 {
            base
        } else {
            base + swing * self.lfo.value(&self.clock, i)
        };
        let top = 0.45 * self.sr;
        s.clamp(-top, top)
    }

    /// `frames` of silence skipped at rest: the carrier turns as it would have.
    pub fn skip(&mut self, frames: usize) {
        let base = self.shift.value;
        for i in 0..frames {
            self.phase += f64::from(self.shift_at(base, i)) / f64::from(self.sr);
            self.phase -= self.phase.floor();
        }
        self.clock.skip(frames);
        self.zrun = self.zrun.saturating_add(frames as u32);
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        let side = self.set.side;
        let mut s_now = self.shift.value;
        for i in 0..n {
            let (x, y) = (l[i], r[i]);
            if x.to_bits() == 0 && y.to_bits() == 0 {
                self.zrun = self.zrun.saturating_add(1);
                if self.zrun >= SETTLE_RUN
                    && self.hilbert.iter().all(|h| h.level() < SETTLE)
                    && self.back.iter().all(|v| v.abs() < SETTLE)
                {
                    for h in &mut self.hilbert {
                        h.reset();
                    }
                    self.back = [0.0; 2];
                    self.fb_hp = [OnePole::default(); 2];
                    self.fb_lp = [OnePole::default(); 2];
                }
            } else {
                self.zrun = 0;
            }
            let base = self.shift.next();
            let fb = self.feedback.next();
            let (gd, gw) = crate::concentus::mix_gains(self.mix.next());
            let out = self.out.next();
            s_now = self.shift_at(base, i);
            let (sn, cs) = crate::math::sin_cos(std::f32::consts::TAU * self.phase as f32);
            let (il, ql) = self.hilbert[0].run(x + fb * self.back[0]);
            let (ir, qr) = self.hilbert[1].run(y + fb * self.back[1]);
            let up = |i: f32, q: f32| i * cs - q * sn;
            let down = |i: f32, q: f32| i * cs + q * sn;
            let (wl, wr) = match side {
                Side::Up => (up(il, ql), up(ir, qr)),
                Side::Down => (down(il, ql), down(ir, qr)),
                Side::Both => (up(il, ql), down(ir, qr)),
            };
            // `+ 0.0`: a product of silence is ±0; a resting state holds +0.
            // The feedback, kept inside the band where the pair is 90° apart: outside it the
            // two halves are not in quadrature, the shifter's gain can pass 1 and the loop
            // would ring. `+ 0.0`: a product of silence is ±0; a resting state holds +0.
            let keep = |w: f32, hp: &mut OnePole, lp: &mut OnePole, gh: f32, gl: f32| {
                let high = w - hp.run(w, gh);
                lp.run(high, gl) + 0.0
            };
            let (gh, gl) = (self.g_hp, self.g_lp);
            self.back = [
                keep(wl, &mut self.fb_hp[0], &mut self.fb_lp[0], gh, gl),
                keep(wr, &mut self.fb_hp[1], &mut self.fb_lp[1], gh, gl),
            ];
            self.phase += f64::from(s_now) / f64::from(self.sr);
            self.phase -= self.phase.floor();
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
        let (pl, _) = self.lfo.phase(&self.clock, 0);
        self.tel[tel::SAMPLE_RATE] = self.sr;
        self.tel[tel::TEMPO] = self.clock.tempo() as f32;
        self.tel[tel::PLAYING] = if self.clock.playing() { 1.0 } else { 0.0 };
        self.tel[tel::PHASE_L] = pl as f32;
        self.tel[tel::PHASE_R] = pl as f32;
        self.tel[tel::MOD_L] = self.lfo.value(&self.clock, 0);
        self.tel[tel::MOD_R] = self.tel[tel::MOD_L];
        self.tel[tel::IN_PEAK] = peak_db(in_peak);
        self.tel[tel::OUT_PEAK] = peak_db(peak_of(&l[..n], &r[..n]));
        self.tel[tel::BLOCKS] = self.blocks as f32;
        self.tel[T_SHIFT] = s_now;
        self.tel[T_SECTIONS] = self.sections as f32;
        self.tel[T_REJECTION] = self.rejection_db;
    }

    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..16].copy_from_slice(&self.tel);
    }
}

#[cfg(test)]
#[path = "translatio_tests.rs"]
mod tests;
