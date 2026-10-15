use super::*;

const SR: f32 = 48_000.0;

fn sine(n: usize, hz: f32, dbfs: f32) -> Vec<f32> {
    let a = math::db_to_gain(dbfs);
    (0..n)
        .map(|i| a * math::sin(std::f32::consts::TAU * hz * i as f32 / SR))
        .collect()
}

fn noise(n: usize, seed: u32, amp: f32) -> Vec<f32> {
    let mut s = seed.wrapping_mul(2_654_435_761).max(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            amp * ((s as f32 / u32::MAX as f32) * 2.0 - 1.0)
        })
        .collect()
}

/// Feed in editor-frame-sized pieces (800 samples, a 60 Hz frame), as the UI does.
fn feed(a: &mut Analysis, l: &[f32], r: &[f32]) {
    for (cl, cr) in l.chunks(800).zip(r.chunks(800)) {
        a.push(cl, cr, None);
    }
}

/// EBU Tech 3341, the 1 kHz stereo cases: what the analyser reads is what the standard
/// says, within its ±0.1 LU.
#[test]
fn it_reads_the_ebu_tech_3341_vectors() {
    // Cases 1 and 2: a steady stereo sine at −23 and −33 dBFS reads M, S and I at that.
    for db in [-23.0f32, -33.0] {
        let mut a = Analysis::new(SR);
        let x = sine(20 * 48_000, 1_000.0, db);
        feed(&mut a, &x, &x);
        let l = a.loudness();
        for (what, v) in [
            ("momentary", l.momentary),
            ("short-term", l.short_term),
            ("integrated", l.integrated.unwrap()),
        ] {
            assert!((v - f64::from(db)).abs() <= 0.1, "{db} dBFS: {what} {v}");
        }
    }
    // Case 3: −36, −23, −36 dBFS for 10, 60, 10 s → I = −23.
    // Case 4: −72, −36, −23, −36, −72 for 10, 10, 60, 10, 10 s → I = −23.
    // Case 5: −26, −20, −26 for 20, 20.1, 20 s → I = −23.
    let cases: [&[(f32, f32)]; 3] = [
        &[(-36.0, 10.0), (-23.0, 60.0), (-36.0, 10.0)],
        &[
            (-72.0, 10.0),
            (-36.0, 10.0),
            (-23.0, 60.0),
            (-36.0, 10.0),
            (-72.0, 10.0),
        ],
        &[(-26.0, 20.0), (-20.0, 20.1), (-26.0, 20.0)],
    ];
    for (i, case) in cases.iter().enumerate() {
        let mut a = Analysis::new(SR);
        for (db, secs) in case.iter() {
            let x = sine((secs * 48_000.0) as usize, 1_000.0, *db);
            feed(&mut a, &x, &x);
        }
        let v = a.loudness().integrated.unwrap();
        assert!((v + 23.0).abs() <= 0.1, "case {}: I = {v}", i + 3);
    }
}

/// EBU Tech 3342, the loudness range of two levels in turn: 10, 5 and 20 LU (±1).
#[test]
fn it_reads_the_ebu_tech_3342_range() {
    for (a_db, b_db, want) in [
        (-20.0f32, -30.0f32, 10.0),
        (-20.0, -15.0, 5.0),
        (-40.0, -20.0, 20.0),
    ] {
        let mut a = Analysis::new(SR);
        for db in [a_db, b_db] {
            let x = sine(20 * 48_000, 1_000.0, db);
            feed(&mut a, &x, &x);
        }
        let lra = a.loudness().range.unwrap();
        assert!((lra - want).abs() <= 1.0, "{a_db}/{b_db}: LRA {lra}");
    }
}

/// Integrated loudness is worked out at its own rate, not every push: between two
/// computations it holds, and a new one comes after 200 ms of audio.
#[test]
fn integrated_loudness_has_its_own_rate() {
    let mut a = Analysis::new(SR);
    let x = sine(48_000, 1_000.0, -20.0);
    feed(&mut a, &x, &x);
    let n0 = a.loudness_updates();
    feed(&mut a, &x[..4_800], &x[..4_800]);
    assert!(a.loudness_updates() - n0 <= 1, "100 ms: at most one update");
    feed(&mut a, &x, &x);
    assert!(a.loudness_updates() - n0 >= 5, "a second: five updates");
}

