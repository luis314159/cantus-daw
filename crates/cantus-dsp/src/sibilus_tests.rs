//! Sibilus's tests: the null tests first (a wire at settings that do nothing, to the bit;
//! the same output at every block size), then what a de-esser is for, in numbers.

use super::detect::Biquad;
use super::*;
use crate::aequalis::{design, Coefs, EqShape, MAX_SECTIONS};
use crate::math;

const SR: f32 = 48_000.0;

fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// A filter chain made from Aequalis's designs (48 dB/octave cuts), for shaping test
/// signals.
fn chain(parts: &[(EqShape, f32)]) -> Vec<Biquad> {
    let mut out = Vec::new();
    for (shape, hz) in parts {
        let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
        let n = design(
            *shape,
            SR,
            *hz,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            48.0,
            &mut c,
        );
        for co in &c[..n] {
            out.push(Biquad::new(*co));
        }
    }
    out
}

/// A synthetic "s": white noise through 5–10 kHz band limits.
fn hiss(n: usize, seed: u32) -> Vec<f32> {
    let mut seed = seed;
    let mut f = chain(&[(EqShape::LowCut, 5_000.0), (EqShape::HighCut, 10_000.0)]);
    (0..n)
        .map(|_| {
            let mut x = noise(&mut seed);
            for b in &mut f {
                x = b.tick(x);
            }
            x
        })
        .collect()
}

/// Bursts of hiss `len_ms` long every `every_ms`, with 3 ms fades, scaled to `amp`.
fn sibilant_bursts(n: usize, len_ms: f32, every_ms: f32, amp: f32) -> Vec<f32> {
    let h = hiss(n, 7);
    let len = (len_ms * 0.001 * SR) as usize;
    let every = (every_ms * 0.001 * SR) as usize;
    let fade = (0.003 * SR) as usize;
    (0..n)
        .map(|i| {
            let p = i % every;
            let g = if p >= len {
                0.0
            } else {
                let a = (p as f32 / fade as f32).min(1.0);
                let b = ((len - p) as f32 / fade as f32).min(1.0);
                a.min(b)
            };
            h[i] * g * amp
        })
        .collect()
}

/// A bright sustained tone: every harmonic of `f0` up to 20 kHz at 1/n — a sawtooth,
/// built without aliasing.
fn bright_tone(n: usize, f0: f32, amp: f32) -> Vec<f32> {
    let harmonics = (20_000.0 / f0) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let mut x = 0.0;
            for h in 1..=harmonics {
                x += math::sin(std::f32::consts::TAU * f0 * h as f32 * t) / h as f32;
            }
            x * amp
        })
        .collect()
}

fn sine(n: usize, hz: f32, amp: f32) -> Vec<f32> {
    (0..n)
        .map(|i| amp * math::sin(std::f32::consts::TAU * hz * i as f32 / SR))
        .collect()
}

fn rms_db(x: &[f32]) -> f32 {
    let ms = x.iter().map(|v| math::powi(*v as f64, 2)).sum::<f64>() / x.len().max(1) as f64;
    (10.0 * math::log10(ms.max(1e-30))) as f32
}

fn run(
    s: &DeessSettings,
    l: &[f32],
    r: &[f32],
    key: Option<(&[f32], &[f32])>,
) -> (Vec<f32>, Vec<f32>) {
    let mut d = Sibilus::new(SR);
    d.set(s);
    let (mut a, mut b) = (l.to_vec(), r.to_vec());
    d.process(&mut a, &mut b, key);
    (a, b)
}

/// Settings for the tests: lookahead 0, wide band — the zero-latency case — unless a test
/// says otherwise.
fn base() -> DeessSettings {
    DeessSettings {
        threshold_db: -40.0,
        range_db: 6.0,
        lo_hz: 4_000.0,
        hi_hz: 12_000.0,
        lookahead_ms: 0.0,
        ..DeessSettings::default()
    }
}

/// The level the detector reads for a signal, dB: the trigger's RMS on its scale (a
/// full-scale sine reads 0).
fn band_level_db(x: &[f32]) -> f32 {
    let s = DeessSettings {
        listen: Listen::Trigger,
        ..base()
    };
    let (t, _) = run(&s, x, x, None);
    rms_db(&t[t.len() / 4..]) + 3.0103
}

