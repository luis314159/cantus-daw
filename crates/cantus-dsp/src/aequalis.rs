//! Aequalis: the parametric equaliser. Up to [`MAX_BANDS`] bands, nine shapes, cuts from
//! 6 to 96 dB/octave, per-band stereo placement (stereo, left, right, mid, side),
//! **dynamic** bands whose gain follows a band-limited detector (above or below a threshold,
//! from the band's own signal or from a sidechain key), three **processing modes**, an
//! audition (solo) of one band, and a measured auto gain.
//!
//! ## Filters that match the analog curve up to Nyquist
//!
//! The bilinear transform (the RBJ cookbook) squeezes the whole analog frequency axis into
//! the digital one, so a bell at 16 kHz on a 48 kHz project comes out narrower and a high
//! shelf does not reach its gain — "cramping". Every section here is designed by
//! **magnitude matching** instead (after Vicanek, *Matched Second Order Digital Filters*,
//! 2016): the poles are the analog poles mapped exactly (`z = e^{sT}`), and the zeros are
//! solved so that the digital magnitude equals the analog one at DC, at Nyquist and at the
//! band's own frequency. A test holds every shape within a fraction of a dB of its analog
//! prototype all the way up.
//!
//! The derivation the solver rests on is short enough to keep beside it. For a quadratic
//! `B(z) = b0 + b1 z⁻¹ + b2 z⁻²`, with `φ1 = sin²(ω/2)`, `φ0 = 1 − φ1`, `φ2 = 4 φ0 φ1`:
//!
//! ```text
//! |B(e^{jω})|² = (b0+b1+b2)² φ0 + (b0−b1+b2)² φ1 − 4 b0 b2 φ2
//! ```
//!
//! so three magnitudes fix `B0 = (b0+b1+b2)²`, `B1 = (b0−b1+b2)²` and `B2 = −4 b0 b2`, and
//! the coefficients follow in closed form (the larger root, so the zeros are minimum phase).
//!
//! ## Three processing modes ([`Mode`])
//!
//! - **Zero latency**: the matched cascade. Analog magnitude; minimum phase.
//! - **Natural phase**: the cascade plus a 128-tap correction, 64 samples late, that makes
//!   magnitude *and* phase follow the analog prototype up to Nyquist (`phase`).
//! - **Linear phase**: an FIR from the analog magnitudes, zero phase, run by partitioned
//!   convolution. Four resolutions; the latency is stated exactly by [`latency_of`].
//!
//! The latency depends on the mode and the resolution only — never on the bands — so
//! adding the first band never moves the project's delay compensation.
//!
//! ## What the null tests rely on
//!
//! - A band with no gain to apply (0 dB on a gain shape) is **skipped**, and a zero-latency
//!   EQ with no active band touches no sample. In the other modes a flat EQ is the input
//!   delayed by the latency, exactly.
//! - Parameter changes glide (so a dragged band does not click) and dynamics move, both
//!   updated every [`UPDATE`] samples counted from the start of the stream — not from the
//!   start of a block — so the output is independent of the block size. The phase modes'
//!   designs, the key's spectrum and the auto gain run on grids counted the same way.

use crate::spectrum::SpectrumAnalyser;
use std::f64::consts::PI;

#[path = "aequalis_fft.rs"]
pub mod fft;
#[path = "aequalis_fit.rs"]
pub mod fit;
#[path = "aequalis_phase.rs"]
pub mod phase;

pub const MAX_BANDS: usize = 24;
/// Sections per band: a 96 dB/octave cut is a 16th-order Butterworth, eight biquads.
pub const MAX_SECTIONS: usize = 8;
/// Samples between coefficient updates for gliding parameters and dynamics.
pub const UPDATE: usize = 16;
/// The highest centre frequency a band is designed at, as a fraction of the sample rate.
/// Matching at a point closer to Nyquist becomes ill-conditioned; 0.45 fs is 21.6 kHz at
/// 48 kHz, above anything anyone hears.
pub const MAX_FREQ_RATIO: f64 = 0.45;
/// Samples between spectra of the sidechain key (and its FFT's hop).
const KEY_HOP: u64 = 2048;
/// The key's spectrum is reported in third octaves: `20 Hz · 2^(k/3)`, k = 0…30.
pub const KEY_BANDS: usize = 31;

/// The centre of the key spectrum's band `k`.
pub fn key_band_hz(k: usize) -> f32 {
    20.0 * 2f32.powf(k as f32 / 3.0)
}

/// What [`Aequalis::telemetry`] reports, index by index (64 numbers, the plug-in contract).
pub mod telemetry {
    /// `[0, 24)`: each band's detector level, dBFS (−200 when the band is not dynamic).
    pub const LEVEL: usize = 0;
    /// `[24, 55)`: the key's spectrum in third octaves (`super::key_band_hz`), dB on the
    /// editor analyser's scale (a full-scale sine reads 0).
    pub const KEY: usize = 24;
    /// 1 when a key arrived in the last block.
    pub const KEY_ON: usize = 55;
    /// The auto gain in force, dB.
    pub const AUTO_GAIN: usize = 56;
    /// Latency, samples.
    pub const LATENCY: usize = 57;
    /// The auditioned band's number plus one, 0 for none.
    pub const SOLO: usize = 58;
    pub const LEN: usize = 64;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum EqShape {
    Bell,
    LowShelf,
    HighShelf,
    LowCut,
    HighCut,
    Notch,
    BandPass,
    TiltShelf,
    AllPass,
}

impl EqShape {
    pub const ALL: [EqShape; 9] = [
        EqShape::Bell,
        EqShape::LowShelf,
        EqShape::HighShelf,
        EqShape::LowCut,
        EqShape::HighCut,
        EqShape::Notch,
        EqShape::BandPass,
        EqShape::TiltShelf,
        EqShape::AllPass,
    ];

    pub fn from_index(i: usize) -> EqShape {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// Whether the gain control means anything for this shape.
    pub fn has_gain(self) -> bool {
        matches!(
            self,
            EqShape::Bell | EqShape::LowShelf | EqShape::HighShelf | EqShape::TiltShelf
        )
    }

    /// Whether the slope control means anything (the cuts).
    pub fn has_slope(self) -> bool {
        matches!(self, EqShape::LowCut | EqShape::HighCut)
    }
}

/// Which part of the stereo image a band acts on.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Placement {
    Stereo,
    Left,
    Right,
    Mid,
    Side,
}

impl Placement {
    pub const ALL: [Placement; 5] = [
        Placement::Stereo,
        Placement::Left,
        Placement::Right,
        Placement::Mid,
        Placement::Side,
    ];

    pub fn from_index(i: usize) -> Placement {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }
}

/// How the EQ trades latency for phase.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    /// The matched cascade: no latency, minimum phase, analog magnitude.
    #[default]
    ZeroLatency,
    /// The cascade plus a short correction: analog magnitude and phase, 64 samples late.
    NaturalPhase,
    /// An FIR: no phase shift at all, at the cost of its length in latency.
    LinearPhase,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::ZeroLatency, Mode::NaturalPhase, Mode::LinearPhase];

    pub fn from_index(i: usize) -> Mode {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }
}

/// The linear-phase FIR's length: half of it is its own delay.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Resolution {
    Low,
    #[default]
    Medium,
    High,
    VeryHigh,
}

impl Resolution {
    pub const ALL: [Resolution; 4] = [
        Resolution::Low,
        Resolution::Medium,
        Resolution::High,
        Resolution::VeryHigh,
    ];

    pub fn from_index(i: usize) -> Resolution {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|r| *r == self).unwrap_or(0)
    }

    /// Half the FIR length at 48 kHz (and 44.1): the FIR's own delay.
    pub fn half(self) -> usize {
        match self {
            Resolution::Low => 2_048,
            Resolution::Medium => 4_096,
            Resolution::High => 8_192,
            Resolution::VeryHigh => 16_384,
        }
    }
}

