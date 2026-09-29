//! Gravis's DSP tests: pitch to half a cent on every engine, the drive that keeps the
//! fundamental, exact harmonics, the sub's phase lock, glide timing, legato and priority,
//! the accent sweep, the filters' bass compensation, the string's combs and stability, and
//! block-size independence.

use super::*;

const SR: f32 = 48_000.0;

fn render(g: &mut Gravis, n: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n);
    let mut left = [0.0f32; 256];
    let mut right = [0.0f32; 256];
    let mut done = 0;
    while done < n {
        let k = (n - done).min(256);
        left[..k].iter_mut().for_each(|x| *x = 0.0);
        right[..k].iter_mut().for_each(|x| *x = 0.0);
        g.process(&mut left[..k], &mut right[..k]);
        out.extend_from_slice(&left[..k]);
        done += k;
    }
    out
}

/// The phase of a Hann-windowed DFT of `x[start..start+win]` at `f` Hz, measured against
/// absolute time (sample 0 of `x`).
fn phase_at(x: &[f32], start: usize, win: usize, f: f64, sr: f64) -> (f64, f64) {
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for i in 0..win {
        let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / win as f64).cos();
        let t = (start + i) as f64;
        let a = -std::f64::consts::TAU * f * t / sr;
        re += x[start + i] as f64 * w * a.cos();
        im += x[start + i] as f64 * w * a.sin();
    }
    (im.atan2(re), (re * re + im * im).sqrt())
}

/// The fundamental's frequency near `nominal`, from the slope of its phase over time.
fn measure_f0(x: &[f32], from: usize, nominal: f64, sr: f64) -> f64 {
    measure_near(
        x,
        from,
        nominal,
        sr,
        ((sr / nominal) * 8.0).round() as usize,
    )
}

/// A partial's frequency near `nominal`, with an analysis window of `win` samples.
fn measure_near(x: &[f32], from: usize, nominal: f64, sr: f64, win: usize) -> f64 {
    let hop = win / 2;
    let mut pts = Vec::new();
    let mut start = from;
    let mut last = None::<f64>;
    let mut unwrap = 0.0;
    while start + win <= x.len() {
        let (p, _) = phase_at(x, start, win, nominal, sr);
        if let Some(l) = last {
            let mut d = p - l;
            while d > std::f64::consts::PI {
                d -= std::f64::consts::TAU;
            }
            while d < -std::f64::consts::PI {
                d += std::f64::consts::TAU;
            }
            unwrap += d;
        }
        last = Some(p);
        pts.push((start as f64, unwrap));
        start += hop;
    }
    let n = pts.len() as f64;
    let mx = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
    let slope = sxy / sxx; // rad per sample
    nominal + slope * sr / std::f64::consts::TAU
}

fn cents(f: f64, want: f64) -> f64 {
    1200.0 * (f / want).log2()
}

/// Goertzel magnitude of `x` at `f` (Hann window).
fn level(x: &[f32], f: f64, sr: f64) -> f64 {
    let (_, m) = phase_at(x, 0, x.len(), f, sr);
    m / (x.len() as f64 / 4.0)
}

fn clean_808() -> GravisSettings {
    let mut s = GravisSettings::default();
    s.e808.pitch_st = 0.0;
    s.e808.knock = 0.0;
    s.e808.click = 0.0;
    s.e808.env.sustain = 1.0;
    s.e808.env.decay_ms = 1000.0;
    s
}

fn clean_synth() -> GravisSettings {
    let mut s = GravisSettings {
        engine: Engine::Synth,
        ..Default::default()
    };
    s.synth.cutoff = 20_000.0;
    s.synth.env_oct = 0.0;
    s.synth.reso = 0.0;
    s.synth.aenv.sustain = 1.0;
    s
}

fn clean_string() -> GravisSettings {
    let mut s = GravisSettings {
        engine: Engine::String,
        ..Default::default()
    };
    s.string.strings = 6;
    s.string.open = string::open_strings(6, 0);
    s.string.frets = 24;
    s.string.buzz = 0.0;
    s.string.sustain_s = 8.0;
    s
}

