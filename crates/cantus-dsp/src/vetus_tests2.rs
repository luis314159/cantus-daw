//! Vetus's measured tests (ADR 0133, provisional): the wobble's pitch, the rate's aliasing
//! and images, the medium's band, Character, the switches and rest.

use super::digital::{BitsMode, RateFilter};
use super::*;
use crate::modfx_clock::Transport;
use crate::modfx_lfo::Rate as LfoRate;

const SR: f32 = 48_000.0;

fn sine(hz: f32, amp: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let c = (f64::from(hz) * i as f64 / f64::from(SR)).fract() as f32;
            amp * math::sin(std::f32::consts::TAU * c)
        })
        .collect()
}

/// The left channel of `x` (both channels fed `x`) through a new unit, 256 at a time.
fn run(s: &VetusSettings, x: &[f32]) -> Vec<f32> {
    let mut v = Vetus::new(SR);
    v.set(s);
    v.reset();
    let (mut l, mut r) = (x.to_vec(), x.to_vec());
    for (i, (a, b)) in l.chunks_mut(256).zip(r.chunks_mut(256)).enumerate() {
        v.set_transport(&Transport {
            playing: true,
            tempo_bpm: 120.0,
            samples: i as i64 * 256,
        });
        v.process(a, b);
    }
    l
}

/// The amplitude of `hz` in `x` (a Hann-windowed DFT bin).
fn level(x: &[f32], hz: f32) -> f32 {
    let n = x.len();
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64);
        let ph = std::f64::consts::TAU * f64::from(hz) * i as f64 / f64::from(SR);
        let (s, c) = math::sin_cos(ph);
        re += f64::from(*v) * w * c;
        im += f64::from(*v) * w * s;
    }
    (2.0 * math::hypot(re, im) / (0.5 * n as f64)) as f32
}

fn db(x: f32) -> f32 {
    20.0 * math::log10(x.max(1e-12))
}

/// The pitch swing of a wow of 20 cents at 1 Hz on a 1 kHz sine, measured from its zero
/// crossings, is ±20 cents within 2.
#[test]
fn the_wow_swings_the_pitch_by_its_cents() {
    let s = VetusSettings {
        wob_on: true,
        wow_ct: 20.0,
        wow_rate: LfoRate::Hz(1.0),
        drift: 0.0,
        flut_ct: 0.0,
        ..VetusSettings::default()
    };
    let y = run(&s, &sine(1_000.0, 0.5, 4 * 48_000));
    let mut cross = Vec::new();
    for i in 48_000..y.len() {
        let (a, b) = (y[i - 1], y[i]);
        if a < 0.0 && b >= 0.0 {
            cross.push((i - 1) as f64 + f64::from(a / (a - b)));
        }
    }
    let cents: Vec<f64> = cross
        .windows(2)
        .map(|w| 1200.0 * math::log2(f64::from(SR) / (w[1] - w[0]) / 1_000.0))
        .collect();
    let hi = cents.iter().copied().fold(f64::MIN, f64::max);
    let lo = cents.iter().copied().fold(f64::MAX, f64::min);
    assert!(
        (hi - 20.0).abs() < 2.0 && (lo + 20.0).abs() < 2.0,
        "{lo:.1} … {hi:.1} ct"
    );
}

/// Raw, a 9 kHz sine held at 8 kHz folds to 1 kHz; behind the anti-alias filter the fold
/// is at least 40 dB lower. Measured: printed.
#[test]
fn the_anti_alias_filter_stops_the_fold() {
    let x = sine(9_000.0, 0.5, 48_000);
    let at = |f: RateFilter| {
        let s = VetusSettings {
            rate_on: true,
            rate_hz: 8_000.0,
            rate_filter: f,
            ..VetusSettings::default()
        };
        db(level(&run(&s, &x)[4_800..], 1_000.0) / 0.5)
    };
    let (raw, aa) = (at(RateFilter::None), at(RateFilter::AntiAlias));
    println!("the fold at 1 kHz: raw {raw:.1} dB, anti-alias {aa:.1} dB");
    assert!(raw > -3.0, "raw folds at full level: {raw:.1}");
    assert!(aa < raw - 40.0, "{aa:.1} against {raw:.1}");
}