/// The delay a mode adds at a sample rate, samples — what [`Aequalis::latency`] reports.
pub fn latency_of(mode: Mode, res: Resolution, sample_rate: f32) -> usize {
    match mode {
        Mode::ZeroLatency => 0,
        Mode::NaturalPhase => phase::NP_DELAY,
        Mode::LinearPhase => {
            let f = phase::rate_factor(sample_rate);
            (res.half() + phase::LP_BLOCK) * f
        }
    }
}

/// Everything a band is, as the engine is told it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BandSettings {
    /// Created and not bypassed.
    pub active: bool,
    pub shape: EqShape,
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
    /// dB/octave for the cuts: 6, 12, … 96.
    pub slope: f32,
    pub placement: Placement,
    pub dynamic: bool,
    /// The level at which the band starts to move.
    pub threshold_db: f32,
    /// The most the band's gain moves, signed: negative cuts, positive boosts.
    pub range_db: f32,
    /// How far past the threshold the band moves per dB — 1 moves not at all, ∞ moves by
    /// the whole excess.
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// The band moves while the level is **under** the threshold (an expander's side)
    /// instead of over it.
    pub below: bool,
    /// The detector listens to the sidechain key instead of the band's own signal.
    pub external: bool,
}

impl Default for BandSettings {
    fn default() -> Self {
        BandSettings {
            active: false,
            shape: EqShape::Bell,
            freq: 1_000.0,
            gain_db: 0.0,
            q: 1.0,
            slope: 12.0,
            placement: Placement::Stereo,
            dynamic: false,
            threshold_db: -24.0,
            range_db: -6.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 120.0,
            below: false,
            external: false,
        }
    }
}

/// The settings that are not a band's.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Options {
    pub output_db: f32,
    pub mode: Mode,
    pub resolution: Resolution,
    /// Match the output's loudness to the input's, measured (K-weighted, 1.5 s).
    pub auto_gain: bool,
    /// The band being auditioned: only the part of the signal it acts on is heard.
    pub solo: Option<usize>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            output_db: 0.0,
            mode: Mode::ZeroLatency,
            resolution: Resolution::Medium,
            auto_gain: false,
            solo: None,
        }
    }
}

/// What a band sounds like when it is auditioned: the region it acts on, at unity — a
/// band-pass for a bell or a notch, the shelf's side of its corner, and for a cut the part
/// it removes.
pub fn audition_of(b: &BandSettings) -> BandSettings {
    let (shape, q, slope) = match b.shape {
        EqShape::Bell | EqShape::Notch | EqShape::BandPass | EqShape::AllPass => {
            (EqShape::BandPass, b.q.max(0.2), 12.0)
        }
        EqShape::TiltShelf => (EqShape::BandPass, 0.3, 12.0),
        EqShape::LowShelf => (EqShape::HighCut, std::f32::consts::FRAC_1_SQRT_2, 12.0),
        EqShape::HighShelf => (EqShape::LowCut, std::f32::consts::FRAC_1_SQRT_2, 12.0),
        EqShape::LowCut => (EqShape::HighCut, std::f32::consts::FRAC_1_SQRT_2, b.slope),
        EqShape::HighCut => (EqShape::LowCut, std::f32::consts::FRAC_1_SQRT_2, b.slope),
    };
    BandSettings {
        active: true,
        shape,
        freq: b.freq,
        gain_db: 0.0,
        q,
        slope,
        placement: b.placement,
        ..BandSettings::default()
    }
}

/// One biquad's coefficients (a0 normalised to 1).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Coefs {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Coefs {
    pub const IDENTITY: Coefs = Coefs {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// Magnitude response at `hz`, in dB.
    pub fn magnitude_db(&self, sample_rate: f32, hz: f32) -> f32 {
        let w = 2.0 * PI * hz as f64 / sample_rate.max(1.0) as f64;
        let (re, im) = self.response(w);
        (10.0 * (re * re + im * im).max(1e-30).log10()) as f32
    }

    /// The complex response at `w` radians per sample.
    pub fn response(&self, w: f64) -> (f64, f64) {
        let (s1, c1) = w.sin_cos();
        let (s2, c2) = (2.0 * w).sin_cos();
        let (b0, b1, b2, a1, a2) = (
            self.b0 as f64,
            self.b1 as f64,
            self.b2 as f64,
            self.a1 as f64,
            self.a2 as f64,
        );
        let nr = b0 + b1 * c1 + b2 * c2;
        let ni = -(b1 * s1 + b2 * s2);
        let dr = 1.0 + a1 * c1 + a2 * c2;
        let di = -(a1 * s1 + a2 * s2);
        let den = (dr * dr + di * di).max(1e-300);
        ((nr * dr + ni * di) / den, (ni * dr - nr * di) / den)
    }
}

/// An analog second-order prototype with `s` normalised to the band's frequency:
/// `(b2 s² + b1 s + b0) / (a2 s² + a1 s + a0)`. A first-order section has `a2 = 0`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Proto {
    pub b: [f64; 3],
    pub a: [f64; 3],
}

impl Proto {
    const UNIT: Proto = Proto {
        b: [1.0, 0.0, 0.0],
        a: [1.0, 0.0, 0.0],
    };

    /// `|H(jΩ)|²` with `Ω` in units of the band frequency.
    pub fn mag2(&self, w: f64) -> f64 {
        let w2 = w * w;
        let nr = self.b[0] - self.b[2] * w2;
        let ni = self.b[1] * w;
        let dr = self.a[0] - self.a[2] * w2;
        let di = self.a[1] * w;
        (nr * nr + ni * ni) / (dr * dr + di * di).max(1e-300)
    }

    /// `H(jΩ)`, complex.
    pub fn response(&self, w: f64) -> (f64, f64) {
        let w2 = w * w;
        let nr = self.b[0] - self.b[2] * w2;
        let ni = self.b[1] * w;
        let dr = self.a[0] - self.a[2] * w2;
        let di = self.a[1] * w;
        let den = (dr * dr + di * di).max(1e-300);
        ((nr * dr + ni * di) / den, (ni * dr - nr * di) / den)
    }
}

/// Magnitude-matched digital section for an analog prototype at `w0` rad/sample.
fn matched(p: Proto, w0: f64) -> Coefs {
    // Poles: the analog ones, mapped exactly.
    let (a0, a1, a2) = (p.a[0], p.a[1], p.a[2]);
    let disc = a1 * a1 - 4.0 * a2 * a0;
    let (da1, da2) = if disc < 0.0 {
        let re = -a1 / (2.0 * a2) * w0;
        let im = (-disc).sqrt() / (2.0 * a2) * w0;
        let r = re.exp();
        (-2.0 * r * im.cos(), r * r)
    } else {
        let s1 = (-a1 + disc.sqrt()) / (2.0 * a2) * w0;
        let s2 = (-a1 - disc.sqrt()) / (2.0 * a2) * w0;
        (-(s1.exp() + s2.exp()), (s1 + s2).exp())
    };
    // Zeros: match the magnitude at DC, at Nyquist and at the band frequency.
    let big_a0 = (1.0 + da1 + da2).powi(2);
    let big_a1 = (1.0 - da1 + da2).powi(2);
    let big_a2 = -4.0 * da2;
    let big_b0 = p.mag2(0.0) * big_a0;
    let big_b1 = p.mag2(PI / w0) * big_a1;
    let wm = w0.min(PI * 0.95);
    let phi1 = (wm * 0.5).sin().powi(2);
    let phi0 = 1.0 - phi1;
    let phi2 = 4.0 * phi0 * phi1;
    let big_b2 = if phi2 > 1e-12 {
        (p.mag2(wm / w0) * (big_a0 * phi0 + big_a1 * phi1 + big_a2 * phi2)
            - big_b0 * phi0
            - big_b1 * phi1)
            / phi2
    } else {
        big_a2
    };
    let r0 = big_b0.max(0.0).sqrt();
    let r1 = big_b1.max(0.0).sqrt();
    let w = 0.5 * (r0 + r1);
    let b0 = 0.5 * (w + (w * w + big_b2).max(0.0).sqrt());
    let b2 = if b0.abs() > 1e-300 {
        -big_b2 / (4.0 * b0)
    } else {
        0.0
    };
    let b1 = 0.5 * (r0 - r1);
    Coefs {
        b0: b0 as f32,
        b1: b1 as f32,
        b2: b2 as f32,
        a1: da1 as f32,
        a2: da2 as f32,
    }
}