/// How much quieter the output is than the input over `[from, to)`, dB (positive = cut),
/// with the latency taken out.
fn cut_db(s: &DeessSettings, x: &[f32], from: usize, to: usize) -> f32 {
    let lat = latency_for(s, SR);
    let mut padded = x.to_vec();
    padded.extend(std::iter::repeat_n(0.0, lat));
    let (y, _) = run(s, &padded, &padded, None);
    rms_db(&x[from..to]) - rms_db(&y[from + lat..to + lat])
}

/// Every mode the de-esser has, for the tests that must hold in all of them.
fn every_mode() -> Vec<DeessSettings> {
    let mut out = Vec::new();
    for detection in Detection::ALL {
        for processing in Processing::ALL {
            for stereo in StereoMode::ALL {
                out.push(DeessSettings {
                    detection: *detection,
                    processing: *processing,
                    stereo: *stereo,
                    ..base()
                });
            }
        }
    }
    out
}

/// Below the threshold (and, separately, at Range 0) the output is the input, delayed by
/// the latency, to the bit — in every mode, at lookahead 0 and 5 ms.
#[test]
fn a_de_esser_that_does_nothing_is_a_wire() {
    let n = 24_000;
    // A quiet phrase: a vowel and a hiss, both far below a 0 dB threshold.
    let tone = bright_tone(n, 180.0, 0.02);
    let s = sibilant_bursts(n, 120.0, 300.0, 0.02);
    let l: Vec<f32> = tone.iter().zip(&s).map(|(a, b)| a + b).collect();
    let r: Vec<f32> = tone.iter().zip(&s).map(|(a, b)| a - 0.5 * b).collect();
    for mode in every_mode() {
        for look in [0.0, 5.0] {
            for (thr, range) in [(0.0, 6.0), (-60.0, 0.0)] {
                let st = DeessSettings {
                    threshold_db: thr,
                    range_db: range,
                    lookahead_ms: look,
                    ..mode
                };
                let lat = latency_for(&st, SR);
                let (a, b) = run(&st, &l, &r, None);
                assert!(a[..lat].iter().all(|x| *x == 0.0));
                assert_eq!(&a[lat..], &l[..n - lat], "{st:?}: left");
                assert_eq!(&b[lat..], &r[..n - lat], "{st:?}: right");
                if look == 0.0 && mode.processing == Processing::Wide {
                    assert_eq!(lat, 0, "wide band with no lookahead has no latency");
                }
            }
        }
    }
}

/// The latency reported is the delay there is: an impulse comes out `latency()` later,
/// unchanged, in every processing mode.
#[test]
fn the_latency_reported_is_the_delay_there_is() {
    for processing in Processing::ALL {
        for look in [0.0, 1.0, 5.0, 15.0] {
            let st = DeessSettings {
                processing: *processing,
                lookahead_ms: look,
                threshold_db: 0.0,
                ..base()
            };
            let mut d = Sibilus::new(SR);
            d.set(&st);
            let lat = d.latency();
            assert_eq!(lat, latency_for(&st, SR));
            assert_eq!(
                lat,
                (look * 48.0) as usize
                    + if *processing == Processing::Wide {
                        0
                    } else {
                        127
                    }
            );
            let mut x = vec![0.0; 2_000];
            x[10] = 0.5;
            let mut r = x.clone();
            let mut l = x.clone();
            d.process(&mut l, &mut r, None);
            let at = l.iter().position(|v| *v != 0.0).unwrap();
            assert_eq!(at, 10 + lat);
            assert_eq!(l[at], 0.5);
        }
    }
}

