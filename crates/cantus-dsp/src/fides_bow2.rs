//! Fides's bow, model 2 (round 42, ADR 0150's amendment): the friction junction Arcus plays
//! its strings with (`arcus::string`), in Fides's own units, for a waveguide string drawn by
//! the bow (a tube or a pipe keeps model 1's).
//!
//! Model 1's bow is the STK's table (`exciter::friction`): a smooth curve evaluated once a
//! sample, so the moment the string breaks away falls on a sample and the period locks to
//! whole samples — round 41 measured the bowed pitch +3.1 … −1.2 cents off from C2 to C6, and
//! round 42's model-1 table could not take it under 1 cent (`docs/round42/arcus.md`). Model
//! 2 is the junction of McIntyre, Schumacher & Woodhouse 1983 with the hyperbolic rosin curve
//! (`arcus::string::slip_root`, the Friedlander rule for its hysteresis) and three things
//! Arcus found it needs to keep its pitch:
//!
//! - **sub-sample timing**: the stick's end (where the excess over the static friction
//!   crosses zero) and the slip's end (where the quadratic's discriminant does) placed at
//!   their fraction of the sample, so the Helmholtz corner moves continuously with the loop;
//! - **the bridge's corner**: a one-pole on the bridge's reflection while the bow is drawn
//!   (its delay counted in the loop), which rounds the corner as a real bridge and bow-hair
//!   width do, so the corner's timing lives between the samples;
//! - **the closed loop**: the fundamental heard from the string's own output against the
//!   note's pitch (`arcus::ear`, a heterodyne over whole periods: the phase's slope, the
//!   quantity a tuner and the gate measure), the loop's length trimmed by what is left — the
//!   motion's own pitch, whatever the pressure, position and speed.
//!
//! **Units.** Fides's waves are dimensionless velocities; the bow's velocity is its
//! envelope × `bow_v`. The junction is written scale-free in it: the bow's grip `R` (its
//! force over twice the string's impedance, a velocity) is `place` × Schelleng's upper limit
//! `|v_b| / ((μs − μd)·β)`, and the rosin's `v₀` is `V0_RATIO × |v_b|` — so the motion's shape
//! does not depend on the bow's level (a ramp of the bow is a ramp of the note, as in model 1)
//! and the pressure places the bow in Schelleng's wedge at every position.

use crate::arcus::ear::Ear;
use crate::arcus::string::{slip_root, Curve};
use crate::arcus::tables::{MU_D, MU_S};
use crate::math;

/// The rosin's `v₀` over the bow's speed (Arcus's cello at mf: 0.1 m/s over 0.2).
pub const V0_RATIO: f32 = 0.5;
/// The bridge's corner while the bow is drawn, Hz (Arcus's cello's), and never under this
/// many times the note's pitch (Fides plays to C8: a fixed corner would leave a high note
/// one or two partials and pull it).
pub const CORNER_HZ: f64 = 3000.0;
pub const CORNER_PER_F0: f64 = 6.0;
/// The shortest bow–bridge segment the bow plays at, samples.
pub const MIN_SEGMENT: f64 = 5.0;
/// The share of Schelleng's upper limit the pressure grips with, from pressure 0 to 1
/// (geometric between): inside the Helmholtz region at Fides's positions (0.03 … 0.5).
pub const PLACE: (f32, f32) = (0.3, 0.75);
/// The loop moves by `GAIN` of what each of the ear's windows heard, never more than `STEP`
/// cents a move, and holds within `LIMIT` cents.
const GAIN: f64 = 0.6;
const STEP: f64 = 2.0;
const LIMIT: f64 = 20.0;

