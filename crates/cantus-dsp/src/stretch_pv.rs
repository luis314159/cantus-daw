//! The phase-locked phase vocoder (Complex's harmonic part).
//!
//! **Frames.** Synthesis frames sit a fixed hop `H = N/8` apart in the output; each reads
//! an analysis frame centred where the map says (rounded to a frame), and a second one
//! `H` source frames earlier — so the instantaneous frequency of every bin comes from two
//! frames exactly `H` apart, whatever the map's speed there (Flanagan–Golden; the two-frame
//! scheme lets the analysis hop vary freely). Hann windows, `N` ≈ 170 ms (8 192 frames at
//! 44.1/48 kHz): long enough to resolve a bass's partials and a chord's close voices; the transients are not here
//! (the percussive part carries them, `stretch_hpss`).
//!
//! **Phase.** Identity phase locking (Laroche–Dolson), channel by channel: each spectral
//! peak advances its phase by `H · ratio · ω`; the bins around it keep their analysed
//! phase relation to it, so a partial stays one coherent lobe (no phasiness). Each channel
//! follows its own partials (a chorus spreads them apart left and right: one phase for
//! both would make each beat), and the image holds: the same partial in both channels
//! has the same ω, so its phase difference is the source's for good, and transients reset
//! both channels together.
//!
//! **Transposition** maps bins: output bin `m` takes the magnitude at source bin `m/ratio`
//! and the phase of the nearest source bin (advanced at `ratio · ω`); with the formants
//! kept, the magnitude is moved back under the source's spectral envelope at `m`
//! (`stretch_env`, the true envelope).

// Index loops over spectra and frames read clearer than iterator chains here.
#![allow(clippy::needless_range_loop)]

use super::env::Envelope;
use super::map::Hold;
use super::Map;
use crate::aequalis::fft::Fft;
use crate::math;
use std::f32::consts::PI;

/// The frame length for `rate`: ≈ 170 ms, a power of two — long enough to keep partials
/// 30 Hz apart apart (at 85 ms a chord's low voices shared lobes, and the bins between,
/// locked to one of them, beat: round 29's review measured a detuned pad's voice
/// wandering 5 dB; at 170 ms, under 0.7 dB). The transients do not pay for it: they are
/// held, and the percussive part has its own short grains.
pub(crate) fn frame_len(rate: f32) -> usize {
    ((0.17 * rate) as usize)
        .next_power_of_two()
        .clamp(2_048, 32_768)
}

pub(crate) fn princarg(x: f32) -> f32 {
    x - 2.0 * PI * (x / (2.0 * PI)).round()
}

/// One synthesis frame's account, for [`level_match`]: its centre (output frames), the
/// source frame it read, and the power it means to give against the power it analysed
/// (their ratio is what the transposition does to the level: a band moved out, the
/// formants put back).
#[derive(Copy, Clone, Debug)]
pub(crate) struct Meant {
    pub centre: i64,
    pub read: i64,
    pub meant: f64,
    pub analysed: f64,
}

/// Render the time map and transposition over one or two channels (the harmonic part),
/// and each synthesis frame's [`Meant`].
pub(crate) fn render(
    channels: &[&[f32]],
    rate: f32,
    map: &Map,
    holds: &[Hold],
    len: usize,
    ratio: f64,
    formants: bool,
) -> (Vec<Vec<f32>>, Vec<Meant>) {
    // Pairs share a phase; a third channel (never in a clip today) starts a new pair.
    let mut out = Vec::with_capacity(channels.len());
    let mut meant: Vec<Meant> = Vec::new();
    for pair in channels.chunks(2) {
        let (o, m) = render_pair(pair, rate, map, holds, len, ratio, formants);
        out.extend(o);
        if meant.is_empty() {
            meant = m;
        } else {
            for (a, b) in meant.iter_mut().zip(m) {
                a.meant += b.meant;
                a.analysed += b.analysed;
            }
        }
    }
    (out, meant)
}