/// The hold's steps image a 1 kHz tone at 7 kHz (8 − 1); smoothed, the image is at least
/// 20 dB lower and the tone itself untouched within 0.5 dB.
#[test]
fn the_smoothing_filter_removes_the_images() {
    let x = sine(1_000.0, 0.5, 48_000);
    let at = |f: RateFilter| {
        let s = VetusSettings {
            rate_on: true,
            rate_hz: 8_000.0,
            rate_filter: f,
            ..VetusSettings::default()
        };
        let y = run(&s, &x);
        (
            db(level(&y[4_800..], 7_000.0)),
            db(level(&y[4_800..], 1_000.0)),
        )
    };
    let ((img_raw, tone_raw), (img_both, tone_both)) =
        (at(RateFilter::AntiAlias), at(RateFilter::Both));
    println!("the image at 7 kHz: held {img_raw:.1} dB, smoothed {img_both:.1} dB");
    assert!(img_both < img_raw - 20.0);
    assert!((tone_both - db(0.5)).abs() < 0.5 && (tone_raw - db(0.5)).abs() < 1.0);
}

/// The telephone's band: 100 Hz and 8 kHz at least 12 dB under 1 kHz.
#[test]
fn the_telephone_is_a_telephone() {
    let s = VetusSettings {
        tone_on: true,
        medium: Medium::Telephone,
        lo: 300.0,
        hi: 3_400.0,
        ..VetusSettings::default()
    };
    let gain = |hz: f32| db(level(&run(&s, &sine(hz, 0.5, 48_000))[9_600..], hz));
    let (lo, mid, hi) = (gain(100.0), gain(1_000.0), gain(8_000.0));
    assert!(
        lo < mid - 12.0 && hi < mid - 12.0,
        "{lo:.1} {mid:.1} {hi:.1}"
    );
}

/// Character takes every module that is on towards its oldest, never younger, and leaves
/// the modules that are off as they are.
#[test]
fn character_ages_only_what_is_on() {
    let s = VetusSettings {
        bits_on: true,
        bits: 16.0,
        rate_on: true,
        rate_hz: 32_000.0,
        tone_on: false,
        hi: 12_000.0,
        noise_on: true,
        ..VetusSettings::default()
    };
    assert_eq!(s.aged(), s, "at 0 nothing moves");
    let full = VetusSettings {
        character: 1.0,
        ..s
    }
    .aged();
    assert_eq!(full.bits, oldest::BITS);
    assert!((full.rate_hz - oldest::RATE_HZ).abs() < 1.0);
    assert_eq!(full.noise.crackle, oldest::CRACKLE);
    assert_eq!(full.hi, 12_000.0, "the tone is off");
    assert_eq!(full.wow_ct, s.wow_ct, "the wobble is off");
    let half = VetusSettings {
        character: 0.5,
        bits: 3.0,
        ..s
    }
    .aged();
    assert_eq!(half.bits, 3.0, "already older than its oldest");
    assert!((half.rate_hz - 32_000.0 * math::exp2(-1.5)).abs() < 1.0);
}

/// A switch crossfades: turning a module on in the middle of a tone moves no sample more
/// than the sound does from one sample to the next before and after the switch.
#[test]
fn a_switch_never_clicks() {
    let x = sine(440.0, 0.5, 36_000);
    let biggest = |y: &[f32]| {
        y.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max)
    };
    for module in 0..6 {
        let mut s = VetusSettings {
            medium: Medium::Telephone,
            hi: 3_000.0,
            lo: 400.0,
            bits: 12.0,
            ..VetusSettings::default()
        };
        let mut v = Vetus::new(SR);
        v.set(&s);
        v.reset();
        let (mut l, mut r) = (x.clone(), x.clone());
        v.process(&mut l[..12_000], &mut r[..12_000]);
        let sw = [
            &mut s.bits_on,
            &mut s.rate_on,
            &mut s.wob_on,
            &mut s.age_on,
            &mut s.noise_on,
            &mut s.tone_on,
        ];
        *sw.into_iter().nth(module).unwrap() = true;
        s.noise.crackle = 0.0;
        v.set(&s);
        v.process(&mut l[12_000..], &mut r[12_000..]);
        let at_switch = biggest(&l[11_990..12_800]);
        let steady = biggest(&l[6_000..11_990]).max(biggest(&l[24_000..]));
        assert!(
            at_switch <= steady * 1.25,
            "module {module}: {at_switch} at the switch, {steady} steady"
        );
    }
}

