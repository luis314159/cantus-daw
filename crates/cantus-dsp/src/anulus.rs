//! Anulus: the ring modulator (ADR 0055).
//!
//! The signal multiplied by a carrier: every partial `f` becomes the pair `f ± f_c`, the
//! metallic, bell-like inharmonic sound of the ring of diodes it is named for (and, at a
//! few hertz, a tremolo that runs through zero).
//!
//! - **The carrier is band-limited**: a sine, or a triangle and a square whose corners and
//!   steps are smoothed with polyBLAMP and polyBLEP residuals (the residual of a unit step,
//!   `±(1 ∓ τ)²/2`, and its integral for a corner, `(1 ∓ τ)³/6`), so a high carrier does
//!   not fold its harmonics back into the band.
//! - **An LFO on the frequency**, in semitones either way, synced or free.
//! - **No DC leak**: a partial at the carrier's own frequency comes out as DC (and the
//!   second harmonic); the wet path's one-pole high-pass at 5 Hz takes the DC away.
//! - **Stereo**: the right carrier's phase apart from the left's.
//!
//! **Nulls**: at mix 0 (the default) the dry passes untouched. The carrier's phase runs
//! on silence; [`Anulus::skip`] advances it exactly as processing silence would.

use crate::modfx_clock::{peak_db, peak_of, tel, Clock, Transport};
use crate::modfx_lfo::{Lfo, LfoSettings, Rate, Shape};
use crate::modfx_line::Glide;

/// The carrier's wave (`anulus.wave.{i}`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Wave {
    #[default]
    Sine,
    Triangle,
    Square,
}

