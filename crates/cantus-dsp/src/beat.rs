//! **Tempo and the beat grid** of a recording (round 29, ADR 0061): the tempo from the
//! onset strength's autocorrelation, combed over its multiples and weighted toward the
//! tempos people play (the octave resolved toward 70–180 BPM), with a confidence; the beats
//! by dynamic programming (Ellis, JNMR 2007); the downbeat by where the low onsets fall;
//! and a loop told by its length (a whole number of bars). Offline, deterministic.

// Index loops over spectra and frames read clearer than iterator chains here.
#![allow(clippy::needless_range_loop)]

use crate::onset::Onsets;

#[path = "beat_testkit.rs"]
pub mod testkit;
#[cfg(test)]
#[path = "beat_tests.rs"]
mod tests;

/// The slowest and fastest tempos the tracker considers.
pub const MIN_BPM: f64 = 40.0;
pub const MAX_BPM: f64 = 240.0;
/// Where an octave-ambiguous tempo is put.
pub const PREFER: (f64, f64) = (70.0, 180.0);

/// What [`analyse`] finds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grid {
    /// The tempo, BPM (0 when none was found).
    pub bpm: f64,
    /// How sure, 0 … 1.
    pub confidence: f32,
    /// The beats, in samples, in order.
    pub beats: Vec<f64>,
    /// The index in `beats` of the first downbeat.
    pub downbeat: usize,
    /// A loop: the file is this many whole bars at `bpm` (which is then exact).
    pub bars: Option<u32>,
}

impl Grid {
    /// The first downbeat, in samples.
    pub fn first_downbeat(&self) -> Option<f64> {
        self.beats.get(self.downbeat).copied()
    }
}

/// The tempo, the beats and the downbeat of a signal `len` samples long at `rate` whose
/// onsets are `onsets`, in a metre of `beats_per_bar`.
pub fn analyse(onsets: &Onsets, rate: f32, len: usize, beats_per_bar: u32) -> Grid {
    let fps = onsets.fps;
    let odf = normalised(&onsets.odf);
    if fps <= 0.0 || onsets.frames.len() < 2 || odf.iter().all(|v| *v == 0.0) {
        return Grid::default();
    }
    let Some((bpm, confidence)) = tempo(&odf, fps) else {
        return Grid::default();
    };
    let period = 60.0 * fps / bpm;
    let track = track(&odf, period);
    if track.len() < 2 {
        return Grid::default();
    }
    // Each beat on the onset nearest it (within 35 ms), to the sample.
    let hop = rate as f64 / fps;
    let snap = 0.035 * rate as f64;
    let snapped: Vec<Option<f64>> = track
        .iter()
        .map(|&t| {
            let at = t as f64 * hop;
            let i = onsets.frames.partition_point(|o| *o < at);
            [i.checked_sub(1), Some(i)]
                .into_iter()
                .flatten()
                .filter_map(|j| onsets.frames.get(j).copied())
                .filter(|o| (o - at).abs() <= snap)
                .min_by(|a, b| (a - at).abs().total_cmp(&(b - at).abs()))
        })
        .collect();
    let beats = fill(&snapped, track.iter().map(|&t| t as f64 * hop).collect());
    // The tempo from the beats themselves: a least-squares line through them.
    let bpm = fitted(&beats, rate).unwrap_or(bpm);
    let bpb = beats_per_bar.max(1) as usize;
    let downbeat = downbeat(&beats, &onsets.low, &odf, hop, bpb);
    let mut grid = Grid {
        bpm,
        confidence,
        beats,
        downbeat,
        bars: None,
    };
    looped(&mut grid, rate, len, bpb);
    grid
}