/// After the music and a silence it rests — and is then as good as a new unit that skipped
/// as far: the next sound comes out the same, to the bit. The noise rests when it follows
/// the music; playing in silence it never does.
#[test]
fn it_rests_and_is_then_as_good_as_new() {
    let mut s = VetusSettings {
        bits_on: true,
        bits: 6.0,
        dither: true,
        bits_mode: BitsMode::Truncate,
        rate_on: true,
        rate_hz: 7_350.0,
        rate_filter: RateFilter::Both,
        wob_on: true,
        age_on: true,
        noise_on: true,
        follow: 1.0,
        tone_on: true,
        medium: Medium::Radio,
        character: 0.3,
        ..VetusSettings::default()
    };
    let tone = sine(330.0, 0.4, 24_000);
    let block = 128;
    let silence = 10 * 48_000;
    let mut a = Vetus::new(SR);
    a.set(&s);
    a.reset();
    let mut b = Vetus::new(SR);
    b.set(&s);
    b.reset();
    let mut at = 0i64;
    let tr = |at: i64| Transport {
        playing: true,
        tempo_bpm: 120.0,
        samples: at,
    };
    let (mut l, mut r) = (tone.clone(), tone.clone());
    for (x, y) in l.chunks_mut(block).zip(r.chunks_mut(block)) {
        a.set_transport(&tr(at));
        b.set_transport(&tr(at));
        a.process(x, y);
        b.skip(x.len());
        at += x.len() as i64;
    }
    // B skipped through the sound: it is a new unit that never heard it. A now hears the
    // silence until it rests.
    let mut rested = None;
    for k in 0..silence / block {
        let (mut x, mut y) = (vec![0.0f32; block], vec![0.0f32; block]);
        a.set_transport(&tr(at));
        b.set_transport(&tr(at));
        a.process(&mut x, &mut y);
        b.skip(block);
        assert!(x.iter().chain(&y).all(|v| v.abs() < 1.0));
        at += block as i64;
        if rested.is_none() && a.at_rest() {
            rested = Some(k * block);
        }
    }
    assert!(rested.is_some(), "never rested: {:?}", a.rest_blocker());
    println!(
        "at rest {} ms into the silence",
        rested.unwrap() as f32 / 48.0
    );
    let (mut la, mut ra) = (tone.clone(), tone.clone());
    let (mut lb, mut rb) = (tone.clone(), tone);
    for ((xa, ya), (xb, yb)) in la
        .chunks_mut(block)
        .zip(ra.chunks_mut(block))
        .zip(lb.chunks_mut(block).zip(rb.chunks_mut(block)))
    {
        a.set_transport(&tr(at));
        b.set_transport(&tr(at));
        a.process(xa, ya);
        b.process(xb, yb);
        at += xa.len() as i64;
    }
    assert!(la.iter().zip(&lb).all(|(x, y)| x.to_bits() == y.to_bits()));
    assert!(ra.iter().zip(&rb).all(|(x, y)| x.to_bits() == y.to_bits()));
    // Noise that plays in silence never rests.
    s.follow = 0.0;
    a.set(&s);
    for _ in 0..30 {
        let (mut x, mut y) = (vec![0.0f32; 4_096], vec![0.0f32; 4_096]);
        a.process(&mut x, &mut y);
    }
    assert_eq!(a.rest_blocker(), Some("the noise plays in silence"));
}

/// The response the editor draws is the one heard: within 0.1 dB from 40 Hz to 16 kHz for
/// every medium.
#[test]
fn the_drawn_band_is_the_heard_band() {
    for medium in Medium::ALL {
        let s = VetusSettings {
            tone_on: true,
            medium,
            lo: 180.0,
            hi: 5_000.0,
            res: 0.5,
            ..VetusSettings::default()
        };
        for hz in [40.0f32, 120.0, 400.0, 1_100.0, 2_600.0, 7_000.0, 16_000.0] {
            let heard = db(level(&run(&s, &sine(hz, 0.25, 48_000))[24_000..], hz) / 0.25);
            let drawn = tone_db(&s, hz, SR);
            assert!(
                (heard - drawn).abs() < 0.1,
                "{medium:?} at {hz} Hz: heard {heard:.2}, drawn {drawn:.2}"
            );
        }
    }
}
