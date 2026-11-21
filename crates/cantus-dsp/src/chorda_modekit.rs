//! Measuring a unison's modes (round 41, `docs/round41/chorda-tune.md`): what the strings of
//! one key ring at, near each partial, to a thousandth of a cent — so the beat a tuner hears
//! between them is read off the sound, not guessed from the design.
//!
//! A key's strings, once the hammer has left them, are a linear system: near its partial `n`
//! a unison rings as a few damped sinusoids (one per loop, as the bridge couples them), and
//! any linear filtering — the heterodyne's low-pass here, the board, a partial's voicing —
//! keeps their frequencies and decays. So each partial is heterodyned to its designed
//! frequency, low-passed by a long Hann and decimated (every `HOP_S`), and the sequence is
//! fitted with damped exponentials by ESPRIT (Roy & Kailath 1989): the signal subspace of its
//! Hankel covariance (a Hermitian Jacobi eigen-decomposition), the rotation between its
//! shifted halves, its eigenvalues the modes' poles. Their amplitudes are then a least-squares
//! fit. The beat a tuner counts between two modes is their frequency difference.
//!
//! Everything in `f64` and `cantus_dsp::math`; allocates; tests only.

use crate::math;
use std::f64::consts::TAU;

/// A complex number (the kit's own: no dependency).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cx {
    pub re: f64,
    pub im: f64,
}

