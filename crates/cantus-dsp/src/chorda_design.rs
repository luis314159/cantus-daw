//! A Chorda string's loop, designed off the audio thread: an integer delay, a linear-phase
//! loss filter (one sample of delay at every frequency), a first-order allpass for the
//! fraction of a sample, and a cascade of second-order allpass sections whose phase makes
//! the loop's resonances fall on the stiff string's partials `f_n = n F₀ √(1 + B n²)`.
//!
//! **The design** follows Abel, Välimäki & Smith ("Robust, efficient design of allpass
//! filters for dispersive string sound synthesis", IEEE SPL 2010): the group delay the
//! stiffness asks for, above the delay of the highest partial fitted, is integrated into a
//! phase; one second-order section is placed wherever that phase crosses another 2π, its
//! pole radius from the spacing of its neighbours. Levenberg–Marquardt then refines every
//! pole and the fraction on the partials' errors in cents. The partials checked — those
//! below 5 kHz, at most 32 — are weighed fully; the ones above, up to 10 kHz (and 0.4·fs)
//! and at most 64, a tenth.
//!
//! The loop's phase at `ω` (rad/sample) is `−ω (N + 1) + φ₁(ω) + Σ φ₂(ω)`; partial `k`
//! sits where it equals `−2πk`.

use crate::math;

/// The most second-order sections a string's loop may take.
pub const MAX_SECTIONS: usize = 24;
/// The partials checked: those below this frequency…
pub const CHECK_HZ: f64 = 5_000.0;
/// …and at most this many.
pub const CHECK_PARTIALS: u32 = 32;
/// The partials weighed a tenth go up to here (and 0.4·fs): enough to keep the top of a
/// string's spectrum near its law without sections spent where little is heard.
pub const FIT_HZ: f64 = 10_000.0;
/// A design is good when every checked partial is this close.
pub const TOLERANCE_CENTS: f64 = 0.5;

const TAU: f64 = std::f64::consts::TAU;
const TO_CENTS: f64 = 1200.0 / std::f64::consts::LN_2;

/// A designed loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopDesign {
    /// Integer delay, samples (the loss filter's one sample not included).
    pub delay: usize,
    /// The first-order fractional section's coefficient.
    pub frac: f64,
    /// Second-order sections' poles: radius and angle (rad/sample); the first `sections`.
    pub poles: [(f64, f64); MAX_SECTIONS],
    pub sections: usize,
    /// What it was designed for: F₀ (Hz), B, the sample rate.
    pub f0: f64,
    pub b: f64,
    pub fs: f64,
    /// The largest error (cents) over the checked partials, from the design's own phase.
    pub worst_cents: f64,
    /// How much `frac` moves to raise the loop's pitch by one cent (a unison string's
    /// detune, a few cents at most, is this slope times its cents).
    pub frac_per_cent: f64,
}

/// A first-order allpass `(a + z⁻¹)/(1 + a z⁻¹)`: phase and group delay at `w`.
#[inline]
pub fn first_phase(w: f64, a: f64) -> f64 {
    let (s, c) = math::sin_cos(w);
    -w + 2.0 * math::atan2(a * s, 1.0 + a * c)
}

#[inline]
pub fn first_delay(w: f64, a: f64) -> f64 {
    (1.0 - a * a) / (1.0 + 2.0 * a * math::cos(w) + a * a)
}

/// A second-order allpass with poles `r e^{±jθ}`: its denominator `1 + a1 z⁻¹ + a2 z⁻²`.
#[inline]
pub fn second_coefs(r: f64, theta: f64) -> (f64, f64) {
    (-2.0 * r * math::cos(theta), r * r)
}

