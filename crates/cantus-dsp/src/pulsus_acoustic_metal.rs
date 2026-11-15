//! The cymbal and the hi-hat: a dense, inharmonic low bank the stick strikes, and a high
//! bank the low one drives — its square (the plate's stretching, which moves energy to
//! sums and differences of its modes) and noise at the level of its energy — so the
//! wash **builds after the hit** and shimmers, as in a real cymbal. A hi-hat is the same
//! pair of plates damped by how closed it is, with the plates' rattle half open.

use super::super::shape::{decay_coef, Band, Noise};
use super::Res;
use crate::math;
use std::f32::consts::PI;

/// Modes of the low and the high bank.
pub(crate) const LOW_MODES: usize = 28;
pub(crate) const HIGH_MODES: usize = 52;

/// What a cymbal hit is made of.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct MetalHit {
    /// The lowest mode, Hz; the cymbal's size sets it.
    pub f0: f32,
    /// The low bank's decay (T60, s) and the high bank's.
    pub decay: f32,
    /// 0 … 1: soft mallet-like … a hard tip.
    pub hardness: f32,
    pub vel: f32,
    /// 0 the bell … 0.5 the bow … 1 the edge.
    pub position: f32,
    /// The wash's centre, Hz, and how long it takes to build, s.
    pub tone: f32,
    pub swell: f32,
    /// The stick's tick.
    pub click: f32,
    /// A hi-hat: how open (0 … 1), and the plates' rattle half open.
    pub hat: bool,
    pub openness: f32,
    pub sizzle: f32,
    /// The foot closing the plates (no stick).
    pub chick: bool,
    /// The cymbal's own seed: its modes are the same cymbal every hit.
    pub identity: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct Metal {
    low: [Res; LOW_MODES],
    high: [Res; HIGH_MODES],
    n_low: usize,
    n_high: usize,
    sr: f32,
    t: u32,
    contact: u32,
    force_k: f32,
    f1: f32,
    f2: f32,
    /// The stick's strike on the high modes (the half-sine's scale).
    ping: f32,
    /// The drive from low to high: the square's and the noise's.
    k_sq: f32,
    k_noise: f32,
    energy: f32,
    energy_coef: f32,
    hp: Band,
    tick_env: f32,
    tick_coef: f32,
    tick_band: Band,
    /// Half open, the plates touch: a slow random gate.
    rattle: f32,
    rattle_lp: Band,
    pub noise: Noise,
    out_hp: Band,
}

impl Metal {
    pub(crate) fn new(sr: f32) -> Metal {
        Metal {
            low: [Res::default(); LOW_MODES],
            high: [Res::default(); HIGH_MODES],
            n_low: 0,
            n_high: 0,
            sr,
            t: 0,
            contact: 1,
            force_k: 0.0,
            f1: 0.0,
            f2: 0.0,
            ping: 0.0,
            k_sq: 0.0,
            k_noise: 0.0,
            energy: 0.0,
            energy_coef: 0.0,
            hp: Band::default(),
            tick_env: 0.0,
            tick_coef: 0.0,
            tick_band: Band::default(),
            rattle: 0.0,
            rattle_lp: Band::default(),
            noise: Noise(1),
            out_hp: Band::default(),
        }
    }