impl Cx {
    pub const ZERO: Cx = Cx { re: 0.0, im: 0.0 };
    pub const ONE: Cx = Cx { re: 1.0, im: 0.0 };
    pub fn new(re: f64, im: f64) -> Cx {
        Cx { re, im }
    }
    pub fn conj(self) -> Cx {
        Cx::new(self.re, -self.im)
    }
    pub fn norm2(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
    pub fn abs(self) -> f64 {
        self.norm2().sqrt()
    }
    pub fn arg(self) -> f64 {
        math::atan2(self.im, self.re)
    }
    pub fn scale(self, k: f64) -> Cx {
        Cx::new(self.re * k, self.im * k)
    }
    pub fn add(self, o: Cx) -> Cx {
        Cx::new(self.re + o.re, self.im + o.im)
    }
    pub fn sub(self, o: Cx) -> Cx {
        Cx::new(self.re - o.re, self.im - o.im)
    }
    pub fn mul(self, o: Cx) -> Cx {
        Cx::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
    pub fn div(self, o: Cx) -> Cx {
        let d = o.norm2();
        Cx::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
}

/// One mode near a partial.
#[derive(Clone, Copy, Debug)]
pub struct Mode {
    /// Its frequency (Hz).
    pub hz: f64,
    /// Its amplitude's decay rate (1/s: `e^{−σt}`; T60 = 6.91/σ).
    pub decay: f64,
    /// Its amplitude at the fit's start (the baseband's scale).
    pub amp: f64,
}

impl Mode {
    /// Its level (dB, the baseband's scale) at `t` seconds after the fit's start.
    pub fn level_db(&self, t: f64) -> f64 {
        20.0 * math::log10(self.amp.max(1e-300)) - 8.685_889_638 * self.decay * t
    }
}

/// The baseband step (s): a 100 Hz sequence.
pub const HOP_S: f64 = 0.01;

/// The complex baseband of `x` at `hz`: heterodyned with the absolute sample index's phase,
/// a Hann `len` samples long centred every `hop` samples from `first` on (its centres at
/// `first + j·hop`), while the window fits.
pub fn baseband(x: &[f64], fs: f64, hz: f64, len: usize, hop: usize, first: usize) -> Vec<Cx> {
    let w = TAU * hz / fs;
    let win: Vec<f64> = (0..len)
        .map(|i| 0.5 - 0.5 * math::cos(TAU * (i as f64 + 0.5) / len as f64))
        .collect();
    let norm = 2.0 / win.iter().sum::<f64>();
    let mut out = Vec::new();
    let mut c = first.max(len / 2);
    while c + len / 2 < x.len() {
        let s0 = c - len / 2;
        let mut acc = Cx::ZERO;
        for (k, wk) in win.iter().enumerate() {
            let i = s0 + k;
            let (s, co) = math::sin_cos((w * i as f64) % TAU);
            acc = acc.add(Cx::new(x[i] * co * wk, -x[i] * s * wk));
        }
        out.push(acc.scale(norm));
        c += hop;
    }
    out
}

/// The eigenvalues (descending) and eigenvectors (columns, `v[row][col]`) of the Hermitian
/// `a` (`n × n`, row-major), by cyclic Jacobi rotations.
pub fn hermitian_eigen(mut a: Vec<Vec<Cx>>) -> (Vec<f64>, Vec<Vec<Cx>>) {
    let n = a.len();
    let mut v: Vec<Vec<Cx>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { Cx::ONE } else { Cx::ZERO }).collect())
        .collect();
    let total: f64 = a.iter().flatten().map(|z| z.norm2()).sum::<f64>().max(1e-300);
    for _sweep in 0..60 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |j| *j != i).map(move |j| (i, j)))
            .map(|(i, j)| a[i][j].norm2())
            .sum();
        if off <= 1e-30 * total {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p][q];
                let m = apq.abs();
                if m <= 1e-300 {
                    continue;
                }
                let e = apq.scale(1.0 / m);
                let (app, aqq) = (a[p][p].re, a[q][q].re);
                let tau = (aqq - app) / (2.0 * m);
                let t = if tau >= 0.0 {
                    1.0 / (tau + (1.0 + tau * tau).sqrt())
                } else {
                    -1.0 / (-tau + (1.0 + tau * tau).sqrt())
                };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = t * c;
                // G = D·P: G_pp = c, G_pq = s, G_qp = −s·ē, G_qq = c·ē.
                let (gpp, gpq) = (Cx::new(c, 0.0), Cx::new(s, 0.0));
                let (gqp, gqq) = (e.conj().scale(-s), e.conj().scale(c));
                for row in a.iter_mut() {
                    let (x, y) = (row[p], row[q]);
                    row[p] = x.mul(gpp).add(y.mul(gqp));
                    row[q] = x.mul(gpq).add(y.mul(gqq));
                }
                for k in 0..n {
                    let (x, y) = (a[p][k], a[q][k]);
                    a[p][k] = gpp.conj().mul(x).add(gqp.conj().mul(y));
                    a[q][k] = gpq.conj().mul(x).add(gqq.conj().mul(y));
                }
                a[p][q] = Cx::ZERO;
                a[q][p] = Cx::ZERO;
                a[p][p].im = 0.0;
                a[q][q].im = 0.0;
                for row in v.iter_mut() {
                    let (x, y) = (row[p], row[q]);
                    row[p] = x.mul(gpp).add(y.mul(gqp));
                    row[q] = x.mul(gpq).add(y.mul(gqq));
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|i, j| a[*j][*j].re.total_cmp(&a[*i][*i].re));
    let vals = order.iter().map(|i| a[*i][*i].re).collect();
    let vecs = (0..n)
        .map(|r| order.iter().map(|c| v[r][*c]).collect())
        .collect();
    (vals, vecs)
}

/// Solve the square complex system `m x = y` by Gaussian elimination with partial pivoting.
pub fn solve(mut m: Vec<Vec<Cx>>, mut y: Vec<Cx>) -> Vec<Cx> {
    let n = y.len();
    for c in 0..n {
        let p = (c..n)
            .max_by(|a, b| m[*a][c].norm2().total_cmp(&m[*b][c].norm2()))
            .unwrap_or(c);
        m.swap(c, p);
        y.swap(c, p);
        let d = m[c][c];
        if d.norm2() == 0.0 {
            continue;
        }
        for r in c + 1..n {
            let f = m[r][c].div(d);
            if f.norm2() == 0.0 {
                continue;
            }
            for k in c..n {
                let t = m[c][k];
                m[r][k] = m[r][k].sub(f.mul(t));
            }
            let t = y[c];
            y[r] = y[r].sub(f.mul(t));
        }
    }
    let mut x = vec![Cx::ZERO; n];
    for r in (0..n).rev() {
        let mut s = y[r];
        for k in r + 1..n {
            s = s.sub(m[r][k].mul(x[k]));
        }
        x[r] = if m[r][r].norm2() == 0.0 {
            Cx::ZERO
        } else {
            s.div(m[r][r])
        };
    }
    x
}