fn play(s: &GravisSettings, key: u8, secs: f32) -> Vec<f32> {
    let mut g = Gravis::new(SR);
    g.set(s);
    g.note_on(key, 1, 0.8);
    render(&mut g, (secs * SR) as usize)
}

#[test]
fn every_engine_is_in_tune_to_half_a_cent_from_b0_to_c5() {
    for (name, s, settle) in [
        ("808", clean_808(), 0.05),
        ("synth", clean_synth(), 0.05),
        ("string", clean_string(), 0.1),
    ] {
        let mut worst = 0.0f64;
        for key in 23u8..=72 {
            let x = play(&s, key, 1.2);
            let want = voice::hz(key as f64, 440.0);
            let f = measure_f0(&x, (settle * SR) as usize, want, SR as f64);
            let c = cents(f, want);
            worst = worst.max(c.abs());
            assert!(c.abs() < 0.5, "{name} key {key}: {c:+.3} cents");
        }
        eprintln!("{name}: worst pitch error B0–C5 {worst:.4} cents");
    }
}

#[test]
fn the_808_settles_in_tune_after_its_pitch_envelope() {
    // Defaults: +12 st falling in 45 ms, the knock, the click.
    let s = GravisSettings::default();
    let x = play(&s, 33, 1.5);
    let want = voice::hz(33.0, 440.0);
    let f = measure_f0(&x, (0.4 * SR) as usize, want, SR as f64);
    assert!(cents(f, want).abs() < 0.5, "{:+.3}", cents(f, want));
}

#[test]
fn the_string_is_in_tune_on_a_four_string_and_with_buzz() {
    let mut s = GravisSettings {
        engine: Engine::String,
        ..Default::default()
    };
    for key in [28u8, 33, 40, 45, 52, 55, 64] {
        let x = play(&s, key, 1.2);
        let want = voice::hz(key as f64, 440.0);
        let f = measure_f0(&x, (0.2 * SR) as usize, want, SR as f64);
        assert!(
            cents(f, want).abs() < 0.5,
            "key {key}: {:+.3}",
            cents(f, want)
        );
    }
    s.string.stiffness = 2.0;
    let x = play(&s, 40, 1.2);
    let want = voice::hz(40.0, 440.0);
    let f = measure_f0(&x, (0.2 * SR) as usize, want, SR as f64);
    assert!(cents(f, want).abs() < 0.5, "stiff: {:+.3}", cents(f, want));
}

#[test]
fn a_stiff_string_has_sharp_upper_partials() {
    let mut s = clean_string();
    s.string.stiffness = 2.0;
    s.string.fingering = string::Fingering::Lowest; // the open E
    let x = play(&s, 28, 1.0);
    let f0 = voice::hz(28.0, 440.0);
    let from = (0.1 * SR) as usize;
    let win = (16.0 * SR as f64 / f0) as usize;
    let f1 = measure_f0(&x, from, f0, SR as f64);
    let b = string::b_open(28.0) * 2.0;
    let ratio = |n: f64| ((1.0 + b * n * n) / (1.0 + b)).sqrt();
    let f8 = measure_near(&x, from, 8.0 * f0 * ratio(8.0), SR as f64, win);
    let stretch = cents(f8 / 8.0, f1);
    let want = 1200.0 * ratio(8.0).log2();
    eprintln!("8th partial: {stretch:.2} cents sharp; the stiff-string formula: {want:.2}");
    assert!(
        stretch > 2.0,
        "the 8th partial is only {stretch:.2} cents sharp"
    );
    assert!(
        (stretch - want).abs() < 0.3 * want,
        "stretch {stretch:.2} cents, the stiff-string formula says {want:.2}"
    );
}

