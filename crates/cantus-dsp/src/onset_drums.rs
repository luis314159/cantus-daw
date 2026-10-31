//! **Drums to notes** (round 34, ADR 0086): where a kick, a snare, a tom, a hi-hat or a
//! cymbal is struck in a recording, and how hard — Cantus's own signal processing, no
//! trained model.
//!
//! The method is the one the drum-transcription literature calls *semi-adaptive
//! non-negative matrix factorisation* (Dittmar and Gärtner, DAFx 2014; Wu et al., IEEE/ACM
//! TASLP 2018, §IV): the magnitude spectrogram on a logarithmic filter bank is written as
//! spectral templates times their activations over time. Six templates start as the
//! instruments' typical spectra (kick, snare, a low and a high tom, hi-hat, cymbal — drawn
//! from where each puts its energy, `templates`) and are let adapt to the recording as the
//! iterations go, so a deep kick or a bright snare is still found; three free templates
//! take what is not a drum (a bass line, a pad) so it does not light a drum's activation.
//! Each drum's activation is then peak-picked against its own moving mean, and each hit
//! named: an open or closed hat by how long it rings, a crash or a ride likewise, a tom as
//! low, mid or high by where its body sits.
//!
//! Offline, deterministic (seeded, `crate::math`), allocates.

// Index loops over spectra and frames read clearer than iterator chains here.
#![allow(clippy::needless_range_loop)]

use crate::math;

/// The five classes (MDB Drums' and most of the literature's).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Class {
    Kick,
    Snare,
    Toms,
    HiHat,
    Cymbals,
}

impl Class {
    pub const ALL: [Class; 5] = [
        Class::Kick,
        Class::Snare,
        Class::Toms,
        Class::HiHat,
        Class::Cymbals,
    ];
}

/// What a hit is, finer than its class (its General MIDI key follows from it).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Kick,
    Snare,
    LowTom,
    MidTom,
    HighTom,
    ClosedHat,
    OpenHat,
    Crash,
    Ride,
}

/// A hit.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// Seconds from the start.
    pub time: f64,
    pub class: Class,
    pub kind: Kind,
    /// 1–127, from the activation's peak against the class's loudest.
    pub velocity: u8,
    /// The activation's peak (relative units).
    pub strength: f32,
}

/// Activation frames a second.
pub const FPS: f64 = 100.0;
/// Filter-bank bands per octave, and its range, Hz.
const BANDS_PER_OCTAVE: f64 = 8.0;
const F_LO: f64 = 40.0;
const F_HI: f64 = 16_000.0;
/// Factorisation iterations.
const ITERATIONS: usize = 40;
/// Free templates for what is not a drum.
const FREE: usize = 3;

/// The drum templates, in this order.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Comp {
    Kick,
    Snare,
    LowTom,
    HighTom,
    Hat,
    Cymbal,
    /// A machine's or a bright snare: its body higher and its noise brighter and louder.
    BrightSnare,
}

const COMPS: [Comp; 7] = [
    Comp::Kick,
    Comp::Snare,
    Comp::LowTom,
    Comp::HighTom,
    Comp::Hat,
    Comp::Cymbal,
    Comp::BrightSnare,
];

/// The analysis: the spectrogram's filter bank and its frames.
pub struct Spectrogram {
    /// Band centres, Hz.
    pub centres: Vec<f64>,
    /// `frames × bands`, magnitudes.
    pub mag: Vec<f32>,
    pub frames: usize,
    /// Samples between frames.
    pub hop: usize,
    pub rate: f32,
}

impl Spectrogram {
    pub fn bands(&self) -> usize {
        self.centres.len()
    }
}

