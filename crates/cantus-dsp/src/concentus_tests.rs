use super::*;
use crate::modfx_clock::DIV_WHOLE;

const SR: f32 = 48_000.0;

fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed | 1;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32 * 2.0 - 1.0) * 0.3
        })
        .collect()
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn unit(s: &ConcentusSettings) -> Concentus {
    let mut c = Concentus::new(SR);
    c.set(s);
    c.reset();
    c
}

fn drive(c: &mut Concentus, l: &mut [f32], r: &mut [f32], mut split: impl FnMut() -> usize) {
    let n = l.len();
    let mut at = 0;
    while at < n {
        let b = split().clamp(1, n - at);
        c.set_transport(&Transport {
            playing: true,
            tempo_bpm: 128.0,
            samples: at as i64,
        });
        c.process(&mut l[at..at + b], &mut r[at..at + b]);
        at += b;
    }
}

fn with(mode: Mode) -> ConcentusSettings {
    ConcentusSettings {
        mode,
        mix: 0.5,
        voices: 4,
        lfo: LfoSettings {
            rate: Rate::Beats(crate::modfx_clock::DIVISIONS[DIV_WHOLE]),
            shape: Shape::Sine,
            ..LfoSettings::default()
        },
        delay_ms: 12.0,
        depth_ms: 4.0,
        spread: 0.8,
        tone_hz: 7_000.0,
        out_db: -1.0,
    }
}

#[test]
fn at_mix_zero_every_mode_is_a_wire() {
    let x = noise(20_000, 1);
    for mode in Mode::ALL {
        let mut s = with(mode);
        s.mix = 0.0;
        s.out_db = 0.0;
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut unit(&s), &mut l, &mut r, || 333);
        assert!(same(&l, &x) && same(&r, &x), "{mode:?}");
    }
    let (mut l, mut r) = (x.clone(), x.clone());
    unit(&ConcentusSettings::default()).process(&mut l, &mut r);
    assert!(same(&l, &x) && same(&r, &x), "the defaults");
}

#[test]
fn every_mode_is_the_same_at_every_split() {
    let x = noise(30_000, 2);
    for mode in Mode::ALL {
        let s = with(mode);
        let (mut la, mut ra) = (x.clone(), x.clone());
        drive(&mut unit(&s), &mut la, &mut ra, || 512);
        let mut k = 0usize;
        let (mut lb, mut rb) = (x.clone(), x.clone());
        drive(&mut unit(&s), &mut lb, &mut rb, || {
            k += 1;
            [1, 7, 64, 4096, 129][k % 5]
        });
        assert!(same(&la, &lb) && same(&ra, &rb), "{mode:?}");
    }
}

/// The Juno's sweeps, measured through the running unit: 1.66 … 5.35 ms at 0.513 and
/// 0.863 Hz; 3.3 … 3.7 ms at 9.75 Hz; the right channel opposite the left.
#[test]
fn the_juno_modes_sweep_as_the_circuit_does() {
    for (mode, lo, hi, hz) in [
        (Mode::JunoI, 1.66f32, 5.35f32, 0.513f32),
        (Mode::JunoII, 1.66, 5.35, 0.863),
        (Mode::JunoBoth, 3.3, 3.7, 9.75),
    ] {
        let mut s = with(mode);
        s.spread = 1.0;
        let mut c = unit(&s);
        let (mut min, mut max) = (f32::MAX, f32::MIN);
        let mut tops = Vec::new();
        let mut last = 0.0f32;
        let mut t = [0.0f32; 64];
        let n = (3.0 * SR) as usize;
        for i in 0..n {
            let (mut l, mut r) = ([0.1f32], [0.1f32]);
            c.process(&mut l, &mut r);
            c.telemetry(&mut t);
            let d = t[T_DELAY_MS];
            min = min.min(d);
            max = max.max(d);
            assert!(
                (t[tel::MOD_L] + t[tel::MOD_R]).abs() < 1e-5,
                "{mode:?}: the right sweeps opposite"
            );
            let gap = (SR / (4.0 * hz)) as usize;
            if d < last && tops.last().is_none_or(|p: &usize| i - p > gap) && d > hi - 0.01 {
                tops.push(i);
            }
            last = d;
        }
        assert!(
            (min - lo).abs() < 0.01 && (max - hi).abs() < 0.01,
            "{mode:?}: {min} … {max} ms"
        );
        let period = (tops[tops.len() - 1] - tops[0]) as f32 / (tops.len() - 1) as f32 / SR;
        assert!(
            (1.0 / period - hz).abs() < 0.01 * hz,
            "{mode:?}: {} Hz",
            1.0 / period
        );
    }
}

