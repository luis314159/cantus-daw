//! The fit engine behind EQ match: a target curve (dB against frequency) becomes a handful
//! of ordinary, editable bands.
//!
//! Greedy, then refined. Each round puts a band where the residual is largest — a shelf
//! when that is at an edge and the residual keeps its sign to the edge, a bell elsewhere,
//! with a Q from the width of the bump — then solves every band's gain together (damped
//! Gauss–Newton on the dB curve) and nudges each band's frequency and Q while that lowers
//! the error. The result states its own residual, so the editor can show how close the
//! match is instead of claiming it.

use super::{analog_db, BandSettings, EqShape};

/// What to fit, and within which limits.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FitOptions {
    /// The band budget.
    pub max_bands: usize,
    /// The most a band may boost, dB.
    pub max_boost_db: f32,
    /// The most a band may cut, dB (positive).
    pub max_cut_db: f32,
    /// The range the fit looks at, Hz.
    pub lo_hz: f32,
    pub hi_hz: f32,
}

impl Default for FitOptions {
    fn default() -> Self {
        FitOptions {
            max_bands: 6,
            max_boost_db: 9.0,
            max_cut_db: 12.0,
            lo_hz: 30.0,
            hi_hz: 16_000.0,
        }
    }
}

/// The bands found, and how far their curve is from the target over the fitted range.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    pub bands: Vec<BandSettings>,
    pub rms_db: f32,
    pub max_db: f32,
}

/// Points per octave of the grid the fit works on.
const PER_OCTAVE: f32 = 12.0;

/// The fit's frequency grid.
pub fn grid(lo: f32, hi: f32) -> Vec<f32> {
    let lo = lo.max(10.0);
    let hi = hi.max(lo * 2.0);
    let n = ((hi / lo).log2() * PER_OCTAVE).ceil() as usize + 1;
    (0..n)
        .map(|i| lo * (hi / lo).powf(i as f32 / (n - 1) as f32))
        .collect()
}

/// A curve given at some frequencies, read at `hz` (linear in log frequency, held flat
/// past its ends).
pub fn interpolate(points: &[(f32, f32)], hz: f32) -> f32 {
    match points {
        [] => 0.0,
        [only] => only.1,
        _ => {
            if hz <= points[0].0 {
                return points[0].1;
            }
            for w in points.windows(2) {
                let ((f0, d0), (f1, d1)) = (w[0], w[1]);
                if hz <= f1 {
                    let t = ((hz / f0).ln() / (f1 / f0).ln()).clamp(0.0, 1.0);
                    return d0 + (d1 - d0) * t;
                }
            }
            points[points.len() - 1].1
        }
    }
}

fn band_curve(b: &BandSettings, sr: f32, grid: &[f32], out: &mut [f32]) {
    for (o, f) in out.iter_mut().zip(grid) {
        *o = analog_db(b, sr, *f);
    }
}

/// Bandwidth in octaves → Q.
fn q_of_octaves(bw: f32) -> f32 {
    let r = 2f32.powf(bw.max(0.05));
    r.sqrt() / (r - 1.0)
}

