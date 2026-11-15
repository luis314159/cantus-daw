//! Model 4, **acoustic**: the struck drum and cymbal Tympanum's modelled kits play (round
//! 40 phase 3, ADR 0124 §2). Pulsus's own kits never use it, so no Pulsus song changes.
//!
//! What it adds over the modal voices of model 3, each measured against recorded kits
//! (`tympanum_measure`):
//!
//! - **The force of the strike drives the modes** (a half-sine of the contact time) rather
//!   than setting them ringing at once: the attack has its real length, and a harder hit —
//!   a shorter contact — is brighter, not only louder (velocity → timbre).
//! - **The stick or the beater is heard**: a short band of its own noise and crack.
//! - **Tension modulation**: a hard hit stretches the head, its modes start sharp and fall
//!   as the amplitude falls (a tom's and a kick's glide).
//! - **Two heads**: the lowest modes come in pairs, a few cents apart, that beat.
//! - **Snare wires** buzz when the snare-side head moves them — noise gated by the head's
//!   motion, compressive (a ghost note is mostly wires), and a sizzle that follows.
//! - **Cymbals** (`metal`): a dense, inharmonic low bank struck by the stick, and a high
//!   bank the low one drives (its square and the noise of its energy) — the wash builds
//!   after the hit and shimmers, as energy cascades upwards in a real cymbal.
//! - **Micro-variation**: every hit moves the strike point, the contact, the tuning a
//!   little (seeded per hit: the same song renders the same bits).
//!
//! Every resonator is one two-pole section; the modes a hit no longer hears are dropped as
//! they decay, so a long tail costs what is still audible.

use super::shape::{decay_coef, Band, Noise};
use crate::math;
use std::f32::consts::TAU;

#[path = "pulsus_acoustic_metal.rs"]
pub(crate) mod metal;

/// Modes at most per bank.
pub(crate) const MAX_RES: usize = 64;

/// The first 64 modes of an ideal circular membrane, `(m, j_mn)` in order (the zeros to
/// five decimals, as computed; f32 keeps what it can).
#[allow(clippy::excessive_precision)]
pub(crate) const MEMBRANE64: [(u8, f32); MAX_RES] = [
    (0, 2.40483),
    (1, 3.83171),
    (2, 5.13562),
    (0, 5.52008),
    (3, 6.38016),
    (1, 7.01559),
    (4, 7.58834),
    (2, 8.41724),
    (0, 8.65373),
    (5, 8.77148),
    (3, 9.76102),
    (6, 9.93611),
    (1, 10.17347),
    (4, 11.06471),
    (7, 11.08637),
    (2, 11.61984),
    (0, 11.79153),
    (8, 12.22509),
    (5, 12.33860),
    (3, 13.01520),
    (1, 13.32369),
    (9, 13.35430),
    (6, 13.58929),
    (4, 14.37254),
    (10, 14.47550),
    (2, 14.79595),
    (7, 14.82127),
    (0, 14.93092),
    (11, 15.58985),
    (5, 15.70017),
    (8, 16.03777),
    (3, 16.22347),
    (1, 16.47063),
    (12, 16.69825),
    (6, 17.00382),
    (9, 17.24122),
    (4, 17.61597),
    (13, 17.80144),
    (2, 17.95982),
    (0, 18.07106),
    (7, 18.28758),
    (10, 18.43346),
    (14, 18.90000),
    (5, 18.98013),
    (3, 19.40942),
    (8, 19.55454),
    (1, 19.61586),
    (11, 19.61597),
    (15, 19.99443),
    (6, 20.32079),
    (12, 20.78991),
    (9, 20.80705),
    (4, 20.82693),
    (16, 21.08515),
    (2, 21.11700),
    (0, 21.21164),
    (7, 21.64154),
    (13, 21.95624),
    (10, 22.04699),
    (17, 22.17249),
    (5, 22.21780),
    (3, 22.58273),
    (1, 22.76008),
    (8, 22.94517),
];

/// One mode: a two-pole resonator driven by a force.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct Res {
    pub y1: f32,
    pub y2: f32,
    pub a1: f32,
    pub r2: f32,
    pub r: f32,
    /// The force's gain (amplitude × sin θ) and the mode's frequency, Hz.
    pub b: f32,
    pub hz: f32,
    /// An upper bound of the mode's amplitude, falling by `r` a sample: below the floor
    /// the mode is dropped.
    pub bound: f32,
}

impl Res {
    /// A mode the strike's **acceleration** drives (the force's second difference): the
    /// head's slow push, which moves air hardly at all, is not heard as a thump; at its
    /// own frequency the mode answers as [`Self::set`]'s would to the force.
    #[inline]
    pub fn set_struck(&mut self, hz: f32, t60: f32, amp: f32, sr: f32) {
        self.set(hz, t60, amp, sr);
        let theta = TAU * hz / sr;
        self.b /= (2.0 - 2.0 * math::cos(theta)).max(1e-9);
    }

