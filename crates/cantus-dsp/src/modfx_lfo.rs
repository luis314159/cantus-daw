//! The modulation family's LFO (ADR 0055): a phase that is a **function of a clock**, never
//! a running sum, so it is the same at every block size, offline and live, and a unit that
//! was skipped through silence ([`crate::modfx_clock::Clock::skip`]) is where a unit that
//! processed the silence would be.
//!
//! - **Synced**: the phase is `song sample × cycles per sample + offset`, the song sample
//!   at the current tempo — a pure function of the song position (beats ÷ beats per cycle).
//!   A tempo change keeps it so: the clock re-anchors at the new tempo's sample count.
//! - **Free**: the phase runs from the start of playback at the rate in Hz. A rate change
//!   re-anchors it where it is, so the wave never jumps; the start of playback starts it
//!   again from the offset.
//!
//! The shapes are bipolar (−1 … 1) and start at their middle rising (a sine's start), so a
//! phase offset means the same for all of them. The square, the saws and the random steps
//! have a **soft edge** (`edge`, 0 … 1 of a half cycle's ramp), so a tremolo or a sweep on
//! them never clicks.

use crate::math;
use crate::modfx_clock::{random_bipolar, synced_inc, Clock};

/// The LFO's waves (`modfx.shape.{i}` in the locale files).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Sine,
    Triangle,
    Square,
    SawUp,
    SawDown,
    /// A new value each cycle (a hash of the seed and the cycle's number), stepped with the
    /// soft edge between.
    Random,
}

impl Shape {
    pub const ALL: [Shape; 6] = [
        Shape::Sine,
        Shape::Triangle,
        Shape::Square,
        Shape::SawUp,
        Shape::SawDown,
        Shape::Random,
    ];

    pub fn from_index(i: f32) -> Shape {
        let i = if i.is_finite() { i.round() as i64 } else { 0 };
        Shape::ALL[i.clamp(0, Shape::ALL.len() as i64 - 1) as usize]
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// Whether the soft edge changes it.
    pub fn has_edge(self) -> bool {
        matches!(
            self,
            Shape::Square | Shape::SawUp | Shape::SawDown | Shape::Random
        )
    }
}

/// The shortest soft edge, of a cycle: even at 0 a stepped wave takes this long to turn.
const MIN_EDGE: f64 = 0.002;

/// A smooth step from 0 to 1 over `x` in 0 … 1 (a raised cosine's half).
#[inline]
fn smooth_step(x: f64) -> f32 {
    let x = x.clamp(0.0, 1.0) as f32;
    0.5 - 0.5 * math::cos(std::f32::consts::PI * x)
}

/// The value of `shape` at `phase` (0 … 1) of cycle number `cycle`, `edge` the soft edge
/// (0 … 1), `seed` the random wave's seed. Bipolar, −1 … 1.
pub fn wave(shape: Shape, phase: f64, cycle: i64, edge: f32, seed: u64) -> f32 {
    let p = phase - phase.floor();
    // The ramp of a stepped wave, as a share of the cycle.
    let e = (f64::from(edge.clamp(0.0, 1.0)) * 0.5).max(MIN_EDGE);
    match shape {
        Shape::Sine => math::sin(std::f32::consts::TAU * p as f32),
        Shape::Triangle => {
            let x = p as f32;
            if x < 0.25 {
                4.0 * x
            } else if x < 0.75 {
                2.0 - 4.0 * x
            } else {
                4.0 * x - 4.0
            }
        }
        Shape::Square => {
            // Rises around 0 (and 1), falls around ½; each turn takes `e` of a cycle.
            let half = e * 0.5;
            if p < half {
                2.0 * smooth_step(0.5 + p / e) - 1.0
            } else if p > 1.0 - half {
                2.0 * smooth_step((p - (1.0 - half)) / e) - 1.0
            } else if (p - 0.5).abs() < half {
                1.0 - 2.0 * smooth_step((p - (0.5 - half)) / e)
            } else if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Shape::SawUp | Shape::SawDown => {
            // A ramp from −1 to 1 starting at 0 (the middle), falling back within `e`
            // around ½ of the cycle.
            let up = |p: f64| -> f64 {
                let q = (p + (1.0 - e) * 0.5).fract();
                if q < 1.0 - e {
                    -1.0 + 2.0 * q / (1.0 - e)
                } else {
                    1.0 - 2.0 * f64::from(smooth_step((q - (1.0 - e)) / e))
                }
            };
            let v = up(p) as f32;
            if shape == Shape::SawUp {
                v
            } else {
                -v
            }
        }
        Shape::Random => {
            let now = random_bipolar(seed, cycle);
            if p >= e {
                now
            } else {
                let before = random_bipolar(seed, cycle - 1);
                before + (now - before) * smooth_step(p / e)
            }
        }
    }
}

/// How an LFO keeps time.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Rate {
    /// Cycles per second, from the start of playback.
    Hz(f64),
    /// Beats per cycle, on the song position.
    Beats(f64),
}

/// An LFO's settings.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LfoSettings {
    pub rate: Rate,
    pub shape: Shape,
    /// Where in its cycle it starts, 0 … 1.
    pub offset: f64,
    /// The soft edge of the stepped waves, 0 … 1.
    pub edge: f32,
    /// Swing on a synced rate: 0 straight … 1 every second cycle starts as late as
    /// three quarters of the pair (MPC's 75 %).
    pub swing: f32,
    pub seed: u64,
}

