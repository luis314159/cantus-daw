//! The onset detector's tests (ADR 0061): every hit found, nothing else, each on its
//! sample within a millisecond; a vibrato is not a new note.

use super::*;
use crate::beat::testkit::{click_track, drum_loop};
use crate::math;

/// (found, missed, extra, worst error in samples) of `got` against `truth` within `tol`.
fn score(got: &[f64], truth: &[f64], tol: f64) -> (usize, usize, usize, f64) {
    let mut used = vec![false; got.len()];
    let (mut found, mut worst) = (0, 0.0f64);
    for &t in truth {
        let best = got
            .iter()
            .enumerate()
            .filter(|(i, _)| !used[*i])
            .min_by(|a, b| (a.1 - t).abs().total_cmp(&(b.1 - t).abs()));
        if let Some((i, g)) = best {
            if (g - t).abs() <= tol {
                used[i] = true;
                found += 1;
                worst = worst.max((g - t).abs());
            }
        }
    }
    let extra = used.iter().filter(|u| !**u).count();
    (found, truth.len() - found, extra, worst)
}

#[test]
fn every_click_is_found_on_its_sample() {
    for bpm in [72.0, 120.0, 176.0] {
        let m = click_track(48_000.0, bpm, 4, 0.31);
        let o = detect(&m.audio, m.rate);
        let (found, missed, extra, worst) = score(&o.frames, &m.onsets, 480.0);
        assert_eq!((missed, extra), (0, 0), "{bpm}: {found} found");
        assert!(worst <= 48.0, "{bpm}: {worst} samples off");
    }
}

#[test]
fn a_swung_drum_loop_is_found_hit_by_hit() {
    for (bpm, swing) in [(92.0, 0.0), (97.0, 0.6), (128.0, 0.3)] {
        let m = drum_loop(44_100.0, bpm, 4, swing, 0.2, 3);
        let o = detect(&m.audio, m.rate);
        let (found, missed, extra, worst) = score(&o.frames, &m.onsets, 441.0);
        let n = m.onsets.len();
        assert!(missed * 20 <= n, "{bpm}/{swing}: missed {missed} of {n}");
        assert!(extra * 20 <= n, "{bpm}/{swing}: {extra} extra ({found} found)");
        assert!(worst <= 88.0, "{bpm}/{swing}: {worst} samples off");
    }
}

#[test]
fn a_vibrato_is_not_a_new_note() {
    let sr = 48_000.0f32;
    let mut phase = 0.0f64;
    let x: Vec<f32> = (0..(2.0 * sr) as usize)
        .map(|i| {
            let t = i as f32 / sr;
            let f = 330.0 * math::exp2(0.5 / 12.0 * math::sin(2.0 * std::f32::consts::PI * 5.5 * t));
            phase += 2.0 * std::f64::consts::PI * f as f64 / sr as f64;
            let a = (t / 0.01).min(1.0);
            (0.4 * a as f64 * (math::sin(phase) + 0.3 * math::sin(2.0 * phase))) as f32
        })
        .collect();
    let o = detect(&x, sr);
    assert_eq!(o.frames.len(), 1, "{:?}", o.frames);
    assert!(o.frames[0] < 0.03 * sr as f64);
}

#[test]
fn silence_has_none_and_one_hit_has_one() {
    let sr = 48_000.0f32;
    assert!(detect(&vec![0.0; 48_000], sr).frames.is_empty());
    let mut x = vec![0.0f32; 96_000];
    let mut g = crate::pitch::testkit::Noise::new(4);
    for k in 0..9_600 {
        x[40_000 + k] = 0.5 * g.next() * math::exp(-(k as f32) / 2_000.0);
    }
    let o = detect(&x, sr);
    assert_eq!(o.frames.len(), 1, "{:?}", o.frames);
    assert!((o.frames[0] - 40_000.0).abs() <= 48.0, "{}", o.frames[0]);
}

#[test]
fn detection_is_deterministic() {
    let m = drum_loop(48_000.0, 110.0, 2, 0.2, 0.1, 9);
    assert_eq!(detect(&m.audio, m.rate), detect(&m.audio, m.rate));
}
