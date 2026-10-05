//! What each of Culmen's stages promises, measured.

use super::*;
use crate::terminus::os::reference_true_peak;

/// The magnitude of `hz` in `x`, by a DFT over the whole buffer.
fn level_at(x: &[f32], hz: f32) -> f32 {
    let (mut re, mut im) = (0f64, 0f64);
    for (i, v) in x.iter().enumerate() {
        let w = std::f64::consts::TAU * hz as f64 * i as f64 / FS as f64;
        re += *v as f64 * w.cos();
        im += *v as f64 * w.sin();
    }
    (2.0 * (re * re + im * im).sqrt() / x.len() as f64) as f32
}

fn to_db(x: f32) -> f32 {
    20.0 * x.max(1e-12).log10()
}

/// A band with a slow detector and a hard knee, so a steady tone sits on the static curve.
fn steady(b: &mut BandSettings) {
    b.knee_db = 0.0;
    b.attack_ms = 1.0;
    b.release_ms = 2_000.0;
}

#[test]
fn the_ceiling_holds_in_true_peak_by_a_32x_reference() {
    // The music, then half a second of silence: a buffer cut off at full level would
    // read an overshoot at its end that is the cut, not the limiter.
    let (mut l, mut r) = music(96_000, 21);
    l.extend(std::iter::repeat_n(0.0, 24_000));
    r.extend(std::iter::repeat_n(0.0, 24_000));
    for mode in [Mode::Dynamic, Mode::Split, Mode::Linear] {
        let mut s = busy(mode);
        s.drive_db = 18.0;
        s.limiter.ceiling_db = -1.0;
        let (a, b, _) = run(&s, &l, &r, 512);
        for (ch, x) in [a, b].iter().enumerate() {
            let tp = to_db(reference_true_peak(x, 1.0));
            assert!(tp <= -0.995, "{mode:?} channel {ch}: {tp:.3} dBTP over −1");
        }
    }
}

#[test]
fn the_latency_reported_is_the_latency_measured() {
    for (mode, count) in [(Mode::Dynamic, 3), (Mode::Linear, 3), (Mode::Linear, 1)] {
        for look in [0.0, 2.0, 5.0] {
            let mut s = MaxSettings {
                mode,
                count,
                ..MaxSettings::default()
            };
            s.limiter.lookahead_ms = look;
            let mut x = vec![0.0f32; 12_000];
            x[100] = 0.5;
            let (a, _, lat) = run(&s, &x, &x, 333);
            let at = a.iter().position(|v| *v != 0.0).unwrap();
            assert_eq!(at, 100 + lat, "{mode:?} {count} bands, lookahead {look}");
            assert_eq!(a[at], 0.5);
        }
    }
}

/// Each band works on its own part of the spectrum: the low band squeezing a loud 60 Hz
/// tone leaves a 6 kHz tone where it was, in every crossover mode.
#[test]
fn each_band_acts_on_its_own_range() {
    let n = 96_000;
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / FS;
            0.5 * (TAU * 40.0 * t).sin() + 0.05 * (TAU * 6_000.0 * t).sin()
        })
        .collect();
    for mode in [Mode::Dynamic, Mode::Split, Mode::Linear] {
        let mut s = MaxSettings {
            mode,
            ..MaxSettings::default()
        };
        s.limiter.ceiling_db = 0.0;
        steady(&mut s.bands[LO]);
        s.bands[LO].thr_db = -20.0;
        s.bands[LO].ratio = 4.0;
        s.bands[LO].range_db = 30.0;
        let (a, _, lat) = run(&s, &x, &x, 512);
        let tail = &a[n - 48_000..];
        let src = &x[n - 48_000 - lat..n - lat];
        let low = to_db(level_at(tail, 40.0)) - to_db(level_at(src, 40.0));
        let high = to_db(level_at(tail, 6_000.0)) - to_db(level_at(src, 6_000.0));
        assert!(
            (low + 10.5).abs() < 0.6,
            "{mode:?}: 40 Hz moved {low:.2} dB, the curve says −10.5"
        );
        assert!(high.abs() < 0.1, "{mode:?}: 6 kHz moved {high:.2} dB");
    }
}

/// A band shape for a test case.
type Shape = fn(&mut BandSettings);