/// Its phase at `w`: `−2w − 2 arg A(e^{jw})`.
#[inline]
pub fn second_phase(w: f64, r: f64, theta: f64) -> f64 {
    let (a1, a2) = second_coefs(r, theta);
    let (s, c) = math::sin_cos(w);
    let (s2, c2) = math::sin_cos(2.0 * w);
    let re = 1.0 + a1 * c + a2 * c2;
    let im = -(a1 * s + a2 * s2);
    -2.0 * w - 2.0 * math::atan2(im, re)
}

/// Its group delay at `w`: one term per pole.
#[inline]
pub fn second_delay(w: f64, r: f64, theta: f64) -> f64 {
    let term = |d: f64| (1.0 - r * r) / (1.0 - 2.0 * r * math::cos(d) + r * r);
    term(w - theta) + term(w + theta)
}

impl LoopDesign {
    /// The loop's phase at `w`.
    pub fn phase(&self, w: f64) -> f64 {
        let mut p = -w * (self.delay as f64 + 1.0) + first_phase(w, self.frac);
        for (r, t) in &self.poles[..self.sections] {
            p += second_phase(w, *r, *t);
        }
        p
    }

    /// The loop's group delay (samples) at `w`.
    pub fn group_delay(&self, w: f64) -> f64 {
        let mut d = self.delay as f64 + 1.0 + first_delay(w, self.frac);
        for (r, t) in &self.poles[..self.sections] {
            d += second_delay(w, *r, *t);
        }
        d
    }

    /// The frequency (Hz) of the loop's `k`-th resonance: Newton on `phase(ω) = −2πk`.
    pub fn resonance(&self, k: u32) -> f64 {
        let target = -TAU * k as f64;
        let mut w = TAU * target_hz(self.f0, self.b, k) / self.fs;
        for _ in 0..40 {
            let e = self.phase(w) - target;
            w += e / self.group_delay(w);
            w = w.clamp(1e-9, std::f64::consts::PI);
        }
        w * self.fs / TAU
    }
}

/// The stiff string's `k`-th partial (Hz).
pub fn target_hz(f0: f64, b: f64, k: u32) -> f64 {
    let k = k as f64;
    k * f0 * (1.0 + b * k * k).sqrt()
}

/// The partials a design is checked on: below [`CHECK_HZ`] (and 0.45·fs), at most
/// [`CHECK_PARTIALS`], always the first.
pub fn checked_partials(f0: f64, b: f64, fs: f64) -> u32 {
    let top = CHECK_HZ.min(0.45 * fs);
    (1..=CHECK_PARTIALS)
        .take_while(|k| *k == 1 || target_hz(f0, b, *k) < top)
        .count() as u32
}

fn cents(a: f64, b: f64) -> f64 {
    1200.0 * math::log2(a / b)
}

/// The partial number at frequency `f` (the stiff-string law inverted).
fn partial_at(f0: f64, b: f64, f: f64) -> f64 {
    let x = f / f0;
    if b <= 0.0 {
        return x;
    }
    ((-1.0 + (1.0 + 4.0 * b * x * x).sqrt()) / (2.0 * b)).sqrt()
}

/// The loop's group delay (samples) the stiff string asks for at `w`: `2π dk/dω`.
fn wanted_delay(f0: f64, b: f64, fs: f64, w: f64) -> f64 {
    let k = partial_at(f0, b, w * fs / TAU);
    fs * (1.0 + b * k * k).sqrt() / (f0 * (1.0 + 2.0 * b * k * k))
}

/// The fit's targets: `(ω, k, weight)`.
fn targets(f0: f64, b: f64, fs: f64) -> Vec<(f64, u32, f64)> {
    let top = (0.4 * fs).min(FIT_HZ);
    let checked = checked_partials(f0, b, fs);
    (1..=64u32)
        .map(|k| (target_hz(f0, b, k), k))
        .take_while(|(f, k)| *k == 1 || *f < top)
        .map(|(f, k)| {
            // The first partial is the key's pitch: weighed ten times.
            let wt = match k {
                1 => 10.0,
                k if k <= checked => 1.0,
                _ => 0.1,
            };
            (TAU * f / fs, k, wt)
        })
        .collect()
}

