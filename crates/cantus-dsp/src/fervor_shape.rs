//! Fervor's curves: the seven models, the drawn curve, and the one formula that makes
//! every model a wire at 0 dB of drive.
//!
//! ## Drive is a gain into a curve whose knee comes down from infinity
//!
//! Each analytic model is a normalised sigmoid `S` — `S(0) = 0`, slope 1 at the origin,
//! bounded by 1 (the fold excepted) — and the output is
//!
//! ```text
//!     y = G · (S(a·x + β) − S(β)) / (a · S'(β)),     a = G − 1
//! ```
//!
//! with `G` the drive as a gain and `β` the bias in the curve's units. The slope at the
//! origin is `G` — the drive *is* an input gain, as on hardware — and the ceiling is
//! `G / a`: far above full scale when the drive is small, coming down to 0 dBFS as it
//! grows. At 0 dB (`G = 1`, `a = 0`) the formula's limit is `y = x`, and the code takes
//! that limit exactly, so the default is a wire to the bit and the first fraction of a dB
//! has no step. `S(β)` is taken off so silence stays silence whatever the bias, and the
//! slope at the bias point is normalised so the bias changes the colour, not the level.
//!
//! The drawn curve is used as drawn: `y = C(G·x + β) − C(β)`, its domain ±1 (beyond it the
//! ends hold).

/// The models, by the `model` parameter's index.
use crate::math;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Model {
    /// tanh, with high-frequency pre-emphasis: highs saturate first, so they are what the
    /// tape loses as it is driven (`char`: the emphasis, 0 … 12 dB at 3 kHz).
    Tape,
    /// Asymmetric: the negative side bends earlier and lower, so even harmonics
    /// (`char`: the asymmetry).
    Tube,
    /// A harder knee, x / (1 + x⁴)^¼, with the low end spared (a low shelf down before the
    /// curve and back up after, the overdrive pedal's mid hump; `char`: 0 … 18 dB at 700 Hz).
    Drive,
    /// A near-square knee, x / (1 + x⁸)^⅛, with the negative half folding toward a
    /// rectifier (`char`: from symmetric to a half-wave gate and past it, to octave fuzz).
    Fuzz,
    /// Clean until the ceiling, then flat: a hard clip with a quadratic knee
    /// (`char`: the knee's width, 0 = hard).
    Clip,
    /// The signal folds back from the ceiling: a sine (`char` 0) or a triangle (1).
    Fold,
    /// The drawn waveshaper.
    Curve,
}

impl Model {
    pub const ALL: [Model; 7] = [
        Model::Tape,
        Model::Tube,
        Model::Drive,
        Model::Fuzz,
        Model::Clip,
        Model::Fold,
        Model::Curve,
    ];

    pub fn from_index(i: usize) -> Model {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    /// Whether the model's curve is the same turned upside down (no even harmonics without
    /// a bias).
    pub fn symmetric(self) -> bool {
        !matches!(self, Model::Tube | Model::Fuzz | Model::Curve)
    }
}

/// How far a bias of 100 % moves the curve, in its own units (1 is its knee).
pub const BIAS_SCALE: f64 = 0.8;
/// Below this `a`, the curve is the wire it tends to.
const A_WIRE: f64 = 1e-6;

/// The drawn curve as a table: 2048 segments over −1 … 1, read with linear interpolation.
pub const LUT_N: usize = 2048;

/// A sigmoid normalised to slope 1 at the origin, and its slope.
#[inline]
fn sig(model: Model, ch: f64, u: f64) -> f64 {
    match model {
        Model::Tape => math::tanh(u),
        Model::Tube => {
            if u >= 0.0 {
                math::tanh(u)
            } else {
                // The negative side bends at 1/k and stops at −1/k.
                let k = 1.0 + 2.0 * ch;
                math::tanh(k * u) / k
            }
        }
        Model::Drive => u / (1.0 + u * u * u * u).sqrt().sqrt(),
        Model::Fuzz => {
            let u2 = u * u;
            let u4 = u2 * u2;
            let h = u / (1.0 + u4 * u4).sqrt().sqrt().sqrt();
            if u >= 0.0 {
                h
            } else {
                // From 1 (symmetric) down to −0.25 (past a half-wave rectifier).
                h * (1.0 - 1.25 * ch)
            }
        }
        Model::Clip => {
            let k = ch.clamp(0.0, 1.0);
            let m = u.abs();
            let y = if m <= 1.0 - k {
                m
            } else if m >= 1.0 + k {
                1.0
            } else {
                let d = m - (1.0 - k);
                m - d * d / (4.0 * k)
            };
            y.copysign(u)
        }
        Model::Fold => {
            let s = math::sin(u);
            // A triangle of slope 1 at the origin and period 4: folds at ±1.
            let t = {
                let p = (u + 1.0).rem_euclid(4.0);
                if p < 2.0 {
                    p - 1.0
                } else {
                    3.0 - p
                }
            };
            s + (t - s) * ch
        }
        Model::Curve => u,
    }
}

/// The slope of [`sig`] at `u`, by a central difference (only taken where the bias sits,
/// once per change of settings).
fn slope(model: Model, ch: f64, u: f64) -> f64 {
    if u == 0.0 {
        return 1.0;
    }
    let h = 1e-5;
    (sig(model, ch, u + h) - sig(model, ch, u - h)) / (2.0 * h)
}

/// A drawn point: where it is, and the tension of the segment that ends there.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub t: f32,
}

