//! **Onsets** — where the transients of a recording are (round 29, ADR 0061): SuperFlux
//! (Böck and Widmer, DAFx 2013) — a log-magnitude spectrogram on a logarithmic filterbank,
//! the positive difference to a frame 10 ms earlier taken after a maximum filter across
//! frequency (so a vibrato or a glide is not a new note), summed — then peaks picked by a
//! moving maximum, an adaptive threshold over a moving mean, and a minimum gap; each onset
//! then refined to the sample where its attack starts. Offline, deterministic, allocates.

// Index loops over spectra and frames read clearer than iterator chains here.
#![allow(clippy::needless_range_loop)]

use crate::math;

/// What [`detect`] finds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Onsets {
    /// The onset-strength function's frames per second.
    pub fps: f64,
    /// The onset strength of every frame (frame `i` at `i / fps` seconds).
    pub odf: Vec<f32>,
    /// The same below 200 Hz only (kicks and basses): the downbeat's cue.
    pub low: Vec<f32>,
    /// The onsets, in samples (the attack's first sample), in order.
    pub frames: Vec<f64>,
    /// Each onset's strength (its peak in `odf`).
    pub strengths: Vec<f32>,
}

/// The onset function's frame rate.
const FPS: f64 = 200.0;
/// Filterbank bands per octave, and its range, Hz.
const BANDS_PER_OCTAVE: f64 = 24.0;
const F_LO: f64 = 30.0;
const F_HI: f64 = 16_000.0;
/// The "low" function's top, Hz.
const LOW_HI: f64 = 200.0;
/// Peak picking, seconds: the moving maximum before and after, the moving mean before and
/// after, and the shortest gap between two onsets.
const PRE_MAX: f64 = 0.030;
const POST_MAX: f64 = 0.030;
const PRE_AVG: f64 = 0.100;
const POST_AVG: f64 = 0.070;
const MIN_GAP: f64 = 0.030;
/// The threshold over the moving mean: this share of the function's top.
const DELTA: f32 = 0.06;

/// Find the onsets of a mono signal at `rate`.
pub fn detect(x: &[f32], rate: f32) -> Onsets {
    let sr = rate as f64;
    let n = ((0.046 * sr) as usize)
        .next_power_of_two()
        .clamp(512, 8_192);
    let hop = (sr / FPS).round().max(1.0) as usize;
    let fps = sr / hop as f64;
    let frames = x.len().div_ceil(hop).max(1);
    let bank = Bank::new(n, sr);
    let fft = crate::aequalis::fft::Fft::new(n);
    let win: Vec<f32> = (0..n)
        .map(|i| (0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / n as f64)) as f32)
        .collect();
    let (mut re, mut im) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut mag = vec![0.0f32; n / 2 + 1];
    let bands = bank.len();
    let mut spec: Vec<Vec<f32>> = Vec::with_capacity(frames);
    let mut level: Vec<f32> = Vec::with_capacity(frames);
    for t in 0..frames {
        let c = (t * hop) as i64 - (n / 2) as i64;
        for i in 0..n {
            let at = c + i as i64;
            re[i] = if at >= 0 && (at as usize) < x.len() {
                x[at as usize] * win[i]
            } else {
                0.0
            };
            im[i] = 0.0;
        }
        fft.run(&mut re, &mut im, false);
        for (k, m) in mag.iter_mut().enumerate() {
            *m = math::hypot(re[k], im[k]);
        }
        let mut row = vec![0.0f32; bands];
        bank.apply(&mag, &mut row);
        level.push(mag.iter().map(|m| m * m).sum::<f32>());
        for v in &mut row {
            *v = math::log10(1.0 + *v);
        }
        spec.push(row);
    }
    // SuperFlux: the difference to the frame `lag` back, maximum-filtered across 3 bands.
    let lag = ((0.25 * n as f64 / hop as f64).round() as usize).max(1);
    let low_bands = bank.below(LOW_HI);
    let mut odf = vec![0.0f32; frames];
    let mut low = vec![0.0f32; frames];
    // Before the signal is silence: a file that starts on a hit has an onset at 0.
    let silent = vec![0.0f32; bands];
    for t in 0..frames {
        let (now, then) = (&spec[t], t.checked_sub(lag).map_or(&silent, |p| &spec[p]));
        let before = t.checked_sub(lag).map_or(0.0, |p| level[p]);
        let (mut s, mut l) = (0.0f32, 0.0f32);
        for b in 0..bands {
            let lo = b.saturating_sub(1);
            let hi = (b + 1).min(bands - 1);
            let m = then[lo..=hi].iter().fold(f32::MIN, |a, v| a.max(*v));
            let d = (now[b] - m).max(0.0);
            s += d;
            if b < low_bands {
                l += d;
            }
        }
        // A frame growing quieter is an end, not an onset (its leakage rises in bands
        // that were silent, but its power falls): weigh the flux by the power it kept.
        let keep = (level[t] / before.max(1e-12)).min(1.0);
        odf[t] = s * keep;
        low[t] = l * keep;
    }
    let picked = pick(&odf, fps);
    let mut out = Onsets {
        fps,
        odf,
        low,
        frames: Vec::with_capacity(picked.len()),
        strengths: Vec::with_capacity(picked.len()),
    };
    let gap = MIN_GAP * sr;
    for t in picked {
        let at = refine(x, t * hop, n, lag * hop, rate);
        // An onset makes the signal louder: an end, a fade or a cut also makes the
        // spectrum change (its splatter), but never louder (+3 dB over 20 ms).
        if !louder(x, at as usize, (0.020 * sr) as usize) {
            continue;
        }
        if out.frames.last().is_some_and(|l| at - l < gap) {
            continue;
        }
        out.frames.push(at);
        out.strengths.push(out.odf[t]);
    }
    out
}

