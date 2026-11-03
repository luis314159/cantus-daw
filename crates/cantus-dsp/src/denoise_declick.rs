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

/// The model's order.
pub const ORDER: usize = 32;
/// The block the model is fitted to and the errors judged in.
pub const BLOCK: usize = 2048;
/// Suspects this close are one click.
pub const MERGE: usize = 24;
/// The longest stretch rebuilt as a click.
pub const MAX_GAP: usize = 400;

/// The prediction-error filter `1 + a1 z^-1 + … + ap z^-p` of `x` (Welch-windowed
/// autocorrelation, Levinson–Durbin). `a[0]` is 1.
pub fn ar_fit(x: &[f32], p: usize) -> Vec<f64> {
    let n = x.len();
    let mut a = vec![0.0f64; p + 1];
    a[0] = 1.0;
    if n <= p + 1 {
        return a;
    }
    // A Welch (parabolic) window, 4 t (1 - t): tapered like Hann, and free of libm.
    let w: Vec<f64> = (0..n)
        .map(|i| {
            let t = i as f64 / (n - 1) as f64;
            4.0 * t * (1.0 - t)
        })
        .collect();
    let xw: Vec<f64> = x.iter().zip(&w).map(|(v, w)| *v as f64 * w).collect();
    let mut r = vec![0.0f64; p + 1];
    for (k, rk) in r.iter_mut().enumerate() {
        *rk = xw[k..].iter().zip(&xw[..n - k]).map(|(a, b)| a * b).sum();
    }
    if r[0] <= 1e-18 {
        return a;
    }
    // A touch of white noise keeps the recursion stable on very clean signals.
    r[0] *= 1.0 + 1e-9;
    let mut err = r[0];
    let mut tmp = vec![0.0f64; p + 1];
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
    a
}

/// The clicks in `x[start..end]` at `sensitivity` (0..1), as `[from, to)` stretches.
pub fn detect(x: &[f32], start: usize, end: usize, sensitivity: f32) -> Vec<(usize, usize)> {
    let end = end.min(x.len());
    let k = 12.0 - 8.0 * sensitivity.clamp(0.0, 1.0) as f64;
    let mut marks: Vec<usize> = Vec::new();
    let mut b0 = start;
    while b0 < end {
        let b1 = (b0 + BLOCK).min(end);
        // The model from the block with some context before it.
        let fit_from = b0.saturating_sub(BLOCK / 2);
        let a = ar_fit(&x[fit_from..b1], ORDER);
        let e: Vec<f64> = (b0..b1)
            .map(|t| {
                // The file's first samples have no history to be predicted from.
                if t < ORDER {
                    return 0.0;
                }
                let mut s = x[t] as f64;
                for (j, aj) in a.iter().enumerate().skip(1) {
                    s += aj * x[t - j] as f64;
                }
                s
            })
            .collect();
        let mut mag: Vec<f64> = e.iter().map(|v| v.abs()).collect();
        mag.sort_by(|p, q| p.total_cmp(q));
        // The robust scale, floored at a thousandth of the block's level: a perfectly
        // predictable tone (a test sine) has errors at rounding level, which would make
        // every rounding error a click.
        let rms = (x[b0..b1]
            .iter()
            .map(|v| (*v as f64) * (*v as f64))
            .sum::<f64>()
            / (b1 - b0) as f64)
            .sqrt();
        let sigma = (mag[mag.len() / 2] / 0.6745).max(1e-3 * rms);
        if sigma > 1e-9 {
            for (i, v) in e.iter().enumerate() {
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
    // A click shows in the error from its first sample to `ORDER` after its last: the gap is
    // its marks, two samples wider each side.
    out.into_iter()
        .map(|(a, b)| (a.saturating_sub(2).max(start), (b + 2).min(end)))
        .filter(|(a, b)| b - a <= MAX_GAP)
        .collect()
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
    let p = ORDER;
    if from < 4 * p || to + p > x.len() {
        return Err("too little audio around it");
    }
    // The model from up to a block before the gap and after it.
    let before = &x[from.saturating_sub(BLOCK)..from];
    let after = &x[to..(to + BLOCK / 2).min(x.len())];
    let fit: Vec<f32> = before.iter().chain(after.iter()).copied().collect();
    let a = ar_fit(&fit, p);
    // Rows: the prediction errors at t = from .. to + p. Each is
    // sum_j a_j x[t - j] = sum over unknowns + known part.
    let rows = m + p;
    let mut au = vec![0.0f64; rows * m];
    let mut d = vec![0.0f64; rows];
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
    let mut g = vec![0.0f64; m * m];
    let mut rhs = vec![0.0f64; m];
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
    let mut y = rhs;
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
