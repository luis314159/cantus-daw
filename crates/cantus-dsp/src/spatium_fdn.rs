//! Spatium's late field: a feedback delay network of [`LINES`] modulated delay lines, a
//! Hadamard mixing matrix, and an absorption filter on every line fitted so the decay
//! time follows a curve.
//!
//! ## Why the decay time follows the curve
//!
//! In a feedback delay network whose matrix is orthogonal (lossless), every bit of energy
//! lost is lost in the absorption filters. If line *i* is *mᵢ* samples long and its filter
//! attenuates by *Aᵢ(f)* dB per pass, the loop loses *Aᵢ(f)/mᵢ* dB per sample at *f*; for
//! that to be the same in every line — so every mode of the network decays alike — the
//! attenuation must be proportional to the length (Jot & Chaigne, 1991):
//!
//! ```text
//! Aᵢ(f) = mᵢ · γ(f),     γ(f) = −60 / (fs · T60(f))   dB per sample
//! ```
//!
//! ## How each filter is fitted
//!
//! Each line's filter is a cascade of a low shelf, eight octave bells (63 Hz … 8 kHz), a
//! bell at 12.5 kHz and a high shelf at 16 kHz, plus a broadband gain — twelve numbers, fitted to that line's target on a
//! third-octave grid from 20 Hz to 20 kHz. This follows Prawda, Välimäki & Schlecht,
//! *Improved Reverberation Time Control for Feedback Delay Networks* (DAFx 2019), who fit a
//! graphic equaliser per line by least squares through an interaction matrix:
//!
//! 1. The interaction matrix `B` holds each section's response in dB (at a prototype gain,
//!    scaled to 1 dB), plus a column of ones for the broadband gain, computed once per
//!    sample rate. Each line solves the weighted least squares `P = (BᵀWB)⁻¹BᵀW` with
//!    `W = 1/t²`, because the decay time's error is relative (see [`Absorber::fit`]).
//! 2. `g = P·t` is the first guess. Because a bell's shape changes a little with its gain,
//!    the real response is then evaluated and the residual fed back — `g += P·(t − a(g))`
//!    — four times. This is the refinement their paper and the accurate graphic
//!    equaliser (Välimäki & Liski, 2017) use; here it is a fixed number of steps so the
//!    cost is bounded.
//! 3. **The loop gain is kept below one** everywhere: the response is checked on a
//!    sixth-octave grid and at both ends, and if any point is above −0.0001 dB the
//!    broadband gain comes down by the excess. A request the filter cannot make (more than
//!    [`MAX_PASS_DB`] per pass) is clamped, and the editor shows the *achieved* decay next
//!    to the target, so a clamp is visible, not silent.
//!
//! ## Why the tail does not ring
//!
//! Line lengths are distinct primes spread geometrically over the room's span, so no two
//! loops share a period; each delay is read through a first-order **all-pass
//! interpolator** whose length is modulated by its own slow sine (Character sets depth and
//! rate). An all-pass interpolator is lossless at every frequency, so the modulation that
//! smears the modes costs no high-frequency decay — the fitted curve stays exact. With no
//! modulation and a whole-sample length the interpolator reduces to a plain delay.
//!
//! **Freeze** cross-fades each line from its filtered output to its raw output times
//! 0.99999: a loop that keeps what it holds (for tens of thousands of seconds) and takes
//! nothing new. The cross-fade is a convex blend of two gains below one, so it is stable
//! at every point of the fade.

// The DSP here walks several parallel arrays (lines, sections, grid points) with one
// index; that reads clearer than zipping five iterators.
#![allow(clippy::needless_range_loop)]

use crate::spatium::model::TargetCurve;
use crate::EqShape;

pub const LINES: usize = 16;
pub const SECTIONS: usize = 11;
/// Third-octave points from 20 Hz: the fit's grid (fewer at low sample rates).
pub const GRID: usize = 31;
const CHECK: usize = 2 * GRID + 2;
/// The most a line's filter is asked to attenuate in one pass, dB.
pub const MAX_PASS_DB: f64 = 40.0;
/// How far below 0 dB every line's loop gain is held, dB.
const SAFETY_DB: f64 = 1e-4;
const ITERATIONS: usize = 4;
/// The gain the interaction matrix is measured at, dB.
const PROTO_DB: f64 = -3.0;
/// A frozen loop's gain per pass.
pub const FREEZE_GAIN: f32 = 0.999_99;

/// The absorption filter's sections: shape, frequency, Q.
pub const BANDS: [(EqShape, f32, f32); SECTIONS] = [
    (EqShape::LowShelf, 44.0, 0.7),
    (EqShape::Bell, 63.0, 0.9),
    (EqShape::Bell, 125.0, 0.9),
    (EqShape::Bell, 250.0, 0.9),
    (EqShape::Bell, 500.0, 0.9),
    (EqShape::Bell, 1_000.0, 0.9),
    (EqShape::Bell, 2_000.0, 0.9),
    (EqShape::Bell, 4_000.0, 0.9),
    (EqShape::Bell, 8_000.0, 0.9),
    (EqShape::Bell, 12_500.0, 1.1),
    (EqShape::HighShelf, 16_000.0, 0.7),
];