#[test]
fn the_chebyshev_harmonics_are_exact() {
    let mut s = clean_808();
    s.e808.harmonics = 1.0;
    s.e808.tilt_db = 0.0;
    s.e808.even = 0.0;
    let x = play(&s, 34, 1.2); // 58.27 Hz
    let f0 = voice::hz(34.0, 440.0);
    let tail = &x[(0.1 * SR) as usize..];
    let fund = level(tail, f0, SR as f64);
    for n in 2..=8 {
        let h = level(tail, n as f64 * f0, SR as f64);
        assert!((h / fund - 0.5).abs() < 0.01, "harmonic {n}: {}", h / fund);
    }
    // Nothing between the harmonics (the window's own leakage aside).
    for between in [1.5, 2.5, 4.5, 7.5, 9.5, 12.5] {
        let e = level(tail, between * f0, SR as f64);
        assert!(20.0 * (e / fund).log10() < -90.0, "{between}·f0: {e}");
    }
}

#[test]
fn drive_keeps_the_fundamental_to_a_tenth_of_a_db() {
    let key = 33u8; // A1, 55 Hz
    let f0 = voice::hz(key as f64, 440.0);
    let base = clean_808();
    let clean = play(&base, key, 1.0);
    let tail = |x: &[f32]| x[(0.3 * SR) as usize..].to_vec();
    let fund0 = level(&tail(&clean), f0, SR as f64);
    let mut last = [0.0f64; 9];
    for (i, amount) in [0.25f32, 0.5, 0.75, 1.0].into_iter().enumerate() {
        for kind in [
            out::DriveType::Tube,
            out::DriveType::Tape,
            out::DriveType::Fold,
            out::DriveType::Hard,
        ] {
            let mut s = base;
            s.drive.amount = amount;
            s.drive.kind = kind;
            let x = tail(&play(&s, key, 1.0));
            let fund = level(&x, f0, SR as f64);
            let db = 20.0 * (fund / fund0).log10();
            assert!(
                db.abs() < 0.1,
                "{kind:?} at {amount}: fundamental {db:+.3} dB"
            );
            // DC, through the same window (a plain mean would see the partial period).
            let dc = 0.5 * level(&x, 0.0, SR as f64);
            assert!(20.0 * dc.max(1e-12).log10() < -80.0, "{kind:?} DC {dc}");
            if kind == out::DriveType::Tube {
                // The harmonics 2…8 together, and the third alone, grow with the drive (the
                // second peaks at mid drive, as an asymmetric curve's does).
                let hs: Vec<f64> = (2..=8)
                    .map(|n| level(&x, n as f64 * f0, SR as f64))
                    .collect();
                let total = hs.iter().map(|h| h * h).sum::<f64>().sqrt();
                if i > 0 {
                    assert!(
                        total > last[0],
                        "harmonics 2…8 fell at {amount}: {total} ≤ {}",
                        last[0]
                    );
                    assert!(hs[1] > last[3], "harmonic 3 fell at {amount}");
                }
                last[0] = total;
                last[3] = hs[1];
            }
        }
    }
    // Without the lock the same test sees the fundamental move: it is sensitive.
    let mut s = base;
    s.drive.amount = 1.0;
    s.drive.lock = false;
    let fund = level(&tail(&play(&s, key, 1.0)), f0, SR as f64);
    let db = 20.0 * (fund / fund0).log10();
    assert!(
        db.abs() > 0.5,
        "unlocked drive moved the fundamental only {db:+.3} dB"
    );
}

#[test]
fn neutral_controls_are_bit_exact() {
    let base = GravisSettings::default();
    let reference = play(&base, 36, 0.6);
    let mut s = base;
    s.drive.keytrack_db = -6.0; // with drive at 0, nothing
    s.mono_below_hz = 300.0;
    s.duck.depth_db = 30.0; // with no duck source, nothing
    s.string.pluck_mm = 300.0; // another engine's control
    s.synth.cutoff = 50.0;
    assert_eq!(play(&s, 36, 0.6), reference);
}

