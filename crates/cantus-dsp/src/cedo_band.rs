//! Cedo's band split: where the duck acts — the whole signal, only the lows, or only the
//! highs — through one of two crossovers, each with an **exact null path** (a band gain of
//! exactly 1 hands the delayed input on to the bit; the filters run beside it so a duck
//! that starts finds them warm).
//!
//! - **Minimum phase** (no latency): two Simper state-variable sections at the crossover
//!   (Butterworth damping, `k = √2`, coefficients fixed by the frequency alone), read as a
//!   **shelf whose depth moves**: a low shelf `(s² + kAs + A²)/(s² + ks + 1)` per section,
//!   `A² = √G` for a band gain `G`. Its magnitude is `|H|² = (A⁴ + ω⁴)/(1 + ω⁴)` per
//!   section — monotone from `G` at DC to 1 above, **no bump anywhere**, and at a deep duck
//!   −6 dB at the crossover (−3 per section), as a Linkwitz–Riley split is. The highs use
//!   the mirrored high shelf `(A²s² + kAs + 1)/(s² + ks + 1)`. Only the outputs' weights
//!   move with the gain, never the filters' own coefficients, so a duck moving every sample
//!   cannot destabilise them.
//! - **Linear phase**: the low band is four boxcars of `M` samples in cascade — a
//!   B-spline kernel, symmetric, `|sinc|⁴` in magnitude (first side lobe −53 dB, −6 dB at
//!   the crossover: `M = 0.3189·fs/fc`) — and the highs are the delayed input minus it, so
//!   the two bands sum to the delayed input exactly and the ducked response
//!   `1 − (1 − G)·sinc⁴` never rises above 1. The boxcars run in **integers** (the input in
//!   steps of 2⁻³⁶, each stage's running sum exact and its output a pure function of the
//!   sum), so the sums never drift and come back to exactly zero. Its latency is fixed at
//!   the lowest crossover's, [`LIN_MIN_HZ`] (12.7 ms at 48 kHz), whatever the frequency, so
//!   moving the crossover never moves the latency; a higher crossover's kernel reads the
//!   input that much later.

use crate::math;

/// The lowest crossover; also what the linear-phase latency is sized for.
pub const MIN_HZ: f32 = 40.0;
/// The highest crossover.
pub const MAX_HZ: f32 = 2_000.0;
/// The lowest crossover the linear-phase kernel reaches (its latency is this one's).
pub const LIN_MIN_HZ: f32 = 50.0;
/// Where `|sinc(x)|⁴ = ½` (−6 dB): the boxcar length is this many crossover periods.
pub const SINC4_HALF: f64 = 0.3189;
/// Boxcars in the linear-phase cascade.
const STAGES: usize = 4;
/// The input's integer step: 2⁻³⁶ (−217 dBFS).
const SCALE: f64 = 68_719_476_736.0;
/// The input is clamped to this before it is made an integer (+84 dBFS): far above any
/// signal, and the sums stay inside `i64` for the longest boxcar at 192 kHz.
const CLAMP: f64 = 16_384.0;
/// SVF states under this, with silence coming in, are stopped (exact rest).
const TINY: f64 = 1e-25;

/// Where the duck acts.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Band {
    #[default]
    Whole,
    Lows,
    Highs,
}

impl Band {
    pub fn from_index(i: usize) -> Band {
        match i {
            1 => Band::Lows,
            2 => Band::Highs,
            _ => Band::Whole,
        }
    }
}

/// How the band is split.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Crossover {
    #[default]
    MinimumPhase,
    LinearPhase,
}

/// The boxcar length for a crossover at `hz`.
pub fn boxcar_len(hz: f32, sample_rate: f32) -> usize {
    let hz = hz.clamp(LIN_MIN_HZ, MAX_HZ) as f64;
    ((SINC4_HALF * sample_rate as f64 / hz).round() as usize).max(2)
}

/// The linear-phase kernel's delay at a rate: the centre of the longest kernel.
pub fn linear_latency(sample_rate: f32) -> usize {
    2 * (boxcar_len(LIN_MIN_HZ, sample_rate) - 1)
}

/// One Simper SVF section's coefficients (fixed by the crossover alone).
#[derive(Copy, Clone, Debug, Default)]
struct Svf {
    a1: f64,
    a2: f64,
    a3: f64,
}

const K: f64 = std::f64::consts::SQRT_2;

impl Svf {
    fn new(hz: f32, sr: f32) -> Svf {
        let w = std::f64::consts::PI * (hz as f64 / sr as f64).min(0.49);
        let g = math::tan(w);
        let a1 = 1.0 / (1.0 + g * (g + K));
        let a2 = g * a1;
        Svf { a1, a2, a3: g * a2 }
    }