/// The spectrogram on a logarithmic bank of triangular filters: a Hann window of about
/// 46 ms, a frame each 10 ms, frame `t` centred on sample `t · hop`.
pub fn spectrogram(x: &[f32], rate: f32) -> Spectrogram {
    let sr = rate as f64;
    let n = ((0.046 * sr) as usize)
        .next_power_of_two()
        .clamp(256, 8_192);
    let hop = (sr / FPS).round().max(1.0) as usize;
    let frames = x.len().div_ceil(hop).max(1);
    let top = F_HI.min(0.45 * sr);
    let count = (BANDS_PER_OCTAVE * math::log2(top / F_LO)).floor().max(1.0) as usize;
    let centres: Vec<f64> = (0..count)
        .map(|b| F_LO * math::exp2(b as f64 / BANDS_PER_OCTAVE))
        .collect();
    // Each band a triangle from the band below's centre to the band above's, at least
    // two FFT bins wide (the low bands are narrower than a bin).
    let bin = sr / n as f64;
    let half = n / 2 + 1;
    let mut weights: Vec<Vec<(usize, f32)>> = Vec::with_capacity(count);
    for b in 0..count {
        let c = centres[b];
        let lo = c * math::exp2(-1.0 / BANDS_PER_OCTAVE);
        let hi = c * math::exp2(1.0 / BANDS_PER_OCTAVE);
        let (lo, hi) = (lo.min(c - bin), hi.max(c + bin));
        let mut w = Vec::new();
        let mut sum = 0.0f32;
        for k in ((lo / bin).floor().max(0.0) as usize)..=((hi / bin).ceil() as usize).min(half - 1)
        {
            let f = k as f64 * bin;
            let v = if f <= c {
                (f - lo) / (c - lo)
            } else {
                (hi - f) / (hi - c)
            };
            if v > 0.0 {
                w.push((k, v as f32));
                sum += v as f32;
            }
        }
        for (_, v) in &mut w {
            *v /= sum.max(1e-12);
        }
        weights.push(w);
    }
    let fft = crate::aequalis::fft::Fft::new(n);
    let win: Vec<f32> = (0..n)
        .map(|i| (0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64)) as f32)
        .collect();
    let (mut re, mut im) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut mag = vec![0.0f32; frames * count];
    let mut spec = vec![0.0f32; half];
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
        for k in 0..half {
            spec[k] = math::hypot(re[k], im[k]);
        }
        for (b, w) in weights.iter().enumerate() {
            // Square-root compressed: the hats and cymbals, 20–40 dB under a kick's
            // body, still weigh in the factorisation.
            let m: f32 = w.iter().map(|(k, v)| spec[*k] * v).sum();
            mag[t * count + b] = m.sqrt();
        }
    }
    Spectrogram {
        centres,
        mag,
        frames,
        hop,
        rate,
    }
}

/// A spectrum drawn as straight lines in decibels over log-frequency between `knots`
/// (Hz, dB), flat beyond the ends.
fn drawn(f: f64, knots: &[(f64, f64)]) -> f64 {
    let db = if f <= knots[0].0 {
        knots[0].1
    } else if f >= knots[knots.len() - 1].0 {
        knots[knots.len() - 1].1
    } else {
        let i = knots
            .iter()
            .position(|k| k.0 > f)
            .unwrap_or(knots.len() - 1);
        let (a, b) = (knots[i - 1], knots[i]);
        let x = math::log2(f / a.0) / math::log2(b.0 / a.0);
        a.1 + x * (b.1 - a.1)
    };
    math::powf(10.0, db / 20.0)
}