/// The frames where the onset function peaks: the largest within ±30 ms, above its mean
/// over 100 ms before and 70 ms after by a share of its top, 30 ms after the last.
fn pick(odf: &[f32], fps: f64) -> Vec<usize> {
    let f = |s: f64| (s * fps).round() as usize;
    let (pre_max, post_max, pre_avg, post_avg, gap) =
        (f(PRE_MAX), f(POST_MAX), f(PRE_AVG), f(POST_AVG), f(MIN_GAP));
    let top = odf.iter().fold(0.0f32, |m, v| m.max(*v));
    if top <= 0.0 {
        return Vec::new();
    }
    let delta = DELTA * top;
    let mut out: Vec<usize> = Vec::new();
    for t in 0..odf.len() {
        let v = odf[t];
        if v <= delta {
            continue;
        }
        let lo = t.saturating_sub(pre_max);
        let hi = (t + post_max + 1).min(odf.len());
        // The first of equal maxima.
        if odf[lo..hi].iter().any(|w| *w > v) || odf[lo..t].contains(&v) {
            continue;
        }
        let lo = t.saturating_sub(pre_avg);
        let hi = (t + post_avg + 1).min(odf.len());
        let mean = odf[lo..hi].iter().sum::<f32>() / (hi - lo) as f32;
        if v < mean + delta {
            continue;
        }
        if out.last().is_some_and(|l| t - l < gap) {
            continue;
        }
        out.push(t);
    }
    out
}