/// While it is working hard, in every mode and with a key, the output is the same to the
/// bit at block sizes 1, 7, 64, 1000 and 4096.
#[test]
fn the_result_does_not_depend_on_the_block_size() {
    let n = 12_000;
    let tone = bright_tone(n, 220.0, 0.2);
    let s = sibilant_bursts(n, 60.0, 150.0, 0.5);
    let l: Vec<f32> = tone.iter().zip(&s).map(|(a, b)| a + b).collect();
    let r: Vec<f32> = tone.iter().zip(&s).map(|(a, b)| 0.7 * a - b).collect();
    let key = sibilant_bursts(n, 40.0, 100.0, 0.3);
    let mut modes = every_mode();
    modes.push(DeessSettings {
        lookahead_ms: 5.0,
        link: 0.4,
        attack_ms: 0.3,
        release_ms: 80.0,
        sensitivity: 0.8,
        ..base()
    });
    for (i, mode) in modes.iter().enumerate() {
        let st = DeessSettings {
            listen: Listen::from_index(i % 3),
            lookahead_ms: if i % 2 == 0 { 3.0 } else { mode.lookahead_ms },
            ..*mode
        };
        for keyed in [false, true] {
            let run_blocks = |block: usize| {
                let mut d = Sibilus::new(SR);
                d.set(&st);
                let (mut a, mut b) = (l.clone(), r.clone());
                let mut at = 0;
                for (ca, cb) in a.chunks_mut(block).zip(b.chunks_mut(block)) {
                    let len = ca.len();
                    let k = keyed.then(|| (&key[at..at + len], &key[at..at + len]));
                    d.process(ca, cb, k);
                    at += len;
                }
                (a, b, d.seq(), d.events())
            };
            let reference = run_blocks(n);
            assert!(reference.0.iter().all(|x| x.is_finite()));
            for block in [1usize, 7, 64, 1000, 4096] {
                assert!(
                    run_blocks(block) == reference,
                    "{st:?} keyed={keyed} differs at block {block}"
                );
            }
        }
    }
}

/// The measurements the rest of the tests rely on: a burst of hiss at a known band level,
/// a bright sawtooth and a pure 8 kHz sine at the *same* band level.
struct Material {
    bursts: Vec<f32>,
    saw: Vec<f32>,
    whistle: Vec<f32>,
    level_db: f32,
}

fn material() -> Material {
    let n = 48_000;
    let bursts = sibilant_bursts(n, 150.0, 400.0, 1.0);
    // Band level of a burst's steady middle.
    let mid = |x: &[f32]| band_level_db(&x[..(0.15 * SR) as usize]);
    let b_level = mid(&bursts);
    let target = -12.0;
    let g = math::powf(10f32, (target - b_level) / 20.0);
    let bursts: Vec<f32> = bursts.iter().map(|x| x * g).collect();
    let fit = |x: Vec<f32>| {
        let lv = band_level_db(&x);
        let g = math::powf(10f32, (target - lv) / 20.0);
        x.into_iter().map(|v| v * g).collect::<Vec<_>>()
    };
    let saw = fit(bright_tone(n, 220.0, 0.5));
    let whistle = fit(sine(n, 8_000.0, 0.5));
    Material {
        bursts,
        saw,
        whistle,
        level_db: target,
    }
}

/// The middle of the first burst, where it is steady.
fn burst_middle() -> (usize, usize) {
    ((0.04 * SR) as usize, (0.13 * SR) as usize)
}

/// A burst of sibilance well over the threshold is cut by the Range, within 1 dB, in every
/// detection and processing mode.
#[test]
fn a_sibilant_burst_is_cut_by_the_range() {
    let m = material();
    let (from, to) = burst_middle();
    assert!((band_level_db(&m.bursts[from..to]) - m.level_db).abs() < 1.0);
    for detection in Detection::ALL {
        for processing in Processing::ALL {
            for range in [6.0, 12.0] {
                // The band holds the whole hiss, skirts and all: split band cuts only
                // what is inside it.
                let st = DeessSettings {
                    detection: *detection,
                    processing: *processing,
                    range_db: range,
                    lo_hz: 4_000.0,
                    hi_hz: 13_000.0,
                    ..base()
                };
                let cut = cut_db(&st, &m.bursts, from, to);
                eprintln!("{detection:?} {processing:?} range {range}: burst cut {cut:.2} dB");
                assert!(
                    (cut - range).abs() < 1.0,
                    "{detection:?} {processing:?}: cut {cut} dB for a range of {range}"
                );
            }
        }
    }
}

