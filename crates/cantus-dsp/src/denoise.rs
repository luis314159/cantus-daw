//! Noise reduction by profile and the spectral editor's region tool, for the audio editor
//! (ADR 0096). Offline, deterministic, never on the audio thread.
//!
//! **Noise reduction.** The user selects a stretch of nothing-but-noise; [`NoiseProfile::learn`]
//! keeps its mean power per bin, per channel. [`denoise`] then runs the STFT of
//! [`crate::denoise::stft`] over the take and, in each frame and bin, subtracts that power
//! (times `sensitivity`) from what is there: the gain is the square root of what is left
//! over what was there, never below the `reduction_db` floor. The gains are smoothed over
//! three neighbouring bins and, falling, over time (they rise at once, so a syllable's
//! start is never dulled), which is what keeps the "musical noise" of plain subtraction
//! down. `amount` mixes the gain with 1: **at 0 the take is returned bit for bit** (the
//! function does not touch it), at 1 the whole reduction.
//!
//! **A region.** [`attenuate_region`] scales a time-frequency rectangle of the spectrogram
//! by a gain in dB, with soft edges (one hop in time, three bins in frequency), and at
//! 0 dB returns the take untouched.

#[path = "denoise_declick.rs"]
pub mod declick;
#[path = "denoise_stft.rs"]
pub mod stft;

use crate::math;
use stft::{Stft, BINS, HOP, N};

/// Frames a profile needs at least: a stretch shorter than this says too little about the
/// noise (about 0.04 s at 48 kHz).
pub const MIN_PROFILE_FRAMES: usize = 4;

/// Why a reduction could not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DenoiseError {
    /// The learned stretch is shorter than [`MIN_PROFILE_FRAMES`] frames of the transform.
    ProfileTooShort,
    /// The profile was learned from another number of channels or another rate.
    ProfileMismatch,
    /// A parameter outside its range.
    Invalid(&'static str),
}

impl std::fmt::Display for DenoiseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DenoiseError::ProfileTooShort => f.write_str("the noise sample is too short"),
            DenoiseError::ProfileMismatch => {
                f.write_str("the noise profile is from another channel count or rate")
            }
            DenoiseError::Invalid(what) => write!(f, "invalid {what}"),
        }
    }
}

/// What noise sounds like: its mean power per bin, per channel.
#[derive(Clone, Debug, PartialEq)]
pub struct NoiseProfile {
    pub rate: u32,
    /// `power[channel][bin]`, [`BINS`] bins each.
    pub power: Vec<Vec<f32>>,
    /// The transform frames it was learned from.
    pub frames: usize,
}

impl NoiseProfile {
    /// Learn from the noise-only stretch of each channel (all the same length).
    pub fn learn(channels: &[Vec<f32>], rate: u32) -> Result<NoiseProfile, DenoiseError> {
        if channels.is_empty() {
            return Err(DenoiseError::Invalid("channels"));
        }
        let mut st = Stft::new();
        let mut power = Vec::new();
        let mut frames = usize::MAX;
        for c in channels {
            let (p, n) = st.mean_power(c);
            if n < MIN_PROFILE_FRAMES {
                return Err(DenoiseError::ProfileTooShort);
            }
            frames = frames.min(n);
            power.push(p);
        }
        Ok(NoiseProfile {
            rate,
            power,
            frames,
        })
    }
}

/// How the reduction behaves.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Params {
    /// 0 (nothing: the take comes back bit for bit) to 1 (all of it).
    pub amount: f32,
    /// The most any bin is turned down, dB (positive), at `amount` 1.
    pub reduction_db: f32,
    /// How many times the learned noise is subtracted (1 = its mean; more catches the
    /// noise's peaks, and takes a little more of the wanted sound).
    pub sensitivity: f32,
    /// 0..1: how much the gains are smoothed.
    pub smoothing: f32,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            amount: 1.0,
            reduction_db: 24.0,
            sensitivity: 3.0,
            smoothing: 0.5,
        }
    }
}

impl Params {
    pub fn check(&self) -> Result<(), DenoiseError> {
        let ok = |v: f32, lo: f32, hi: f32| v.is_finite() && v >= lo && v <= hi;
        if !ok(self.amount, 0.0, 1.0) {
            return Err(DenoiseError::Invalid("amount"));
        }
        if !ok(self.reduction_db, 0.0, 80.0) {
            return Err(DenoiseError::Invalid("reduction_db"));
        }
        if !ok(self.sensitivity, 0.5, 8.0) {
            return Err(DenoiseError::Invalid("sensitivity"));
        }
        if !ok(self.smoothing, 0.0, 1.0) {
            return Err(DenoiseError::Invalid("smoothing"));
        }
        Ok(())
    }
}