/// The same prototype through the bilinear transform, pre-warped so the band frequency
/// lands where it should. Used only where it tracks the analog curve better than matching
/// does: a high shelf above 0.35 fs (see `design`, and the test that measured it).
fn bilinear(p: Proto, w0: f64) -> Coefs {
    let k = (w0 * 0.5).tan();
    let k2 = k * k;
    let (b0, b1, b2) = (p.b[0], p.b[1], p.b[2]);
    let (a0, a1, a2) = (p.a[0], p.a[1], p.a[2]);
    let n0 = b2 + b1 * k + b0 * k2;
    let n1 = 2.0 * (b0 * k2 - b2);
    let n2 = b2 - b1 * k + b0 * k2;
    let d0 = a2 + a1 * k + a0 * k2;
    let d1 = 2.0 * (a0 * k2 - a2);
    let d2 = a2 - a1 * k + a0 * k2;
    Coefs {
        b0: (n0 / d0) as f32,
        b1: (n1 / d0) as f32,
        b2: (n2 / d0) as f32,
        a1: (d1 / d0) as f32,
        a2: (d2 / d0) as f32,
    }
}

/// Above this fraction of the sample rate a high (or tilt) shelf's curve reaches so far
/// past Nyquist that matching three points bends it more than the bilinear transform does.
/// Measured, not guessed: `matched_sections_follow_the_analog_curve_up_to_nyquist`.
const SHELF_BILINEAR_ABOVE: f64 = 0.35;

/// A first-order low- or high-pass at `w0`, matched at DC and Nyquist.
fn matched_first_order(high: bool, w0: f64) -> Coefs {
    let p = (-w0).exp();
    let a1 = -p;
    // Analog: 1/(s+1) or s/(s+1), Ω = ω/w0.
    let mag = |w: f64| -> f64 {
        if high {
            w / (w * w + 1.0).sqrt()
        } else {
            1.0 / (w * w + 1.0).sqrt()
        }
    };
    let s0 = mag(0.0) * (1.0 + a1); // b0 + b1
    let sp = mag(PI / w0) * (1.0 - a1); // b0 − b1
    let (b0, b1) = if high {
        // A high-pass needs the zero at DC: b0 = −b1.
        (0.5 * sp, -0.5 * sp)
    } else {
        (0.5 * (s0 + sp), 0.5 * (s0 - sp))
    };
    Coefs {
        b0: b0 as f32,
        b1: b1 as f32,
        b2: 0.0,
        a1: a1 as f32,
        a2: 0.0,
    }
}

/// How one prototype section becomes digital.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Kind {
    Matched,
    Bilinear,
    /// Matched poles, zeros at their reciprocals.
    AllPass,
    FirstOrder {
        high: bool,
    },
}

/// A band's analog prototypes, one per section, and how each becomes digital. Returns the
/// section count and the band's frequency in radians per sample; `None` when the band is a
/// wire (a gain shape at 0 dB).
#[allow(clippy::too_many_arguments)]
fn prototypes(
    shape: EqShape,
    sample_rate: f32,
    freq: f32,
    gain_db: f32,
    q: f32,
    slope: f32,
    protos: &mut [Proto; MAX_SECTIONS],
    kinds: &mut [Kind; MAX_SECTIONS],
) -> Option<(usize, f64)> {
    if shape.has_gain() && (gain_db == 0.0 || !gain_db.is_finite()) {
        return None;
    }
    let sr = sample_rate.max(1.0) as f64;
    let f = (freq as f64).clamp(10.0, sr * MAX_FREQ_RATIO);
    let w0 = 2.0 * PI * f / sr;
    let q = (q as f64).clamp(0.025, 40.0);
    let a = 10f64.powf(gain_db as f64 / 40.0);
    let sa = a.sqrt();
    let mut one = |p: Proto, k: Kind| {
        protos[0] = p;
        kinds[0] = k;
        Some((1, w0))
    };
    match shape {
        EqShape::Bell => one(
            Proto {
                b: [1.0, a / q, 1.0],
                a: [1.0, 1.0 / (a * q), 1.0],
            },
            Kind::Matched,
        ),
        EqShape::LowShelf => one(
            Proto {
                b: [a * a, a * sa / q, a],
                a: [1.0, sa / q, a],
            },
            Kind::Matched,
        ),
        EqShape::HighShelf | EqShape::TiltShelf => {
            let p = if shape == EqShape::HighShelf {
                Proto {
                    b: [a, a * sa / q, a * a],
                    a: [a, sa / q, 1.0],
                }
            } else {
                Proto {
                    b: [1.0, sa / q, a],
                    a: [a, sa / q, 1.0],
                }
            };
            let k = if f / sr > SHELF_BILINEAR_ABOVE {
                Kind::Bilinear
            } else {
                Kind::Matched
            };
            one(p, k)
        }
        EqShape::Notch => one(
            Proto {
                b: [1.0, 0.0, 1.0],
                a: [1.0, 1.0 / q, 1.0],
            },
            Kind::Matched,
        ),
        EqShape::BandPass => one(
            Proto {
                b: [0.0, 1.0 / q, 0.0],
                a: [1.0, 1.0 / q, 1.0],
            },
            Kind::Matched,
        ),
        // Magnitude is flat, so matching cannot place the zeros: an all-pass has them at
        // the poles' reciprocals, which is the denominator reversed.
        EqShape::AllPass => one(
            Proto {
                b: [1.0, -1.0 / q, 1.0],
                a: [1.0, 1.0 / q, 1.0],
            },
            Kind::AllPass,
        ),
        EqShape::LowCut | EqShape::HighCut => {
            let high = shape == EqShape::LowCut;
            let order = ((slope / 6.0).round() as usize).clamp(1, 2 * MAX_SECTIONS);
            let pairs = order / 2;
            let mut n = 0;
            // Butterworth: Q_k = 1 / (2 sin((2k+1)π / 2N)), the highest first. The band's Q
            // is a resonance on that section, relative to the flat 0.707.
            let resonance = q / std::f64::consts::FRAC_1_SQRT_2;
            for k in 0..pairs {
                let mut qk = 1.0 / (2.0 * ((2 * k + 1) as f64 * PI / (2 * order) as f64).sin());
                if k == 0 {
                    qk *= resonance;
                }
                protos[n] = if high {
                    Proto {
                        b: [0.0, 0.0, 1.0],
                        a: [1.0, 1.0 / qk, 1.0],
                    }
                } else {
                    Proto {
                        b: [1.0, 0.0, 0.0],
                        a: [1.0, 1.0 / qk, 1.0],
                    }
                };
                kinds[n] = Kind::Matched;
                n += 1;
            }
            if order % 2 == 1 {
                protos[n] = if high {
                    Proto {
                        b: [0.0, 1.0, 0.0],
                        a: [1.0, 1.0, 0.0],
                    }
                } else {
                    Proto {
                        b: [1.0, 0.0, 0.0],
                        a: [1.0, 1.0, 0.0],
                    }
                };
                kinds[n] = Kind::FirstOrder { high };
                n += 1;
            }
            Some((n, w0))
        }
    }
}