/// The point of Single vocal: a bright sawtooth and a pure 8 kHz whistle at the *same*
/// band level as the burst are left almost alone, while Allround — level only — cuts them
/// as hard as the burst.
#[test]
fn single_vocal_tells_a_sibilant_from_a_bright_tone() {
    let m = material();
    let (from, to) = burst_middle();
    let steady = ((0.2 * SR) as usize, (0.9 * SR) as usize);
    for detection in Detection::ALL {
        let st = DeessSettings {
            detection: *detection,
            ..base()
        };
        let burst = cut_db(&st, &m.bursts, from, to);
        let saw = cut_db(&st, &m.saw, steady.0, steady.1);
        let whistle = cut_db(&st, &m.whistle, steady.0, steady.1);
        eprintln!("{detection:?}: burst {burst:.2} dB, saw {saw:.2} dB, whistle {whistle:.2} dB");
        match detection {
            Detection::Voice => {
                assert!((burst - 6.0).abs() < 1.0, "burst cut {burst}");
                assert!(saw < 0.5, "the sawtooth was cut {saw} dB");
                assert!(whistle < 0.5, "the whistle was cut {whistle} dB");
            }
            Detection::Allround => {
                assert!((burst - 6.0).abs() < 1.0, "burst cut {burst}");
                assert!(saw > 5.0, "Allround should cut the saw: {saw}");
                assert!(whistle > 5.0, "Allround should cut the whistle: {whistle}");
            }
        }
    }
}

/// What the classifier reads on each material, as numbers: the share and flatness of an
/// "s", a vowel-like sawtooth and a whistle.
#[test]
fn the_classifier_reads_what_it_says() {
    let m = material();
    let read = |x: &[f32], from: usize, to: usize| {
        let mut d = Sibilus::new(SR);
        d.set(&base());
        let (mut a, mut b) = (x[..to].to_vec(), x[..to].to_vec());
        let mut sum = [0f64; 3];
        let mut count = 0;
        for (i, (ca, cb)) in a.chunks_mut(16).zip(b.chunks_mut(16)).enumerate() {
            d.process(ca, cb, None);
            if i * 16 >= from {
                let f = d.features()[0];
                sum[0] += f.share_db as f64;
                sum[1] += f.flatness as f64;
                sum[2] += f.confidence as f64;
                count += 1;
            }
        }
        sum.map(|v| (v / count as f64) as f32)
    };
    let (from, to) = burst_middle();
    let s = read(&m.bursts, from, to);
    let saw = read(&m.saw, 9_600, 40_000);
    let whistle = read(&m.whistle, 9_600, 40_000);
    eprintln!(
        "hiss: share {:.1} dB flat {:.2} conf {:.2}",
        s[0], s[1], s[2]
    );
    eprintln!(
        "saw: share {:.1} dB flat {:.2} conf {:.2}",
        saw[0], saw[1], saw[2]
    );
    eprintln!(
        "whistle: share {:.1} dB flat {:.2} conf {:.2}",
        whistle[0], whistle[1], whistle[2]
    );
    assert!(s[0] > -3.0 && s[1] > 0.6 && s[2] > 0.95);
    assert!(saw[0] < -15.0 && saw[2] < 0.05);
    assert!(whistle[0] > -3.0, "a whistle is all in the band");
    assert!(whistle[1] < 0.2 && whistle[2] < 0.05, "but it is not noise");
}

/// A phrase: vowel, "s", vowel. The vowels are untouched, the "s" is cut by the Range.
#[test]
fn a_phrase_keeps_its_vowels() {
    let n = 48_000;
    let vowel = bright_tone(n, 180.0, 0.3);
    let hiss = sibilant_bursts(n, 150.0, 1_000_000.0, 1.0);
    let s0 = (0.3 * SR) as usize;
    let len = (0.15 * SR) as usize;
    // Vowel, then the hiss alone for 150 ms, then the vowel again.
    let x: Vec<f32> = (0..n)
        .map(|i| {
            if (s0..s0 + len).contains(&i) {
                hiss[i - s0] * 0.25
            } else {
                vowel[i]
            }
        })
        .collect();
    let st = DeessSettings {
        threshold_db: -50.0,
        lookahead_ms: 5.0,
        ..base()
    };
    let before = cut_db(&st, &x, (0.1 * SR) as usize, (0.28 * SR) as usize);
    let during = cut_db(&st, &x, s0 + 480, s0 + len - 480);
    let after = cut_db(&st, &x, s0 + len + 4_800, n - 100);
    eprintln!("vowel {before:.2} dB, s {during:.2} dB, vowel {after:.2} dB");
    assert!(before < 0.2 && after < 0.2);
    assert!((during - 6.0).abs() < 1.0);
}

