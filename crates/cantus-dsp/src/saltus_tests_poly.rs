//! Saltus measured, continued: the poly engine on chords (a synthetic triad, and the
//! reference guitar's real strums when the files are there), and the CPU of every mode.

use super::tests::{every_setting, material, run, SR};
use super::tests_measure::{cents, peak_hz, shift_settings, through};
use super::*;
use crate::aequalis::fft::Fft;

/// An A minor triad (A3, C4, E4), eight harmonics each at 1/k, slowly decaying.
fn triad(secs: f32) -> (Vec<f32>, Vec<(f32, f32)>) {
    let n = (secs * SR) as usize;
    let roots = [220.0f32, 261.63, 329.63];
    let mut partials = Vec::new();
    for r in roots {
        for k in 1..=8 {
            partials.push((r * k as f32, 0.12 / k as f32));
        }
    }
    let x = (0..n)
        .map(|i| {
            let t = i as f64 / SR as f64;
            let env = math::exp(-0.4 * t);
            partials
                .iter()
                .map(|&(f, a)| a as f64 * math::sin(2.0 * std::f64::consts::PI * f as f64 * t))
                .sum::<f64>() as f32
                * env as f32
        })
        .collect();
    (x, partials)
}

/// The power spectrum of `y` (Hann, 65 536 points from its start), and its bin width.
fn power(y: &[f32]) -> (Vec<f64>, f64) {
    let n = 65_536usize.min(y.len().next_power_of_two() / 2);
    let fft = Fft::new(n);
    let mut re: Vec<f32> = (0..n)
        .map(|i| {
            let w = 0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / n as f64);
            y[i] * w as f32
        })
        .collect();
    let mut im = vec![0.0f32; n];
    fft.run(&mut re, &mut im, false);
    let p = (0..n / 2)
        .map(|k| (re[k] as f64).powi(2) + (im[k] as f64).powi(2))
        .collect();
    (p, SR as f64 / n as f64)
}

/// The share of the power within ±12 Hz of the wanted partials, as the rest's level
/// against the whole, dB (the inharmonic residue: what the shift added that is no partial).
fn residue_db(y: &[f32], wanted: &[f32]) -> f32 {
    let (p, df) = power(y);
    let total: f64 = p.iter().sum();
    let near: f64 = p
        .iter()
        .enumerate()
        .filter(|(k, _)| {
            let f = *k as f64 * df;
            wanted.iter().any(|w| (f - *w as f64).abs() <= 12.0)
        })
        .map(|(_, v)| v)
        .sum();
    (10.0 * math::log10(((total - near).max(1e-30) / total) as f32)).max(-200.0)
}

/// The poly engine moves every partial of a chord: each where it should be (cents),
/// at its level (dB), and little else (the inharmonic residue) — where the mono engine,
/// hearing no single note, leaves the chord where it was.
#[test]
fn the_poly_engine_moves_a_chord() {
    let (x, partials) = triad(2.0);
    let skip = (0.4 * SR) as usize;
    let input_residue = residue_db(
        &x[skip..],
        &partials.iter().map(|p| p.0).collect::<Vec<_>>(),
    );
    eprintln!("an A minor triad, 24 partials (the input's own residue {input_residue:.1} dB):");
    let mut results = Vec::new();
    for st in [-12.0f32, -7.0, 5.0, 12.0] {
        let r = math::exp2(st / 12.0);
        let y = through(&shift_settings(st, Engine::Poly, false), &x);
        let ys = &y[skip..];
        let (mut worst_c, mut worst_l) = (0.0f32, 0.0f32);
        for &(f, _) in &partials {
            // Partials that coincide with another (the fifth's harmonics on the root's)
            // beat as one: judged in the residue, not one by one.
            if partials
                .iter()
                .any(|&(g, _)| g != f && (g - f).abs() < 30.0)
            {
                continue;
            }
            let got = peak_hz(ys, f * r);
            worst_c = worst_c.max(cents(got, f * r).abs());
            let lin = crate::pitch::testkit::amplitude_db(&x[skip..], SR, f);
            let lout = crate::pitch::testkit::amplitude_db(ys, SR, f * r);
            worst_l = worst_l.max((lout - lin).abs());
        }
        let wanted: Vec<f32> = partials.iter().map(|p| p.0 * r).collect();
        let res = residue_db(ys, &wanted);
        let mono = through(&shift_settings(st, Engine::Mono, false), &x);
        let mono_res = residue_db(&mono[skip..], &wanted);
        eprintln!(
            "  {st:+} st: partials within {worst_c:.2} cents and {worst_l:.2} dB; residue {res:.1} dB (mono engine: {mono_res:.1} dB)"
        );
        results.push((st, worst_c, worst_l, res, mono_res));
    }
    for (st, worst_c, worst_l, res, mono_res) in results {
        assert!(worst_c < 1.0, "{st}: {worst_c} cents");
        assert!(worst_l < 5.0, "{st}: {worst_l} dB");
        assert!(res < -12.0, "{st}: residue {res} dB");
        assert!(
            mono_res > res + 10.0,
            "{st}: the mono engine {mono_res} vs {res}"
        );
    }
}