/// The drawn curve's settings: up to 16 points between its two ends.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveSpec {
    /// Odd-symmetric: only x > 0 is drawn, from the origin, and mirrored.
    pub symmetric: bool,
    /// The values at x = −1 and x = 1.
    pub lo: f32,
    pub hi: f32,
    /// Tension of the last segment (into x = 1).
    pub end_t: f32,
    pub points: [Option<Point>; 16],
}

impl Default for CurveSpec {
    fn default() -> Self {
        CurveSpec {
            symmetric: true,
            lo: -1.0,
            hi: 1.0,
            end_t: 0.0,
            points: [None; 16],
        }
    }
}

/// The tension warp of a segment, 0 … 1 → 0 … 1: a straight line at 0, bowed up for a
/// positive tension and down for a negative one (a rational curve: cheap, monotonic, and
/// symmetric between the two signs).
#[inline]
pub fn warp(u: f64, t: f64) -> f64 {
    let c = math::exp(-4.0 * t);
    u / (u + c * (1.0 - u))
}

impl CurveSpec {
    /// The curve's knots in x order, ends included: (x, y, tension of the segment into
    /// it). Returns how many of `out` are used.
    pub fn knots(&self, out: &mut [(f32, f32, f32); 18]) -> usize {
        let mut n = 0;
        let start = if self.symmetric { 0.0 } else { -1.0 };
        out[n] = (start, if self.symmetric { 0.0 } else { self.lo }, 0.0);
        n += 1;
        for p in self.points.iter().flatten() {
            if p.x <= start || p.x >= 1.0 {
                continue;
            }
            out[n] = (p.x, p.y, p.t);
            n += 1;
        }
        // Insertion sort by x (sixteen at most; no allocation).
        for i in 2..n {
            let mut j = i;
            while j > 1 && out[j - 1].0 > out[j].0 {
                out.swap(j - 1, j);
                j -= 1;
            }
        }
        out[n] = (1.0, self.hi, self.end_t);
        n + 1
    }

    /// The curve's value at `x` (−1 … 1), exactly as drawn (the table is made from this).
    pub fn eval(&self, x: f32) -> f32 {
        let mut k = [(0.0f32, 0.0f32, 0.0f32); 18];
        let n = self.knots(&mut k);
        eval_knots(&k[..n], self.symmetric, x)
    }

    /// Fill a table of `LUT_N + 1` values over −1 … 1.
    pub fn table(&self, lut: &mut [f32; LUT_N + 1]) {
        let mut k = [(0.0f32, 0.0f32, 0.0f32); 18];
        let n = self.knots(&mut k);
        for (i, v) in lut.iter_mut().enumerate() {
            let x = -1.0 + 2.0 * i as f32 / LUT_N as f32;
            *v = eval_knots(&k[..n], self.symmetric, x);
        }
    }
}

fn eval_knots(k: &[(f32, f32, f32)], symmetric: bool, x: f32) -> f32 {
    if symmetric && x < 0.0 {
        return -eval_knots(k, true, -x);
    }
    let x = x.clamp(k[0].0, 1.0);
    for w in k.windows(2) {
        let (x0, y0, _) = w[0];
        let (x1, y1, t) = w[1];
        if x <= x1 {
            let span = x1 - x0;
            let u = if span > 0.0 { (x - x0) / span } else { 1.0 };
            return y0 + (y1 - y0) * warp(u as f64, t as f64) as f32;
        }
    }
    k[k.len() - 1].1
}