/// Fit `target` (Hz, dB pairs, any order) with at most `opts.max_bands` bands.
pub fn fit(target: &[(f32, f32)], opts: &FitOptions, sample_rate: f32) -> Fit {
    let mut pts: Vec<(f32, f32)> = target
        .iter()
        .copied()
        .filter(|(f, d)| f.is_finite() && d.is_finite() && *f > 0.0)
        .collect();
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let g = grid(opts.lo_hz, opts.hi_hz);
    let t: Vec<f32> = g
        .iter()
        .map(|f| interpolate(&pts, *f).clamp(-opts.max_cut_db, opts.max_boost_db))
        .collect();
    let n = g.len();
    let mut bands: Vec<BandSettings> = Vec::new();
    let mut curves: Vec<Vec<f32>> = Vec::new();
    let total = |curves: &[Vec<f32>], i: usize| -> f32 { curves.iter().map(|c| c[i]).sum() };
    let error = |curves: &[Vec<f32>]| -> f32 {
        (0..n)
            .map(|i| {
                let r = t[i] - total(curves, i);
                r * r
            })
            .sum::<f32>()
            / n as f32
    };
    let clamp_gain = |g: f32| g.clamp(-opts.max_cut_db, opts.max_boost_db);

    for _ in 0..opts.max_bands {
        let resid: Vec<f32> = (0..n).map(|i| t[i] - total(&curves, i)).collect();
        let (peak, &pv) = resid
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap_or((0, &0.0));
        if pv.abs() < 0.25 {
            break;
        }
        let sign = pv.signum();
        let half = pv.abs() * 0.5;
        let mut lo = peak;
        while lo > 0 && resid[lo - 1] * sign > half {
            lo -= 1;
        }
        let mut hi = peak;
        while hi + 1 < n && resid[hi + 1] * sign > half {
            hi += 1;
        }
        let edge = PER_OCTAVE as usize / 2;
        let band = if lo == 0 && peak <= edge {
            BandSettings {
                active: true,
                shape: EqShape::LowShelf,
                freq: g[hi.min(n - 1)],
                gain_db: clamp_gain(pv),
                q: std::f32::consts::FRAC_1_SQRT_2,
                ..BandSettings::default()
            }
        } else if hi == n - 1 && peak + edge >= n - 1 {
            BandSettings {
                active: true,
                shape: EqShape::HighShelf,
                freq: g[lo],
                gain_db: clamp_gain(pv),
                q: std::f32::consts::FRAC_1_SQRT_2,
                ..BandSettings::default()
            }
        } else {
            let bw = (g[hi] / g[lo]).log2() + 1.0 / PER_OCTAVE;
            BandSettings {
                active: true,
                shape: EqShape::Bell,
                freq: g[peak],
                gain_db: clamp_gain(pv),
                q: q_of_octaves(bw).clamp(0.3, 8.0),
                ..BandSettings::default()
            }
        };
        let mut c = vec![0.0; n];
        band_curve(&band, sample_rate, &g, &mut c);
        bands.push(band);
        curves.push(c);
        refine_gains(&mut bands, &mut curves, &t, &g, sample_rate, opts, 2);
    }
    // Nudge frequency and Q while it helps, then settle the gains again.
    for _ in 0..2 {
        for j in 0..bands.len() {
            let before = error(&curves);
            let mut best = (before, bands[j]);
            let steps: [(f32, f32); 4] = [
                (2f32.powf(1.0 / 12.0), 1.0),
                (2f32.powf(-1.0 / 12.0), 1.0),
                (1.0, 2f32.powf(0.25)),
                (1.0, 2f32.powf(-0.25)),
            ];
            for (fm, qm) in steps {
                let mut trial = bands[j];
                trial.freq = (trial.freq * fm).clamp(opts.lo_hz * 0.5, opts.hi_hz * 1.5);
                if trial.shape == EqShape::Bell {
                    trial.q = (trial.q * qm).clamp(0.3, 8.0);
                }
                let saved = std::mem::take(&mut curves[j]);
                let mut c = vec![0.0; n];
                band_curve(&trial, sample_rate, &g, &mut c);
                curves[j] = c;
                let e = error(&curves);
                if e < best.0 {
                    best = (e, trial);
                }
                curves[j] = saved;
            }
            if best.0 < before {
                bands[j] = best.1;
                band_curve(&bands[j], sample_rate, &g, &mut curves[j]);
            }
        }
        refine_gains(&mut bands, &mut curves, &t, &g, sample_rate, opts, 6);
    }
    // Bands that ended up doing nothing are dropped.
    let mut keep = Vec::new();
    let mut kept_curves = Vec::new();
    for (b, c) in bands.into_iter().zip(curves) {
        if b.gain_db.abs() >= 0.1 {
            keep.push(b);
            kept_curves.push(c);
        }
    }
    let resid: Vec<f32> = (0..n).map(|i| t[i] - total(&kept_curves, i)).collect();
    let rms_db = (resid.iter().map(|r| r * r).sum::<f32>() / n.max(1) as f32).sqrt();
    let max_db = resid.iter().fold(0.0f32, |m, r| m.max(r.abs()));
    Fit {
        bands: keep,
        rms_db,
        max_db,
    }
}

