//! Signals for Purus's tests and listening examples: mains hum and buzz (drifting if asked),
//! a bass line on the hum's own frequencies, a synthetic spoken line, a synthetic room with
//! a known decay and direct-to-reverberant ratio, and the measures (a narrow-band level, a
//! projection, the kurtosis ratio). Offline; allocates freely; deterministic.

use crate::aequalis::fft::Fft;
use crate::math;
use crate::pitch::testkit::{self as voice_kit, Vowel};
use std::f64::consts::TAU;

/// A deterministic noise source (xorshift), −1…1.
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u32) -> Self {
        Noise(seed.max(1))
    }
    pub fn next(&mut self) -> f32 {
        let mut s = self.0;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.0 = s;
        (s as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    /// About Gaussian (the sum of four).
    pub fn gauss(&mut self) -> f32 {
        (self.next() + self.next() + self.next() + self.next()) * 0.866
    }
}

/// Mains hum: harmonic `k` (1-based) at `amps[k−1]` (linear), its phase drawn from `seed`,
/// the fundamental following `f0(t)` (Hz; a drift if it moves). `buzz` adds the odd
/// harmonics' sharp edges (a rectifier's: amplitudes falling as 1/k up to 24).
pub fn hum(
    sr: f32,
    seconds: f32,
    amps: &[f32],
    buzz: f32,
    seed: u32,
    f0: impl Fn(f64) -> f64,
) -> Vec<f32> {
    let n = (seconds as f64 * sr as f64) as usize;
    let mut rng = Noise::new(seed);
    let ph: Vec<f64> = (0..24)
        .map(|_| rng.next() as f64 * std::f64::consts::PI)
        .collect();
    let mut theta = 0.0f64;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / sr as f64;
        let mut v = 0.0;
        for (k, a) in amps.iter().enumerate() {
            v += *a as f64 * math::sin((k + 1) as f64 * theta + ph[k]);
        }
        if buzz > 0.0 {
            for k in (3..24).step_by(2) {
                v += buzz as f64 / k as f64 * math::sin(k as f64 * theta + ph[k]);
            }
        }
        out.push(v as f32);
        theta += TAU * f0(t) / sr as f64;
        if theta > TAU * 1000.0 {
            theta -= TAU * 1000.0;
        }
    }
    out
}

/// A bass line: notes `(start s, length s, Hz)` of a plucked tone (fundamental plus two
/// partials, a fast attack and an exponential decay), peak about `peak`.
pub fn bass(sr: f32, seconds: f32, notes: &[(f32, f32, f32)], peak: f32) -> Vec<f32> {
    let n = (seconds * sr) as usize;
    let mut out = vec![0.0f32; n];
    for &(start, len, hz) in notes {
        let s0 = (start * sr) as usize;
        let m = (len * sr) as usize;
        for i in 0..m {
            if s0 + i >= n {
                break;
            }
            let t = i as f64 / sr as f64;
            let att = (t / 0.005).min(1.0);
            let rel = ((len as f64 - t) / 0.02).clamp(0.0, 1.0);
            let env = att * rel * math::exp(-t / 1.2);
            let w = TAU * hz as f64 * t;
            let v = math::sin(w) + 0.35 * math::sin(2.0 * w) + 0.15 * math::sin(3.0 * w);
            out[s0 + i] += (peak as f64 / 1.3 * env * v) as f32;
        }
    }
    out
}

/// A spoken line, synthetic: syllables of the pitch tests' formant voice (vowels A, I, U
/// in turn, a falling-then-rising intonation around `f0`), 140–320 ms each with gaps of
/// 60–260 ms, some opened by a noise burst (a consonant), then a pause of `tail` seconds.
pub fn spoken(sr: f32, f0: f32, syllables: usize, tail: f32, seed: u32) -> Vec<f32> {
    let mut rng = Noise::new(seed);
    let mut out = Vec::new();
    let vowels = [Vowel::A, Vowel::I, Vowel::U, Vowel::A];
    for s in 0..syllables {
        let len = 0.14 + 0.18 * (0.5 + 0.5 * rng.next());
        let gap = 0.06 + 0.2 * (0.5 + 0.5 * rng.next());
        if rng.next() > 0.2 {
            let m = (0.03 * sr) as usize;
            for i in 0..m {
                let e = 1.0 - i as f32 / m as f32;
                out.push(0.08 * e * rng.next());
            }
        }
        let base = f0 * (1.0 + 0.12 * rng.next());
        let v = voice_kit::voice(sr, len, vowels[s % 4], 0.5, move |t| {
            base * (1.0 - 0.15 * (t as f32 / len) + 0.05 * math::sin(9.0 * t as f32))
        });
        // A short fade at both ends (a syllable's own envelope).
        let m = v.len();
        let fade = (0.015 * sr) as usize;
        for (i, x) in v.into_iter().enumerate() {
            let e = (i.min(m - 1 - i) as f32 / fade as f32).min(1.0);
            out.push(x * e);
        }
        out.resize(out.len() + (gap * sr) as usize, 0.0);
    }
    out.resize(out.len() + (tail * sr) as usize, 0.0);
    out
}

