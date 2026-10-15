use super::*;

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

fn sine(f: f64, n: usize) -> Vec<f32> {
    let w = std::f64::consts::TAU * f / f64::from(SR);
    (0..n)
        .map(|i| crate::math::sin((w * i as f64) % std::f64::consts::TAU) as f32 * 0.5)
        .collect()
}

/// The amplitude of `f` Hz in `x` (Goertzel over a Hann window).
fn level(x: &[f32], f: f64) -> f64 {
    let n = x.len();
    let w = std::f64::consts::TAU * f / f64::from(SR);
    let (mut re, mut im, mut win) = (0.0f64, 0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        let h = 0.5 - 0.5 * crate::math::cos(std::f64::consts::TAU * i as f64 / n as f64);
        let (s, c) = crate::math::sin_cos(w * i as f64);
        re += f64::from(*v) * h * c;
        im += f64::from(*v) * h * s;
        win += h;
    }
    2.0 * (re * re + im * im).sqrt() / win
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn unit(s: &AnulusSettings) -> Anulus {
    let mut a = Anulus::new(SR);
    a.set(s);
    a.reset();
    a
}

fn drive(a: &mut Anulus, l: &mut [f32], r: &mut [f32], mut split: impl FnMut() -> usize) {
    let n = l.len();
    let mut at = 0;
    while at < n {
        let b = split().clamp(1, n - at);
        a.set_transport(&Transport {
            playing: true,
            tempo_bpm: 128.0,
            samples: at as i64,
        });
        a.process(&mut l[at..at + b], &mut r[at..at + b]);
        at += b;
    }
}

fn busy() -> AnulusSettings {
    AnulusSettings {
        freq_hz: 330.0,
        wave: Wave::Triangle,
        lfo_st: 7.0,
        lfo: LfoSettings {
            rate: Rate::Beats(1.0),
            shape: Shape::Triangle,
            ..LfoSettings::default()
        },
        spread: 0.25,
        mix: 0.7,
        out_db: -2.0,
    }
}

#[test]
fn at_mix_zero_it_is_a_wire() {
    let x = noise(20_000, 1);
    let mut s = busy();
    s.mix = 0.0;
    s.out_db = 0.0;
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut unit(&s), &mut l, &mut r, || 333);
    assert!(same(&l, &x) && same(&r, &x));
    let (mut l, mut r) = (x.clone(), x.clone());
    unit(&AnulusSettings::default()).process(&mut l, &mut r);
    assert!(same(&l, &x) && same(&r, &x), "the defaults");
}

#[test]
fn a_tone_becomes_its_sum_and_difference_and_nothing_else() {
    let s = AnulusSettings {
        freq_hz: 300.0,
        mix: 1.0,
        ..AnulusSettings::default()
    };
    let x = sine(1_000.0, 48_000);
    let (mut l, mut r) = (x.clone(), x.clone());
    unit(&s).process(&mut l, &mut r);
    let tail = &l[24_000..];
    let (sum, diff, orig) = (
        level(tail, 1_300.0),
        level(tail, 700.0),
        level(tail, 1_000.0),
    );
    assert!(
        (sum - 0.25).abs() < 0.01 && (diff - 0.25).abs() < 0.01,
        "{sum} {diff}"
    );
    assert!(
        20.0 * crate::math::log10(orig / 0.5) < -80.0,
        "the input is gone: {orig}"
    );
}

#[test]
fn the_square_and_triangle_carriers_alias_far_less_than_naive_ones() {
    // A 5 kHz square's ninth harmonic (45 kHz) folds to 3 kHz at 48 kHz.
    let f = 5_000.0f64;
    let dt = f / f64::from(SR);
    for (wave, alias_hz) in [(Wave::Square, 3_000.0f64), (Wave::Triangle, 3_000.0)] {
        let n = 48_000;
        let mut p = 0.0f64;
        let mut bl = Vec::with_capacity(n);
        let mut naive = Vec::with_capacity(n);
        for _ in 0..n {
            bl.push(carrier(wave, p, dt));
            naive.push(carrier(wave, p, 1e-9));
            p += dt;
            p -= p.floor();
        }
        let (a, b) = (level(&bl, alias_hz), level(&naive, alias_hz));
        let gain = 20.0 * crate::math::log10(b / a);
        assert!(gain > 10.0, "{wave:?}: the alias is {gain:.1} dB lower");
        // The fundamental is kept.
        let fund = level(&bl, f);
        assert!(fund > 0.7, "{wave:?}: fundamental {fund}");
    }
}

