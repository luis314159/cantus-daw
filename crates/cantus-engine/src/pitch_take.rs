//! A take's analysis for the pitch editor: the base's offline tracker (ADR 0053), then the
//! **notes** — voiced stretches cut where the pitch steps to another note or the level
//! dips for a new syllable — and the **key** the notes are in.

use cantus_core::pitch::curves::Frames;
use cantus_core::pitch::{PitchEdit, PitchKey, PitchNote};
use cantus_dsp::pitch::{analyse, hz_to_midi, Analysis};

/// The analyser's version: a better one gets a new number (the disk cache is keyed by it;
/// a stored edit keeps the notes it was made on, ADR 0054 §2).
pub const ANALYSER: u32 = 1;

/// The shortest note the analysis makes, seconds.
pub const MIN_NOTE_S: f64 = 0.060;
/// Two neighbouring stretches whose means differ by less than this (semitones) are one
/// note that drifts.
pub const STEP_ST: f32 = 0.5;
/// A dip in level this deep (dB) under the loudest point on both sides within
/// [`DIP_SPAN_S`] is a new syllable on the same pitch.
pub const DIP_DB: f32 = 9.0;
pub const DIP_SPAN_S: f64 = 0.060;
/// The smoothing that takes the vibrato out before the cut points are chosen, seconds.
pub const SMOOTH_S: f64 = 0.180;

/// What a frame of the take is, for the editor's drawing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FrameKind {
    Voiced,
    /// An "s", "sh", "f", "t": noise above the voice's band.
    Sibilant,
    /// Breath: quiet aperiodic noise.
    Breath,
    Silence,
}

/// Everything the editor and the render need from a take.
#[derive(Clone, Debug, PartialEq)]
pub struct TakeAnalysis {
    /// The rate the take was analysed at: its file's own.
    pub rate: u32,
    /// The take's length, frames.
    pub frames: u64,
    /// The tracker's frames and epochs (the render reads the epochs).
    pub analysis: Analysis,
    /// Per analysis frame, the pitch (MIDI) where voiced, NaN elsewhere.
    pub midi: Vec<f32>,
    /// The notes found, untouched.
    pub notes: Vec<PitchNote>,
    /// The key they are in.
    pub key: PitchKey,
}

impl TakeAnalysis {
    /// The pitch per frame, for the curves.
    pub fn frames_view(&self) -> Frames<'_> {
        Frames {
            hop: self.analysis.hop,
            midi: &self.midi,
        }
    }

    /// The edit an untouched take starts from: these notes, this key.
    pub fn edit(&self) -> PitchEdit {
        PitchEdit {
            rate: self.rate,
            frames: self.frames,
            analyser: ANALYSER,
            key: self.key,
            notes: self.notes.clone(),
            bypass: false,
        }
    }

    /// What frame `i` is.
    pub fn kind(&self, i: usize) -> FrameKind {
        let Some(f) = self.analysis.frames.get(i) else {
            return FrameKind::Silence;
        };
        if f.voiced {
            FrameKind::Voiced
        } else if f.level_db < cantus_dsp::pitch::GATE_DB {
            FrameKind::Silence
        } else if f.sibilance > 0.5 {
            FrameKind::Sibilant
        } else {
            FrameKind::Breath
        }
    }
}

/// The mono sum of a take's channels.
pub fn mixdown(channels: &[&[f32]]) -> Vec<f32> {
    match channels {
        [] => Vec::new(),
        [one] => one.to_vec(),
        many => {
            let len = many.iter().map(|c| c.len()).min().unwrap_or(0);
            let k = 1.0 / many.len() as f32;
            (0..len)
                .map(|i| many.iter().map(|c| c[i]).sum::<f32>() * k)
                .collect()
        }
    }
}

/// Analyse a take (its channels at `rate`): deterministic, a second or so per minute.
pub fn analyse_take(channels: &[&[f32]], rate: u32) -> TakeAnalysis {
    let mono = mixdown(channels);
    let analysis = analyse(&mono, rate as f32);
    from_analysis(analysis, mono.len() as u64, rate)
}

/// The notes and the key of an analysis already made (one read from the disk cache).
pub fn from_analysis(analysis: Analysis, frames: u64, rate: u32) -> TakeAnalysis {
    let midi: Vec<f32> = analysis
        .frames
        .iter()
        .map(|f| {
            if f.voiced && f.f0 > 0.0 {
                hz_to_midi(f.f0)
            } else {
                f32::NAN
            }
        })
        .collect();
    let notes = segment(&analysis, &midi, frames, rate);
    let key = detect_key(&notes);
    TakeAnalysis {
        rate,
        frames,
        analysis,
        midi,
        notes,
        key,
    }
}

fn frames_of(seconds: f64, hop: usize, rate: u32) -> usize {
    ((seconds * rate as f64 / hop.max(1) as f64).round() as usize).max(1)
}

