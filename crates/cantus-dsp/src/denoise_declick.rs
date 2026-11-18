//! Click repair for the audio editor (ADR 0096 §5a): find the clicks in a stretch and
//! rebuild each from the audio around it. Offline, deterministic (additions,
//! multiplications, divisions and `sqrt` only: no transcendental function at all).
//!
//! **Finding.** Music and speech are predictable a few milliseconds ahead; a click is not.
//! An autoregressive model of order [`ORDER`] is fitted to each block of [`BLOCK`] samples
//! (autocorrelation of the Welch-windowed block, Levinson–Durbin); a sample whose prediction
//! error exceeds `k` times the block's robust error scale (the median absolute error /
//! 0.6745) is suspect, with `k` from 12 (sensitivity 0) to 4 (sensitivity 1). Suspects
//! closer than [`MERGE`] samples are one click; a click longer than [`MAX_GAP`] is not a
//! click (a drum hit, a consonant) and is left alone.
//!
//! **Rebuilding.** The least-squares autoregressive interpolation (Vaseghi, Godsill & Rayner):
//! the missing samples are those that make the model's prediction errors over the gap and
//! [`ORDER`] samples past it as small as possible, given the model fitted to the audio just
//! before the gap — a small symmetric positive-definite system solved by Cholesky.
//!
//! **Shared** (fx-e:purus): the model's fit ([`ar_fit_into`]), the block's errors and robust
//! scale ([`block_errors`]), the threshold, the widening and the rebuilding ([`rebuild`])
//! work in the caller's scratch, so Purus's real-time de-click runs the same detector and
//! the same rebuilding without allocating. The editor's functions are those cores with their
//! scratch allocated per call — the same arithmetic in the same order, the same bits
//! (`purus_tests_shared.rs` holds the hashes taken before the sharing).

/// The model's order.
pub const ORDER: usize = 32;
/// The block the model is fitted to and the errors judged in.
pub const BLOCK: usize = 2048;
/// Suspects this close are one click.
pub const MERGE: usize = 24;
/// The longest stretch rebuilt as a click.
pub const MAX_GAP: usize = 400;

/// Scratch for [`ar_fit_into`], sized once for the longest stretch and order fitted.
#[derive(Clone, Debug)]
pub struct ArScratch {
    xw: Vec<f64>,
    r: Vec<f64>,
    tmp: Vec<f64>,
}

impl ArScratch {
    pub fn new(max_len: usize, p: usize) -> Self {
        ArScratch {
            xw: vec![0.0; max_len],
            r: vec![0.0; p + 1],
            tmp: vec![0.0; p + 1],
        }
    }
}

/// The prediction-error filter `1 + a1 z^-1 + … + ap z^-p` of `x` (Welch-windowed
/// autocorrelation, Levinson–Durbin). `a[0]` is 1.
pub fn ar_fit(x: &[f32], p: usize) -> Vec<f64> {
    let mut a = vec![0.0f64; p + 1];
    let mut s = ArScratch::new(x.len(), p);
    ar_fit_into(x, p, &mut a, &mut s);
    a
}

/// [`ar_fit`] into `a` (at least `p + 1` long) with the caller's scratch: no allocation.
pub fn ar_fit_into(x: &[f32], p: usize, a: &mut [f64], s: &mut ArScratch) {
    let n = x.len();
    let a = &mut a[..=p];
    a.fill(0.0);
    a[0] = 1.0;
    if n <= p + 1 {
        return;
    }
    // A Welch (parabolic) window, 4 t (1 - t): tapered like Hann, and free of libm.
    let xw = &mut s.xw[..n];
    for (i, (o, v)) in xw.iter_mut().zip(x).enumerate() {
        let t = i as f64 / (n - 1) as f64;
        let w = 4.0 * t * (1.0 - t);
        *o = *v as f64 * w;
    }
    let r = &mut s.r[..=p];
    for (k, rk) in r.iter_mut().enumerate() {
        *rk = xw[k..].iter().zip(&xw[..n - k]).map(|(a, b)| a * b).sum();
    }
    if r[0] <= 1e-18 {
        return;
    }
    // A touch of white noise keeps the recursion stable on very clean signals.
    r[0] *= 1.0 + 1e-9;
    let mut err = r[0];
    let tmp = &mut s.tmp[..=p];
    for i in 1..=p {
        let mut acc = r[i];
        for j in 1..i {
            acc += a[j] * r[i - j];
        }
        let k = -acc / err;
        tmp[..=i].copy_from_slice(&a[..=i]);
        for j in 1..i {
            a[j] = tmp[j] + k * tmp[i - j];
        }
        a[i] = k;
        err *= 1.0 - k * k;
        if err <= 0.0 {
            break;
        }
    }
}

