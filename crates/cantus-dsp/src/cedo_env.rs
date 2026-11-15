//! Cedo's sidechain detector: the key through its filter, a threshold, and an envelope that
//! attacks, holds and releases in exact sample counts.
//!
//! - **The key filter** is Comprimo's key path shared: Aequalis's [`design`] (a 24 dB/oct
//!   high-pass, or a band-pass with its Q) run through [`comprimo::key_band`] — the same
//!   `f64` sections, the same flush of a band that has died away.
//! - **The threshold** is on the filtered key's sample peak, per channel. A sample at or
//!   above it **triggers**; the **hold** counts from the *last* such sample, so a kick's
//!   low half-cycles that dip under the threshold do not let go as long as the hold covers
//!   them.
//! - **The envelope** is the duck's *amount*, 0 (none) … 1 (the full depth). It rises along
//!   an S (`3p² − 2p³`, no corner at either end) and reaches the full depth exactly `attack`
//!   samples after the trigger (0 is at once), stays while triggered or held, then falls
//!   over exactly `release` samples along the release shape — **linear**, **exponential**
//!   (`(1 − e^{−5p})/(1 − e^{−5})`: quick, then easing in, landing on 0 at the release's end)
//!   or an **S-curve** (`3p² − 2p³`). A trigger while it falls attacks again from where it is.
//!
//! Every count is per sample, so nothing depends on the blocks, and the envelope comes back
//! to exactly 0 (a new detector's state).

use crate::aequalis::{design, Coefs, EqShape, MAX_SECTIONS};
use crate::comprimo::key_band;
use crate::math;

/// How the duck lets go.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ReleaseShape {
    Linear,
    #[default]
    Exponential,
    SCurve,
}

impl ReleaseShape {
    pub fn from_index(i: usize) -> ReleaseShape {
        match i {
            0 => ReleaseShape::Linear,
            2 => ReleaseShape::SCurve,
            _ => ReleaseShape::Exponential,
        }
    }

    /// How far the release has come at progress `p` (0 … 1): 0 at 0, exactly 1 at 1.
    #[inline]
    pub fn at(self, p: f64) -> f64 {
        let p = p.clamp(0.0, 1.0);
        match self {
            ReleaseShape::Linear => p,
            ReleaseShape::Exponential => {
                if p >= 1.0 {
                    1.0
                } else {
                    (1.0 - math::exp(-EXP_K * p)) / (1.0 - math::exp(-EXP_K))
                }
            }
            ReleaseShape::SCurve => p * p * (3.0 - 2.0 * p),
        }
    }
}

/// The exponential release's steepness: e⁻⁵, the last 0.7 % of the way taken at the end.
const EXP_K: f64 = 5.0;

/// What filters the key.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum KeyFilterMode {
    Off,
    /// A 24 dB/oct high-pass: the key without its rumble (or a kick's key without its sub).
    #[default]
    HighPass,
    /// A band-pass: one instrument out of a mix (a kick's 60 Hz, a voice's 1–3 kHz).
    BandPass,
}

impl KeyFilterMode {
    pub fn from_index(i: usize) -> KeyFilterMode {
        match i {
            0 => KeyFilterMode::Off,
            2 => KeyFilterMode::BandPass,
            _ => KeyFilterMode::HighPass,
        }
    }
}

/// The key filter, both channels.
#[derive(Clone, Debug)]
pub struct KeyFilter {
    n: usize,
    coefs: [Coefs; MAX_SECTIONS],
    z: [[[f64; 2]; MAX_SECTIONS]; 2],
}

impl Default for KeyFilter {
    fn default() -> Self {
        KeyFilter {
            n: 0,
            coefs: [Coefs::IDENTITY; MAX_SECTIONS],
            z: [[[0.0; 2]; MAX_SECTIONS]; 2],
        }
    }
}

