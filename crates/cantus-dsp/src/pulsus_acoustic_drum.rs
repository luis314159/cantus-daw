//! The membrane drum's strike and its sample.

use super::{bessel_j, decay_coef, Drum, DrumHit, MAX_RES, MEMBRANE64};
use crate::math;
use std::f32::consts::{PI, TAU};

/// Modes retune (the glide) every this many samples.
const RETUNE: u32 = 32;
/// The modes' bounds are checked every this many samples.
const CULL: u32 = 256;
/// The lowest modes, whose sum moves the snare's wires.
const LOW: usize = 6;
/// Samples before the wires answer (~1 ms at 48 kHz), and over which they grow (~5 ms).
const WIRE_DELAY: u32 = 48;
const WIRE_RISE: f32 = 240.0;

impl Drum {
    /// Strike the drum. Allocation-free; a handful of transcendental calls per mode.
    pub(crate) fn strike(&mut self, h: &DrumHit) {
        let sr = self.sr;
        let hard = h.hardness.clamp(0.0, 1.0);
        // The contact: a stick 2.2 ms soft … 0.35 ms hard; a felt beater 7 … 1.6 ms.
        let (soft_s, hard_s) = if h.beater {
            (0.007, 0.0016)
        } else {
            (0.0022, 0.000_35)
        };
        let contact_s = soft_s * math::powf(hard_s / soft_s, hard);
        self.contact = ((contact_s * sr) as u32).max(2);
        // A half-sine whose samples sum to one: a low mode's amplitude is its weight.
        self.force_k = PI / (2.0 * self.contact as f32);
        self.t = 0;
        self.f1 = 0.0;
        self.f2 = 0.0;
        let rho = 0.04 + 0.9 * h.position.clamp(0.0, 1.0);
        let gamma = 0.3 + 1.2 * h.damping.clamp(0.0, 1.0);
        let heads = h.heads.clamp(0.0, 1.0);
        let f0 = h.f0.max(20.0);
        let top = (sr * 0.45).min(16_000.0);
        let mut n = 0;
        let mut total = 0.0;
        for (k, &(m, j)) in MEMBRANE64.iter().enumerate().take(h.modes.min(MAX_RES)) {
            if n >= MAX_RES {
                break;
            }
            let ideal = j / MEMBRANE64[0].1;
            // Air loading and the second head pull the modes towards harmonic.
            let ratio = math::powf(ideal, 1.0 - 0.18 * heads);
            let hz = f0 * ratio;
            if hz >= top {
                break;
            }
            // The mode shape at the strike point, and how well the mode radiates: the
            // round modes push air, the others mostly cancel themselves.
            let w = bessel_j(m as u32, j * rho).abs();
            let rad = 1.0 / (1.0 + 0.55 * m as f32);
            let jitter = 1.0 + 0.18 * self.noise.next();
            let amp = w * rad * jitter;
            let mut t60 = h.decay * math::powf(f0 / hz, gamma) * (1.0 + 0.08 * self.noise.next());
            if k == 0 {
                // The (0,1) mode radiates best and loses its energy fastest.
                t60 *= 0.55 + 0.35 * heads;
            }
            self.res[n].set_struck(hz, t60, amp, sr);
            total += amp;
            n += 1;
            // Two heads: the lowest modes in pairs a few cents apart, beating.
            if heads > 0.0 && k < 5 && n < MAX_RES {
                let d = (0.003 + 0.009 * self.noise.unit()) * if k % 2 == 0 { 1.0 } else { -1.0 };
                let a = amp * 0.45 * heads;
                self.res[n].set_struck(hz * (1.0 + d), t60 * 1.25, a, sr);
                total += a;
                n += 1;
            }
        }
        // The shell and the air inside it: a few low, short resonances above the head's.
        if h.shell > 0.0 {
            for (i, r) in [2.31f32, 3.27, 6.9].iter().enumerate() {
                if n >= MAX_RES {
                    break;
                }
                let hz = f0 * r * (1.0 + 0.03 * self.noise.next());
                if hz >= top {
                    break;
                }
                let a = h.shell * 0.35 / (1.0 + i as f32);
                self.res[n].set_struck(hz, 0.09 + 0.05 * self.noise.unit(), a, sr);
                total += a;
                n += 1;
            }
        }
        self.n = n;
        if total > 0.0 {
            let g = 1.0 / total;
            for r in &mut self.res[..n] {
                r.b *= g;
                r.bound *= g * 4.0;
            }
        }
        // Tension: a hard hit starts sharp and falls as the head settles.
        let vel = h.vel.clamp(0.0, 1.0);
        self.glide = math::powf(2.0, h.glide.max(0.0) * vel * vel / 12.0) - 1.0;
        let tau = (h.decay * 0.12).clamp(0.02, 0.15);
        self.glide_coef = math::exp(-(RETUNE as f32) / (tau * sr));
        if self.glide > 0.0 {
            self.retune();
        }
        // The stick's (or beater's) own sound: a short band of noise and a crack above it.
        self.click = h.click.max(0.0) * (0.35 + 0.65 * math::powf(vel, 1.3));
        self.click_coef = decay_coef(
            if h.beater {
                0.003
            } else {
                0.0012 + 0.001 * (1.0 - hard)
            },
            sr,
        );
        let chz = h.click_hz.clamp(300.0, 12_000.0) * (0.8 + 0.4 * hard);
        self.click_band.reset();
        self.click_band.set(chz, 0.9, sr);
        self.click_crack.reset();
        self.click_crack.set((chz * 2.2).min(sr * 0.45), 0.7, sr);
        // Above the modes the bank holds, the head's modes are too dense to count: two
        // bands of noise ring for a fraction of the body's decay, brighter the harder.
        let c1 = (f0 * 14.0).clamp(900.0, 6_000.0);
        self.hb1.reset();
        self.hb1.set(c1, 1.1, sr);
        self.hb2.reset();
        self.hb2.set((c1 * 2.6).min(sr * 0.45), 1.1, sr);
        self.hb_env = 0.05 * (0.25 + 0.75 * hard * vel);
        self.hb_coef = decay_coef((h.decay * 0.6).clamp(0.08, 0.25), sr);
        // The wires.
        self.wires = h.wires.max(0.0);
        self.low = 0.0;
        self.wire_follow = 0.0;
        self.wire_follow_coef = decay_coef(0.025, sr);
        self.wire_env = 1.0;
        self.wire_coef = decay_coef(h.wire_decay.max(0.02), sr);
        self.wire_hp.reset();
        self.wire_hp.set(1_200.0, 0.6, sr);
        self.wire_bp.reset();
        self.wire_bp.set(3_200.0, 0.7, sr);
        self.wire_lp = 0.0;
        self.wire_lp_a = 1.0 - math::exp(-TAU * 9_000.0 / sr);
        // The head's real excursion: what the wires feel.
        self.wire_scale = math::powf(vel.max(0.01), 1.5);
        // A rimshot's rim: a few metal partials, struck by the same force.
        self.rim_on = h.rim > 0.0;
        if self.rim_on {
            for (i, hz) in [1_130.0f32, 2_350.0, 3_710.0, 5_620.0].iter().enumerate() {
                let hz = hz * (1.0 + 0.02 * self.noise.next());
                let a = h.rim * 0.25 / (1.0 + i as f32);
                self.rim[i].set_struck(hz, 0.16 - 0.025 * i as f32, a, sr);
            }
        }
        self.floor = 2e-6;
    }