/// Split band leaves what is outside the band alone: a 200 Hz tone under the burst keeps
/// its level, and the split's amplitude is 1 in the band and 0 well below it.
#[test]
fn split_band_leaves_the_rest_alone() {
    let m = material();
    let n = m.bursts.len();
    let low = sine(n, 200.0, 0.5);
    let x: Vec<f32> = m.bursts.iter().zip(&low).map(|(a, b)| a + b).collect();
    for processing in [Processing::SplitHigh, Processing::SplitBand] {
        let st = DeessSettings {
            processing,
            ..base()
        };
        let mut d = Sibilus::new(SR);
        d.set(&st);
        assert!((d.split_amplitude(7_000.0) - 1.0).abs() < 0.01);
        assert!(d.split_amplitude(1_000.0).abs() < 0.01);
        if processing == Processing::SplitBand {
            assert!(d.split_amplitude(18_000.0).abs() < 0.01);
        }
        // The removed signal is all above the split: correlate it with the low tone.
        let lat = d.latency();
        let st_removed = DeessSettings {
            listen: Listen::Removed,
            ..st
        };
        let (removed, _) = run(&st_removed, &x, &x, None);
        let (from, to) = burst_middle();
        let low_in_removed = removed[from + lat..to + lat]
            .iter()
            .zip(&low[from..to])
            .map(|(a, b)| (*a as f64) * (*b as f64))
            .sum::<f64>()
            / low[from..to]
                .iter()
                .map(|b| math::powi(*b as f64, 2))
                .sum::<f64>();
        eprintln!("{processing:?}: share of the 200 Hz tone removed {low_in_removed:.2e}");
        assert!(low_in_removed.abs() < 1e-3);
        let cut = cut_db(&st, &m.bursts, from, to);
        assert!((cut - 6.0).abs() < 1.0, "{processing:?}: burst cut {cut}");
    }
}