impl KeyFilter {
    /// New settings; the states are kept.
    pub fn set(&mut self, mode: KeyFilterMode, hz: f32, q: f32, sample_rate: f32) {
        let hz = if hz.is_finite() { hz } else { 100.0 };
        self.n = match mode {
            KeyFilterMode::Off => 0,
            KeyFilterMode::HighPass => design(
                EqShape::LowCut,
                sample_rate,
                hz.clamp(20.0, 15_000.0),
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
                24.0,
                &mut self.coefs,
            ),
            KeyFilterMode::BandPass => design(
                EqShape::BandPass,
                sample_rate,
                hz.clamp(20.0, 15_000.0),
                0.0,
                q.clamp(0.3, 12.0),
                12.0,
                &mut self.coefs,
            ),
        };
        if self.n == 0 {
            self.z = [[[0.0; 2]; MAX_SECTIONS]; 2];
        }
    }

    #[inline]
    pub fn tick(&mut self, k: [f32; 2]) -> [f32; 2] {
        if self.n == 0 {
            return k;
        }
        let mut out = k;
        for (ch, o) in out.iter_mut().enumerate() {
            *o = key_band(&self.coefs[..self.n], &mut self.z[ch], k[ch] as f64) as f32;
        }
        out
    }

    pub fn is_clear(&self) -> bool {
        self.z.iter().flatten().flatten().all(|v| v.to_bits() == 0)
    }

    pub fn reset(&mut self) {
        self.z = [[[0.0; 2]; MAX_SECTIONS]; 2];
    }
}

/// The envelope's timing, in samples.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Timing {
    pub attack: u32,
    pub hold: u32,
    pub release: u32,
    pub shape: ReleaseShape,
}

impl Timing {
    pub fn new(attack_ms: f32, hold_ms: f32, release_ms: f32, shape: ReleaseShape, sr: f32) -> Timing {
        let n = |ms: f32, hi: f32| (ms.clamp(0.0, hi) as f64 * 0.001 * sr as f64).round() as u32;
        Timing {
            attack: n(attack_ms, MAX_ATTACK_MS),
            hold: n(hold_ms, MAX_HOLD_MS),
            release: n(release_ms, MAX_RELEASE_MS).max(1),
            shape,
        }
    }
}

pub const MAX_ATTACK_MS: f32 = 500.0;
pub const MAX_HOLD_MS: f32 = 2_000.0;
pub const MAX_RELEASE_MS: f32 = 5_000.0;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
enum Stage {
    #[default]
    Idle,
    Attack,
    Full,
    Release,
}

/// One channel's envelope.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Envelope {
    a: f32,
    stage: Stage,
    hold_left: u32,
    from: f32,
    i: u32,
}

impl Envelope {
    /// One sample: `above` whether the key reached the threshold. The amount after it.
    #[inline]
    pub fn tick(&mut self, above: bool, t: &Timing) -> f32 {
        let on = if above {
            self.hold_left = t.hold;
            true
        } else if self.hold_left > 0 {
            self.hold_left -= 1;
            true
        } else {
            false
        };
        if on {
            if matches!(self.stage, Stage::Idle | Stage::Release) {
                self.stage = Stage::Attack;
                self.from = self.a;
                self.i = 0;
            }
            if self.stage == Stage::Attack {
                // From where it was to 1 along an S (`3p² − 2p³`, no corner at either end,
                // so the attack splatters least), exactly 1 after `attack` samples
                // (counted, not accumulated).
                self.i += 1;
                let a = if t.attack == 0 || self.i >= t.attack {
                    1.0
                } else {
                    let p = self.i as f64 / t.attack as f64;
                    let from = self.from as f64;
                    from + (1.0 - from) * p * p * (3.0 - 2.0 * p)
                };
                if a >= 1.0 {
                    self.a = 1.0;
                    self.stage = Stage::Full;
                } else {
                    self.a = a as f32;
                }
            }
        } else if self.stage != Stage::Idle {
            if self.stage != Stage::Release {
                self.stage = Stage::Release;
                self.from = self.a;
                self.i = 0;
            }
            self.i += 1;
            if self.i >= t.release {
                *self = Envelope::default();
            } else {
                let p = self.i as f64 / t.release as f64;
                self.a = (self.from as f64 * (1.0 - t.shape.at(p))) as f32;
            }
        }
        self.a
    }

    pub fn amount(&self) -> f32 {
        self.a
    }

    /// A new envelope's state.
    pub fn is_idle(&self) -> bool {
        *self == Envelope::default()
    }
}