/// One channel's spectra and phase state.
struct Chan {
    mag: Vec<f32>,
    /// The magnitudes of the frame a synthesis hop before (a transient is where they rise).
    mag0: Vec<f32>,
    phase: Vec<f32>,
    phase0: Vec<f32>,
    psi: Vec<f32>,
    omega: Vec<f32>,
    peaks: Vec<usize>,
    owner: Vec<usize>,
    y: Vec<(f32, f32)>,
}

/// How a frame's phases start.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Reset {
    /// Advanced from the last frame (locked around peaks).
    None,
    /// Every bin the analysed phase (the first frame).
    All,
    /// At a transient: the regions whose peak rose against the frame a hop before take the
    /// analysed phase; the rest (a held bass, a pad) go on advancing, so a sustained partial
    /// never jumps between two phase versions under a hit (Röbel 2003's transient bins).
    Rising,
}

/// A peak whose magnitude grew by this factor over a hop is a transient's.
const RISE: f32 = 1.5;
/// The bins at the bottom (DC and its Hann lobe) and the Nyquist bin keep their analysed
/// phase: a DC offset stays a constant, never a slow oscillation.
const DC_BINS: usize = 2;

impl Chan {
    fn new(h: usize) -> Self {
        Chan {
            mag: vec![0.0; h],
            mag0: vec![0.0; h],
            phase: vec![0.0; h],
            phase0: vec![0.0; h],
            psi: vec![0.0; h],
            omega: vec![0.0; h],
            peaks: Vec::with_capacity(h / 2),
            owner: vec![0; h],
            y: vec![(0.0, 0.0); h],
        }
    }

    /// The phases advanced to this frame (or reset to the analysed ones).
    fn advance(&mut self, n: usize, hs: usize, ratio: f32, reset: Reset) {
        let bins = self.mag.len();
        for k in 0..bins {
            let wk = 2.0 * PI * k as f32 / n as f32;
            let d = self.phase[k] - self.phase0[k] - wk * hs as f32;
            self.omega[k] = wk + princarg(d) / hs as f32;
        }
        find_peaks(&self.mag, &mut self.peaks, &mut self.owner);
        let step = hs as f32 * ratio;
        if reset == Reset::All {
            self.psi.copy_from_slice(&self.phase);
        } else if self.peaks.is_empty() {
            for k in 0..bins {
                self.psi[k] = if reset == Reset::Rising && self.mag[k] > RISE * self.mag0[k] {
                    self.phase[k]
                } else {
                    princarg(self.psi[k] + step * self.omega[k])
                };
            }
        } else {
            for &p in &self.peaks {
                self.psi[p] = if reset == Reset::Rising && self.mag[p] > RISE * self.mag0[p] {
                    self.phase[p]
                } else {
                    princarg(self.psi[p] + step * self.omega[p])
                };
            }
            for k in 0..bins {
                let p = self.owner[k];
                if p != k {
                    self.psi[k] = princarg(self.psi[p] + self.phase[k] - self.phase[p]);
                }
            }
        }
        for k in (0..DC_BINS.min(bins)).chain([bins - 1]) {
            self.psi[k] = self.phase[k];
        }
    }