/// `cos ω, sin ω, cos 2ω, sin 2ω` for a frequency.
fn trig(sample_rate: f32, hz: f64) -> [f64; 4] {
    let w = 2.0 * std::f64::consts::PI * hz / sample_rate as f64;
    [w.cos(), w.sin(), (2.0 * w).cos(), (2.0 * w).sin()]
}

/// One absorption section in the form that stays exact at small gains and low
/// frequencies: `H(z) = 1 + D(z)/A(z)`, with `D = B − A` and the poles stored as their
/// distance from `z = 1` (`e1 = a1 + 2`, `e2 = a2 − 1`).
///
/// The direct form loses the bass: a 44 Hz shelf's `1 + a1 + a2` is about 3·10⁻⁵ at
/// 48 kHz, so one f32 step of `a1` moves its DC gain by 0.03 dB — 18 % of the 0.17 dB a
/// long line loses per pass there. Here a section with little to do has small
/// coefficients, each exact to f32's relative precision, and the error scales with the
/// section's own effect instead of with unity.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Section {
    pub d0: f32,
    pub d1: f32,
    pub d2: f32,
    pub e1: f32,
    pub e2: f32,
}

impl Section {
    pub const IDENTITY: Section = Section {
        d0: 0.0,
        d1: 0.0,
        d2: 0.0,
        e1: 2.0,
        e2: 0.0,
    };

    /// From a biquad designed in f64 (`a0` normalised to 1).
    fn from_biquad(b: [f64; 3], a: [f64; 2]) -> Section {
        Section {
            d0: (b[0] - 1.0) as f32,
            d1: (b[1] - a[0]) as f32,
            d2: (b[2] - a[1]) as f32,
            e1: (a[0] + 2.0) as f32,
            e2: (a[1] - 1.0) as f32,
        }
    }

    /// `|H|²` at precomputed trig values, from the stored (rounded) numbers — what the
    /// audio thread really runs.
    #[inline]
    fn mag2(&self, t: &[f64; 4]) -> f64 {
        let a1 = self.e1 as f64 - 2.0;
        let a2 = self.e2 as f64 + 1.0;
        let (d0, d1, d2) = (self.d0 as f64, self.d1 as f64, self.d2 as f64);
        // B = D + A.
        let (b0, b1, b2) = (d0 + 1.0, d1 + a1, d2 + a2);
        let nr = b0 + b1 * t[0] + b2 * t[2];
        let ni = b1 * t[1] + b2 * t[3];
        let dr = 1.0 + a1 * t[0] + a2 * t[2];
        let di = a1 * t[1] + a2 * t[3];
        (nr * nr + ni * ni) / (dr * dr + di * di).max(1e-300)
    }

    /// Magnitude at `hz`, dB.
    pub fn magnitude_db(&self, sample_rate: f32, hz: f32) -> f32 {
        (10.0 * self.mag2(&trig(sample_rate, hz as f64)).max(1e-300).log10()) as f32
    }
}

/// A shelf or a bell by the bilinear cookbook (Bristow-Johnson), in f64.
fn section(shape: EqShape, sample_rate: f32, hz: f32, gain_db: f32, q: f32) -> Section {
    if gain_db == 0.0 {
        return Section::IDENTITY;
    }
    let w0 = 2.0 * std::f64::consts::PI * (hz as f64).min(0.45 * sample_rate as f64)
        / sample_rate as f64;
    let (sn, cs) = w0.sin_cos();
    let a = 10f64.powf(gain_db as f64 / 40.0);
    let alpha = sn / (2.0 * q as f64);
    let sa = 2.0 * a.sqrt() * alpha;
    let (b, den) = match shape {
        EqShape::LowShelf => (
            [
                a * ((a + 1.0) - (a - 1.0) * cs + sa),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cs),
                a * ((a + 1.0) - (a - 1.0) * cs - sa),
            ],
            [
                (a + 1.0) + (a - 1.0) * cs + sa,
                -2.0 * ((a - 1.0) + (a + 1.0) * cs),
                (a + 1.0) + (a - 1.0) * cs - sa,
            ],
        ),
        EqShape::HighShelf => (
            [
                a * ((a + 1.0) + (a - 1.0) * cs + sa),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
                a * ((a + 1.0) + (a - 1.0) * cs - sa),
            ],
            [
                (a + 1.0) - (a - 1.0) * cs + sa,
                2.0 * ((a - 1.0) - (a + 1.0) * cs),
                (a + 1.0) - (a - 1.0) * cs - sa,
            ],
        ),
        _ => (
            [1.0 + alpha * a, -2.0 * cs, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * cs, 1.0 - alpha / a],
        ),
    };
    let n = den[0];
    Section::from_biquad([b[0] / n, b[1] / n, b[2] / n], [den[1] / n, den[2] / n])
}