/// Reduce the noise of one channel of `x` against `noise` (one channel of a profile).
pub fn denoise_channel(x: &[f32], noise: &[f32], p: &Params) -> Vec<f32> {
    if p.amount <= 0.0 {
        return x.to_vec();
    }
    let floor = math::db_to_gain(-p.reduction_db);
    let fall = 0.6 * p.smoothing;
    let mut prev = vec![1.0f32; BINS];
    let mut raw = vec![1.0f32; BINS];
    let mut st = Stft::new();
    st.run(x, |_, re, im| {
        for b in 0..BINS {
            let power = re[b] * re[b] + im[b] * im[b];
            let g = if power > 1e-20 {
                ((power - p.sensitivity * noise[b]).max(0.0) / power).sqrt()
            } else {
                1.0
            };
            raw[b] = g.max(floor);
        }
        // Over frequency: three bins, as far as smoothing asks.
        let side = 0.25 * p.smoothing;
        for b in 0..BINS {
            let lo = raw[b.saturating_sub(1)];
            let hi = raw[(b + 1).min(BINS - 1)];
            let g = (1.0 - 2.0 * side) * raw[b] + side * (lo + hi);
            // Over time: up at once, down slowly.
            let g = if g < prev[b] {
                prev[b] + (g - prev[b]) * (1.0 - fall)
            } else {
                g
            };
            prev[b] = g;
            let eff = 1.0 + p.amount * (g - 1.0);
            re[b] *= eff;
            im[b] *= eff;
        }
    })
}

/// Reduce the noise of every channel (`channels[c]` against `profile.power[c]`).
pub fn denoise(
    channels: &[Vec<f32>],
    profile: &NoiseProfile,
    p: &Params,
) -> Result<Vec<Vec<f32>>, DenoiseError> {
    p.check()?;
    if profile.power.len() != channels.len() {
        return Err(DenoiseError::ProfileMismatch);
    }
    Ok(channels
        .iter()
        .zip(&profile.power)
        .map(|(x, n)| denoise_channel(x, n, p))
        .collect())
}

/// A rectangle of the spectrogram: samples `start..end` and frequencies `lo_hz..hi_hz`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Region {
    pub start: usize,
    pub end: usize,
    pub lo_hz: f32,
    pub hi_hz: f32,
}

/// Scale `region` of one channel by `gain_db` (negative to attenuate), soft at its edges.
pub fn attenuate_region(x: &[f32], rate: u32, region: &Region, gain_db: f32) -> Vec<f32> {
    if gain_db == 0.0 || region.end <= region.start {
        return x.to_vec();
    }
    let g = math::db_to_gain(gain_db);
    let bin_hz = rate as f32 / N as f32;
    let lo = region.lo_hz / bin_hz;
    let hi = region.hi_hz / bin_hz;
    // The frequency weight of a bin: 1 inside, a raised cosine over 1.5 bins outside.
    let wf = |b: usize| -> f32 {
        let b = b as f32;
        let d = if b < lo {
            lo - b
        } else if b > hi {
            b - hi
        } else {
            0.0
        };
        ramp(d / 1.5)
    };
    // The time weight of a frame centred at `c`: 1 inside, a raised cosine over one hop.
    let wt = |c: f32| -> f32 {
        let (s, e) = (region.start as f32, region.end as f32);
        let d = if c < s {
            s - c
        } else if c > e {
            c - e
        } else {
            0.0
        };
        ramp(d / HOP as f32)
    };
    let weights: Vec<f32> = (0..BINS).map(wf).collect();
    Stft::new().run(x, |k, re, im| {
        let centre = (Stft::frame_start(k) + (N / 2) as isize) as f32;
        let t = wt(centre);
        if t <= 0.0 {
            return;
        }
        for b in 0..BINS {
            let w = t * weights[b];
            if w > 0.0 {
                let eff = 1.0 + w * (g - 1.0);
                re[b] *= eff;
                im[b] *= eff;
            }
        }
    })
}

/// 1 at 0, 0 at 1 and beyond: a half raised cosine.
fn ramp(x: f32) -> f32 {
    if x >= 1.0 {
        0.0
    } else if x <= 0.0 {
        1.0
    } else {
        0.5 + 0.5 * math::cos(std::f32::consts::PI * x)
    }
}

#[cfg(test)]
#[path = "denoise_tests.rs"]
mod tests;