    /// The output bins. Untransposed, each bin as it is (its magnitude, its phase).
    /// Transposed, **each peak's region is moved whole** (Laroche–Dolson): the region of a
    /// partial at bin `p` (its true frequency, from ω) is moved by `p·(ratio − 1)` bins —
    /// fractionally, its magnitudes interpolated — so the partial's lobe keeps its shape
    /// and its level, centred on its new frequency (mapping bin `m` from `m / ratio`
    /// instead stretched a lobe by the ratio: +1.8 dB an octave up, −4 dB down). Regions
    /// moved apart leave gaps, moved together add up — so noise keeps its power as a
    /// resampled signal would. With `env`, each bin's magnitude is moved back under the
    /// source's envelope at its new place (the formants kept). DC and its lobe stay put.
    fn synthesise(&mut self, ratio: f32, env: Option<&[f32]>, n: usize) {
        let half = self.mag.len() - 1;
        if ratio == 1.0 {
            for m in 0..=half {
                let (s, c) = math::sin_cos(self.psi[m]);
                self.y[m] = (self.mag[m] * c, self.mag[m] * s);
            }
        } else {
            for v in self.y.iter_mut() {
                *v = (0.0, 0.0);
            }
            let lerp = |v: &[f32], q: f32| -> f32 {
                let f = (q.floor().max(0.0) as usize).min(half);
                let t = (q - f as f32).clamp(0.0, 1.0);
                let b = v[(f + 1).min(half)];
                v[f] + (b - v[f]) * t
            };
            let bins = |q: f32| -> f32 { between(&self.mag, q) };
            let to_bin = n as f32 / (2.0 * PI);
            for k in 0..DC_BINS.min(half) {
                let (s, c) = math::sin_cos(self.psi[k]);
                self.y[k] = (self.mag[k] * c, self.mag[k] * s);
            }
            // The regions: runs of bins with one owner, above the DC bins.
            let mut lo = DC_BINS;
            while lo < half {
                let p = self.owner[lo];
                let mut hi = lo + 1;
                while hi < half && self.owner[hi] == p {
                    hi += 1;
                }
                let pf = p as f32;
                let true_bin = (self.omega[p] * to_bin).clamp(pf - 0.5, pf + 0.5);
                let d = true_bin * (ratio - 1.0);
                let first = ((lo as f32 + d).ceil().max(DC_BINS as f32)) as usize;
                let last = ((hi as f32 + d).ceil().max(0.0) as usize).min(half);
                for m in first..last {
                    let q = m as f32 - d;
                    if q < 0.0 || q > half as f32 {
                        continue;
                    }
                    let gain = match env {
                        Some(e) => (e[m] / lerp(e, q).max(1e-12)).clamp(1.0 / 32.0, 32.0),
                        None => 1.0,
                    };
                    let a = bins(q) * gain;
                    let i = (q.round() as usize).clamp(lo, hi - 1);
                    let (s, c) = math::sin_cos(self.psi[i]);
                    self.y[m].0 += a * c;
                    self.y[m].1 += a * s;
                }
                lo = hi;
            }
        }
        self.y[0].1 = 0.0;
        self.y[half].1 = 0.0;
    }
}

/// Taps either side of a fractional bin in [`between`].
const SPAN: i64 = 4;
/// Fractional positions per table.
const STEPS: usize = 64;

/// The interpolation weights: for `STEPS + 1` fractions `f` of a bin, `2·SPAN` taps at
/// `k − f`, `k` from `1 − SPAN` to `SPAN` — a Hann-windowed sinc, each row summing to 1.
fn weights() -> &'static Vec<[f32; 8]> {
    static W: std::sync::OnceLock<Vec<[f32; 8]>> = std::sync::OnceLock::new();
    W.get_or_init(|| {
        (0..=STEPS)
            .map(|i| {
                let f = i as f64 / STEPS as f64;
                let mut row = [0.0f64; 8];
                for (j, w) in row.iter_mut().enumerate() {
                    let x = (j as i64 + 1 - SPAN) as f64 - f;
                    let sinc = if x.abs() < 1e-12 {
                        1.0
                    } else {
                        let a = std::f64::consts::PI * x;
                        math::sin(a) / a
                    };
                    let win = 0.5 + 0.5 * math::cos(std::f64::consts::PI * x / (SPAN as f64 + 1.0));
                    *w = sinc * win;
                }
                let sum: f64 = row.iter().sum();
                let mut out = [0.0f32; 8];
                for (o, w) in out.iter_mut().zip(row) {
                    *o = (w / sum) as f32;
                }
                out
            })
            .collect()
    })
}

/// The magnitude spectrum `v` (bins `0..=half`) between its bins, at `q`: a windowed-sinc
/// interpolation, mirrored at DC and Nyquist (a real signal's spectrum). Unlike a straight
/// line between two bins it keeps a lobe's height and a noise's power at any fraction (a
/// line loses up to 3 dB of noise half-way between bins).
fn between(v: &[f32], q: f32) -> f32 {
    let half = (v.len() - 1) as i64;
    let i = q.floor() as i64;
    let f = q - i as f32;
    let row = &weights()[((f * STEPS as f32).round() as usize).min(STEPS)];
    let at = |k: i64| -> f32 {
        let k = k.abs();
        let k = if k > half { 2 * half - k } else { k };
        v[k.clamp(0, half) as usize]
    };
    let mut acc = 0.0f32;
    for (j, w) in row.iter().enumerate() {
        acc += w * at(i + j as i64 + 1 - SPAN);
    }
    acc.max(0.0)
}

