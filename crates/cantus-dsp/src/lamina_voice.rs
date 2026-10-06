//! One key of Lamina: its tine (or reed) as a few modes, the hammer that strikes it, the
//! pickup that hears its tip, its damper and its noises.
//!
//! **Modes** are complex one-pole resonators (a phasor turning at the mode's frequency and
//! shrinking by its decay each sample), so a decay can change every sample — the damper
//! landing, half a pedal — without a click, and the tip's displacement, in mm, is the sum
//! of their real parts.
//!
//! **The hammer** is a mass on a power-law spring against a near-rigid point (it meets the
//! tine close to its clamp, where the tine barely moves): `m ÿ = −K yᵖ`, `p = 2.5`. Its
//! force pulse has one shape whatever the strike, only its length changes — shorter for a
//! harder tip and, by the spring's hardening, for a faster hammer. That is why brightness
//! rises with velocity by itself: a shorter pulse reaches further up the modes. The shape
//! is computed once ([`HammerShape`]); a strike scales it in time and drives every mode
//! through it, sample by sample, weighted by the mode's shape at the strike point.
//!
//! **Loudness** is set, not emergent: the fundamental's swing after the strike is exactly
//! `swing_ff · 10^(−dynamics·(1 − v)/20)` mm (the pulse's own spectrum at the fundamental is
//! measured at the strike to scale it), so the dynamics range is the one printed; the
//! physics decides the colour.

// Parallel per-wheel / per-mode arrays read clearest by index (as in `spatium_*`).
#![allow(clippy::needless_range_loop)]

use super::geometry::{
    strike_weight, swing_ff_mm, transfer64, Engine, BAR_RATIO, REED_RATIOS, TINE_RATIOS,
};
use crate::math;
use crate::organum::filters::OnePole;

pub const MAX_MODES: usize = 5;
/// Points in the hammer pulse's shape.
pub const SHAPE_POINTS: usize = 256;
/// The spring's exponent.
pub const HAMMER_P: f32 = 2.5;
/// The longest contact, s.
pub const MAX_CONTACT_S: f32 = 0.012;
/// A voice below this swing (mm, −120 dB re 1 mm) with nothing else moving is over.
const QUIET_MM2: f32 = 1e-12;

/// The universal pulse of a power-law contact, normalised to peak 1 over τ ∈ [0, 1].
#[derive(Clone, Debug)]
pub struct HammerShape {
    pub shape: [f32; SHAPE_POINTS + 1],
}

impl Default for HammerShape {
    fn default() -> Self {
        Self::new()
    }
}

impl HammerShape {
    /// Integrate `ÿ = −yᵖ` from `y = 0, ẏ = 1` until the mass leaves, finely (RK4).
    pub fn new() -> Self {
        let p = HAMMER_P as f64;
        let f = |y: f64| -math::powf(y.max(0.0), p);
        let dt = 1e-4;
        let (mut y, mut v, mut t) = (0.0f64, 1.0f64, 0.0f64);
        let mut ys = Vec::with_capacity(40_000);
        loop {
            ys.push(y);
            let (k1y, k1v) = (v, f(y));
            let (k2y, k2v) = (v + 0.5 * dt * k1v, f(y + 0.5 * dt * k1y));
            let (k3y, k3v) = (v + 0.5 * dt * k2v, f(y + 0.5 * dt * k2y));
            let (k4y, k4v) = (v + dt * k3v, f(y + dt * k3y));
            y += dt / 6.0 * (k1y + 2.0 * k2y + 2.0 * k3y + k4y);
            v += dt / 6.0 * (k1v + 2.0 * k2v + 2.0 * k3v + k4v);
            t += dt;
            if (y <= 0.0 && t > dt * 4.0) || ys.len() > 1_000_000 {
                break;
            }
        }
        let peak = math::powf(ys.iter().cloned().fold(0.0, f64::max), p);
        let mut shape = [0.0f32; SHAPE_POINTS + 1];
        for (i, s) in shape.iter_mut().enumerate() {
            let pos = i as f64 / SHAPE_POINTS as f64 * (ys.len() - 1) as f64;
            let j = (pos as usize).min(ys.len() - 1);
            *s = (math::powf(ys[j].max(0.0), p) / peak) as f32;
        }
        HammerShape { shape }
    }