/// The roots of `Σ c[k] zᵏ` (`c` from the constant term, its last entry the leading one), by
/// Durand–Kerner's iteration, each polished by Newton's.
pub fn roots(c: &[Cx]) -> Vec<Cx> {
    let n = c.len() - 1;
    if n == 0 {
        return Vec::new();
    }
    let lead = c[n];
    let a: Vec<Cx> = c.iter().map(|x| x.div(lead)).collect();
    let eval = |z: Cx| -> (Cx, Cx) {
        let (mut p, mut d) = (Cx::ONE, Cx::ZERO);
        for k in (0..n).rev() {
            d = d.mul(z).add(p);
            p = p.mul(z).add(a[k]);
        }
        (p, d)
    };
    let mut z: Vec<Cx> = (0..n)
        .map(|k| {
            let (s, co) = math::sin_cos(0.4 + TAU * k as f64 / n as f64);
            Cx::new(0.9 * co, 0.9 * s)
        })
        .collect();
    for _ in 0..2_000 {
        let mut moved = 0.0f64;
        for i in 0..n {
            let (p, _) = eval(z[i]);
            let mut den = Cx::ONE;
            for (j, zj) in z.iter().enumerate() {
                if j != i {
                    den = den.mul(z[i].sub(*zj));
                }
            }
            if den.norm2() == 0.0 {
                continue;
            }
            let step = p.div(den);
            z[i] = z[i].sub(step);
            moved = moved.max(step.abs());
        }
        if moved < 1e-15 {
            break;
        }
    }
    for zi in z.iter_mut() {
        for _ in 0..4 {
            let (p, d) = eval(*zi);
            if d.norm2() == 0.0 {
                break;
            }
            *zi = zi.sub(p.div(d));
        }
    }
    z
}

/// The eigenvalues of the small square complex `m` (its characteristic polynomial by
/// Faddeev–LeVerrier, then [`roots`]).
pub fn eigenvalues(m: &[Vec<Cx>]) -> Vec<Cx> {
    let n = m.len();
    let mut c = vec![Cx::ZERO; n + 1];
    c[n] = Cx::ONE;
    let mut mk: Vec<Vec<Cx>> = vec![vec![Cx::ZERO; n]; n];
    for k in 1..=n {
        // M_k = A·M_{k−1} + c_{n−k+1}·I; c_{n−k} = −tr(A·M_k)/k.
        let mut next = vec![vec![Cx::ZERO; n]; n];
        for i in 0..n {
            for j in 0..n {
                let mut s = Cx::ZERO;
                for l in 0..n {
                    s = s.add(m[i][l].mul(mk[l][j]));
                }
                next[i][j] = s;
            }
            next[i][i] = next[i][i].add(c[n - k + 1]);
        }
        let mut tr = Cx::ZERO;
        for i in 0..n {
            for l in 0..n {
                tr = tr.add(m[i][l].mul(next[l][i]));
            }
        }
        c[n - k] = tr.scale(-1.0 / k as f64);
        mk = next;
    }
    roots(&c)
}