/// The working state of one pair's render.
struct State {
    n: usize,
    hs: usize,
    fft: Fft,
    win: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    ch: Vec<Chan>,
    env: Option<Envelope>,
    env_src: Vec<f32>,
    sum: Vec<f32>,
}

fn render_pair(
    ch: &[&[f32]],
    rate: f32,
    map: &Map,
    holds: &[Hold],
    len: usize,
    ratio: f64,
    formants: bool,
) -> (Vec<Vec<f32>>, Vec<Meant>) {
    let n = frame_len(rate);
    let hs = n / 8;
    let half = n / 2;
    let bins = half + 1;
    let win: Vec<f32> = (0..n)
        .map(|i| (0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / n as f64)) as f32)
        .collect();
    let ola: f32 = (0..n / hs).map(|j| win[j * hs] * win[j * hs]).sum();
    let scale = 1.0 / (n as f32 * ola);
    let mut st = State {
        n,
        hs,
        fft: Fft::new(n),
        win,
        re: vec![0.0; n],
        im: vec![0.0; n],
        ch: (0..ch.len()).map(|_| Chan::new(bins)).collect(),
        env: (formants && ratio != 1.0).then(|| Envelope::new(n, rate)),
        env_src: vec![1.0; bins],
        sum: vec![0.0; bins],
    };
    let pad = n;
    let stereo = ch.len() == 2;
    let mut acc: Vec<Vec<f32>> = (0..ch.len()).map(|_| vec![0.0f32; len + 2 * pad]).collect();
    let mut first = true;
    let mut next_hold = 0usize;
    // Each frame's centre and the power it means to give (its own content, as if every
    // overlapping frame agreed with it).
    let w2: f64 = st.win.iter().map(|w| (*w as f64) * (*w as f64)).sum();
    let mut meant: Vec<Meant> = Vec::new();
    let mut j = -((half / hs) as i64);
    loop {
        let c = j * hs as i64;
        if c - half as i64 >= len as i64 {
            break;
        }
        let a = map.source(c as f64).round() as i64;
        // Around a transient the map is a pure shift: untransposed, every frame there resets
        // the bins the transient raised to their analysed phase, so the attack is the
        // source's; transposed, once, on the frame nearest the transient (Röbel). A hold is
        // passed only once both its end and that frame are behind (a short hold may end
        // before its nearest frame).
        let at_hold = at_hold(holds, &mut next_hold, c as f64, hs, ratio == 1.0);
        let reset = if first {
            Reset::All
        } else if at_hold {
            Reset::Rising
        } else {
            Reset::None
        };
        st.frame(ch, a, ratio as f32, reset);
        first = false;
        let e: f64 = (0..n)
            .map(|i| {
                let (r, m) = (st.re[i] as f64, st.im[i] as f64);
                r * r + if stereo { m * m } else { 0.0 }
            })
            .sum();
        let analysed: f64 = st
            .ch
            .iter()
            .map(|c| {
                let m = &c.mag;
                let h = m.len() - 1;
                let inner: f64 = m[1..h].iter().map(|v| (*v as f64) * (*v as f64)).sum();
                (m[0] as f64) * (m[0] as f64) + (m[h] as f64) * (m[h] as f64) + 2.0 * inner
            })
            .sum::<f64>()
            / (n as f64 * w2);
        meant.push(Meant {
            centre: c,
            read: a,
            meant: e / ((n * n) as f64 * w2),
            analysed,
        });
        let base = c - half as i64 + pad as i64;
        for i in 0..n {
            let at = base + i as i64;
            if at < 0 || at as usize >= acc[0].len() {
                continue;
            }
            let g = st.win[i] * scale;
            let k = (i + half) % n;
            acc[0][at as usize] += st.re[k] * g;
            if stereo {
                acc[1][at as usize] += st.im[k] * g;
            }
        }
        j += 1;
    }
    let out = acc
        .into_iter()
        .map(|v| v[pad..pad + len].to_vec())
        .collect();
    (out, meant)
}

