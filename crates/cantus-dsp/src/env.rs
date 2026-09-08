//! ADSR envelope with analog-style exponential segments.
//!
//! Each segment is a one-pole approach toward a target that lies *past* the real end value
//! (the "overshoot" trick), which gives the familiar fast-then-slow curve and makes the
//! attack actually reach 1.0 in the requested time.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Debug)]
pub struct Adsr {
    sample_rate: f32,
    stage: Stage,
    output: f32,
    attack_coef: f32,
    attack_base: f32,
    decay_coef: f32,
    decay_base: f32,
    release_coef: f32,
    release_base: f32,
    sustain: f32,
}

const ATTACK_RATIO: f32 = 0.3;
const DECAY_RATIO: f32 = 0.0001;

impl Adsr {
    pub fn new(sample_rate: f32) -> Self {
        let mut e = Adsr {
            sample_rate,
            stage: Stage::Idle,
            output: 0.0,
            attack_coef: 0.0,
            attack_base: 0.0,
            decay_coef: 0.0,
            decay_base: 0.0,
            release_coef: 0.0,
            release_base: 0.0,
            sustain: 1.0,
        };
        e.set(0.01, 0.1, 0.8, 0.2);
        e
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    /// Times in seconds, sustain 0..=1.
    pub fn set(&mut self, attack_s: f32, decay_s: f32, sustain: f32, release_s: f32) {
        self.sustain = sustain.clamp(0.0, 1.0);
        let (c, b) = coef(attack_s, self.sample_rate, ATTACK_RATIO, 1.0 + ATTACK_RATIO);
        self.attack_coef = c;
        self.attack_base = b;
        let (c, b) = coef(
            decay_s,
            self.sample_rate,
            DECAY_RATIO,
            self.sustain - DECAY_RATIO,
        );
        self.decay_coef = c;
        self.decay_base = b;
        let (c, b) = coef(release_s, self.sample_rate, DECAY_RATIO, -DECAY_RATIO);
        self.release_coef = c;
        self.release_base = b;
    }

    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
    }

    pub fn gate_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    pub fn reset(&mut self) {
        self.stage = Stage::Idle;
        self.output = 0.0;
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn is_active(&self) -> bool {
        self.stage != Stage::Idle
    }

    pub fn is_releasing(&self) -> bool {
        self.stage == Stage::Release
    }

    pub fn value(&self) -> f32 {
        self.output
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.output = self.attack_base + self.output * self.attack_coef;
                if self.output >= 1.0 {
                    self.output = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.output = self.decay_base + self.output * self.decay_coef;
                if self.output <= self.sustain {
                    self.output = self.sustain;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {
                self.output = self.sustain;
            }
            Stage::Release => {
                self.output = self.release_base + self.output * self.release_coef;
                if self.output <= 0.0 {
                    self.output = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.output
    }
}

/// One-pole coefficient/base for a segment lasting `seconds` that approaches `target`
/// with the given overshoot ratio.
fn coef(seconds: f32, sample_rate: f32, ratio: f32, target: f32) -> (f32, f32) {
    let samples = (seconds.max(0.0) * sample_rate).max(1.0);
    let c = (-((1.0 + ratio) / ratio).ln() / samples).exp();
    let base = target * (1.0 - c);
    (c, base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaches_one_then_sustain_then_zero() {
        let sr = 48_000.0;
        let mut e = Adsr::new(sr);
        e.set(0.01, 0.05, 0.5, 0.1);
        e.gate_on();
        let mut peak = 0f32;
        let mut reached_at = None;
        for i in 0..48_000 {
            let v = e.next();
            peak = peak.max(v);
            if reached_at.is_none() && e.stage() == Stage::Decay {
                reached_at = Some(i);
            }
        }
        assert_eq!(peak, 1.0);
        let at = reached_at.unwrap();
        assert!((at as f32 - 480.0).abs() < 60.0, "attack took {at} samples");
        assert_eq!(e.stage(), Stage::Sustain);
        assert!((e.value() - 0.5).abs() < 1e-5);
        e.gate_off();
        let mut n = 0;
        while e.is_active() {
            e.next();
            n += 1;
            assert!(n < 48_000);
        }
        assert!(
            (n as f32 - 4_800.0).abs() < 600.0,
            "release took {n} samples"
        );
        assert_eq!(e.value(), 0.0);
    }

    #[test]
    fn zero_length_segments_do_not_hang() {
        let mut e = Adsr::new(48_000.0);
        e.set(0.0, 0.0, 1.0, 0.0);
        e.gate_on();
        e.next();
        e.next();
        assert!(matches!(e.stage(), Stage::Sustain | Stage::Decay));
        e.gate_off();
        for _ in 0..10 {
            e.next();
        }
        assert_eq!(e.stage(), Stage::Idle);
    }
}