/// The drums' starting spectra (magnitude, relative to each one's peak), drawn after the
/// average spectra of isolated hits measured on real kits (ADR 0086 §3): a kick flat to
/// 100 Hz and falling 25 dB by 300 Hz, its beater's click near 2 kHz; a snare's body at
/// 190 Hz over its wires, 20–40 dB down from 800 Hz to 10 kHz; a tom's body (the low one
/// near 100 Hz, the high one near 200 Hz) falling fast above it; a hi-hat flat from 4 to
/// 16 kHz with nothing low; a cymbal broad from 1 kHz up, its top from 3 to 7 kHz.
fn template(c: Comp, f: f64) -> f64 {
    let knots: &[(f64, f64)] = match c {
        Comp::Kick => &[
            (40.0, 0.0),
            (100.0, -3.0),
            (135.0, -7.0),
            (190.0, -12.0),
            (300.0, -25.0),
            (1_000.0, -35.0),
            (2_000.0, -33.0),
            (4_000.0, -45.0),
            (8_000.0, -54.0),
            (16_000.0, -62.0),
        ],
        Comp::Snare => &[
            (40.0, -25.0),
            (113.0, -23.0),
            (160.0, -8.0),
            (190.0, 0.0),
            (226.0, -6.0),
            (300.0, -13.0),
            (500.0, -15.0),
            (800.0, -20.0),
            (1_500.0, -25.0),
            (3_000.0, -30.0),
            (6_000.0, -36.0),
            (10_000.0, -43.0),
            (16_000.0, -52.0),
        ],
        Comp::LowTom => &[
            (40.0, -30.0),
            (60.0, -18.0),
            (85.0, -3.0),
            (100.0, 0.0),
            (130.0, -2.0),
            (200.0, -10.0),
            (400.0, -20.0),
            (1_000.0, -18.0),
            (3_000.0, -27.0),
            (6_000.0, -36.0),
            (16_000.0, -60.0),
        ],
        Comp::HighTom => &[
            (40.0, -35.0),
            (100.0, -18.0),
            (150.0, -3.0),
            (190.0, 0.0),
            (250.0, -3.0),
            (350.0, -10.0),
            (600.0, -20.0),
            (1_200.0, -17.0),
            (3_000.0, -26.0),
            (6_000.0, -35.0),
            (16_000.0, -58.0),
        ],
        Comp::Hat => &[
            (40.0, -45.0),
            (500.0, -35.0),
            (1_000.0, -24.0),
            (2_000.0, -10.0),
            (4_000.0, -3.0),
            (8_000.0, 0.0),
            (14_000.0, 0.0),
            (16_000.0, -2.0),
        ],
        Comp::BrightSnare => &[
            (40.0, -35.0),
            (120.0, -20.0),
            (180.0, -3.0),
            (220.0, 0.0),
            (300.0, -8.0),
            (600.0, -14.0),
            (1_500.0, -10.0),
            (3_000.0, -6.0),
            (6_000.0, -6.0),
            (10_000.0, -10.0),
            (16_000.0, -18.0),
        ],
        Comp::Cymbal => &[
            (40.0, -32.0),
            (200.0, -22.0),
            (400.0, -14.0),
            (1_000.0, -8.0),
            (2_000.0, -4.0),
            (3_500.0, 0.0),
            (7_000.0, 0.0),
            (10_000.0, -5.0),
            (14_000.0, -10.0),
        ],
    };
    // In the factorisation's compressed domain (the square root of the magnitude).
    drawn(f, knots).sqrt()
}

/// The factorisation: `V ≈ W · H`, `W` (bands × components) the templates, columns
/// summing to one; `H` (components × frames) their activations.
pub struct Factors {
    pub w: Vec<f32>,
    pub h: Vec<f32>,
    pub comps: usize,
}

/// A small deterministic generator for the free templates' start.
fn lcg(state: &mut u64) -> f32 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*state >> 40) as f32) / (1u64 << 24) as f32
}

/// Semi-adaptive NMF under the generalised Kullback–Leibler divergence (multiplicative
/// updates, Lee and Seung): the drum templates are pulled back toward their starting
/// spectra, less with each iteration (`α` from 1 to 0.5), the free ones adapt freely.
pub fn factorise(s: &Spectrogram) -> Factors {
    factorise_with(s, SPARSITY)
}

/// How strongly a frame's energy is pushed onto few templates (an L1 penalty on the
/// activations, in units of the spectrogram's mean): without it a broadband hit is shared
/// among every template it half fits.
pub const SPARSITY: f32 = 0.0;

/// Half the window, seconds, the free templates' activations are averaged over (0: off).
pub const FREE_SMOOTH: f64 = 0.0;

/// [`factorise`] with another sparsity (the measurement's).
pub fn factorise_with(s: &Spectrogram, sparsity: f32) -> Factors {
    factorise_until(s, sparsity, &|| false).expect("never cancelled")
}