    #[inline]
    pub fn at(&self, tau: f32) -> f32 {
        if !(0.0..1.0).contains(&tau) {
            return 0.0;
        }
        let x = tau * SHAPE_POINTS as f32;
        let i = x as usize;
        let f = x - i as f32;
        self.shape[i] + (self.shape[i + 1] - self.shape[i]) * f
    }
}

/// How long the hammer stays on the tine, s: the tip's hardness (0…1) sets it at full
/// velocity (3.2 ms soft, 0.5 ms hard), and a slower hammer stays longer (`v^(−(p−1)/(p+1))`,
/// the power-law spring's scaling). `vf` is the hammer's speed relative to full.
pub fn contact_s(hardness: f32, vf: f32) -> f32 {
    let t_ff = 0.0032 * (1.0 - 0.85 * hardness.clamp(0.0, 1.0));
    let e = (HAMMER_P - 1.0) / (HAMMER_P + 1.0);
    (t_ff * math::powf(vf.clamp(1e-3, 1.0), -e)).min(MAX_CONTACT_S)
}

/// Everything one strike is: latched when the key goes down.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Strike {
    pub key: u8,
    pub id: u64,
    pub engine: Engine,
    /// The fundamental, Hz (tuning, spread and bend applied).
    pub hz: f32,
    /// After the velocity curve, 0…1.
    pub velocity: f32,
    pub hardness: f32,
    pub dynamics_db: f32,
    /// Where the hammer meets it, fraction of the length from the clamp.
    pub strike: f32,
    /// Fundamental's decay to −60 dB, s.
    pub decay_s: f32,
    /// The tone bar's share of the fundamental (tine), 0…1.
    pub couple: f32,
    /// Upper modes against the physics, dB (tine: bell; reed: overtones).
    pub bell_db: f32,
    pub bell_decay_s: f32,
    pub gap_mm: f32,
    pub align_mm: f32,
    /// Everything that scales the output, dB (force, the key curve's level, pickup).
    pub level_db: f32,
    pub hammer_noise_db: f32,
    pub damper_ms: f32,
    pub damper_tone: f32,
    pub release_noise_db: f32,
    pub damper_noise_db: f32,
    /// Detune of the upper modes, fraction (a tuning spring, age).
    pub mode_detune: [f32; 3],
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Mode {
    re: f32,
    im: f32,
    c: f32,
    s: f32,
    r: f32,
    ln_free: f32,
    ln_damp: f32,
    gain: f32,
    ratio: f32,
}

/// A short burst of seeded noise, band-passed.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Burst {
    rng: u64,
    env: f32,
    decay: f32,
    lo: OnePole,
    hi: OnePole,
}

impl Burst {
    fn start(&mut self, seed: u64, level: f32, sr: f32, ms: f32, lo_hz: f32, hi_hz: f32) {
        self.rng = seed | 1;
        self.env = level;
        self.decay = math::exp(-6.9078 / (ms * 0.001 * sr));
        self.lo.set_hz(sr, hi_hz);
        self.hi.set_hz(sr, lo_hz);
    }

    #[inline]
    fn next(&mut self) -> f32 {
        if self.env == 0.0 {
            return 0.0;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        let w = (self.rng >> 40) as f32 / (1u64 << 23) as f32 - 1.0;
        let band = self.lo.run(w) - self.hi.run(w);
        let y = band * self.env;
        self.env *= self.decay;
        if self.env < 1e-6 {
            self.env = 0.0;
            self.lo.reset();
            self.hi.reset();
        }
        y
    }

    fn busy(&self) -> bool {
        self.env != 0.0
    }
}

/// One key.
#[derive(Clone, Debug, Default)]
pub struct Voice {
    pub active: bool,
    pub key: u8,
    pub id: u64,
    /// The key is down (the damper is off the tine whatever the pedal).
    pub key_down: bool,
    s: Strike,
    modes: [Mode; MAX_MODES],
    n: usize,
    /// Samples since the strike, and the contact's length in samples.
    t: u32,
    contact: f32,
    prev: f64,
    level: f32,
    /// The damper: 0 off the tine, 1 on it; where it is going; per-sample step.
    damp: f32,
    damp_target: f32,
    damp_step: f32,
    hammer_noise: Burst,
    release_noise: Burst,
    /// A stolen voice fades out before its new strike.
    fade: f32,
    fade_step: f32,
    pending: Option<Strike>,
    /// The tip's swing over the last stretch, for the pickup display.
    pub swing_lo: f32,
    pub swing_hi: f32,
    pub x: f32,
    sr: f32,
}

fn ln_r(t60_s: f32, sr: f32) -> f32 {
    -6.9078 / (t60_s.max(1e-3) * sr)
}

impl Voice {
    pub fn new(sample_rate: f32) -> Self {
        Voice {
            sr: sample_rate.max(1.0),
            fade: 1.0,
            ..Default::default()
        }
    }