/// Solve `m · x = v` (`n × n`, row-major) by Gaussian elimination with partial pivoting.
fn solve(m: &mut [f64], v: &mut [f64], n: usize) -> bool {
    for c in 0..n {
        let p = (c..n)
            .max_by(|a, b| m[a * n + c].abs().total_cmp(&m[b * n + c].abs()))
            .unwrap_or(c);
        if m[p * n + c].abs() < 1e-300 {
            return false;
        }
        if p != c {
            for j in 0..n {
                m.swap(p * n + j, c * n + j);
            }
            v.swap(p, c);
        }
        for r in c + 1..n {
            let f = m[r * n + c] / m[c * n + c];
            for j in c..n {
                m[r * n + j] -= f * m[c * n + j];
            }
            v[r] -= f * v[c];
        }
    }
    for c in (0..n).rev() {
        let mut s = v[c];
        for j in c + 1..n {
            s -= m[c * n + j] * v[j];
        }
        v[c] = s / m[c * n + c];
    }
    true
}

/// The design a parameter vector stands for: per section `(θ, y)` with `r = 1 − e^y`,
/// then the fraction's coefficient.
fn from_params(x: &[f64], base: &LoopDesign) -> LoopDesign {
    let mut d = *base;
    let n = (x.len() - 1) / 2;
    d.sections = n;
    for i in 0..n {
        let theta = x[2 * i].clamp(1e-5, std::f64::consts::PI - 1e-5);
        let r = 1.0 - math::exp(x[2 * i + 1].clamp(-12.0, -0.1));
        d.poles[i] = (r, theta);
    }
    d.frac = x[x.len() - 1].clamp(-0.95, 0.95);
    d
}

fn residuals(d: &LoopDesign, tg: &[(f64, u32, f64)], out: &mut Vec<f64>) {
    out.clear();
    for (w, k, wt) in tg {
        let e = d.phase(*w) + TAU * *k as f64;
        out.push(wt * TO_CENTS * e / d.group_delay(*w) / w);
    }
}

