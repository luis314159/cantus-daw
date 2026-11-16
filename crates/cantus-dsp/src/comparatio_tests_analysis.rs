//! Each analysis against a signal whose answer is known: pink noise is flat in
//! fractional-octave bands and white noise rises 3.01 dB an octave; a tilt made in the
//! frequency domain is the difference read back; known correlations and widths; the crest
//! of a sine, a square and a pulse train; PLR and PSR of a sine; the histogram of two
//! levels; the match-EQ suggestion fitted by Aequalis.

use super::match_eq::{suggest, MatchOptions};
use super::measure::Analyser;
use super::testkit::*;
use super::tonal::{compare, Curve, Resolution, Tonal, MIN_FRAMES};
use crate::math;

const N: usize = 1 << 20; // 21.8 s

fn curve(l: &[f32], r: &[f32], res: Resolution) -> Curve {
    let mut t = Tonal::new(FS, res);
    for (a, b) in l.chunks(1_000).zip(r.chunks(1_000)) {
        t.push(a, b);
    }
    t.curve()
}

/// The least-squares slope of the curve, dB per octave, and the largest deviation from
/// that line, over `lo … hi` Hz.
fn slope(c: &Curve, lo: f32, hi: f32) -> (f64, f64) {
    let pts: Vec<(f64, f64)> = (0..c.bands.count)
        .filter(|b| (lo..=hi).contains(&c.bands.centre[*b]))
        .map(|b| (math::log2(c.bands.centre[b] as f64), c.db[b] as f64))
        .collect();
    let n = pts.len() as f64;
    let (mx, my) = (
        pts.iter().map(|p| p.0).sum::<f64>() / n,
        pts.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let k = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>()
        / pts.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>();
    let dev = pts
        .iter()
        .map(|p| (p.1 - (my + k * (p.0 - mx))).abs())
        .fold(0f64, f64::max);
    (k, dev)
}

#[test]
fn pink_noise_is_flat_and_white_rises_three_db_an_octave() {
    for res in [Resolution::Third, Resolution::Sixth] {
        let p = curve(&pink(N, 1, 0.1), &pink(N, 2, 0.1), res);
        let (k, dev) = slope(&p, 25.0, 16_000.0);
        eprintln!("{res:?} pink: {k:+.4} dB/oct, off the line by {dev:.3} dB at most");
        assert!(k.abs() < 0.05 && dev < 0.6, "pink {res:?}: {k} {dev}");
        let w = curve(&white(N, 3, 0.1), &white(N, 4, 0.1), res);
        let (k, dev) = slope(&w, 25.0, 16_000.0);
        eprintln!("{res:?} white: {k:+.4} dB/oct (3.0103), off the line by {dev:.3} dB");
        assert!((k - 3.0103).abs() < 0.05 && dev < 0.6, "white {res:?}: {k} {dev}");
    }
}

/// A full-scale sine reads −3.01 dB in its band (mean square ½), and the curve is
/// level-free: 6 dB louder draws the same curve.
#[test]
fn a_sine_reads_its_mean_square_and_the_curve_is_level_free() {
    let s = sine(1_000.0, 0.0, 10.0, 0.0);
    let z = vec![0.0f32; s.len()];
    let c = curve(&s, &z, Resolution::Third);
    let b = (0..c.bands.count)
        .find(|b| (c.bands.centre[*b] - 1_000.0).abs() < 1.0)
        .unwrap();
    eprintln!("a 0 dBFS sine at 1 kHz: {:.3} dB in its band", c.abs_db[b]);
    assert!((c.abs_db[b] as f64 + 3.0103).abs() < 0.05);
    let (l, r) = (pink(N, 5, 0.05), pink(N, 6, 0.05));
    let a = curve(&l, &r, Resolution::Third);
    let loud: (Vec<f32>, Vec<f32>) = (l.iter().map(|v| v * 2.0).collect(), r.iter().map(|v| v * 2.0).collect());
    let b = curve(&loud.0, &loud.1, Resolution::Third);
    for k in 0..a.bands.count {
        assert!((a.db[k] - b.db[k]).abs() < 1e-3, "band {k}");
        assert!((b.abs_db[k] - a.abs_db[k] - 6.0206).abs() < 1e-3);
    }
}

/// A tilt made in the frequency domain (−4 dB under 300 Hz to +5 dB over 5 kHz, smooth)
/// is read back as the difference, every band from 31.5 Hz to 16 kHz within 0.25 dB once
/// the constant the normalisation leaves is taken off; the confidence is high.
#[test]
fn a_known_tilt_is_the_difference() {
    let tilt = |f: f64| 0.5 + 4.5 * math::tanh(1.2 * math::log2(f / 1_200.0));
    let mag = |f: f64| math::powf(10f64, tilt(f) / 20.0) / f.sqrt();
    let mix = curve(&pink(N, 7, 0.05), &pink(N, 8, 0.05), Resolution::Third);
    let rl = shaped(N, 9, 0.05, mag);
    let rr = shaped(N, 10, 0.05, mag);
    let rf = curve(&rl, &rr, Resolution::Third);
    let d = compare(&mix, &rf);
    let bands = d.bands;
    let inside: Vec<usize> = (0..bands.count)
        .filter(|b| (30.0..=16_500.0).contains(&bands.centre[*b]))
        .collect();
    // d = −tilt + c: the constant is the mean of d + tilt.
    let c = inside
        .iter()
        .map(|b| d.db[*b] as f64 + tilt(bands.centre[*b] as f64))
        .sum::<f64>()
        / inside.len() as f64;
    let mut worst = 0f64;
    for &b in &inside {
        let err = d.db[b] as f64 + tilt(bands.centre[b] as f64) - c;
        worst = worst.max(err.abs());
        // A band holds fewer bins the lower it is, so its frames wander more and it earns
        // less confidence in the same time (≈ 0.25 dB of uncertainty at 125 Hz in 22 s).
        let floor = if bands.centre[b] > 400.0 { 0.9 } else { 0.4 };
        assert!(d.confidence[b] > floor, "band {b}: confidence {}", d.confidence[b]);
    }
    eprintln!("a known tilt read back: worst band off by {worst:.3} dB (constant {c:+.2})");
    assert!(worst < 0.25, "{worst}");
    // The match suggestion, all the way (amount 1, room to 12 dB, a light smoothing):
    // its shape is the tilt's, and Aequalis's fit draws it in a few bands.
    let opts = MatchOptions {
        amount: 1.0,
        max_db: 12.0,
        smooth_oct: 0.2,
        lo_hz: 25.0,
        hi_hz: 18_000.0,
    };
    let m = suggest(&d, &opts);
    let pts = m.points();
    let at = |hz: f32| crate::aequalis::fit::interpolate(pts, hz) as f64;
    let mut worst = 0f64;
    for hz in [60.0f32, 150.0, 400.0, 1_000.0, 2_500.0, 6_000.0, 12_000.0] {
        let want = tilt(hz as f64) - tilt(1_000.0);
        let got = at(hz) - at(1_000.0);
        worst = worst.max((got - want).abs());
    }
    eprintln!("the match curve's shape against the tilt: worst {worst:.3} dB; makeup {:+.2} dB", m.makeup_db);
    assert!(worst < 0.5, "{worst}");
    let fit = crate::aequalis::fit::fit(pts, &crate::aequalis::fit::FitOptions::default(), FS);
    eprintln!(
        "Aequalis fits it in {} bands, rms {:.2} dB, worst {:.2} dB",
        fit.bands.len(),
        fit.rms_db,
        fit.max_db
    );
    assert!(fit.rms_db < 0.75, "the fit: {}", fit.rms_db);
    // Half the amount is half the curve; the limit holds.
    let half = suggest(&d, &MatchOptions { amount: 0.5, ..opts });
    let limited = suggest(&d, &MatchOptions { max_db: 2.0, ..opts });
    for b in 0..m.count {
        let (full, h) = (m.points[b].1, half.points[b].1);
        assert!((h - full / 2.0).abs() < 0.05 + full.abs() * 0.02, "band {b}: {h} {full}");
        assert!(limited.points[b].1.abs() <= 2.0);
    }
}

/// Confidence is zero before enough frames, and grows with the measurement.
#[test]
fn confidence_grows_with_the_measurement() {
    let (l, r) = (pink(N, 11, 0.05), pink(N, 12, 0.05));
    let (l2, r2) = (pink(N, 13, 0.05), pink(N, 14, 0.05));
    let mean_conf = |secs: f64| {
        let n = (secs * FS as f64) as usize;
        let a = curve(&l[..n], &r[..n], Resolution::Third);
        let b = curve(&l2[..n], &r2[..n], Resolution::Third);
        let d = compare(&a, &b);
        (0..d.bands.count).map(|k| d.confidence[k]).sum::<f32>() / d.bands.count as f32
    };
    let (short, mid, long) = (mean_conf(1.0), mean_conf(4.0), mean_conf(21.0));
    eprintln!("mean confidence: 1 s {short:.3}, 4 s {mid:.3}, 21 s {long:.3}");
    assert_eq!(short, 0.0, "under {MIN_FRAMES} frames: none");
    assert!(mid < long && long > 0.8, "{mid} {long}");
}

fn analyse(l: &[f32], r: &[f32]) -> super::measure::Readings {
    let mut a = Analyser::new(FS);
    a.process(l, r);
    let z = vec![0.0f32; a.to_step()];
    a.process(&z, &z);
    a.readings()
}

/// Correlation and width: mono, out of phase, unrelated, and a known half; per band, low
/// mono under high out of phase.
#[test]
fn correlation_and_width_read_what_they_are_made_of() {
    let n = 5 * FS as usize;
    let s = noise(n, 21);
    let t = noise(n, 22);
    let u = noise(n, 23);
    let neg: Vec<f32> = s.iter().map(|v| -v).collect();
    let mut cases: Vec<(&str, Vec<f32>, Vec<f32>, f64, f64)> = vec![
        ("mono", s.clone(), s.clone(), 1.0, 0.0),
        ("out of phase", s.clone(), neg, -1.0, 2.0),
        ("unrelated", s.clone(), t.clone(), 0.0, 1.0),
    ];
    // L = s + t, R = s + u (unit variances): ρ = ½, width ½.
    let l: Vec<f32> = s.iter().zip(&t).map(|(a, b)| a + b).collect();
    let r: Vec<f32> = s.iter().zip(&u).map(|(a, b)| a + b).collect();
    cases.push(("half", l, r, 0.5, 0.5));
    for (what, l, r, rho, w) in cases {
        let st = analyse(&l, &r).stereo[super::stereo::WHOLE];
        let (c, wd) = (st.correlation.unwrap(), st.width.unwrap());
        eprintln!("{what}: correlation {c:+.4} (want {rho}), width {wd:.4} (want {w})");
        assert!((c - rho).abs() < 0.02 && (wd - w).abs() < 0.02, "{what}");
    }
    let lo = sine(60.0, -6.0, 5.0, 0.0);
    let hi = sine(9_000.0, -6.0, 5.0, 0.0);
    let l: Vec<f32> = lo.iter().zip(&hi).map(|(a, b)| a + b).collect();
    let r: Vec<f32> = lo.iter().zip(&hi).map(|(a, b)| a - b).collect();
    let st = analyse(&l, &r).stereo;
    let (lows, highs) = (st[super::stereo::LOWS], st[super::stereo::HIGHS]);
    eprintln!(
        "60 Hz mono under 9 kHz out of phase: lows {:+.4}, highs {:+.4}",
        lows.correlation.unwrap(),
        highs.correlation.unwrap()
    );
    assert!(lows.correlation.unwrap() > 0.99 && lows.width.unwrap() < 0.02);
    assert!(highs.correlation.unwrap() < -0.99 && highs.width.unwrap() > 1.98);
    let quiet = analyse(&vec![0.0; 9_600], &vec![0.0; 9_600]).stereo[super::stereo::WHOLE];
    assert_eq!((quiet.correlation, quiet.width), (None, None), "silence reads nothing");
}

/// Crest factor: a sine 3.01 dB, a square 0 dB, one sample in sixteen 12.04 dB — with
/// silence around them not counted. PLR and PSR of a steady 1 kHz sine at −20 dBFS: 0.
#[test]
fn crest_plr_and_psr_of_known_signals() {
    let pad = |mut x: Vec<f32>| {
        let mut v = vec![0.0f32; 48_000];
        v.append(&mut x);
        v.extend(std::iter::repeat_n(0.0f32, 48_000));
        v
    };
    let n = 4 * FS as usize;
    let sq: Vec<f32> = (0..n).map(|i| if (i / 50) % 2 == 0 { 0.5 } else { -0.5 }).collect();
    let pulses: Vec<f32> = (0..n).map(|i| if i % 16 == 0 { 0.8 } else { 0.0 }).collect();
    for (what, x, want) in [
        ("sine", sine(440.0, -6.0, 4.0, 0.0), 3.0103),
        ("square", sq, 0.0),
        ("one in sixteen", pulses, 20.0 * math::log10(4.0f64)),
    ] {
        let x = pad(x);
        let c = analyse(&x, &x).crest_db.unwrap();
        eprintln!("crest of a {what}: {c:.4} dB (want {want:.4})");
        assert!((c - want).abs() < 0.02, "{what}: {c}");
    }
    let x = sine(1_000.0, -20.0, 10.0, 0.0);
    let r = analyse(&x, &x);
    eprintln!(
        "a −20 dBFS sine: PLR {:.3}, PSR {:.3} (mean {:.3})",
        r.plr.unwrap(),
        r.psr.unwrap_or(f64::NAN),
        r.psr_mean.unwrap()
    );
    assert!(r.plr.unwrap().abs() < 0.1 && r.psr_mean.unwrap().abs() < 0.1);
}

/// The short-term histogram of 20 s at −20 and 20 s at −30 (Tech 3342 case 1): two heaps
/// at their levels, and the 3 s of passage between them in between.
#[test]
fn the_histogram_holds_the_two_levels() {
    let x = segments(&[(-20.0, 20.0), (-30.0, 20.0)]);
    let mut a = Analyser::new(FS);
    a.process(&x, &x);
    let h = a.histogram();
    // The two bins either side of each level (a sine at −20 dBFS reads −20.00 ± 0.01).
    let heap = |lufs: f64| {
        let b = (lufs - super::measure::HIST_LO) as usize;
        h[b - 1] + h[b]
    };
    let total: u32 = h.iter().sum();
    let (at20, at30) = (heap(-20.0), heap(-30.0));
    eprintln!("histogram: {total} steps, {at20} at −20, {at30} at −30");
    assert!(at20 >= 170 && at30 >= 160, "{at20} {at30}");
    assert!(total - at20 - at30 <= 32);
}
