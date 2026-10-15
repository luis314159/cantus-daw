//! The envelope follower (ADR 0056): a level that rises at one speed and falls at
//! another, the piece every dynamics unit is built on — standalone, so anything can take it
//! (Impetus and Discrimen here; a modulator that follows a strip's level).
//!
//! - [`Times`] — an attack and a release as one-pole coefficients. A time is the
//!   **time constant**: after `attack` ms of a step the envelope has covered 1 − 1/e
//!   (63 %) of the way; 0 ms follows at once.
//! - [`Follower`] — one envelope over a level that is already rectified (`|x|`, or `x²`
//!   for a mean square). One branch per sample: up at the attack's speed, down at the
//!   release's.
//! - [`EnvelopeFollower`] — the standalone unit: peak or RMS detection of a sample or a
//!   stereo pair (the louder side), times in milliseconds, the level in dB.
//!
//! **Homogeneous**: the envelope of `a·x` is `a` times the envelope of `x`, to the bit when
//! `a` is a power of two (each step is a subtraction, a product and a sum, all scaled
//! exactly) — so a detector that compares two envelopes reads the same at any level.
//!
//! **Rest is exact** (ADR 0031 §5.1): after [`SETTLE_RUN`] silent samples in a row, an
//! envelope that has fallen under [`SETTLE`] (−120 dB) is set to +0.0 — per sample, so
//! where it happens does not depend on the blocks. Until then it is not at rest; after it,
//! it is exactly a new follower.

/// An envelope under this, after a run of silence, is flushed to +0.0 (−120 dBFS).
pub const SETTLE: f32 = 1e-6;
/// Silent samples in a row before a small envelope is flushed.
pub const SETTLE_RUN: u32 = 64;

/// The one-pole coefficient of a time constant of `ms` at `sample_rate`: 1 for 0 ms (the
/// envelope jumps).
pub fn coef(ms: f32, sample_rate: f32) -> f32 {
    let n = ms * 0.001 * sample_rate;
    if n <= 0.0 {
        return 1.0;
    }
    1.0 - crate::math::exp(-1.0 / n.max(1e-3))
}

/// An attack and a release, as coefficients.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Times {
    pub attack: f32,
    pub release: f32,
}

impl Times {
    pub fn new(attack_ms: f32, release_ms: f32, sample_rate: f32) -> Times {
        Times {
            attack: coef(attack_ms, sample_rate),
            release: coef(release_ms, sample_rate),
        }
    }
}

/// One envelope over a rectified level.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Follower {
    env: f32,
}

impl Follower {
    /// One sample of `level` (≥ 0): the envelope after it.
    #[inline]
    pub fn run(&mut self, level: f32, t: &Times) -> f32 {
        let k = if level > self.env {
            t.attack
        } else {
            t.release
        };
        if k >= 1.0 {
            self.env = level;
        } else {
            self.env += (level - self.env) * k;
        }
        self.env
    }

    pub fn value(&self) -> f32 {
        self.env
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
    }

    /// The envelope is +0.0, as a new one.
    pub fn is_zero(&self) -> bool {
        self.env.to_bits() == 0
    }

    /// Flush an envelope under `below` to +0.0.
    #[inline]
    pub fn settle(&mut self, below: f32) {
        if self.env < below {
            self.env = 0.0;
        }
    }
}

/// How a level is read: the peak (`|x|`) or the mean square (`x²`, the root taken on the
/// way out).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Detect {
    #[default]
    Peak,
    Rms,
}

/// Counts silent samples and says when small states may be flushed (per sample, so the
/// moment does not depend on the blocks).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Quiet {
    run: u32,
}

impl Quiet {
    /// One sample whose level is `level`: true when the silence has lasted long enough to
    /// flush.
    #[inline]
    pub fn note(&mut self, level: f32) -> bool {
        if level.to_bits() == 0 {
            self.run = self.run.saturating_add(1);
        } else {
            self.run = 0;
        }
        self.run >= SETTLE_RUN
    }

    /// `frames` silent samples skipped.
    pub fn skip(&mut self, frames: usize) {
        self.run = self.run.saturating_add(frames.min(u32::MAX as usize) as u32);
    }

    pub fn reset(&mut self) {
        self.run = 0;
    }
}

/// The standalone follower: a detection, two times in milliseconds, one envelope.
#[derive(Clone, Debug)]
pub struct EnvelopeFollower {
    sr: f32,
    detect: Detect,
    times: Times,
    f: Follower,
    quiet: Quiet,
}

impl EnvelopeFollower {
    /// Peak detection, 1 ms attack, 100 ms release.
    pub fn new(sample_rate: f32) -> EnvelopeFollower {
        EnvelopeFollower {
            sr: sample_rate,
            detect: Detect::Peak,
            times: Times::new(1.0, 100.0, sample_rate),
            f: Follower::default(),
            quiet: Quiet::default(),
        }
    }

    /// New times and detection; the envelope carries on from where it is.
    pub fn set(&mut self, detect: Detect, attack_ms: f32, release_ms: f32) {
        self.detect = detect;
        self.times = Times::new(attack_ms, release_ms, self.sr);
    }

    /// One mono sample: the envelope, linear.
    #[inline]
    pub fn run(&mut self, x: f32) -> f32 {
        let level = match self.detect {
            Detect::Peak => x.abs(),
            Detect::Rms => x * x,
        };
        self.run_level(level)
    }

    /// One stereo frame: the louder side's level.
    #[inline]
    pub fn run_pair(&mut self, l: f32, r: f32) -> f32 {
        let level = match self.detect {
            Detect::Peak => l.abs().max(r.abs()),
            Detect::Rms => (l * l).max(r * r),
        };
        self.run_level(level)
    }

    #[inline]
    fn run_level(&mut self, level: f32) -> f32 {
        self.f.run(level, &self.times);
        if self.quiet.note(level) {
            self.f.settle(self.settle_below());
        }
        self.value()
    }

    /// The flush threshold in the follower's own domain (a square for RMS).
    fn settle_below(&self) -> f32 {
        match self.detect {
            Detect::Peak => SETTLE,
            Detect::Rms => SETTLE * SETTLE,
        }
    }

    /// The envelope now, linear.
    pub fn value(&self) -> f32 {
        match self.detect {
            Detect::Peak => self.f.value(),
            Detect::Rms => self.f.value().sqrt(),
        }
    }

    /// The envelope now in dBFS (−inf as −200).
    pub fn db(&self) -> f32 {
        to_db(self.value())
    }

    pub fn reset(&mut self) {
        self.f.reset();
        self.quiet.reset();
    }

    /// Exactly as new.
    pub fn at_rest(&self) -> bool {
        self.f.is_zero()
    }

    /// `frames` of silence skipped at rest.
    pub fn skip(&mut self, frames: usize) {
        self.quiet.skip(frames);
    }
}

/// A linear level in dB, −200 for silence.
#[inline]
pub fn to_db(x: f32) -> f32 {
    if x <= 1e-10 {
        -200.0
    } else {
        20.0 * crate::math::log10(x)
    }
}

#[cfg(test)]
#[path = "dynfx_follow_tests.rs"]
mod tests;