fn digital(p: Proto, kind: Kind, w0: f64) -> Coefs {
    match kind {
        Kind::Matched => matched(p, w0),
        Kind::Bilinear => bilinear(p, w0),
        Kind::AllPass => {
            let m = matched(p, w0);
            Coefs {
                b0: m.a2,
                b1: m.a1,
                b2: 1.0,
                a1: m.a1,
                a2: m.a2,
            }
        }
        Kind::FirstOrder { high } => matched_first_order(high, w0),
    }
}

/// The sections for a band. Returns how many of `out` are used; zero means the band is a
/// wire (a gain shape at 0 dB) and is skipped.
pub fn design(
    shape: EqShape,
    sample_rate: f32,
    freq: f32,
    gain_db: f32,
    q: f32,
    slope: f32,
    out: &mut [Coefs; MAX_SECTIONS],
) -> usize {
    let mut protos = [Proto::UNIT; MAX_SECTIONS];
    let mut kinds = [Kind::Matched; MAX_SECTIONS];
    let Some((n, w0)) = prototypes(
        shape,
        sample_rate,
        freq,
        gain_db,
        q,
        slope,
        &mut protos,
        &mut kinds,
    ) else {
        return 0;
    };
    for i in 0..n {
        out[i] = digital(protos[i], kinds[i], w0);
    }
    n
}

/// A band's **analog** magnitude at `hz`, dB: the curve the natural- and linear-phase modes
/// realise (and the zero-latency one approaches).
pub fn analog_db(b: &BandSettings, sample_rate: f32, hz: f32) -> f32 {
    let mut protos = [Proto::UNIT; MAX_SECTIONS];
    let mut kinds = [Kind::Matched; MAX_SECTIONS];
    let Some((n, w0)) = prototypes(
        b.shape,
        sample_rate,
        b.freq,
        b.gain_db,
        b.q,
        b.slope,
        &mut protos,
        &mut kinds,
    ) else {
        return 0.0;
    };
    let w = 2.0 * PI * hz as f64 / sample_rate.max(1.0) as f64;
    let m2: f64 = protos[..n].iter().map(|p| p.mag2(w / w0)).product();
    (10.0 * m2.max(1e-30).log10()) as f32
}

/// A band's magnitude at `hz` as a mode realises it, dB.
pub fn band_db(b: &BandSettings, mode: Mode, sample_rate: f32, hz: f32) -> f32 {
    match mode {
        Mode::ZeroLatency => Aequalis::band_response_db(b, sample_rate, hz),
        // All-pass has no effect in linear phase: the FIR has no phase to shift.
        Mode::LinearPhase if b.shape == EqShape::AllPass => 0.0,
        _ => analog_db(b, sample_rate, hz),
    }
}

/// A band's magnitude at many frequencies as a mode realises it, dB — designed once, read
/// at every point (what a display wants).
pub fn band_curve_db(b: &BandSettings, mode: Mode, sample_rate: f32, hz: &[f32], out: &mut [f32]) {
    let mut protos = [Proto::UNIT; MAX_SECTIONS];
    let mut kinds = [Kind::Matched; MAX_SECTIONS];
    let plan = prototypes(
        b.shape,
        sample_rate,
        b.freq,
        b.gain_db,
        b.q,
        b.slope,
        &mut protos,
        &mut kinds,
    );
    let flat = !b.active || (mode == Mode::LinearPhase && b.shape == EqShape::AllPass);
    let Some((n, w0)) = plan.filter(|_| !flat) else {
        out.iter_mut().for_each(|o| *o = 0.0);
        return;
    };
    let sr = sample_rate.max(1.0) as f64;
    if mode == Mode::ZeroLatency {
        let mut coefs = [Coefs::IDENTITY; MAX_SECTIONS];
        for i in 0..n {
            coefs[i] = digital(protos[i], kinds[i], w0);
        }
        for (o, f) in out.iter_mut().zip(hz) {
            let w = 2.0 * PI * *f as f64 / sr;
            let m2: f64 = coefs[..n]
                .iter()
                .map(|c| {
                    let (re, im) = c.response(w);
                    re * re + im * im
                })
                .product();
            *o = (10.0 * m2.max(1e-30).log10()) as f32;
        }
    } else {
        for (o, f) in out.iter_mut().zip(hz) {
            let w = 2.0 * PI * *f as f64 / sr;
            let m2: f64 = protos[..n].iter().map(|p| p.mag2(w / w0)).product();
            *o = (10.0 * m2.max(1e-30).log10()) as f32;
        }
    }
}

fn stage_of(
    b: &BandSettings,
    freq: f32,
    gain: f32,
    q: f32,
    sample_rate: f32,
    mute_other: bool,
) -> Option<phase::Stage> {
    let mut protos = [Proto::UNIT; MAX_SECTIONS];
    let mut kinds = [Kind::Matched; MAX_SECTIONS];
    let (n, w0) = prototypes(
        b.shape,
        sample_rate,
        freq,
        gain,
        q,
        b.slope,
        &mut protos,
        &mut kinds,
    )?;
    let mut coefs = [Coefs::IDENTITY; MAX_SECTIONS];
    for i in 0..n {
        coefs[i] = digital(protos[i], kinds[i], w0);
    }
    Some(phase::Stage {
        placement: b.placement,
        mute_other,
        w0,
        n,
        protos,
        coefs,
    })
}

/// Transposed direct form II state for one section on one channel.
#[derive(Copy, Clone, Debug, Default)]
struct State {
    z1: f32,
    z2: f32,
}

impl State {
    #[inline]
    fn tick(&mut self, c: &Coefs, x: f32) -> f32 {
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        if self.z1.abs() < 1e-20 {
            self.z1 = 0.0;
        }
        if self.z2.abs() < 1e-20 {
            self.z2 = 0.0;
        }
        y
    }
}

#[derive(Clone, Debug)]
struct Band {
    target: BandSettings,
    /// The parameters actually in force: they glide toward `target`.
    freq: f64,
    gain: f64,
    q: f64,
    coefs: [Coefs; MAX_SECTIONS],
    sections: usize,
    /// Per section: channel 0 and channel 1 (L/R, or M/S).
    state: [[State; 2]; MAX_SECTIONS],
    /// Detector: a band-pass at the band's frequency, and the envelope in linear level.
    det_coefs: Coefs,
    det_state: [State; 2],
    env: f32,
    /// The detector's level, dB (for the display).
    level_db: f32,
    /// The dynamic gain change in force, dB.
    delta_db: f32,
}

impl Default for Band {
    fn default() -> Self {
        Band {
            target: BandSettings::default(),
            freq: 1_000.0,
            gain: 0.0,
            q: 1.0,
            coefs: [Coefs::IDENTITY; MAX_SECTIONS],
            sections: 0,
            state: [[State::default(); 2]; MAX_SECTIONS],
            det_coefs: Coefs::IDENTITY,
            det_state: [State::default(); 2],
            env: 0.0,
            level_db: -200.0,
            delta_db: 0.0,
        }
    }
}

