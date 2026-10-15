//! The offline tracker: a whole take in, f0 per hop smoothed by a Viterbi pass (pYIN-style),
//! voicing, level, sibilance, and the epochs.

use super::{
    fine_period_centred, level_and_sibilance, Design, PitchFrame, Yin, F_MAX, F_MIN, GATE_DB,
};
use crate::math;

/// The offline hop, in streaming hops (256 samples at 48 kHz: 5.3 ms).
const HOPS: usize = 2;
/// Candidates kept per frame.
const MAX_CANDIDATES: usize = 8;
/// The pitch states: 20 cents apart from F_MIN to F_MAX.
const BIN_CENTS: f32 = 20.0;
/// The widest move between two frames, in bins (2.6 semitones per 5.3 ms).
const MAX_STEP: usize = 13;
/// pYIN's trust in YIN: the share of a frame's probability a voiced state can take.
const YIN_TRUST: f32 = 0.5;
/// The chance of a voiced stretch ending (or an unvoiced one starting) between frames.
const SWITCH: f32 = 0.01;
/// The prior on YIN's thresholds: Beta(2, 18) over 0.01 … 1.00 (pYIN's choice, mean 0.1).
const THRESHOLDS: usize = 100;

/// A pitch mark: a point one period after the last on the same phase of the waveform.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Epoch {
    /// Where, in samples (fractional) from the start of the take.
    pub at: f64,
    /// The local period, samples.
    pub period: f32,
}

/// What [`analyse`] finds in a take.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Analysis {
    pub sample_rate: f32,
    /// Samples between frames; frame `i` is centred on sample `i · hop`.
    pub hop: usize,
    pub frames: Vec<PitchFrame>,
    /// The epochs of every voiced stretch, in order.
    pub epochs: Vec<Epoch>,
}

impl Analysis {
    /// The frame nearest sample `t`.
    pub fn frame_at(&self, t: f64) -> Option<&PitchFrame> {
        if self.frames.is_empty() {
            return None;
        }
        let i = (t / self.hop as f64).round().max(0.0) as usize;
        self.frames.get(i.min(self.frames.len() - 1))
    }

    /// The f0 at sample `t`, Hz, interpolated between the two frames around it when both
    /// are voiced; `None` where the take is unvoiced.
    pub fn f0_at(&self, t: f64) -> Option<f32> {
        let x = t / self.hop as f64;
        if x < 0.0 || self.frames.is_empty() {
            return None;
        }
        let i = x.floor() as usize;
        let a = self.frames.get(i)?;
        let b = self.frames.get(i + 1);
        let frac = (x - i as f64) as f32;
        match b {
            Some(b) if a.voiced && b.voiced => Some(a.f0 + (b.f0 - a.f0) * frac),
            _ if frac < 0.5 && a.voiced => Some(a.f0),
            Some(b) if b.voiced => Some(b.f0),
            _ => None,
        }
    }