/// The Hann window of `n`.
pub(crate) fn window(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / n as f64)) as f32)
        .collect()
}

/// The power of `x` (all channels summed) around output frame `c`, weighted by the
/// window's square (as a synthesis frame centred there sees it).
pub(crate) fn power_at(x: &[Vec<f32>], c: i64, win: &[f32], w2: f64) -> f64 {
    let n = win.len() as i64;
    let mut e = 0.0f64;
    for i in 0..n {
        let at = c - n / 2 + i;
        if at < 0 || at as usize >= x[0].len() {
            continue;
        }
        let w = win[i as usize] as f64;
        for ch in x {
            let v = ch[at as usize] as f64;
            e += v * v * w * w;
        }
    }
    e / w2
}

/// The power of the source `x` (all channels summed) around source frame `a`, weighted
/// by the window's square.
pub(crate) fn power_of(x: &[&[f32]], a: i64, win: &[f32], w2: f64) -> f64 {
    let n = win.len() as i64;
    let mut e = 0.0f64;
    for i in 0..n {
        let at = a - n / 2 + i;
        let w = win[i as usize] as f64;
        for ch in x {
            if at >= 0 && (at as usize) < ch.len() {
                let v = ch[at as usize] as f64;
                e += v * v * w * w;
            }
        }
    }
    e / w2
}

/// Frames either side over which the level is matched (≈ 170 ms at 48 kHz).
const MATCH_FRAMES: usize = 8;
/// The most the level match may raise or lower, as a gain.
const MATCH_MOST: f64 = 1.41;

/// Hold the output's level to what its frames mean to give. A phase vocoder's frames agree
/// exactly on a steady partial, but on noise and dense mixtures the peaks move from frame
/// to frame, overlapping frames add with partly random phases and the level sinks (about
/// 1 dB on dense noise): the power each frame means (`meant`, from its own content) is
/// compared with the power the overlap-add gave around its centre, both summed over
/// [`MATCH_FRAMES`] either side, and the ratio — at most [`MATCH_MOST`] either way —
/// applied, interpolated between frame centres. On a steady tone it is 1 (round 29's
/// review: the level of a transposed render).
pub(crate) fn level_match(acc: &mut [Vec<f32>], meant: &[(i64, f64)], rate: f32) {
    // `meant`: each frame's centre and the power the output should have around it.
    if meant.is_empty() || acc.is_empty() {
        return;
    }
    let win = window(frame_len(rate));
    let w2: f64 = win.iter().map(|w| (*w as f64) * (*w as f64)).sum();
    let pad = 0usize;
    let got: Vec<f64> = meant
        .iter()
        .map(|&(c, _)| power_at(acc, c, &win, w2))
        .collect();
    // Only frames wholly inside the output are measured (one cut by its start or end sees
    // part of what it meant); the others take their nearest measured neighbour's gain.
    let n = win.len() as i64;
    let len = acc[0].len() as i64;
    let whole: Vec<bool> = meant
        .iter()
        .map(|&(c, _)| c - n / 2 >= 0 && c + n / 2 <= len)
        .collect();
    let m = meant.len();
    let mut gains: Vec<Option<f64>> = (0..m)
        .map(|j| {
            if !whole[j] {
                return None;
            }
            let lo = j.saturating_sub(MATCH_FRAMES);
            let hi = (j + MATCH_FRAMES + 1).min(m);
            let (mut want, mut have) = (0.0f64, 0.0f64);
            for i in (lo..hi).filter(|i| whole[*i]) {
                want += meant[i].1;
                have += got[i];
            }
            Some(if want <= 1e-20 || have <= 1e-20 {
                1.0
            } else {
                (want / have).sqrt().clamp(1.0 / MATCH_MOST, MATCH_MOST)
            })
        })
        .collect();
    let Some(first) = gains.iter().position(Option::is_some) else {
        return;
    };
    for j in 0..m {
        if gains[j].is_none() {
            let back = (0..j).rev().find_map(|i| gains[i]);
            gains[j] = Some(back.unwrap_or_else(|| gains[first].unwrap_or(1.0)));
        }
    }
    let gains: Vec<f64> = gains.into_iter().map(|g| g.unwrap_or(1.0)).collect();
    if gains.iter().all(|g| (*g - 1.0).abs() < 1e-9) {
        return;
    }
    for t in 0..len {
        let c = t - pad as i64;
        let j = meant.partition_point(|x| x.0 <= c);
        let g = match j {
            0 => gains[0],
            j if j >= m => gains[m - 1],
            j => {
                let (a, b) = (meant[j - 1].0, meant[j].0);
                let f = (c - a) as f64 / (b - a).max(1) as f64;
                gains[j - 1] + (gains[j] - gains[j - 1]) * f
            }
        };
        for ch in acc.iter_mut() {
            ch[t as usize] *= g as f32;
        }
    }
}