    #[inline]
    pub fn set(&mut self, hz: f32, t60: f32, amp: f32, sr: f32) {
        let r = math::exp(-6.907_755 / (t60.max(0.002) * sr));
        let theta = TAU * hz / sr;
        self.r = r;
        self.r2 = r * r;
        self.a1 = 2.0 * r * math::cos(theta);
        self.b = amp * math::sin(theta);
        self.hz = hz;
        self.y1 = 0.0;
        self.y2 = 0.0;
        self.bound = amp.abs();
    }

    /// Retune by `ratio` keeping the state (continuous).
    #[inline]
    pub fn retune(&mut self, ratio: f32, k: f32) {
        self.a1 = 2.0 * self.r * math::cos((k * self.hz * ratio).min(3.0));
    }

    #[inline]
    pub fn tick(&mut self, force: f32) -> f32 {
        let y = self.a1 * self.y1 - self.r2 * self.y2 + self.b * force;
        self.y2 = self.y1;
        self.y1 = if y.abs() < 1e-20 { 0.0 } else { y };
        y
    }
}

/// Bessel function of the first kind by its power series (x ≤ ~25).
pub(crate) fn bessel_j(m: u32, x: f32) -> f32 {
    let half = 0.5 * x as f64;
    let mut term = 1.0f64;
    for i in 1..=m {
        term *= half / i as f64;
    }
    let mut sum = term;
    for k in 1..90u32 {
        term *= -half * half / (k as f64 * (k + m) as f64);
        sum += term;
        if term.abs() < 1e-12 && k > m + 4 {
            break;
        }
    }
    sum as f32
}

/// What a membrane hit is made of (from the pad, the velocity and the hit's variation).
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct DrumHit {
    pub f0: f32,
    pub decay: f32,
    pub damping: f32,
    pub position: f32,
    /// 0 soft … 1 hard, already with the velocity.
    pub hardness: f32,
    /// The velocity, 0 … 1 (linear).
    pub vel: f32,
    /// Semitones the head starts above its pitch at full force.
    pub glide: f32,
    /// The second head's coupling, 0 … 1.
    pub heads: f32,
    pub modes: usize,
    /// The stick's (or beater's) noise: level and band centre, Hz.
    pub click: f32,
    pub click_hz: f32,
    /// A felt beater (the kick): a longer contact, a duller knock.
    pub beater: bool,
    /// Snare wires' level and their buzz's decay, seconds.
    pub wires: f32,
    pub wire_decay: f32,
    /// The shell's ring (level).
    pub shell: f32,
    /// A rimshot: the rim's metal rings too.
    pub rim: f32,
}

/// A struck membrane drum.
#[derive(Clone, Debug)]
pub(crate) struct Drum {
    res: [Res; MAX_RES],
    n: usize,
    sr: f32,
    /// The force pulse: samples left, its length, its scale; its last two samples.
    f1: f32,
    f2: f32,
    t: u32,
    contact: u32,
    force_k: f32,
    /// Tension modulation: the ratio's excess now and its fall per retune.
    glide: f32,
    glide_coef: f32,
    /// The stick's noise.
    click: f32,
    click_coef: f32,
    click_band: Band,
    click_crack: Band,
    /// The low modes' sum, the wires' drive.
    low: f32,
    wires: f32,
    wire_follow: f32,
    wire_follow_coef: f32,
    wire_env: f32,
    wire_coef: f32,
    wire_hp: Band,
    wire_bp: Band,
    wire_lp: f32,
    wire_lp_a: f32,
    wire_scale: f32,
    /// The head's dense high modes, as two bands of noise that ring and fall away.
    hb1: Band,
    hb2: Band,
    hb_env: f32,
    hb_coef: f32,
    /// The rim's ring.
    rim: [Res; 4],
    rim_on: bool,
    pub noise: Noise,
    floor: f32,
}

impl Drum {
    pub(crate) fn new(sr: f32) -> Drum {
        Drum {
            res: [Res::default(); MAX_RES],
            n: 0,
            sr,
            f1: 0.0,
            f2: 0.0,
            t: 0,
            contact: 1,
            force_k: 0.0,
            glide: 0.0,
            glide_coef: 1.0,
            click: 0.0,
            click_coef: 0.0,
            click_band: Band::default(),
            click_crack: Band::default(),
            low: 0.0,
            wires: 0.0,
            wire_follow: 0.0,
            wire_follow_coef: 0.0,
            wire_env: 0.0,
            wire_coef: 0.0,
            wire_hp: Band::default(),
            wire_bp: Band::default(),
            wire_lp: 0.0,
            wire_lp_a: 1.0,
            wire_scale: 1.0,
            hb1: Band::default(),
            hb2: Band::default(),
            hb_env: 0.0,
            hb_coef: 0.0,
            rim: [Res::default(); 4],
            rim_on: false,
            noise: Noise(1),
            floor: 1e-6,
        }
    }
}