impl Band {
    /// Run the band's sections on its part of the image. `mute` silences the rest (an
    /// audition).
    fn apply(&mut self, left: &mut [f32], right: &mut [f32], mute: bool) {
        let secs = self.sections;
        let place = self.target.placement;
        match place {
            Placement::Stereo => {
                for x in left.iter_mut() {
                    let mut v = *x;
                    for s in 0..secs {
                        v = self.state[s][0].tick(&self.coefs[s], v);
                    }
                    *x = v;
                }
                for x in right.iter_mut() {
                    let mut v = *x;
                    for s in 0..secs {
                        v = self.state[s][1].tick(&self.coefs[s], v);
                    }
                    *x = v;
                }
            }
            Placement::Left | Placement::Right => {
                let (ch, other) = if place == Placement::Left {
                    (&mut *left, &mut *right)
                } else {
                    (&mut *right, &mut *left)
                };
                for x in ch.iter_mut() {
                    let mut v = *x;
                    for s in 0..secs {
                        v = self.state[s][0].tick(&self.coefs[s], v);
                    }
                    *x = v;
                }
                if mute {
                    other.fill(0.0);
                }
            }
            Placement::Mid | Placement::Side => {
                let mid = place == Placement::Mid;
                for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                    let (mut m, mut sd) = ((*l + *r) * 0.5, (*l - *r) * 0.5);
                    let v = if mid { &mut m } else { &mut sd };
                    for s in 0..secs {
                        *v = self.state[s][0].tick(&self.coefs[s], *v);
                    }
                    if mute {
                        if mid {
                            sd = 0.0;
                        } else {
                            m = 0.0;
                        }
                    }
                    *l = m + sd;
                    *r = m - sd;
                }
            }
        }
    }

    /// Feed the detector: the band's own channel(s), or the key's.
    fn detect(&mut self, left: &[f32], right: &[f32], sr: f32) {
        let t = self.target;
        let att = (-1.0 / (t.attack_ms.max(0.1) * 0.001 * sr)).exp();
        let rel = (-1.0 / (t.release_ms.max(1.0) * 0.001 * sr)).exp();
        for (l, r) in left.iter().zip(right.iter()) {
            let x = match t.placement {
                Placement::Stereo => (l.abs()).max(r.abs()) * l.signum(),
                Placement::Left => *l,
                Placement::Right => *r,
                Placement::Mid => (l + r) * 0.5,
                Placement::Side => (l - r) * 0.5,
            };
            let y = self.det_state[0].tick(&self.det_coefs, x).abs();
            let c = if y > self.env { att } else { rel };
            self.env = y + (self.env - y) * c;
        }
    }
}

/// A two-biquad K-weighting (ITU-R BS.1770) per channel, for the auto gain's loudness.
#[derive(Clone, Debug)]
struct KWeight {
    shelf: Coefs,
    hp: Coefs,
    state: [[State; 2]; 2],
}

impl KWeight {
    fn new(sample_rate: f32) -> Self {
        // The standard's filters as analog specs (pyloudnorm's derivation), designed at
        // this rate with the cookbook's formulas.
        let sr = sample_rate.max(1.0) as f64;
        let (g, q, fc) = (
            3.999_843_853_973_347,
            0.707_175_236_955_419_6,
            1_681.974_450_955_533,
        );
        let a = 10f64.powf(g / 40.0);
        let w0 = 2.0 * PI * fc / sr;
        let alpha = w0.sin() / (2.0 * q);
        let c = w0.cos();
        let a0 = (a + 1.0) - (a - 1.0) * c + 2.0 * a.sqrt() * alpha;
        let shelf = Coefs {
            b0: (a * ((a + 1.0) + (a - 1.0) * c + 2.0 * a.sqrt() * alpha) / a0) as f32,
            b1: (-2.0 * a * ((a - 1.0) + (a + 1.0) * c) / a0) as f32,
            b2: (a * ((a + 1.0) + (a - 1.0) * c - 2.0 * a.sqrt() * alpha) / a0) as f32,
            a1: (2.0 * ((a - 1.0) - (a + 1.0) * c) / a0) as f32,
            a2: (((a + 1.0) - (a - 1.0) * c - 2.0 * a.sqrt() * alpha) / a0) as f32,
        };
        let (q, fc) = (0.500_327_037_323_877_3, 38.135_470_876_024_44);
        let w0 = 2.0 * PI * fc / sr;
        let alpha = w0.sin() / (2.0 * q);
        let c = w0.cos();
        let a0 = 1.0 + alpha;
        let hp = Coefs {
            b0: ((1.0 + c) / 2.0 / a0) as f32,
            b1: (-(1.0 + c) / a0) as f32,
            b2: ((1.0 + c) / 2.0 / a0) as f32,
            a1: (-2.0 * c / a0) as f32,
            a2: ((1.0 - alpha) / a0) as f32,
        };
        KWeight {
            shelf,
            hp,
            state: [[State::default(); 2]; 2],
        }
    }

    /// Mean square of a stereo block after K-weighting, summed over channels.
    fn power(&mut self, left: &[f32], right: &[f32], acc: &mut f64, coef: f64) {
        for (l, r) in left.iter().zip(right.iter()) {
            let sl = self.state[0][0].tick(&self.shelf, *l);
            let a = self.state[0][1].tick(&self.hp, sl);
            let sr = self.state[1][0].tick(&self.shelf, *r);
            let b = self.state[1][1].tick(&self.hp, sr);
            let z = (a * a + b * b) as f64;
            *acc += (z - *acc) * coef;
        }
    }
}

/// The measured auto gain: the input's and the output's K-weighted power over about
/// 1.5 s, and the gain that makes them equal, gliding.
#[derive(Clone, Debug)]
struct AutoGain {
    k_in: KWeight,
    k_out: KWeight,
    e_in: f64,
    e_out: f64,
    coef: f64,
    glide: f64,
    gain_db: f64,
    gain: f32,
}

impl AutoGain {
    fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0) as f64;
        AutoGain {
            k_in: KWeight::new(sample_rate),
            k_out: KWeight::new(sample_rate),
            e_in: 0.0,
            e_out: 0.0,
            coef: 1.0 - (-1.0 / (1.5 * sr)).exp(),
            glide: 1.0 - (-(UPDATE as f64) / (0.3 * sr)).exp(),
            gain_db: 0.0,
            gain: 1.0,
        }
    }

    fn reset(&mut self) {
        self.k_in.state = [[State::default(); 2]; 2];
        self.k_out.state = [[State::default(); 2]; 2];
        self.e_in = 0.0;
        self.e_out = 0.0;
        self.gain_db = 0.0;
        self.gain = 1.0;
    }

    /// One update step: move toward the gain that equalises the two powers. Below
    /// −70 dB of K-weighted power (silence) the gain holds.
    fn update(&mut self) {
        const GATE: f64 = 1e-7;
        if self.e_in > GATE && self.e_out > GATE {
            let target = (10.0 * (self.e_in / self.e_out).log10()).clamp(-24.0, 24.0);
            self.gain_db += (target - self.gain_db) * self.glide;
            if (self.gain_db - target).abs() < 1e-6 {
                self.gain_db = target;
            }
            self.gain = 10f64.powf(self.gain_db / 20.0) as f32;
        }
    }
}

/// The sidechain key's spectrum, third octaves, for the editor's external analyser, its
/// collision view and EQ match. Computed on the audio thread with the editor analyser's
/// own FFT and scale, so the two are comparable number for number.
#[derive(Clone, Debug)]
struct KeySpectrum {
    analyser: SpectrumAnalyser,
    pairs: [f32; 2 * UPDATE],
    levels: [f32; KEY_BANDS],
    seen: bool,
    fresh: bool,
}

impl KeySpectrum {
    fn new() -> Self {
        KeySpectrum {
            analyser: SpectrumAnalyser::new(4096),
            pairs: [0.0; 2 * UPDATE],
            levels: [-200.0; KEY_BANDS],
            seen: false,
            fresh: false,
        }
    }

    fn push(&mut self, kl: &[f32], kr: &[f32]) {
        let n = kl.len().min(kr.len()).min(UPDATE);
        for i in 0..n {
            self.pairs[2 * i] = (kl[i] + kr[i]) * 0.5;
            self.pairs[2 * i + 1] = 0.0;
        }
        self.analyser.push(&self.pairs[..2 * n]);
        self.fresh = true;
    }

