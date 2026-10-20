//! Complex's two parts: the input split into **harmonic** and **percussive** (Driedger,
//! Müller and Ewert 2014: median filtering of the spectrogram — along time, a partial
//! survives and a hit does not; along frequency, the reverse), and the percussive part
//! re-laid by short grains.
//!
//! The split is a soft (Wiener) mask on a ≈ 21 ms STFT, the same mask for both channels;
//! the percussive part is the input less the harmonic one, so the two always sum back to
//! the input exactly (up to rounding).

// Index loops over spectra and frames read clearer than iterator chains here.
#![allow(clippy::needless_range_loop)]

use super::Map;
use crate::aequalis::fft::Fft;
use crate::math;

/// The median filters' half-lengths: frames along time, bins along frequency (17 each,
/// the paper's lengths at its resolution).
const HALF_T: usize = 8;
const HALF_F: usize = 8;

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / n as f64)) as f32)
        .collect()
}

fn get(x: &[f32], i: i64) -> f32 {
    if i >= 0 && (i as usize) < x.len() {
        x[i as usize]
    } else {
        0.0
    }
}

fn median(v: &mut [f32]) -> f32 {
    let m = v.len() / 2;
    v.select_nth_unstable_by(m, |a, b| a.total_cmp(b));
    v[m]
}

/// Split `channels` into their harmonic and percussive parts, each as long as its channel.
pub(super) fn split(channels: &[&[f32]], rate: f32) -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {
    let mut harmonic = Vec::with_capacity(channels.len());
    for pair in channels.chunks(2) {
        harmonic.extend(split_pair(pair, rate));
    }
    let percussive = channels
        .iter()
        .zip(&harmonic)
        .map(|(x, h)| x.iter().zip(h).map(|(a, b)| a - b).collect())
        .collect();
    (harmonic, percussive)
}

/// One frame's spectra: the pair's packed transform and the mid's magnitudes.
struct Frame {
    re: Vec<f32>,
    im: Vec<f32>,
    mag: Vec<f32>,
}

/// The split's frame length for `rate`: ≈ 21 ms, a power of two (its hop a quarter).
pub(super) fn frame_len(rate: f32) -> usize {
    ((0.0213 * rate) as usize)
        .next_power_of_two()
        .clamp(256, 4_096)
}

fn split_pair(ch: &[&[f32]], rate: f32) -> Vec<Vec<f32>> {
    let n = frame_len(rate);
    let hop = n / 4;
    let half = n / 2;
    let bins = half + 1;
    let len = ch.iter().map(|c| c.len()).max().unwrap_or(0);
    let stereo = ch.len() == 2;
    let fft = Fft::new(n);
    let win = hann(n);
    let ola: f32 = (0..n / hop).map(|j| win[j * hop] * win[j * hop]).sum();
    let scale = 1.0 / (n as f32 * ola);
    let first = -((half / hop) as i64);
    let last = (len + half) as i64 / hop as i64 + 1;
    let pad = n;
    let mut acc: Vec<Vec<f32>> = (0..ch.len()).map(|_| vec![0.0; len + 2 * pad]).collect();
    // A ring of 2·HALF_T + 1 frames: the one masked is in the middle.
    let span = 2 * HALF_T + 1;
    let mut ring: Vec<Option<Frame>> = (0..span).map(|_| None).collect();
    let make = |f: i64| -> Frame {
        let start = f * hop as i64 - half as i64;
        let mut re = vec![0.0f32; n];
        let mut im = vec![0.0f32; n];
        for i in 0..n {
            re[i] = get(ch[0], start + i as i64) * win[i];
            if stereo {
                im[i] = get(ch[1], start + i as i64) * win[i];
            }
        }
        fft.run(&mut re, &mut im, false);
        let mut mag = vec![0.0f32; bins];
        for (k, m) in mag.iter_mut().enumerate() {
            let (a, b) = (re[k], im[k]);
            *m = if stereo {
                // |L + R| out of the packed L + jR.
                let nk = (n - k) % n;
                let (c, d) = (re[nk], im[nk]);
                math::hypot(0.5 * (a + c + b + d), 0.5 * (b - d + c - a))
            } else {
                math::hypot(a, b)
            };
        }
        Frame { re, im, mag }
    };
    let mut col = vec![0.0f32; span];
    let mut row = vec![0.0f32; 2 * HALF_F + 1];
    let mut mask = vec![0.0f32; bins];
    for f in first..=last + HALF_T as i64 {
        // Frame f enters the ring; frame f − HALF_T is masked.
        ring.rotate_left(1);
        ring[span - 1] = (f <= last).then(|| make(f));
        let c = f - HALF_T as i64;
        if c < first {
            continue;
        }
        let Some(centre) = ring[HALF_T].as_ref() else {
            continue;
        };
        for k in 0..bins {
            for (v, fr) in col.iter_mut().zip(&ring) {
                *v = fr.as_ref().map_or(0.0, |fr| fr.mag[k]);
            }
            let h = median(&mut col);
            for (i, v) in row.iter_mut().enumerate() {
                let b = k as i64 + i as i64 - HALF_F as i64;
                *v = if b >= 0 && (b as usize) < bins {
                    centre.mag[b as usize]
                } else {
                    0.0
                };
            }
            let p = median(&mut row);
            let (h2, p2) = (h * h, p * p);
            // Percussive only where it clearly is (Driedger's separation factor, 2, with
            // a soft edge): what is neither — a dense chord, a held noise — stays with the
            // phase vocoder, which keeps it whole; short grains would make it flutter.
            let _ = (h2, p2);
            let r = p / h.max(1e-12);
            let x = (r - 1.0).clamp(0.0, 1.0);
            mask[k] = 1.0 - x * x * (3.0 - 2.0 * x);
        }
        // The harmonic frame: the packed spectrum masked (the mask is real and the same
        // for both channels, so it may be applied to the packed transform directly, bin k
        // and bin n − k alike).
        let mut re = centre.re.clone();
        let mut im = centre.im.clone();
        for k in 0..n {
            let m = mask[if k <= half { k } else { n - k }];
            re[k] *= m;
            im[k] *= m;
        }
        fft.run(&mut re, &mut im, true);
        let base = c * hop as i64 - half as i64 + pad as i64;
        for i in 0..n {
            let at = base + i as i64;
            if at < 0 || at as usize >= acc[0].len() {
                continue;
            }
            let g = win[i] * scale;
            acc[0][at as usize] += re[i] * g;
            if stereo {
                acc[1][at as usize] += im[i] * g;
            }
        }
    }
    acc.into_iter()
        .zip(ch)
        .map(|(v, x)| v[pad..pad + x.len()].to_vec())
        .collect()
}

