//! Speculum's ear (round 34, ADR 0083): its loudness is the house meter's to the bit, its
//! spectrum puts a sine in its band at its level, its stereo numbers read what a pair is,
//! and how the stream is cut into blocks changes nothing.

use super::ear::*;
use crate::math;
use crate::terminus::loudness::LoudnessMeter;

const SR: f32 = 48_000.0;

fn sine(hz: f32, db: f32, seconds: f32) -> Vec<f32> {
    let a = math::powf(10f32, db / 20.0);
    let n = (seconds * SR) as usize;
    (0..n)
        .map(|i| a * math::sin(2.0 * std::f32::consts::PI * hz * i as f32 / SR))
        .collect()
}

/// A small deterministic noise (xorshift), −1…1.
fn noise(seed: u32, n: usize, gain: f32) -> Vec<f32> {
    let mut x = seed.max(1);
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            gain * ((x as f32 / u32::MAX as f32) * 2.0 - 1.0)
        })
        .collect()
}

fn hear(l: &[f32], r: &[f32], block: usize) -> Ear {
    let mut e = Ear::new(SR, l.len() as f64 / SR as f64);
    for (a, b) in l.chunks(block).zip(r.chunks(block)) {
        e.push(a, b);
    }
    e
}

#[test]
fn its_loudness_and_true_peak_are_the_house_meters_to_the_bit() {
    let l = noise(7, 480_000, 0.3);
    let r: Vec<f32> = sine(440.0, -12.0, 10.0)
        .iter()
        .zip(&l)
        .map(|(a, b)| a + 0.5 * b)
        .collect();
    let mut m = LoudnessMeter::new(SR);
    m.process(&l, &r);
    let e = hear(&l, &r, 512);
    let w = e.whole();
    assert_eq!(w.integrated_lufs, m.integrated());
    assert_eq!(w.range_lu, m.range());
    assert_eq!(w.true_peak_dbtp, m.true_peak_db());
    assert_eq!(w.short_term_max_lufs, Some(m.columns.max_short_term));
}

#[test]
fn how_the_stream_is_cut_into_blocks_changes_no_reading() {
    let l = noise(3, 200_000, 0.5);
    let r = noise(9, 200_000, 0.4);
    let a = hear(&l, &r, 4_096);
    for block in [1, 7, 333, 8_192] {
        let b = hear(&l, &r, block);
        assert_eq!(a.columns(), b.columns(), "block {block}");
        assert_eq!(a.spectra(), b.spectra(), "block {block}");
        assert_eq!(a.whole(), b.whole(), "block {block}");
    }
}

#[test]
fn a_sine_reads_its_level_in_its_band_and_nothing_far_from_it() {
    let s = sine(1_000.0, -20.0, 4.0);
    let w = hear(&s, &s, 1_024).whole();
    // Mean square of a −20 dBFS sine is −23 dB; the 1 kHz octave holds it.
    assert!((w.octaves_db[5] - -23.01).abs() < 0.3, "{:?}", w.octaves_db);
    for (o, v) in w.octaves_db.iter().enumerate() {
        if (o as i32 - 5).abs() >= 2 {
            assert!(*v < -23.0 - 50.0, "octave {o}: {v}");
        }
    }
    let loudest = (0..THIRDS)
        .max_by(|a, b| w.thirds_db[*a].total_cmp(&w.thirds_db[*b]))
        .unwrap();
    assert_eq!(loudest, 17, "1 kHz is the 18th third");
    // Mono: no side at all, nothing lost folding down, correlation one.
    assert_eq!(w.width_db, f32::NEG_INFINITY);
    // (One FFT carries mid and side; what the symmetry leaves of the other is rounding.)
    assert!(w.side_octaves_db.iter().all(|v| *v < -23.0 - 90.0));
    assert_eq!(w.mono_loss_lu, Some(0.0));
    assert!((w.correlation.unwrap() - 1.0).abs() < 1e-6);
    // A −20 dBFS sine on both channels: about −20 LUFS at 1 kHz, RMS −23, crest 3 dB.
    assert!((w.integrated_lufs.unwrap() - -20.0).abs() < 0.2, "{w:?}");
    assert!((w.rms_dbfs - -23.01).abs() < 0.01);
    assert!((w.crest_db - 3.01).abs() < 0.02);
}