/// Normal and Removed add up to the input; Trigger is the band.
#[test]
fn what_you_hear_adds_up() {
    let m = material();
    let n = m.bursts.len();
    let x: Vec<f32> = m
        .bursts
        .iter()
        .zip(sine(n, 300.0, 0.3))
        .map(|(a, b)| a + b)
        .collect();
    for processing in Processing::ALL {
        let st = DeessSettings {
            processing: *processing,
            lookahead_ms: 2.0,
            ..base()
        };
        let lat = latency_for(&st, SR);
        let (y, _) = run(&st, &x, &x, None);
        let (d, _) = run(
            &DeessSettings {
                listen: Listen::Removed,
                ..st
            },
            &x,
            &x,
            None,
        );
        let worst = (0..n - lat)
            .map(|i| (y[i + lat] + d[i + lat] - x[i]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            worst < 1e-6,
            "{processing:?}: normal + removed is off by {worst}"
        );
        assert!(rms_db(&d) > -60.0, "something was removed");
    }
    // The trigger passes an 8 kHz sine and stops a 500 Hz one.
    let st = DeessSettings {
        listen: Listen::Trigger,
        ..base()
    };
    let (hi, _) = run(
        &st,
        &sine(9_600, 8_000.0, 0.5),
        &sine(9_600, 8_000.0, 0.5),
        None,
    );
    let (lo, _) = run(
        &st,
        &sine(9_600, 500.0, 0.5),
        &sine(9_600, 500.0, 0.5),
        None,
    );
    let (a, b) = (rms_db(&hi[4_800..]), rms_db(&lo[4_800..]));
    assert!((a - rms_db(&sine(4_800, 8_000.0, 0.5))).abs() < 0.5);
    assert!(b < a - 60.0, "500 Hz leaks into the trigger: {b} vs {a}");
}

/// An external key: a signal with no sibilance of its own is cut when the key hisses, and
/// left alone when the key is silent.
#[test]
fn an_external_key_decides() {
    let m = material();
    let n = m.bursts.len();
    let x = sine(n, 400.0, 0.3);
    let st = DeessSettings {
        detection: Detection::Allround,
        ..base()
    };
    let (own, _) = run(&st, &x, &x, None);
    assert_eq!(own, x, "no sibilance of its own: a wire");
    let (keyed, _) = run(&st, &x, &x, Some((&m.bursts, &m.bursts)));
    let (from, to) = burst_middle();
    let cut = rms_db(&x[from..to]) - rms_db(&keyed[from..to]);
    assert!((cut - 6.0).abs() < 1.0, "keyed cut {cut}");
    let silent = vec![0.0; n];
    let (quiet, _) = run(&st, &x, &x, Some((&silent, &silent)));
    assert_eq!(quiet, x);
}

/// Mid only leaves the side alone and side only the mid; stereo link shares the cut.
#[test]
fn stereo_modes_touch_only_what_they_name() {
    let m = material();
    let (from, to) = burst_middle();
    let s = &m.bursts;
    let neg: Vec<f32> = s.iter().map(|v| -v).collect();
    // Sibilance only in the side.
    let (a, b) = run(
        &DeessSettings {
            stereo: StereoMode::Mid,
            ..base()
        },
        s,
        &neg,
        None,
    );
    assert_eq!(
        (&a, &b),
        (s, &neg),
        "mid only: a side-only hiss passes untouched"
    );
    let (a, _) = run(
        &DeessSettings {
            stereo: StereoMode::Side,
            ..base()
        },
        s,
        &neg,
        None,
    );
    let cut = rms_db(&s[from..to]) - rms_db(&a[from..to]);
    assert!((cut - 6.0).abs() < 1.0, "side only: {cut}");
    // Sibilance only in the mid, processing the side: a wire.
    let (a, _) = run(
        &DeessSettings {
            stereo: StereoMode::Side,
            ..base()
        },
        s,
        s,
        None,
    );
    assert_eq!(&a, s);
    // Hiss on the left only: linked, the right is cut as much; unlinked, not at all.
    let quiet = sine(s.len(), 300.0, 0.1);
    for (link, expect) in [(1.0, 6.0), (0.0, 0.0)] {
        let (_, r) = run(
            &DeessSettings {
                detection: Detection::Allround,
                link,
                ..base()
            },
            s,
            &quiet,
            None,
        );
        let cut = rms_db(&quiet[from..to]) - rms_db(&r[from..to]);
        assert!((cut - expect).abs() < 1.0, "link {link}: right cut {cut}");
    }
}

/// With lookahead the onset is already cut; without it, the first millisecond gets
/// through.
#[test]
fn lookahead_catches_the_onset() {
    let n = 24_000;
    let h = hiss(n, 3);
    let start = 9_600;
    let x: Vec<f32> = (0..n)
        .map(|i| if i >= start { h[i] * 2.0 } else { 0.0 })
        .collect();
    let first_ms = |look: f32| {
        let st = DeessSettings {
            detection: Detection::Allround,
            lookahead_ms: look,
            attack_ms: 0.5,
            ..base()
        };
        cut_db(&st, &x, start, start + 48)
    };
    let (with, without) = (first_ms(5.0), first_ms(0.0));
    eprintln!("first ms cut: lookahead 5 ms {with:.2} dB, none {without:.2} dB");
    assert!(with > 5.0, "with lookahead the first ms is cut {with}");
    assert!(without < 3.0, "without it the onset passes: {without}");
}

/// After a burst, a long silence ends in exact zeros (no denormal tail), and nothing is
/// ever NaN, whatever the settings.
#[test]
fn silence_is_silence_and_nothing_is_nan() {
    let m = material();
    let mut x = m.bursts[..9_600].to_vec();
    x.extend(std::iter::repeat_n(0.0, 96_000));
    for mode in every_mode() {
        let st = DeessSettings {
            lookahead_ms: 5.0,
            ..mode
        };
        let (a, b) = run(&st, &x, &x, None);
        assert!(a[a.len() - 1_000..].iter().all(|v| *v == 0.0), "{st:?}");
        assert!(b.iter().all(|v| v.is_finite()));
    }
    // Wild settings and a loud, clipped input.
    let mut seed = 11u32;
    let loud: Vec<f32> = (0..9_600).map(|_| 8.0 * noise(&mut seed)).collect();
    for i in 0..40u32 {
        let r = |k: u32| ((i * 7919 + k * 104_729) % 1000) as f32 / 1000.0;
        let st = DeessSettings {
            detection: Detection::from_index(i as usize % 2),
            processing: Processing::from_index(i as usize % 3),
            stereo: StereoMode::from_index(i as usize % 4),
            listen: Listen::from_index(i as usize % 3),
            threshold_db: -60.0 * r(1),
            range_db: 30.0 * r(2),
            lo_hz: 1_000.0 + 19_000.0 * r(3),
            hi_hz: 1_000.0 + 19_000.0 * r(4),
            attack_ms: 0.1 + 50.0 * r(5),
            release_ms: 5.0 + 500.0 * r(6),
            lookahead_ms: 15.0 * r(7),
            link: r(8),
            sensitivity: r(9),
        };
        let (a, b) = run(&st, &loud, &loud, None);
        assert!(a.iter().chain(&b).all(|v| v.is_finite()), "{st:?}");
    }
}

/// Events: three bursts are three events, each cut by the range, centred in the band; the
/// history columns line up with them.
#[test]
fn events_are_counted_and_placed() {
    let m = material();
    let st = DeessSettings {
        lookahead_ms: 5.0,
        ..base()
    };
    let mut d = Sibilus::new(SR);
    d.set(&st);
    // Three bursts, and time for the last one's release to end it.
    let mut x = m.bursts.clone();
    x.extend(std::iter::repeat_n(0.0, (0.3 * SR) as usize));
    let x = &x[..];
    let (mut a, mut b) = (x.to_vec(), x.to_vec());
    let mut cols = Vec::new();
    let mut seen = 0;
    for (ca, cb) in a.chunks_mut(256).zip(b.chunks_mut(256)) {
        d.process(ca, cb, None);
        while seen < d.seq() {
            cols.push(d.column(seen).expect("a recent column is kept"));
            seen += 1;
        }
    }
    let e = d.events();
    eprintln!("{e:?}");
    assert_eq!(e.count, 3);
    assert!((e.mean_gr_db - 6.0).abs() < 0.5 && (e.max_gr_db - 6.0).abs() < 0.5);
    assert!(
        (5_000.0..10_000.0).contains(&e.last_hz),
        "the centre of a 5–10 kHz hiss: {}",
        e.last_hz
    );
    // Columns: about 11 ms each, cut where the bursts are and not between them.
    assert_eq!(cols.len(), x.len() / d.chunk());
    let col_s = d.chunk() as f32 / SR;
    let at = |s: f32| cols[(s / col_s) as usize];
    assert!(at(0.08).gr_db > 5.0 && at(0.08).conf > 0.9 && at(0.08).det_db > -14.0);
    assert!(at(0.3).gr_db < 0.25 && at(0.3).input == 0.0);
    assert!(at(0.48).gr_db > 5.0);
    assert!(at(0.48).output < at(0.48).input * 0.6);
    assert!(d.column(0).is_none(), "only the last few columns are kept");
}

/// The static curve: nothing below the knee, the excess above it, capped at the Range.
#[test]
fn the_curve_is_the_one_drawn() {
    let s = DeessSettings {
        threshold_db: -30.0,
        range_db: 8.0,
        ..DeessSettings::default()
    };
    assert_eq!(curve_db(-34.0, &s), 0.0);
    assert!(curve_db(-30.0, &s) > 0.0 && curve_db(-30.0, &s) < 1.0);
    assert!((curve_db(-25.0, &s) - 4.95).abs() < 0.1);
    assert_eq!(curve_db(0.0, &s), 8.0);
}

/// Band edges come back ordered and in range whatever they are given; a glide reaches its
/// target.
#[test]
fn band_edges_are_sane_and_glide() {
    assert_eq!(band_edges(12_000.0, 5_000.0, SR), (5_000.0, 12_000.0));
    let (a, b) = band_edges(8_000.0, 8_000.0, SR);
    assert!(b / a > 1.12 && b / a < 1.13);
    let (a, b) = band_edges(10.0, 90_000.0, SR);
    assert_eq!(a, MIN_HZ);
    assert_eq!(b, MAX_HZ);
    let mut d = Sibilus::new(SR);
    d.set(&base());
    let mut x = vec![0.0; 256];
    let mut y = x.clone();
    d.process(&mut x, &mut y, None);
    d.set(&DeessSettings {
        lo_hz: 6_000.0,
        ..base()
    });
    let mut x = vec![0.0; 48_000];
    let mut y = x.clone();
    d.process(&mut x, &mut y, None);
    assert_eq!(d.band(), (6_000.0, 12_000.0));
}
