//! The phase-locked phase vocoder (Complex's harmonic part).
//!
//! **Frames.** Synthesis frames sit a fixed hop `H = N/8` apart in the output; each reads
//! an analysis frame centred where the map says (rounded to a frame), and a second one
//! `H` source frames earlier — so the instantaneous frequency of every bin comes from two
//! frames exactly `H` apart, whatever the map's speed there (Flanagan–Golden; the two-frame
//! scheme lets the analysis hop vary freely). Hann windows, `N` ≈ 85 ms (4 096 frames at
//! 44.1/48 kHz): long enough to resolve a bass's partials; the transients are not here
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

/// The frame length for `rate`: ≈ 85 ms, a power of two.
pub(crate) fn frame_len(rate: f32) -> usize {
    ((0.085 * rate) as usize)
        .next_power_of_two()
        .clamp(1_024, 16_384)
}

pub(crate) fn princarg(x: f32) -> f32 {
    x - 2.0 * PI * (x / (2.0 * PI)).round()
}

/// Render the time map and transposition over one or two channels (the harmonic part).
pub(crate) fn render(
    channels: &[&[f32]],
    rate: f32,
    map: &Map,
    holds: &[Hold],
    len: usize,
    ratio: f64,
    formants: bool,
) -> Vec<Vec<f32>> {
    // Pairs share a phase; a third channel (never in a clip today) starts a new pair.
    let mut out = Vec::with_capacity(channels.len());
    for pair in channels.chunks(2) {
        out.extend(render_pair(pair, rate, map, holds, len, ratio, formants));
    }
    out
}

/// One channel's spectra and phase state.
struct Chan {
    mag: Vec<f32>,
    phase: Vec<f32>,
    phase0: Vec<f32>,
    psi: Vec<f32>,
    omega: Vec<f32>,
    peaks: Vec<usize>,
    owner: Vec<usize>,
    y: Vec<(f32, f32)>,
}

impl Chan {
    fn new(h: usize) -> Self {
        Chan {
            mag: vec![0.0; h],
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
    fn advance(&mut self, n: usize, hs: usize, ratio: f32, reset: bool) {
        let bins = self.mag.len();
        for k in 0..bins {
            let wk = 2.0 * PI * k as f32 / n as f32;
            let d = self.phase[k] - self.phase0[k] - wk * hs as f32;
            self.omega[k] = wk + princarg(d) / hs as f32;
        }
        find_peaks(&self.mag, &mut self.peaks, &mut self.owner);
        let step = hs as f32 * ratio;
        if reset {
            self.psi.copy_from_slice(&self.phase);
        } else if self.peaks.is_empty() {
            for k in 0..bins {
                self.psi[k] = princarg(self.psi[k] + step * self.omega[k]);
            }
        } else {
            for &p in &self.peaks {
                self.psi[p] = princarg(self.psi[p] + step * self.omega[p]);
            }
            for k in 0..bins {
                let p = self.owner[k];
                if p != k {
                    self.psi[k] = princarg(self.psi[p] + self.phase[k] - self.phase[p]);
                }
            }
        }
    }

    /// The output bins: output bin `m` from source bin `m / ratio` — the magnitude
    /// interpolated (and, with `env`, moved back under the source's envelope), the phase
    /// the nearest source bin's (on zero-phase frames, the same across a lobe). Moving
    /// whole peak regions by whole bins (Laroche–Dolson 1999b) was measured too: it kept a
    /// shifted voice's envelope a dB worse (a lobe a fraction of a bin off its frequency
    /// beats from frame to frame).
    fn synthesise(&mut self, ratio: f32, env: Option<&[f32]>) {
        let half = self.mag.len() - 1;
        let shift = ratio != 1.0;
        for m in 0..=half {
            let q = if shift { m as f32 / ratio } else { m as f32 };
            if q > half as f32 {
                self.y[m] = (0.0, 0.0);
                continue;
            }
            let i = (q.round() as usize).min(half);
            let lerp = |v: &[f32]| -> f32 {
                if !shift {
                    return v[m];
                }
                let f = (q.floor() as usize).min(half);
                let t = q - f as f32;
                let b = v[(f + 1).min(half)];
                v[f] + (b - v[f]) * t
            };
            let gain = match env {
                Some(e) => (e[m] / lerp(e).max(1e-12)).clamp(1.0 / 32.0, 32.0),
                None => 1.0,
            };
            let a = lerp(&self.mag) * gain;
            let (s, c) = math::sin_cos(self.psi[i]);
            self.y[m] = (a * c, a * s);
        }
        self.y[0].1 = 0.0;
        self.y[half].1 = 0.0;
    }
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
) -> Vec<Vec<f32>> {
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
    let mut j = -((half / hs) as i64);
    loop {
        let c = j * hs as i64;
        if c - half as i64 >= len as i64 {
            break;
        }
        let a = map.source(c as f64).round() as i64;
        // Around a transient the map is a pure shift: untransposed, every frame there is
        // the analysed frame itself (its phases reset), so the attack is the source's;
        // transposed, the phases reset once, on the frame nearest the transient (Röbel).
        let cf = c as f64;
        while next_hold < holds.len() && holds[next_hold].to < cf {
            next_hold += 1;
        }
        let reset = holds.get(next_hold).is_some_and(|h| {
            if ratio == 1.0 {
                h.from <= cf
            } else {
                (h.at / hs as f64).round() * hs as f64 == cf
            }
        });
        st.frame(ch, a, ratio as f32, first || reset);
        first = false;
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
    acc.into_iter()
        .map(|v| v[pad..pad + len].to_vec())
        .collect()
}

impl State {
    /// One synthesis frame reading the source around frame `a`: on return, `re` (and `im`,
    /// for a pair) hold the frame to overlap-add, zero-phase and unscaled.
    fn frame(&mut self, ch: &[&[f32]], a: i64, ratio: f32, reset: bool) {
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
        // The image: where both channels hold the same partial (the same frequency to a
        // quarter of a bin), the right one keeps the source's phase difference from the
        // left — every frame, so silence or a reset never loses it; where they differ (a
        // chorus), each keeps its own.
        if stereo {
            let tol = 2.0 * PI * 0.25 / n as f32;
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
            c.synthesise(ratio, env);
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
