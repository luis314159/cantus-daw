//! Analog variance, seeded: slow drift, and voice cards with their calibration errors.
//!
//! **Counter-based.** Every value here is a pure function of a seed and a position — a
//! card number, a moment in time — never of how many notes were played or where playback
//! started. Drift is value noise: a random level per step of a slow clock, hashed from
//! (seed, octave, step), joined by a smooth curve, three octaves summed for a 1/f-like
//! wander. So a render is the same at every block size, the same live and offline, and a
//! note sounds the same whether the song is played from the start or from its bar.
//!
//! **Voice cards** are the circuit boards of a polysynth: each is a little off in its own
//! way — its oscillators' tuning, its filter's cutoff, its envelopes' speed, its pulse
//! width, its glide, its place in the stereo field, how well it tracks the keyboard.
//! [`card_trims`] gives card *n*'s offsets for a seed; a "re-roll" is a new seed.

use super::math::{bipolar, hash3, mix64};

/// Cards a synth has (its hardware voice boards).
pub const CARDS: usize = 16;

/// Drift at `t` seconds for a seed: three octaves of smooth value noise, `rate` Hz the
/// slowest. Roughly −1…1 (its standard deviation is about 0.4).
pub fn drift(seed: u64, t: f64, rate: f32) -> f32 {
    const OCTAVES: [(f64, f32); 3] = [(1.0, 0.6), (2.63, 0.3), (6.91, 0.15)];
    let mut sum = 0.0;
    for (o, (mult, weight)) in OCTAVES.iter().enumerate() {
        let pos = (t * rate as f64 * mult).max(0.0);
        let ki = pos as u64;
        let f = (pos - ki as f64) as f32;
        let a = bipolar(hash3(seed, o as u64, ki));
        let b = bipolar(hash3(seed, o as u64, ki.wrapping_add(1)));
        let s = f * f * (3.0 - 2.0 * f);
        sum += weight * (a + (b - a) * s);
    }
    sum
}

/// How far the cards stray, in real units, before a card's own random draw.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Variance {
    /// Oscillator detune spread, cents.
    pub detune_cents: f32,
    /// Cutoff spread, semitones.
    pub cutoff_st: f32,
    /// Envelope time spread, as a fraction (0.05 = ±5 %).
    pub env: f32,
    /// Pulse-width spread, as a fraction of the cycle.
    pub pw: f32,
    /// Glide-time spread, as a fraction.
    pub glide: f32,
    /// Keyboard-tracking error, cents per octave from middle C.
    pub tracking_cents: f32,
    /// Stereo spread of the cards, 0…1.
    pub pan: f32,
}

impl Variance {
    /// Every spread times `s` (the Condition's scale).
    pub fn scaled(&self, s: f32) -> Variance {
        Variance {
            detune_cents: self.detune_cents * s,
            cutoff_st: self.cutoff_st * s,
            env: self.env * s,
            pw: self.pw * s,
            glide: self.glide * s,
            tracking_cents: self.tracking_cents * s,
            pan: self.pan,
        }
    }
}

/// One card's calibration errors, in real units.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CardTrims {
    /// Each oscillator's tuning offset, cents.
    pub osc_cents: [f32; 3],
    /// Cutoff offset, semitones.
    pub cutoff_st: f32,
    /// Envelope times multiply by this.
    pub env_scale: f32,
    /// Pulse-width offset, fraction of the cycle.
    pub pw: f32,
    /// Glide time multiplies by this.
    pub glide_scale: f32,
    /// Place in the stereo field, −1…1.
    pub pan: f32,
    /// Tracking error, cents per octave from middle C.
    pub tracking_cents: f32,
    /// The seed of this card's drift and noise.
    pub seed: u64,
}

/// Fields a card draws, each from its own hash stream.
const F_OSC: u64 = 1;
const F_CUT: u64 = 4;
const F_ENV: u64 = 5;
const F_PW: u64 = 6;
const F_GLIDE: u64 = 7;
const F_PAN: u64 = 8;
const F_TRACK: u64 = 9;

/// A soft-edged draw: uniform, with its tails pulled in (most cards near the middle, a
/// few far out, as components in tolerance are).
fn draw(h: u64) -> f32 {
    let u = bipolar(h);
    u * (0.55 + 0.45 * u * u)
}

/// Card `card`'s trims for a seed and a per-card re-roll (`roll`: 0 is the card as the
/// seed made it; any other number re-draws it alone).
pub fn card_trims(seed: u32, card: usize, roll: u32, v: &Variance) -> CardTrims {
    let base = mix64(((seed as u64) << 32) ^ ((roll as u64) << 8) ^ card as u64);
    let d = |field: u64| draw(hash3(base, field, 0));
    let osc = |i: u64| d(F_OSC + i) * v.detune_cents;
    // Pan: an even spread across the cards (left, right, alternating out), plus a little.
    let slot = card as f32 / (CARDS - 1) as f32;
    let spread = if card.is_multiple_of(2) {
        slot - 1.0
    } else {
        slot
    } * v.pan;
    CardTrims {
        osc_cents: [osc(0), osc(1), osc(2)],
        cutoff_st: d(F_CUT) * v.cutoff_st,
        env_scale: 1.0 + d(F_ENV) * v.env,
        pw: d(F_PW) * v.pw,
        glide_scale: 1.0 + d(F_GLIDE) * v.glide,
        pan: (spread + 0.1 * v.pan * d(F_PAN)).clamp(-1.0, 1.0),
        tracking_cents: d(F_TRACK) * v.tracking_cents,
        seed: base,
    }
}

/// The phase a note's oscillator starts at, from the note's id and the oscillator: the
/// look of free-running oscillators, with no memory of the notes before.
pub fn start_phase(note_id: u64, osc: u64) -> f64 {
    (hash3(note_id, 0x05c1, osc) >> 11) as f64 / (1u64 << 53) as f64
}
