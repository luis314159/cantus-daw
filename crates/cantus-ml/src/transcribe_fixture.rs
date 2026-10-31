//! A stand-in for Basic Pitch with its contract, for tests (ADR 0086): a window of 43 844
//! samples at 22 050 Hz in; 172 frames of note, onset and contour posteriors out, a frame
//! every 256 samples. Its posteriors come from a filter bank tuned to the 88 semitones
//! (one Goertzel filter a semitone, a 2048-sample Hann window centred on each frame), so
//! sine tones become notes exactly where they sound and the helper's whole path — the
//! windows, their joins, the note creation, the times, the result file — is tested in CI
//! without the model, which is never in the repository.

use super::notes::{CONTOURS, FFT_HOP, NOTES, SAMPLE_RATE, WINDOW_FRAMES};
use cantus_dsp::math;

/// The Goertzel window, samples.
const N: usize = 2048;
/// A tone of this amplitude reaches a note posterior of about 0.86.
const REFERENCE: f64 = 0.05;

/// One window's posteriors: `(note, onset, contour)`, each `WINDOW_FRAMES × bins`.
pub fn run(window: &[f32]) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let hann: Vec<f64> = (0..N)
        .map(|i| 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / N as f64))
        .collect();
    let gain = 2.0 / hann.iter().sum::<f64>();
    let mut note = vec![0.0f32; WINDOW_FRAMES * NOTES];
    for t in 0..WINDOW_FRAMES {
        let centre = (t * FFT_HOP) as isize;
        for k in 0..NOTES {
            let hz = 27.5 * math::exp2(k as f64 / 12.0);
            let w = std::f64::consts::TAU * hz / SAMPLE_RATE as f64;
            let coeff = 2.0 * math::cos(w);
            let (mut s1, mut s2) = (0.0f64, 0.0f64);
            for (i, h) in hann.iter().enumerate() {
                let at = centre + i as isize - (N / 2) as isize;
                let x = if at >= 0 && (at as usize) < window.len() {
                    window[at as usize] as f64
                } else {
                    0.0
                };
                let s = x * h + coeff * s1 - s2;
                s2 = s1;
                s1 = s;
            }
            let power = s1 * s1 + s2 * s2 - coeff * s1 * s2;
            let amp = power.max(0.0).sqrt() * gain;
            note[t * NOTES + k] = (1.0 - math::exp(-2.0 * amp / REFERENCE)) as f32;
        }
    }
    // A semitone's neighbours leak into it (the window is not that narrow at the bottom):
    // only a local peak across frequency is a note, as a trained network would answer.
    let mut peaked = note.clone();
    for t in 0..WINDOW_FRAMES {
        for k in 0..NOTES {
            let v = note[t * NOTES + k];
            let lo = if k > 0 { note[t * NOTES + k - 1] } else { 0.0 };
            let hi = if k + 1 < NOTES {
                note[t * NOTES + k + 1]
            } else {
                0.0
            };
            if v < lo || v < hi {
                peaked[t * NOTES + k] = 0.0;
            }
        }
    }
    let mut onset = vec![0.0f32; WINDOW_FRAMES * NOTES];
    for t in 1..WINDOW_FRAMES {
        for k in 0..NOTES {
            let rise = peaked[t * NOTES + k] - peaked[(t - 1) * NOTES + k];
            onset[t * NOTES + k] = (2.0 * rise).clamp(0.0, 1.0);
        }
    }
    let mut contour = vec![0.0f32; WINDOW_FRAMES * CONTOURS];
    for t in 0..WINDOW_FRAMES {
        for k in 0..NOTES {
            contour[t * CONTOURS + 3 * k] = peaked[t * NOTES + k];
        }
    }
    (peaked, onset, contour)
}