/// A true peak between the samples, and peak and RMS per channel.
#[test]
fn it_reads_true_peak_peak_and_rms() {
    let mut a = Analysis::new(SR);
    // A quarter of the sample rate at 45°: samples at 0.5, the waveform at 0.707.
    let l: Vec<f32> = (0..9_600)
        .map(|i| {
            0.5 * std::f32::consts::SQRT_2
                * math::sin(std::f32::consts::FRAC_PI_2 * i as f32 + std::f32::consts::FRAC_PI_4)
        })
        .collect();
    let r = sine(9_600, 1_000.0, -12.0);
    feed(&mut a, &l, &r);
    let lv = a.levels();
    assert!(
        (lv.true_peak_db - -3.01).abs() < 0.1,
        "true peak {}",
        lv.true_peak_db
    );
    assert!(
        (lv.peak_db[0] - -6.02).abs() < 0.05,
        "left peak {}",
        lv.peak_db[0]
    );
    assert!(
        (lv.rms_db[1] - -15.01).abs() < 0.1,
        "right rms {}",
        lv.rms_db[1]
    );
}

/// The phase correlation: +1 for one signal in both channels, −1 for one inverted, about
/// 0 for two unrelated noises.
#[test]
fn it_reads_the_correlation() {
    let x = noise(24_000, 1, 0.5);
    let y = noise(24_000, 2, 0.5);
    let neg: Vec<f32> = x.iter().map(|v| -v).collect();
    for (r, want, tol) in [(&x, 1.0, 1e-3), (&neg, -1.0, 1e-3), (&y, 0.0, 0.1)] {
        let mut a = Analysis::new(SR);
        feed(&mut a, &x, r);
        let c = a.correlation().unwrap();
        assert!((c - want).abs() < tol, "{c} vs {want}");
    }
    let mut a = Analysis::new(SR);
    let z = vec![0f32; 4_800];
    feed(&mut a, &z, &z);
    assert!(a.correlation().is_none(), "silence has none");
}

/// The spectrum: a sine reads its level at its frequency (0 dB for full scale, slope 0),
/// and much lower an octave away.
#[test]
fn the_spectrum_reads_a_sine_at_its_level() {
    for size in FFT_SIZES {
        let mut a = Analysis::new(SR);
        a.set(&SpeculumSettings {
            fft_size: size,
            slope_db: 0.0,
            avg_ms: 0.0,
            ..SpeculumSettings::default()
        });
        let x = sine(48_000, 1_000.0, -12.0);
        feed(&mut a, &x, &x);
        let at = a.spectrum_db(0, 1_000.0);
        assert!((at - -12.0).abs() < 0.6, "{size}: {at} at 1 kHz");
        assert!(a.spectrum_db(0, 2_000.0) < -60.0, "{size}: an octave up");
    }
}

/// The slope tilts the display around 1 kHz: at 4.5 dB an octave, a sine at 4 kHz reads
/// 9 dB higher than without it.
#[test]
fn the_slope_tilts_around_one_kilohertz() {
    let read = |slope: f32| {
        let mut a = Analysis::new(SR);
        a.set(&SpeculumSettings {
            slope_db: slope,
            avg_ms: 0.0,
            ..SpeculumSettings::default()
        });
        let x = sine(48_000, 4_000.0, -20.0);
        feed(&mut a, &x, &x);
        a.spectrum_db(0, 4_000.0)
    };
    assert!((read(4.5) - read(0.0) - 9.0).abs() < 0.05);
}