/// [`factorise_with`], asking `cancelled` before each iteration: `None` once it says so.
pub fn factorise_until(
    s: &Spectrogram,
    sparsity: f32,
    cancelled: &dyn Fn() -> bool,
) -> Option<Factors> {
    let (bands, frames) = (s.bands(), s.frames);
    let drums = COMPS.len();
    let comps = drums + FREE;
    let eps = 1e-9f32;
    let mut w0 = vec![0.0f32; bands * comps];
    for (k, c) in COMPS.iter().enumerate() {
        let col: Vec<f64> = s.centres.iter().map(|f| template(*c, *f) + 1e-4).collect();
        let sum: f64 = col.iter().sum();
        for b in 0..bands {
            w0[b * comps + k] = (col[b] / sum) as f32;
        }
    }
    let mut seed = 0x005e_edd2_u64;
    for k in drums..comps {
        let col: Vec<f32> = (0..bands).map(|_| 0.5 + lcg(&mut seed)).collect();
        let sum: f32 = col.iter().sum();
        for b in 0..bands {
            w0[b * comps + k] = col[b] / sum;
        }
    }
    let mut w = w0.clone();
    // Activations start at each frame's level.
    let mut h = vec![0.0f32; comps * frames];
    for t in 0..frames {
        let level: f32 = (0..bands).map(|b| s.mag[t * bands + b]).sum::<f32>() / comps as f32;
        for k in 0..comps {
            h[k * frames + t] = level + eps;
        }
    }
    let v = &s.mag;
    let lambda = sparsity * v.iter().sum::<f32>() / v.len().max(1) as f32;
    let mut wh = vec![0.0f32; frames * bands];
    let mut ratio = vec![0.0f32; frames * bands];
    let product = |w: &[f32], h: &[f32], wh: &mut [f32]| {
        for t in 0..frames {
            for b in 0..bands {
                let mut acc = 0.0f32;
                for k in 0..comps {
                    acc += w[b * comps + k] * h[k * frames + t];
                }
                wh[t * bands + b] = acc + eps;
            }
        }
    };
    for it in 0..ITERATIONS {
        if cancelled() {
            return None;
        }
        // H ← H ⊙ Wᵀ(V / WH) / Wᵀ1 (each template's column sums to one).
        product(&w, &h, &mut wh);
        for i in 0..frames * bands {
            ratio[i] = v[i] / wh[i];
        }
        for k in 0..comps {
            let colsum: f32 = (0..bands).map(|b| w[b * comps + k]).sum::<f32>()
                + eps
                + if k < drums { lambda } else { 0.0 };
            for t in 0..frames {
                let mut acc = 0.0f32;
                for b in 0..bands {
                    acc += w[b * comps + k] * ratio[t * bands + b];
                }
                h[k * frames + t] *= acc / colsum;
            }
        }
        // The free templates take what sustains (a bass line, a pad), never a hit: their
        // activations are held smooth over 150 ms, so an attack is left to the drums.
        for k in drums..comps {
            let row = &mut h[k * frames..(k + 1) * frames];
            smooth(row, (FREE_SMOOTH * FPS) as usize);
        }
        // W ← W ⊙ (V / WH) Hᵀ / 1Hᵀ, then the drums pulled toward their start.
        product(&w, &h, &mut wh);
        for i in 0..frames * bands {
            ratio[i] = v[i] / wh[i];
        }
        let alpha = 1.0 - 0.5 * (it as f32 / (ITERATIONS - 1).max(1) as f32);
        for k in 0..comps {
            let rowsum: f32 = (0..frames).map(|t| h[k * frames + t]).sum::<f32>() + eps;
            let mut col = vec![0.0f32; bands];
            for b in 0..bands {
                let mut acc = 0.0f32;
                for t in 0..frames {
                    acc += ratio[t * bands + b] * h[k * frames + t];
                }
                col[b] = w[b * comps + k] * acc / rowsum;
            }
            let sum: f32 = col.iter().sum::<f32>() + eps;
            for b in 0..bands {
                let adapted = col[b] / sum;
                w[b * comps + k] = if k < drums {
                    alpha * w0[b * comps + k] + (1.0 - alpha) * adapted
                } else {
                    adapted
                };
            }
        }
    }
    Some(Factors { w, h, comps })
}

/// A centred moving average over `2 · half + 1` frames, in place.
fn smooth(row: &mut [f32], half: usize) {
    let n = row.len();
    if n == 0 || half == 0 {
        return;
    }
    let mut prefix = vec![0.0f64; n + 1];
    for (i, v) in row.iter().enumerate() {
        prefix[i + 1] = prefix[i] + *v as f64;
    }
    for (i, v) in row.iter_mut().enumerate() {
        let (lo, hi) = (i.saturating_sub(half), (i + half + 1).min(n));
        *v = ((prefix[hi] - prefix[lo]) / (hi - lo) as f64) as f32;
    }
}

/// Peak picking, seconds: a hit is the largest activation within ±30 ms, above its mean
/// over 100 ms before and 70 ms after by `delta` of the class's top, 40 ms after the
/// class's last hit.
fn pick(h: &[f32], delta: f32) -> Vec<usize> {
    let f = |s: f64| (s * FPS).round() as usize;
    let (wmax, pre, post, gap) = (f(0.03), f(0.10), f(0.07), f(0.04));
    let top = h.iter().fold(0.0f32, |m, v| m.max(*v));
    if top <= 0.0 {
        return Vec::new();
    }
    let d = delta * top;
    let mut out: Vec<usize> = Vec::new();
    for t in 0..h.len() {
        let v = h[t];
        if v <= d {
            continue;
        }
        let lo = t.saturating_sub(wmax);
        let hi = (t + wmax + 1).min(h.len());
        if h[lo..hi].iter().any(|x| *x > v) || h[lo..t].contains(&v) {
            continue;
        }
        let lo = t.saturating_sub(pre);
        let hi = (t + post + 1).min(h.len());
        let mean = h[lo..hi].iter().sum::<f32>() / (hi - lo) as f32;
        if v < mean + d {
            continue;
        }
        if out.last().is_some_and(|l| t - l < gap) {
            continue;
        }
        out.push(t);
    }
    out
}