#[test]
fn no_dc_leaks_when_a_partial_sits_on_the_carrier() {
    let s = AnulusSettings {
        freq_hz: 440.0,
        mix: 1.0,
        ..AnulusSettings::default()
    };
    let mut x = sine(440.0, 96_000);
    for v in &mut x {
        *v += 0.2; // and an offset in the input
    }
    let (mut l, mut r) = (x.clone(), x.clone());
    unit(&s).process(&mut l, &mut r);
    let tail = &l[48_000..];
    let mean = tail.iter().map(|v| f64::from(*v)).sum::<f64>() / tail.len() as f64;
    assert!(mean.abs() < 1e-3, "DC {mean}");
}

#[test]
fn the_lfo_swings_the_carrier_by_its_semitones() {
    let mut s = busy();
    s.lfo.rate = Rate::Hz(3.0);
    let mut a = unit(&s);
    let (mut lo, mut hi) = (f32::MAX, 0.0f32);
    let mut t = [0.0f32; 64];
    for _ in 0..1_000 {
        let (mut l, mut r) = ([0.1f32; 32], [0.1f32; 32]);
        a.process(&mut l, &mut r);
        a.telemetry(&mut t);
        lo = lo.min(t[T_FREQ]);
        hi = hi.max(t[T_FREQ]);
    }
    let want_lo = 330.0 * crate::math::exp2(-7.0f32 / 12.0);
    let want_hi = 330.0 * crate::math::exp2(7.0f32 / 12.0);
    assert!(
        (lo - want_lo).abs() < 1.0 && (hi - want_hi).abs() < 1.5,
        "{lo} … {hi}"
    );
}

#[test]
fn it_is_the_same_at_every_split() {
    let x = noise(30_000, 4);
    let (mut la, mut ra) = (x.clone(), x.clone());
    drive(&mut unit(&busy()), &mut la, &mut ra, || 512);
    let mut k = 0usize;
    let (mut lb, mut rb) = (x.clone(), x.clone());
    drive(&mut unit(&busy()), &mut lb, &mut rb, || {
        k += 1;
        [1, 7, 64, 4096, 129][k % 5]
    });
    assert!(same(&la, &lb) && same(&ra, &rb));
}

#[test]
fn it_comes_to_rest_and_skipping_is_processing_silence() {
    let x = noise(10_000, 8);
    let (mut l, mut r) = (x.clone(), x.clone());
    let mut a = unit(&busy());
    drive(&mut a, &mut l, &mut r, || 512);
    let mut silent = 0;
    while !a.at_rest() && silent < 480_000 {
        let (mut zl, mut zr) = (vec![0.0f32; 64], vec![0.0f32; 64]);
        a.process(&mut zl, &mut zr);
        silent += 64;
    }
    assert!(a.at_rest(), "{:?}", a.rest_blocker());
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
    assert!(same(&la, &lb) && same(&ra, &rb));
}

/// Live, a unit runs (stopped) long before Play; offline, it starts with Play. The carrier
/// starts again at playback, so both play the same.
#[test]
fn the_carrier_starts_again_at_playback() {
    let sr = 48_000.0f32;
    let s = busy();
    let stopped = Transport {
        playing: false,
        tempo_bpm: 128.0,
        samples: 0,
    };
    let mut live = unit(&s);
    for _ in 0..37 {
        let (mut l, mut r) = (vec![0.0f32; 333], vec![0.0f32; 333]);
        live.set_transport(&stopped);
        live.process(&mut l, &mut r);
    }
    let x = noise(20_000, 31);
    let (mut la, mut ra) = (x.clone(), x.clone());
    drive(&mut live, &mut la, &mut ra, || 512);
    let (mut lb, mut rb) = (x.clone(), x);
    drive(&mut unit(&s), &mut lb, &mut rb, || 512);
    assert!(same(&la, &lb) && same(&ra, &rb));
    let _ = sr;
}
