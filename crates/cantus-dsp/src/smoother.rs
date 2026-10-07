//! One-pole parameter smoother: removes zipper noise when a value jumps between blocks.

use crate::math;

#[derive(Clone, Debug)]
pub struct Smoother {
    current: f32,
    target: f32,
    coef: f32,
}

impl Smoother {
    /// `time_s` is the time to cover ~99% of a step.
    pub fn new(initial: f32, time_s: f32, sample_rate: f32) -> Self {
        let mut s = Smoother {
            current: initial,
            target: initial,
            coef: 0.0,
        };
        s.set_time(time_s, sample_rate);
        s
    }

    pub fn set_time(&mut self, time_s: f32, sample_rate: f32) {
        let samples = (time_s * sample_rate).max(1.0);
        // e^(-4.6/n) reaches 1% remaining after n samples.
        self.coef = math::exp(-4.6 / samples);
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Jump immediately (e.g. on voice start) so a new note does not glide.
    pub fn snap(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        self.current = self.target + (self.current - self.target) * self.coef;
        // Snap once within -80 dB of the target: f32 cannot creep closer than a few ulps
        // anyway, and the residual is inaudible.
        if (self.current - self.target).abs() < 1e-4 * self.target.abs() + 1e-9 {
            self.current = self.target;
        }
        self.current
    }

    pub fn value(&self) -> f32 {
        self.current
    }

    pub fn is_settled(&self) -> bool {
        self.current == self.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_in_about_the_requested_time() {
        let mut s = Smoother::new(0.0, 0.01, 48_000.0);
        s.set_target(1.0);
        for _ in 0..480 {
            s.next();
        }
        assert!(s.value() > 0.98 && s.value() < 1.0);
        for _ in 0..4_800 {
            s.next();
        }
        assert!(s.is_settled());
    }
}