/// Whether the synthesis frame centred at `cf` resets at a hold: untransposed, every
/// frame inside one; transposed, the frame nearest its transient. `next` walks the holds
/// (in order) as the frames do. Untransposed, a hold is passed once it ends, and every
/// hold that has begun is asked (round 29's second review: a flam's short first hold hid
/// the second); transposed, only once both its end and its nearest frame are behind — a
/// short hold may end before the frame nearest its transient.
pub(crate) fn at_hold(holds: &[Hold], next: &mut usize, cf: f64, hs: usize, plain: bool) -> bool {
    let nearest = |h: &Hold| (h.at / hs as f64).round() * hs as f64;
    if plain {
        while *next < holds.len() && holds[*next].to < cf {
            *next += 1;
        }
        return holds[*next..]
            .iter()
            .take_while(|h| h.from <= cf)
            .any(|h| cf <= h.to);
    }
    while *next < holds.len() && holds[*next].to < cf && nearest(&holds[*next]) < cf {
        *next += 1;
    }
    holds.get(*next).is_some_and(|h| nearest(h) == cf)
}

impl State {
    /// One synthesis frame reading the source around frame `a`: on return, `re` (and `im`,
    /// for a pair) hold the frame to overlap-add, zero-phase and unscaled.
    fn frame(&mut self, ch: &[&[f32]], a: i64, ratio: f32, reset: Reset) {
        let (n, hs, half) = (self.n, self.hs, self.n / 2);
        let stereo = ch.len() == 2;
        // The frame and the one a synthesis hop before it.
        analyse(&self.fft, ch, &self.win, a, &mut self.re, &mut self.im);
        unpack(&self.re, &self.im, &mut self.ch, true);
        analyse(
            &self.fft,
            ch,
            &self.win,
            a - hs as i64,
            &mut self.re,
            &mut self.im,
        );
        unpack(&self.re, &self.im, &mut self.ch, false);
        for c in &mut self.ch {
            c.advance(n, hs, ratio, reset);
        }
        // The image: where both channels hold the same partial — their phase difference
        // holding still over a hop, the same frequency to a twenty-fifth of a bin (half a
        // hertz at 48 kHz) — the right one keeps the source's phase difference from the
        // left, every frame, so silence or a reset never loses it; where they differ (a
        // chorus, a detuned voice a few hertz off), each keeps its own: locked, the right
        // one was pulled to the left's frequency and beat against itself (round 29's
        // review measured a detuned pad's voice wandering 5 dB).
        if stereo {
            let tol = 2.0 * PI * 0.04 / n as f32;
            let (l, r) = self.ch.split_at_mut(1);
            let (l, r) = (&l[0], &mut r[0]);
            let top = l.mag.iter().chain(&r.mag).fold(0.0f32, |m, v| m.max(*v));
            let floor = top * 1e-4;
            for k in 0..=half {
                if (l.omega[k] - r.omega[k]).abs() < tol && l.mag[k] > floor && r.mag[k] > floor {
                    r.psi[k] = princarg(l.psi[k] + (r.phase[k] - l.phase[k]));
                }
            }
        }
        // One envelope for both channels: of their summed magnitudes (no cancellation).
        let env = match self.env.as_mut() {
            Some(e) => {
                for (k, s) in self.sum.iter_mut().enumerate() {
                    *s = self.ch.iter().map(|c| c.mag[k]).sum();
                }
                e.compute(&self.sum, &mut self.env_src);
                Some(self.env_src.as_slice())
            }
            None => None,
        };
        for c in &mut self.ch {
            c.synthesise(ratio, env, n);
        }
        // Pack L + jR into one inverse transform.
        let zero = vec![(0.0f32, 0.0f32); 0];
        let yl = &self.ch[0].y;
        let yr = if stereo { &self.ch[1].y } else { &zero };
        for k in 0..=half {
            let (lr, li) = yl[k];
            let (rr, ri) = yr.get(k).copied().unwrap_or((0.0, 0.0));
            self.re[k] = lr - ri;
            self.im[k] = li + rr;
            if k > 0 && k < half {
                self.re[n - k] = lr + ri;
                self.im[n - k] = rr - li;
            }
        }
        self.fft.run(&mut self.re, &mut self.im, true);
    }
}