/// Read a mono WAV (any rate, float or integer).
fn read_wav(path: &std::path::Path) -> Option<(Vec<f32>, u32)> {
    let mut r = hound::WavReader::open(path).ok()?;
    let spec = r.spec();
    let ch = spec.channels as usize;
    let x: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().filter_map(Result::ok).collect(),
        hound::SampleFormat::Int => {
            let k = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .filter_map(Result::ok)
                .map(|v| v as f32 * k)
                .collect()
        }
    };
    Some((x.chunks(ch).map(|c| c[0]).collect(), spec.sample_rate))
}

/// Third-octave band levels (dB) of a long-term spectrum, at `rate`, between 150 Hz
/// and 6 kHz, the bands' frequencies scaled by `scale` (to compare a shifted output with
/// its input's spectrum moved).
fn bands(y: &[f32], rate: f32, scale: f32) -> Vec<f32> {
    let (p, _) = power(y);
    let df = rate as f64 / (2 * p.len()) as f64;
    let mut out = Vec::new();
    let mut lo = 150.0f64;
    while lo * 1.26 < 6_000.0 {
        let hi = lo * 1.26;
        let (a, b) = (lo * scale as f64, hi * scale as f64);
        let e: f64 = p
            .iter()
            .enumerate()
            .filter(|(k, _)| {
                let f = *k as f64 * df;
                f >= a && f < b
            })
            .map(|(_, v)| v)
            .sum();
        out.push((10.0 * math::log10(e.max(1e-30))) as f32);
        lo = hi;
    }
    out
}

/// On the reference guitar's real strums (Carbonthief's DI chords, CC0 — not in the
/// repository: `SALTUS_GUITAR_DIR` or `C:\projects\daw-fx\tmp\guitar-wav`), the poly
/// engine's output spectrum is the input's moved by the interval: third-octave bands
/// within a few dB, formants let move. Skipped when the files are absent.
#[test]
fn the_poly_engine_moves_a_real_guitar_chord() {
    let dir = std::env::var("SALTUS_GUITAR_DIR")
        .unwrap_or_else(|_| r"C:\projects\daw-fx\tmp\guitar-wav".into());
    let path = std::path::Path::new(&dir).join("Sample07 (A Minor).wav");
    let Some((x, rate)) = read_wav(&path) else {
        eprintln!("(no guitar files at {dir}: skipped)");
        return;
    };
    let x: Vec<f32> = x.into_iter().take(rate as usize * 2).collect();
    eprintln!("Carbonthief's A minor strum, {rate} Hz:");
    let mut results = Vec::new();
    for st in [-12.0f32, -5.0, 7.0, 12.0] {
        let r = math::exp2(st / 12.0);
        let s = shift_settings(st, Engine::Poly, false);
        let mut u = Saltus::new(rate as f32);
        u.set(&s);
        let lat = u.latency();
        let mut l = x.clone();
        l.extend(std::iter::repeat_n(0.0, lat));
        let mut rr = l.clone();
        u.process(&mut l, &mut rr);
        let y = &l[lat..];
        let bi = bands(&x, rate as f32, 1.0);
        let bo = bands(y, rate as f32, r);
        // Only the bands the input fills (within 40 dB of its loudest), level-matched.
        let top = bi.iter().fold(f32::NEG_INFINITY, |m, v| m.max(*v));
        let pairs: Vec<(f32, f32)> = bi
            .iter()
            .zip(&bo)
            .filter(|(a, _)| **a > top - 40.0)
            .map(|(a, b)| (*a, *b))
            .collect();
        let n = pairs.len() as f32;
        let mean = pairs.iter().map(|(a, b)| b - a).sum::<f32>() / n;
        let rms = (pairs
            .iter()
            .map(|(a, b)| (b - a - mean).powi(2))
            .sum::<f32>()
            / n)
            .sqrt();
        eprintln!("  {st:+} st: the spectrum moved within {rms:.2} dB RMS over {n} third-octave bands (level {mean:+.2} dB)");
        results.push((st, rms));
    }
    for (st, rms) in results {
        assert!(rms < 1.5, "{st}: {rms} dB");
    }
}

/// The CPU each mode costs: ten seconds of the test material through each, in blocks of
/// 256, as a share of real time. Run alone, with nothing else building:
/// `cargo test -p cantus-dsp --lib saltus::tests_poly::cpu -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement: run alone"]
fn cpu_per_mode() {
    let (mut l, mut r) = material(3);
    while l.len() < 10 * SR as usize {
        let (a, b) = material(l.len() as u64);
        l.extend(a);
        r.extend(b);
    }
    l.truncate(10 * SR as usize);
    r.truncate(10 * SR as usize);
    let mut rows = vec![("defaults (the null)", SaltusSettings::default())];
    rows.extend(every_setting());
    for (name, s) in rows {
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let _ = run(&s, &l, &r, 256);
            best = best.min(t.elapsed().as_secs_f64());
        }
        eprintln!(
            "  {name:<24} {:.2} % of real time ({:.1} ms per second)",
            best * 10.0,
            best * 100.0
        );
    }
}