/// The damped exponentials in `y` (a baseband every `hop_s` seconds, heterodyned at `hz`):
/// at most `k_max`, those whose share of the Hankel covariance's power is over `1e-11` of the
/// strongest's (−110 dB), with their amplitudes at `y[0]`, strongest first; any under −80 dB
/// of the strongest dropped.
pub fn modes(y: &[Cx], k_max: usize, hop_s: f64, hz: f64) -> Vec<Mode> {
    let n = y.len();
    if n < 3 * (k_max + 2) {
        return Vec::new();
    }
    let m = (n / 3).clamp(k_max + 2, 40);
    let l = n - m + 1;
    let mut r = vec![vec![Cx::ZERO; m]; m];
    for i in 0..m {
        for j in i..m {
            let mut s = Cx::ZERO;
            for row in 0..l {
                s = s.add(y[row + i].conj().mul(y[row + j]));
            }
            r[i][j] = s;
            r[j][i] = s.conj();
        }
    }
    let (vals, vecs) = hermitian_eigen(r);
    let top = vals[0].max(1e-300);
    let k = vals
        .iter()
        .take(k_max)
        .filter(|v| **v > 1e-11 * top)
        .count()
        .max(1);
    // ESPRIT: the signal subspace's rows shifted by one, related by Φ; Φ's eigenvalues are
    // the poles.
    let mut a = vec![vec![Cx::ZERO; k]; k];
    let mut b = vec![vec![Cx::ZERO; k]; k];
    for i in 0..k {
        for j in 0..k {
            let (mut sa, mut sb) = (Cx::ZERO, Cx::ZERO);
            for row in 0..m - 1 {
                sa = sa.add(vecs[row][i].conj().mul(vecs[row][j]));
                sb = sb.add(vecs[row][i].conj().mul(vecs[row + 1][j]));
            }
            a[i][j] = sa;
            b[i][j] = sb;
        }
    }
    let mut phi = vec![vec![Cx::ZERO; k]; k];
    for j in 0..k {
        let col = solve(a.clone(), (0..k).map(|i| b[i][j]).collect());
        for i in 0..k {
            phi[i][j] = col[i];
        }
    }
    // `XᴴX`'s eigenvectors span the poles' conjugate powers (`X = Σ cₖ zₖʳ ⊗ zₖᶜ`): Φ's
    // eigenvalues are the conjugate poles.
    let poles: Vec<Cx> = eigenvalues(&phi).into_iter().map(Cx::conj).collect();
    // The amplitudes: least squares on the Vandermonde.
    let mut g = vec![vec![Cx::ZERO; k]; k];
    let mut h = vec![Cx::ZERO; k];
    let mut pw: Vec<Cx> = vec![Cx::ONE; k];
    for yj in y {
        for i in 0..k {
            for jj in 0..k {
                g[i][jj] = g[i][jj].add(pw[i].conj().mul(pw[jj]));
            }
            h[i] = h[i].add(pw[i].conj().mul(*yj));
        }
        for (p, z) in pw.iter_mut().zip(&poles) {
            *p = p.mul(*z);
        }
    }
    let amps = solve(g, h);
    let mut out: Vec<Mode> = poles
        .iter()
        .zip(&amps)
        .map(|(z, c)| Mode {
            hz: hz + z.arg() / (TAU * hop_s),
            decay: -math::ln(z.abs().max(1e-300)) / hop_s,
            amp: c.abs(),
        })
        .collect();
    out.sort_by(|p, q| q.amp.total_cmp(&p.amp));
    let strongest = out.first().map_or(0.0, |m| m.amp);
    out.retain(|m| m.amp >= 1e-4 * strongest);
    out
}

/// One partial's modes.
#[derive(Clone, Debug)]
pub struct PartialModes {
    pub n: u32,
    /// Where it was heterodyned (Hz) and the fit's span (s from the strike).
    pub near_hz: f64,
    pub span: (f64, f64),
    /// Strongest first.
    pub modes: Vec<Mode>,
    /// Its peak level in the fit (dB, the baseband's scale).
    pub peak_db: f64,
}

impl PartialModes {
    /// The strongest mode's offset from `hz` (cents).
    pub fn cents_of_strongest(&self, hz: f64) -> f64 {
        self.modes
            .first()
            .map_or(f64::NAN, |m| 1_200.0 * math::log2(m.hz / hz))
    }

    /// The modes' power-weighted mean frequency over the fit's first second (Hz) — the
    /// pitch a beating set gives the ear (`tunekit`'s measure, from the modes).
    pub fn mean_hz(&self) -> f64 {
        let (mut sw, mut sf) = (0.0, 0.0);
        for m in &self.modes {
            let s2 = 2.0 * m.decay;
            // ∫₀¹ a² e^{−2σt} dt.
            let e = if s2.abs() < 1e-9 {
                1.0
            } else {
                (1.0 - math::exp(-s2)) / s2
            };
            let w = m.amp * m.amp * e;
            sw += w;
            sf += w * m.hz;
        }
        if sw > 0.0 {
            sf / sw
        } else {
            f64::NAN
        }
    }
}

