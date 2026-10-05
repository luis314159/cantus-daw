//! Terminus's true-peak ceiling on band-processed drums (round 19, fixes-mix): Culmen's builder
//! measured 0.032 dB over the ceiling on drums and hats split into three bands, squeezed,
//! and driven 18 dB into Terminus's clipper. These tests rebuild that material and hold the
//! ceiling by the independent 32× reference.

use super::os::reference_true_peak;
use super::*;
use crate::culmen::{Culmen, MaxSettings, FULL, HI, LO, MID};
use crate::multiplex::{Mode, Placement};
use std::f32::consts::TAU;

const FS: f32 = 48_000.0;

fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.max(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

/// A windowed-sinc low-pass at 0.42·fs (Blackman, 127 taps), centred: band-limited as a
/// recording is.
fn band_limit(x: &[f32]) -> Vec<f32> {
    const HALF: i64 = 63;
    let fc = 0.42;
    let taps: Vec<f64> = (-HALF..=HALF)
        .map(|i| {
            let t = i as f64;
            let sinc = if i == 0 {
                2.0 * fc
            } else {
                (std::f64::consts::TAU * fc * t).sin() / (std::f64::consts::PI * t)
            };
            let w = std::f64::consts::TAU * (i + HALF) as f64 / (2 * HALF) as f64;
            sinc * (0.42 - 0.5 * w.cos() + 0.08 * (2.0 * w).cos())
        })
        .collect();
    (0..x.len() as i64)
        .map(|i| {
            let mut acc = 0f64;
            for (j, h) in taps.iter().enumerate() {
                let k = i + j as i64 - HALF;
                if k >= 0 && (k as usize) < x.len() {
                    acc += h * x[k as usize] as f64;
                }
            }
            acc as f32
        })
        .collect()
}

/// Culmen's test music (a kick, noise-burst hats, a pad), then half a second of silence so
/// the reference does not read the cut at the end as a peak.
fn music(n: usize, seed: u32) -> (Vec<f32>, Vec<f32>) {
    let nz = band_limit(&noise(n, seed));
    let nz2 = band_limit(&noise(n, seed ^ 0x55aa));
    let make = |nz: &[f32], phase: f32| -> Vec<f32> {
        let mut x: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / FS;
                let beat = (i % 24_000) as f32 / FS;
                let kick = 0.55 * (-beat * 18.0).exp() * (TAU * 52.0 * beat).sin();
                let hat = 0.18 * (-(((i + 12_000) % 12_000) as f32) / FS * 60.0).exp() * nz[i];
                let pad = 0.12 * (TAU * 440.0 * t + phase).sin();
                kick + hat + pad
            })
            .collect();
        x.extend(std::iter::repeat_n(0.0, 24_000));
        x
    };
    (make(&nz, 0.0), make(&nz2, 0.7))
}

/// The material Culmen hands its brickwall: three bands squeezed and lifted through a
/// crossover of `mode`, the full band compressed, driven `drive_db` — Culmen with its
/// brickwall left far above the signal, so it is only a delay.
fn band_processed(mode: Mode, drive_db: f32, seed: u32) -> (Vec<f32>, Vec<f32>) {
    let (l, r) = music(96_000, seed);
    let mut s = MaxSettings {
        mode,
        drive_db,
        ..MaxSettings::default()
    };
    s.bands[LO].thr_db = -24.0;
    s.bands[LO].ratio = 3.0;
    s.bands[LO].gain_db = 1.5;
    s.bands[MID].thr_lo_db = -40.0;
    s.bands[MID].ratio_lo = 2.0;
    s.bands[HI].thr_db = -30.0;
    s.bands[HI].ratio = 0.7;
    s.bands[HI].range_db = 4.0;
    s.bands[HI].placement = Placement::Side;
    s.bands[FULL].thr_db = -18.0;
    s.bands[FULL].ratio = 2.0;
    s.bands[FULL].thr_lo_db = -45.0;
    s.bands[FULL].ratio_lo = 1.5;
    s.limiter.ceiling_db = 60.0;
    s.limiter.true_peak = false;
    let mut c = Culmen::new(FS);
    c.set(&s);
    c.reset();
    let (mut a, mut b) = (l, r);
    for (x, y) in a.chunks_mut(512).zip(b.chunks_mut(512)) {
        c.process(x, y);
    }
    (a, b)
}

fn run(s: &LimSettings, l: &[f32], r: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let mut t = Terminus::new(FS);
    t.set(s);
    t.reset();
    let (mut a, mut b) = (l.to_vec(), r.to_vec());
    for (x, y) in a.chunks_mut(512).zip(b.chunks_mut(512)) {
        t.process(x, y, None);
    }
    (a, b)
}

/// The worst true peak re the ceiling over a grid of clipper amounts, styles and drives.
fn worst_over(material: &[(String, Vec<f32>, Vec<f32>)], clips: &[f32]) -> (f32, String) {
    let ceiling = -1.0f32;
    let mut worst = f32::NEG_INFINITY;
    let mut at = String::new();
    for (name, l, r) in material {
        for &clip in clips {
            for (style, look, gain) in [
                (LimStyle::Punch, 1.0, 0.0),
                (LimStyle::Loud, 0.5, 3.0),
                (LimStyle::Clean, 2.0, 0.0),
                (LimStyle::Safe, 5.0, 6.0),
            ] {
                let s = LimSettings {
                    style,
                    lookahead_ms: look,
                    gain_db: gain,
                    ceiling_db: ceiling,
                    true_peak: true,
                    clip,
                    ..LimSettings::default()
                };
                let (a, b) = run(&s, l, r);
                for out in [&a, &b] {
                    let tp = 20.0 * reference_true_peak(out, 0.5).log10() - ceiling;
                    if tp > worst {
                        worst = tp;
                        let det = 20.0 * super::os::true_peak_of(out).log10() - ceiling;
                        at = format!(
                            "{name}, clip {clip}, {style:?}, {look} ms, +{gain} dB; the 4×                              detector reads {det:+.4} dB"
                        );
                    }
                }
            }
        }
    }
    (worst, at)
}

/// The case Culmen's builder found: band-processed drums driven 18 dB, every crossover
/// mode, two seeds, the clipper off and at four amounts. The output's true peak by the 32×
/// reference stays at or under the ceiling. Before round 19's fix (the safety stage aiming
/// 0.05 dB under, the clipper's residual through the interpolator's own prototype) this
/// material read +0.032 dB over with the clipper off and +0.244 dB with it at 100 %; the
/// 4× detector read it 0.08–0.29 dB low (its content in the last tenth below Nyquist).
#[test]
fn band_processed_drums_driven_into_the_clipper_stay_under_the_ceiling() {
    let mut material = Vec::new();
    for mode in [Mode::Split, Mode::Dynamic, Mode::Linear] {
        for seed in [21u32, 5] {
            let (l, r) = band_processed(mode, 18.0, seed);
            material.push((format!("{mode:?} seed {seed}"), l, r));
        }
    }
    let mut worst = (f32::NEG_INFINITY, String::new());
    for clip in [0.0f32, 0.2, 0.4, 0.7, 1.0] {
        let (w, at) = worst_over(&material, &[clip]);
        eprintln!("clip {clip}: worst {w:+.4} dB re ceiling ({at})");
        if w > worst.0 {
            worst = (w, at);
        }
    }
    assert!(
        worst.0 <= 0.0,
        "{:+.4} dB over the ceiling ({})",
        worst.0,
        worst.1
    );
}