    /// One sample through a section with states `s`: (band, low).
    #[inline]
    fn tick(&self, s: &mut [f64; 2], v0: f64) -> (f64, f64) {
        let v3 = v0 - s[1];
        let v1 = self.a1 * s[0] + self.a2 * v3;
        let v2 = s[1] + self.a2 * s[0] + self.a3 * v3;
        s[0] = 2.0 * v1 - s[0];
        s[1] = 2.0 * v2 - s[1];
        if v0 == 0.0 && s[0].abs() < TINY && s[1].abs() < TINY {
            *s = [0.0; 2];
        }
        (v1, v2)
    }
}

/// One boxcar of the linear-phase cascade: its last `m` inputs and their exact sum.
#[derive(Clone, Debug)]
struct Boxcar {
    line: Vec<i64>,
    pos: usize,
    sum: i64,
}

impl Boxcar {
    fn new(cap: usize) -> Boxcar {
        Boxcar {
            line: vec![0; cap.max(1)],
            pos: 0,
            sum: 0,
        }
    }

    /// One input; the mean of the last `m`, rounded to the nearest step.
    #[inline]
    fn tick(&mut self, x: i64, m: usize) -> i64 {
        self.sum += x - self.line[self.pos];
        self.line[self.pos] = x;
        self.pos += 1;
        if self.pos == m {
            self.pos = 0;
        }
        mean(self.sum, m)
    }

    fn clear(&mut self) {
        self.line.iter_mut().for_each(|v| *v = 0);
        self.pos = 0;
        self.sum = 0;
    }

    fn is_clear(&self) -> bool {
        self.sum == 0 && self.line.iter().all(|v| *v == 0)
    }
}

/// `sum / m` to the nearest integer (halves up): a pure function of the sum.
#[inline]
fn mean(sum: i64, m: usize) -> i64 {
    let m = m as i64;
    (sum + m / 2).div_euclid(m)
}

#[inline]
fn to_int(x: f32) -> i64 {
    ((x as f64).clamp(-CLAMP, CLAMP) * SCALE).round() as i64
}

/// The split for both channels.
#[derive(Clone, Debug)]
pub struct Split {
    sr: f32,
    band: Band,
    cross: Crossover,
    hz: f32,
    svf: Svf,
    /// Per channel, per section: the SVF's two states.
    sv: [[[f64; 2]; 2]; 2],
    m: usize,
    boxes: [[Boxcar; STAGES]; 2],
}

impl Split {
    /// Allocates the boxcars for the longest kernel.
    pub fn new(sample_rate: f32) -> Split {
        let cap = boxcar_len(LIN_MIN_HZ, sample_rate);
        let mk = || std::array::from_fn(|_| Boxcar::new(cap));
        let mut s = Split {
            sr: sample_rate,
            band: Band::Whole,
            cross: Crossover::MinimumPhase,
            hz: 120.0,
            svf: Svf::default(),
            sv: [[[0.0; 2]; 2]; 2],
            m: cap,
            boxes: [mk(), mk()],
        };
        s.svf = Svf::new(s.hz, sample_rate);
        s
    }

    /// The latency the split adds: the linear-phase kernel's centre, when it is in.
    pub fn latency(&self) -> usize {
        if self.uses_kernel() {
            linear_latency(self.sr)
        } else {
            0
        }
    }

    fn uses_kernel(&self) -> bool {
        self.band != Band::Whole && self.cross == Crossover::LinearPhase
    }

    /// How much later than the dry path's newest sample the kernel's input is read, so its
    /// centre lands on the latency whatever its length.
    pub fn kernel_offset(&self) -> usize {
        2 * (boxcar_len(LIN_MIN_HZ, self.sr) - self.m)
    }

    /// The boxcar length in force.
    pub fn kernel_len(&self) -> usize {
        self.m
    }

    /// New settings. A new kernel length rebuilds the boxcars from `history` (the dry
    /// path's input, newest first: `history(d)` is the input `d` samples ago), so the
    /// kernel is the one it would be had it always been that long. No allocation.
    pub fn set(
        &mut self,
        band: Band,
        cross: Crossover,
        hz: f32,
        history: &dyn Fn(usize) -> [f32; 2],
    ) {
        let hz = if hz.is_finite() {
            hz.clamp(MIN_HZ, MAX_HZ)
        } else {
            120.0
        };
        if hz != self.hz {
            self.hz = hz;
            self.svf = Svf::new(hz, self.sr);
        }
        let was = self.uses_kernel();
        self.band = band;
        self.cross = cross;
        let m = boxcar_len(hz, self.sr);
        if self.uses_kernel() && (m != self.m || !was) {
            self.m = m;
            self.rebuild(history);
        } else {
            self.m = m;
        }
    }