#[test]
fn the_sub_stays_locked_to_the_main_phase_through_a_glide() {
    // The 808's main is a sine rising through zero at every wrap of its phase; the sub
    // counts those wraps, so it crosses zero there too — through a one-octave glide, a
    // vibrato and the pitch envelope, on both sub octaves.
    for div in [2u32, 4] {
        let s = Settings808 {
            env: EnvTimes {
                attack_ms: 0.0,
                sustain: 1.0,
                ..Default::default()
            },
            click: 0.0,
            ..Default::default()
        };
        let harm = s.harmonic_levels();
        let mut v = Voice808::default();
        v.start(&s, SR, 1.0, Some(0), 7);
        let (mut main, mut sub) = (Vec::new(), Vec::new());
        let n = 48_000;
        for i in 0..n {
            let t = i as f64 / n as f64;
            let note = 33.0 + 12.0 * t + 0.3 * (std::f64::consts::TAU * 5.0 * t).sin();
            let (m, sb) = v.next(&s, &harm, voice::hz(note, 440.0), SR, div, 0);
            main.push(m);
            sub.push(sb);
        }
        // Every zero crossing of the sub falls on a rising zero crossing of the main.
        let rising = |i: usize| main[i - 1] < 0.0 && main[i] >= 0.0;
        let mut crossings = 0;
        let mut locked = 0;
        for i in 3..n - 3 {
            if (sub[i - 1] < 0.0) != (sub[i] < 0.0) {
                crossings += 1;
                if (i - 1..=i + 1).any(rising) {
                    locked += 1;
                }
            }
        }
        assert!(crossings > 20, "{crossings}");
        assert_eq!(
            locked, crossings,
            "sub ÷{div}: {locked} of {crossings} aligned"
        );
    }
}

#[test]
fn glide_times_are_kept() {
    for shape in [GlideShape::Linear, GlideShape::Exponential, GlideShape::S] {
        let mut s = clean_808();
        s.glide_ms = 100.0;
        s.glide_shape = shape;
        let mut g = Gravis::new(SR);
        g.set(&s);
        g.note_on(28, 1, 1.0);
        render(&mut g, 2400);
        g.note_on(40, 2, 1.0); // held over: legato
        render(&mut g, 4800); // 100 ms
        let p = g.readout().pitch as f64;
        match shape {
            GlideShape::Exponential => assert!(p - 28.0 >= 0.99 * 12.0 - 1e-3, "{p}"),
            _ => assert!((p - 40.0).abs() < 0.01, "{shape:?}: {p}"),
        }
    }
    // Constant rate: twice the interval, twice the time.
    let mut s = clean_808();
    s.glide_ms = 100.0;
    s.glide_rate = true;
    s.glide_shape = GlideShape::Linear;
    let mut g = Gravis::new(SR);
    g.set(&s);
    g.note_on(28, 1, 1.0);
    render(&mut g, 100);
    g.note_on(52, 2, 1.0); // two octaves: 200 ms
    render(&mut g, 4800);
    assert!(
        (g.readout().pitch as f64 - 40.0).abs() < 0.05,
        "{}",
        g.readout().pitch
    );
    render(&mut g, 4800);
    assert!((g.readout().pitch as f64 - 52.0).abs() < 0.01);
}

#[test]
fn touching_notes_slide_inside_the_legato_window_and_attack_outside_it() {
    let s = clean_808();
    // Touching: off and on at the same sample.
    let mut g = Gravis::new(SR);
    g.set(&s);
    g.note_on(28, 1, 1.0);
    render(&mut g, 4800);
    g.note_off(28, 1);
    g.note_on(35, 2, 1.0);
    render(&mut g, 480);
    assert!(g.readout().pitch < 34.9, "touching notes glide");
    assert_eq!(g.readout().age, 4800 + 480, "no new attack");
    // A gap longer than the window: a new note.
    let mut g = Gravis::new(SR);
    g.set(&s);
    g.note_on(28, 1, 1.0);
    render(&mut g, 4800);
    g.note_off(28, 1);
    render(&mut g, 480); // 10 ms > 5 ms
    g.note_on(35, 2, 1.0);
    render(&mut g, 480);
    assert_eq!(g.readout().pitch, 35.0, "a new note jumps");
    assert_eq!(g.readout().age, 480, "and attacks");
}

