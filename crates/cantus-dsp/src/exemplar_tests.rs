//! Exemplar's DSP tests: the resampler (unity, aliasing, passband, pitch), loops, the zone
//! engine (crossfade laws, round robin, release samples, chokes) and the house rules.

use super::play::Playhead;
use super::resample::{mip_chain, MAX_TAPS};
use super::*;
use crate::math;
use std::sync::Arc;

/// A source from interleaved audio, with its mip-maps.
pub fn source_of(data: Vec<f32>, channels: usize, rate: f32, periodic: bool) -> Source {
    let frames = data.len() / channels;
    let mut levels: [Option<Arc<[f32]>>; MAX_LEVELS] = Default::default();
    for (k, l) in mip_chain(&data, channels, MAX_LEVELS - 1, periodic)
        .into_iter()
        .enumerate()
    {
        levels[k + 1] = Some(Arc::from(l));
    }
    levels[0] = Some(Arc::from(data));
    Source {
        levels,
        channels,
        rate,
        frames,
        tune_ct: 0.0,
    }
}

fn sines(freqs: &[f64], amp: f64, rate: f64, secs: f64) -> Vec<f32> {
    (0..(rate * secs) as usize)
        .map(|i| {
            freqs
                .iter()
                .map(|f| amp * math::sin(std::f64::consts::TAU * f * i as f64 / rate))
                .sum::<f64>() as f32
        })
        .collect()
}

/// Read `n` output frames from a playhead at a fixed ratio.
fn read(src: &Source, region: Region, ratio: f64, q: Quality, n: usize) -> Vec<[f32; 2]> {
    let reader = Reader::new(q, 48_000.0);
    let mut p = Playhead::default();
    p.start(src.clone(), region, ratio, &reader, 0);
    let mut coefs = [0.0f32; MAX_TAPS];
    (0..n).map(|_| p.next(ratio, &reader, &mut coefs)).collect()
}

fn whole(frames: usize) -> Region {
    Region {
        end: frames as i64,
        ..Region::default()
    }
}

// ---- a spectrum good to −150 dB ------------------------------------------------------------

fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f64::consts::TAU / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = math::sin_cos(ang * k as f64);
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * c - im[b] * s;
                let ti = re[b] * s + im[b] * c;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term) = (1.0, 1.0);
    for k in 1..100 {
        term *= (x / 2.0) * (x / 2.0) / (k * k) as f64;
        sum += term;
    }
    sum
}

/// Magnitude spectrum in dBFS (a full-scale sine reads 0), Kaiser β = 20 window.
pub fn spectrum_db(x: &[f32]) -> Vec<f64> {
    let n = x.len();
    let w: Vec<f64> = (0..n)
        .map(|i| {
            let r = 2.0 * i as f64 / (n - 1) as f64 - 1.0;
            bessel_i0(20.0 * (1.0 - r * r).max(0.0).sqrt()) / bessel_i0(20.0)
        })
        .collect();
    let gain: f64 = w.iter().sum::<f64>() / 2.0;
    let mut re: Vec<f64> = x.iter().zip(&w).map(|(a, b)| *a as f64 * b).collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    (0..n / 2)
        .map(|k| 20.0 * math::log10((math::hypot(re[k], im[k]) / gain).max(1e-15)))
        .collect()
}

/// The loudest line in `spec` more than `guard` bins from every expected frequency.
fn worst_spur(spec: &[f64], expected: &[f64], fs: f64, guard: usize) -> (f64, f64) {
    let n = spec.len() * 2;
    let bin = |f: f64| (f * n as f64 / fs).round() as i64;
    let mut worst = (-300.0, 0.0);
    for (k, &db) in spec.iter().enumerate().skip(guard) {
        if expected
            .iter()
            .any(|f| (k as i64 - bin(*f)).unsigned_abs() as usize <= guard)
        {
            continue;
        }
        if db > worst.0 {
            worst = (db, k as f64 * fs / n as f64);
        }
    }
    worst
}

/// Largest alias (dBFS) when the four-sine test signal is played `st` semitones up.
pub fn alias_floor(q: Quality, st: f64) -> (f64, f64) {
    let rate = 44_100.0;
    let freqs = [5_000.0, 10_000.0, 15_000.0, 19_000.0];
    let src = source_of(sines(&freqs, 0.2, rate, 8.0), 1, rate as f32, false);
    let ratio = rate / 48_000.0 * math::exp2(st / 12.0);
    let out = read(&src, whole(src.frames), ratio, q, 4_096 + 32_768);
    let x: Vec<f32> = out[4_096..].iter().map(|v| v[0]).collect();
    let spec = spectrum_db(&x);
    let expected: Vec<f64> = freqs
        .iter()
        .map(|f| f * math::exp2(st / 12.0))
        .filter(|f| *f < 24_000.0)
        .collect();
    worst_spur(&spec, &expected, 48_000.0, 12)
}