/// The detector's threshold, in robust error scales, at `sensitivity` (0..1): 12 to 4.
pub fn threshold(sensitivity: f32) -> f64 {
    12.0 - 8.0 * sensitivity.clamp(0.0, 1.0) as f64
}

/// The prediction errors of `x[b0..b1]` under the filter `a` into `e`, and the block's
/// robust error scale (the median absolute error / 0.6745, floored at a thousandth of the
/// block's level; `mag` is scratch, sorted in place). No allocation.
pub fn block_errors(
    x: &[f32],
    b0: usize,
    b1: usize,
    a: &[f64],
    e: &mut [f64],
    mag: &mut [f64],
) -> f64 {
    let n = b1 - b0;
    for (o, t) in e[..n].iter_mut().zip(b0..b1) {
        // The file's first samples have no history to be predicted from.
        *o = if t < ORDER {
            0.0
        } else {
            let mut s = x[t] as f64;
            for (j, aj) in a.iter().enumerate().skip(1) {
                s += aj * x[t - j] as f64;
            }
            s
        };
    }
    for (m, v) in mag[..n].iter_mut().zip(&e[..n]) {
        *m = v.abs();
    }
    // The median is the same value whatever the sort (total order; equal means the same
    // bits), so the unstable sort, which never allocates, gives the editor's own.
    mag[..n].sort_unstable_by(|p, q| p.total_cmp(q));
    // The robust scale, floored at a thousandth of the block's level: a perfectly
    // predictable tone (a test sine) has errors at rounding level, which would make
    // every rounding error a click.
    let rms = (x[b0..b1]
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        / n as f64)
        .sqrt();
    (mag[n / 2] / 0.6745).max(1e-3 * rms)
}

/// A click's marks `[a, b)` as the stretch to rebuild: a click shows in the error from its
/// first sample to `ORDER` after its last, so the gap is its marks, two samples wider each
/// side, within `start..end`.
pub fn widen(a: usize, b: usize, start: usize, end: usize) -> (usize, usize) {
    (a.saturating_sub(2).max(start), (b + 2).min(end))
}

/// The clicks in `x[start..end]` at `sensitivity` (0..1), as `[from, to)` stretches.
pub fn detect(x: &[f32], start: usize, end: usize, sensitivity: f32) -> Vec<(usize, usize)> {
    let end = end.min(x.len());
    let k = threshold(sensitivity);
    let mut marks: Vec<usize> = Vec::new();
    let mut e = vec![0.0f64; BLOCK];
    let mut mag = vec![0.0f64; BLOCK];
    let mut b0 = start;
    while b0 < end {
        let b1 = (b0 + BLOCK).min(end);
        // The model from the block with some context before it.
        let fit_from = b0.saturating_sub(BLOCK / 2);
        let a = ar_fit(&x[fit_from..b1], ORDER);
        let sigma = block_errors(x, b0, b1, &a, &mut e, &mut mag);
        if sigma > 1e-9 {
            for (i, v) in e[..b1 - b0].iter().enumerate() {
                if v.abs() > k * sigma {
                    marks.push(b0 + i);
                }
            }
        }
        b0 = b1;
    }
    let mut out: Vec<(usize, usize)> = Vec::new();
    for m in marks {
        match out.last_mut() {
            Some(last) if m <= last.1 + MERGE => last.1 = m + 1,
            _ => out.push((m, m + 1)),
        }
    }
    out.into_iter()
        .map(|(a, b)| widen(a, b, start, end))
        .filter(|(a, b)| b - a <= MAX_GAP)
        .collect()
}

/// Scratch for [`rebuild`]: the model's stretch, the system and its solution, sized once
/// for the longest gap and context.
#[derive(Clone, Debug)]
pub struct GapScratch {
    max_gap: usize,
    fit: Vec<f32>,
    ar: ArScratch,
    a: Vec<f64>,
    au: Vec<f64>,
    d: Vec<f64>,
    g: Vec<f64>,
    rhs: Vec<f64>,
}

impl GapScratch {
    /// For gaps up to `max_gap` with up to `before` and `after` samples of context.
    pub fn new(max_gap: usize, before: usize, after: usize) -> Self {
        let p = ORDER;
        let rows = max_gap + p;
        GapScratch {
            max_gap,
            fit: vec![0.0; before + after],
            ar: ArScratch::new(before + after, p),
            a: vec![0.0; p + 1],
            au: vec![0.0; rows * max_gap],
            d: vec![0.0; rows],
            g: vec![0.0; max_gap * max_gap],
            rhs: vec![0.0; max_gap],
        }
    }
}