    fn compute(&mut self, sample_rate: f32) {
        if !self.fresh || !self.analyser.has_data() {
            return;
        }
        self.fresh = false;
        self.analyser.compute();
        for (k, l) in self.levels.iter_mut().enumerate() {
            *l = self
                .analyser
                .level_db(0, sample_rate, key_band_hz(k), 1.0 / 3.0);
        }
    }
}

/// The equaliser, stereo in place.
#[derive(Clone, Debug)]
pub struct Aequalis {
    sample_rate: f32,
    bands: Vec<Band>,
    output_gain: f32,
    /// Samples since the last coefficient update, counted across blocks.
    phase: usize,
    /// Samples since the stream started (or the last reset).
    clock: u64,
    /// False until the first `set`: the first settings are taken at once, later ones glide.
    configured: bool,
    opts: Options,
    /// The auditioned band's stand-in (`audition_of`), with its own state.
    solo: Band,
    np: Box<phase::NaturalPhase>,
    /// The correction no longer matches the cascade.
    np_dirty: bool,
    /// The linear-phase convolver, once some mode has needed it: megabytes an EQ in zero
    /// latency or natural phase never holds (ADR 0022).
    lp: Option<Box<phase::LinearPhase>>,
    /// The mode the audio runs in: the one asked for, unless linear phase is waiting for
    /// its buffers — then the previous path keeps running until they arrive.
    running: Mode,
    /// Off the audio thread, make the linear-phase buffers when they are needed; on it
    /// ([`Aequalis::realtime`]), ask for them instead ([`Aequalis::linear_need`]).
    grows: bool,
    /// What the phase modes design from: the bands as stages, in band order.
    stages: Vec<phase::Stage>,
    auto: AutoGain,
    key: KeySpectrum,
    /// The input delayed with the linear-phase output, for its detectors and loudness.
    dry: [[f32; UPDATE]; 2],
}

/// How much of the remaining distance a glide covers per update: 16 samples at 48 kHz per
/// step, about 20 ms to settle.
const GLIDE: f64 = 0.35;

impl Aequalis {
    /// An EQ that makes its own linear-phase buffers the moment a mode needs them: for use
    /// off the audio thread (tests, the editor's pictures, offline tools).
    pub fn new(sample_rate: f32) -> Self {
        Aequalis {
            grows: true,
            ..Self::realtime(sample_rate)
        }
    }

    /// An EQ for the audio thread: it never allocates after this. When linear phase needs
    /// buffers it has not got, it keeps running its previous path and says what it needs
    /// ([`Self::linear_need`]); the host builds them elsewhere and hands them over
    /// ([`Self::supply_linear`]), and the switch happens then.
    pub fn realtime(sample_rate: f32) -> Self {
        Aequalis {
            sample_rate,
            bands: vec![Band::default(); MAX_BANDS],
            output_gain: 1.0,
            phase: 0,
            clock: 0,
            configured: false,
            opts: Options::default(),
            solo: Band::default(),
            np: Box::default(),
            np_dirty: true,
            lp: None,
            running: Mode::ZeroLatency,
            grows: false,
            stages: Vec::with_capacity(MAX_BANDS + 1),
            auto: AutoGain::new(sample_rate),
            key: KeySpectrum::new(),
            dry: [[0.0; UPDATE]; 2],
        }
    }

    pub fn reset(&mut self) {
        for b in self.bands.iter_mut().chain(std::iter::once(&mut self.solo)) {
            b.state = [[State::default(); 2]; MAX_SECTIONS];
            b.det_state = [State::default(); 2];
            b.env = 0.0;
            b.level_db = -200.0;
            b.delta_db = 0.0;
        }
        self.phase = 0;
        self.clock = 0;
        self.np.reset();
        if let Some(lp) = self.lp.as_deref_mut() {
            lp.reset();
        }
        self.auto.reset();
        self.np_dirty = true;
    }

    pub fn options(&self) -> Options {
        self.opts
    }

    /// The global settings. A new mode (or resolution, in linear phase) starts its path
    /// clean: its latency is new, so continuity with the old one is not possible anyway.
    pub fn set_options(&mut self, o: Options) {
        let solo = o.solo.filter(|b| *b < MAX_BANDS);
        let o = Options { solo, ..o };
        let old = self.opts;
        self.output_gain = if o.output_db == 0.0 {
            1.0
        } else {
            10f32.powf(o.output_db / 20.0)
        };
        self.opts = o;
        if self.grows {
            if let Some(res) = self.linear_need() {
                // Off the audio thread: make what linear phase needs, now.
                self.lp = Some(Box::new(phase::LinearPhase::new(self.sample_rate, res)));
            }
        }
        self.settle_mode(o.solo != old.solo);
        if !o.auto_gain && old.auto_gain {
            self.auto.reset();
        }
    }

    /// Bring the path that runs in line with the options, as far as the buffers allow: a
    /// linear phase whose buffers have not arrived leaves the previous path running.
    fn settle_mode(&mut self, solo_changed: bool) {
        let want = self.opts.mode;
        let res = self.opts.resolution;
        let fits = self.lp.as_ref().is_some_and(|lp| lp.fits(res));
        let next = if want == Mode::LinearPhase && !fits {
            self.running
        } else {
            want
        };
        let mode_changed = next != self.running;
        self.running = next;
        if mode_changed {
            self.np.reset();
            if let Some(lp) = self.lp.as_deref_mut() {
                lp.reset();
            }
            self.np_dirty = true;
            for b in &mut self.bands {
                b.state = [[State::default(); 2]; MAX_SECTIONS];
                b.delta_db = 0.0;
            }
            self.redesign_all();
        }
        if fits {
            if let Some(lp) = self.lp.as_deref_mut() {
                lp.set_resolution(res);
            }
        }
        if solo_changed || mode_changed {
            self.refresh_solo(true);
            self.np_dirty = true;
            if next == Mode::LinearPhase {
                self.build_stages(true);
                if let Some(lp) = self.lp.as_deref_mut() {
                    lp.request_design();
                }
            }
        }
    }

    /// The mode the audio runs in now: [`Options::mode`], unless linear phase is still
    /// waiting for its buffers.
    pub fn running_mode(&self) -> Mode {
        self.running
    }

    /// The linear-phase buffers this EQ needs and has not got: `Some(resolution)` while
    /// linear phase is asked for and the convolver it holds, if any, is too small for it.
    pub fn linear_need(&self) -> Option<Resolution> {
        let res = self.opts.resolution;
        let fits = self.lp.as_ref().is_some_and(|lp| lp.fits(res));
        (self.opts.mode == Mode::LinearPhase && !fits).then_some(res)
    }

    /// Hand over a linear-phase convolver built elsewhere ([`phase::LinearPhase::new`]).
    /// It is taken if it has more room than the one held (or none is held), and a switch
    /// to linear phase that was waiting for it happens now. What comes back — the
    /// convolver it replaced, or the one offered if it was not taken — is the caller's to
    /// free, off the audio thread. No allocation.
    pub fn supply_linear(
        &mut self,
        fresh: Box<phase::LinearPhase>,
    ) -> Option<Box<phase::LinearPhase>> {
        let room = self.lp.as_ref().map_or(0, |lp| lp.capacity());
        if fresh.capacity() <= room {
            return Some(fresh);
        }
        let old = self.lp.replace(fresh);
        if self.running == Mode::LinearPhase {
            // The path in force ran on the old buffers; the new ones start clean, as a
            // change of resolution does.
            self.running = Mode::ZeroLatency;
        }
        self.settle_mode(false);
        old
    }