    pub fn strike(&self) -> &Strike {
        &self.s
    }

    /// The fundamental's swing now, mm.
    pub fn amplitude(&self) -> f32 {
        let m = &self.modes[0];
        let b = &self.modes[1];
        (math::powi(m.re + b.re, 2) + math::powi(m.im + b.im, 2)).sqrt()
    }

    /// Start a strike. `stolen`: the voice was sounding another note — fade that out over
    /// 3 ms first.
    pub fn start(&mut self, s: &Strike, shape: &HammerShape, stolen: bool) {
        if stolen && self.active {
            self.pending = Some(*s);
            self.fade_step = 1.0 / (0.003 * self.sr);
            self.key = s.key;
            self.id = s.id;
            self.key_down = true;
            return;
        }
        self.begin(s, shape, false);
    }

    /// Take over the ringing state of another voice on the same key: a re-strike of a
    /// moving tine, as on the instrument.
    pub fn take_state(&mut self, other: &Voice) {
        self.modes = other.modes;
        self.n = other.n;
        self.prev = other.prev;
        self.active = true;
    }

    /// Strike again, keeping the modes' motion (after [`take_state`](Self::take_state)).
    pub fn restrike(&mut self, s: &Strike, shape: &HammerShape) {
        self.begin(s, shape, true);
    }

    /// Bend: every mode's frequency times `ratio`.
    pub fn retune(&mut self, ratio: f32) {
        self.s.hz *= ratio;
        let nyq = 0.45 * self.sr;
        for m in &mut self.modes[..self.n] {
            let hz = self.s.hz * m.ratio;
            if m.gain == 0.0 || hz >= nyq {
                continue;
            }
            let w = std::f32::consts::TAU * hz / self.sr;
            (m.s, m.c) = math::sin_cos(w);
        }
    }