#[test]
fn legato_returns_to_the_held_note_and_mono_restarts() {
    for (mode, restarts) in [(VoiceMode::Legato, false), (VoiceMode::Mono, true)] {
        let mut s = clean_808();
        s.mode = mode;
        s.glide_trigger = GlideTrigger::Off;
        let mut g = Gravis::new(SR);
        g.set(&s);
        g.note_on(28, 1, 1.0);
        render(&mut g, 1000);
        g.note_on(31, 2, 1.0);
        render(&mut g, 1000);
        g.note_off(31, 2);
        render(&mut g, 100);
        assert_eq!(g.readout().pitch, 28.0, "{mode:?}");
        let age = g.readout().age;
        assert_eq!(age < 1000, restarts, "{mode:?}: age {age}");
    }
    // Low priority ignores a higher note played second.
    let mut s = clean_808();
    s.priority = crate::voices::Priority::Low;
    let mut g = Gravis::new(SR);
    g.set(&s);
    g.note_on(28, 1, 1.0);
    render(&mut g, 100);
    g.note_on(40, 2, 1.0);
    render(&mut g, 4800);
    assert_eq!(g.readout().pitch, 28.0);
}

#[test]
fn a_run_of_accents_climbs() {
    let mut s = clean_synth();
    s.synth.accent = 1.0;
    let mut g = Gravis::new(SR);
    g.set(&s);
    let step = (0.115 * SR) as usize; // sixteenths at 130 BPM
    let mut peaks = Vec::new();
    for i in 0..4 {
        g.note_on(36, i, 1.0);
        let mut peak = 0.0f32;
        for _ in 0..step / 64 {
            render(&mut g, 64);
            peak = peak.max(g.readout().sweep);
        }
        g.note_off(36, i);
        peaks.push(peak);
    }
    for w in peaks.windows(2) {
        assert!(w[1] > w[0] * 1.02, "the sweep does not climb: {peaks:?}");
    }
    // Unaccented notes charge nothing.
    let mut g = Gravis::new(SR);
    g.set(&s);
    for i in 0..4 {
        g.note_on(36, i, 0.5);
        render(&mut g, step);
        g.note_off(36, i);
    }
    assert_eq!(g.readout().sweep, 0.0);
}

#[test]
fn bass_compensation_restores_the_passband_at_any_resonance() {
    let cal = filter::DiodeCal::compute();
    for kind in [filter::FilterKind::Ladder, filter::FilterKind::Diode] {
        for reso in [0.0f32, 0.5, 0.95] {
            let mut f = filter::BassFilter::default();
            // A small DC input (linear region), settled.
            let mut y = 0.0;
            for _ in 0..48_000 {
                y = f.process(kind, 0.01, 2_000.0, reso, 1.0, 0.0, SR, &cal);
            }
            assert!(
                (y / 0.01 - 1.0).abs() < 0.01,
                "{kind:?} reso {reso}: DC gain {}",
                y / 0.01
            );
            // Uncompensated, the ladder loses 1/(1+k).
            if kind == filter::FilterKind::Ladder && reso > 0.9 {
                let mut f = filter::BassFilter::default();
                for _ in 0..48_000 {
                    y = f.process(kind, 0.01, 2_000.0, reso, 0.0, 0.0, SR, &cal);
                }
                assert!(y / 0.01 < 0.3, "{}", y / 0.01);
            }
        }
    }
    // Both ladders resonate where the cutoff says: the loudest of a sweep of sines is
    // within a few per cent of it.
    for kind in [filter::FilterKind::Ladder, filter::FilterKind::Diode] {
        let fc = 1_000.0f32;
        let gain_at = |f: f32| {
            let mut flt = filter::BassFilter::default();
            let n = 9_600;
            let y: Vec<f32> = (0..n)
                .map(|i| {
                    let x = 0.01 * (std::f32::consts::TAU * f * i as f32 / SR).sin();
                    flt.process(kind, x, fc, 0.9, 0.0, 0.0, SR, &cal)
                })
                .collect();
            level(&y[n / 2..], f as f64, SR as f64)
        };
        let peak = (0..60)
            .map(|i| 500.0 * 2f32.powf(i as f32 / 60.0))
            .max_by(|a, b| gain_at(*a).total_cmp(&gain_at(*b)))
            .unwrap();
        assert!(
            (peak / fc - 1.0).abs() < 0.05,
            "{kind:?} resonates at {peak} Hz"
        );
    }
}

