//! Gravis's synth filters: a transistor ladder, a diode ladder (the TB-303's) and a
//! state-variable filter, all zero-delay-feedback (trapezoidal, pre-warped) with `f64`
//! state, because a bass filter lives at 30–300 Hz where direct-form biquads in `f32`
//! quantise badly (`research/instruments/bass.md` §4.4; Zavalishin, *The Art of VA Filter
//! Design*).
//!
//! **Bass compensation.** A ladder with feedback `k` passes DC at `1/(1+k)`: at
//! self-oscillation the lows drop 14 dB, which is why "resonance kills the bass". The input
//! is scaled by `1 + comp·k`, so at 100 % the passband is exactly unity at any resonance.
//!
//! **Nonlinearity** is the cheap ZDF kind: the linear loop is solved for the output, the
//! feedback node is saturated with that estimate, and the stages then run on the saturated
//! input. Stable at any resonance; self-oscillation is bounded.

/// Which filter the synth runs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum FilterKind {
    #[default]
    Ladder,
    Diode,
    SvfLow,
    SvfBand,
}

impl FilterKind {
    pub fn from_index(i: f32) -> Self {
        match i.round() as i32 {
            1 => FilterKind::Diode,
            2 => FilterKind::SvfLow,
            3 => FilterKind::SvfBand,
            _ => FilterKind::Ladder,
        }
    }
}

/// A soft saturator with unit slope at zero.
#[inline]
fn sat(x: f64) -> f64 {
    // x / sqrt(1 + x²): tanh-like, cheaper, and smooth.
    x / (1.0 + x * x).sqrt()
}

/// The ladder's feedback at 100 % resonance: just past self-oscillation (k = 4).
pub const LADDER_K_MAX: f64 = 4.1;

/// The level the filter core runs at, relative to the signal.
const HEADROOM: f64 = 0.15;

/// The diode ladder, as a passive RC ladder of four equal sections loaded by each other:
/// `v' = ωc (A v + b u)`. Its poles are spread, which is the 303's softer slope near the
/// cutoff.
const DIODE_A: [[f64; 4]; 4] = [
    [-2.0, 1.0, 0.0, 0.0],
    [1.0, -2.0, 1.0, 0.0],
    [0.0, 1.0, -2.0, 1.0],
    [0.0, 0.0, 1.0, -1.0],
];

/// The complex response of the open diode ladder at `ω` (with ωc = 1).
pub fn diode_response(w: f64) -> (f64, f64) {
    // Solve (jω I − A) v = b for v4, in complex numbers, by Gaussian elimination.
    let mut m = [[(0.0f64, 0.0f64); 5]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, c) in row.iter_mut().take(4).enumerate() {
            *c = (-DIODE_A[i][j], if i == j { w } else { 0.0 });
        }
        row[4] = (if i == 0 { 1.0 } else { 0.0 }, 0.0);
    }
    let mul = |a: (f64, f64), b: (f64, f64)| (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0);
    let div = |a: (f64, f64), b: (f64, f64)| {
        let d = b.0 * b.0 + b.1 * b.1;
        ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
    };
    for p in 0..4 {
        let pivot = m[p];
        for row in m.iter_mut().skip(p + 1) {
            let f = div(row[p], pivot[p]);
            for (c, pc) in row.iter_mut().zip(pivot.iter()).skip(p) {
                let t = mul(f, *pc);
                *c = (c.0 - t.0, c.1 - t.1);
            }
        }
    }
    let mut v = [(0.0, 0.0); 4];
    for r in (0..4).rev() {
        let mut acc = m[r][4];
        for c in r + 1..4 {
            let t = mul(m[r][c], v[c]);
            acc = (acc.0 - t.0, acc.1 - t.1);
        }
        v[r] = div(acc, m[r][r]);
    }
    v[3]
}

/// The diode ladder's constants, computed once: its open −3 dB point and its −180° point
/// (where it resonates) for ωc = 1, and the feedback at which it self-oscillates. The
/// cutoff knob is the resonant frequency, as on the transistor ladder, so the peak is
/// where the knob says; the spread poles put its unresonated −3 dB point lower
/// (`w3db / w180` of the cutoff) — the 303's darker, softer slope.
#[derive(Copy, Clone, Debug)]
pub struct DiodeCal {
    pub w3db: f64,
    pub w180: f64,
    pub k_max: f64,
}

impl DiodeCal {
    pub fn compute() -> Self {
        let mag = |w: f64| {
            let (re, im) = diode_response(w);
            (re * re + im * im).sqrt()
        };
        // −3 dB point by bisection.
        let (mut lo, mut hi) = (1e-4, 10.0);
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if mag(mid) > std::f64::consts::FRAC_1_SQRT_2 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let w3db = 0.5 * (lo + hi);
        // The −180° frequency. The phase falls monotonically from 0 towards −360° (four real
        // poles), so the imaginary part is negative before −180° and positive after.
        let (mut lo, mut hi) = (1e-3, 100.0f64);
        for _ in 0..100 {
            let mid = (lo * hi).sqrt();
            let (_, im) = diode_response(mid);
            if im < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let w180 = (lo * hi).sqrt();
        DiodeCal {
            w3db,
            w180,
            k_max: 1.0 / mag(w180),
        }
    }
}

/// One of the synth's filters, with its state.
#[derive(Copy, Clone, Debug, Default)]
pub struct BassFilter {
    /// Integrator states (ladder / diode: four; SVF: two).
    s: [f64; 4],
}

impl BassFilter {
    pub fn reset(&mut self) {
        self.s = [0.0; 4];
    }