/// The notes of a take (see the module docs).
fn segment(a: &Analysis, midi: &[f32], len: u64, rate: u32) -> Vec<PitchNote> {
    let hop = a.hop.max(1);
    let min = frames_of(MIN_NOTE_S, hop, rate);
    let view = Frames { hop, midi };
    // Voiced runs, bridging single-frame dropouts.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (s, e) in a.voiced_runs() {
        match runs.last_mut() {
            Some(last) if s <= last.1 + 2 => last.1 = e,
            _ => runs.push((s, e)),
        }
    }
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    for (s, e) in runs {
        if e - s < min {
            continue;
        }
        let mut pieces = vec![s];
        for c in dips(a, s, e, min, hop, rate) {
            pieces.push(c);
        }
        pieces.push(e);
        for w in pieces.windows(2) {
            for (x, y) in steps(midi, w[0], w[1], min, hop, rate) {
                cuts.push((x, y));
            }
        }
    }
    let half = hop as u64 / 2;
    let mut notes: Vec<PitchNote> = Vec::new();
    let mut prev_end_frame = usize::MAX;
    for (x, y) in cuts {
        // A note's edges: half a hop out from its first and last frames, unless it shares
        // an edge with the note before (a step inside a run: legato).
        let start = if x == prev_end_frame {
            notes.last().map(|n| n.src_end).unwrap_or(0)
        } else {
            (x as u64 * hop as u64).saturating_sub(half)
        };
        let end = if y < midi.len() && y > 0 {
            if is_inner(midi, y) {
                y as u64 * hop as u64
            } else {
                ((y - 1) as u64 * hop as u64 + half).min(len)
            }
        } else {
            len
        };
        let end = end.min(len);
        if end <= start {
            continue;
        }
        let sung = view.centre(start, end).unwrap_or(f32::NAN);
        if !sung.is_finite() {
            continue;
        }
        let level = mean_level(a, x, y);
        notes.push(PitchNote::sung(notes.len() as u32, start, end, sung, level));
        prev_end_frame = y;
    }
    notes
}

/// Whether frame `y` (the first after a cut) is voiced: the cut is inside a run.
fn is_inner(midi: &[f32], y: usize) -> bool {
    midi.get(y).is_some_and(|m| m.is_finite())
}

fn mean_level(a: &Analysis, x: usize, y: usize) -> f32 {
    let f = &a.frames[x..y.min(a.frames.len())];
    if f.is_empty() {
        return -120.0;
    }
    f.iter().map(|f| f.level_db).sum::<f32>() / f.len() as f32
}

