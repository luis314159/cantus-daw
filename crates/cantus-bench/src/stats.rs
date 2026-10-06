//! Summaries of repeated measurements. A machine shared with other work is noisy, so
//! every number the suite reports is a **median** of several runs with its **spread**
//! (the 10th and 90th percentiles, min and max), never a single run and never a mean.

use serde::{Deserialize, Serialize};

/// What a set of runs measured.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub median: f64,
    pub p10: f64,
    pub p90: f64,
    pub min: f64,
    pub max: f64,
    pub runs: usize,
}

impl Summary {
    /// Summarise `values` (non-finite ones are dropped). Empty gives zeros.
    pub fn of(values: &[f64]) -> Summary {
        let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
        if v.is_empty() {
            return Summary {
                median: 0.0,
                p10: 0.0,
                p90: 0.0,
                min: 0.0,
                max: 0.0,
                runs: 0,
            };
        }
        v.sort_by(f64::total_cmp);
        Summary {
            median: quantile_sorted(&v, 0.5),
            p10: quantile_sorted(&v, 0.1),
            p90: quantile_sorted(&v, 0.9),
            min: v[0],
            max: v[v.len() - 1],
            runs: v.len(),
        }
    }

    /// A single value (a count, a capacity): no spread.
    pub fn exact(value: f64) -> Summary {
        Summary::of(&[value])
    }

    /// The spread from p10 to p90 relative to the median, in percent: how far a run of
    /// this metric can be trusted on this machine today.
    pub fn spread_pct(&self) -> f64 {
        if self.median.abs() < f64::EPSILON {
            0.0
        } else {
            (self.p90 - self.p10) / self.median.abs() * 100.0
        }
    }
}

/// The `q` quantile of sorted values, interpolated linearly between neighbours.
pub fn quantile_sorted(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let pos = q.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    let t = pos - lo as f64;
    sorted[lo] * (1.0 - t) + sorted[hi] * t
}

/// Quantiles of unsorted values (sorts a copy).
pub fn quantile(values: &[f64], q: f64) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    quantile_sorted(&v, q)
}

/// The median of unsorted values.
pub fn median(values: &[f64]) -> f64 {
    quantile(values, 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_summary_is_the_median_and_its_spread() {
        let s = Summary::of(&[5.0, 1.0, 3.0, 2.0, 4.0, f64::NAN]);
        assert_eq!(s.runs, 5);
        assert_eq!(s.median, 3.0);
        assert_eq!(s.min, 1.0);
        assert_eq!(s.max, 5.0);
        assert!((s.p10 - 1.4).abs() < 1e-9 && (s.p90 - 4.6).abs() < 1e-9);
        assert!((s.spread_pct() - 320.0 / 3.0).abs() < 1e-9);
        assert_eq!(Summary::of(&[]).runs, 0);
        assert_eq!(median(&[2.0, 1.0]), 1.5);
    }
}
