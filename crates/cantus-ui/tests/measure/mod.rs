//! What the level tests read off a render: integrated loudness (ITU-R BS.1770-4) and true
//! peak (the limiter's own 4× detector, the one the meters show). Shared by `demos.rs` and
//! `instrument_levels.rs`.
#![allow(dead_code)]

use cantus_dsp::terminus::loudness::KWeighting;
use cantus_dsp::terminus::os::true_peak_of;
use cantus_engine::Rendered;

/// Integrated loudness (ITU-R BS.1770-4: K-weighting, 400 ms blocks overlapping by 75 %,
/// the absolute gate at −70 LUFS and the relative one 10 LU under the mean).
pub fn lufs(r: &Rendered) -> f64 {
    lufs_of(&r.samples, r.channels as usize, r.sample_rate as f32)
}

/// [`lufs`] of interleaved samples.
pub fn lufs_of(samples: &[f32], ch: usize, sr: f32) -> f64 {
    let frames = samples.len() / ch;
    let mut k: Vec<KWeighting> = (0..ch).map(|_| KWeighting::new(sr)).collect();
    let mut energy = vec![0f64; frames];
    for (f, slot) in energy.iter_mut().enumerate() {
        let mut e = 0.0;
        for (c, kw) in k.iter_mut().enumerate() {
            let y = kw.tick(samples[f * ch + c] as f64);
            e += y * y;
        }
        *slot = e;
    }
    let block = (0.4 * sr as f64) as usize;
    let hop = block / 4;
    let mut blocks = Vec::new();
    let mut at = 0;
    while at + block <= frames {
        let mean = energy[at..at + block].iter().sum::<f64>() / block as f64;
        blocks.push(mean);
        at += hop;
    }
    let l = |e: f64| -0.691 + 10.0 * e.max(1e-30).log10();
    let gated: Vec<f64> = blocks.iter().copied().filter(|e| l(*e) > -70.0).collect();
    if gated.is_empty() {
        return -100.0;
    }
    let mean = gated.iter().sum::<f64>() / gated.len() as f64;
    let rel = l(mean) - 10.0;
    let kept: Vec<f64> = gated.into_iter().filter(|e| l(*e) > rel).collect();
    l(kept.iter().sum::<f64>() / kept.len().max(1) as f64)
}

/// The true peak of a render, dBTP: the louder channel, by the 4× detector the meters use
/// (BS.1770-4 Annex 2).
pub fn true_peak_db(r: &Rendered) -> f32 {
    true_peak_of_db(&r.samples, r.channels as usize)
}

/// [`true_peak_db`] of interleaved samples.
pub fn true_peak_of_db(samples: &[f32], ch: usize) -> f32 {
    let mut tp = 0f32;
    for c in 0..ch {
        let x: Vec<f32> = samples.iter().skip(c).step_by(ch).copied().collect();
        tp = tp.max(true_peak_of(&x));
    }
    db(tp)
}

pub fn db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

pub fn rms(r: &Rendered) -> f32 {
    let n = r.samples.len().max(1) as f64;
    ((r.samples.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / n).sqrt()) as f32
}