/// The peak hold keeps a level after it is gone (falling 6 dB a second); the average lets
/// it go.
#[test]
fn the_peak_hold_keeps_what_the_average_lets_go() {
    let mut a = Analysis::new(SR);
    a.set(&SpeculumSettings {
        slope_db: 0.0,
        avg_ms: 30.0,
        ..SpeculumSettings::default()
    });
    let x = sine(24_000, 3_000.0, -6.0);
    feed(&mut a, &x, &x);
    let z = vec![0f32; 24_000];
    feed(&mut a, &z, &z);
    let avg = a.spectrum_db(0, 3_000.0);
    let held = a.peak_db(0, 3_000.0);
    assert!(held > avg + 20.0, "held {held}, average {avg}");
    assert!(held > -10.0, "held {held}");
}

/// The spectrogram adds a row per analysis, and a row reads the sine's band.
#[test]
fn the_spectrogram_adds_rows() {
    let mut a = Analysis::new(SR);
    let x = sine(48_000, 1_000.0, -6.0);
    feed(&mut a, &x, &x);
    assert!(a.spectrogram_rows() > 10);
    let row = a.spectrogram_row(0);
    let band = band_of_hz(1_000.0);
    let loud = row[band];
    assert!(row.iter().enumerate().all(|(i, v)| i == band || *v <= loud));
}

/// Synced to the beat: the window starts on a beat. Clicks on every beat at 120 BPM, the
/// clock given with the samples: the first column of the window holds a click.
#[test]
fn the_scope_starts_its_window_on_a_beat() {
    let mut a = Analysis::new(SR);
    a.set(&SpeculumSettings {
        sync: Sync::Beat,
        scope_beats: 1.0,
        ..SpeculumSettings::default()
    });
    let per_beat = 24_000usize; // 120 BPM at 48 kHz
    let n = 5 * per_beat + 7_000;
    let x: Vec<f32> = (0..n)
        .map(|i| if i % per_beat < 24 { 0.9 } else { 0.0 })
        .collect();
    for (k, c) in x.chunks(800).enumerate() {
        let last = k * 800 + c.len() - 1;
        a.push(c, c, Some((last as f64 / per_beat as f64, 120.0)));
    }
    let mut lo = vec![0f32; 200];
    let mut hi = vec![0f32; 200];
    assert!(a.scope_columns(&mut lo, &mut hi));
    assert!(hi[0] > 0.5, "the window starts on the click");
    assert!(hi[1..].iter().all(|v| *v < 0.5), "one click in one beat");
}

/// Triggered: the window starts at a rising zero crossing.
#[test]
fn the_triggered_scope_starts_on_a_rising_crossing() {
    let mut a = Analysis::new(SR);
    a.set(&SpeculumSettings {
        sync: Sync::Trigger,
        scope_ms: 10.0,
        ..SpeculumSettings::default()
    });
    let x = sine(20_000, 220.0, -6.0);
    a.push(&x, &x, None);
    let mut lo = vec![0f32; 480];
    let mut hi = vec![0f32; 480];
    assert!(a.scope_columns(&mut lo, &mut hi));
    assert!(hi[0].abs() < 0.03 && hi[3] > hi[0], "{} {}", hi[0], hi[3]);
}

/// A sample that is not a number reads as silence and leaves nothing behind.
#[test]
fn a_non_finite_sample_poisons_nothing() {
    let mut a = Analysis::new(SR);
    let mut x = sine(48_000, 1_000.0, -23.0);
    x[100] = f32::NAN;
    x[200] = f32::INFINITY;
    feed(&mut a, &x, &x);
    let l = a.loudness();
    assert!(l.momentary.is_finite() && (l.momentary + 23.0).abs() < 0.2);
    assert!(a.levels().true_peak_db.is_finite());
    assert!(a.correlation().unwrap().is_finite());
    assert!(a.spectrum_db(0, 1_000.0).is_finite());
}

/// The vectorscope keeps mid and side: a signal in the left only is a line at 45°.
#[test]
fn the_vectorscope_keeps_mid_and_side() {
    let mut a = Analysis::new(SR);
    let x = sine(4_800, 300.0, -6.0);
    let z = vec![0f32; 4_800];
    feed(&mut a, &x, &z);
    let pts: Vec<(f32, f32)> = a.vector_points().collect();
    assert!(pts.len() >= 1_000);
    assert!(pts.iter().all(|(m, s)| (m - s).abs() < 1e-6));
}