    fn begin(&mut self, s: &Strike, shape: &HammerShape, keep_state: bool) {
        let sr = self.sr;
        let nyq = 0.45 * sr;
        let keep = keep_state && self.active;
        self.s = *s;
        self.key = s.key;
        self.id = s.id;
        self.key_down = true;
        self.active = true;
        self.t = 0;
        self.fade = 1.0;
        self.fade_step = 0.0;
        self.pending = None;
        self.damp = 0.0;
        self.damp_target = 0.0;
        self.damp_step = 1.0 / (0.008 * sr);
        let f0 = s.hz;
        let vf = math::powf(10f32, -s.dynamics_db * (1.0 - s.velocity) / 40.0);
        // A tine moving back faster than the hammer throws it off: the contact never
        // outlasts half a period of the fundamental.
        let tc = contact_s(s.hardness, vf).min(0.5 / f0.max(1.0));
        self.contact = tc * sr;
        let swing = swing_ff_mm(s.engine, f0)
            * math::powf(10f32, -s.dynamics_db * (1.0 - s.velocity) / 20.0);
        let bell = math::powf(10f32, s.bell_db / 20.0);
        let xi = s.strike.clamp(0.01, 0.5);
        let e = [
            strike_weight(0, xi),
            strike_weight(1, xi),
            strike_weight(2, xi),
        ];
        // (ratio, share of the pulse relative to the fundamental, decay).
        let mut spec = [(0.0f32, 0.0f32, 1.0f32); MAX_MODES];
        let n = match s.engine {
            Engine::Tine => {
                let c = s.couple.clamp(0.0, 1.0);
                spec[0] = (1.0, 1.0 - c, s.decay_s * 0.35);
                spec[1] = (1.0, c, s.decay_s);
                spec[2] = (
                    TINE_RATIOS[1] * (1.0 + s.mode_detune[1]),
                    e[1] / e[0] * bell,
                    s.bell_decay_s,
                );
                spec[3] = (
                    TINE_RATIOS[2] * (1.0 + s.mode_detune[2]),
                    e[2] / e[0] * bell,
                    s.bell_decay_s * 0.5,
                );
                spec[4] = (
                    BAR_RATIO * (1.0 + s.mode_detune[0]),
                    c * 0.5 * bell,
                    s.bell_decay_s * 0.8,
                );
                5
            }
            Engine::Reed => {
                spec[0] = (1.0, 1.0, s.decay_s);
                spec[1] = (
                    REED_RATIOS[1] * (1.0 + s.mode_detune[1]),
                    e[1] / e[0] * bell,
                    s.decay_s * 0.15,
                );
                spec[2] = (
                    REED_RATIOS[2] * (1.0 + s.mode_detune[2]),
                    e[2] / e[0] * bell,
                    s.decay_s * 0.05,
                );
                3
            }
        };
        // The fundamental's response to this pulse: Σ F(n) e^{−jωnT}, so its swing after the
        // strike can be set exactly.
        let w0 = std::f32::consts::TAU * f0 / sr;
        let (mut cr, mut ci) = (1.0f32, 0.0f32);
        let (rs, rc) = math::sin_cos(w0);
        let (mut sum_r, mut sum_i) = (0.0f32, 0.0f32);
        let len = self.contact.ceil() as usize + 1;
        for k in 0..len {
            let f = shape.at(k as f32 / self.contact);
            sum_r += f * cr;
            sum_i -= f * ci;
            let nr = cr * rc - ci * rs;
            ci = cr * rs + ci * rc;
            cr = nr;
        }
        let resp = (sum_r * sum_r + sum_i * sum_i).sqrt().max(1e-9);
        let g1 = swing / resp;
        let damper_t60 = s.damper_ms * 0.001;
        for (i, (ratio, share, t60)) in spec.iter().enumerate().take(n) {
            let hz = f0 * ratio;
            let m = &mut self.modes[i];
            if !keep {
                m.re = 0.0;
                m.im = 0.0;
            }
            m.ratio = *ratio;
            if hz >= nyq || *share <= 0.0 {
                m.gain = 0.0;
                m.c = 1.0;
                m.s = 0.0;
                m.ln_free = ln_r(0.001, sr);
                m.ln_damp = m.ln_free;
                m.r = math::exp(m.ln_free);
                continue;
            }
            let w = std::f32::consts::TAU * hz / sr;
            (m.s, m.c) = math::sin_cos(w);
            m.ln_free = ln_r(*t60, sr);
            // Damped: the damper's time, the upper modes faster as its tone rises.
            let damp_t60 = damper_t60 * math::powf(*ratio, -s.damper_tone.clamp(0.0, 1.0));
            m.ln_damp = ln_r(damp_t60, sr).min(m.ln_free);
            m.r = math::exp(m.ln_free);
            m.gain = g1 * share / ratio;
        }
        for m in &mut self.modes[n..] {
            *m = Mode::default();
        }
        self.n = n;
        if !keep {
            self.prev = transfer64(s.engine, 0.0, s.gap_mm as f64, s.align_mm as f64);
        }
        self.level = math::powf(10f32, s.level_db / 20.0);
        let hn = math::powf(10f32, s.hammer_noise_db / 20.0)
            * math::powf(10f32, -s.dynamics_db * (1.0 - s.velocity) / 20.0);
        self.hammer_noise
            .start(s.id ^ 0x4841_4d4d, hn * 0.5, sr, 4.0, 700.0, 3_500.0);
        self.release_noise = Burst::default();
        self.swing_lo = 0.0;
        self.swing_hi = 0.0;
    }

