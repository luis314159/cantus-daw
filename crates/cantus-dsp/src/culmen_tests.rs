//! Culmen's tests: the null tests first, then what each stage promises.

use super::*;
use crate::math;
#[path = "culmen_tests_stages.rs"]
mod stages;
use std::f32::consts::TAU;

const FS: f32 = 48_000.0;

fn tone(amp: f32, hz: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| amp * math::sin(TAU * hz * i as f32 / FS))
        .collect()
}

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

/// A mix-like signal: a kick-ish low thump, decaying noise bursts, a mid tone, at about
/// −14 dBFS RMS with peaks near −3 dBFS.
fn music(n: usize, seed: u32) -> (Vec<f32>, Vec<f32>) {
    // Band-limited as any recording is (to 0.42·fs): a converter's or a synth's output
    // has no energy at Nyquist, and true peak is defined on such a signal.
    let nz = band_limit(&noise(n, seed));
    let nz2 = band_limit(&noise(n, seed ^ 0x55aa));
    let make = |nz: &[f32], phase: f32| -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / FS;
                let beat = (i % 24_000) as f32 / FS;
                let kick = 0.55 * math::exp(-beat * 18.0) * math::sin(TAU * 52.0 * beat);
                let hat = 0.18 * math::exp(-(((i + 12_000) % 12_000) as f32) / FS * 60.0) * nz[i];
                let pad = 0.12 * math::sin(TAU * 440.0 * t + phase);
                kick + hat + pad
            })
            .collect()
    };
    (make(&nz, 0.0), make(&nz2, 0.7))
}