/// Partials `near` (Hz, the first's index 1) of `x` (one channel, struck at sample 0), each
/// fitted with at most `k_max` modes from `t0` seconds on until it has fallen 70 dB under its
/// peak or `t1`. The Hann is `8 fs / f0` long, at least 0.1 s (the next partial ≥ 8 bins
/// away, every mode within ±f0/4 of its partial in its main lobe).
pub fn analyse(
    x: &[f64],
    fs: f64,
    f0: f64,
    near: &[f64],
    t0: f64,
    t1: f64,
    k_max: usize,
) -> Vec<PartialModes> {
    let len = (((8.0 * fs / f0).max(0.1 * fs)) as usize) & !1;
    let hop = (HOP_S * fs).round() as usize;
    let first = (t0 * fs) as usize;
    let last = ((t1 * fs) as usize).min(x.len());
    let mut out = Vec::new();
    for (i, &hz) in near.iter().enumerate() {
        if hz > 0.45 * fs {
            break;
        }
        let mut y = baseband(&x[..last], fs, hz, len, hop, first);
        let peak = y.iter().map(|z| z.abs()).fold(0.0f64, f64::max);
        if peak <= 0.0 {
            continue;
        }
        // Cut where it has fallen 70 dB (the next partials' leakage is ~−70 dB under them).
        if let Some(cut) = y.iter().rposition(|z| z.abs() > 3.16e-4 * peak) {
            y.truncate(cut + 1);
        }
        let start = first.max(len / 2) as f64 / fs;
        let span = (start, start + y.len() as f64 * HOP_S);
        let modes = modes(&y, k_max, HOP_S, hz);
        out.push(PartialModes {
            n: i as u32 + 1,
            near_hz: hz,
            span,
            modes,
            peak_db: 20.0 * math::log10(peak),
        });
    }
    out
}

/// A beat between two modes, as a tuner hears it.
#[derive(Clone, Copy, Debug)]
pub struct Beat {
    /// Its rate (Hz) and its deepest swing (dB, peak to trough) while audible.
    pub rate: f64,
    pub depth_db: f64,
    /// The two modes (indices into the partial's modes).
    pub pair: (usize, usize),
    /// How many whole swells the ear hears of it: its rate times the time it is audible.
    pub cycles: f64,
}

/// The fastest audible beat among `p`'s modes over its first `seconds`. A pair swings the
/// partial's level by its softer mode `m` against the partial's whole amplitude `A` (every
/// mode's, as powers): `20·log10((A + m)/(A − m))` peak to trough. It is audible while that
/// swing is at least `min_depth_db` (about 1 dB: the least a slow beat is heard by) and the
/// partial is within `floor_db` of its peak — so two soft modes under a loud one, beating
/// between themselves, are masked by it.
pub fn fastest_beat(
    p: &PartialModes,
    seconds: f64,
    floor_db: f64,
    min_depth_db: f64,
) -> Option<Beat> {
    let mut best: Option<Beat> = None;
    let steps = (seconds / 0.02).ceil() as usize;
    let lin = |db: f64| math::powf(10.0f64, db / 20.0);
    for i in 0..p.modes.len() {
        for j in i + 1..p.modes.len() {
            let (a, b) = (p.modes[i], p.modes[j]);
            let mut depth = f64::NEG_INFINITY;
            let mut heard = 0usize;
            for s in 0..=steps {
                let t = s as f64 * 0.02;
                let whole: f64 = p.modes.iter().map(|m| lin(2.0 * m.level_db(t))).sum::<f64>();
                let whole = whole.sqrt();
                if 20.0 * math::log10(whole.max(1e-300)) < p.peak_db - floor_db {
                    continue;
                }
                let m = lin(a.level_db(t)).min(lin(b.level_db(t)));
                let d = 20.0 * math::log10((whole + m) / (whole - m).max(1e-12 * whole));
                if d >= min_depth_db {
                    depth = depth.max(d);
                    heard += 1;
                }
            }
            if depth.is_finite() {
                let rate = (a.hz - b.hz).abs();
                if best.is_none_or(|x| rate > x.rate) {
                    best = Some(Beat {
                        rate,
                        depth_db: depth,
                        pair: (i, j),
                        cycles: rate * heard as f64 * 0.02,
                    });
                }
            }
        }
    }
    best
}