/// Levenberg–Marquardt over the parameter vector. The Jacobian is analytic — each
/// section's phase against its pole's angle and radius, the fraction's against its
/// coefficient — with the loop's group delay (which turns a phase error into a frequency
/// error) held for the step.
fn refine(x: &mut [f64], base: &LoopDesign, tg: &[(f64, u32, f64)], iters: usize) {
    let m = x.len();
    let n = (m - 1) / 2;
    let trig: Vec<(f64, f64, f64, f64)> = tg
        .iter()
        .map(|(w, _, _)| {
            let (s, c) = math::sin_cos(*w);
            let (s2, c2) = math::sin_cos(2.0 * *w);
            (s, c, s2, c2)
        })
        .collect();
    let mut r = Vec::with_capacity(tg.len());
    let mut d = from_params(x, base);
    residuals(&d, tg, &mut r);
    let mut c: f64 = r.iter().map(|v| v * v).sum();
    let mut lambda = 1e-2;
    let mut jac = vec![0.0; tg.len() * m];
    let mut rn = Vec::with_capacity(tg.len());
    let mut probe = x.to_vec();
    let mut jtj = vec![0.0; m * m];
    let mut jtr = vec![0.0; m];
    let w1 = tg[0].0;
    for it in 0..iters {
        // Good enough already (the fundamental exact, every checked partial well inside the
        // tolerance)? The fit's own optimum is not the goal; the partials are.
        if it % 8 == 7 {
            let mut t = d;
            tune_first(&mut t, w1);
            if worst(&t) < 0.8 * TOLERANCE_CENTS {
                break;
            }
        }
        for (i, (w, _, wt)) in tg.iter().enumerate() {
            let (s, cw, s2, c2) = trig[i];
            let scale = wt * TO_CENTS / (d.group_delay(*w) * w);
            for k in 0..n {
                let (rad, th) = d.poles[k];
                let (sin_t, cos_t) = math::sin_cos(th);
                let (a1, a2) = (-2.0 * rad * cos_t, rad * rad);
                let re = 1.0 + a1 * cw + a2 * c2;
                let im = -(a1 * s + a2 * s2);
                let mag = re * re + im * im;
                // φ = −2w − 2 atan2(im, re): its derivative against a1 and a2.
                let dp_da1 = -2.0 * (re * (-s) - im * cw) / mag;
                let dp_da2 = -2.0 * (re * (-s2) - im * c2) / mag;
                let dp_dth = dp_da1 * (2.0 * rad * sin_t);
                let dr_dy = -(1.0 - rad);
                let dp_dy = (dp_da1 * (-2.0 * cos_t) + dp_da2 * (2.0 * rad)) * dr_dy;
                jac[i * m + 2 * k] = scale * dp_dth;
                jac[i * m + 2 * k + 1] = scale * dp_dy;
            }
            let a = d.frac;
            jac[i * m + m - 1] = scale * 2.0 * s / (1.0 + 2.0 * a * cw + a * a);
        }
        let mut improved = false;
        for _ in 0..16 {
            jtj.fill(0.0);
            jtr.fill(0.0);
            for i in 0..tg.len() {
                let row = &jac[i * m..(i + 1) * m];
                for a in 0..m {
                    jtr[a] -= row[a] * r[i];
                    for b in a..m {
                        jtj[a * m + b] += row[a] * row[b];
                    }
                }
            }
            for a in 0..m {
                for b in 0..a {
                    jtj[a * m + b] = jtj[b * m + a];
                }
                jtj[a * m + a] = jtj[a * m + a] * (1.0 + lambda) + 1e-12;
            }
            if !solve(&mut jtj, &mut jtr, m) {
                lambda *= 10.0;
                continue;
            }
            for j in 0..m {
                probe[j] = x[j] + jtr[j];
            }
            let nd = from_params(&probe, base);
            residuals(&nd, tg, &mut rn);
            let cn: f64 = rn.iter().map(|v| v * v).sum();
            if cn < c {
                x.copy_from_slice(&probe);
                d = nd;
                std::mem::swap(&mut r, &mut rn);
                c = cn;
                lambda = (lambda * 0.2).max(1e-12);
                improved = true;
                break;
            }
            lambda *= 10.0;
        }
        if !improved || c < 1e-12 {
            break;
        }
    }
}