#[cfg(test)]
thread_local! {
    /// A probe's scale on the grip (tests only).
    pub static GRIP_SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

/// The grip at a high note: above `TOP_HZ` the Helmholtz region's upper force falls much
/// faster than Schelleng's `v/β` (a period of a few dozen samples rounds the corner the stick
/// must hold through), so the grip is scaled by `(TOP_HZ / f)^TOP_EXP` (never under
/// `TOP_FLOOR`) — measured (`tests::probe_model2_grip`, pressure 0, 0.5 and 1, the worst
/// within a cent): C5 holds its pitch at 0.7 of the grip and not at 1 (−1.0 cent), G5 at
/// 0.25–0.7 (−32 at 1), C6 at 0.18 (−67 at 1), E6 to C7 at 0.12, E7 at 0.06, A♯7 at 0.04 —
/// the law gives 0.53, 0.28, 0.17, 0.12–0.06, 0.04, 0.023.
pub const TOP_HZ: f64 = 350.0;
pub const TOP_EXP: f64 = 1.6;
pub const TOP_FLOOR: f64 = 0.01;

/// The grip's scale at the note's pitch `f0` (Hz): 1 up to `TOP_HZ`.
pub fn pitch_scale(f0: f64) -> f32 {
    if f0 <= TOP_HZ {
        1.0
    } else {
        math::powf(TOP_HZ / f0, TOP_EXP).max(TOP_FLOOR) as f32
    }
}

/// The grip's share of Schelleng's limit at `pressure` (0 … 1).
pub fn place(pressure: f32) -> f32 {
    PLACE.0 * math::powf(PLACE.1 / PLACE.0, pressure.clamp(0.0, 1.0))
}

/// The junction's state, the loop's ear and its trim.
#[derive(Clone, Debug, Default)]
pub struct Bow2 {
    /// `R / |v_b|`: the grip per unit of bow speed (`place / ((μs − μd)·β)`).
    grip: f32,
    sticking: bool,
    excess: f32,
    disc: f32,
    last_slip: f32,
    /// The fundamental heard against the note's pitch.
    pub ear: Ear,
    /// The trim on the loop's pitch, cents (the loop is set this much flat of the note).
    pub trim: f64,
}

impl Bow2 {
    /// A new stroke (the bow placed on a string at rest): the junction, its ear and the trim
    /// start over.
    pub fn reset(&mut self) {
        let mut ear = Ear::default();
        ear.reset();
        *self = Bow2 {
            ear,
            ..Bow2::default()
        };
    }

    /// The step's grip (`place` at the bow's position `beta`).
    pub fn design(&mut self, place: f32, beta: f64) {
        #[cfg(test)]
        let place = place * GRIP_SCALE.with(|g| g.get());
        self.grip = place / ((MU_S - MU_D) * (beta.max(0.01) as f32));
    }

    /// The trim moved by what the ear heard since the last step, and the ear aimed at the
    /// note's pitch `f` (cycles per sample) at the rate `sr`.
    pub fn listen(&mut self, f: f64, sr: f64) {
        if let Some(heard) = self.ear.take() {
            self.trim = (self.trim + (GAIN * heard).clamp(-STEP, STEP)).clamp(-LIMIT, LIMIT);
        }
        self.ear.aim(f, sr);
    }

    /// The junction for one sample: the bow's velocity `vbow` meets the string whose
    /// arriving waves give it `vh` there. Returns the velocity the bow adds to both
    /// outgoing waves.
    #[inline]
    pub fn junction(&mut self, vbow: f32, vh: f32) -> f32 {
        let speed = vbow.abs();
        let curve = Curve {
            v0: (V0_RATIO * speed).max(1e-9),
            ..Curve::default()
        };
        let r = self.grip * speed;
        let u = vbow - vh;
        let au = u.abs();
        let sgn = if u < 0.0 { -1.0 } else { 1.0 };
        let mut slip = 0.0f32;
        let excess = au - r * curve.mu_s;
        let out = if self.sticking && excess <= 0.0 {
            u
        } else {
            match slip_root(&curve, au, r) {
                Some(x) => {
                    slip = sgn * x;
                    if self.sticking && self.excess <= 0.0 && excess > 0.0 {
                        // The stick ended between the samples, at the excess's crossing.
                        let tau = self.excess / (self.excess - excess);
                        u - (1.0 - tau) * slip
                    } else {
                        u - slip
                    }
                }
                None => {
                    // The slip ended between the samples, where the discriminant crossed.
                    let b = curve.v0 + r * curve.mu_d - au;
                    let c = curve.v0 * (r * curve.mu_s - au);
                    let disc = b * b - 4.0 * c;
                    if !self.sticking && self.disc >= 0.0 && disc < 0.0 {
                        let tau = self.disc / (self.disc - disc);
                        u - tau * self.last_slip
                    } else {
                        u
                    }
                }
            }
        };
        self.excess = excess;
        let b = curve.v0 + r * curve.mu_d - au;
        let c = curve.v0 * (r * curve.mu_s - au);
        self.disc = b * b - 4.0 * c;
        self.last_slip = slip;
        self.sticking = slip == 0.0;
        out
    }
}