    /// The longest linear-phase FIR half the EQ holds buffers for, samples (0 until some
    /// mode has needed them).
    pub fn linear_capacity(&self) -> usize {
        self.lp.as_ref().map_or(0, |lp| lp.capacity())
    }

    /// New band settings and output gain. The first call snaps; later ones glide over
    /// ~20 ms (in linear phase, a new design fades in instead).
    pub fn set(&mut self, bands: &[BandSettings], output_db: f32) {
        let snap = !self.configured;
        self.configured = true;
        self.opts.output_db = output_db;
        self.output_gain = if output_db == 0.0 {
            1.0
        } else {
            10f32.powf(output_db / 20.0)
        };
        let lp = self.running == Mode::LinearPhase;
        let mut changed = snap;
        for (i, b) in self.bands.iter_mut().enumerate() {
            let t = bands.get(i).copied().unwrap_or_default();
            if t == b.target && !snap {
                continue;
            }
            changed = true;
            let was_active = b.target.active;
            let shape_changed = t.shape != b.target.shape || t.slope != b.target.slope;
            b.target = t;
            if snap || !was_active || shape_changed {
                b.freq = t.freq as f64;
                b.gain = t.gain_db as f64;
                b.q = t.q as f64;
                if !was_active || shape_changed {
                    b.state = [[State::default(); 2]; MAX_SECTIONS];
                }
            }
            Self::redesign(self.sample_rate, b, lp);
        }
        if changed {
            self.np_dirty = true;
            self.refresh_solo(snap);
            if lp {
                self.build_stages(true);
                if let Some(lp) = self.lp.as_deref_mut() {
                    lp.request_design();
                }
            }
        }
    }

    fn redesign_all(&mut self) {
        let lp = self.running == Mode::LinearPhase;
        for b in &mut self.bands {
            Self::redesign(self.sample_rate, b, lp);
        }
    }

    /// Point the solo stand-in at the auditioned band's settings.
    fn refresh_solo(&mut self, snap: bool) {
        let Some(s) = self.opts.solo else {
            self.solo.target.active = false;
            return;
        };
        let t = audition_of(&self.bands[s].target);
        let t = BandSettings {
            active: self.bands[s].target.active,
            ..t
        };
        let restart = !self.solo.target.active
            || t.shape != self.solo.target.shape
            || t.slope != self.solo.target.slope
            || t.placement != self.solo.target.placement;
        self.solo.target = t;
        if snap || restart {
            self.solo.freq = t.freq as f64;
            self.solo.gain = 0.0;
            self.solo.q = t.q as f64;
            self.solo.state = [[State::default(); 2]; MAX_SECTIONS];
        }
        Self::redesign(self.sample_rate, &mut self.solo, false);
    }

    /// The bands as the phase modes see them: the targets (linear phase designs from where
    /// the bands are going) or the values in force (natural phase corrects the cascade as
    /// it is). Dynamic movement is not part of a design.
    fn build_stages(&mut self, targets: bool) {
        self.stages.clear();
        let sr = self.sample_rate;
        let pick = |b: &Band| -> (f32, f32, f32) {
            if targets {
                (b.target.freq, b.target.gain_db, b.target.q)
            } else {
                (b.freq as f32, b.gain as f32, b.q as f32)
            }
        };
        if self.opts.solo.is_some() {
            if self.solo.target.active {
                let (f, g, q) = pick(&self.solo);
                if let Some(s) = stage_of(&self.solo.target, f, g, q, sr, true) {
                    self.stages.push(s);
                }
            }
            return;
        }
        for b in &self.bands {
            if !b.target.active {
                continue;
            }
            let (f, g, q) = pick(b);
            if let Some(s) = stage_of(&b.target, f, g, q, sr, false) {
                self.stages.push(s);
            }
        }
    }

    /// Design a band's biquads from the values in force. In linear phase the FIR carries
    /// the static curve, so the biquads carry only a dynamic band's movement.
    fn redesign(sr: f32, b: &mut Band, lp: bool) {
        let gain = if lp {
            b.delta_db as f64
        } else {
            b.gain + b.delta_db as f64
        };
        b.sections = if b.target.active && (!lp || b.target.dynamic) {
            design(
                b.target.shape,
                sr,
                b.freq as f32,
                gain as f32,
                b.q as f32,
                b.target.slope,
                &mut b.coefs,
            )
        } else {
            0
        };
        if b.target.dynamic {
            let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
            design(
                EqShape::BandPass,
                sr,
                b.freq as f32,
                0.0,
                b.q as f32,
                12.0,
                &mut c,
            );
            b.det_coefs = c[0];
        }
    }

    /// Samples of delay this adds: none in zero latency, the correction's centre in
    /// natural phase, the FIR's centre plus a block in linear phase.
    pub fn latency(&self) -> usize {
        match self.running {
            Mode::ZeroLatency => 0,
            Mode::NaturalPhase => phase::NP_DELAY,
            Mode::LinearPhase => self.lp.as_ref().map_or(0, |lp| lp.latency()),
        }
    }

    /// Whether processing would change anything at all (in zero latency; the other modes
    /// always delay).
    pub fn is_active(&self) -> bool {
        self.output_gain != 1.0
            || self.opts.solo.is_some()
            || self
                .bands
                .iter()
                .any(|b| b.target.active && (b.sections > 0 || b.target.dynamic))
    }

    /// The gain change each band's dynamics are applying now, dB (for the display).
    pub fn dynamic_deltas(&self) -> [f32; MAX_BANDS] {
        let mut out = [0.0; MAX_BANDS];
        for (o, b) in out.iter_mut().zip(&self.bands) {
            *o = b.delta_db;
        }
        out
    }

    /// Each band's detector level, dBFS (−200 for a band that is not dynamic).
    pub fn detector_levels(&self) -> [f32; MAX_BANDS] {
        let mut out = [-200.0; MAX_BANDS];
        for (o, b) in out.iter_mut().zip(&self.bands) {
            if b.target.active && b.target.dynamic {
                *o = b.level_db;
            }
        }
        out
    }

    /// The auto gain in force, dB.
    pub fn auto_gain_db(&self) -> f32 {
        if self.opts.auto_gain {
            self.auto.gain_db as f32
        } else {
            0.0
        }
    }

    /// The key's spectrum in third octaves (`key_band_hz`), dB; −200 with no key.
    pub fn key_levels(&self) -> [f32; KEY_BANDS] {
        self.key.levels
    }

    /// What the editor draws (see [`telemetry`] for the layout).
    pub fn telemetry(&self) -> [f32; telemetry::LEN] {
        let mut t = [0.0; telemetry::LEN];
        t[telemetry::LEVEL..telemetry::LEVEL + MAX_BANDS].copy_from_slice(&self.detector_levels());
        t[telemetry::KEY..telemetry::KEY + KEY_BANDS].copy_from_slice(&self.key.levels);
        t[telemetry::KEY_ON] = if self.key.seen { 1.0 } else { 0.0 };
        t[telemetry::AUTO_GAIN] = self.auto_gain_db();
        t[telemetry::LATENCY] = self.latency() as f32;
        t[telemetry::SOLO] = self.opts.solo.map_or(0.0, |s| s as f32 + 1.0);
        t
    }