    /// The voiced stretches as ranges of frames (end exclusive).
    pub fn voiced_runs(&self) -> Vec<(usize, usize)> {
        let mut runs = Vec::new();
        let mut start = None;
        for (i, f) in self.frames.iter().enumerate() {
            match (f.voiced, start) {
                (true, None) => start = Some(i),
                (false, Some(s)) => {
                    runs.push((s, i));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            runs.push((s, self.frames.len()));
        }
        runs
    }
}

/// One frame's candidates: coarse lags (refined, coarse-rate samples) and probabilities.
#[derive(Copy, Clone, Debug, Default)]
struct Candidates {
    n: usize,
    tau: [f32; MAX_CANDIDATES],
    prob: [f32; MAX_CANDIDATES],
}

/// Analyse a mono take (see the module docs). Deterministic; allocates.
pub fn analyse(x: &[f32], sample_rate: f32) -> Analysis {
    let design = Design::new(sample_rate);
    let d = design.decimation;
    let hop = design.hop * HOPS;
    let dec = decimate(x, &design);
    let frames_n = x.len().div_ceil(hop).max(1);
    let (w, tau_min, tau_max) = (design.window, design.tau_min, design.tau_max);
    let wf = design.fine_window();
    let coarse_half = (w + tau_max) / 2 + 2;
    let coarse_len = 2 * coarse_half + 1;
    let fine_half = (wf + tau_max * d + d) / 2 + 3;
    let fine_len = 2 * fine_half + 1;
    let mut yin = Yin::new(tau_max);
    let mut lin = vec![0.0f32; coarse_len];
    let mut lin_fine = vec![0.0f32; fine_len];
    let mut scratch = vec![0.0f32; 2 * d + 6];
    let prior = beta_prior();

    let mut frames = Vec::with_capacity(frames_n);
    let mut cands = Vec::with_capacity(frames_n);
    for i in 0..frames_n {
        let c = (i * hop) as i64;
        // Both windows are centred on the frame's sample.
        fill(&mut lin, &dec, c.div_euclid(d as i64) + coarse_half as i64);
        fill(&mut lin_fine, x, c + fine_half as i64);
        let raw_e: f32 = lin_fine[fine_half - wf / 2..fine_half - wf / 2 + wf]
            .iter()
            .map(|v| v * v)
            .sum();
        let low_e: f32 = lin[coarse_half - w / 2..coarse_half - w / 2 + w]
            .iter()
            .map(|v| v * v)
            .sum();
        let (level_db, sibilance) = level_and_sibilance(raw_e, wf, low_e, d);
        yin.run_centred(&lin, coarse_half, w, tau_max);
        let mut cs = candidates(&yin, tau_min, tau_max, &prior);
        // The gate and the sibilance cue take the voiced probability away.
        let keep = if level_db <= GATE_DB {
            0.0
        } else {
            1.0 - ramp(sibilance, 0.4, 0.7)
        };
        for p in &mut cs.prob[..cs.n] {
            *p *= keep;
        }
        let best = (0..cs.n).max_by(|&a, &b| cs.prob[a].total_cmp(&cs.prob[b]).then(b.cmp(&a)));
        let global = yin.global_min(tau_min, tau_max);
        let tau = best
            .map(|k| cs.tau[k])
            .unwrap_or(yin.refine(global, tau_max));
        let silent = level_db <= -120.0;
        let period = if silent {
            0.0
        } else {
            fine_period_centred(
                &lin_fine,
                fine_half,
                wf,
                tau * d as f32,
                d + 1,
                &mut scratch,
            )
        };
        frames.push(PitchFrame {
            centre: c as f64,
            f0: if period > 0.0 {
                design.sample_rate / period
            } else {
                0.0
            },
            period,
            voiced: false,
            aperiodicity: yin.cmnd[(tau.round() as usize).clamp(1, tau_max)],
            level_db,
            sibilance,
            voiced_prob: cs.prob[..cs.n].iter().sum(),
        });
        cands.push(cs);
    }

    let fs = design.coarse_rate();
    let path = viterbi(&cands, fs);
    for (i, state) in path.into_iter().enumerate() {
        let Some(bin) = state else { continue };
        let target = bin_hz(bin);
        // The candidate nearest the state's pitch, refined at the full rate.
        let cs = &cands[i];
        let near = (0..cs.n).min_by(|&a, &b| {
            let da = math::ln(fs / cs.tau[a] / target).abs();
            let db = math::ln(fs / cs.tau[b] / target).abs();
            da.total_cmp(&db)
        });
        let tau = near.map(|k| cs.tau[k]).unwrap_or(fs / target);
        let c = (i * hop) as i64;
        fill(&mut lin_fine, x, c + fine_half as i64);
        let period = fine_period_centred(
            &lin_fine,
            fine_half,
            wf,
            tau * d as f32,
            d + 1,
            &mut scratch,
        );
        let f = &mut frames[i];
        f.voiced = true;
        f.period = period;
        f.f0 = design.sample_rate / period;
    }

    let mut a = Analysis {
        sample_rate: design.sample_rate,
        hop,
        frames,
        epochs: Vec::new(),
    };
    a.epochs = epochs(x, &dec, &design, &a);
    a
}

/// `out` filled with `src[newest + 1 − out.len() ..= newest]`, zeros outside `src`.
fn fill(out: &mut [f32], src: &[f32], newest: i64) {
    let start = newest + 1 - out.len() as i64;
    for (k, v) in out.iter_mut().enumerate() {
        let j = start + k as i64;
        *v = if j >= 0 && (j as usize) < src.len() {
            src[j as usize]
        } else {
            0.0
        };
    }
}

/// The whole take through the decimator, aligned: decimated sample `k` is the low band at
/// full-rate sample `k · D`.
fn decimate(x: &[f32], design: &Design) -> Vec<f32> {
    let d = design.decimation;
    let n = design.fir.len();
    let g = (n / 2) as i64;
    (0..x.len().div_ceil(d))
        .map(|k| {
            let at = (k * d) as i64 + g;
            let mut acc = 0.0f32;
            for (j, h) in design.fir.iter().enumerate() {
                let i = at - j as i64;
                if i >= 0 && (i as usize) < x.len() {
                    acc += h * x[i as usize];
                }
            }
            acc
        })
        .collect()
}

/// A value's place on a ramp from `lo` (0) to `hi` (1), smoothed at both ends.
fn ramp(v: f32, lo: f32, hi: f32) -> f32 {
    let x = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The Beta(2, 18) weights of the thresholds 0.01 … 1.00, summing to 1.
fn beta_prior() -> [f32; THRESHOLDS] {
    let mut w = [0.0f64; THRESHOLDS];
    for (k, v) in w.iter_mut().enumerate() {
        let s = (k + 1) as f64 / THRESHOLDS as f64;
        // s^(a−1) (1 − s)^(b−1), a = 2, b = 18.
        *v = s * math::powi(1.0 - s, 17);
    }
    let sum: f64 = w.iter().sum();
    let mut out = [0.0f32; THRESHOLDS];
    for (o, v) in out.iter_mut().zip(w) {
        *o = (v / sum) as f32;
    }
    out
}

/// A frame's candidates: each dip of the CMNDF (in order of lag) takes the thresholds
/// under which it is the first dip below; when no dip is below a threshold, the global
/// minimum takes a hundredth of its weight (pYIN's rule).
fn candidates(yin: &Yin, lo: usize, hi: usize, prior: &[f32; THRESHOLDS]) -> Candidates {
    let c = &yin.cmnd;
    let mut out = Candidates::default();
    let mut floor = f32::INFINITY; // the lowest bottom of the dips before
    let mut tau = lo;
    let weight_of = |from: f32, to: f32| -> f32 {
        // thresholds s with from < s ≤ to
        prior
            .iter()
            .enumerate()
            .filter(|(k, _)| {
                let s = (k + 1) as f32 / THRESHOLDS as f32;
                s > from && s <= to
            })
            .map(|(_, w)| *w)
            .sum()
    };
    while tau <= hi && out.n < MAX_CANDIDATES {
        let is_min = (tau == lo || c[tau] <= c[tau - 1]) && (tau == hi || c[tau] < c[tau + 1]);
        if is_min && c[tau] < floor && c[tau] < 1.0 {
            let p = weight_of(c[tau], floor.min(1.0));
            if p > 0.0 {
                out.tau[out.n] = yin.refine(tau, hi);
                out.prob[out.n] = p;
                out.n += 1;
            }
            floor = c[tau];
        }
        tau += 1;
    }
    // Thresholds no dip reached: the global minimum, at a hundredth.
    let rest = weight_of(-1.0, floor.min(1.0));
    if rest > 0.0 && out.n < MAX_CANDIDATES {
        let g = yin.global_min(lo, hi);
        let t = yin.refine(g, hi);
        if let Some(k) = (0..out.n).find(|&k| (out.tau[k] - t).abs() < 0.5) {
            out.prob[k] += rest * 0.01;
        } else {
            out.tau[out.n] = t;
            out.prob[out.n] = rest * 0.01;
            out.n += 1;
        }
    }
    out
}

fn bins() -> usize {
    (1200.0 * math::log2(F_MAX / F_MIN) / BIN_CENTS).ceil() as usize + 1
}

fn bin_hz(b: usize) -> f32 {
    F_MIN * math::exp2(b as f32 * BIN_CENTS / 1200.0)
}

fn hz_bin(hz: f32) -> Option<usize> {
    if hz.is_nan() || hz <= 0.0 {
        return None;
    }
    let b = (1200.0 * math::log2(hz / F_MIN) / BIN_CENTS).round();
    (b >= 0.0 && (b as usize) < bins()).then_some(b as usize)
}

/// The most likely path through voiced and unvoiced pitch states: per frame, the voiced
/// state's bin or `None`.
fn viterbi(cands: &[Candidates], fs: f32) -> Vec<Option<usize>> {
    let n = bins();
    let t_len = cands.len();
    if t_len == 0 {
        return Vec::new();
    }
    // log T(i, j): a triangle of half-width MAX_STEP, normalised per source bin.
    let m = MAX_STEP as i64;
    let row_norm: Vec<f32> = (0..n as i64)
        .map(|i| {
            let s: f32 = (-m..=m)
                .filter(|o| (0..n as i64).contains(&(i + o)))
                .map(|o| (m + 1 - o.abs()) as f32)
                .sum();
            math::ln(s)
        })
        .collect();
    let log_tri: Vec<f32> = (0..=m).map(|o| math::ln((m + 1 - o) as f32)).collect();
    let (stay, switch) = (math::ln(1.0 - SWITCH), math::ln(SWITCH));
    let floor = 1e-12f32;

    // δ for voiced (0..n) and unvoiced (n..2n) states.
    let mut delta = vec![0.0f32; 2 * n];
    let mut next = vec![0.0f32; 2 * n];
    let mut obs = vec![0.0f32; 2 * n];
    let mut best_in = vec![0.0f32; 2 * n];
    let mut back = vec![0u8; t_len * 2 * n];
    let observe = |cs: &Candidates, obs: &mut [f32]| {
        obs[..n].fill(0.0);
        let mut total = 0.0;
        for k in 0..cs.n {
            if let Some(b) = hz_bin(fs / cs.tau[k]) {
                obs[b] += cs.prob[k] * YIN_TRUST;
                total += cs.prob[k] * YIN_TRUST;
            }
        }
        let un = ((1.0 - total) / n as f32).max(floor);
        for o in &mut obs[..n] {
            *o = math::ln(o.max(floor));
        }
        obs[n..].fill(math::ln(un));
    };
    observe(&cands[0], &mut obs);
    let start = -math::ln(2.0 * n as f32);
    for s in 0..2 * n {
        delta[s] = start + obs[s];
    }
    for t in 1..t_len {
        observe(&cands[t], &mut obs);
        // For each target voicing, the best way into source bin i (stay or switch).
        // best_in[v·n + i] = max(δ(v, i) + stay, δ(¬v, i) + switch) − row_norm(i)
        for v in 0..2 {
            for i in 0..n {
                let same = delta[v * n + i] + stay;
                let other = delta[(1 - v) * n + i] + switch;
                best_in[v * n + i] = same.max(other) - row_norm[i];
            }
        }
        let bt = &mut back[t * 2 * n..(t + 1) * 2 * n];
        for v in 0..2 {
            for j in 0..n {
                let lo = j.saturating_sub(MAX_STEP);
                let hi = (j + MAX_STEP).min(n - 1);
                let mut best = f32::NEG_INFINITY;
                let mut arg = j;
                for i in lo..=hi {
                    let s = best_in[v * n + i] + log_tri[i.abs_diff(j)];
                    if s > best {
                        best = s;
                        arg = i;
                    }
                }
                next[v * n + j] = best + obs[v * n + j];
                // Which voicing the best source had: re-derive it.
                let same = delta[v * n + arg] + stay;
                let other = delta[(1 - v) * n + arg] + switch;
                let from_v = if same >= other { v } else { 1 - v };
                let off = (arg as i64 - j as i64 + m) as u8; // 0 ..= 2m
                bt[v * n + j] = off | ((from_v as u8) << 7);
            }
        }
        std::mem::swap(&mut delta, &mut next);
    }
    let mut s = (0..2 * n)
        .max_by(|&a, &b| delta[a].total_cmp(&delta[b]).then(b.cmp(&a)))
        .unwrap_or(n);
    let mut path = vec![None; t_len];
    for t in (0..t_len).rev() {
        let (v, j) = (s / n, s % n);
        path[t] = (v == 0).then_some(j);
        if t == 0 {
            break;
        }
        let b = back[t * 2 * n + s];
        let from_v = (b >> 7) as usize;
        let i = (j as i64 + (b & 0x7f) as i64 - m) as usize;
        s = from_v * n + i;
    }
    path
}

/// The epochs of every voiced stretch: anchored on the stretch's largest peak in its
/// first period, then one period at a time, each refined by the cross-correlation with the
/// period before (coarse on the low band, fine at the full rate) so every mark sits on the
/// same phase of the waveform.
fn epochs(x: &[f32], dec: &[f32], design: &Design, a: &Analysis) -> Vec<Epoch> {
    let d = design.decimation;
    let mut out = Vec::new();
    let half_hop = a.hop as f64 * 0.5;
    for (r0, r1) in a.voiced_runs() {
        let start = (r0 as f64 * a.hop as f64 - half_hop).max(0.0);
        let end = ((r1 - 1) as f64 * a.hop as f64 + half_hop).min(x.len() as f64 - 1.0);
        let period_at = |t: f64| -> f32 {
            let i = ((t / a.hop as f64).floor().max(r0 as f64) as usize).min(r1 - 1);
            let j = (i + 1).min(r1 - 1);
            let frac = ((t / a.hop as f64) - i as f64).clamp(0.0, 1.0) as f32;
            let (p, q) = (a.frames[i].period, a.frames[j].period);
            p + (q - p) * frac
        };
        let p0 = period_at(start);
        if p0.is_nan() || p0 <= 1.0 {
            continue;
        }
        // The anchor: the largest sample in the first period.
        let s0 = start as usize;
        let s1 = ((start + p0 as f64) as usize).min(x.len() - 1);
        let mut m = s0;
        for i in s0..=s1 {
            if x[i] > x[m] {
                m = i;
            }
        }
        let mut mark = m as f64;
        while mark <= end {
            let p = period_at(mark);
            out.push(Epoch {
                at: mark,
                period: p,
            });
            let pred = mark + p as f64;
            if pred > end {
                break;
            }
            mark = align(x, dec, d, mark, pred, p);
        }
    }
    out
}

/// The point near `pred` whose surroundings best match those of `prev`: the lag of the
/// highest cross-correlation within ±p/8, coarse on the low band, then within ±D at the
/// full rate, refined by a parabola.
fn align(x: &[f32], dec: &[f32], d: usize, prev: f64, pred: f64, p: f32) -> f64 {
    let corr = |s: &[f32], a: i64, b: i64, h: i64| -> f32 {
        let mut acc = 0.0f32;
        for j in -h..=h {
            let (i, k) = (a + j, b + j);
            if i >= 0 && k >= 0 && (i as usize) < s.len() && (k as usize) < s.len() {
                acc += s[i as usize] * s[k as usize];
            }
        }
        acc
    };
    // Coarse.
    let dd = d as f64;
    let (pa, pb) = ((prev / dd).round() as i64, (pred / dd).round() as i64);
    let h = ((p / d as f32) * 0.5).ceil() as i64;
    let span = ((p / d as f32) / 8.0).ceil() as i64;
    let mut best = pb;
    let mut best_c = f32::NEG_INFINITY;
    for o in -span..=span {
        let c = corr(dec, pa, pb + o, h);
        if c > best_c {
            best_c = c;
            best = pb + o;
        }
    }
    // Fine.
    let (fa, fb) = (prev.round() as i64, best * d as i64);
    let hf = (p * 0.5).ceil() as i64;
    let r = d as i64 + 1;
    let cs: Vec<f32> = (-r..=r).map(|o| corr(x, fa, fb + o, hf)).collect();
    let mut k = 0;
    for i in 1..cs.len() {
        if cs[i] > cs[k] {
            k = i;
        }
    }
    let neg: Vec<f32> = cs.iter().map(|v| -v).collect();
    let v = super::parabola(&neg, k, cs.len() - 1);
    let at = (fb - r) as f64 + v as f64 + (prev - prev.round());
    // Never backwards, never more than half a period off the prediction.
    at.clamp(prev + p as f64 * 0.5, pred + p as f64 * 0.5)
}