    /// One sample through `kind` at `cutoff` Hz with resonance 0…1, bass compensation 0…1
    /// and input drive 0…1.
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn process(
        &mut self,
        kind: FilterKind,
        x: f32,
        cutoff: f32,
        reso: f32,
        comp: f32,
        drive: f32,
        sr: f32,
        cal: &DiodeCal,
    ) -> f32 {
        let fc = (cutoff as f64).clamp(10.0, 0.45 * sr as f64);
        let mut x = x as f64;
        if drive > 0.0 {
            let k = 1.0 + 8.0 * drive as f64;
            x = sat(k * x) / sat(k);
        }
        // The feedback node's saturator sees the signal −16 dB down, so an undriven filter
        // is all but linear (nothing to alias); the drive pushes it in.
        let h = HEADROOM * (1.0 + 4.0 * drive as f64);
        let x = x * h;
        let y = match kind {
            FilterKind::Ladder => self.ladder(x, fc, reso as f64, comp as f64, sr as f64),
            FilterKind::Diode => self.diode(x, fc, reso as f64, comp as f64, sr as f64, cal),
            FilterKind::SvfLow | FilterKind::SvfBand => {
                self.svf(x, fc, reso as f64, sr as f64, kind == FilterKind::SvfBand)
            }
        };
        for s in &mut self.s {
            if s.abs() < 1e-30 {
                *s = 0.0;
            }
        }
        (y / h) as f32
    }

    fn ladder(&mut self, x: f64, fc: f64, reso: f64, comp: f64, sr: f64) -> f64 {
        let g = (std::f64::consts::PI * fc / sr).tan();
        let gg = g / (1.0 + g);
        let b = 1.0 / (1.0 + g);
        let k = reso.clamp(0.0, 1.0) * LADDER_K_MAX;
        let x = x * (1.0 + comp.clamp(0.0, 1.0) * k);
        // y4 = G⁴ u + S, u = x − k y4.
        let s = &mut self.s;
        let big_s = gg * gg * gg * b * s[0] + gg * gg * b * s[1] + gg * b * s[2] + b * s[3];
        let g4 = gg * gg * gg * gg;
        let y4_lin = (g4 * x + big_s) / (1.0 + k * g4);
        let u = sat(x - k * y4_lin);
        let mut v = u;
        for st in s.iter_mut() {
            let y = gg * v + b * *st;
            *st = 2.0 * y - *st;
            v = y;
        }
        v
    }

    fn diode(&mut self, x: f64, fc: f64, reso: f64, comp: f64, sr: f64, cal: &DiodeCal) -> f64 {
        // Pre-warp the resonant frequency itself, then scale by the prototype's, so the
        // digital filter resonates exactly at fc.
        let g = ((std::f64::consts::PI * fc / sr).tan() / cal.w180).min(20.0);
        let k = reso.clamp(0.0, 1.0) * cal.k_max * 1.02;
        let x = x * (1.0 + comp.clamp(0.0, 1.0) * k);
        // Linear, with feedback: (I − gA + g k b e4ᵀ) v = g b x + s.
        let mut m = [[0.0f64; 5]; 4];
        for i in 0..4 {
            for j in 0..4 {
                m[i][j] = if i == j { 1.0 } else { 0.0 } - g * DIODE_A[i][j];
            }
            m[i][4] = self.s[i];
        }
        m[0][3] += g * k;
        m[0][4] += g * x;
        let v = solve4(m);
        let u = sat(x - k * v[3]);
        // Open loop with the saturated input: (I − gA) v = g b u + s.
        let mut m = [[0.0f64; 5]; 4];
        for i in 0..4 {
            for j in 0..4 {
                m[i][j] = if i == j { 1.0 } else { 0.0 } - g * DIODE_A[i][j];
            }
            m[i][4] = self.s[i];
        }
        m[0][4] += g * u;
        let v = solve4(m);
        for (s, vi) in self.s.iter_mut().zip(v) {
            *s = 2.0 * vi - *s;
        }
        v[3]
    }

    fn svf(&mut self, x: f64, fc: f64, reso: f64, sr: f64, band: bool) -> f64 {
        let g = (std::f64::consts::PI * fc / sr).tan();
        // Q from 0.5 to about 25.
        let k = 2.0 - 1.96 * reso.clamp(0.0, 1.0);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let (ic1, ic2) = (self.s[0], self.s[1]);
        let v3 = x - ic2;
        let v1 = a1 * ic1 + a2 * v3;
        let v2 = ic2 + a2 * ic1 + a3 * v3;
        self.s[0] = 2.0 * v1 - ic1;
        self.s[1] = 2.0 * v2 - ic2;
        if band {
            // Unity at the centre.
            v1 * k
        } else {
            v2
        }
    }
}

/// Gaussian elimination with partial pivoting on an augmented 4×5 system.
#[inline]
fn solve4(mut m: [[f64; 5]; 4]) -> [f64; 4] {
    for p in 0..4 {
        let mut best = p;
        for r in p + 1..4 {
            if m[r][p].abs() > m[best][p].abs() {
                best = r;
            }
        }
        m.swap(p, best);
        let d = m[p][p];
        if d.abs() < 1e-300 {
            continue;
        }
        let pivot = m[p];
        for row in m.iter_mut().skip(p + 1) {
            let f = row[p] / d;
            if f != 0.0 {
                for (c, pc) in row.iter_mut().zip(pivot.iter()).skip(p) {
                    *c -= f * pc;
                }
            }
        }
    }
    let mut v = [0.0; 4];
    for r in (0..4).rev() {
        let mut acc = m[r][4];
        for c in r + 1..4 {
            acc -= m[r][c] * v[c];
        }
        v[r] = if m[r][r].abs() < 1e-300 {
            0.0
        } else {
            acc / m[r][r]
        };
    }
    v
}