/// A windowed-sinc low-pass at 0.42·fs (Blackman, 127 taps), centred: no delay.
fn band_limit(x: &[f32]) -> Vec<f32> {
    const HALF: i64 = 63;
    let fc = 0.42; // cycles per sample: 20.2 kHz at 48 kHz
    let taps: Vec<f64> = (-HALF..=HALF)
        .map(|i| {
            let t = i as f64;
            let sinc = if i == 0 {
                2.0 * fc
            } else {
                math::sin(std::f64::consts::TAU * fc * t) / (std::f64::consts::PI * t)
            };
            let w = std::f64::consts::TAU * (i + HALF) as f64 / (2 * HALF) as f64;
            sinc * (0.42 - 0.5 * math::cos(w) + 0.08 * math::cos(2.0 * w))
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

fn run(s: &MaxSettings, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>, usize) {
    let mut c = Culmen::new(FS);
    c.set(s);
    c.reset();
    let (mut a, mut b) = (l.to_vec(), r.to_vec());
    for (x, y) in a.chunks_mut(block).zip(b.chunks_mut(block)) {
        c.process(x, y);
    }
    (a, b, c.latency())
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

#[track_caller]
fn assert_delayed(out: &[f32], input: &[f32], latency: usize, what: &str) {
    for (i, y) in out.iter().enumerate() {
        let x = if i >= latency {
            input[i - latency]
        } else {
            0.0
        };
        assert_eq!(
            y.to_bits(),
            x.to_bits(),
            "{what}: sample {i}: {y} against {x}"
        );
    }
}

/// The settings that make every stage work: three bands each squeezing or lifting, the
/// full band compressing, the drive into the ceiling, the clipper.
pub(crate) fn busy(mode: Mode) -> MaxSettings {
    let mut s = MaxSettings {
        mode,
        drive_db: 9.0,
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
    s.bands[HI].placement = multiplex::Placement::Side;
    s.bands[FULL].thr_db = -18.0;
    s.bands[FULL].ratio = 2.0;
    s.bands[FULL].thr_lo_db = -45.0;
    s.bands[FULL].ratio_lo = 1.5;
    s.limiter.clip = 0.4;
    s.limiter.style = terminus::LimStyle::Punch;
    s
}

// ---- the null tests -------------------------------------------------------------------

/// At every default — and with every band at 1:1 in every crossover mode and at every band
/// count — the output is the input delayed by the latency reported, to the bit.
#[test]
fn a_culmen_with_nothing_to_do_is_an_exact_delay() {
    let n = 30_000;
    let (l, r) = (tone(0.45, 997.0, n), tone(0.3, 61.0, n)); // under the −1 dB ceiling
    let (a, b, lat) = run(&MaxSettings::default(), &l, &r, 512);
    assert_eq!(
        lat,
        terminus::latency_of(&MaxSettings::default().brickwall(), FS)
    );
    assert_delayed(&a, &l, lat, "default, left");
    assert_delayed(&b, &r, lat, "default, right");
    for mode in [Mode::Dynamic, Mode::Split, Mode::Linear] {
        for count in 1..=3 {
            for slope in Slope::ALL {
                let s = MaxSettings {
                    mode,
                    count,
                    slope,
                    ..MaxSettings::default()
                };
                let (a, b, lat) = run(&s, &l, &r, 300);
                let what = format!("{mode:?}, {count} bands, {slope:?}");
                assert_delayed(&a, &l, lat, &what);
                assert_delayed(&b, &r, lat, &what);
            }
        }
    }
}

/// Bands that could act but are not reached (the signal under their upper thresholds and
/// over their lower ones) leave a wire too; and the delta of anything at rest is silence.
#[test]
fn bands_that_are_not_reached_and_the_delta_at_rest_are_exact() {
    let n = 20_000;
    let (l, r) = (tone(0.2, 440.0, n), tone(0.2, 3_000.0, n)); // about −14 dBFS
    let mut s = MaxSettings::default();
    for b in 0..4 {
        s.bands[b].thr_db = -3.0;
        s.bands[b].ratio = 4.0;
        s.bands[b].thr_lo_db = -60.0;
        s.bands[b].ratio_lo = 3.0;
    }
    let (a, b, lat) = run(&s, &l, &r, 256);
    // Release from silence takes the detector a moment to reach the level: compare after.
    let skip = 4_800;
    for (i, (y, z)) in a.iter().zip(&b).enumerate().skip(skip) {
        assert_eq!(y.to_bits(), l[i - lat].to_bits(), "left {i}");
        assert_eq!(z.to_bits(), r[i - lat].to_bits(), "right {i}");
    }
    let s = MaxSettings {
        delta: true,
        ..MaxSettings::default()
    };
    let (a, b, _) = run(&s, &l, &r, 77);
    assert!(
        a.iter().chain(&b).all(|v| *v == 0.0),
        "the delta of a wire is silence"
    );
}

#[test]
fn the_result_does_not_depend_on_the_block_size() {
    let (l, r) = music(40_000, 3);
    for mode in [Mode::Dynamic, Mode::Split, Mode::Linear] {
        let mut s = busy(mode);
        s.dither = 5;
        s.shaping = Shaping::Optimized;
        s.monitor = Monitor::Match;
        s.limiter.dc_filter = true;
        let (ra, rb, _) = run(&s, &l, &r, 4096);
        for block in [1, 7, 64, 1000] {
            let (a, b, _) = run(&s, &l, &r, block);
            assert!(a == ra && b == rb, "{mode:?} at blocks of {block}");
        }
        // The telemetry too: it is read on the grid only.
        let tel_of = |block: usize| {
            let mut c = Culmen::new(FS);
            c.set(&s);
            c.reset();
            let (mut a, mut b) = (l.clone(), r.clone());
            for (x, y) in a.chunks_mut(block).zip(b.chunks_mut(block)) {
                c.process(x, y);
            }
            c.telemetry()
        };
        assert_eq!(tel_of(4096), tel_of(13), "{mode:?}: telemetry");
    }
}

#[test]
fn it_is_deterministic_and_finite_on_nasty_input() {
    let n = 24_000;
    let mut l: Vec<f32> = noise(n, 5).iter().map(|v| v * 40.0).collect();
    let mut r: Vec<f32> = (0..n)
        .map(|i| if (i / 50) % 2 == 0 { 1.0 } else { -1.0 })
        .collect(); // a full-scale square
    for i in 12_000..16_000 {
        l[i] = 1e-30;
        r[i] = 0.0;
    }
    let mut s = busy(Mode::Split);
    s.drive_db = 30.0;
    let (a1, b1, _) = run(&s, &l, &r, 512);
    let (a2, b2, _) = run(&s, &l, &r, 512);
    assert!(a1 == a2 && b1 == b2, "two runs are identical");
    assert!(a1.iter().chain(&b1).all(|v| v.is_finite()));
}

#[test]
fn silence_in_is_silence_out_and_no_denormals_after_noise() {
    let n = 48_000 * 6;
    let mut l = noise(n, 9);
    let mut r = noise(n, 10);
    for i in 24_000..n {
        l[i] = 0.0;
        r[i] = 0.0;
    }
    for mode in [Mode::Dynamic, Mode::Split, Mode::Linear] {
        let mut s = busy(mode);
        s.dither = 5; // dither falls silent on digital silence
        let (a, b, _) = run(&s, &l, &r, 512);
        let tail = &a[n - 48_000..];
        assert!(
            tail.iter().chain(&b[n - 48_000..]).all(|v| *v == 0.0),
            "{mode:?}"
        );
        assert!(a.iter().all(|v| *v == 0.0 || v.is_normal()), "no denormals");
    }
}
