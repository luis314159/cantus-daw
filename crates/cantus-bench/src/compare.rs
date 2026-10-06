//! The perf gate: a run against a stored baseline of the same machine class.
//!
//! A gated metric **regresses** when both hold:
//! 1. its median is worse than the baseline's by more than the threshold (25 % by
//!    default — a shared laptop's run-to-run noise is 5–15 % on the kernels), and
//! 2. the runs do not overlap: the current run's *best* decile is worse than the
//!    baseline's *worst* decile (for lower-is-better: current p10 > baseline p90).
//!
//! The second condition is what keeps a noisy machine from failing the build on one bad
//! run; the first is what keeps a tight but harmless shift (2 % slower, very repeatable)
//! from failing it. Improvements are reported, never failed; a metric missing from either
//! side is reported, not failed, so adding a benchmark does not need a new baseline first.

use crate::report::{Better, Report};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Within the threshold or the noise.
    Same,
    /// Better by more than the threshold, and outside the noise.
    Improved,
    /// Worse by more than the threshold, and outside the noise: fails the gate.
    Regressed,
    /// Worse by more than the threshold but the runs overlap: reported as a warning.
    Noisy,
    /// Not gated: reported only.
    Ungated,
    /// In this run, not in the baseline.
    New,
    /// In the baseline, not in this run.
    Missing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub id: String,
    pub unit: String,
    pub baseline: Option<f64>,
    pub current: Option<f64>,
    /// How much worse (positive) or better (negative), percent of the baseline median.
    pub worse_pct: f64,
    pub status: Status,
}

/// Compare `current` with `baseline`, metric by metric.
pub fn compare(current: &Report, baseline: &Report, threshold_pct: f64) -> Vec<Verdict> {
    let mut out = Vec::new();
    for m in &current.metrics {
        let Some(b) = baseline.metric(&m.id) else {
            out.push(Verdict {
                id: m.id.clone(),
                unit: m.unit.clone(),
                baseline: None,
                current: Some(m.summary.median),
                worse_pct: 0.0,
                status: Status::New,
            });
            continue;
        };
        let (c, s) = (&m.summary, &b.summary);
        let worse_pct = if s.median.abs() < f64::EPSILON {
            0.0
        } else {
            match m.better {
                Better::Lower => (c.median - s.median) / s.median.abs() * 100.0,
                Better::Higher => (s.median - c.median) / s.median.abs() * 100.0,
            }
        };
        // The runs are apart when even the current run's good end is past the baseline's
        // bad end (and the other way round for an improvement).
        let (worse_apart, better_apart) = match m.better {
            Better::Lower => (c.p10 > s.p90, c.p90 < s.p10),
            Better::Higher => (c.p90 < s.p10, c.p10 > s.p90),
        };
        let status = if !(m.gate && b.gate) {
            Status::Ungated
        } else if worse_pct > threshold_pct {
            if worse_apart {
                Status::Regressed
            } else {
                Status::Noisy
            }
        } else if -worse_pct > threshold_pct && better_apart {
            Status::Improved
        } else {
            Status::Same
        };
        out.push(Verdict {
            id: m.id.clone(),
            unit: m.unit.clone(),
            baseline: Some(s.median),
            current: Some(c.median),
            worse_pct,
            status,
        });
    }
    for b in &baseline.metrics {
        if current.metric(&b.id).is_none() {
            out.push(Verdict {
                id: b.id.clone(),
                unit: b.unit.clone(),
                baseline: Some(b.summary.median),
                current: None,
                worse_pct: 0.0,
                status: Status::Missing,
            });
        }
    }
    out
}

/// Whether the gate fails.
pub fn failed(verdicts: &[Verdict]) -> bool {
    verdicts.iter().any(|v| v.status == Status::Regressed)
}

/// The verdicts as lines a person reads in a CI log, worst first.
pub fn describe(verdicts: &[Verdict]) -> String {
    let mut v: Vec<&Verdict> = verdicts.iter().collect();
    v.sort_by(|a, b| b.worse_pct.total_cmp(&a.worse_pct));
    let mut s = String::new();
    for x in v {
        let tag = match x.status {
            Status::Same => "ok      ",
            Status::Improved => "better  ",
            Status::Regressed => "REGRESS ",
            Status::Noisy => "noisy   ",
            Status::Ungated => "info    ",
            Status::New => "new     ",
            Status::Missing => "missing ",
        };
        let f = |o: Option<f64>| o.map_or("—".to_string(), crate::report::num);
        s += &format!(
            "{tag} {:<40} {:>10} -> {:>10} {:<10} {:+6.1} %\n",
            x.id,
            f(x.baseline),
            f(x.current),
            x.unit,
            x.worse_pct
        );
    }
    s
}