/// The onset function less its mean, over its deviation (so the weights below mean the
/// same whatever the material's level).
fn normalised(odf: &[f32]) -> Vec<f64> {
    let n = odf.len().max(1) as f64;
    let mean = odf.iter().map(|v| *v as f64).sum::<f64>() / n;
    let var = odf
        .iter()
        .map(|v| (*v as f64 - mean) * (*v as f64 - mean))
        .sum::<f64>()
        / n;
    let sd = var.sqrt();
    if sd <= 1e-12 {
        return vec![0.0; odf.len()];
    }
    odf.iter().map(|v| (*v as f64 - mean) / sd).collect()
}

/// The tempo and a confidence: the autocorrelation of the onset function, combed over
/// its first four multiples, weighted toward 120 BPM (a log-Gaussian an octave and a half
/// wide), its peak refined by a parabola, then put inside [`PREFER`].
fn tempo(odf: &[f64], fps: f64) -> Option<(f64, f32)> {
    let lag_lo = (60.0 * fps / MAX_BPM).floor() as usize;
    let lag_hi = (60.0 * fps / MIN_BPM).ceil() as usize;
    let n = odf.len();
    if n < 2 * lag_lo + 2 {
        return None;
    }
    let max_lag = (4 * lag_hi + 2).min(n - 1);
    let acf: Vec<f64> = (0..=max_lag)
        .map(|l| (l..n).map(|i| odf[i] * odf[i - l]).sum::<f64>() / (n - l) as f64)
        .collect();
    let at = |x: f64| -> f64 {
        let i = x.floor() as usize;
        if i + 1 > max_lag {
            return 0.0;
        }
        let f = x - i as f64;
        acf[i] * (1.0 - f) + acf[i + 1] * f
    };
    let comb = |l: f64| -> f64 { (1..=4).map(|k| at(l * k as f64) / k as f64).sum::<f64>() };
    let prior = |l: f64| -> f64 {
        let bpm = 60.0 * fps / l;
        let x = crate::math::log2(bpm / 120.0) / 1.5;
        crate::math::exp(-0.5 * x * x)
    };
    let hi = lag_hi.min(max_lag);
    let s = |l: usize| comb(l as f64) * prior(l as f64);
    let scores: Vec<f64> = (lag_lo..=hi).map(s).collect();
    let (best, &top) = scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1).then(b.0.cmp(&a.0)))?;
    if top <= 0.0 {
        return None;
    }
    // A parabola through the peak and its neighbours.
    let l = lag_lo + best;
    let frac = if l > lag_lo && l < hi {
        let (a, b, c) = (s(l - 1), s(l), s(l + 1));
        let d = a - 2.0 * b + c;
        if d < 0.0 {
            (0.5 * (a - c) / d).clamp(-0.5, 0.5)
        } else {
            0.0
        }
    } else {
        0.0
    };
    let bpm = 60.0 * fps / (l as f64 + frac);
    let bpm = octave(odf, fps, bpm);
    let mut sorted = scores.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2].max(0.0);
    let confidence = ((top - median) / top).clamp(0.0, 1.0) as f32;
    Some((bpm, confidence))
}