    /// Advance glides and dynamics by one update step.
    fn update(&mut self) {
        let sr = self.sample_rate;
        let lp = self.running == Mode::LinearPhase;
        let mut glided = false;
        for b in self.bands.iter_mut().chain(std::iter::once(&mut self.solo)) {
            if !b.target.active {
                continue;
            }
            let t = b.target;
            let mut moved = false;
            let glide = |cur: &mut f64, target: f64, log: bool| -> bool {
                if *cur == target {
                    return false;
                }
                let (c, tg) = if log {
                    (cur.max(1e-6).ln(), target.max(1e-6).ln())
                } else {
                    (*cur, target)
                };
                let next = c + (tg - c) * GLIDE;
                let v = if log { next.exp() } else { next };
                *cur = if (v - target).abs() <= target.abs().max(1.0) * 1e-5 {
                    target
                } else {
                    v
                };
                true
            };
            moved |= glide(&mut b.freq, t.freq as f64, true);
            moved |= glide(&mut b.gain, t.gain_db as f64, false);
            moved |= glide(&mut b.q, t.q as f64, true);
            glided |= moved;
            if t.dynamic && t.shape.has_gain() {
                let level_db = if b.env > 1e-9 {
                    20.0 * b.env.log10()
                } else {
                    -200.0
                };
                b.level_db = level_db;
                let over = if t.below {
                    t.threshold_db - level_db
                } else {
                    level_db - t.threshold_db
                };
                // A 6 dB soft knee around the threshold.
                let knee = 6.0;
                let excess = if over <= -knee * 0.5 {
                    0.0
                } else if over >= knee * 0.5 {
                    over
                } else {
                    (over + knee * 0.5).powi(2) / (2.0 * knee)
                };
                let amount = excess * (1.0 - 1.0 / t.ratio.max(1.0));
                let delta = amount.min(t.range_db.abs()) * t.range_db.signum();
                if (delta - b.delta_db).abs() > 1e-4 {
                    b.delta_db = delta;
                    moved = true;
                }
            } else if b.delta_db != 0.0 {
                b.delta_db = 0.0;
                moved = true;
            }
            if moved {
                Self::redesign(sr, b, lp);
            }
        }
        if glided {
            self.np_dirty = true;
        }
        if self.opts.auto_gain && self.opts.solo.is_none() {
            self.auto.update();
        }
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.process_keyed(left, right, None);
    }

    /// Process with the sidechain key, when one is routed: dynamic bands set to listen to
    /// it do, and its spectrum is measured for the editor.
    pub fn process_keyed(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        key: Option<(&[f32], &[f32])>,
    ) {
        let n = left.len().min(right.len());
        let key = key.filter(|(a, b)| a.len() >= n && b.len() >= n);
        self.key.seen = key.is_some();
        if self.running == Mode::ZeroLatency && !self.is_active() {
            // Nothing to do to the audio. Keep the clocks running so a band switched on
            // later lands on the same grid whatever the block size was before, and keep
            // measuring the key.
            let mut i = 0;
            while i < n {
                let chunk = (UPDATE - self.phase).min(n - i);
                if let Some((kl, kr)) = key {
                    self.key_chunk(&kl[i..i + chunk], &kr[i..i + chunk]);
                }
                self.phase = (self.phase + chunk) % UPDATE;
                self.clock += chunk as u64;
                i += chunk;
            }
            return;
        }
        let mut i = 0;
        while i < n {
            let chunk = (UPDATE - self.phase).min(n - i);
            let k = key.map(|(kl, kr)| (&kl[i..i + chunk], &kr[i..i + chunk]));
            if let Some((kl, kr)) = k {
                self.key_chunk(kl, kr);
            }
            self.chunk(&mut left[i..i + chunk], &mut right[i..i + chunk], k);
            self.phase += chunk;
            self.clock += chunk as u64;
            i += chunk;
            if self.phase == UPDATE {
                self.phase = 0;
                self.update();
            }
        }
    }

    fn key_chunk(&mut self, kl: &[f32], kr: &[f32]) {
        if self.clock.is_multiple_of(KEY_HOP) {
            self.key.compute(self.sample_rate);
        }
        self.key.push(kl, kr);
    }

    /// One piece of a block, inside one update step.
    fn chunk(&mut self, l: &mut [f32], r: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let auto = self.opts.auto_gain && self.opts.solo.is_none();
        let coef = self.auto.coef;
        match self.running {
            Mode::ZeroLatency => {
                if auto {
                    self.auto.k_in.power(l, r, &mut self.auto.e_in, coef);
                }
                self.run_cascade(l, r, key);
            }
            Mode::NaturalPhase => {
                if auto {
                    self.auto.k_in.power(l, r, &mut self.auto.e_in, coef);
                }
                if self.np_dirty && self.clock.is_multiple_of(phase::NP_FADE as u64) {
                    self.build_stages(false);
                    self.np.design(&self.stages);
                    self.np_dirty = false;
                }
                self.run_cascade(l, r, key);
                self.np.process(l, r);
            }
            Mode::LinearPhase => {
                let n = l.len();
                let [dl, dr] = &mut self.dry;
                if let Some(lp) = self.lp.as_deref_mut() {
                    lp.process(l, r, &mut dl[..n], &mut dr[..n], &self.stages);
                }
                if auto {
                    self.auto
                        .k_in
                        .power(&dl[..n], &dr[..n], &mut self.auto.e_in, coef);
                }
                if self.opts.solo.is_none() {
                    let sr = self.sample_rate;
                    for b in &mut self.bands {
                        if !b.target.active || !b.target.dynamic || !b.target.shape.has_gain() {
                            continue;
                        }
                        match key {
                            Some((kl, kr)) if b.target.external => b.detect(kl, kr, sr),
                            _ => b.detect(&dl[..n], &dr[..n], sr),
                        }
                        if b.sections > 0 {
                            b.apply(l, r, false);
                        }
                    }
                }
            }
        }
        if auto {
            self.auto.k_out.power(l, r, &mut self.auto.e_out, coef);
        }
        let g = self.output_gain * if auto { self.auto.gain } else { 1.0 };
        if g != 1.0 {
            for x in l.iter_mut().chain(r.iter_mut()) {
                *x *= g;
            }
        }
    }

    /// The minimum-phase cascade: every band in number order (or the auditioned one).
    fn run_cascade(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let sr = self.sample_rate;
        if self.opts.solo.is_some() {
            if self.solo.target.active && self.solo.sections > 0 {
                self.solo.apply(left, right, true);
            }
            return;
        }
        for b in &mut self.bands {
            if !b.target.active {
                continue;
            }
            let t = b.target;
            // Detector first, on the band's own channel(s) — or the key's — before the band
            // acts.
            if t.dynamic && t.shape.has_gain() {
                match key {
                    Some((kl, kr)) if t.external => b.detect(kl, kr, sr),
                    _ => b.detect(left, right, sr),
                }
            }
            if b.sections == 0 {
                continue;
            }
            b.apply(left, right, false);
        }
    }

    /// The whole EQ's static response at `hz` (dynamics at rest), dB — the curve drawn.
    /// Computed from the same design as the audio path.
    pub fn response_db(bands: &[BandSettings], sample_rate: f32, hz: f32) -> f32 {
        let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
        bands
            .iter()
            .filter(|b| b.active)
            .map(|b| {
                let n = design(
                    b.shape,
                    sample_rate,
                    b.freq,
                    b.gain_db,
                    b.q,
                    b.slope,
                    &mut c,
                );
                c[..n]
                    .iter()
                    .map(|s| s.magnitude_db(sample_rate, hz))
                    .sum::<f32>()
            })
            .sum()
    }

    /// One band's static response at `hz`, dB.
    pub fn band_response_db(band: &BandSettings, sample_rate: f32, hz: f32) -> f32 {
        Self::response_db(std::slice::from_ref(band), sample_rate, hz)
    }

    /// The whole EQ's static response at `hz` as `mode` realises it, dB.
    pub fn curve_db(bands: &[BandSettings], mode: Mode, sample_rate: f32, hz: f32) -> f32 {
        bands
            .iter()
            .filter(|b| b.active)
            .map(|b| band_db(b, mode, sample_rate, hz))
            .sum()
    }
}

#[cfg(test)]
#[path = "aequalis_tests.rs"]
mod tests;