/// The audible beat among `p`'s modes the ear hears most swells of over its first
/// `seconds` (as [`fastest_beat`] judges a pair audible): a pair counts as a beat once it
/// swells and falls a whole time or more while audible — slower, or heard for less than a
/// cycle (a fast first stage over a slow second), it is a change of level, not a beat.
pub fn most_heard_beat(
    p: &PartialModes,
    seconds: f64,
    floor_db: f64,
    min_depth_db: f64,
) -> Option<Beat> {
    let steps = (seconds / 0.02).ceil() as usize;
    let lin = |db: f64| math::powf(10.0f64, db / 20.0);
    let mut best: Option<Beat> = None;
    for i in 0..p.modes.len() {
        for j in i + 1..p.modes.len() {
            let (a, b) = (p.modes[i], p.modes[j]);
            let (mut depth, mut heard) = (f64::NEG_INFINITY, 0usize);
            for s in 0..=steps {
                let t = s as f64 * 0.02;
                let whole: f64 = p.modes.iter().map(|m| lin(2.0 * m.level_db(t))).sum::<f64>();
                let whole = whole.sqrt();
                if 20.0 * math::log10(whole.max(1e-300)) < p.peak_db - floor_db {
                    continue;
                }
                let m = lin(a.level_db(t)).min(lin(b.level_db(t)));
                let d = 20.0 * math::log10((whole + m) / (whole - m).max(1e-12 * whole));
                if d >= min_depth_db {
                    depth = depth.max(d);
                    heard += 1;
                }
            }
            if heard == 0 {
                continue;
            }
            let rate = (a.hz - b.hz).abs();
            let cycles = rate * heard as f64 * 0.02;
            if best.is_none_or(|x| cycles > x.cycles) {
                best = Some(Beat {
                    rate,
                    depth_db: depth,
                    pair: (i, j),
                    cycles,
                });
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two damped tones 0.12 Hz apart near 440 Hz and a third, 30 dB down, 1.3 Hz away, plus
    /// the next partial: the kit finds each to a thousandth of a hertz.
    #[test]
    fn the_kit_finds_close_modes_to_a_thousandth_of_a_hertz() {
        let fs = 48_000.0;
        let tones = [
            (440.0, 0.8, 1.0),
            (440.12, 1.9, 0.6),
            (441.3, 0.5, 0.03),
            (880.7, 2.0, 0.5),
        ];
        let x: Vec<f64> = (0..(3.0 * fs) as usize)
            .map(|i| {
                let t = i as f64 / fs;
                tones
                    .iter()
                    .map(|(f, s, a)| a * math::exp(-s * t) * math::sin(TAU * f * t + 0.3))
                    .sum()
            })
            .collect();
        let p = analyse(&x, fs, 440.0, &[440.05], 0.1, 3.0, 4);
        let m = &p[0].modes;
        for (f, s, _) in &tones[..3] {
            let got = m
                .iter()
                .min_by(|a, b| (a.hz - f).abs().total_cmp(&(b.hz - f).abs()))
                .expect("a mode");
            assert!((got.hz - f).abs() < 1e-3, "{f} Hz: found {:?}", m);
            assert!((got.decay - s).abs() < 1e-2, "{f} Hz decay: {:?}", m);
        }
        let b = fastest_beat(&p[0], 2.0, 40.0, 1.0).expect("a beat");
        // The third, 25–30 dB under the first, swings the partial by under a decibel.
        assert!((b.rate - 0.12).abs() < 1e-3, "{b:?}");
    }
}