/// A synthetic room's impulse response: a direct impulse, `early` discrete reflections
/// between 3 and `keep_ms`, then a Gaussian-noise tail decaying at `t60` (s) with its high
/// frequencies a little faster (a first-order low-pass that closes over the tail), scaled
/// so the direct-to-reverberant ratio is `drr_db` (the direct impulse's energy over
/// everything after it). Returns the response and where the late part starts (samples).
pub fn room(sr: f32, t60: f32, drr_db: f32, keep_ms: f32, seed: u32) -> (Vec<f32>, usize) {
    let mut rng = Noise::new(seed);
    let len = (t60 * 1.5 * sr) as usize;
    let mut h = vec![0.0f32; len];
    let delta = 3.0 * std::f64::consts::LN_10 / t60 as f64;
    let onset = (0.003 * sr) as usize;
    let late = (keep_ms * 0.001 * sr) as usize;
    // Early reflections: six taps, falling, alternating in sign.
    for j in 0..6 {
        let at =
            onset + ((late - onset) as f32 * (j as f32 + 0.5 + 0.3 * rng.next()) / 6.0) as usize;
        let t = at as f64 / sr as f64;
        let s = if j % 2 == 0 { 1.0 } else { -1.0 };
        h[at] += (s * 0.5 * math::exp(-delta * t)) as f32;
    }
    // The diffuse tail, from the first reflection on, growing in for 10 ms.
    let mut lp = 0.0f32;
    for (i, v) in h.iter_mut().enumerate().skip(onset) {
        let t = i as f64 / sr as f64;
        let grow = ((i - onset) as f64 / (0.01 * sr as f64)).min(1.0);
        let k = (0.9 - 0.6 * (t / t60 as f64).min(1.0)) as f32;
        lp += k * (rng.gauss() - lp);
        *v += (0.25 * grow * math::exp(-delta * t)) as f32 * lp;
    }
    // Scale the reverberation to the ratio asked.
    let rev: f64 = h[1..].iter().map(|v| (*v as f64) * (*v as f64)).sum();
    let want = 1.0 / math::powf(10.0f64, drr_db as f64 / 10.0);
    let g = (want / rev).sqrt() as f32;
    for v in h[1..].iter_mut() {
        *v *= g;
    }
    h[0] = 1.0;
    (h, late)
}

/// `x` convolved with `h` (by one transform), as long as `x` plus `h` minus one.
pub fn convolve(x: &[f32], h: &[f32]) -> Vec<f32> {
    let len = x.len() + h.len() - 1;
    let n = len.next_power_of_two();
    let fft = Fft::new(n);
    let (mut ar, mut ai) = (vec![0.0f32; n], vec![0.0f32; n]);
    let (mut br, mut bi) = (vec![0.0f32; n], vec![0.0f32; n]);
    ar[..x.len()].copy_from_slice(x);
    br[..h.len()].copy_from_slice(h);
    fft.run(&mut ar, &mut ai, false);
    fft.run(&mut br, &mut bi, false);
    for i in 0..n {
        let (r, im) = (ar[i] * br[i] - ai[i] * bi[i], ar[i] * bi[i] + ai[i] * br[i]);
        ar[i] = r;
        ai[i] = im;
    }
    fft.run(&mut ar, &mut ai, true);
    ar.truncate(len);
    let k = 1.0 / n as f32;
    ar.iter_mut().for_each(|v| *v *= k);
    ar
}

