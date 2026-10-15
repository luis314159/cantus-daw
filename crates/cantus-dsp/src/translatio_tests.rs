use super::*;

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

fn sine(f: f64, n: usize, sr: f32) -> Vec<f32> {
    let w = std::f64::consts::TAU * f / f64::from(sr);
    (0..n)
        .map(|i| crate::math::sin((w * i as f64) % std::f64::consts::TAU) as f32 * 0.5)
        .collect()
}

/// The amplitude of `f` Hz in `x` (Goertzel over a Blackman–Harris window, for the
/// dynamic range a 60 dB measurement needs).
fn level(x: &[f32], f: f64, sr: f32) -> f64 {
    let n = x.len();
    let w = std::f64::consts::TAU * f / f64::from(sr);
    let (mut re, mut im, mut win) = (0.0f64, 0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        let a = std::f64::consts::TAU * i as f64 / n as f64;
        let h = 0.35875 - 0.48829 * crate::math::cos(a) + 0.14128 * crate::math::cos(2.0 * a)
            - 0.01168 * crate::math::cos(3.0 * a);
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

fn unit(s: &TranslatioSettings, sr: f32) -> Translatio {
    let mut t = Translatio::new(sr);
    t.set(s);
    t.reset();
    t
}

fn shifted(shift: f32, side: Side) -> TranslatioSettings {
    TranslatioSettings {
        shift_hz: shift,
        side,
        mix: 1.0,
        ..TranslatioSettings::default()
    }
}

fn drive(t: &mut Translatio, l: &mut [f32], r: &mut [f32], mut split: impl FnMut() -> usize) {
    let n = l.len();
    let mut at = 0;
    while at < n {
        let b = split().clamp(1, n - at);
        t.set_transport(&Transport {
            playing: true,
            tempo_bpm: 128.0,
            samples: at as i64,
        });
        t.process(&mut l[at..at + b], &mut r[at..at + b]);
        at += b;
    }
}

fn busy() -> TranslatioSettings {
    TranslatioSettings {
        shift_hz: -35.0,
        side: Side::Both,
        feedback: 0.6,
        lfo_hz: 20.0,
        lfo: LfoSettings {
            rate: Rate::Beats(1.0),
            ..LfoSettings::default()
        },
        mix: 0.6,
        out_db: -1.0,
    }
}

#[test]
fn at_mix_zero_it_is_a_wire() {
    let x = noise(20_000, 1);
    let mut s = busy();
    s.mix = 0.0;
    s.out_db = 0.0;
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut unit(&s, 48_000.0), &mut l, &mut r, || 333);
    assert!(same(&l, &x) && same(&r, &x));
    let (mut l, mut r) = (x.clone(), x.clone());
    unit(&TranslatioSettings::default(), 48_000.0).process(&mut l, &mut r);
    assert!(same(&l, &x) && same(&r, &x), "the defaults");
}

#[test]
fn up_and_down_move_a_tone_by_the_shift() {
    let sr = 48_000.0;
    let x = sine(1_000.0, 48_000, sr);
    for (side, want, gone) in [(Side::Up, 1_100.0, 900.0), (Side::Down, 900.0, 1_100.0)] {
        let (mut l, mut r) = (x.clone(), x.clone());
        unit(&shifted(100.0, side), sr).process(&mut l, &mut r);
        let tail = &l[12_000..];
        assert!((level(tail, want, sr) - 0.5).abs() < 0.01, "{side:?}");
        assert!(
            level(tail, gone, sr) < 0.5e-3,
            "{side:?}: the other sideband"
        );
        assert!(level(tail, 1_000.0, sr) < 0.5e-3, "{side:?}: the input");
    }
    // Both: up on the left, down on the right; a negative shift swaps them.
    let (mut l, mut r) = (x.clone(), x.clone());
    unit(&shifted(-100.0, Side::Both), sr).process(&mut l, &mut r);
    assert!(level(&l[12_000..], 900.0, sr) > 0.49 && level(&r[12_000..], 1_100.0, sr) > 0.49);
}

/// The requirement: 60 dB between the sidebands from 20 Hz to 20 kHz at 44.1 and 48 kHz,
/// measured through the running shifter.
#[test]
fn the_other_sideband_is_60_db_down_across_the_band() {
    let mut worst = f64::INFINITY;
    for sr in [44_100.0f32, 48_000.0] {
        let shift = 40.0f64;
        for f in [
            20.0f64, 31.5, 63.0, 125.0, 250.0, 1_000.0, 4_000.0, 10_000.0, 16_000.0, 19_900.0,
        ] {
            let n = sr as usize * 2;
            let x = sine(f, n, sr);
            let (mut l, mut r) = (x.clone(), x.clone());
            unit(&shifted(shift as f32, Side::Up), sr).process(&mut l, &mut r);
            let tail = &l[n / 2..];
            let wanted = level(tail, f + shift, sr);
            let unwanted = level(tail, (f - shift).abs(), sr);
            let db = 20.0 * crate::math::log10(wanted / unwanted.max(1e-12));
            worst = worst.min(db);
            assert!(db >= 60.0, "{sr} Hz, a {f} Hz tone: {db:.1} dB");
        }
    }
    eprintln!("worst sideband rejection measured: {worst:.1} dB");
}

#[test]
fn feedback_shifts_again_on_every_pass() {
    let sr = 48_000.0;
    let x = sine(1_000.0, 48_000, sr);
    let mut s = shifted(150.0, Side::Up);
    s.feedback = 0.5;
    let (mut l, mut r) = (x.clone(), x);
    unit(&s, sr).process(&mut l, &mut r);
    let tail = &l[24_000..];
    let (a, b, c) = (
        level(tail, 1_150.0, sr),
        level(tail, 1_300.0, sr),
        level(tail, 1_450.0, sr),
    );
    assert!(a > b && b > c && c > 0.02, "{a} {b} {c}");
    assert!(
        (b / a - 0.5).abs() < 0.02,
        "each pass at the feedback's level"
    );
}

#[test]
fn the_design_says_how_good_it_is() {
    let t = Translatio::new(48_000.0);
    let (sections, db) = t.design_quality();
    assert!(sections >= 4 && db >= 60.0, "{sections} sections, {db} dB");
}

#[test]
fn it_is_the_same_at_every_split() {
    let x = noise(30_000, 4);
    let (mut la, mut ra) = (x.clone(), x.clone());
    drive(&mut unit(&busy(), 48_000.0), &mut la, &mut ra, || 512);
    let mut k = 0usize;
    let (mut lb, mut rb) = (x.clone(), x.clone());
    drive(&mut unit(&busy(), 48_000.0), &mut lb, &mut rb, || {
        k += 1;
        [1, 7, 64, 4096, 129][k % 5]
    });
    assert!(same(&la, &lb) && same(&ra, &rb));
}

#[test]
fn it_comes_to_rest_and_skipping_is_processing_silence() {
    let x = noise(10_000, 8);
    let (mut l, mut r) = (x.clone(), x.clone());
    let mut s = busy();
    s.feedback = 0.95;
    let mut a = unit(&s, 48_000.0);
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

#[test]
fn the_heaviest_feedback_stays_bounded_whatever_the_shift() {
    let x = noise(96_000, 11);
    for shift in [-500.0f32, -35.0, -3.0, 3.0, 35.0, 2_000.0] {
        let mut s = busy();
        s.shift_hz = shift;
        s.feedback = 0.95;
        s.mix = 1.0;
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut unit(&s, 48_000.0), &mut l, &mut r, || 512);
        let peak = l.iter().chain(&r).fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(
            peak.is_finite() && peak < 10.0,
            "shift {shift}: peak {peak}"
        );
    }
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
    let mut live = unit(&s, 48_000.0);
    for _ in 0..37 {
        let (mut l, mut r) = (vec![0.0f32; 333], vec![0.0f32; 333]);
        live.set_transport(&stopped);
        live.process(&mut l, &mut r);
    }
    let x = noise(20_000, 31);
    let (mut la, mut ra) = (x.clone(), x.clone());
    drive(&mut live, &mut la, &mut ra, || 512);
    let (mut lb, mut rb) = (x.clone(), x);
    drive(&mut unit(&s, 48_000.0), &mut lb, &mut rb, || 512);
    assert!(same(&la, &lb) && same(&ra, &rb));
    let _ = sr;
}
