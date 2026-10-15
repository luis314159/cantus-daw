//! Measuring Chorda's sound in tests: a partial's frequency (to a hundredth of a cent) and
//! its decay, read off a rendered signal — never from the filters' formulas.

use crate::aequalis::fft::Fft;
use crate::math;

/// Hann-windowed spectrum of `x[start..start + n]` (`n` a power of two): magnitude and phase.
fn spectrum(fft: &Fft, x: &[f64], start: usize, n: usize) -> (Vec<f32>, Vec<f32>) {
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    for i in 0..n {
        let w = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64);
        re[i] = (x.get(start + i).copied().unwrap_or(0.0) * w) as f32;
    }
    fft.run(&mut re, &mut im, false);
    let mag = re
        .iter()
        .zip(&im)
        .map(|(r, i)| (r * r + i * i).sqrt())
        .collect();
    let ph = re
        .iter()
        .zip(&im)
        .map(|(r, i)| math::atan2(*i, *r))
        .collect();
    (mag, ph)
}

/// The frequency (Hz) of the strongest partial within `±tol_hz` of `near_hz`, from the
/// phase advance of its bin between two frames `n/8` apart (`n` a power of two).
pub fn partial_hz(x: &[f64], fs: f64, near_hz: f64, tol_hz: f64, start: usize, n: usize) -> f64 {
    let fft = Fft::new(n);
    let hop = n / 8;
    let (m1, p1) = spectrum(&fft, x, start, n);
    let (_, p2) = spectrum(&fft, x, start + hop, n);
    let bin_hz = fs / n as f64;
    let lo = (((near_hz - tol_hz) / bin_hz).floor().max(1.0)) as usize;
    let hi = (((near_hz + tol_hz) / bin_hz).ceil() as usize).min(n / 2 - 1);
    let k = (lo..=hi)
        .max_by(|a, b| m1[*a].total_cmp(&m1[*b]))
        .unwrap_or(lo);
    let expected = std::f64::consts::TAU * k as f64 * hop as f64 / n as f64;
    let mut dev = (p2[k] - p1[k]) as f64 - expected;
    dev -= std::f64::consts::TAU * (dev / std::f64::consts::TAU).round();
    (k as f64 + dev * n as f64 / (std::f64::consts::TAU * hop as f64)) * bin_hz
}

/// A partial's level (dB) frame by frame: `(time s, dB)`, `n`-point frames every `hop`.
pub fn partial_track(x: &[f64], fs: f64, hz: f64, n: usize, hop: usize) -> Vec<(f64, f64)> {
    let fft = Fft::new(n);
    let bin = (hz * n as f64 / fs).round() as usize;
    let mut out = Vec::new();
    let mut start = 0;
    while start + n <= x.len() {
        let (m, _) = spectrum(&fft, x, start, n);
        let peak = (bin.saturating_sub(2)..=(bin + 2).min(n / 2 - 1))
            .map(|i| m[i])
            .fold(0.0f32, f32::max) as f64;
        out.push((
            (start + n / 2) as f64 / fs,
            20.0 * math::log10(peak.max(1e-30)),
        ));
        start += hop;
    }
    out
}

/// A partial's T60 (s): the slope of its level from `from_s` until it has fallen `range_db`
/// below where the fit starts (or the end), by least squares.
pub fn partial_t60(x: &[f64], fs: f64, hz: f64, from_s: f64, range_db: f64) -> f64 {
    let track = partial_track(x, fs, hz, 8192, 2048);
    let start = track.iter().position(|(t, _)| *t >= from_s).unwrap_or(0);
    let top = track[start].1;
    let pts: Vec<(f64, f64)> = track[start..]
        .iter()
        .take_while(|(_, db)| *db > top - range_db)
        .copied()
        .collect();
    let n = pts.len() as f64;
    let (mt, md) = (
        pts.iter().map(|p| p.0).sum::<f64>() / n,
        pts.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let num: f64 = pts.iter().map(|(t, d)| (t - mt) * (d - md)).sum();
    let den: f64 = pts.iter().map(|(t, _)| (t - mt) * (t - mt)).sum();
    -60.0 / (num / den)
}

/// The level (dB) of a partial over the first `seconds` after `start_s`, for picking the
/// partials worth measuring.
pub fn partial_level(x: &[f64], fs: f64, hz: f64, start_s: f64) -> f64 {
    let n = 8192;
    let fft = Fft::new(n);
    let (m, _) = spectrum(&fft, x, (start_s * fs) as usize, n);
    let bin = (hz * n as f64 / fs).round() as usize;
    let peak = (bin.saturating_sub(2)..=(bin + 2).min(n / 2 - 1))
        .map(|i| m[i])
        .fold(0.0f32, f32::max) as f64;
    20.0 * math::log10(peak.max(1e-30))
}