/// The level at `hz` of `x` (a Hann-windowed single-bin transform over the whole of it),
/// as a sine's amplitude.
pub fn level_at(x: &[f32], sr: f32, hz: f64) -> f64 {
    let n = x.len();
    let (mut re, mut im, mut wsum) = (0.0f64, 0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * math::cos(TAU * i as f64 / n as f64);
        let (s, c) = math::sin_cos(TAU * hz * i as f64 / sr as f64);
        re += w * *v as f64 * c;
        im -= w * *v as f64 * s;
        wsum += w;
    }
    2.0 * (re * re + im * im).sqrt() / wsum
}

pub fn energy(x: &[f32]) -> f64 {
    x.iter().map(|v| (*v as f64) * (*v as f64)).sum()
}

pub fn db(x: f64) -> f64 {
    10.0 * math::log10(x.max(1e-30))
}

/// The least-squares gain of `x` onto `target`: `<x, t> / <t, t>`.
pub fn projection(x: &[f32], target: &[f32]) -> f64 {
    let xt: f64 = x
        .iter()
        .zip(target)
        .map(|(a, b)| *a as f64 * *b as f64)
        .sum();
    xt / energy(target).max(1e-30)
}

/// The kurtosis of the power spectral values of `x` (frames of 1024, hop 256, Hann; bins
/// from `lo_hz` to `hi_hz`, frames whose energy is above −100 dBFS): high when a few
/// isolated bins stand out — musical noise (Uemura et al., *Musical noise generation
/// analysis…*, 2008: the log of its ratio after/before processing is the measure).
pub fn power_kurtosis(x: &[f32], sr: f32, lo_hz: f32, hi_hz: f32) -> f64 {
    let n = 1024;
    let fft = Fft::new(n);
    let lo = (lo_hz * n as f32 / sr) as usize;
    let hi = ((hi_hz * n as f32 / sr) as usize).min(n / 2);
    let mut vals: Vec<f64> = Vec::new();
    let mut k = 0;
    while k + n <= x.len() {
        let frame = &x[k..k + n];
        if energy(frame) / (n as f64) > 1e-10 {
            let mut re: Vec<f32> = frame
                .iter()
                .enumerate()
                .map(|(i, v)| v * (0.5 - 0.5 * math::cos(TAU * i as f64 / n as f64)) as f32)
                .collect();
            let mut im = vec![0.0f32; n];
            fft.run(&mut re, &mut im, false);
            for b in lo..hi {
                vals.push((re[b] * re[b] + im[b] * im[b]) as f64);
            }
        }
        k += 256;
    }
    if vals.is_empty() {
        return 0.0;
    }
    let m = vals.iter().sum::<f64>() / vals.len() as f64;
    let m2 = vals.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / vals.len() as f64;
    let m4 = vals.iter().map(|v| math::powi(v - m, 4)).sum::<f64>() / vals.len() as f64;
    m4 / (m2 * m2).max(1e-300)
}

/// Render `x` (mono, both channels) through a unit-like closure in blocks of `block`.
pub fn render(
    x: &[f32],
    block: usize,
    mut f: impl FnMut(&mut [f32], &mut [f32]),
) -> (Vec<f32>, Vec<f32>) {
    let mut l = x.to_vec();
    let mut r = x.to_vec();
    let mut i = 0;
    while i < l.len() {
        let e = (i + block).min(l.len());
        f(&mut l[i..e], &mut r[i..e]);
        i = e;
    }
    (l, r)
}

/// The hum generator's running phase (radians, unwrapped by whole turns of 1000), sample
/// by sample, for `f0(t)`.
pub fn hum_phase(sr: f32, n: usize, f0: impl Fn(f64) -> f64) -> Vec<f64> {
    let mut theta = 0.0f64;
    (0..n)
        .map(|i| {
            let t = i as f64 / sr as f64;
            let v = theta;
            theta += TAU * f0(t) / sr as f64;
            if theta > TAU * 1000.0 {
                theta -= TAU * 1000.0;
            }
            v
        })
        .collect()
}

/// Harmonic `k`'s amplitude in `x`, following the phase `theta` (a drifting hum's), over a
/// Hann window: what is left of that harmonic, however it moved.
pub fn level_following(x: &[f32], theta: &[f64], k: usize) -> f64 {
    let n = x.len().min(theta.len());
    let (mut re, mut im, mut wsum) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let w = 0.5 - 0.5 * math::cos(TAU * i as f64 / n as f64);
        let (s, c) = math::sin_cos(k as f64 * theta[i]);
        re += w * x[i] as f64 * c;
        im -= w * x[i] as f64 * s;
        wsum += w;
    }
    2.0 * (re * re + im * im).sqrt() / wsum
}