/// The full band's two sides, on a steady tone: down above, up below — on its curve.
#[test]
fn the_full_band_follows_its_two_sided_curve() {
    let down: Shape = |b| {
        b.thr_db = -20.0;
        b.ratio = 4.0;
        b.range_db = 30.0;
    };
    let up: Shape = |b| {
        b.thr_lo_db = -30.0;
        b.ratio_lo = 2.0;
    };
    for (name, amp, shape, want) in [("down", 0.5, down, -10.5), ("up", 0.01, up, 5.0)] {
        let mut s = MaxSettings::default();
        s.limiter.ceiling_db = 0.0;
        steady(&mut s.bands[FULL]);
        shape(&mut s.bands[FULL]);
        let x = tone(amp, 1_000.0, 72_000);
        let (a, _, _) = run(&s, &x, &x, 256);
        let got = to_db(peak(&a[48_000..])) - to_db(peak(&x[48_000..]));
        assert!(
            (got - want).abs() < 0.3,
            "{name}: {got:.2} dB, the curve says {want}"
        );
    }
}

#[test]
fn two_bands_split_at_the_first_crossover_and_one_band_is_none() {
    let n = 48_000;
    let x = tone(0.5, 1_000.0, n);
    // Two bands, split at 150 Hz: 1 kHz is the high band's; the mid band does not exist.
    let mut s = MaxSettings {
        count: 2,
        ..MaxSettings::default()
    };
    s.limiter.ceiling_db = 0.0;
    s.bands[MID].ratio = 10.0;
    s.bands[MID].thr_db = -40.0;
    let (a, _, lat) = run(&s, &x, &x, 512);
    assert_delayed(&a, &x, lat, "the mid band is not in a two-band split");
    s.bands[HI].ratio = 10.0;
    s.bands[HI].thr_db = -40.0;
    let (a, _, _) = run(&s, &x, &x, 512);
    assert!(peak(&a[n / 2..]) < 0.2, "the high band takes 1 kHz");
    // One band: no band stage at all, whatever the bands say.
    s.count = 1;
    let (a, _, lat) = run(&s, &x, &x, 512);
    assert_delayed(&a, &x, lat, "no band stage");
}

#[test]
fn solo_hears_one_band_and_a_band_switched_off_does_nothing() {
    let n = 72_000;
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / FS;
            0.3 * (TAU * 60.0 * t).sin() + 0.3 * (TAU * 6_000.0 * t).sin()
        })
        .collect();
    let mut s = MaxSettings {
        mode: Mode::Split,
        ..MaxSettings::default()
    };
    s.limiter.ceiling_db = 0.0;
    s.bands[LO].solo = true;
    let (a, _, _) = run(&s, &x, &x, 512);
    let tail = &a[n - 24_000..];
    assert!(
        to_db(level_at(tail, 6_000.0)) - to_db(0.3) < -40.0,
        "the high tone is gone"
    );
    assert!(
        (to_db(level_at(tail, 60.0)) - to_db(0.3)).abs() < 0.3,
        "the low one stays"
    );
    // Off: a band with everything set does nothing.
    let mut s = MaxSettings::default();
    s.limiter.ceiling_db = 0.0;
    s.bands[LO].thr_db = -40.0;
    s.bands[LO].ratio = 8.0;
    s.bands[LO].on = false;
    let (a, _, lat) = run(&s, &x, &x, 512);
    assert_delayed(&a, &x, lat, "a band switched off");
}

/// Output plus delta is the input, driven and delayed — what Culmen changes, exactly.
#[test]
fn delta_is_everything_culmen_changes() {
    let (l, r) = music(48_000, 4);
    for mode in [Mode::Dynamic, Mode::Linear] {
        let s = busy(mode);
        let (a, b, lat) = run(&s, &l, &r, 512);
        let d = MaxSettings { delta: true, ..s };
        let (da, db_, _) = run(&d, &l, &r, 512);
        let g = 10f32.powf(s.drive_db / 20.0);
        for i in lat + 2_000..l.len() {
            let want = (l[i - lat] * g, r[i - lat] * g);
            assert!((a[i] + da[i] - want.0).abs() < 1e-5, "{mode:?} left {i}");
            assert!((b[i] + db_[i] - want.1).abs() < 1e-5, "{mode:?} right {i}");
        }
        assert!(peak(&da) > 0.05, "{mode:?}: the delta is not empty");
    }
}

#[test]
fn unity_hears_the_processing_without_the_drive() {
    let (l, r) = music(48_000, 8);
    let s = busy(Mode::Dynamic);
    let (a, _, _) = run(&s, &l, &r, 512);
    let u = MaxSettings {
        monitor: Monitor::Unity,
        ..s
    };
    let (ua, _, _) = run(&u, &l, &r, 512);
    let g = 10f32.powf(s.drive_db / 20.0);
    for i in 0..a.len() {
        assert!((ua[i] * g - a[i]).abs() < 1e-5, "sample {i}");
    }
}