/// Rebuild `x[from..to]` from the audio around it. Refused (left untouched) when there is
/// not enough audio before the gap to fit a model, or the gap is too long.
pub fn interpolate(x: &mut [f32], from: usize, to: usize) -> Result<(), &'static str> {
    let m = to.saturating_sub(from);
    if m == 0 {
        return Ok(());
    }
    if m > MAX_GAP {
        return Err("too long");
    }
    let mut s = GapScratch::new(m, BLOCK, BLOCK / 2);
    rebuild(x, from, to, BLOCK, BLOCK / 2, &mut s)
}

/// [`interpolate`] with the model fitted to up to `before` samples before the gap and
/// `after` after it, in the caller's scratch: no allocation.
pub fn rebuild(
    x: &mut [f32],
    from: usize,
    to: usize,
    before: usize,
    after: usize,
    s: &mut GapScratch,
) -> Result<(), &'static str> {
    let m = to.saturating_sub(from);
    if m == 0 {
        return Ok(());
    }
    if m > s.max_gap {
        return Err("too long");
    }
    let p = ORDER;
    if from < 4 * p || to + p > x.len() {
        return Err("too little audio around it");
    }
    // The model from up to `before` samples before the gap and `after` after it.
    let b = &x[from.saturating_sub(before)..from];
    let af = &x[to..(to + after).min(x.len())];
    let nf = b.len() + af.len();
    s.fit[..b.len()].copy_from_slice(b);
    s.fit[b.len()..nf].copy_from_slice(af);
    ar_fit_into(&s.fit[..nf], p, &mut s.a, &mut s.ar);
    let a = &s.a[..=p];
    // Rows: the prediction errors at t = from .. to + p. Each is
    // sum_j a_j x[t - j] = sum over unknowns + known part.
    let rows = m + p;
    let au = &mut s.au[..rows * m];
    au.fill(0.0);
    let d = &mut s.d[..rows];
    d.fill(0.0);
    for r in 0..rows {
        let t = from + r;
        for (j, aj) in a.iter().enumerate() {
            if t < j {
                continue;
            }
            let s = t - j;
            if s >= from && s < to {
                au[r * m + (s - from)] += aj;
            } else if s < x.len() {
                d[r] += aj * x[s] as f64;
            }
        }
    }
    // Normal equations: (Au' Au) u = -Au' d.
    let g = &mut s.g[..m * m];
    g.fill(0.0);
    let rhs = &mut s.rhs[..m];
    rhs.fill(0.0);
    for r in 0..rows {
        let row = &au[r * m..(r + 1) * m];
        for i in 0..m {
            if row[i] == 0.0 {
                continue;
            }
            rhs[i] -= row[i] * d[r];
            for j in 0..m {
                g[i * m + j] += row[i] * row[j];
            }
        }
    }
    // Cholesky, in place (lower).
    for j in 0..m {
        let mut s = g[j * m + j];
        for k in 0..j {
            s -= g[j * m + k] * g[j * m + k];
        }
        if s <= 1e-18 {
            return Err("the model could not rebuild it");
        }
        let l = s.sqrt();
        g[j * m + j] = l;
        for i in j + 1..m {
            let mut v = g[i * m + j];
            for k in 0..j {
                v -= g[i * m + k] * g[j * m + k];
            }
            g[i * m + j] = v / l;
        }
    }
    let y = rhs;
    for i in 0..m {
        let mut v = y[i];
        for k in 0..i {
            v -= g[i * m + k] * y[k];
        }
        y[i] = v / g[i * m + i];
    }
    for i in (0..m).rev() {
        let mut v = y[i];
        for k in i + 1..m {
            v -= g[k * m + i] * y[k];
        }
        y[i] = v / g[i * m + i];
    }
    for (i, v) in y.iter().enumerate() {
        x[from + i] = *v as f32;
    }
    Ok(())
}

/// Find and repair the clicks of `x[start..end]`; returns how many were repaired.
pub fn declick(x: &mut [f32], start: usize, end: usize, sensitivity: f32) -> usize {
    let gaps = detect(x, start, end, sensitivity);
    let mut n = 0;
    for (a, b) in gaps {
        if interpolate(x, a, b).is_ok() {
            n += 1;
        }
    }
    n
}