#[path = "pulsus_acoustic_drum.rs"]
mod drum;

/// Model 4's state in a voice: a drum or a cymbal.
#[derive(Clone, Debug)]
pub(crate) struct Acoustic {
    pub drum: Drum,
    pub metal: metal::Metal,
    is_metal: bool,
    sizzle: f32,
}

impl Acoustic {
    pub(crate) fn new(sr: f32) -> Acoustic {
        Acoustic {
            drum: Drum::new(sr),
            metal: metal::Metal::new(sr),
            is_metal: false,
            sizzle: 0.0,
        }
    }

    #[inline]
    pub(crate) fn tick(&mut self) -> f32 {
        if self.is_metal {
            self.metal.tick()
        } else {
            self.drum.tick()
        }
    }

    pub(crate) fn alive(&self) -> bool {
        if self.is_metal {
            self.metal.alive()
        } else {
            self.drum.alive()
        }
    }

    pub(crate) fn set_openness(&mut self, openness: f32) {
        if self.is_metal {
            self.metal.set_openness(openness, self.sizzle);
        }
    }
}

/// A modelled drum's Tune 0, Hz (as model 3's toms), and a cymbal's.
const DRUM_BASE_HZ: f32 = 220.0;
const METAL_BASE_HZ: f32 = 330.0;

impl super::voice::Voice {
    /// Start a model 4 voice. A kit that never enabled the acoustic models has no state
    /// for them: the hit is silent rather than wrong.
    pub(crate) fn start_acoustic(
        &mut self,
        s: &super::PadSettings,
        c: &super::start::HitCtx,
        tune: f32,
        decay: f32,
        _sr: f32,
    ) {
        use super::DrumVoice;
        let Some(ac) = self.ac.as_deref_mut() else {
            self.active = false;
            return;
        };
        self.uses_fade = true;
        let vel = c.velocity.clamp(1, 127) as f32 / 127.0;
        // The hit's own variation: where the stick lands, how hard, the head's tuning.
        let mut v = Noise::from_seed(c.seed ^ 0x7A3F_19C4_D2E1_0B55);
        let cents = 3.0 * v.next();
        match s.voice {
            DrumVoice::HiHat => {
                ac.is_metal = true;
                ac.sizzle = s.band_mix.clamp(0.0, 1.0);
                ac.metal.noise = Noise::from_seed(c.seed);
                let chick = c.note_id == u64::MAX - 1;
                ac.metal.strike(&metal::MetalHit {
                    f0: super::shape::semitones(METAL_BASE_HZ, tune),
                    // A hat's plates ring as long as open; its pedal (the voice's
                    // envelope) closes them.
                    decay: if s.pedal {
                        2.0 * s.open_decay.max(decay)
                    } else {
                        decay
                    },
                    hardness: (s.hardness * (0.6 + 0.4 * vel) + 0.06 * v.next()).clamp(0.0, 1.0),
                    vel,
                    position: (s.position + 0.66 * s.scatter * v.next()).clamp(0.0, 1.0),
                    tone: s.tone,
                    swell: s.swell,
                    click: s.click,
                    hat: s.pedal,
                    openness: c.openness,
                    sizzle: ac.sizzle,
                    chick,
                    identity: (s.id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xC1A5,
                });
                // A hat's master envelope is its pedal: closed or open, it follows it.
                if !s.pedal {
                    self.amp_coef = 1.0;
                }
            }
            _ => {
                ac.is_metal = false;
                ac.drum.noise = Noise::from_seed(c.seed);
                let hit = DrumHit {
                    f0: super::shape::semitones(DRUM_BASE_HZ, tune + cents / 100.0),
                    decay,
                    damping: s.damping,
                    position: (s.position + s.scatter * v.next()).clamp(0.0, 1.0),
                    hardness: (s.hardness * (0.55 + 0.45 * vel) + 0.05 * v.next()).clamp(0.0, 1.0),
                    vel,
                    glide: s.glide,
                    heads: s.shell,
                    modes: s.modes.clamp(4, MAX_RES as u8) as usize,
                    click: s.click,
                    click_hz: s.tone,
                    beater: s.voice == DrumVoice::Kick,
                    wires: if s.voice == DrumVoice::Snare {
                        s.wires
                    } else {
                        0.0
                    },
                    wire_decay: s.noise_decay,
                    shell: s.mix.max(0.0),
                    rim: s.band_mix.max(0.0),
                };
                ac.drum.strike(&hit);
                // The drum decays by its modes; the voice frees itself when they are gone.
                self.amp_coef = 1.0;
            }
        }
    }
}

#[cfg(test)]
#[path = "pulsus_acoustic_tests.rs"]
mod tests;