/// The octave of a tempo: put inside [`PREFER`], then between a tempo and its double (both
/// inside), the faster when the slower's off-beats are nearly as strong as its beats (the
/// slower is then counting every other beat), and the slower when the faster's
/// off-beats are weak (the faster is then counting a subdivision).
fn octave(odf: &[f64], fps: f64, bpm: f64) -> f64 {
    let mut bpm = bpm;
    while bpm < PREFER.0 {
        bpm *= 2.0;
    }
    while bpm > PREFER.1 {
        bpm /= 2.0;
    }
    // (beats, off-beats): the mean onset strength on the best grid of a period, and
    // halfway between its beats.
    let pulse = |bpm: f64| -> (f64, f64) {
        let period = 60.0 * fps / bpm;
        let near = |at: f64| -> f64 {
            let c = at.round() as i64;
            (c - 2..=c + 2)
                .filter(|i| *i >= 0 && (*i as usize) < odf.len())
                .map(|i| odf[i as usize].max(0.0))
                .fold(0.0, f64::max)
        };
        let mean = |phase: f64| -> f64 {
            let mut s = 0.0;
            let mut n = 0usize;
            let mut at = phase;
            while (at as usize) < odf.len() {
                s += near(at);
                n += 1;
                at += period;
            }
            s / n.max(1) as f64
        };
        let steps = period.ceil() as usize;
        let best = (0..steps)
            .map(|p| p as f64)
            .max_by(|a, b| mean(*a).total_cmp(&mean(*b)).then(b.total_cmp(a)))
            .unwrap_or(0.0);
        (mean(best), mean(best + period / 2.0))
    };
    const STRONG: f64 = 0.55;
    // A tempo a hair past a bound (180.06 measured for 180) is still inside.
    let (lo, hi) = (PREFER.0 / 1.02, PREFER.1 * 1.02);
    if bpm * 2.0 <= hi {
        let (on, off) = pulse(bpm);
        if on > 0.0 && off >= STRONG * on {
            return bpm * 2.0;
        }
    } else if bpm / 2.0 >= lo {
        let (on, off) = pulse(bpm / 2.0);
        if on > 0.0 && off < STRONG * on {
            return bpm / 2.0;
        }
    }
    bpm
}
/// The beats, as frames of the onset function: Ellis's dynamic programme — each frame's
/// best score is its onset strength plus the best earlier beat's score, less how far that
/// beat is from one period back (squared, in octaves).
fn track(odf: &[f64], period: f64) -> Vec<usize> {
    const TIGHTNESS: f64 = 100.0;
    let n = odf.len();
    let mut score = vec![0.0f64; n];
    let mut from = vec![usize::MAX; n];
    let lo = ((0.5 * period).round() as usize).max(1);
    let hi = (2.0 * period).round() as usize;
    for t in 0..n {
        let mut best = 0.0f64;
        let mut arg = usize::MAX;
        if t >= lo {
            for p in t.saturating_sub(hi)..=t - lo {
                let x = crate::math::ln((t - p) as f64 / period);
                let v = score[p] - TIGHTNESS * x * x;
                if arg == usize::MAX || v > best {
                    best = v;
                    arg = p;
                }
            }
        }
        if arg == usize::MAX || best < 0.0 {
            best = 0.0;
            arg = usize::MAX;
        }
        score[t] = odf[t] + best;
        from[t] = arg;
    }
    // From the best frame within the last period back.
    let tail = n.saturating_sub(period.ceil() as usize);
    let Some(mut t) = (tail..n).max_by(|&a, &b| score[a].total_cmp(&score[b]).then(b.cmp(&a)))
    else {
        return Vec::new();
    };
    let mut beats = vec![t];
    while from[t] != usize::MAX {
        t = from[t];
        beats.push(t);
    }
    beats.reverse();
    beats
}

/// BPM from beats (samples) by a least-squares line through (index, time).
fn fitted(beats: &[f64], rate: f32) -> Option<f64> {
    let n = beats.len();
    if n < 4 {
        return None;
    }
    let mx = (n - 1) as f64 / 2.0;
    let my = beats.iter().sum::<f64>() / n as f64;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (i, b) in beats.iter().enumerate() {
        let dx = i as f64 - mx;
        sxy += dx * (b - my);
        sxx += dx * dx;
    }
    let per = sxy / sxx;
    (per > 0.0).then(|| 60.0 * rate as f64 / per)
}

