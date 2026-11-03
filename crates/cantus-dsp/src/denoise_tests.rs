use super::*;
use crate::math;

const RATE: u32 = 48_000;

/// Deterministic white noise in -1..1.
fn noise(seed: u32, n: usize, amp: f32) -> Vec<f32> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((s >> 8) as f32 / (1u32 << 23) as f32 - 1.0) * amp
        })
        .collect()
}

fn tone(hz: f32, amp: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| amp * math::sin(std::f32::consts::TAU * hz * i as f32 / RATE as f32))
        .collect()
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len().max(1) as f64).sqrt() as f32
}

fn db(x: f32) -> f32 {
    20.0 * math::log10(x.max(1e-12))
}

#[test]
fn an_untouched_stft_comes_back_to_float_rounding() {
    let x: Vec<f32> = tone(440.0, 0.5, 20_000)
        .iter()
        .zip(noise(1, 20_000, 0.2))
        .map(|(a, b)| a + b)
        .collect();
    let y = Stft::new().run(&x, |_, _, _| {});
    assert_eq!(y.len(), x.len());
    let worst = x
        .iter()
        .zip(&y)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(worst < 2e-6, "{worst}");
}

#[test]
fn amount_zero_returns_the_take_bit_for_bit() {
    let x: Vec<f32> = noise(3, 30_000, 0.3);
    let profile = NoiseProfile::learn(&[x[..10_000].to_vec()], RATE).unwrap();
    let p = Params {
        amount: 0.0,
        ..Params::default()
    };
    let y = denoise(std::slice::from_ref(&x), &profile, &p).unwrap();
    assert!(x.iter().zip(&y[0]).all(|(a, b)| a.to_bits() == b.to_bits()));
}

#[test]
fn a_region_at_zero_db_returns_the_take_bit_for_bit() {
    let x = noise(4, 20_000, 0.3);
    let r = Region {
        start: 1000,
        end: 9000,
        lo_hz: 500.0,
        hi_hz: 4000.0,
    };
    let y = attenuate_region(&x, RATE, &r, 0.0);
    assert!(x.iter().zip(&y).all(|(a, b)| a.to_bits() == b.to_bits()));
}

/// A second of hiss, then a "voice" of harmonics in hiss, then a second of hiss.
fn take() -> (Vec<f32>, usize, usize) {
    let hiss = 48_000;
    let voice = 48_000;
    let n = hiss * 2 + voice;
    let mut x = noise(7, n, 0.02);
    for (i, v) in x[hiss..hiss + voice].iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        // A pitch that glides, three harmonics, a syllable-like envelope.
        let f = 180.0 + 20.0 * t;
        let env = 0.5 + 0.5 * math::sin(std::f32::consts::TAU * 3.0 * t);
        for h in 1..=3 {
            *v += env * 0.25 / h as f32 * math::sin(std::f32::consts::TAU * f * h as f32 * t);
        }
    }
    (x, hiss, voice)
}

#[test]
fn a_cleaned_take_is_quieter_in_the_noise_and_the_voice_is_kept() {
    let (x, hiss, voice) = take();
    let profile = NoiseProfile::learn(&[x[..hiss].to_vec()], RATE).unwrap();
    let y = denoise(std::slice::from_ref(&x), &profile, &Params::default()).unwrap();
    let y = &y[0];
    // Noise-only frames, away from the voice (the tail's edge excluded).
    let quiet = |s: &[f32]| rms(&s[2000..hiss - 2000]);
    let drop = db(quiet(&x)) - db(quiet(y));
    assert!(drop >= 12.0, "noise only {drop:.1} dB quieter");
    let tail = hiss + voice + 6000;
    let drop_tail = db(rms(&x[tail..x.len() - 2000])) - db(rms(&y[tail..y.len() - 2000]));
    assert!(drop_tail >= 12.0, "tail only {drop_tail:.1} dB quieter");
    // The voice, in its loud middle: within 1 dB.
    let v = hiss + 4000..hiss + voice - 4000;
    let change = (db(rms(&y[v.clone()])) - db(rms(&x[v]))).abs();
    assert!(change < 1.0, "the voice moved {change:.2} dB");
}

#[test]
fn more_amount_takes_more_noise() {
    let (x, hiss, _) = take();
    let profile = NoiseProfile::learn(&[x[..hiss].to_vec()], RATE).unwrap();
    let level = |amount: f32| {
        let p = Params {
            amount,
            ..Params::default()
        };
        let y = denoise(std::slice::from_ref(&x), &profile, &p).unwrap();
        rms(&y[0][2000..hiss - 2000])
    };
    let (a, b, c) = (level(0.25), level(0.5), level(1.0));
    assert!(a > b && b > c, "{a} {b} {c}");
}

#[test]
fn the_same_input_gives_the_same_bits() {
    let (x, hiss, _) = take();
    let profile = NoiseProfile::learn(&[x[..hiss].to_vec()], RATE).unwrap();
    let a = denoise(std::slice::from_ref(&x), &profile, &Params::default()).unwrap();
    let b = denoise(&[x], &profile, &Params::default()).unwrap();
    assert!(a[0]
        .iter()
        .zip(&b[0])
        .all(|(p, q)| p.to_bits() == q.to_bits()));
}