#[test]
fn a_pair_in_opposite_phase_reads_minus_one_and_folds_to_nothing() {
    let s = sine(220.0, -10.0, 3.0);
    let inv: Vec<f32> = s.iter().map(|x| -x).collect();
    let w = hear(&s, &inv, 1_000).whole();
    assert!((w.correlation.unwrap() + 1.0).abs() < 1e-6);
    assert!(w.correlation_min.unwrap() < -0.999);
    assert_eq!(w.mono_loss_lu, None, "the fold-down is silence");
    assert_eq!(w.width_db, f32::INFINITY, "all side, no mid");
    assert!(w
        .octaves_db
        .iter()
        .all(|v| *v < w.side_octaves_db[3] - 90.0));
}

#[test]
fn independent_channels_are_wide_and_lose_three_db_in_mono() {
    let l = noise(11, 240_000, 0.3);
    let r = noise(12, 240_000, 0.3);
    let w = hear(&l, &r, 512).whole();
    assert!(w.correlation.unwrap().abs() < 0.02, "{:?}", w.correlation);
    assert!(w.width_db.abs() < 0.2, "{}", w.width_db);
    assert!(
        (w.mono_loss_lu.unwrap() + 3.01).abs() < 0.2,
        "{:?}",
        w.mono_loss_lu
    );
}

#[test]
fn a_range_reads_its_own_part_of_the_stream() {
    let mut s = sine(500.0, -20.0, 5.0);
    s.extend(sine(500.0, -30.0, 5.0));
    let e = hear(&s, &s, 777);
    let half = (5.0 * SR) as u64;
    let a = e.reading(0, half);
    let b = e.reading(half, 2 * half);
    assert!((a.integrated_lufs.unwrap() - b.integrated_lufs.unwrap() - 10.0).abs() < 0.1);
    assert!((a.seconds - 5.0).abs() < 0.011 && (b.seconds - 5.0).abs() < 0.011);
    assert!((a.octaves_db[4] - b.octaves_db[4] - 10.0).abs() < 0.3);
    // Shorter than a spectrum frame: the frame nearest its middle stands for it.
    let c = e.reading(half + 100, half + 200);
    assert!(c.octaves_db[4].is_finite());
}

#[test]
fn a_click_is_sharper_than_the_music_around_it() {
    let mut s = sine(200.0, -12.0, 2.0);
    let at = s.len() / 2;
    s[at] += 0.4;
    let e = hear(&s, &s, 256);
    let radius = (0.03 * SR) as u64;
    let click = e.sharpness_near(at as u64, radius);
    let calm = e.sharpness_near((at / 2) as u64, radius);
    assert!(click > 20.0 * calm, "click {click}, calm {calm}");
}

#[test]
fn what_is_not_a_number_is_heard_as_silence() {
    let mut l = sine(300.0, -6.0, 1.0);
    l[1_000] = f32::NAN;
    l[2_000] = f32::INFINITY;
    let w = hear(&l, &l, 512).whole();
    assert!(w.integrated_lufs.unwrap().is_finite());
    assert!(w.true_peak_dbtp < 0.0);
}

/// The lowest correlation counts whole 400 ms windows only: a scrap of 100 ms at the end
/// is no window (review 1), and a range shorter than one window is read whole.
#[test]
fn the_lowest_correlation_ignores_a_last_scrap_shorter_than_a_window() {
    let s = sine(330.0, -12.0, 0.9);
    let cut = (0.8 * SR) as usize;
    let r: Vec<f32> = s
        .iter()
        .enumerate()
        .map(|(i, x)| if i < cut { *x } else { -*x })
        .collect();
    let w = hear(&s, &r, 512).whole();
    assert!(w.correlation_min.unwrap() > 0.99, "{:?}", w.correlation_min);
    let short = hear(&s[..4_800], &r[..4_800], 512).whole();
    assert!(short.correlation_min.unwrap() > 0.99);
}