    /// Strike the cymbal.
    pub(crate) fn strike(&mut self, h: &MetalHit) {
        let sr = self.sr;
        let top = (sr * 0.45).min(18_000.0);
        // The cymbal's identity: its modes, the same every hit.
        let mut id = Noise((h.identity as u32 ^ (h.identity >> 32) as u32) | 1);
        let hard = h.hardness.clamp(0.0, 1.0);
        let vel = h.vel.clamp(0.0, 1.0);
        let pos = h.position.clamp(0.0, 1.0);
        let contact_s = if h.chick {
            0.004
        } else {
            0.0012 * math::powf(0.25, hard * (0.5 + 0.5 * vel))
        };
        self.contact = ((contact_s * sr) as u32).max(2);
        self.force_k = PI / (2.0 * self.contact as f32);
        self.t = 0;
        self.f1 = 0.0;
        self.f2 = 0.0;
        // Low bank: from f0 up three and a half octaves, log-spaced with a jitter.
        let f0 = h.f0.max(60.0);
        let mut total = 0.0;
        let mut n = 0;
        for k in 0..LOW_MODES {
            let x = (k as f32 + 0.5 * id.unit()) / LOW_MODES as f32;
            let hz = f0 * math::exp2(3.6 * x);
            if hz >= top {
                break;
            }
            // The bell rings its lowest few long; the edge sets the whole plate going.
            let shape = if pos < 0.3 {
                let bell = 1.0 - pos / 0.3;
                1.0 + 2.5 * bell * math::exp(-(k as f32) / 3.0)
            } else {
                1.0 + 0.6 * (pos - 0.5).max(0.0) * math::exp(-(k as f32) / 8.0)
            };
            let amp = shape * (0.5 + id.unit()) / (1.0 + 0.15 * k as f32);
            let t60 = h.decay * math::powf(f0 / hz, 0.25) * (0.8 + 0.4 * id.unit());
            self.low[n].set_struck(hz, t60, amp, sr);
            total += amp;
            n += 1;
        }
        self.n_low = n;
        // An open hat rings long and loud: as loud as DRSKit's against its closed one.
        let open_trim = if h.hat {
            1.0 - 0.4 * h.openness.clamp(0.0, 1.0)
        } else {
            1.0
        };
        let g = if total > 0.0 { open_trim / total } else { 0.0 };
        for r in &mut self.low[..n] {
            r.b *= g;
        }
        // High bank: 2.5 … 17 kHz, denser; struck a little by the stick, driven by the low.
        let mut m = 0;
        let lo = (f0 * 5.0).max(1_800.0);
        let centre = h.tone.clamp(2_000.0, 14_000.0);
        for k in 0..HIGH_MODES {
            let x = (k as f32 + id.unit()) / HIGH_MODES as f32;
            let hz = lo * math::exp2(3.2 * x);
            if hz >= top {
                break;
            }
            let t60 = h.decay * 0.6 * math::powf(lo / hz, 0.6) * (0.7 + 0.6 * id.unit());
            // The wash's colour: most around the cymbal's `tone`, an octave either side.
            let o = math::log2(hz / centre);
            let amp = (0.4 + id.unit()) * math::exp(-0.5 * o * o);
            self.high[m].set(hz, t60, amp * g * 0.25 * (0.3 + 0.7 * hard), sr);
            m += 1;
        }
        self.n_high = m;
        // How strongly the plate stretches: with the force (a hard crash washes most).
        let swell = h.swell.clamp(0.005, 1.0);
        self.k_sq = 2.5 * (0.35 + 0.65 * vel) * (0.4 + 0.6 * hard) / swell.sqrt();
        self.k_noise = 0.05 * (0.3 + vel);
        self.energy = 0.0;
        self.energy_coef = decay_coef(0.02, sr);
        self.hp.reset();
        self.hp.set(h.tone.clamp(1_500.0, 14_000.0) * 0.35, 0.7, sr);
        // The stick's tick (none for the foot's chick: the plates clap).
        self.tick_env = if h.chick {
            0.0
        } else {
            h.click * (0.4 + 0.6 * vel)
        };
        self.ping = if h.chick {
            0.0
        } else {
            25.0 * h.click * (0.3 + 0.7 * hard)
        };
        self.tick_coef = decay_coef(0.0008 + 0.0012 * (1.0 - hard), sr);
        self.tick_band.reset();
        self.tick_band.set(5_500.0 + 3_000.0 * hard, 0.8, sr);
        self.rattle = if h.hat {
            h.sizzle * hat_rattle(h.openness)
        } else {
            0.0
        };
        self.rattle_lp.reset();
        self.rattle_lp.set(45.0, 0.7, sr);
        self.out_hp.reset();
        self.out_hp
            .set(Self::low_cut(h.hat, h.openness, h.chick), 0.6, sr);
    }

    /// The pedal moved a ringing hat: how much the plates rattle now, and how low they
    /// sound.
    pub(crate) fn set_openness(&mut self, openness: f32, sizzle: f32) {
        self.rattle = sizzle * hat_rattle(openness);
        self.out_hp
            .set(Self::low_cut(true, openness, false), 0.6, self.sr);
    }

    /// Where the plate's sound starts: a cymbal's lows, a closed hat's highs only — the
    /// two plates pressed together cannot swing slowly — opening down to the open hat's.
    fn low_cut(hat: bool, openness: f32, chick: bool) -> f32 {
        if !hat {
            160.0
        } else if chick {
            320.0
        } else {
            2_400.0 * math::powf(450.0 / 2_400.0, openness.clamp(0.0, 1.0))
        }
    }

    /// Whether the plate still sounds (its energy over −100 dB).
    pub(crate) fn alive(&self) -> bool {
        self.t < self.contact + 4_800 || self.energy > 1e-10
    }

    #[inline]
    pub(crate) fn tick(&mut self) -> f32 {
        let f = if self.t < self.contact {
            math::sin(PI * self.t as f32 / self.contact as f32) * self.force_k
        } else {
            0.0
        };
        let force = f - 2.0 * self.f1 + self.f2;
        self.f2 = self.f1;
        self.f1 = f;
        self.t = self.t.wrapping_add(1);
        let mut low = 0.0;
        for r in &mut self.low[..self.n_low] {
            low += r.tick(force);
        }
        // The energy of the low bank, and the drive it gives the high one.
        self.energy = (low * low).max(self.energy * self.energy_coef);
        let w = self.noise.next();
        // The square spreads the energy to sums and differences; divided by the plate's
        // amplitude it stays as loud as the plate (a louder hit is brighter by `k_sq`,
        // not by a square law that would run away).
        let amp = self.energy.sqrt() + 1e-9;
        let sq = self.hp.tick(low * low / amp).2;
        let drive = self.k_sq * sq + self.k_noise * w * amp * 8.0;
        let mut high = 0.0;
        // The stick's ping: where the tip lands it sets the high modes ringing at once
        // (a ride is its ping first, its wash under it).
        let ping = self.ping * f;
        for r in &mut self.high[..self.n_high] {
            high += r.tick(drive + ping);
        }
        let mut out = low + high;
        if self.tick_env > 1e-6 {
            out += self.tick_band.tick(w).1 * self.tick_env;
            self.tick_env *= self.tick_coef;
        }
        if self.rattle > 0.0 {
            // A slow random gate: the plates meeting and parting.
            let g = self.rattle_lp.tick(self.noise.next()).0 * 18.0;
            let gate = g.clamp(0.0, 1.0);
            out *= 1.0 - self.rattle * 0.6 + self.rattle * 0.6 * gate;
            out += self.rattle * gate * w * self.energy.sqrt() * 0.6;
        }
        self.out_hp.tick(out).2
    }
}

/// How much the plates rattle at an openness: none closed or wide open, most half open.
fn hat_rattle(openness: f32) -> f32 {
    let o = openness.clamp(0.0, 1.0);
    (4.0 * o * (1.0 - o)).clamp(0.0, 1.0)
}