#[test]
fn the_pluck_comb_notches_the_harmonics_it_should() {
    // Plucked at the middle: the even harmonics vanish.
    let mut s = clean_string();
    s.string.fingering = string::Fingering::Lowest; // the open E
    s.string.stiffness = 0.0;
    s.string.hardness = 1.0;
    s.string.technique = string::Technique::Pick;
    // Open E on an 800 mm scale plucked at 400 mm (β = ½), heard through the piezo alone:
    // the bridge sees the pluck's comb and nothing else (the magnetic pickups add their
    // own comb and, through the magnet's nonlinearity, even harmonics of their own).
    s.string.pluck_mm = 400.0;
    s.string.scale_mm = 800.0;
    s.string.pu_level = [0.0, 0.0];
    s.string.piezo = 1.0;
    let x = play(&s, 28, 1.0);
    let f0 = voice::hz(28.0, 440.0);
    let t = &x[(0.05 * SR) as usize..(0.5 * SR) as usize];
    let h = |n: usize| level(t, n as f64 * f0, SR as f64);
    for n in [2usize, 4, 6] {
        let db = 20.0 * (h(n) / (0.5 * (h(n - 1) + h(n + 1)))).log10();
        assert!(
            db < -25.0,
            "harmonic {n} only {db:.1} dB under its neighbours"
        );
    }
}

#[test]
fn the_string_is_stable_under_any_settings() {
    let mut rng = voice::Rng(12345);
    for trial in 0..24 {
        let mut s = clean_string();
        let u = |r: &mut voice::Rng| 0.5 + 0.5 * r.bipolar();
        s.string.buzz = u(&mut rng);
        s.string.action_mm = 1.0 + 3.0 * u(&mut rng);
        s.string.stiffness = 2.0 * u(&mut rng);
        s.string.bright = u(&mut rng);
        s.string.technique = string::Technique::from_index(3.0 * u(&mut rng));
        s.string.pluck_mm = 20.0 + 380.0 * u(&mut rng);
        s.string.pu_mm = [20.0 + 280.0 * u(&mut rng), 20.0 + 280.0 * u(&mut rng)];
        s.string.piezo = u(&mut rng);
        s.string.sustain_s = 0.3 + 5.0 * u(&mut rng);
        s.string.mute = u(&mut rng);
        let key = 23 + (u(&mut rng) * 49.0) as u8;
        let mut g = Gravis::new(SR);
        g.set(&s);
        g.note_on(key, 1, 1.0);
        let x = render(&mut g, (1.0 * SR) as usize);
        assert!(x.iter().all(|v| v.is_finite()), "trial {trial}");
        let peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
        let st = &s.string;
        assert!(
            peak < 4.0,
            "trial {trial}: peak {peak} — key {key} {:?} buzz {} action {} stiff {} pluck {} \
             pu {:?} piezo {} mute {} sustain {} bright {}",
            st.technique,
            st.buzz,
            st.action_mm,
            st.stiffness,
            st.pluck_mm,
            st.pu_mm,
            st.piezo,
            st.mute,
            st.sustain_s,
            st.bright
        );
        // Energy never grows after the excitation: the last 100 ms is quieter than the
        // first.
        let e = |a: &[f32]| a.iter().map(|v| v * v).sum::<f32>();
        let first = e(&x[..4800]);
        let last = e(&x[x.len() - 4800..]);
        assert!(last <= first * 1.01, "trial {trial}: {first} → {last}");
    }
}