/// Response of a cascade and a broadband gain (dB) at precomputed trig values, dB.
fn cascade_db(coefs: &[Section; SECTIONS], g0_db: f64, t: &[f64; 4]) -> f64 {
    let mut m = 1.0;
    for c in coefs {
        m *= c.mag2(t);
    }
    10.0 * m.max(1e-300).log10() + g0_db
}

/// The fitter: the grid and the interaction matrix's inverse for one sample rate.
#[derive(Clone, Debug)]
pub struct Absorber {
    sample_rate: f32,
    points: usize,
    grid_hz: [f64; GRID],
    grid: [[f64; 4]; GRID],
    check: [[f64; 4]; CHECK],
    check_points: usize,
    /// The interaction matrix: each section's response (dB per dB of gain) at each grid
    /// point, and a column of ones for the broadband gain.
    b: [[f64; SECTIONS + 1]; GRID],
}

impl Absorber {
    pub fn new(sample_rate: f32) -> Self {
        let top = (0.45 * sample_rate as f64).min(20_000.0);
        let mut grid_hz = [0.0; GRID];
        let mut points = 0;
        for (j, g) in grid_hz.iter_mut().enumerate() {
            let f = 20.0 * 2f64.powf(j as f64 / 3.0);
            if f <= top * 1.001 {
                *g = f;
                points = j + 1;
            }
        }
        let mut grid = [[0.0; 4]; GRID];
        for j in 0..points {
            grid[j] = trig(sample_rate, grid_hz[j]);
        }
        // Check points: the grid, the midpoints, and both ends of the spectrum.
        let mut check = [[0.0; 4]; CHECK];
        let mut n = 0;
        for j in 0..points {
            check[n] = grid[j];
            n += 1;
            if j + 1 < points {
                check[n] = trig(sample_rate, (grid_hz[j] * grid_hz[j + 1]).sqrt());
                n += 1;
            }
        }
        check[n] = trig(sample_rate, 5.0);
        n += 1;
        check[n] = trig(sample_rate, 0.499 * sample_rate as f64);
        n += 1;

        // The interaction matrix at the prototype gain, one dB per unit.
        let k = SECTIONS + 1;
        let mut b = [[0.0f64; SECTIONS + 1]; GRID];
        for (s, (shape, hz, q)) in BANDS.iter().enumerate() {
            let c = section(*shape, sample_rate, *hz, PROTO_DB as f32, *q);
            for j in 0..points {
                b[j][s] = 10.0 * c.mag2(&grid[j]).log10() / PROTO_DB;
            }
        }
        for row in b.iter_mut().take(points) {
            row[SECTIONS] = 1.0;
        }
        let _ = k;
        Absorber {
            sample_rate,
            points,
            grid_hz,
            grid,
            check,
            check_points: n,
            b,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// The fit's grid, Hz.
    pub fn grid_hz(&self) -> &[f64] {
        &self.grid_hz[..self.points]
    }

    /// Fit one line's filter to a target (dB per pass at each grid point). Returns the
    /// broadband gain, linear; the sections are written to `out`.
    ///
    /// The least squares is **weighted by 1/t²**: what has to be right is the decay *time*,
    /// and its error is relative — 0.05 dB off a 0.5 dB loss is 10 %, off a 10 dB loss
    /// half a percent. Unweighted, the fit spends its accuracy on the large losses of the
    /// short high end and the long bass comes out wrong.
    pub fn fit(&self, target_db: &[f64; GRID], out: &mut [Section; SECTIONS]) -> f32 {
        const K: usize = SECTIONS + 1;
        let n = self.points;
        // P = (BᵀWB + λI)⁻¹ BᵀW for this line's weights.
        let mut w = [0.0f64; GRID];
        for j in 0..n {
            w[j] = 1.0 / target_db[j].abs().max(1e-3).powi(2);
        }
        let mut m = [[0.0f64; K]; K];
        for r in 0..K {
            for c in r..K {
                let v: f64 = (0..n).map(|j| self.b[j][r] * w[j] * self.b[j][c]).sum();
                m[r][c] = v;
                m[c][r] = v;
            }
        }
        let scale = (0..K).map(|r| m[r][r]).fold(0.0f64, f64::max);
        for (r, row) in m.iter_mut().enumerate() {
            row[r] += 1e-9 * scale;
        }
        let inv = invert(m);
        let mut p = [[0.0f64; GRID]; K];
        for r in 0..K {
            for j in 0..n {
                p[r][j] = (0..K).map(|c| inv[r][c] * self.b[j][c]).sum::<f64>() * w[j];
            }
        }
        let mut g = [0.0f64; K];
        for (k, gk) in g.iter_mut().enumerate() {
            *gk = (0..n).map(|j| p[k][j] * target_db[j]).sum();
        }
        for _ in 0..ITERATIONS {
            self.sections(&g, out);
            let mut e = [0.0f64; GRID];
            for j in 0..n {
                e[j] = target_db[j] - cascade_db(out, g[SECTIONS], &self.grid[j]);
            }
            for (k, gk) in g.iter_mut().enumerate() {
                *gk += (0..n).map(|j| p[k][j] * e[j]).sum::<f64>();
                *gk = gk.clamp(-80.0, 24.0);
            }
        }
        self.sections(&g, out);
        // Hold the loop gain below one everywhere.
        let mut peak = f64::MIN;
        for t in &self.check[..self.check_points] {
            peak = peak.max(cascade_db(out, g[SECTIONS], t));
        }
        if peak > -SAFETY_DB {
            g[SECTIONS] -= peak + SAFETY_DB;
        }
        10f64.powf(g[SECTIONS] / 20.0) as f32
    }

    fn sections(&self, g: &[f64; SECTIONS + 1], out: &mut [Section; SECTIONS]) {
        for (s, (shape, hz, q)) in BANDS.iter().enumerate() {
            out[s] = section(*shape, self.sample_rate, *hz, g[s] as f32, *q);
        }
    }
}

/// Gauss–Jordan inverse of a small symmetric positive matrix.
fn invert<const K: usize>(mut m: [[f64; K]; K]) -> [[f64; K]; K] {
    let mut inv = [[0.0; K]; K];
    for (i, row) in inv.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for col in 0..K {
        let pivot = (col..K)
            .max_by(|a, b| m[*a][col].abs().total_cmp(&m[*b][col].abs()))
            .unwrap_or(col);
        m.swap(col, pivot);
        inv.swap(col, pivot);
        let d = m[col][col];
        if d.abs() < 1e-300 {
            continue;
        }
        for c in 0..K {
            m[col][c] /= d;
            inv[col][c] /= d;
        }
        for r in 0..K {
            if r != col {
                let f = m[r][col];
                if f != 0.0 {
                    for c in 0..K {
                        m[r][c] -= f * m[col][c];
                        inv[r][c] -= f * inv[col][c];
                    }
                }
            }
        }
    }
    inv
}

/// Everything the late field is set to: line lengths and each line's fitted filter.
#[derive(Clone, Debug, PartialEq)]
pub struct LateDesign {
    pub sample_rate: f32,
    /// Samples, whole numbers.
    pub lengths: [f32; LINES],
    pub coefs: [[Section; SECTIONS]; LINES],
    pub g0: [f32; LINES],
}

impl LateDesign {
    /// A wire: identity filters (used before the first design).
    pub fn identity(sample_rate: f32) -> Self {
        LateDesign {
            sample_rate,
            lengths: [1000.0; LINES],
            coefs: [[Section::IDENTITY; SECTIONS]; LINES],
            g0: [0.5; LINES],
        }
    }

    /// Design for a target curve and a set of line lengths (samples).
    pub fn new(absorber: &Absorber, curve: &TargetCurve, lengths: &[f32; LINES]) -> Self {
        let fs = absorber.sample_rate as f64;
        // γ at each grid point, dB per sample.
        let mut gamma = [0.0f64; GRID];
        for (j, hz) in absorber.grid_hz().iter().enumerate() {
            gamma[j] = -60.0 / (fs * curve.t60(*hz as f32) as f64);
        }
        let mut coefs = [[Section::IDENTITY; SECTIONS]; LINES];
        let mut g0 = [1.0; LINES];
        for i in 0..LINES {
            let m = lengths[i] as f64;
            let mut t = [0.0f64; GRID];
            for j in 0..absorber.points {
                t[j] = (m * gamma[j]).clamp(-MAX_PASS_DB, -SAFETY_DB * 2.0);
            }
            g0[i] = absorber.fit(&t, &mut coefs[i]);
        }
        LateDesign {
            sample_rate: absorber.sample_rate,
            lengths: *lengths,
            coefs,
            g0,
        }
    }

    /// Line `i`'s attenuation per pass at `hz`, dB (negative).
    pub fn pass_db(&self, i: usize, hz: f32) -> f64 {
        let t = trig(self.sample_rate, hz as f64);
        cascade_db(&self.coefs[i], 20.0 * (self.g0[i] as f64).log10(), &t)
    }

    /// Line `i`'s own decay time at `hz`, seconds.
    pub fn line_t60(&self, i: usize, hz: f32) -> f32 {
        let per_sample = self.pass_db(i, hz) / self.lengths[i] as f64;
        (-60.0 / (self.sample_rate as f64 * per_sample.min(-1e-12))) as f32
    }

    /// The decay the network achieves at `hz`, seconds: from the mean loss per sample over
    /// the lines, which is how the network's modes decay on average.
    pub fn achieved_t60(&self, hz: f32) -> f32 {
        let t = trig(self.sample_rate, hz as f64);
        let mut per_sample = 0.0;
        for i in 0..LINES {
            per_sample += cascade_db(&self.coefs[i], 20.0 * (self.g0[i] as f64).log10(), &t)
                / self.lengths[i] as f64;
        }
        per_sample /= LINES as f64;
        (-60.0 / (self.sample_rate as f64 * per_sample.min(-1e-12))) as f32
    }
}

fn is_prime(n: u32) -> bool {
    if n < 2 {
        return false;
    }
    if n.is_multiple_of(2) {
        return n == 2;
    }
    let mut d = 3;
    while d * d <= n {
        if n.is_multiple_of(d) {
            return false;
        }
        d += 2;
    }
    true
}

/// Lengths in samples: each the nearest prime to its target not already taken.
pub fn prime_lengths(ms: &[f32; LINES], sample_rate: f32) -> [f32; LINES] {
    let mut out = [0.0f32; LINES];
    for i in 0..LINES {
        let target = (ms[i] * 0.001 * sample_rate).max(8.0) as u32;
        let mut best = target;
        for step in 0..10_000u32 {
            let taken = |n: u32| out[..i].iter().any(|x| *x as u32 == n);
            let up = target + step;
            if is_prime(up) && !taken(up) {
                best = up;
                break;
            }
            if step <= target {
                let down = target - step;
                if is_prime(down) && !taken(down) {
                    best = down;
                    break;
                }
            }
        }
        out[i] = best as f32;
    }
    out
}

/// One step of a one-pole glide that always lands: in f32 a step of `(target − v)·k`
/// falls under half an ulp before `v` reaches the target (near 1.0 that happens 7·10⁻⁵
/// short), and a glide that stops short never lands. So it snaps when the step no longer
/// moves the value, or when it is within `eps`.
#[inline]
pub fn glide(v: &mut f32, target: f32, k: f32, eps: f32) {
    if *v != target {
        let next = *v + (target - *v) * k;
        *v = if next == *v || (next - target).abs() < eps {
            target
        } else {
            next
        };
    }
}

/// The feedback matrix: signs, then the Hadamard transform, then a permutation —
/// `A = P·H·D`, still orthogonal. The Sylvester–Hadamard matrix alone is an involution
/// (`A·A = I`): whatever leaves a line comes back to it, all of it, two passes later, and
/// a still tail shows it as a repetition at a line's period. Signs and a permutation
/// break that and cost nothing.
const SIGNS: [f32; LINES] = [
    1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0,
];
const PERMUTE: [usize; LINES] = [5, 12, 1, 9, 14, 3, 7, 0, 10, 15, 6, 2, 13, 8, 4, 11];

/// Entry `j` of row `r` of the 16×16 Sylvester–Hadamard matrix.
fn hadamard_entry(r: usize, j: usize) -> f32 {
    if (r & j).count_ones().is_multiple_of(2) {
        1.0
    } else {
        -1.0
    }
}

/// In-place fast Walsh–Hadamard transform, normalised so the matrix is orthogonal.
#[inline]
fn hadamard(v: &mut [f32; LINES]) {
    let mut h = 1;
    while h < LINES {
        let mut i = 0;
        while i < LINES {
            for j in i..i + h {
                let a = v[j];
                let b = v[j + h];
                v[j] = a + b;
                v[j + h] = a - b;
            }
            i += 2 * h;
        }
        h *= 2;
    }
    for x in v.iter_mut() {
        *x *= 0.25;
    }
}

/// The network's running state.
#[derive(Clone, Debug)]
pub struct Fdn {
    size: usize,
    mask: usize,
    buf: Vec<f32>,
    w: usize,
    len: [f32; LINES],
    len_target: [f32; LINES],
    glide: f32,
    ap: [f32; LINES],
    lfo_re: [f32; LINES],
    lfo_im: [f32; LINES],
    rot_c: [f32; LINES],
    rot_s: [f32; LINES],
    depth_scale: [f32; LINES],
    depth: f32,
    depth_target: f32,
    smooth: f32,
    d0: [[f32; LINES]; SECTIONS],
    d1: [[f32; LINES]; SECTIONS],
    d2: [[f32; LINES]; SECTIONS],
    e1: [[f32; LINES]; SECTIONS],
    e2: [[f32; LINES]; SECTIONS],
    x1: [[f32; LINES]; SECTIONS],
    x2: [[f32; LINES]; SECTIONS],
    y1: [[f32; LINES]; SECTIONS],
    y2: [[f32; LINES]; SECTIONS],
    g0: [f32; LINES],
    freeze: f32,
    freeze_target: f32,
    /// Freeze's level hold: the energy of the lines (smoothed), the level it is held at,
    /// and samples since the loop closed (the reference is taken once it has settled).
    energy: f32,
    energy_ref: f32,
    energy_coef: f32,
    frozen_for: u32,
    b_l: [f32; LINES],
    b_r: [f32; LINES],
    c_l: [f32; LINES],
    c_r: [f32; LINES],
    sample_rate: f32,
    configured: bool,
}

impl Fdn {
    /// A network whose lines can reach `max_ms` (plus modulation).
    pub fn new(sample_rate: f32, max_ms: f32) -> Self {
        let need = (sample_rate * (max_ms + 12.0) * 0.001) as usize + 8;
        let size = need.next_power_of_two();
        let mut depth_scale = [0.0; LINES];
        let mut b_l = [0.0; LINES];
        let mut b_r = [0.0; LINES];
        let mut c_l = [0.0; LINES];
        let mut c_r = [0.0; LINES];
        for i in 0..LINES {
            depth_scale[i] = 0.6 + 0.4 * crate::spatium::model::hash01(i as u32, 71);
            // Four different rows of the Hadamard matrix: orthogonal, so the two inputs
            // excite different patterns and the two outputs are decorrelated.
            b_l[i] = 0.25 * hadamard_entry(3, i);
            b_r[i] = 0.25 * hadamard_entry(5, i);
            c_l[i] = 0.25 * hadamard_entry(6, i);
            c_r[i] = 0.25 * hadamard_entry(9, i);
        }
        let mut f = Fdn {
            size,
            mask: size - 1,
            buf: vec![0.0; size * LINES],
            w: 0,
            len: [1000.0; LINES],
            len_target: [1000.0; LINES],
            glide: 1.0 - (-1.0 / (0.08 * sample_rate)).exp(),
            ap: [0.0; LINES],
            lfo_re: [1.0; LINES],
            lfo_im: [0.0; LINES],
            rot_c: [1.0; LINES],
            rot_s: [0.0; LINES],
            depth_scale,
            depth: 0.0,
            depth_target: 0.0,
            smooth: 1.0 - (-1.0 / (0.05 * sample_rate)).exp(),
            d0: [[0.0; LINES]; SECTIONS],
            d1: [[0.0; LINES]; SECTIONS],
            d2: [[0.0; LINES]; SECTIONS],
            e1: [[2.0; LINES]; SECTIONS],
            e2: [[0.0; LINES]; SECTIONS],
            x1: [[0.0; LINES]; SECTIONS],
            x2: [[0.0; LINES]; SECTIONS],
            y1: [[0.0; LINES]; SECTIONS],
            y2: [[0.0; LINES]; SECTIONS],
            g0: [0.5; LINES],
            freeze: 0.0,
            freeze_target: 0.0,
            energy: 0.0,
            energy_ref: 0.0,
            energy_coef: 1.0 - (-1.0 / (0.1 * sample_rate)).exp(),
            frozen_for: 0,
            b_l,
            b_r,
            c_l,
            c_r,
            sample_rate,
            configured: false,
        };
        f.reset_phases();
        f
    }

    fn reset_phases(&mut self) {
        for i in 0..LINES {
            // Spread the phases by the golden angle so no two lines move together.
            let ph = i as f32 * 2.399_963;
            self.lfo_re[i] = ph.cos();
            self.lfo_im[i] = ph.sin();
        }
    }

    /// The longest line the buffers hold, samples.
    pub fn capacity(&self) -> usize {
        self.size - 8
    }

    pub fn reset(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.w = 0;
        self.ap = [0.0; LINES];
        self.x1 = [[0.0; LINES]; SECTIONS];
        self.x2 = [[0.0; LINES]; SECTIONS];
        self.y1 = [[0.0; LINES]; SECTIONS];
        self.y2 = [[0.0; LINES]; SECTIONS];
        self.len = self.len_target;
        self.depth = self.depth_target;
        self.freeze = self.freeze_target;
        self.energy = 0.0;
        self.energy_ref = 0.0;
        self.frozen_for = 0;
        self.reset_phases();
    }

    /// Apply a design: lengths glide to their new values, filters take theirs at once.
    pub fn set_design(&mut self, d: &LateDesign) {
        let cap = self.capacity() as f32;
        for i in 0..LINES {
            self.len_target[i] = d.lengths[i].clamp(8.0, cap - 4.0);
            for k in 0..SECTIONS {
                let c = d.coefs[i][k];
                self.d0[k][i] = c.d0;
                self.d1[k][i] = c.d1;
                self.d2[k][i] = c.d2;
                self.e1[k][i] = c.e1;
                self.e2[k][i] = c.e2;
            }
            self.g0[i] = d.g0[i];
        }
        if !self.configured {
            self.len = self.len_target;
            self.configured = true;
        }
    }

    /// Modulation: depth in samples, rate in Hz.
    pub fn set_modulation(&mut self, depth_samples: f32, rate_hz: f32) {
        self.depth_target = depth_samples.max(0.0);
        for i in 0..LINES {
            let r = rate_hz * (0.55 + 0.9 * crate::spatium::model::hash01(i as u32, 97));
            let w = 2.0 * std::f32::consts::PI * r / self.sample_rate;
            self.rot_c[i] = w.cos();
            self.rot_s[i] = w.sin();
        }
    }

    pub fn set_freeze(&mut self, on: bool) {
        self.freeze_target = if on { 1.0 } else { 0.0 };
    }

    /// How frozen the loop is now, 0…1.
    pub fn freeze_amount(&self) -> f32 {
        self.freeze
    }

    /// The mean line length now, samples.
    pub fn mean_length(&self) -> f32 {
        self.len.iter().sum::<f32>() / LINES as f32
    }

    pub fn depth(&self) -> f32 {
        self.depth
    }

    /// Freeze's gain correction for this sample. The all-pass interpolator is lossless
    /// for a fixed delay, but its steps between whole samples while the modulation moves
    /// are not quite: a frozen tail would sink by about 0.4 dB a second. So once the loop
    /// has been closed for 150 ms its energy is taken as the reference, and the loop gain
    /// is trimmed toward it by at most 0.2 % per pass either way: a level hold, slow
    /// enough not to be heard and bounded so it can never run away.
    fn hold_level(&mut self, y: &[f32; LINES]) -> f32 {
        let e: f32 = y.iter().map(|x| x * x).sum();
        self.energy += (e - self.energy) * self.energy_coef;
        if self.freeze < 1.0 {
            return 1.0;
        }
        self.frozen_for = self.frozen_for.saturating_add(1);
        if self.energy_ref == 0.0 {
            if self.frozen_for as f32 >= 0.15 * self.sample_rate {
                self.energy_ref = self.energy.max(1e-30);
            }
            return 1.0;
        }
        if self.energy < 1e-24 {
            return 1.0;
        }
        (1.0 + 0.5 * (self.energy_ref / self.energy - 1.0)).clamp(0.998, 1.002)
    }

    /// One sample: the stereo input in, the stereo late field out.
    #[inline]
    pub fn tick(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let size = self.size;
        let mask = self.mask;
        let w = self.w;
        glide(&mut self.depth, self.depth_target, self.smooth, 1e-4);
        let modulated = self.depth > 0.0;
        let mut y = [0.0f32; LINES];
        for i in 0..LINES {
            glide(&mut self.len[i], self.len_target[i], self.glide, 1e-3);
            let mut d = self.len[i];
            if modulated {
                let (c, s) = (self.rot_c[i], self.rot_s[i]);
                let re = self.lfo_re[i] * c - self.lfo_im[i] * s;
                let im = self.lfo_re[i] * s + self.lfo_im[i] * c;
                let k = 1.5 - 0.5 * (re * re + im * im);
                self.lfo_re[i] = re * k;
                self.lfo_im[i] = im * k;
                let depth = (self.depth * self.depth_scale[i])
                    .min(0.45 * d - 2.0)
                    .max(0.0);
                d += depth * self.lfo_im[i];
            }
            let nf = (d - 0.5).floor();
            let frac = d - nf;
            let n = nf as usize;
            let base = i * size;
            let x0 = self.buf[base + (w.wrapping_sub(n) & mask)];
            let x1 = self.buf[base + (w.wrapping_sub(n + 1) & mask)];
            let out = if frac == 1.0 {
                x1
            } else {
                let eta = (1.0 - frac) / (1.0 + frac);
                eta * (x0 - self.ap[i]) + x1
            };
            let out = if out.abs() < 1e-18 { 0.0 } else { out };
            self.ap[i] = out;
            y[i] = out;
        }
        // Absorption: section by section across all lines (vectorises).
        let mut v = y;
        for k in 0..SECTIONS {
            let (d0, d1, d2, e1, e2) = (
                &self.d0[k],
                &self.d1[k],
                &self.d2[k],
                &self.e1[k],
                &self.e2[k],
            );
            let (x1, x2, v1, v2) = (
                &mut self.x1[k],
                &mut self.x2[k],
                &mut self.y1[k],
                &mut self.y2[k],
            );
            for i in 0..LINES {
                let x = v[i];
                // v = D/A · x, then y = x + v (see `Section`).
                let d = d0[i] * x + d1[i] * x1[i] + d2[i] * x2[i] + 2.0 * v1[i]
                    - v2[i]
                    - e1[i] * v1[i]
                    - e2[i] * v2[i];
                let d = if d.abs() < 1e-18 { 0.0 } else { d };
                x2[i] = x1[i];
                x1[i] = x;
                v2[i] = v1[i];
                v1[i] = d;
                v[i] = x + d;
            }
        }
        for i in 0..LINES {
            v[i] *= self.g0[i];
        }
        glide(&mut self.freeze, self.freeze_target, self.smooth, 1e-5);
        if self.freeze > 0.0 {
            let g = FREEZE_GAIN * self.hold_level(&y);
            let f = self.freeze;
            for i in 0..LINES {
                v[i] += (y[i] * g - v[i]) * f;
            }
        } else if self.frozen_for != 0 {
            self.frozen_for = 0;
            self.energy_ref = 0.0;
        }
        let mut ol = 0.0;
        let mut or = 0.0;
        for i in 0..LINES {
            ol += self.c_l[i] * v[i];
            or += self.c_r[i] * v[i];
        }
        for i in 0..LINES {
            v[i] *= SIGNS[i];
        }
        hadamard(&mut v);
        for i in 0..LINES {
            self.buf[i * size + w] = v[PERMUTE[i]] + self.b_l[i] * in_l + self.b_r[i] * in_r;
        }
        self.w = (w + 1) & mask;
        (ol, or)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatium::model::{DecayBand, Style, DECAY_BANDS};

    #[test]
    fn the_hadamard_matrix_is_orthogonal() {
        for r in 0..LINES {
            let mut v = [0.0; LINES];
            v[r] = 1.0;
            hadamard(&mut v);
            let e: f32 = v.iter().map(|x| x * x).sum();
            assert!((e - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn the_feedback_matrix_is_orthogonal_and_not_an_involution() {
        let apply = |x: [f32; LINES]| -> [f32; LINES] {
            let mut v = x;
            for i in 0..LINES {
                v[i] *= SIGNS[i];
            }
            hadamard(&mut v);
            let mut out = [0.0; LINES];
            for i in 0..LINES {
                out[i] = v[PERMUTE[i]];
            }
            out
        };
        let mut sorted = PERMUTE;
        sorted.sort();
        assert!(
            sorted.iter().enumerate().all(|(i, p)| i == *p),
            "a permutation"
        );
        for r in 0..LINES {
            let mut e = [0.0; LINES];
            e[r] = 1.0;
            let once = apply(e);
            assert!((once.iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 1e-6);
            // Two passes do not come back to where they started.
            let twice = apply(once);
            assert!(twice[r].abs() < 0.9, "row {r} returns {}", twice[r]);
        }
    }

    #[test]
    fn a_glide_lands_on_its_target() {
        let mut v = 0.0f32;
        for _ in 0..200_000 {
            glide(&mut v, 1.0, 4e-4, 1e-6);
        }
        assert_eq!(v, 1.0);
        let mut v = 1000.3f32;
        for _ in 0..200_000 {
            glide(&mut v, 1009.0, 2.6e-4, 1e-3);
        }
        assert_eq!(v, 1009.0);
    }

    #[test]
    fn lengths_are_distinct_primes() {
        let ms = crate::spatium::model::line_lengths_ms::<LINES>(
            &crate::spatium::model::room_at(0.5),
            Style::Room,
        );
        let l = prime_lengths(&ms, 48_000.0);
        for i in 0..LINES {
            assert!(is_prime(l[i] as u32));
            for j in 0..i {
                assert_ne!(l[i], l[j]);
            }
        }
    }

    /// The fit, against the target it was given: at the grid points and between them,
    /// for rooms from small to vast and for bright and dark settings, every line's decay
    /// time is within 4 % of the target from 50 Hz to 16 kHz — under the 5 % a listener
    /// can notice (ISO 3382-1). Measured worst: 3.3 %, at 5 kHz on the darkest setting.
    #[test]
    fn every_line_decays_at_the_target_rate() {
        let a = Absorber::new(48_000.0);
        let bands = [DecayBand::default(); DECAY_BANDS];
        let mut worst = 0.0f32;
        for (space, decay, bright) in [
            (0.1, 0.4, 0.45),
            (0.5, 2.0, 0.45),
            (0.8, 5.0, 0.2),
            (1.0, 12.0, 0.8),
            (0.5, 1.2, 0.0),
        ] {
            let curve = TargetCurve::new(Style::Room, space, decay, bright, true, &bands);
            let ms = crate::spatium::model::line_lengths_ms::<LINES>(&curve.room, Style::Room);
            let lengths = prime_lengths(&ms, 48_000.0);
            let d = LateDesign::new(&a, &curve, &lengths);
            for i in 0..LINES {
                for k in 0..=26 {
                    let hz = 50.0 * 2f32.powf(k as f32 / 3.0);
                    if hz > 16_000.0 {
                        continue;
                    }
                    let want = curve.t60(hz);
                    let got = d.line_t60(i, hz);
                    let err = (got / want - 1.0).abs();
                    if err > worst {
                        worst = err;
                        eprintln!(
                            "space {space} decay {decay}: line {i} at {hz:.0} Hz: {got:.3} s for {want:.3} s"
                        );
                    }
                }
            }
        }
        eprintln!("worst line error {:.2} %", worst * 100.0);
        assert!(worst < 0.04, "worst line error {:.2} %", worst * 100.0);
    }

    #[test]
    fn the_loop_gain_stays_below_one() {
        let a = Absorber::new(44_100.0);
        let mut bands = [DecayBand::default(); DECAY_BANDS];
        // A steep request: long highs on top of a short decay, a deep notch.
        bands[0] = DecayBand {
            active: true,
            shape: crate::spatium::model::DecayShape::HighShelf,
            freq: 6_000.0,
            mult: 4.0,
            q: 2.0,
        };
        bands[1] = DecayBand {
            active: true,
            shape: crate::spatium::model::DecayShape::Notch,
            freq: 150.0,
            mult: 1.0,
            q: 4.0,
        };
        let curve = TargetCurve::new(Style::Room, 0.9, 30.0, 1.0, true, &bands);
        let ms = crate::spatium::model::line_lengths_ms::<LINES>(&curve.room, Style::Room);
        let d = LateDesign::new(&a, &curve, &prime_lengths(&ms, 44_100.0));
        for i in 0..LINES {
            for k in 0..2000 {
                let hz = 1.0 + k as f32 * 11.0;
                assert!(d.pass_db(i, hz) < 0.0, "line {i} gains at {hz} Hz");
            }
        }
    }
}