#[test]
fn a_profile_too_short_or_for_other_channels_is_refused() {
    assert_eq!(
        NoiseProfile::learn(&[vec![0.0; 2500]], RATE),
        Err(DenoiseError::ProfileTooShort)
    );
    let p = NoiseProfile::learn(&[noise(1, 20_000, 0.1)], RATE).unwrap();
    assert_eq!(
        denoise(&[vec![0.0; 100], vec![0.0; 100]], &p, &Params::default()),
        Err(DenoiseError::ProfileMismatch)
    );
    let bad = Params {
        amount: 2.0,
        ..Params::default()
    };
    assert!(denoise(&[vec![0.0; 100]], &p, &bad).is_err());
}

#[test]
fn a_region_turns_down_its_tone_in_its_time_and_leaves_the_rest() {
    let n = 48_000 * 2;
    let x: Vec<f32> = tone(1000.0, 0.4, n)
        .iter()
        .zip(tone(3000.0, 0.4, n))
        .map(|(a, b)| a + b)
        .collect();
    let r = Region {
        start: 24_000,
        end: 72_000,
        lo_hz: 700.0,
        hi_hz: 1400.0,
    };
    let y = attenuate_region(&x, RATE, &r, -24.0);
    // Inside the region's time, 1 kHz is gone and 3 kHz stays: compare band energy through
    // single-bin projections.
    let proj = |s: &[f32], hz: f32| -> f32 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, v) in s.iter().enumerate() {
            let a = std::f64::consts::TAU * hz as f64 * i as f64 / RATE as f64;
            re += *v as f64 * math::cos(a);
            im += *v as f64 * math::sin(a);
        }
        (2.0 * (re * re + im * im).sqrt() / s.len() as f64) as f32
    };
    let inside = 30_000..66_000;
    let low = db(proj(&y[inside.clone()], 1000.0)) - db(proj(&x[inside.clone()], 1000.0));
    let high = db(proj(&y[inside.clone()], 3000.0)) - db(proj(&x[inside.clone()], 3000.0));
    assert!(low < -20.0, "1 kHz moved {low:.1} dB");
    assert!(high.abs() < 0.3, "3 kHz moved {high:.2} dB");
    // Before the region in time, nothing moved.
    let before = 2000..18_000;
    let early = db(proj(&y[before.clone()], 1000.0)) - db(proj(&x[before], 1000.0));
    assert!(early.abs() < 0.3, "before it, 1 kHz moved {early:.2} dB");
}

#[test]
fn the_spectrogram_reads_a_full_scale_sine_near_zero_db_in_its_bin() {
    let x = tone(1500.0, 1.0, 20_000);
    let s = stft::Spectrogram::of(&x, RATE, 1024);
    let bin = s.bin_of(1500.0);
    let peak = s.db_at(s.frames / 2, bin);
    assert!(peak > -2.0 && peak <= 0.5, "{peak}");
    assert!(s.db_at(s.frames / 2, s.bin_of(8000.0)) < -60.0);
    assert!((s.hz_of(bin) - 1500.0).abs() < 25.0);
}

// --- click repair -----------------------------------------------------------------------

/// A voice-like signal: a gliding fundamental and five harmonics, plus a little hiss.
fn music(n: usize) -> Vec<f32> {
    let hiss = noise(11, n, 0.002);
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = 200.0 + 30.0 * math::sin(std::f32::consts::TAU * 0.7 * t);
            let mut v = 0.0;
            for h in 1..=5 {
                v += 0.15 / h as f32 * math::sin(std::f32::consts::TAU * f * h as f32 * t);
            }
            v + hiss[i]
        })
        .collect()
}

const CLICKS: [usize; 4] = [12_000, 30_500, 51_234, 70_001];

fn clicked(clean: &[f32]) -> Vec<f32> {
    let mut x = clean.to_vec();
    for (k, c) in CLICKS.iter().enumerate() {
        for j in 0..3 {
            x[c + j] += if k % 2 == 0 { 0.6 } else { -0.5 } * (1.0 - j as f32 / 3.0);
        }
    }
    x
}

#[test]
fn clicks_are_found_and_nothing_else_is() {
    let clean = music(90_000);
    let found = declick::detect(&clicked(&clean), 0, 90_000, 0.5);
    for c in CLICKS {
        assert!(
            found.iter().any(|(a, b)| *a <= c && *b >= c + 3),
            "click at {c} not found in {found:?}"
        );
    }
    assert_eq!(found.len(), CLICKS.len(), "{found:?}");
    assert!(
        declick::detect(&clean, 0, 90_000, 0.5).is_empty(),
        "a clean take has no clicks"
    );
}

#[test]
fn a_repaired_click_is_close_to_the_audio_it_hid() {
    let clean = music(90_000);
    let mut x = clicked(&clean);
    let n = declick::declick(&mut x, 0, 90_000, 0.5);
    assert_eq!(n, CLICKS.len());
    for c in CLICKS {
        let worst = (c - 40..c + 40)
            .map(|i| (x[i] - clean[i]).abs())
            .fold(0.0, f32::max);
        assert!(worst < 0.03, "click at {c}: {worst}");
    }
    // Far from the clicks, nothing moved.
    assert!(x[..11_900]
        .iter()
        .zip(&clean)
        .all(|(a, b)| a.to_bits() == b.to_bits()));
}

#[test]
fn a_gap_without_audio_around_it_is_refused() {
    let mut x = music(2_000);
    assert!(declick::interpolate(&mut x, 10, 20).is_err());
    assert!(declick::interpolate(&mut x, 500, 500 + declick::MAX_GAP + 1).is_err());
}
