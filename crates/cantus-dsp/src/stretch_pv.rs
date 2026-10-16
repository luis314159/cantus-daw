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
//! **Phase.** Identity phase locking (Laroche–Dolson): each spectral peak of the mid
//! channel advances its phase by `H · ratio · ω`; the bins around it keep their analysed
//! phase relation to it, so a partial stays one coherent lobe (no phasiness). **Both
//! channels share that phase**: each keeps its own analysed difference from the mid, so the
//! stereo image is the source's at any stretch.
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

struct Bins {
    mag: Vec<f32>,
    phase: Vec<f32>,
}

impl Bins {
    fn new(h: usize) -> Self {
        Bins {
            mag: vec![0.0; h],
            phase: vec![0.0; h],
        }
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
    mid: Bins,
    mid0: Vec<f32>,
    l: Bins,
    r: Bins,
    psi: Vec<f32>,
    omega: Vec<f32>,
    peaks: Vec<usize>,
    owner: Vec<usize>,
    yl: Vec<(f32, f32)>,
    yr: Vec<(f32, f32)>,
    env: Option<Envelope>,
    env_src: Vec<f32>,
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
        mid: Bins::new(bins),
        mid0: vec![0.0; bins],
        l: Bins::new(bins),
        r: Bins::new(bins),
        psi: vec![0.0; bins],
        omega: vec![0.0; bins],
        peaks: Vec::with_capacity(bins / 2),
        owner: vec![0; bins],
        yl: vec![(0.0, 0.0); bins],
        yr: vec![(0.0, 0.0); bins],
        env: (formants && ratio != 1.0).then(|| Envelope::new(n, rate)),
        env_src: vec![1.0; bins],
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
    /// for a pair) hold the frame to overlap-add, unwindowed and unscaled.
    fn frame(&mut self, ch: &[&[f32]], a: i64, ratio: f32, first: bool) {
        let (n, hs, half) = (self.n, self.hs, self.n / 2);
        let bins = half + 1;
        let stereo = ch.len() == 2;
        // The frame and the one a synthesis hop before it.
        analyse(&self.fft, ch, &self.win, a, &mut self.re, &mut self.im);
        unpack(
            &self.re,
            &self.im,
            stereo,
            &mut self.mid,
            &mut self.l,
            &mut self.r,
        );
        analyse(
            &self.fft,
            ch,
            &self.win,
            a - hs as i64,
            &mut self.re,
            &mut self.im,
        );
        unpack_mid_phase(&self.re, &self.im, stereo, &mut self.mid0);
        // The instantaneous frequency of every bin (radians per frame).
        for k in 0..bins {
            let wk = 2.0 * PI * k as f32 / n as f32;
            let d = self.mid.phase[k] - self.mid0[k] - wk * hs as f32;
            self.omega[k] = wk + princarg(d) / hs as f32;
        }
        find_peaks(&self.mid.mag, &mut self.peaks, &mut self.owner);
        let step = hs as f32 * ratio;
        if first {
            self.psi.copy_from_slice(&self.mid.phase);
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
                    self.psi[k] = princarg(self.psi[p] + self.mid.phase[k] - self.mid.phase[p]);
                }
            }
        }
        if let Some(e) = self.env.as_mut() {
            e.compute(&self.mid.mag, &mut self.env_src);
        }
        self.map_bins(stereo, ratio);
        // DC and Nyquist are real.
        for y in [&mut self.yl, &mut self.yr] {
            y[0].1 = 0.0;
            y[half].1 = 0.0;
        }
        // Pack L + jR into one inverse transform.
        for k in 0..=half {
            let (lr, li) = self.yl[k];
            let (rr, ri) = self.yr[k];
            self.re[k] = lr - ri;
            self.im[k] = li + rr;
            if k > 0 && k < half {
                self.re[n - k] = lr + ri;
                self.im[n - k] = rr - li;
            }
        }
        self.fft.run(&mut self.re, &mut self.im, true);
    }

    /// Bin by bin: output bin `m` from source bin `m / ratio` — the magnitude interpolated,
    /// the phase the nearest source bin's (on zero-phase frames, the same across a lobe).
    /// Moving whole peak regions by whole bins (Laroche–Dolson 1999b) was measured too: it
    /// kept a shifted voice's envelope a dB worse (a lobe a fraction of a bin off its
    /// frequency beats from frame to frame).
    fn map_bins(&mut self, stereo: bool, ratio: f32) {
        let half = self.n / 2;
        let bins = half + 1;
        let shift = ratio != 1.0;
        for m in 0..bins {
            let q = if shift { m as f32 / ratio } else { m as f32 };
            if q > half as f32 {
                self.yl[m] = (0.0, 0.0);
                self.yr[m] = (0.0, 0.0);
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
            let mut gain = 1.0f32;
            if self.env.is_some() {
                let den = lerp(&self.env_src);
                gain = (self.env_src[m] / den.max(1e-12)).clamp(1.0 / 32.0, 32.0);
            }
            let base = self.psi[i];
            if stereo {
                let pl = base + (self.l.phase[i] - self.mid.phase[i]);
                let pr = base + (self.r.phase[i] - self.mid.phase[i]);
                let (ml, mr) = (lerp(&self.l.mag) * gain, lerp(&self.r.mag) * gain);
                let (sl, cl) = math::sin_cos(pl);
                let (sr, cr) = math::sin_cos(pr);
                self.yl[m] = (ml * cl, ml * sl);
                self.yr[m] = (mr * cr, mr * sr);
            } else {
                let ml = lerp(&self.mid.mag) * gain;
                let (s, c) = math::sin_cos(base);
                self.yl[m] = (ml * c, ml * s);
                self.yr[m] = (0.0, 0.0);
            }
        }
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

/// The two channels' spectra out of one transform of `L + jR`: magnitudes and phases of
/// the mid (`L + R`), and of each channel when `stereo`.
fn unpack(re: &[f32], im: &[f32], stereo: bool, mid: &mut Bins, l: &mut Bins, r: &mut Bins) {
    let n = re.len();
    for k in 0..mid.mag.len() {
        let (a, b) = (re[k], im[k]);
        if !stereo {
            mid.mag[k] = math::hypot(a, b);
            mid.phase[k] = math::atan2(b, a);
            continue;
        }
        let nk = (n - k) % n;
        let (c, d) = (re[nk], im[nk]);
        let (lr, li) = (0.5 * (a + c), 0.5 * (b - d));
        let (rr, ri) = (0.5 * (b + d), 0.5 * (c - a));
        let (mr, mi) = (lr + rr, li + ri);
        l.mag[k] = math::hypot(lr, li);
        l.phase[k] = math::atan2(li, lr);
        r.mag[k] = math::hypot(rr, ri);
        r.phase[k] = math::atan2(ri, rr);
        mid.mag[k] = math::hypot(mr, mi);
        mid.phase[k] = math::atan2(mi, mr);
    }
}

fn unpack_mid_phase(re: &[f32], im: &[f32], stereo: bool, phase: &mut [f32]) {
    let n = re.len();
    for (k, p) in phase.iter_mut().enumerate() {
        let (a, b) = (re[k], im[k]);
        if !stereo {
            *p = math::atan2(b, a);
            continue;
        }
        let nk = (n - k) % n;
        let (c, d) = (re[nk], im[nk]);
        // Mid = L + R = ((a + c) + (b + d))/2 + j((b − d) + (c − a))/2.
        let mr = 0.5 * (a + c + b + d);
        let mi = 0.5 * (b - d + c - a);
        *p = math::atan2(mi, mr);
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