    fn retune(&mut self) {
        let k = TAU / self.sr;
        let ratio = 1.0 + self.glide;
        for r in &mut self.res[..self.n] {
            r.retune(ratio, k);
        }
    }

    /// The drum's next sample.
    #[inline]
    pub(crate) fn tick(&mut self) -> f32 {
        let f = if self.t < self.contact {
            math::sin(PI * self.t as f32 / self.contact as f32) * self.force_k
        } else {
            0.0
        };
        // The modes hear the strike's acceleration (see `Res::set_struck`).
        let force = f - 2.0 * self.f1 + self.f2;
        self.f2 = self.f1;
        self.f1 = f;
        self.t = self.t.wrapping_add(1);
        if self.glide > 1e-5 && self.t.is_multiple_of(RETUNE) {
            self.glide *= self.glide_coef;
            self.retune();
        }
        if self.t.is_multiple_of(CULL) {
            self.cull();
        }
        let mut body = 0.0;
        let mut low = 0.0;
        for (i, r) in self.res[..self.n].iter_mut().enumerate() {
            let y = r.tick(force);
            body += y;
            if i < LOW {
                low += y;
            }
        }
        let mut out = body;
        if self.hb_env > 1e-6 {
            let w = self.noise.next();
            out += (self.hb1.tick(w).1 + 0.6 * self.hb2.tick(w).1) * self.hb_env;
            self.hb_env *= self.hb_coef;
        }
        if self.click > 1e-6 {
            let w = self.noise.next();
            let c = self.click_band.tick(w).1 + 0.5 * self.click_crack.tick(w).1;
            out += c * self.click;
            self.click *= self.click_coef;
        }
        if self.wires > 0.0 && self.t > WIRE_DELAY {
            // The snare-side head's motion, at the force it really has (the air inside the
            // shell reaches it, and the wires answer, a moment after the stick).
            let d = low * self.wire_scale * 6.0;
            let a = d.abs();
            self.wire_follow = a.max(self.wire_follow * self.wire_follow_coef);
            // Compressive: the wires saturate, so a ghost note is mostly wires.
            let e = self.wire_follow;
            let sat = e / (e + 0.4);
            // Grainy: the wires slap the head as it swings.
            let grain = 0.45 + 0.55 * (a / (e + 1e-9));
            let w = self.noise.next();
            let hp = self.wire_hp.tick(w).2;
            let bp = self.wire_bp.tick(w).1;
            // The wires' own top: little above 9 kHz (a recorded snare's 16 kHz band sits
            // 30 dB under its body).
            self.wire_lp += self.wire_lp_a * (0.6 * bp + 0.4 * hp - self.wire_lp);
            // The wires take a few milliseconds to start rattling: the attack is the
            // stick's and the head's (a recorded snare's attack is darker soft than hard).
            let rise = ((self.t - WIRE_DELAY) as f32 / WIRE_RISE).min(1.0);
            let buzz = self.wire_lp * sat * grain * (0.25 + 0.75 * self.wire_env) * rise;
            self.wire_env *= self.wire_coef;
            out += buzz * self.wires / self.wire_scale.max(0.02) * 0.35;
        }
        if self.rim_on {
            for r in &mut self.rim {
                out += r.tick(force);
            }
        }
        out
    }

    /// Drop the trailing modes no one can hear any more.
    fn cull(&mut self) {
        for r in &mut self.res[..self.n] {
            // r^CULL by squaring (CULL = 2^8).
            let mut p = r.r;
            for _ in 0..8 {
                p *= p;
            }
            r.bound *= p;
        }
        while self.n > 0 && self.res[self.n - 1].bound < self.floor {
            self.n -= 1;
        }
    }

    /// Whether anything still sounds (the voice frees itself on its own envelope too).
    pub(crate) fn alive(&self) -> bool {
        self.n > 0 || self.click > 1e-6 || self.hb_env > 1e-6
    }
}