impl Wave {
    pub fn from_index(i: f32) -> Wave {
        match i.round() as i32 {
            1 => Wave::Triangle,
            2 => Wave::Square,
            _ => Wave::Sine,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AnulusSettings {
    /// The carrier's frequency, Hz.
    pub freq_hz: f32,
    pub wave: Wave,
    /// The LFO's swing of the frequency either way, semitones.
    pub lfo_st: f32,
    pub lfo: LfoSettings,
    /// The right carrier's phase ahead of the left, cycles (0 … ½).
    pub spread: f32,
    /// 0 … 1, the equal-gain crossfade (100 %: the product alone).
    pub mix: f32,
    pub out_db: f32,
}

impl Default for AnulusSettings {
    fn default() -> Self {
        AnulusSettings {
            freq_hz: 440.0,
            wave: Wave::Sine,
            lfo_st: 0.0,
            lfo: LfoSettings {
                rate: Rate::Hz(0.5),
                shape: Shape::Sine,
                ..LfoSettings::default()
            },
            spread: 0.0,
            mix: 0.0,
            out_db: 0.0,
        }
    }
}

const SNAP: f32 = 1e-6;
const SETTLE: f32 = 1e-20;
const SETTLE_RUN: u32 = 64;
/// The DC blocker's corner, Hz.
const DC_HZ: f32 = 5.0;

/// Anulus's own telemetry: the carrier's frequency now (Hz) and its value (−1 … 1).
pub const T_FREQ: usize = tel::OWN;
pub const T_CARRIER: usize = tel::OWN + 1;

/// The polyBLEP residual of a unit step at `tau` samples from it (−1 … 1).
#[inline]
fn blep(tau: f64) -> f64 {
    if tau < 0.0 {
        0.5 * (1.0 + tau) * (1.0 + tau)
    } else {
        -0.5 * (1.0 - tau) * (1.0 - tau)
    }
}

/// The polyBLAMP residual of a unit change of slope (per sample) at `tau` samples.
#[inline]
fn blamp(tau: f64) -> f64 {
    let a = 1.0 - tau.abs();
    a * a * a / 6.0
}

/// The distance in samples from phase `p` to a corner at phase `c`, at `dt` cycles a
/// sample, when it is within one sample.
#[inline]
fn near(p: f64, c: f64, dt: f64) -> Option<f64> {
    let mut d = p - c;
    d -= d.round();
    let tau = d / dt;
    (tau.abs() < 1.0).then_some(tau)
}

/// The band-limited carrier at phase `p` (0 … 1), `dt` cycles a sample.
pub fn carrier(wave: Wave, p: f64, dt: f64) -> f32 {
    let dt = dt.clamp(1e-9, 0.25);
    match wave {
        Wave::Sine => crate::math::sin(std::f64::consts::TAU * p) as f32,
        Wave::Square => {
            let mut v = if p < 0.5 { 1.0 } else { -1.0 };
            if let Some(t) = near(p, 0.0, dt) {
                v += 2.0 * blep(t);
            }
            if let Some(t) = near(p, 0.5, dt) {
                v -= 2.0 * blep(t);
            }
            v as f32
        }
        Wave::Triangle => {
            let mut v = if p < 0.25 {
                4.0 * p
            } else if p < 0.75 {
                2.0 - 4.0 * p
            } else {
                4.0 * p - 4.0
            };
            // The slope turns by −8 a cycle at the top, +8 at the bottom: ∓8·dt a sample.
            if let Some(t) = near(p, 0.25, dt) {
                v -= 8.0 * dt * blamp(t);
            }
            if let Some(t) = near(p, 0.75, dt) {
                v += 8.0 * dt * blamp(t);
            }
            v as f32
        }
    }
}

/// A one-pole DC blocker: `y = x − x₁ + R·y₁`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct DcBlock {
    x1: f32,
    y1: f32,
}

impl DcBlock {
    #[inline]
    fn run(&mut self, x: f32, r: f32) -> f32 {
        let y = x - self.x1 + r * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }

    fn is_zero(&self) -> bool {
        self.x1.to_bits() == 0 && self.y1.to_bits() == 0
    }
}

#[derive(Clone, Debug)]
pub struct Anulus {
    sr: f32,
    set: AnulusSettings,
    clock: Clock,
    lfo: Lfo,
    /// The carrier's phase, cycles (0 … 1).
    phase: f64,
    dc: [DcBlock; 2],
    dc_r: f32,
    /// log2 of the frequency, glided.
    freq: Glide,
    spread: Glide,
    mix: Glide,
    out: Glide,
    zrun: u32,
    tel: [f32; 16],
    blocks: u32,
}

impl Anulus {
    pub fn new(sample_rate: f32) -> Anulus {
        let mut a = Anulus {
            sr: sample_rate,
            set: AnulusSettings::default(),
            clock: Clock::new(),
            lfo: Lfo::new(sample_rate),
            phase: 0.0,
            dc: [DcBlock::default(); 2],
            dc_r: 1.0 - std::f32::consts::TAU * DC_HZ / sample_rate,
            freq: Glide::new(0.0, 20.0, sample_rate, 1e-6),
            spread: Glide::new(0.0, 30.0, sample_rate, SNAP),
            mix: Glide::new(0.0, 10.0, sample_rate, SNAP),
            out: Glide::new(1.0, 10.0, sample_rate, SNAP),
            zrun: 0,
            tel: [0.0; 16],
            blocks: 0,
        };
        let s = a.set;
        a.set(&s);
        a.reset();
        a
    }

    pub fn settings(&self) -> &AnulusSettings {
        &self.set
    }

    pub fn set(&mut self, s: &AnulusSettings) {
        self.freq.target = crate::math::log2(s.freq_hz.clamp(0.1, 0.45 * self.sr));
        self.spread.target = s.spread.clamp(0.0, 0.5);
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
        let epoch = self.clock.epoch();
        self.clock.set(t);
        if self.clock.epoch() != epoch {
            // Playback starts the carrier again, so live plays what the offline render does.
            self.phase = 0.0;
        }
        self.lfo.follow(&self.clock);
    }

    pub fn reset(&mut self) {
        self.clock.reset();
        self.lfo.reset();
        self.phase = 0.0;
        self.dc = [DcBlock::default(); 2];
        for g in [
            &mut self.freq,
            &mut self.spread,
            &mut self.mix,
            &mut self.out,
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

    /// At rest: the glides landed and the DC blocker empty. The carrier's phase is not
    /// audio state — silence times any carrier is silence — and [`Anulus::skip`] keeps it
    /// where processing would have.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if [&self.freq, &self.spread, &self.mix, &self.out]
            .iter()
            .any(|g| !g.still())
        {
            return Some("a glide is moving");
        }
        if !self.dc.iter().all(DcBlock::is_zero) {
            return Some("the DC blocker holds a value");
        }
        None
    }

    /// The carrier's increment `i` samples into the next call: its frequency, swung by
    /// the LFO, as cycles a sample.
    #[inline]
    fn inc(&self, log_f: f32, i: usize) -> f64 {
        let st = self.set.lfo_st;
        let lf = if st == 0.0 {
            log_f
        } else {
            log_f + st / 12.0 * self.lfo.value(&self.clock, i)
        };
        let f = crate::math::exp2(lf).min(0.45 * self.sr);
        f64::from(f) / f64::from(self.sr)
    }

    /// `frames` of silence skipped at rest: the carrier turns as it would have.
    pub fn skip(&mut self, frames: usize) {
        let log_f = self.freq.value;
        for i in 0..frames {
            self.phase += self.inc(log_f, i);
            self.phase -= self.phase.floor();
        }
        self.clock.skip(frames);
        self.zrun = self.zrun.saturating_add(frames as u32);
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        let wave = self.set.wave;
        let (mut c_now, mut f_now) = (0.0f32, 0.0f32);
        for i in 0..n {
            let (x, y) = (l[i], r[i]);
            if x.to_bits() == 0 && y.to_bits() == 0 {
                self.zrun = self.zrun.saturating_add(1);
                if self.zrun >= SETTLE_RUN
                    && self
                        .dc
                        .iter()
                        .all(|d| d.x1.abs() < SETTLE && d.y1.abs() < SETTLE)
                {
                    self.dc = [DcBlock::default(); 2];
                }
            } else {
                self.zrun = 0;
            }
            let log_f = self.freq.next();
            let spread = self.spread.next();
            let (gd, gw) = crate::concentus::mix_gains(self.mix.next());
            let out = self.out.next();
            let dt = self.inc(log_f, i);
            let cl = carrier(wave, self.phase, dt);
            let cr = if spread == 0.0 {
                cl
            } else {
                let p = self.phase + f64::from(spread);
                carrier(wave, p - p.floor(), dt)
            };
            // `+ 0.0`: silence times a negative carrier is −0.0, which is not the +0.0 a
            // resting state holds (ADR 0031 §5.1).
            let wl = self.dc[0].run(x * cl + 0.0, self.dc_r);
            let wr = self.dc[1].run(y * cr + 0.0, self.dc_r);
            self.phase += dt;
            self.phase -= self.phase.floor();
            c_now = cl;
            f_now = (dt * f64::from(self.sr)) as f32;
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
        self.tel[T_FREQ] = if n > 0 { f_now } else { self.tel[T_FREQ] };
        self.tel[T_CARRIER] = c_now;
    }

    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..16].copy_from_slice(&self.tel);
    }
}

#[cfg(test)]
#[path = "anulus_tests.rs"]
mod tests;