#[test]
fn match_levels_the_output_to_the_input_loudness() {
    let (l, r) = music(48_000 * 12, 12);
    let mut s = busy(Mode::Dynamic);
    s.drive_db = 12.0;
    s.monitor = Monitor::Match;
    let (a, b, _) = run(&s, &l, &r, 512);
    let measure = |x: &[f32], y: &[f32]| {
        let mut m = loudness::LoudnessMeter::new(FS);
        m.process(x, y);
        m.integrated().unwrap()
    };
    let from = 48_000 * 6;
    let li = measure(&l[from..], &r[from..]);
    let lo = measure(&a[from..], &b[from..]);
    assert!((li - lo).abs() < 0.5, "in {li:.2} LUFS, out {lo:.2} LUFS");
}

#[test]
fn dither_lands_on_the_grid_and_silence_stays_silent() {
    let (mut l, mut r) = music(24_000 + 4 * 48_000, 2);
    for i in 18_000..l.len() {
        l[i] = 0.0;
        r[i] = 0.0;
    }
    let s = MaxSettings {
        dither: 5,
        ..busy(Mode::Dynamic)
    };
    let (a, _, _) = run(&s, &l, &r, 512);
    let q = 2f32.powi(-15);
    assert!(
        a.iter().all(|v| ((v / q).round() * q - v).abs() < 1e-9),
        "16-bit grid"
    );
    let tail = &a[a.len() - 24_000..];
    assert!(tail.iter().all(|v| *v == 0.0), "dither stops on silence");
}

#[test]
fn telemetry_counts_columns_and_reports_every_stage() {
    let n = 48_000;
    let x: Vec<f32> = (0..n)
        .map(|i| 0.7 * (TAU * 60.0 * i as f32 / FS).sin())
        .collect();
    let mut s = MaxSettings {
        drive_db: 18.0,
        ..MaxSettings::default()
    };
    s.bands[LO].thr_db = -20.0;
    s.bands[LO].ratio = 4.0;
    s.bands[FULL].thr_lo_db = -50.0;
    s.bands[FULL].ratio_lo = 2.0;
    let mut c = Culmen::new(FS);
    c.set(&s);
    c.reset();
    let (mut a, mut b) = (x.clone(), x.clone());
    for (p, q) in a.chunks_mut(480).zip(b.chunks_mut(480)) {
        c.process(p, q);
    }
    let t = c.telemetry();
    assert_eq!(t[telemetry::SEQ], 100.0, "a column per 10 ms");
    assert_eq!(t[telemetry::LATENCY] as usize, c.latency());
    assert_eq!(t[telemetry::RATE], FS);
    let col = telemetry::Column::read(&t, 99);
    assert!(
        col.bands_db[LO] < -3.0,
        "the low band cuts: {:?}",
        col.bands_db
    );
    assert_eq!(col.bands_db[HI], 0.0, "the high band has nothing to move");
    assert!(col.limit_db > 1.0, "the brickwall works: {}", col.limit_db);
    assert!(
        col.output_db <= -0.95 && col.output_db > -3.0,
        "{}",
        col.output_db
    );
    assert!(col.energy_out > 0.0 && col.energy_in > 0.0);
    assert!((col.input_db - to_db(0.7)).abs() < 0.2, "{}", col.input_db);
    assert!(
        t[telemetry::LEVELS + LO] > -6.0,
        "the low band hears the tone"
    );
}

/// A plug-in is activated with its defaults and then given its state; a DAW gives the
/// state at once. Both must sound the same from the first sample: settings given before
/// anything is processed are taken at once, however many came before.
#[test]
fn settings_before_the_first_sample_are_taken_at_once() {
    let (l, r) = music(24_000, 6);
    let s = busy(Mode::Split);
    let (a, b, _) = run(&s, &l, &r, 512);
    let mut c = Culmen::new(FS);
    c.set(&MaxSettings::default());
    c.reset();
    c.set(&s);
    let (mut x, mut y) = (l.clone(), r.clone());
    for (p, q) in x.chunks_mut(512).zip(y.chunks_mut(512)) {
        c.process(p, q);
    }
    assert!(
        x == a && y == b,
        "defaults first, then the state: the same sound"
    );
}