#[test]
fn the_mono_check_reads_what_the_mono_sum_keeps() {
    let (c, l) = mono_check(1.0, 1.0, 1.0);
    assert!(
        (c - 1.0).abs() < 1e-6 && l.abs() < 1e-6,
        "identical: {c} {l}"
    );
    let (c, l) = mono_check(1.0, 1.0, 0.0);
    assert!(
        c.abs() < 1e-6 && (l + 3.0103).abs() < 1e-3,
        "uncorrelated: {c} {l}"
    );
    let (c, l) = mono_check(1.0, 1.0, -1.0);
    assert!((c + 1.0).abs() < 1e-6 && l <= -59.9, "opposite: {c} {l}");
    assert_eq!(mono_check(0.0, 0.0, 0.0), (1.0, 0.0));
}

#[test]
fn the_dimension_modes_are_wider_than_the_juno_yet_keep_the_mono_sum() {
    let x = noise(96_000, 3);
    let check = |mode| {
        let mut s = with(mode);
        s.spread = 1.0;
        s.mix = 0.5;
        let mut c = unit(&s);
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut c, &mut l, &mut r, || 480);
        let mut t = [0.0f32; 64];
        c.telemetry(&mut t);
        (t[T_CORR], t[T_MONO_DB])
    };
    let (cj, mj) = check(Mode::JunoI);
    let (cd, md) = check(Mode::Dim4);
    assert!(cd < cj, "Dimension 4 is wider: {cd} against {cj}");
    assert!(
        md > -3.0 && mj > -3.0,
        "neither loses half its level in mono: {md} {mj}"
    );
}

#[test]
fn free_voices_spread_across_the_field_at_about_one_voices_level() {
    let x = noise(48_000, 4);
    let level = |voices: u32| {
        let mut s = with(Mode::Free);
        s.voices = voices;
        s.mix = 1.0;
        s.spread = 1.0;
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut unit(&s), &mut l, &mut r, || 512);
        let e: f32 = l.iter().chain(&r).map(|v| v * v).sum::<f32>() / (2 * x.len()) as f32;
        10.0 * crate::math::log10(e)
    };
    let one = level(1);
    for v in 2..=4 {
        assert!((level(v) - one).abs() < 2.5, "{v} voices against one");
    }
    assert_eq!(voice_pan(0, 4, 1.0), -1.0);
    assert_eq!(voice_pan(3, 4, 1.0), 1.0);
    assert_eq!(voice_pan(0, 1, 1.0), 0.0);
}

#[test]
fn it_comes_to_rest_and_skipping_is_processing_silence() {
    for mode in [Mode::JunoBoth, Mode::Dim2, Mode::Free] {
        let x = noise(10_000, 8);
        let (mut l, mut r) = (x.clone(), x.clone());
        let mut a = unit(&with(mode));
        drive(&mut a, &mut l, &mut r, || 512);
        let mut silent = 0;
        while !a.at_rest() && silent < 96_000 {
            let (mut zl, mut zr) = (vec![0.0f32; 64], vec![0.0f32; 64]);
            a.process(&mut zl, &mut zr);
            silent += 64;
        }
        assert!(a.at_rest(), "{mode:?}: {:?}", a.rest_blocker());
        let mut b = a.clone();
        let (mut zl, mut zr) = (vec![0.0f32; 999], vec![0.0f32; 999]);
        a.process(&mut zl, &mut zr);
        assert!(zl.iter().chain(&zr).all(|v| v.to_bits() == 0));
        b.skip(999);
        let y = noise(5_000, 9);
        let (mut la, mut ra) = (y.clone(), y.clone());
        let (mut lb, mut rb) = (y.clone(), y);
        a.process(&mut la, &mut ra);
        b.process(&mut lb, &mut rb);
        assert!(same(&la, &lb) && same(&ra, &rb), "{mode:?}");
    }
}

#[test]
fn opening_the_tone_strands_nothing_and_the_unit_comes_to_rest() {
    let x = noise(10_000, 22);
    let mut s = with(Mode::JunoII);
    let mut a = unit(&s);
    let (mut l, mut r) = (x.clone(), x);
    drive(&mut a, &mut l, &mut r, || 512);
    s.tone_hz = 20_000.0;
    a.set(&s);
    let mut silent = 0;
    while !a.at_rest() && silent < 480_000 {
        let (mut zl, mut zr) = (vec![0.0f32; 64], vec![0.0f32; 64]);
        a.process(&mut zl, &mut zr);
        silent += 64;
    }
    assert!(a.at_rest(), "{:?}", a.rest_blocker());
}