/// Abel et al.'s construction for `n` sections (radius spread `beta`), the integer delay
/// and fraction set so the first partial lands, then refined.
fn construct(f0: f64, b: f64, fs: f64, tg: &[(f64, u32, f64)], n: usize, beta: f64) -> LoopDesign {
    let w_top = tg[tg.len() - 1].0;
    let floor = wanted_delay(f0, b, fs, w_top);
    // The dispersive phase, integrated on a grid.
    const GRID: usize = 2048;
    let mut cum = [0.0f64; GRID + 1];
    for i in 0..GRID {
        let (w0, w1) = (
            w_top * i as f64 / GRID as f64,
            w_top * (i + 1) as f64 / GRID as f64,
        );
        let d0 = wanted_delay(f0, b, fs, w0.max(1e-9)) - floor;
        let d1 = wanted_delay(f0, b, fs, w1) - floor;
        cum[i + 1] = cum[i] + 0.5 * (d0 + d1) * (w1 - w0);
    }
    let total = cum[GRID];
    let mut base = LoopDesign {
        delay: 0,
        frac: 0.0,
        poles: [(0.0, 0.0); MAX_SECTIONS],
        sections: n,
        f0,
        b,
        fs,
        worst_cents: f64::INFINITY,
        frac_per_cent: 0.0,
    };
    // Where the phase crosses (i − ½)·total/n.
    let mut thetas = [0.0f64; MAX_SECTIONS];
    for (i, t) in thetas.iter_mut().enumerate().take(n) {
        let want = (i as f64 + 0.5) * total / n as f64;
        let j = cum.partition_point(|c| *c < want).clamp(1, GRID);
        let frac = (want - cum[j - 1]) / (cum[j] - cum[j - 1]).max(1e-300);
        *t = w_top * ((j - 1) as f64 + frac) / GRID as f64;
    }
    for i in 0..n {
        let lo = if i == 0 { 0.0 } else { thetas[i - 1] };
        let hi = if i + 1 == n { w_top } else { thetas[i + 1] };
        let spacing = ((hi - lo) / 2.0).max(1e-5);
        let r = (1.0 - beta * spacing).clamp(0.05, 1.0 - 1e-5);
        base.poles[i] = (r, thetas[i]);
    }
    // The integer delay and the fraction (a section of about one sample, flat in phase).
    let w1 = tg[0].0;
    let mut bq = 0.0;
    for (r, t) in &base.poles[..n] {
        bq += second_phase(w1, *r, *t);
    }
    let rest = (TAU + bq) / w1 - 1.0;
    let whole = (rest - 0.5).floor().max(1.0);
    let d_frac = (rest - whole).clamp(0.3, 1.7);
    base.delay = whole as usize;
    base.frac = (1.0 - d_frac) / (1.0 + d_frac);
    let mut x: Vec<f64> = Vec::with_capacity(2 * n + 1);
    for (r, t) in &base.poles[..n] {
        x.push(*t);
        x.push(math::ln(1.0 - r));
    }
    x.push(base.frac);
    refine(&mut x, &base, tg, 300);
    let mut d = from_params(&x, &base);
    tune_first(&mut d, w1);
    d.worst_cents = worst(&d);
    d
}

/// The first partial — the key's pitch — exactly: Newton on the fraction alone.
fn tune_first(d: &mut LoopDesign, w1: f64) {
    let (s1, c1) = math::sin_cos(w1);
    for _ in 0..8 {
        let e = d.phase(w1) + TAU;
        let a = d.frac;
        let slope = 2.0 * s1 / (1.0 + 2.0 * a * c1 + a * a);
        d.frac = (a - e / slope).clamp(-0.99, 0.99);
    }
}

/// The largest error (cents) over the checked partials.
fn worst(d: &LoopDesign) -> f64 {
    let k_check = checked_partials(d.f0, d.b, d.fs);
    (1..=k_check)
        .map(|k| cents(d.resonance(k), target_hz(d.f0, d.b, k)).abs())
        .fold(0.0, f64::max)
}

/// Design the loop of a string with fundamental `f0` (the ideal string's) and
/// inharmonicity `b` at sample rate `fs`: the fewest sections that put every checked
/// partial within [`TOLERANCE_CENTS`] (or the best tried).
pub fn design(f0: f64, b: f64, fs: f64) -> LoopDesign {
    let tg = targets(f0, b, fs);
    let w_top = tg[tg.len() - 1].0;
    let floor = wanted_delay(f0, b, fs, w_top);
    // The dispersive phase decides how many sections Abel's construction places.
    let mut total = 0.0;
    let steps = 512;
    for i in 0..steps {
        let w = w_top * (i as f64 + 0.5) / steps as f64;
        total += (wanted_delay(f0, b, fs, w) - floor) * w_top / steps as f64;
    }
    let guess = (total / TAU).round().clamp(1.0, MAX_SECTIONS as f64) as usize;
    let mut best: Option<LoopDesign> = None;
    let mut tries: Vec<usize> = vec![guess];
    for extra in 1..=MAX_SECTIONS {
        if guess + extra <= MAX_SECTIONS {
            tries.push(guess + extra);
        }
        if extra < guess {
            tries.push(guess - extra);
        }
    }
    'tries: for n in tries.into_iter().take(8) {
        for beta in [1.0, 0.6, 1.6] {
            let d = construct(f0, b, fs, &tg, n, beta);
            if best.is_none_or(|b| d.worst_cents < b.worst_cents) {
                best = Some(d);
            }
            if d.worst_cents < TOLERANCE_CENTS {
                best = Some(d);
                break 'tries;
            }
        }
    }
    let mut d = best.expect("a design");
    // The fraction's slope per cent, for the unison strings' detune.
    let w1 = TAU * target_hz(f0, b, 1) / fs;
    let mut up = d;
    tune_first(&mut up, w1 * math::powf(2.0f64, 1.0 / 1200.0));
    d.frac_per_cent = up.frac - d.frac;
    d
}