/// The reference level the output is matched at: a sine at −18 dBFS RMS (its peak).
pub const REF_PEAK: f64 = 0.178_255_19;
/// The most the level match takes away and adds, as gains (−48 dB, +24 dB).
const COMP_MIN: f64 = 0.003_981_07;
const COMP_MAX: f64 = 15.848_932;

/// One setting of the curve, ready to run per sample: the model, the drive as `G` and
/// `a = G − 1`, the bias, and the normalisation.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Shaper {
    pub model: Model,
    /// The character, 0 … 1.
    pub ch: f64,
    pub g: f64,
    pub a: f64,
    /// The bias, in the curve's units.
    pub beta: f64,
    /// `G / (a · S'(β))`.
    norm: f64,
    /// `S(β)` (or the drawn curve's value at the bias).
    offset: f64,
}

impl Shaper {
    /// The wire: what every analytic model is at 0 dB.
    pub const WIRE: Shaper = Shaper {
        model: Model::Tape,
        ch: 0.5,
        g: 1.0,
        a: 0.0,
        beta: 0.0,
        norm: 1.0,
        offset: 0.0,
    };

    /// `drive_db` is the whole drive (the ×100 included); `bias` −1 … 1.
    pub fn new(model: Model, drive_db: f64, ch: f64, bias: f64, lut: &[f32]) -> Shaper {
        let g = math::powf(10f64, drive_db.max(0.0) / 20.0);
        let a = g - 1.0;
        let ch = ch.clamp(0.0, 1.0);
        if model == Model::Curve {
            let beta = bias.clamp(-1.0, 1.0) * 0.5;
            let offset = lut_read(lut, beta) as f64;
            return Shaper {
                model,
                ch,
                g,
                a,
                beta,
                norm: g,
                offset,
            };
        }
        let beta = bias.clamp(-1.0, 1.0) * BIAS_SCALE;
        let s1 = slope(model, ch, beta).max(1e-3);
        Shaper {
            model,
            ch,
            g,
            a,
            beta,
            norm: if a > 0.0 { g / (a * s1) } else { 1.0 },
            offset: sig(model, ch, beta),
        }
    }

    /// The model at 0 dB of drive, which is exactly a wire (the drawn curve never is).
    pub fn is_wire(&self) -> bool {
        self.model != Model::Curve && self.a < A_WIRE
    }

    /// The curve at `x`.
    #[inline]
    pub fn apply(&self, x: f32, lut: &[f32]) -> f32 {
        if self.model == Model::Curve {
            let u = self.g * x as f64 + self.beta;
            return (lut_read(lut, u) as f64 - self.offset) as f32;
        }
        if self.a < A_WIRE {
            // The limit of the formula as a → 0: x itself (g is 1 to within 1e-6).
            return x;
        }
        let u = self.a * x as f64 + self.beta;
        ((sig(self.model, self.ch, u) - self.offset) * self.norm) as f32
    }

    /// The gain that takes the curve's loudness at the reference level back out: the RMS
    /// of a reference sine over the RMS of what the curve makes of it (its DC removed, as
    /// the DC blocker removes it). Exactly 1 for a wire.
    pub fn level_match(&self, lut: &[f32]) -> f32 {
        if self.is_wire() {
            return 1.0;
        }
        const N: usize = 64;
        let mut y = [0.0f64; N];
        let mut mean = 0.0;
        for (i, v) in y.iter_mut().enumerate() {
            let x = REF_PEAK * math::sin(std::f64::consts::TAU * (i as f64 + 0.5) / N as f64);
            *v = self.apply(x as f32, lut) as f64;
            mean += *v;
        }
        mean /= N as f64;
        let rms = (y.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / N as f64).sqrt();
        let want = REF_PEAK / std::f64::consts::SQRT_2;
        if rms <= want * COMP_MIN {
            return COMP_MAX as f32;
        }
        (want / rms).clamp(COMP_MIN, COMP_MAX) as f32
    }
}

/// The drawn curve's table read at `u` (−1 … 1, held beyond), linearly interpolated.
#[inline]
pub fn lut_read(lut: &[f32], u: f64) -> f32 {
    let n = (lut.len() - 1) as f64;
    let p = ((u + 1.0) * 0.5 * n).clamp(0.0, n);
    let i = (p as usize).min(lut.len() - 2);
    let f = (p - i as f64) as f32;
    lut[i] + (lut[i + 1] - lut[i]) * f
}
