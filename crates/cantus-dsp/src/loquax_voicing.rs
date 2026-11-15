//! Loquax's ear for consonants: a voiced/unvoiced detector on the modulator, the
//! sibilance it lets through, and the gate that turns the carrier toward noise.
//!
//! A vocoder's carrier is pitched; a voice's *s*, *f*, *sh* and *t* are noise. Vocoded
//! through a chord they come out as a faint whistle, and the words blur. The classic
//! machines answered in two ways, and Loquax has both:
//!
//! - **Sibilance pass-through** (Sennheiser VSM-201, Roland SVC-350): the modulator itself,
//!   high-passed (fourth order, Butterworth), added to the output — gated by the detector,
//!   so a voiced vowel's own top end stays out, or always on.
//! - **Unvoiced noise** (EMS Vocoder 2000's voiced/unvoiced switch): while the detector
//!   hears a consonant, noise is mixed into the carrier, so the bank shapes the consonant
//!   from noise as the mouth did.
//!
//! **The detector** compares the modulator's energy above 3.5 kHz with its energy below
//! 1.2 kHz (two-pole state-variable filters, one-pole envelopes on the squares, 1 ms up and
//! 15 ms down): a vowel's energy is low (its first formants), a fricative's high. The
//! unvoiced measure `u = hf / (hf + lf)` opens the gate over 0.35 … 0.65, and only above
//! −55 dBFS (silence and breath stay shut). The gate glides open in 1 ms and closes in
//! 25 ms, landing on +0.0 exactly.

use crate::dynfx_follow::{Follower, Times};
use crate::loquax::bank::{Svf, SvfCoef};

/// The gate's measure, where it starts and ends opening.
pub const U_OPEN_FROM: f32 = 0.35;
pub const U_OPEN_TO: f32 = 0.65;
/// The modulator's level under which the gate stays shut (energy, −55 dBFS squared).
const FLOOR: f32 = 3.162e-6;
const SNAP: f32 = 1e-6;

/// The detector and the sibilance high-pass.
#[derive(Clone, Debug)]
pub struct Voicing {
    sr: f32,
    hp: Svf,
    lp: Svf,
    hp_c: SvfCoef,
    lp_c: SvfCoef,
    hf: Follower,
    lf: Follower,
    t_det: Times,
    gate: Follower,
    t_gate: Times,
    /// The sibilance high-pass: two Butterworth sections.
    sib: [Svf; 2],
    sib_c: [SvfCoef; 2],
    sib_hz: f32,
    u: f32,
}

impl Voicing {
    pub fn new(sr: f32) -> Voicing {
        let mut v = Voicing {
            sr,
            hp: Svf::default(),
            lp: Svf::default(),
            hp_c: SvfCoef::new(3_500.0, core::f32::consts::FRAC_1_SQRT_2, sr),
            lp_c: SvfCoef::new(1_200.0, core::f32::consts::FRAC_1_SQRT_2, sr),
            hf: Follower::default(),
            lf: Follower::default(),
            t_det: Times::new(1.0, 15.0, sr),
            gate: Follower::default(),
            t_gate: Times::new(1.0, 25.0, sr),
            sib: [Svf::default(); 2],
            sib_c: [SvfCoef::default(); 2],
            sib_hz: 0.0,
            u: 0.0,
        };
        v.set_sibilance_hz(5_000.0);
        v
    }

    /// The sibilance high-pass's corner, Hz (2 … 12 kHz).
    pub fn set_sibilance_hz(&mut self, hz: f32) {
        let hz = hz.clamp(2_000.0, 12_000.0).min(0.45 * self.sr);
        if hz == self.sib_hz {
            return;
        }
        self.sib_hz = hz;
        // Fourth-order Butterworth: the sections' Q are 1/(2 sin(π/8)) and 1/(2 sin(3π/8)).
        self.sib_c = [
            SvfCoef::new(hz, 1.306_563, self.sr),
            SvfCoef::new(hz, 0.541_196_1, self.sr),
        ];
    }

    /// One modulator sample: (the gate 0 … 1, the high-passed modulator).
    #[inline]
    pub fn run(&mut self, x: f32) -> (f32, f32) {
        let (_, _, h) = self.hp.run(x, &self.hp_c);
        let (l, _, _) = self.lp.run(x, &self.lp_c);
        let (eh, el) = (h * h, l * l);
        let hf = self.hf.run(eh, &self.t_det);
        let lf = self.lf.run(el, &self.t_det);
        if eh.to_bits() == 0 {
            self.hf.settle(1e-14);
        }
        if el.to_bits() == 0 {
            self.lf.settle(1e-14);
        }
        let total = hf + lf;
        self.u = if total > 1e-20 { hf / total } else { 0.0 };
        let open = if total > FLOOR {
            ((self.u - U_OPEN_FROM) / (U_OPEN_TO - U_OPEN_FROM)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.gate.run(open, &self.t_gate);
        if open == 0.0 {
            self.gate.settle(SNAP);
        }
        let s0 = self.sib[0].run(x, &self.sib_c[0]).2;
        let s1 = self.sib[1].run(s0, &self.sib_c[1]).2;
        (self.gate.value(), s1)
    }

    /// The unvoiced measure now (0 … 1).
    pub fn measure(&self) -> f32 {
        self.u
    }

    pub fn gate(&self) -> f32 {
        self.gate.value()
    }

    pub fn reset(&mut self) {
        self.hp.reset();
        self.lp.reset();
        self.hf.reset();
        self.lf.reset();
        self.gate.reset();
        for s in &mut self.sib {
            s.reset();
        }
        self.u = 0.0;
    }

    /// Every state +0.0: a new detector.
    pub fn at_rest(&self) -> bool {
        self.hp.at_rest()
            && self.lp.at_rest()
            && self.hf.is_zero()
            && self.lf.is_zero()
            && self.gate.is_zero()
            && self.sib.iter().all(Svf::at_rest)
            && self.u.to_bits() == 0
    }
}