/// The percussive part re-laid through `map` by short Hann grains (≈ 5 ms, a quarter
/// apart), each read at `ratio` (the transposition) from where the map puts its centre.
/// Through a map that is a pure shift (`Map::held_at` around a transient) the grains read
/// whole frames one hop apart, so the hit is the source's, moved, sample for sample.
pub(super) fn grains(
    channels: &[&[f32]],
    rate: f32,
    map: &Map,
    len: usize,
    ratio: f64,
) -> Vec<Vec<f32>> {
    let g = ((0.0053 * rate) as usize)
        .next_power_of_two()
        .clamp(64, 1_024);
    grains_of(channels, map, len, ratio, g)
}

/// [`grains`] with grains `g` frames long (a power of two).
pub(super) fn grains_of(
    channels: &[&[f32]],
    map: &Map,
    len: usize,
    ratio: f64,
    g: usize,
) -> Vec<Vec<f32>> {
    let hop = g / 4;
    let win = hann(g);
    let ola: f32 = (0..4).map(|j| win[j * hop]).sum();
    let half = (g / 2) as i64;
    // Transposed up, a grain is read faster than recorded: band-limited (no alias).
    let reach = (g as f64) * ratio.max(1.0);
    let (a, b) = (map.source(0.0), map.source(len as f64));
    let fast: Vec<Option<super::read::Fast>> = channels
        .iter()
        .map(|x| {
            (ratio > 1.0)
                .then(|| super::read::Fast::new(x, a.min(b) - reach, a.max(b) + reach, ratio))
        })
        .collect();
    let mut out: Vec<Vec<f32>> = channels.iter().map(|_| vec![0.0; len]).collect();
    let mut j = -(half / hop as i64);
    loop {
        let c = j * hop as i64;
        if c - half >= len as i64 {
            break;
        }
        let at = map.source(c as f64);
        let start = at.round() as i64;
        for (ci, (o, x)) in out.iter_mut().zip(channels).enumerate() {
            for (k, w) in win.iter().enumerate() {
                let t = c - half + k as i64;
                if t < 0 || t >= len as i64 {
                    continue;
                }
                let d = k as i64 - half;
                let pos = start as f64 + d as f64 * ratio;
                let v = if ratio == 1.0 {
                    get(x, start + d)
                } else if let Some(f) = &fast[ci] {
                    f.read(pos, ratio)
                } else {
                    crate::psola::read_at(x, pos)
                };
                o[t as usize] += v * w / ola;
            }
        }
        j += 1;
    }
    out
}