/// Frames inside run `[s, e)` where the level dips for a new syllable.
fn dips(a: &Analysis, s: usize, e: usize, min: usize, hop: usize, rate: u32) -> Vec<usize> {
    let span = frames_of(DIP_SPAN_S, hop, rate);
    let lv = |i: usize| a.frames[i].level_db;
    let mut out: Vec<usize> = Vec::new();
    let mut i = s + min;
    while i + min <= e {
        let local_min = lv(i) <= lv(i - 1) && lv(i) <= lv(i + 1);
        if local_min {
            let left = (i.saturating_sub(span).max(s)..i).map(lv).fold(f32::MIN, f32::max);
            let right = (i + 1..(i + 1 + span).min(e)).map(lv).fold(f32::MIN, f32::max);
            if lv(i) < left - DIP_DB && lv(i) < right - DIP_DB {
                out.push(i);
                i += min;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Notes inside a stretch `[s, e)`: a least-squares cut into steps of the pitch smoothed
/// over a vibrato cycle, steps closer than [`STEP_ST`] joined, each cut moved to where
/// the pitch moves fastest.
fn steps(
    midi: &[f32],
    s: usize,
    e: usize,
    min: usize,
    hop: usize,
    rate: u32,
) -> Vec<(usize, usize)> {
    let n = e - s;
    if n < 2 * min {
        return vec![(s, e)];
    }
    let raw = fill(&midi[s..e]);
    let smooth = average(&raw, frames_of(SMOOTH_S, hop, rate));
    // Dynamic programming: segments of at least `min` frames, a penalty per cut.
    let (mut p1, mut p2) = (vec![0.0f64; n + 1], vec![0.0f64; n + 1]);
    for i in 0..n {
        let v = smooth[i] as f64;
        p1[i + 1] = p1[i] + v;
        p2[i + 1] = p2[i] + v * v;
    }
    let cost = |i: usize, j: usize| {
        let k = (j - i) as f64;
        let m = (p1[j] - p1[i]) / k;
        (p2[j] - p2[i]) - m * m * k
    };
    const PENALTY: f64 = 1.0;
    let mut best = vec![f64::INFINITY; n + 1];
    let mut from = vec![0usize; n + 1];
    best[0] = 0.0;
    for j in min..=n {
        for i in (0..=j - min).rev() {
            if i != 0 && i < min {
                continue;
            }
            if best[i].is_finite() {
                let c = best[i] + cost(i, j) + PENALTY;
                if c < best[j] {
                    best[j] = c;
                    from[j] = i;
                }
            }
        }
    }
    let mut bounds = vec![n];
    let mut j = n;
    while j > 0 {
        j = from[j];
        bounds.push(j);
    }
    bounds.reverse();
    // Join steps closer than STEP_ST, the closest pair first.
    let mean = |a: usize, b: usize| raw[a..b].iter().sum::<f32>() / (b - a) as f32;
    loop {
        let mut pick: Option<(usize, f32)> = None;
        for k in 1..bounds.len() - 1 {
            let d = (mean(bounds[k - 1], bounds[k]) - mean(bounds[k], bounds[k + 1])).abs();
            if d < STEP_ST && pick.is_none_or(|(_, b)| d < b) {
                pick = Some((k, d));
            }
        }
        match pick {
            Some((k, _)) => {
                bounds.remove(k);
            }
            None => break,
        }
    }
    // Each cut to the steepest point of the raw pitch near it.
    let reach = frames_of(SMOOTH_S / 2.0, hop, rate);
    for k in 1..bounds.len() - 1 {
        let (lo, hi) = (
            bounds[k].saturating_sub(reach).max(bounds[k - 1] + min / 2 + 1),
            (bounds[k] + reach).min(bounds[k + 1].saturating_sub(min / 2 + 1)),
        );
        let mut at = bounds[k];
        let mut steep = -1.0f32;
        for c in lo.max(1)..hi.min(n - 1) {
            let d = (raw[(c + 1).min(n - 1)] - raw[c - 1]).abs();
            if d > steep {
                steep = d;
                at = c;
            }
        }
        bounds[k] = at;
    }
    bounds.windows(2).map(|w| (s + w[0], s + w[1])).collect()
}

/// Unvoiced frames inside a run filled from their neighbours (the stretch is voiced but
/// for a frame or two the bridge let through).
fn fill(x: &[f32]) -> Vec<f32> {
    let mut out = x.to_vec();
    let mut last = x.iter().copied().find(|v| v.is_finite()).unwrap_or(60.0);
    for v in &mut out {
        if v.is_finite() {
            last = *v;
        } else {
            *v = last;
        }
    }
    out
}

/// A centred moving average of `len` frames (the window shrinks at the ends).
fn average(x: &[f32], len: usize) -> Vec<f32> {
    let half = len / 2;
    let mut pre = vec![0.0f64; x.len() + 1];
    for (i, v) in x.iter().enumerate() {
        pre[i + 1] = pre[i] + *v as f64;
    }
    (0..x.len())
        .map(|i| {
            let lo = i.saturating_sub(half);
            let hi = (i + len - half).min(x.len());
            ((pre[hi] - pre[lo]) / (hi - lo) as f64) as f32
        })
        .collect()
}

/// Krumhansl and Kessler's key profiles (1982): how well each pitch class fits a major
/// and a minor key on C.
const MAJOR: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

/// The key of the notes: the major or minor key whose profile correlates best with how
/// long each pitch class is sung (Krumhansl–Schmuckler). Fewer than three notes, or no
/// fit at all: chromatic, on the most sung class.
pub fn detect_key(notes: &[PitchNote]) -> PitchKey {
    let mut h = [0.0f32; 12];
    for n in notes {
        let pc = (n.sung.round() as i32).rem_euclid(12) as usize;
        h[pc] += (n.src_end - n.src_start) as f32;
    }
    let top = (0..12)
        .max_by(|a, b| h[*a].total_cmp(&h[*b]).then(b.cmp(a)))
        .unwrap_or(0) as u8;
    if notes.len() < 3 {
        return PitchKey {
            tonic: top,
            scale: 0,
        };
    }
    let corr = |profile: &[f32; 12], tonic: usize| -> f32 {
        let p = |i: usize| profile[(i + 12 - tonic) % 12];
        let mh = h.iter().sum::<f32>() / 12.0;
        let mp = profile.iter().sum::<f32>() / 12.0;
        let (mut sxy, mut sxx, mut syy) = (0.0f32, 0.0f32, 0.0f32);
        for (i, hv) in h.iter().enumerate() {
            let (x, y) = (hv - mh, p(i) - mp);
            sxy += x * y;
            sxx += x * x;
            syy += y * y;
        }
        sxy / (sxx * syy).sqrt().max(1e-12)
    };
    let mut best = (f32::MIN, PitchKey::default());
    for tonic in 0..12 {
        for (scale, profile) in [(1u8, &MAJOR), (2u8, &MINOR)] {
            let c = corr(profile, tonic);
            if c > best.0 {
                best = (
                    c,
                    PitchKey {
                        tonic: tonic as u8,
                        scale,
                    },
                );
            }
        }
    }
    if best.0 < 0.3 {
        PitchKey {
            tonic: top,
            scale: 0,
        }
    } else {
        best.1
    }
}