#[test]
fn unity_hands_back_the_file_bit_for_bit() {
    let mut s = 0x1234_5678u32;
    let data: Vec<f32> = (0..20_000)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32) * 1.6 - 0.8
        })
        .collect();
    let src = source_of(data.clone(), 2, 48_000.0, false);
    for q in [
        Quality::Draft,
        Quality::Standard,
        Quality::High,
        Quality::Best,
    ] {
        let out = read(&src, whole(src.frames), 1.0, q, src.frames);
        for (i, v) in out.iter().enumerate() {
            assert_eq!(
                v[0].to_bits(),
                data[2 * i].to_bits(),
                "{q:?} frame {i} left"
            );
            assert_eq!(
                v[1].to_bits(),
                data[2 * i + 1].to_bits(),
                "{q:?} frame {i} right"
            );
        }
    }
}

#[test]
fn transposing_up_does_not_alias_at_high_or_best() {
    for (q, limit) in [(Quality::High, -80.0), (Quality::Best, -100.0)] {
        for st in [12.0, 24.0] {
            let (db, at) = alias_floor(q, st);
            assert!(
                db <= limit,
                "{q:?} at +{st} st: a spurious line at {at:.0} Hz reaches {db:.1} dBFS"
            );
        }
    }
}

#[test]
fn draft_aliases_as_documented() {
    // Linear interpolation on the file: the lo-fi tier is honest about what it is.
    let (db, _) = alias_floor(Quality::Draft, 12.0);
    assert!(
        db > -60.0,
        "Draft is expected to alias audibly ({db:.1} dBFS)"
    );
}

#[test]
fn pitch_is_right_to_a_hundredth_of_a_cent_across_the_range() {
    // A 1 kHz sine at 48 kHz, played from −24 to +36 semitones.
    let src = source_of(sines(&[1_000.0], 0.5, 48_000.0, 6.0), 1, 48_000.0, false);
    for st in [-24.0f64, -7.0, -0.5, 0.37, 5.0, 12.0, 19.0, 31.0] {
        let ratio = math::exp2(st / 12.0);
        let n = 1 << 15;
        let out = read(&src, whole(src.frames), ratio, Quality::High, 2_048 + n);
        let x: Vec<f32> = out[2_048..].iter().map(|v| v[0]).collect();
        let spec = spectrum_db(&x);
        let k = spec
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        // Parabolic interpolation of the peak (in dB) for the sub-bin frequency.
        let (a, b, c) = (spec[k - 1], spec[k], spec[k + 1]);
        let p = 0.5 * (a - c) / (a - 2.0 * b + c);
        let f = (k as f64 + p) * 48_000.0 / n as f64;
        let want = 1_000.0 * ratio;
        let cents = 1200.0 * math::log2(f / want);
        assert!(
            cents.abs() < 0.05,
            "{st} st: {f:.3} Hz, {cents:.4} cents off"
        );
    }
}

/// The level of a steady sine in dBFS (a full-scale sine reads 0), from its windowed RMS —
/// free of the scalloping an FFT peak has between bins.
pub fn sine_level_db(x: &[f32]) -> f64 {
    let n = x.len();
    let (mut num, mut den) = (0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64);
        num += math::powi(w * *v as f64, 2);
        den += w * w;
    }
    10.0 * math::log10(2.0 * num / den)
}

#[test]
fn pitching_down_keeps_the_passband_flat() {
    // Sines across the band at −12 and −24 semitones keep their level to ±0.1 dB.
    for st in [-12.0f64, -24.0] {
        for f in [100.0, 2_000.0, 8_000.0, 15_000.0, 17_500.0] {
            let src = source_of(sines(&[f], 0.5, 44_100.0, 4.0), 1, 44_100.0, false);
            let ratio = 44_100.0 / 48_000.0 * math::exp2(st / 12.0);
            let out = read(
                &src,
                whole(src.frames),
                ratio,
                Quality::High,
                8_192 + 16_384,
            );
            let x: Vec<f32> = out[8_192..].iter().map(|v| v[0]).collect();
            let peak = sine_level_db(&x);
            let want = 20.0 * math::log10(0.5f64);
            assert!(
                (peak - want).abs() < 0.1,
                "{f} Hz at {st} st reads {peak:.3} dBFS, want {want:.3}"
            );
        }
    }
}