/// Window the source around frame `a` (`L` in the real part, `R` in the imaginary) and
/// transform.
fn analyse(fft: &Fft, ch: &[&[f32]], win: &[f32], a: i64, re: &mut [f32], im: &mut [f32]) {
    let n = win.len();
    let start = a - (n / 2) as i64;
    let get = |x: &[f32], i: i64| -> f32 {
        if i >= 0 && (i as usize) < x.len() {
            x[i as usize]
        } else {
            0.0
        }
    };
    // Zero-phase: the frame's centre at index 0 (rotated by half), so a steady partial's
    // phase is nearly the same across its lobe and bins can be moved without breaking it.
    for i in 0..n {
        let at = start + i as i64;
        let k = (i + n / 2) % n;
        re[k] = get(ch[0], at) * win[i];
        im[k] = if ch.len() > 1 {
            get(ch[1], at) * win[i]
        } else {
            0.0
        };
    }
    fft.run(re, im, false);
}

/// The channels' spectra out of one transform of `L + jR`: magnitudes and phases (`now`),
/// or the phases of the frame a hop before.
fn unpack(re: &[f32], im: &[f32], ch: &mut [Chan], now: bool) {
    let n = re.len();
    let stereo = ch.len() == 2;
    let bins = ch[0].mag.len();
    for k in 0..bins {
        let (a, b) = (re[k], im[k]);
        let nk = (n - k) % n;
        let (c, d) = (re[nk], im[nk]);
        let parts = if stereo {
            [
                (0.5 * (a + c), 0.5 * (b - d)),
                (0.5 * (b + d), 0.5 * (c - a)),
            ]
        } else {
            [(a, b), (0.0, 0.0)]
        };
        for (x, (pr, pi)) in ch.iter_mut().zip(parts) {
            if now {
                x.mag[k] = math::hypot(pr, pi);
                x.phase[k] = math::atan2(pi, pr);
            } else {
                x.mag0[k] = math::hypot(pr, pi);
                x.phase0[k] = math::atan2(pi, pr);
            }
        }
    }
}

/// The spectral peaks of `mag` (above a floor 100 dB under the largest), and for every bin
/// the peak whose region it is in (regions split at the lowest bin between two peaks).
fn find_peaks(mag: &[f32], peaks: &mut Vec<usize>, owner: &mut [usize]) {
    peaks.clear();
    let top = mag.iter().fold(0.0f32, |m, v| m.max(*v));
    let floor = top * 1e-5;
    let h = mag.len();
    for k in 2..h.saturating_sub(2) {
        let v = mag[k];
        if v > floor && v > mag[k - 1] && v >= mag[k + 1] && v > mag[k - 2] && v >= mag[k + 2] {
            peaks.push(k);
        }
    }
    if peaks.is_empty() {
        for (k, o) in owner.iter_mut().enumerate() {
            *o = k;
        }
        return;
    }
    let mut start = 0;
    for w in 0..peaks.len() {
        let p = peaks[w];
        let end = match peaks.get(w + 1) {
            // Up to and including the lowest bin between the two peaks.
            Some(&q) => {
                (p..=q)
                    .min_by(|&a, &b| mag[a].total_cmp(&mag[b]))
                    .unwrap_or(p)
                    + 1
            }
            None => h,
        };
        for o in &mut owner[start..end] {
            *o = p;
        }
        start = end;
    }
}
