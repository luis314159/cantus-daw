use super::*;
use crate::modfx_clock::{DIVISIONS, DIV_EIGHTH, DIV_QUARTER};
use crate::modfx_lfo::{Rate, Shape};

const SR: f32 = 48_000.0;

/// A bar of noise and two tones, stereo.
fn music(n: usize, seed: u32) -> (Vec<f32>, Vec<f32>) {
    let mut s = seed | 1;
    let mut noise = move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / SR;
        let tone = crate::math::sin(std::f32::consts::TAU * 220.0 * (t % 1.0)) * 0.4;
        l.push(tone + noise() * 0.1);
        r.push(tone * 0.8 + noise() * 0.1);
    }
    (l, r)
}

fn playing(samples: i64) -> Transport {
    Transport {
        playing: true,
        tempo_bpm: 128.0,
        samples,
    }
}

/// Run a unit over `l`, `r` from song sample `start`, cut by `split`, the transport
/// given before each call as the engine gives it.
fn drive(
    t: &mut Tremor,
    l: &mut [f32],
    r: &mut [f32],
    start: i64,
    mut split: impl FnMut() -> usize,
) {
    let n = l.len();
    let mut at = 0;
    while at < n {
        let b = split().clamp(1, n - at);
        t.set_transport(&playing(start + at as i64));
        t.process(&mut l[at..at + b], &mut r[at..at + b]);
        at += b;
    }
}

fn unit(s: &TremorSettings) -> Tremor {
    let mut t = Tremor::new(SR);
    t.set(s);
    t.reset();
    t
}

fn busy(mode: Mode) -> TremorSettings {
    TremorSettings {
        mode,
        depth: 0.8,
        lfo: LfoSettings {
            rate: Rate::Beats(DIVISIONS[DIV_EIGHTH]),
            shape: Shape::Square,
            edge: 0.2,
            swing: 0.3,
            ..LfoSettings::default()
        },
        spread: 0.25,
        xover_hz: 600.0,
        out_db: -2.0,
    }
}