/// The index of the first downbeat: of the `bpb` ways to count bars over the beats, the
/// one whose first beats carry the most low onsets (kicks, basses), then the most onsets.
fn downbeat(beats: &[f64], low: &[f32], odf: &[f64], hop: f64, bpb: usize) -> usize {
    if bpb <= 1 {
        return 0;
    }
    let near = |v: &dyn Fn(usize) -> f64, at: f64| -> f64 {
        let c = (at / hop).round() as i64;
        (c - 2..=c + 2)
            .filter(|i| *i >= 0)
            .map(|i| v(i as usize))
            .fold(0.0, f64::max)
    };
    let lowf = |i: usize| low.get(i).map_or(0.0, |v| *v as f64);
    let all = |i: usize| odf.get(i).copied().unwrap_or(0.0).max(0.0);
    let score = |k: usize| -> (f64, f64) {
        beats
            .iter()
            .skip(k)
            .step_by(bpb)
            .fold((0.0, 0.0), |(l, o), &b| {
                (l + near(&lowf, b), o + near(&all, b))
            })
    };
    // A bar of kick-snare-kick-snare counts from its first or its third beat alike: when
    // another way of counting is nearly as good, the earliest (a file starts on its bar).
    let scores: Vec<(f64, f64)> = (0..bpb.min(beats.len())).map(score).collect();
    let best = scores.iter().map(|s| s.0).fold(0.0, f64::max);
    let close: Vec<usize> = (0..scores.len())
        .filter(|&k| scores[k].0 >= 0.85 * best)
        .collect();
    close
        .iter()
        .copied()
        .max_by(|&a, &b| {
            let (sa, sb) = (scores[a], scores[b]);
            let tie = |x: f64, y: f64| (x - y).abs() <= 0.15 * x.max(y);
            if tie(sa.0, sb.0) && tie(sa.1, sb.1) {
                b.cmp(&a)
            } else {
                (sa.0 + sa.1).total_cmp(&(sb.0 + sb.1)).then(b.cmp(&a))
            }
        })
        .unwrap_or(0)
}

/// A loop: a file that starts on its first downbeat (within 30 ms) and lasts a whole
/// number of bars (within 2 % of a bar) — its tempo is then made exact from its length.
fn looped(grid: &mut Grid, rate: f32, len: usize, bpb: usize) {
    let Some(first) = grid.first_downbeat() else {
        return;
    };
    if first > 0.030 * rate as f64 || grid.bpm <= 0.0 {
        return;
    }
    let seconds = len as f64 / rate as f64;
    let bars = seconds * grid.bpm / 60.0 / bpb as f64;
    let whole = bars.round();
    if !(1.0..=256.0).contains(&whole) || (bars - whole).abs() > 0.02 {
        return;
    }
    grid.bars = Some(whole as u32);
    grid.bpm = whole * bpb as f64 * 60.0 / seconds;
}

/// The beats, each on its onset where one was found; a beat with none (a rest, the file's
/// end) placed from its found neighbours — halfway between the beats around it, or a
/// period on from the nearest — rather than where the onset function's frames put it.
fn fill(snapped: &[Option<f64>], rough: Vec<f64>) -> Vec<f64> {
    let n = snapped.len();
    let found: Vec<usize> = (0..n).filter(|&i| snapped[i].is_some()).collect();
    if found.len() < 2 {
        return rough;
    }
    let at = |i: usize| snapped[i].unwrap_or(rough[i]);
    (0..n)
        .map(|i| {
            if let Some(v) = snapped[i] {
                return v;
            }
            let before = found.iter().rev().find(|&&j| j < i).copied();
            let after = found.iter().find(|&&j| j > i).copied();
            match (before, after) {
                (Some(a), Some(b)) => at(a) + (at(b) - at(a)) * (i - a) as f64 / (b - a) as f64,
                (Some(a), None) => {
                    let p = found.iter().rev().find(|&&j| j < a).copied().unwrap_or(a);
                    let per = if p < a {
                        (at(a) - at(p)) / (a - p) as f64
                    } else {
                        0.0
                    };
                    at(a) + per * (i - a) as f64
                }
                (None, Some(b)) => {
                    let q = found.iter().find(|&&j| j > b).copied().unwrap_or(b);
                    let per = if q > b {
                        (at(q) - at(b)) / (q - b) as f64
                    } else {
                        0.0
                    };
                    at(b) - per * (b - i) as f64
                }
                (None, None) => rough[i],
            }
        })
        .collect()
}