#[test]
fn a_retrigger_mid_cycle_does_not_click() {
    let step = |declick: f32| {
        let mut s = clean_808();
        s.mode = VoiceMode::Mono;
        s.glide_trigger = GlideTrigger::Off;
        s.e808.env.attack_ms = 0.0;
        s.declick_ms = declick;
        let mut g = Gravis::new(SR);
        g.set(&s);
        g.note_on(33, 1, 1.0);
        let mut x = render(&mut g, 5000); // near a trough of the 55 Hz sine
        g.note_on(33, 2, 1.0);
        x.extend(render(&mut g, 1200));
        let slope = |a: &[f32]| {
            a.windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0f32, f32::max)
        };
        (slope(&x[1000..4990]), slope(&x[4990..5100]))
    };
    // With the crossfade the two voices overlap for 1.5 ms: at most the sum of their
    // slopes, never a step.
    let (steady, around) = step(1.5);
    assert!(
        around <= steady * 3.0,
        "retrigger step {around} vs slope {steady}"
    );
    // Without it the old voice is cut mid-cycle: the test sees the click.
    let (steady, around) = step(0.0);
    assert!(around > steady * 10.0, "{around} vs {steady}");
}

#[test]
fn it_goes_exactly_silent_and_says_so() {
    for s in [GravisSettings::default(), clean_synth(), clean_string()] {
        let mut g = Gravis::new(SR);
        g.set(&s);
        g.note_on(40, 1, 1.0);
        render(&mut g, 4800);
        g.note_off(40, 1);
        let mut left = [0.0f32; 480];
        let mut right = [0.0f32; 480];
        let mut quiet_at = None;
        for block in 0..400 {
            left.iter_mut().for_each(|x| *x = 0.0);
            right.iter_mut().for_each(|x| *x = 0.0);
            let wrote = g.process(&mut left, &mut right);
            if !wrote {
                assert!(left.iter().chain(&right).all(|x| *x == 0.0));
                quiet_at.get_or_insert(block);
            }
        }
        let at = quiet_at.expect("never went idle");
        assert!(at < 300, "{:?}: idle only after {} ms", s.engine, at * 10);
    }
}

#[test]
fn renders_do_not_depend_on_the_block_size() {
    let mut glide = GravisSettings {
        glide_trigger: GlideTrigger::Always,
        clip: true,
        sub_level: 0.5,
        ..Default::default()
    };
    glide.drive.amount = 0.6;
    let mut synth = clean_synth();
    synth.synth.unison = 5;
    synth.synth.width = 1.0;
    synth.synth.filter = filter::FilterKind::Diode;
    synth.synth.reso = 0.7;
    synth.phase = PhaseMode::Random;
    let mut string = clean_string();
    string.string.buzz = 0.8;
    string.string.technique = string::Technique::Slap;
    for s in [glide, synth, string] {
        let run = |block: usize| {
            let mut g = Gravis::new(SR);
            g.set(&s);
            let mut out = Vec::new();
            let events: [(usize, u8, bool); 6] = [
                (0, 36, true),
                (7_000, 43, true),
                (9_000, 36, false),
                (15_000, 43, false),
                (15_000, 40, true),
                (30_000, 40, false),
            ];
            let mut t = 0;
            let total = 48_000;
            let mut next = 0;
            while t < total {
                while next < events.len() && events[next].0 == t {
                    let (_, k, on) = events[next];
                    if on {
                        g.note_on(k, k as u64, 0.9);
                    } else {
                        g.note_off(k, k as u64);
                    }
                    next += 1;
                }
                let until = events.get(next).map_or(total, |e| e.0);
                let n = block.min(until - t).max(1);
                let mut l = vec![0.0; n];
                let mut r = vec![0.0; n];
                g.process(&mut l, &mut r);
                out.extend(l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]));
                t += n;
            }
            out
        };
        let reference = run(512);
        for block in [1usize, 7, 64, 480, 4096] {
            assert!(run(block) == reference, "{:?} at block {block}", s.engine);
        }
    }
}