/// The sample where the attack picked at frame centre `centre` starts. The frame's flux
/// rises as soon as the attack enters its window, so the attack is searched from a quarter
/// window before the centre to the window's end and a lag past it: the block of 0.3 ms
/// whose level jumps most above the 10 ms before it, then walked back to where it first
/// passes a tenth of what it reaches (and twice what was there), to the sample.
fn refine(x: &[f32], centre: usize, n: usize, reach: usize, rate: f32) -> f64 {
    let lo = centre.saturating_sub(n / 4);
    let hi = (centre + n / 2 + reach).min(x.len());
    if lo >= hi {
        return centre as f64;
    }
    let block = ((0.0003 * rate) as usize).max(4);
    let env: Vec<f32> = x[lo..hi]
        .chunks(block)
        .map(|c| c.iter().fold(0.0f32, |m, v| m.max(v.abs())))
        .collect();
    let top = env.iter().fold(0.0f32, |m, v| m.max(*v));
    if top <= 0.0 {
        return centre as f64;
    }
    let back = ((0.010 * rate) as usize / block).max(1);
    let eps = 0.02 * top;
    let before = |j: usize| -> f32 {
        env[j.saturating_sub(back)..j]
            .iter()
            .fold(0.0f32, |m, v| m.max(*v))
    };
    let jump = (1..env.len())
        .max_by(|&a, &b| {
            let sa = env[a] / (before(a) + eps);
            let sb = env[b] / (before(b) + eps);
            sa.total_cmp(&sb).then(b.cmp(&a))
        })
        .unwrap_or(0);
    let reached = env[jump..(jump + back).min(env.len())]
        .iter()
        .fold(0.0f32, |m, v| m.max(*v));
    let level = (0.1 * reached).max(2.0 * before(jump));
    let mut j = jump;
    while j > 0 && env[j - 1] >= level {
        j -= 1;
    }
    let from = lo + j.saturating_sub(1) * block;
    let to = (lo + (jump + 1) * block).min(x.len());
    (from..to)
        .find(|&i| x[i].abs() >= level)
        .unwrap_or(lo + j * block) as f64
}

/// The logarithmic filterbank: triangular filters on bins, 24 per octave, no two on the
/// same centre bin.
struct Bank {
    /// Each band's (first bin, centre bin, last bin).
    bands: Vec<(usize, usize, usize)>,
    /// Each band's centre frequency.
    centres: Vec<f64>,
}

impl Bank {
    fn new(n: usize, sr: f64) -> Self {
        let bin_hz = sr / n as f64;
        let top = F_HI.min(sr / 2.0 * 0.95);
        let count = (BANDS_PER_OCTAVE * math::log2(top / F_LO)).floor() as usize;
        let mut bins: Vec<usize> = (0..=count + 1)
            .map(|i| {
                let f = F_LO * math::exp2(i as f64 / BANDS_PER_OCTAVE);
                (f / bin_hz).round() as usize
            })
            .collect();
        bins.dedup();
        let bands: Vec<(usize, usize, usize)> =
            bins.windows(3).map(|w| (w[0], w[1], w[2])).collect();
        let centres = bands.iter().map(|b| b.1 as f64 * bin_hz).collect();
        Bank { bands, centres }
    }

    fn len(&self) -> usize {
        self.bands.len()
    }

    /// How many bands centre below `hz`.
    fn below(&self, hz: f64) -> usize {
        self.centres.iter().filter(|c| **c < hz).count()
    }

    fn apply(&self, mag: &[f32], out: &mut [f32]) {
        for (o, &(a, c, b)) in out.iter_mut().zip(&self.bands) {
            let mut s = 0.0f32;
            for (k, m) in mag.iter().enumerate().take(b + 1).skip(a) {
                let w = if k <= c {
                    if c == a {
                        1.0
                    } else {
                        (k - a) as f32 / (c - a) as f32
                    }
                } else {
                    (b - k) as f32 / (b - c) as f32
                };
                s += w * m;
            }
            *o = s;
        }
    }
}

#[cfg(test)]
#[path = "onset_tests.rs"]
mod tests;

/// Whether the `w` samples from `at` are at least 3 dB louder than the `w` before it —
/// as they are, or in their first difference (a hi-hat over a kick's tail is louder up
/// high).
fn louder(x: &[f32], at: usize, w: usize) -> bool {
    let e = |a: usize, b: usize, diff: bool| -> f64 {
        let (a, b) = (a.min(x.len()), b.min(x.len()));
        (a.max(1)..b)
            .map(|i| {
                let v = if diff { x[i] - x[i - 1] } else { x[i] } as f64;
                v * v
            })
            .sum()
    };
    let lo = at.saturating_sub(w);
    [false, true]
        .into_iter()
        .any(|d| e(at, at + w, d) > 2.0 * e(lo, at, d))
}
