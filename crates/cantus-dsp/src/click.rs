//! Metronome click: a short, exponentially decaying sine burst. Downbeats are higher.

use crate::math;

#[derive(Clone, Debug)]
pub struct Click {
    sample_rate: f32,
    phase: f32,
    inc: f32,
    env: f32,
    decay: f32,
    gain: f32,
}

impl Click {
    pub fn new(sample_rate: f32) -> Self {
        Click {
            sample_rate,
            phase: 0.0,
            inc: 0.0,
            env: 0.0,
            decay: 0.0,
            gain: 0.5,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain.clamp(0.0, 1.0);
    }

    /// Start a click. `accent` = first beat of the bar.
    pub fn trigger(&mut self, accent: bool) {
        let hz = if accent { 1_760.0 } else { 1_174.7 };
        self.inc = hz / self.sample_rate;
        self.phase = 0.0;
        self.env = if accent { 1.0 } else { 0.7 };
        // ~25 ms to −60 dB.
        self.decay = math::exp(-6.9 / (0.025 * self.sample_rate));
    }

    pub fn is_active(&self) -> bool {
        self.env > 1e-4
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        if self.env <= 1e-4 {
            self.env = 0.0;
            return 0.0;
        }
        let s = math::sin(self.phase * std::f32::consts::TAU) * self.env * self.gain;
        self.phase = (self.phase + self.inc).rem_euclid(1.0);
        self.env *= self.decay;
        s
    }
}