#[test]
fn the_half_band_rejects_images_and_passes_the_band() {
    let mut os = out::Os4::new();
    let f = 0.2 * SR as f64; // 9.6 kHz at the base rate
    let n = 16_384;
    let mut hi = Vec::with_capacity(4 * n);
    for i in 0..n {
        let x = (std::f64::consts::TAU * f * i as f64 / SR as f64).sin() * 0.5;
        hi.extend(os.up(x).map(|v| v as f32));
    }
    let sr4 = 4.0 * SR as f64;
    let tail = &hi[hi.len() / 2..];
    let want = level(tail, f, sr4);
    for image in [SR as f64 - f, SR as f64 + f, 2.0 * SR as f64 - f] {
        let e = level(tail, image, sr4);
        assert!(
            20.0 * (e / want).log10() < -70.0,
            "image at {image}: {}",
            e / want
        );
    }
    // Up and down again: the level of a 1 kHz tone is kept.
    let mut os = out::Os4::new();
    let f = 1_000.0;
    let y: Vec<f32> = (0..n)
        .map(|i| {
            let x = (std::f64::consts::TAU * f * i as f64 / SR as f64).sin() * 0.5;
            let u = os.up(x);
            os.down(u) as f32
        })
        .collect();
    let g = level(&y[n / 2..], f, SR as f64) / 0.5;
    assert!((20.0 * g.log10()).abs() < 0.05, "{} dB", 20.0 * g.log10());
}

#[test]
fn the_duck_reaches_its_depth_on_the_kick_and_starts_early() {
    let s = out::DuckSettings {
        depth_db: 8.0,
        pre_ms: 2.0,
        hold_ms: 10.0,
        release_ms: 50.0,
        shape: 1,
    };
    let mut d = out::Duck::default();
    let pre = 96; // 2 ms
    d.kick_in(pre);
    let gains: Vec<f32> = (0..4800).map(|_| d.next(&s, SR as f64)).collect();
    assert!(gains[0] <= 1.0 && gains[0] > 0.99);
    let at_kick = 20.0 * gains[pre as usize].log10();
    assert!((at_kick + 8.0).abs() < 0.1, "{at_kick} dB at the kick");
    assert!(gains[1] < 1.0, "it starts moving at once, `pre` early");
    assert_eq!(*gains.last().unwrap(), 1.0, "and comes back");
}

#[test]
fn the_808_keeps_its_peak_and_headroom() {
    let x = play(&GravisSettings::default(), 36, 0.5);
    let peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!(peak > 0.3 && peak < 0.9, "peak {peak}");
}

#[test]
fn szabo_curves_have_their_measured_ends() {
    assert!((synth::detune_curve(1.0) - 1.0).abs() < 0.01);
    assert!(synth::detune_curve(0.0) < 0.01);
    let (c, s) = synth::mix_gains(1.0);
    assert!((c - 0.444).abs() < 0.01 && (s - 0.591).abs() < 0.01);
    // The outer voice at full detune: 1.11 of the note, about 181 cents.
    assert!((synth::outer_cents(7, 1.0) - 180.7).abs() < 1.0);
}

#[test]
fn every_engine_peaks_near_minus_six_dbfs() {
    for (name, s) in [
        ("808", GravisSettings::default()),
        (
            "synth",
            GravisSettings {
                engine: Engine::Synth,
                ..Default::default()
            },
        ),
        (
            "string",
            GravisSettings {
                engine: Engine::String,
                ..Default::default()
            },
        ),
    ] {
        let mut peaks = vec![];
        for key in [28u8, 36, 43, 55] {
            let mut g = Gravis::new(SR);
            g.set(&s);
            g.note_on(key, 1, 1.0);
            let x = render(&mut g, 24_000);
            peaks.push(x.iter().fold(0f32, |m, v| m.max(v.abs())));
        }
        eprintln!("{name}: peaks at full velocity {peaks:?}");
        for p in &peaks {
            assert!(*p > 0.2 && *p < 0.9, "{name}: {peaks:?}");
        }
    }
}