/// Solve every band's gain together: damped Gauss–Newton on the dB curve, a few steps.
#[allow(clippy::too_many_arguments)]
fn refine_gains(
    bands: &mut [BandSettings],
    curves: &mut [Vec<f32>],
    t: &[f32],
    g: &[f32],
    sr: f32,
    opts: &FitOptions,
    iterations: usize,
) {
    let m = bands.len();
    let n = g.len();
    if m == 0 {
        return;
    }
    let mut sens = vec![vec![0.0f32; n]; m];
    let mut plus = vec![0.0f32; n];
    let mut minus = vec![0.0f32; n];
    for _ in 0..iterations {
        for j in 0..m {
            let mut b = bands[j];
            b.gain_db += 0.5;
            band_curve(&b, sr, g, &mut plus);
            b.gain_db -= 1.0;
            band_curve(&b, sr, g, &mut minus);
            for i in 0..n {
                sens[j][i] = plus[i] - minus[i];
            }
        }
        let resid: Vec<f32> = (0..n)
            .map(|i| t[i] - curves.iter().map(|c| c[i]).sum::<f32>())
            .collect();
        // (SᵀS + λI) Δ = Sᵀ r
        let mut a = vec![vec![0.0f64; m + 1]; m];
        for (j, row) in a.iter_mut().enumerate() {
            for k in 0..m {
                row[k] = (0..n)
                    .map(|i| sens[j][i] as f64 * sens[k][i] as f64)
                    .sum::<f64>();
            }
            row[j] += 1e-3 * n as f64;
            row[m] = (0..n).map(|i| sens[j][i] as f64 * resid[i] as f64).sum();
        }
        let Some(delta) = solve(a) else {
            return;
        };
        for j in 0..m {
            let g0 = bands[j].gain_db;
            bands[j].gain_db = (g0 + delta[j] as f32).clamp(-opts.max_cut_db, opts.max_boost_db);
            if bands[j].gain_db.abs() < 1e-3 {
                bands[j].gain_db = 1e-3f32.copysign(g0);
            }
            band_curve(&bands[j], sr, g, &mut curves[j]);
        }
    }
}

/// Gaussian elimination with partial pivoting on an augmented matrix.
fn solve(mut a: Vec<Vec<f64>>) -> Option<Vec<f64>> {
    let m = a.len();
    for col in 0..m {
        let pivot = (col..m).max_by(|x, y| a[*x][col].abs().total_cmp(&a[*y][col].abs()))?;
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        let pivot_row = a[col].clone();
        for (row, r) in a.iter_mut().enumerate() {
            if row == col {
                continue;
            }
            let f = r[col] / pivot_row[col];
            if f != 0.0 {
                for (x, p) in r.iter_mut().zip(&pivot_row).skip(col) {
                    *x -= f * p;
                }
            }
        }
    }
    Some((0..m).map(|i| a[i][m] / a[i][i]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve_of(bands: &[BandSettings], hz: f32) -> f32 {
        bands.iter().map(|b| analog_db(b, 48_000.0, hz)).sum()
    }

    /// A target made of known bands is recovered: the fitted curve lands within half a dB
    /// RMS of it with the budget the target was built with.
    #[test]
    fn a_curve_made_of_bands_is_fitted_back() {
        let truth = [
            BandSettings {
                active: true,
                shape: EqShape::Bell,
                freq: 800.0,
                gain_db: 4.0,
                q: 1.2,
                ..BandSettings::default()
            },
            BandSettings {
                active: true,
                shape: EqShape::HighShelf,
                freq: 6_000.0,
                gain_db: -3.0,
                q: 0.71,
                ..BandSettings::default()
            },
            BandSettings {
                active: true,
                shape: EqShape::Bell,
                freq: 120.0,
                gain_db: -5.0,
                q: 2.0,
                ..BandSettings::default()
            },
        ];
        let target: Vec<(f32, f32)> = grid(25.0, 18_000.0)
            .into_iter()
            .map(|f| (f, curve_of(&truth, f)))
            .collect();
        let opts = FitOptions {
            max_bands: 4,
            ..FitOptions::default()
        };
        let fit = fit(&target, &opts, 48_000.0);
        assert!(fit.bands.len() <= 4);
        assert!(
            fit.rms_db < 0.5,
            "rms {} dB with {:?}",
            fit.rms_db,
            fit.bands
        );
        // And the claim is honest: recompute it.
        let g = grid(opts.lo_hz, opts.hi_hz);
        let rms = (g
            .iter()
            .map(|f| {
                let r = curve_of(&truth, *f) - curve_of(&fit.bands, *f);
                r * r
            })
            .sum::<f32>()
            / g.len() as f32)
            .sqrt();
        assert!((rms - fit.rms_db).abs() < 0.05, "{rms} vs {}", fit.rms_db);
    }

    #[test]
    fn limits_and_budget_are_kept() {
        let target = [(30.0, 20.0), (1_000.0, 0.0), (16_000.0, -20.0)];
        let opts = FitOptions {
            max_bands: 2,
            max_boost_db: 6.0,
            max_cut_db: 6.0,
            ..FitOptions::default()
        };
        let fit = fit(&target, &opts, 48_000.0);
        assert!(fit.bands.len() <= 2);
        for b in &fit.bands {
            assert!(b.gain_db <= 6.0 && b.gain_db >= -6.0, "{b:?}");
        }
        // A flat target needs nothing.
        let flat = fit_flat();
        assert!(flat.bands.is_empty() && flat.rms_db == 0.0);
    }

    fn fit_flat() -> Fit {
        fit(
            &[(20.0, 0.0), (20_000.0, 0.0)],
            &FitOptions::default(),
            48_000.0,
        )
    }
}