/// How long an activation rings after its peak at `t`: seconds until it falls under a
/// quarter of the peak (or the next hit, or the end).
fn ring(h: &[f32], t: usize, next: Option<usize>) -> f64 {
    let end = next.unwrap_or(h.len()).min(h.len());
    // What the hit added: above the level just before it (a crash may still be ringing).
    let base = h[t.saturating_sub(4)..=t]
        .iter()
        .fold(f32::MAX, |m, v| m.min(*v));
    let floor = base + 0.25 * (h[t] - base);
    let mut i = t;
    while i + 1 < end && h[i + 1] > floor {
        i += 1;
    }
    (i - t) as f64 / FPS
}

/// The decisions' numbers. [`Tuning::default`] is what Cantus uses; the measurement
/// (`tests/onset_drums_mdb.rs`) varies them on the tuning tracks only.
#[derive(Clone, Debug, PartialEq)]
pub struct Tuning {
    /// A low hit is a tom when the toms' templates hold more than this share of it and its
    /// body sits at or above `tom_body_hz` (a kick's is lower).
    pub tom_share: f32,
    pub tom_body_hz: f64,
    /// A metal hit is a cymbal when the cymbal's template rises this many times more than
    /// the hat's, or it rings longer than `cymbal_ring` seconds (hats ring 30 ms, cymbals
    /// 300 ms, median, on real kits).
    pub cymbal_rise: f32,
    pub cymbal_ring: f64,
    /// A hit of a class that another class's hit at the same moment leaks into is kept
    /// only when it rose at least this share of that hit's rise: `leak[a][b]` for a hit of
    /// class `a` beside one of class `b` (the snare's wires light the cymbal, its body the
    /// toms; a kick's beater the snare).
    pub leak: [[f32; 5]; 5],
    /// The rarer classes ask more of a hit: their threshold over the moving mean is the
    /// common one times this (toms, cymbals).
    pub rare: f32,
    /// When above 0, a metal hit is a hi-hat when what it adds above 7 kHz is at least
    /// this many times what it adds from 2 to 5 kHz (a hat is brighter than a ride or a
    /// crash), whatever the templates say.
    pub hat_bright: f32,
    /// Pick the kick and the toms on their own templates (else on their sum, then named).
    pub split_low: bool,
    /// Pick the hats and the cymbals on their own templates (else on their sum).
    pub split_metal: bool,
}

impl Default for Tuning {
    fn default() -> Self {
        // Found by a coordinate search over MDB Drums' first eight tracks (the other
        // fifteen held out) and Pulsus's seven factory kits playing a known pattern, both
        // weighed equally (`tests/onset_drums_mdb.rs`, ADR 0086 §3). Rows: the hit judged
        // (kick, snare, toms, hi-hat, cymbals); columns: the hit beside it.
        Tuning {
            tom_share: 0.45,
            tom_body_hz: 125.0,
            cymbal_rise: 1.5,
            cymbal_ring: 0.08,
            leak: [
                [0.0, 3.0, 0.0, 0.0, 0.0],
                [0.45, 0.0, 0.45, 1.0, 0.45],
                [0.0, 3.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0, 0.0],
                [0.0, 0.1, 0.0, 0.6, 0.0],
            ],
            rare: 3.0,
            hat_bright: 0.0,
            split_low: false,
            split_metal: true,
        }
    }
}

/// No hit is quieter than this share of the loudest group's top (a file with no hats
/// still has a hat activation, all noise: its own top is no measure).
const FLOOR: f32 = 0.02;

/// The threshold over the moving mean, as a share of a class's top, for a sensitivity.
pub fn delta(sensitivity: f32) -> f32 {
    // 0.5 (the default) gives 0.0624, the measured best; 1 finds hits a fiftieth of the
    // loudest over their surroundings, 0 only those a tenth of it.
    0.0208 + 0.0832 * (1.0 - sensitivity.clamp(0.0, 1.0))
}