fn same_bits(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn at_depth_zero_every_mode_is_a_wire() {
    for mode in [Mode::Tremolo, Mode::Pan, Mode::Harmonic] {
        let mut s = busy(mode);
        s.depth = 0.0;
        s.out_db = 0.0;
        let (l0, r0) = music(30_000, 3);
        let (mut l, mut r) = (l0.clone(), r0.clone());
        drive(&mut unit(&s), &mut l, &mut r, 0, || 333);
        assert!(same_bits(&l, &l0) && same_bits(&r, &r0), "{mode:?}");
    }
}

#[test]
fn the_defaults_are_a_wire() {
    let (l0, r0) = music(10_000, 4);
    let (mut l, mut r) = (l0.clone(), r0.clone());
    unit(&TremorSettings::default()).process(&mut l, &mut r);
    assert!(same_bits(&l, &l0) && same_bits(&r, &r0));
}

#[test]
fn it_is_the_same_at_every_split() {
    for mode in [Mode::Tremolo, Mode::Pan, Mode::Harmonic] {
        let s = busy(mode);
        let (l0, r0) = music(40_000, 5);
        let (mut la, mut ra) = (l0.clone(), r0.clone());
        drive(&mut unit(&s), &mut la, &mut ra, 0, || 512);
        let mut k = 0usize;
        let (mut lb, mut rb) = (l0.clone(), r0.clone());
        drive(&mut unit(&s), &mut lb, &mut rb, 0, || {
            k += 1;
            [1, 7, 64, 4096, 129][k % 5]
        });
        assert!(same_bits(&la, &lb) && same_bits(&ra, &rb), "{mode:?}");
    }
}

#[test]
fn a_synced_tremolo_from_bar_five_is_the_full_render_from_bar_five() {
    let bar5 = (16.0 * 60.0 / 128.0 * f64::from(SR)).round() as usize;
    let s = busy(Mode::Tremolo);
    let (l0, r0) = music(bar5 + 20_000, 6);
    let (mut lf, mut rf) = (l0.clone(), r0.clone());
    drive(&mut unit(&s), &mut lf, &mut rf, 0, || 480);
    let (mut lp, mut rp) = (l0[bar5..].to_vec(), r0[bar5..].to_vec());
    drive(&mut unit(&s), &mut lp, &mut rp, bar5 as i64, || 256);
    assert!(same_bits(&lf[bar5..], &lp) && same_bits(&rf[bar5..], &rp));
}

#[test]
fn a_free_tremolo_is_deterministic_from_the_start_of_playback() {
    let mut s = busy(Mode::Tremolo);
    s.lfo.rate = Rate::Hz(5.3);
    let (l0, r0) = music(20_000, 7);
    // One unit has been running, stopped, for a while; the other is new.
    let mut old = unit(&s);
    let (mut zl, mut zr) = (vec![0.25f32; 7_777], vec![0.25f32; 7_777]);
    old.set_transport(&Transport {
        playing: false,
        tempo_bpm: 128.0,
        samples: 0,
    });
    old.process(&mut zl, &mut zr);
    let (mut la, mut ra) = (l0.clone(), r0.clone());
    drive(&mut old, &mut la, &mut ra, 0, || 300);
    let (mut lb, mut rb) = (l0.clone(), r0.clone());
    drive(&mut unit(&s), &mut lb, &mut rb, 0, || 1000);
    assert!(same_bits(&la, &lb) && same_bits(&ra, &rb));
}

#[test]
fn a_full_square_tremolo_opens_and_closes_on_the_beat() {
    let s = TremorSettings {
        depth: 1.0,
        lfo: LfoSettings {
            rate: Rate::Beats(DIVISIONS[DIV_QUARTER]),
            shape: Shape::Square,
            edge: 0.0,
            ..LfoSettings::default()
        },
        ..TremorSettings::default()
    };
    let beat = (60.0 / 128.0 * SR) as usize;
    let mut l = vec![1.0f32; 2 * beat];
    let mut r = l.clone();
    drive(&mut unit(&s), &mut l, &mut r, 0, || 512);
    // Open through the first half of each beat, shut through the second.
    assert!((l[beat / 4] - 1.0).abs() < 1e-6);
    assert!(l[3 * beat / 4].abs() < 1e-6);
    assert!((l[beat + beat / 4] - 1.0).abs() < 1e-6);
}

#[test]
fn auto_pan_keeps_a_mono_sources_power() {
    let s = TremorSettings {
        mode: Mode::Pan,
        depth: 1.0,
        lfo: LfoSettings {
            rate: Rate::Hz(3.0),
            shape: Shape::Triangle,
            ..LfoSettings::default()
        },
        ..TremorSettings::default()
    };
    let mut l = vec![0.5f32; 30_000];
    let mut r = l.clone();
    drive(&mut unit(&s), &mut l, &mut r, 0, || 777);
    for i in (0..30_000).step_by(97) {
        let p = l[i] * l[i] + r[i] * r[i];
        assert!((p - 0.5).abs() < 1e-5, "{i}: {p}");
    }
    // It reaches both sides.
    assert!(l.iter().any(|x| *x < 1e-3) && r.iter().any(|x| *x < 1e-3));
}

#[test]
fn the_pan_law_is_unity_at_the_centre_and_equal_power_everywhere() {
    let (a, b) = pan_gains(0.0);
    assert!((a - 1.0).abs() < 1e-6 && (b - 1.0).abs() < 1e-6);
    for k in 0..=40 {
        let p = -1.0 + k as f32 / 20.0;
        let (gl, gr) = pan_gains(p);
        assert!((gl * gl + gr * gr - 2.0).abs() < 1e-5);
    }
}

#[test]
fn the_harmonic_tremolo_moves_the_lows_against_the_highs() {
    let s = TremorSettings {
        mode: Mode::Harmonic,
        depth: 1.0,
        lfo: LfoSettings {
            rate: Rate::Hz(2.0),
            ..LfoSettings::default()
        },
        xover_hz: 700.0,
        ..TremorSettings::default()
    };
    let level = |f: f32, at: usize| -> f32 {
        let n = 48_000;
        let (mut l, mut r): (Vec<f32>, Vec<f32>) = (0..n)
            .map(|i| crate::math::sin(std::f32::consts::TAU * f * i as f32 / SR))
            .map(|x| (x, x))
            .unzip();
        drive(&mut unit(&s), &mut l, &mut r, 0, || 512);
        // The envelope around `at`.
        l[at - 600..at + 600]
            .iter()
            .fold(0.0f32, |m, x| m.max(x.abs()))
    };
    // A quarter of a 2 Hz cycle in (the LFO's top): the lows open, the highs shut; three
    // quarters in, the other way round.
    let top = 6_000;
    let bottom = 18_000;
    assert!(level(80.0, top) > 0.9 && level(80.0, bottom) < 0.2);
    assert!(level(7_100.0, top) < 0.2 && level(7_100.0, bottom) > 0.9);
}

#[test]
fn it_comes_to_rest_and_skipping_is_processing_silence() {
    let s = busy(Mode::Harmonic);
    let (mut l, mut r) = music(10_000, 8);
    let mut a = unit(&s);
    drive(&mut a, &mut l, &mut r, 0, || 512);
    let mut silent = 0;
    while !a.at_rest() && silent < 48_000 {
        let (mut zl, mut zr) = (vec![0.0f32; 64], vec![0.0f32; 64]);
        a.process(&mut zl, &mut zr);
        silent += 64;
    }
    assert!(a.at_rest(), "{:?}", a.rest_blocker());
    let mut b = a.clone();
    let (mut zl, mut zr) = (vec![0.0f32; 999], vec![0.0f32; 999]);
    a.process(&mut zl, &mut zr);
    assert!(zl.iter().chain(&zr).all(|x| x.to_bits() == 0));
    b.skip(999);
    let (l0, r0) = music(5_000, 9);
    let (mut la, mut ra) = (l0.clone(), r0.clone());
    let (mut lb, mut rb) = (l0, r0);
    a.process(&mut la, &mut ra);
    b.process(&mut lb, &mut rb);
    assert!(same_bits(&la, &lb) && same_bits(&ra, &rb));
}