impl Default for LfoSettings {
    fn default() -> Self {
        LfoSettings {
            rate: Rate::Hz(1.0),
            shape: Shape::Sine,
            offset: 0.0,
            edge: 0.0,
            swing: 0.0,
            seed: 0,
        }
    }
}

/// An LFO: its settings and, running free, the anchor a rate change leaves.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Lfo {
    set: LfoSettings,
    sample_rate: f64,
    /// Free running: the phase `anchor_p` at free sample `anchor_t`, and cycles per sample.
    anchor_t: i64,
    anchor_p: f64,
    inc: f64,
    /// The clock's play epoch the anchor belongs to.
    epoch: u32,
}

impl Lfo {
    pub fn new(sample_rate: f32) -> Lfo {
        Lfo {
            set: LfoSettings::default(),
            sample_rate: f64::from(sample_rate),
            anchor_t: 0,
            anchor_p: 0.0,
            inc: 1.0 / f64::from(sample_rate),
            epoch: 0,
        }
    }

    /// Back to the start of a stream.
    pub fn reset(&mut self) {
        self.anchor_t = 0;
        self.anchor_p = 0.0;
        self.epoch = 0;
        if let Rate::Hz(hz) = self.set.rate {
            self.inc = hz / self.sample_rate;
        }
    }

    pub fn settings(&self) -> &LfoSettings {
        &self.set
    }

    /// New settings, `clock` as it is now (before the next call). A free rate that changes
    /// re-anchors where the wave is, so it never jumps.
    pub fn set(&mut self, s: &LfoSettings, clock: &Clock) {
        self.follow(clock);
        if let Rate::Hz(hz) = s.rate {
            let inc = hz.max(0.0) / self.sample_rate;
            if inc != self.inc {
                let t = clock.free(0);
                self.anchor_p = self.free_cycles(t);
                self.anchor_p -= self.anchor_p.floor();
                self.anchor_t = t;
                self.inc = inc;
            }
        }
        self.set = *s;
    }

    /// Take a new start of playback: a free LFO begins its cycle again.
    #[inline]
    pub fn follow(&mut self, clock: &Clock) {
        if clock.epoch() != self.epoch {
            self.epoch = clock.epoch();
            self.anchor_t = 0;
            self.anchor_p = 0.0;
        }
    }

    #[inline]
    fn free_cycles(&self, t: i64) -> f64 {
        self.anchor_p + (t - self.anchor_t) as f64 * self.inc
    }

    /// The LFO's position in cycles (unbounded, before the offset and the swing) `i`
    /// samples into the next call.
    #[inline]
    pub fn cycles(&self, clock: &Clock, i: usize) -> f64 {
        match self.set.rate {
            Rate::Hz(_) => self.free_cycles(clock.free(i)),
            Rate::Beats(b) => clock.song(i) as f64 * synced_inc(b, clock.tempo(), self.sample_rate),
        }
    }

    /// The phase (0 … 1) and the cycle's number `i` samples into the next call, with the
    /// offset and (synced) the swing in it.
    #[inline]
    pub fn phase(&self, clock: &Clock, i: usize) -> (f64, i64) {
        self.phase_at(self.cycles(clock, i))
    }

    /// The phase and cycle of a position in cycles.
    #[inline]
    pub fn phase_at(&self, cycles: f64) -> (f64, i64) {
        let q = cycles + self.set.offset;
        let swing = f64::from(self.set.swing.clamp(0.0, 1.0)) * 0.5;
        if swing > 0.0 && matches!(self.set.rate, Rate::Beats(_)) {
            // Pairs of cycles: the first takes 1 + s of the pair's two, the second 1 − s.
            let pair = (q * 0.5).floor();
            let v = q - 2.0 * pair;
            let (p, second) = if v < 1.0 + swing {
                (v / (1.0 + swing), 0)
            } else {
                ((v - 1.0 - swing) / (1.0 - swing), 1)
            };
            (p, pair as i64 * 2 + second)
        } else {
            let c = q.floor();
            (q - c, c as i64)
        }
    }

    /// The wave `i` samples into the next call, −1 … 1.
    #[inline]
    pub fn value(&self, clock: &Clock, i: usize) -> f32 {
        let (p, c) = self.phase(clock, i);
        wave(self.set.shape, p, c, self.set.edge, self.set.seed)
    }

    /// The wave at a phase offset by `delta` cycles (a second channel's LFO).
    #[inline]
    pub fn value_offset(&self, clock: &Clock, i: usize, delta: f64) -> f32 {
        let (p, c) = self.phase_at(self.cycles(clock, i) + delta);
        wave(self.set.shape, p, c, self.set.edge, self.set.seed)
    }
}

#[cfg(test)]
#[path = "modfx_lfo_tests.rs"]
mod tests;