/// The hits of a mono signal at `rate`, in time order.
pub fn transcribe(x: &[f32], rate: f32, sensitivity: f32) -> Vec<Hit> {
    transcribe_with(x, rate, sensitivity, &Tuning::default())
}

/// [`transcribe`] with other numbers (the measurement's).
pub fn transcribe_with(x: &[f32], rate: f32, sensitivity: f32, tuning: &Tuning) -> Vec<Hit> {
    transcribe_until(x, rate, sensitivity, tuning, &|| false).unwrap_or_default()
}

/// [`transcribe_with`] that stops when `cancelled` says so (it is asked before each of the
/// factorisation's iterations): `None` then.
pub fn transcribe_until(
    x: &[f32],
    rate: f32,
    sensitivity: f32,
    tuning: &Tuning,
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<Hit>> {
    if x.is_empty() || rate <= 0.0 {
        return Some(Vec::new());
    }
    let s = spectrogram(x, rate);
    let fac = factorise_until(&s, SPARSITY, cancelled)?;
    Some(hits(&s, &fac, sensitivity, tuning))
}

/// What an activation rose by into frame `t`: above its lowest over the 40 ms before.
fn rise(h: &[f32], t: usize) -> f32 {
    h[t] - h[t.saturating_sub(4)..=t]
        .iter()
        .fold(f32::MAX, |m, v| m.min(*v))
}

/// A candidate hit: its class, its peak frame, its group's activation index, its kind.
struct Candidate {
    class: Class,
    t: usize,
    group: usize,
    kind: Kind,
}

/// The activations' hits, named.
///
/// Templates that share a register share a hit's energy (a kick's sweep passes through the
/// low tom's band; a hat's noise lights the cymbal's): so hits are picked per *group* —
/// the low drums (kick and toms) on their summed activations, the snare on its own, the
/// metal (hats and cymbals) on theirs — and each group's hit then named by what it is: a
/// tom when the toms' templates hold most of it and its body rings above the kick's
/// register; a cymbal when the cymbal's template holds most of it or it rings long.
/// Last, a hit that is only another's leak (it rose little beside a big rise of a class
/// that lights its template) is dropped.
pub fn hits(s: &Spectrogram, fac: &Factors, sensitivity: f32, tune: &Tuning) -> Vec<Hit> {
    let frames = s.frames;
    let d = delta(sensitivity);
    let act = |k: usize| &fac.h[k * frames..(k + 1) * frames];
    let sum = |ks: &[usize]| -> Vec<f32> {
        (0..frames)
            .map(|t| ks.iter().map(|k| act(*k)[t]).sum())
            .collect()
    };
    // The groups' activations: low, snare, metal; then each template alone (kick, toms,
    // hat, cymbal) for the split picking.
    let groups = [
        sum(&[0, 2, 3]),
        sum(&[1, 6]),
        sum(&[4, 5]),
        act(0).to_vec(),
        sum(&[2, 3]),
        act(4).to_vec(),
        act(5).to_vec(),
    ];
    let mut cands: Vec<Candidate> = Vec::new();
    for t in pick(&groups[1], d) {
        cands.push(Candidate {
            class: Class::Snare,
            t,
            group: 1,
            kind: Kind::Snare,
        });
    }
    let tom_kind = |t: usize| {
        let (lo, hi) = (act(2)[t], act(3)[t]);
        let body = body_hz(s, (t + 5).min(frames - 1));
        if body < 120.0 || lo > 2.0 * hi {
            Kind::LowTom
        } else if body > 175.0 || hi > 2.0 * lo {
            Kind::HighTom
        } else {
            Kind::MidTom
        }
    };
    if tune.split_low {
        for t in pick(&groups[3], d) {
            cands.push(Candidate {
                class: Class::Kick,
                t,
                group: 3,
                kind: Kind::Kick,
            });
        }
        for t in pick(&groups[4], d) {
            if body_hz(s, (t + 5).min(frames - 1)) >= tune.tom_body_hz {
                cands.push(Candidate {
                    class: Class::Toms,
                    t,
                    group: 4,
                    kind: tom_kind(t),
                });
            }
        }
    } else {
        for t in pick(&groups[0], d) {
            let (k, lo, hi) = (act(0)[t], act(2)[t], act(3)[t]);
            let tom_share = (lo + hi) / (k + lo + hi).max(1e-12);
            let body = body_hz(s, (t + 5).min(frames - 1));
            let (class, kind) = if tom_share > tune.tom_share && body >= tune.tom_body_hz {
                (Class::Toms, tom_kind(t))
            } else {
                (Class::Kick, Kind::Kick)
            };
            cands.push(Candidate {
                class,
                t,
                group: 0,
                kind,
            });
        }
    }
    let hat_kind = |h: &[f32], t: usize, next: Option<usize>| {
        if ring(h, t, next) > 0.12 {
            Kind::OpenHat
        } else {
            Kind::ClosedHat
        }
    };
    let cymbal_kind = |h: &[f32], t: usize, next: Option<usize>| {
        if ring(h, t, next) > 0.6 {
            Kind::Crash
        } else {
            Kind::Ride
        }
    };
    if tune.split_metal {
        for (g, class) in [(5usize, Class::HiHat), (6, Class::Cymbals)] {
            let peaks = pick(&groups[g], d);
            for (i, &t) in peaks.iter().enumerate() {
                let next = peaks.get(i + 1).copied();
                let kind = if class == Class::HiHat {
                    hat_kind(&groups[g], t, next)
                } else {
                    cymbal_kind(&groups[g], t, next)
                };
                cands.push(Candidate {
                    class,
                    t,
                    group: g,
                    kind,
                });
            }
        }
    } else {
        let metal_peaks = pick(&groups[2], d);
        for (i, &t) in metal_peaks.iter().enumerate() {
            let next = metal_peaks.get(i + 1).copied();
            let (hat, cym) = (rise(act(4), t), rise(act(5), t));
            let rings = ring(&groups[2], t, next);
            let cymbal = if tune.hat_bright > 0.0 {
                brightness(s, t) < tune.hat_bright
            } else {
                cym > tune.cymbal_rise * hat || rings > tune.cymbal_ring
            };
            cands.push(Candidate {
                class: if cymbal { Class::Cymbals } else { Class::HiHat },
                t,
                group: 2,
                kind: if cymbal {
                    cymbal_kind(&groups[2], t, next)
                } else {
                    hat_kind(&groups[2], t, next)
                },
            });
        }
    }
    // The rare classes' bar: their group's moving mean plus a larger share of its top.
    let tops: Vec<f32> = groups
        .iter()
        .map(|g| g.iter().fold(0.0f32, |m, v| m.max(*v)))
        .collect();
    let rises: Vec<f32> = cands.iter().map(|c| rise(&groups[c.group], c.t)).collect();
    let loudest = tops.iter().fold(0.0f32, |m, v| m.max(*v));
    // A hit makes the sound louder; an end, a cut or a fade-out also changes the spectrum
    // (its splatter lights templates) but never adds level.
    // Power (not the compressed magnitudes: splatter lights many bands a little), in the
    // register of the hit's group: a hat over a ringing kick adds little power overall.
    let level = |t: usize, (lo, hi): (f64, f64)| -> f32 {
        let b = s.bands();
        let t = t.min(frames - 1);
        (0..b)
            .filter(|k| (lo..hi).contains(&s.centres[*k]))
            .map(|k| {
                let m = s.mag[t * b + k];
                let p = m * m;
                p * p
            })
            .sum()
    };
    let louder = |t: usize, group: usize| {
        let reg = match group {
            0 | 3 | 4 => (0.0, 400.0),
            1 => (120.0, 8_000.0),
            _ => (3_000.0, f64::MAX),
        };
        let after = level(t, reg).max(level(t + 1, reg));
        let before = level(t.saturating_sub(4), reg).min(level(t.saturating_sub(3), reg));
        after > 1.25 * before
    };
    let keep: Vec<bool> = cands
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if !louder(c.t, c.group) || groups[c.group][c.t] < FLOOR * loudest {
                return false;
            }
            if matches!(c.class, Class::Toms | Class::Cymbals)
                && rises[i] < tune.rare * d * tops[c.group]
            {
                return false;
            }
            !cands.iter().enumerate().any(|(j, o)| {
                j != i
                    && o.t.abs_diff(c.t) <= 3
                    && rises[i] < tune.leak[c.class as usize][o.class as usize] * rises[j]
            })
        })
        .collect();
    let picked: Vec<(Class, f64, f32, Kind)> = cands
        .iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(c, _)| {
            let g = &groups[c.group];
            (c.class, attack(s, g, c.t), g[c.t], c.kind)
        })
        .collect();
    let mut tops = [0.0f32; 5];
    for (c, _, v, _) in &picked {
        tops[*c as usize] = tops[*c as usize].max(*v);
    }
    let mut out: Vec<Hit> = picked
        .into_iter()
        .map(|(class, time, v, kind)| {
            let top = tops[class as usize].max(1e-12);
            // The activations are in square-rooted magnitude: 40·log₁₀ is decibels. A hit
            // 48 dB under the class's loudest is velocity 1, as the pitch editor's notes.
            let db = 40.0 * math::log10((v / top).max(1e-6));
            Hit {
                time,
                class,
                kind,
                velocity: (127.0 + db * 126.0 / 48.0).round().clamp(1.0, 127.0) as u8,
                strength: v,
            }
        })
        .collect();
    out.sort_by(|a, b| a.time.total_cmp(&b.time).then(a.class.cmp(&b.class)));
    out
}
/// What a hit adds above 7 kHz against what it adds from 2 to 5 kHz (frames `t − 3` to
/// `t + 1`).
fn brightness(s: &Spectrogram, t: usize) -> f32 {
    let bands = s.bands();
    let after = (t + 1).min(s.frames - 1);
    let before = t.saturating_sub(3);
    let (mut hi, mut mid) = (0.0f32, 0.0f32);
    for b in 0..bands {
        let d = (s.mag[after * bands + b] - s.mag[before * bands + b]).max(0.0);
        let f = s.centres[b];
        if f >= 7_000.0 {
            hi += d;
        } else if (2_000.0..5_000.0).contains(&f) {
            mid += d;
        }
    }
    hi / mid.max(1e-9)
}