    /// The key came up. `damp`: where the damper goes (1 = fully on the tine; less under
    /// half a pedal).
    pub fn key_up(&mut self, damp: f32, release_velocity: f32) {
        self.key_down = false;
        self.set_damper(damp, release_velocity);
    }

    /// Move the damper toward `damp` (0…1). When it newly lands on a moving tine, it
    /// makes its noise.
    pub fn set_damper(&mut self, damp: f32, release_velocity: f32) {
        let damp = if self.key_down {
            0.0
        } else {
            damp.clamp(0.0, 1.0)
        };
        if damp > self.damp_target + 0.25 && damp > 0.5 {
            // The felt lands on a moving tine: a thump that grows with the swing it stops.
            let a = self.amplitude().min(4.0);
            let level = math::powf(10f32, self.s.release_noise_db / 20.0)
                * (0.3 + a)
                * (0.5 + release_velocity);
            let seed = self.s.id.wrapping_mul(0x9E37_79B9) ^ 0x5245_4c45;
            self.release_noise
                .start(seed, level * 0.3, self.sr, 25.0, 120.0, 900.0);
        } else if damp + 0.25 < self.damp_target && self.active {
            // The pedal lifts the felt off: a softer brush of the damper leaving the tine.
            let level = math::powf(10f32, self.s.damper_noise_db / 20.0) * 0.15;
            let seed = self.s.id.wrapping_mul(0x2545_F491) ^ 0x4441_4d50;
            self.release_noise
                .start(seed, level, self.sr, 40.0, 300.0, 2_500.0);
        }
        self.damp_target = damp;
    }

    /// Silence at once.
    pub fn clear(&mut self) {
        *self = Voice::new(self.sr);
    }

    /// One sample of this voice's pickup, added into the mono sum. Returns false when the
    /// voice has finished (its state is then exactly zero).
    #[inline]
    pub fn tick(&mut self, shape: &HammerShape, out: &mut f32) -> bool {
        if !self.active {
            return false;
        }
        // The damper moving: every mode's decay between free and damped.
        if self.damp != self.damp_target {
            if (self.damp_target - self.damp).abs() <= self.damp_step {
                self.damp = self.damp_target;
            } else if self.damp_target > self.damp {
                self.damp += self.damp_step;
            } else {
                self.damp -= self.damp_step;
            }
            let g = self.damp;
            for m in &mut self.modes[..self.n] {
                m.r = math::exp(m.ln_free + (m.ln_damp - m.ln_free) * g);
            }
        }
        let t = self.t as f32;
        let force = if t < self.contact {
            shape.at(t / self.contact)
        } else {
            0.0
        };
        self.t = self.t.saturating_add(1);
        let mut x = 0.0f64;
        let mut energy = 0.0;
        for m in &mut self.modes[..self.n] {
            let mut re = m.r * (m.re * m.c - m.im * m.s);
            let mut im = m.r * (m.re * m.s + m.im * m.c) - m.gain * force;
            // A fast mode dies long before the fundamental: flush it before it sinks into
            // denormals (a hundred times slower), deterministically, per sample.
            if re.abs() < 1e-20 && im.abs() < 1e-20 {
                re = 0.0;
                im = 0.0;
            }
            m.re = re;
            m.im = im;
            x += re as f64;
            energy += re * re + im * im;
        }
        let xf = x as f32;
        self.x = xf;
        self.swing_lo = self.swing_lo.min(xf);
        self.swing_hi = self.swing_hi.max(xf);
        let p = transfer64(
            self.s.engine,
            x,
            self.s.gap_mm as f64,
            self.s.align_mm as f64,
        );
        let v =
            ((p - self.prev) * self.sr as f64 * (1.0 / (std::f64::consts::TAU * 261.63))) as f32;
        self.prev = p;
        let mut y = v * self.level + self.hammer_noise.next() + self.release_noise.next();
        if let Some(next) = self.pending {
            y *= self.fade;
            self.fade -= self.fade_step;
            if self.fade <= 0.0 {
                self.begin(&next, shape, false);
                return true;
            }
        }
        *out += y;
        let busy = t < self.contact || self.hammer_noise.busy() || self.release_noise.busy();
        if !busy && energy < QUIET_MM2 {
            self.clear();
            return false;
        }
        true
    }
}