    /// The boxcars as they would be after hearing `history` through the kernel's input tap.
    fn rebuild(&mut self, history: &dyn Fn(usize) -> [f32; 2]) {
        let m = self.m;
        let off = self.kernel_offset();
        // Stage s needs its last m inputs, which need (s + 1)·(m − 1) + 1 inputs back.
        let span = STAGES * (m - 1) + 1;
        for ch in 0..2 {
            for b in self.boxes[ch].iter_mut() {
                b.clear();
            }
            for back in (0..span).rev() {
                let x = to_int(history(off + back)[ch]);
                let mut v = x;
                for b in self.boxes[ch].iter_mut() {
                    v = b.tick(v, m);
                }
            }
        }
    }

    /// One sample: `dry` the delayed input (what passes at a band gain of 1), `kernel_in`
    /// the input at the kernel's tap, `g` each channel's band gain (1 is no duck).
    #[inline]
    pub fn tick(&mut self, dry: [f32; 2], kernel_in: [f32; 2], g: [f32; 2]) -> [f32; 2] {
        let mut out = dry;
        match (self.band, self.cross) {
            (Band::Whole, _) => {
                for ch in 0..2 {
                    if g[ch] != 1.0 {
                        out[ch] = dry[ch] * g[ch];
                    }
                }
            }
            (band, Crossover::MinimumPhase) => {
                for ch in 0..2 {
                    out[ch] = self.shelves(ch, dry[ch], g[ch], band == Band::Lows);
                }
            }
            (band, Crossover::LinearPhase) => {
                for ch in 0..2 {
                    let mut v = to_int(kernel_in[ch]);
                    for b in self.boxes[ch].iter_mut() {
                        v = b.tick(v, self.m);
                    }
                    if g[ch] == 1.0 {
                        continue;
                    }
                    let low = (v as f64 / SCALE) as f32;
                    let x = dry[ch];
                    out[ch] = if band == Band::Lows {
                        x + (g[ch] - 1.0) * low
                    } else {
                        g[ch] * x + (1.0 - g[ch]) * low
                    };
                }
            }
        }
        out
    }

    /// The two shelves of channel `ch` at band gain `g`; at 1 the input, to the bit.
    #[inline]
    fn shelves(&mut self, ch: usize, x: f32, g: f32, low: bool) -> f32 {
        let svf = self.svf;
        let s = &mut self.sv[ch];
        if g == 1.0 {
            // The null path: each section's output is its input; the states follow it.
            let v0 = x as f64;
            svf.tick(&mut s[0], v0);
            svf.tick(&mut s[1], v0);
            return x;
        }
        // A² = √G per section, so the two give G.
        let a2 = (g.max(0.0) as f64).sqrt();
        let a = a2.sqrt();
        let mut y = x as f64;
        for sec in s.iter_mut() {
            let (v1, v2) = svf.tick(sec, y);
            y = if low {
                y + K * (a - 1.0) * v1 + (a2 - 1.0) * v2
            } else {
                a2 * y + K * a * (1.0 - a) * v1 + (1.0 - a2) * v2
            };
        }
        y as f32
    }

    /// Every filter state empty.
    pub fn is_clear(&self) -> bool {
        self.sv.iter().flatten().flatten().all(|v| v.to_bits() == 0)
            && self.boxes.iter().flatten().all(Boxcar::is_clear)
    }

    pub fn reset(&mut self) {
        self.sv = [[[0.0; 2]; 2]; 2];
        for b in self.boxes.iter_mut().flatten() {
            b.clear();
        }
    }

    /// The magnitude, dB, at `hz` for a band gain `g`: what the display draws.
    pub fn response_db(&self, hz: f32, g: f32) -> f32 {
        let g = g.clamp(0.0, 1.0) as f64;
        let m2 = match (self.band, self.cross) {
            (Band::Whole, _) => g * g,
            (band, Crossover::MinimumPhase) => {
                let pi = std::f64::consts::PI;
                let w = math::tan(pi * (hz as f64 / self.sr as f64).min(0.4999))
                    / math::tan(pi * (self.hz as f64 / self.sr as f64).min(0.49));
                let w4 = (w * w) * (w * w);
                // A⁴ per section is √G·√G = G.
                let sec = if band == Band::Lows {
                    (g + w4) / (1.0 + w4)
                } else {
                    (1.0 + g * w4) / (1.0 + w4)
                };
                sec * sec
            }
            (band, Crossover::LinearPhase) => {
                let d = dirichlet(hz as f64 / self.sr as f64, self.m);
                let s4 = (d * d) * (d * d);
                let h = if band == Band::Lows {
                    1.0 + (g - 1.0) * s4
                } else {
                    g + (1.0 - g) * s4
                };
                h * h
            }
        };
        (10.0 * math::log10(m2.max(1e-30))) as f32
    }
}

/// A boxcar of `m` samples at `f` (cycles per sample): `sin(πfm) / (m·sin(πf))`.
fn dirichlet(f: f64, m: usize) -> f64 {
    let pi = std::f64::consts::PI;
    let d = math::sin(pi * f);
    if d.abs() < 1e-12 {
        return 1.0;
    }
    math::sin(pi * f * m as f64) / (m as f64 * d)
}