/// A design with at most `max_sections` sections (the sympathetic strings'): the first
/// partial exact, the others as close as that many sections bring them.
pub fn design_limited(f0: f64, b: f64, fs: f64, max_sections: usize) -> LoopDesign {
    let tg = targets(f0, b, fs);
    let mut best: Option<LoopDesign> = None;
    for n in 1..=max_sections.clamp(1, MAX_SECTIONS) {
        for beta in [1.0, 0.6, 1.6] {
            let d = construct(f0, b, fs, &tg, n, beta);
            if best.is_none_or(|b| d.worst_cents < b.worst_cents) {
                best = Some(d);
            }
        }
    }
    best.expect("a design")
}

/// What a keyboard's designs depend on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// A4's first partial, Hz.
    pub a4: f64,
    /// The octaves' stretch above 2:1 (1: Rigaud et al.'s average tuner).
    pub stretch: f64,
    /// The strings' stiffness against the model's (scales B).
    pub stiffness: f64,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            a4: 440.0,
            stretch: 1.0,
            stiffness: 1.0,
        }
    }
}

/// Every key's numbers and its loop's design, for one tuning at one sample rate.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyboard {
    pub tuning: Tuning,
    pub fs: f64,
    pub keys: [super::tables::Key; super::tables::KEYS],
    pub loops: Vec<LoopDesign>,
    /// The sympathetic strings' reduced designs (at most two sections).
    pub sympathetic: Vec<LoopDesign>,
}

impl Keyboard {
    /// Design all 88 loops (and the sympathetic strings' reduced ones) on `threads` threads
    /// (1: on the caller). The result does not depend on the thread count: each key is
    /// designed on its own.
    pub fn compute(tuning: Tuning, fs: f64, threads: usize) -> Keyboard {
        let keys = super::tables::keyboard(tuning.a4, tuning.stretch, tuning.stiffness);
        let threads = threads.clamp(1, 16);
        let one =
            |k: &super::tables::Key| (design(k.f0(), k.b, fs), design_limited(k.f0(), k.b, fs, 2));
        let mut pairs: Vec<Option<(LoopDesign, LoopDesign)>> = vec![None; keys.len()];
        if threads == 1 {
            for (slot, k) in pairs.iter_mut().zip(keys.iter()) {
                *slot = Some(one(k));
            }
        } else {
            // The slow keys are the low ones: deal the keys out round-robin.
            std::thread::scope(|s| {
                let handles: Vec<_> = (0..threads)
                    .map(|t| {
                        let keys = &keys;
                        let one = &one;
                        s.spawn(move || {
                            (t..keys.len())
                                .step_by(threads)
                                .map(|i| (i, one(&keys[i])))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                for h in handles {
                    for (i, d) in h.join().expect("a design thread") {
                        pairs[i] = Some(d);
                    }
                }
            });
        }
        let pairs: Vec<(LoopDesign, LoopDesign)> = pairs
            .into_iter()
            .map(|d| d.expect("every key designed"))
            .collect();
        Keyboard {
            tuning,
            fs,
            keys,
            loops: pairs.iter().map(|p| p.0).collect(),
            sympathetic: pairs.iter().map(|p| p.1).collect(),
        }
    }
}