/// A tom struck at `time` seconds, named by where its body sits (low under 120 Hz, high
/// over 175 Hz), for a detector that knows only that it is a tom.
pub fn tom_kind_at(s: &Spectrogram, time: f64) -> Kind {
    let t = ((time * s.rate as f64 / s.hop as f64).round() as usize + 5)
        .min(s.frames.saturating_sub(1));
    let body = body_hz(s, t);
    if body < 120.0 {
        Kind::LowTom
    } else if body > 175.0 {
        Kind::HighTom
    } else {
        Kind::MidTom
    }
}

/// Where a hit's body sits: the centroid of the bands from 50 to 400 Hz at frame `t`.
fn body_hz(s: &Spectrogram, t: usize) -> f64 {
    let bands = s.bands();
    let (mut num, mut den) = (0.0f64, 0.0f64);
    for b in 0..bands {
        let f = s.centres[b];
        if (50.0..=400.0).contains(&f) {
            let m = s.mag[t * bands + b] as f64;
            num += m * f;
            den += m;
        }
    }
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
}

/// When the hit picked at frame `t` of activation `h` struck, seconds: where the
/// activation rises fastest in the 80 ms before its peak. A frame's window is centred on
/// `t · hop`, and a Hann window's weight is largest at its centre, so the rise is steepest
/// when the attack crosses a window's centre — between the two frames whose difference it
/// is, interpolated over the slopes either side.
fn attack(s: &Spectrogram, h: &[f32], t: usize) -> f64 {
    let lo = t.saturating_sub(8).max(1);
    let slope = |i: usize| -> f32 {
        if i >= 1 && i < h.len() {
            h[i] - h[i - 1]
        } else {
            0.0
        }
    };
    let mut best = t.max(1);
    for i in lo..=t.max(1) {
        if slope(i) > slope(best) {
            best = i;
        }
    }
    let (a, b, c) = (slope(best - 1), slope(best), slope(best + 1));
    let den = a - 2.0 * b + c;
    let shift = if den < 0.0 {
        (0.5 * (a - c) / den).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let frame = best as f64 - 0.5 + shift as f64;
    (frame * s.hop as f64 / s.rate as f64).max(0.0)
}

#[cfg(test)]
#[path = "onset_drums_tests.rs"]
mod tests;

#[cfg(test)]
mod cancel_tests {
    use super::*;

    #[test]
    fn a_cancel_stops_the_factorisation() {
        let x: Vec<f32> = (0..44_100)
            .map(|i| ((i % 441) as f32 / 441.0) - 0.5)
            .collect();
        let asked = std::cell::Cell::new(0);
        let r = transcribe_until(&x, 44_100.0, 0.5, &Tuning::default(), &|| {
            asked.set(asked.get() + 1);
            asked.get() > 3
        });
        assert!(r.is_none());
        assert_eq!(
            asked.get(),
            4,
            "asked before each iteration, stopped at once"
        );
        assert!(transcribe_until(&x, 44_100.0, 0.5, &Tuning::default(), &|| false).is_some());
    }
}
