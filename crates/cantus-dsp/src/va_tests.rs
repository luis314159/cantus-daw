//! The virtual-analog library's own tests: the arithmetic, then each filter's
//! self-oscillation pitch, bass loss, small-signal shape and robustness, then the
//! oscillators' pitch and aliasing floor, then drift, cards and envelopes.

use super::math::*;
use super::testkit::*;
use super::*;

// ---- arithmetic --------------------------------------------------------------------------

#[test]
fn our_own_functions_match_the_library_ones() {
    let mut worst = [0f64; 4];
    for i in -4000..=4000 {
        let x = i as f32 * 0.001;
        worst[0] = worst[0].max((tanh(x) as f64 - (x as f64).tanh()).abs());
    }
    for i in 1..4999 {
        let r = i as f32 * 1e-4;
        let want = (std::f64::consts::PI * r as f64).tan();
        worst[1] = worst[1].max(((tan_pi(r) as f64 - want) / want).abs());
    }
    for i in -2000..=2000 {
        let x = i as f32 * 0.01;
        let want = 2f64.powf(x as f64);
        worst[2] = worst[2].max(((exp2(x) as f64 - want) / want).abs());
    }
    for i in -3000..=3000 {
        let p = i as f32 * 0.001;
        worst[3] = worst[3]
            .max((sin_turns(p) as f64 - (2.0 * std::f64::consts::PI * p as f64).sin()).abs());
    }
    println!(
        "worst error: tanh {:.1e}, tan {:.1e} (relative), exp2 {:.1e} (relative), sin {:.1e}",
        worst[0], worst[1], worst[2], worst[3]
    );
    assert!(worst[0] < 1.5e-3, "tanh {}", worst[0]);
    assert!(worst[1] < 2e-6, "tan {}", worst[1]);
    assert!(worst[2] < 5e-7, "exp2 {}", worst[2]);
    assert!(worst[3] < 5e-7, "sin {}", worst[3]);
    assert_eq!(tanh_gain(0.0), 1.0);
    assert!((tanh_gain(10.0) - 0.1).abs() < 1e-6);
}

// ---- filters -----------------------------------------------------------------------------

fn settings(model: Model, reso: f32) -> FilterSettings {
    FilterSettings {
        model,
        reso,
        ..Default::default()
    }
}

/// A filter left alone after a kick: what it rings at, over the last half second of three.
fn ring(model: Model, fc: f32, fs: f32, reso: f32, kick: f32, solve: Solve) -> Vec<f32> {
    let c = FilterCoefs::new(&settings(model, reso));
    let g = tan_pi(fc / fs);
    let mut f = VaFilter::default();
    let n = (fs * 3.0) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let x = if i == 0 { kick } else { 0.0 };
        out.push(f.tick(x, g, &c, solve));
    }
    out.split_off(n - n / 6)
}

#[test]
fn every_model_self_oscillates_on_its_cutoff() {
    // Resonance 1.05, at each quality: Draft (48 kHz, one pass), Good (96 kHz, two
    // passes), Best (192 kHz, converged). The saturation pulls the poles down as the
    // oscillation grows; the tuning tracks that sag and trims it back.
    let qualities = [
        ("draft", 48_000.0f32, Solve::Once, 2500.0f32),
        ("good", 96_000.0, Solve::Twice, 10_000.0),
        ("best", 192_000.0, Solve::Converge, 10_000.0),
    ];
    for (name, fs, solve, top) in qualities {
        let mut worst = 0f64;
        for model in Model::ALL {
            let mut line = format!("{name:>5} {:>14}:", filter::describe(model));
            for fc in [50.0f32, 110.0, 440.0, 1000.0, 2500.0, 5000.0, 10_000.0] {
                if fc > top {
                    continue;
                }
                let y = ring(model, fc, fs, 1.05, 0.5, solve);
                let f =
                    frequency(&y, fs).unwrap_or_else(|| panic!("{model:?} at {fc} Hz is silent"));
                let err = cents(f, fc as f64);
                line.push_str(&format!(" {fc:>5}Hz {err:+5.1}¢"));
                worst = worst.max(err.abs());
                assert!(
                    err.abs() <= 10.0,
                    "{name}: {model:?} self-oscillates {err:+.2} cents off {fc} Hz"
                );
                let peak = y.iter().fold(0f32, |m, v| m.max(v.abs()));
                assert!(peak < 2.0, "{model:?} at {fc} Hz: amplitude {peak}");
            }
            println!("{line}");
        }
        println!("{name}: worst self-oscillation error {worst:.2} cents");
    }
}

#[test]
fn at_the_threshold_the_ladder_rings_exactly_on_its_cutoff() {
    // Resonance 1.0 with a small kick: the loop is linear and on the edge, so the ZDF
    // ladder rings at the cutoff to within cents all the way up.
    let fs = 96_000.0;
    for fc in [30.0f32, 100.0, 1000.0, 5000.0, 12_000.0] {
        let y = ring(Model::Ladder, fc, fs, 1.0, 1e-4, Solve::Twice);
        let f = frequency(&y, fs).expect("rings");
        let err = cents(f, fc as f64);
        assert!(err.abs() <= 3.0, "ladder at {fc} Hz: {err:+.3} cents");
    }
}

/// Small-signal gain (dB) of a model at `f` Hz: a sine at −60 dBFS, read by correlation.
fn gain_db(s: &FilterSettings, fc: f32, f: f32, fs: f32) -> f64 {
    let c = FilterCoefs::new(s);
    let g = tan_pi(fc / fs);
    let mut filt = VaFilter::default();
    let a = 1e-3;
    let settle = (fs * 0.25) as usize;
    let periods = ((f * 0.25).ceil() as usize).max(8);
    let n = (periods as f64 * fs as f64 / f as f64).round() as usize;
    let (mut c_re, mut c_im) = (0.0f64, 0.0f64);
    for i in 0..settle + n {
        let ph = 2.0 * std::f64::consts::PI * f as f64 * i as f64 / fs as f64;
        let y = filt.tick(a * ph.sin() as f32, g, &c, Solve::Converge) as f64;
        if i >= settle {
            c_re += y * ph.sin();
            c_im += y * ph.cos();
        }
    }
    let amp = 2.0 * (c_re * c_re + c_im * c_im).sqrt() / n as f64;
    20.0 * (amp / a as f64).log10()
}

#[test]
fn resonance_costs_the_four_poles_their_bass_and_compensation_restores_it() {
    let fs = 96_000.0;
    for reso in [0.0f32, 0.25, 0.5, 0.75, 0.95] {
        let mut s = settings(Model::Ladder, reso);
        let k = 4.0 * reso as f64;
        let want = -20.0 * (1.0 + k).log10();
        let got = gain_db(&s, 1000.0, 100.0, fs);
        // 100 Hz is a decade under the cutoff: the four poles cost ~0 there.
        assert!(
            (got - want).abs() < 0.3,
            "reso {reso}: {got:.2} dB, want {want:.2}"
        );
        s.bass_comp = 1.0;
        let comp = gain_db(&s, 1000.0, 100.0, fs);
        assert!(comp.abs() <= 0.5, "reso {reso} compensated: {comp:.2} dB");
        let svf = gain_db(&settings(Model::Svf, reso), 1000.0, 100.0, fs);
        assert!(
            svf.abs() <= 0.2,
            "the SVF keeps its bass: {svf:.2} dB at reso {reso}"
        );
    }
}

#[test]
fn the_small_signal_response_matches_the_drawn_one() {
    let fs = 96_000.0;
    let cases = [
        (Model::Ladder, 0.5, 4u8, 0.0, 0.0),
        (Model::Ladder, 0.3, 2, 0.0, 0.5),
        (Model::Cascade, 0.6, 4, 0.0, 0.0),
        (Model::Diode, 0.5, 4, 0.0, 0.0),
        (Model::Svf, 0.5, 4, 0.0, 0.0),
        (Model::Svf, 0.4, 4, 0.5, 0.0),
        (Model::Svf, 0.4, 4, 1.0, 0.0),
        (Model::SallenKey, 0.6, 4, 0.0, 0.0),
    ];
    for (model, reso, poles, morph, comp) in cases {
        let s = FilterSettings {
            model,
            reso,
            poles,
            morph,
            bass_comp: comp,
            ..Default::default()
        };
        for f in [100.0f32, 500.0, 1200.0, 3000.0, 9000.0, 25_000.0, 38_000.0] {
            let want = response::magnitude_db(&s, 1200.0, f, fs) as f64;
            if want < -80.0 {
                continue; // below what a −60 dBFS sine can show
            }
            let got = gain_db(&s, 1200.0, f, fs);
            assert!(
                (got - want).abs() < 0.1,
                "{model:?} reso {reso} poles {poles} morph {morph} at {f} Hz: {got:.2} dB, drawn {want:.2} dB"
            );
        }
    }
}

#[test]
fn filters_stay_finite_and_bounded_under_random_modulation() {
    let fs = 96_000.0;
    for model in Model::ALL {
        for rev in [Revision::Clean, Revision::Rough, Revision::Pushed] {
            let mut rng = Noise::new(model as u64 * 7 + rev as u64);
            let mut f = VaFilter::default();
            let mut peak = 0f32;
            for i in 0..200_000 {
                let fc = 20.0 * exp2(rng.next().abs() * 10.0);
                let s = FilterSettings {
                    model,
                    reso: 1.2 * rng.next().abs(),
                    drive: db_gain(24.0 * rng.next().abs()),
                    bass_comp: 1.0,
                    rev,
                    ..Default::default()
                };
                let c = FilterCoefs::new(&s);
                let x = if i % 3 == 0 {
                    4.0 * rng.next()
                } else {
                    rng.next()
                };
                let y = f.tick(x, tan_pi(fc / fs), &c, Solve::Twice);
                assert!(y.is_finite(), "{model:?} {rev:?} at sample {i}");
                peak = peak.max(y.abs());
            }
            assert!(peak < 16.0, "{model:?} {rev:?} peaked at {peak} (+24 dBFS)");
        }
    }
}

#[test]
fn distortion_rises_with_level() {
    // A 200 Hz sine into the ladder at 1 kHz: the harmonics grow with the level, never
    // shrink — the mixer's levels are a timbre control.
    let fs = 96_000.0;
    let c = FilterCoefs::new(&settings(Model::Ladder, 0.3));
    let g = tan_pi(1000.0 / fs);
    let mut last = f64::NEG_INFINITY;
    for db in [-40.0f32, -20.0, -6.0, 0.0, 6.0, 12.0] {
        let a = db_gain(db);
        let mut f = VaFilter::default();
        let n = 1 << 15;
        let y: Vec<f32> = (0..n + 8192)
            .map(|i| {
                let x = a * (2.0 * std::f32::consts::PI * 187.5 * i as f32 / fs).sin();
                f.tick(x, g, &c, Solve::Converge)
            })
            .skip(8192)
            .collect();
        let mag = spectrum(&y);
        let bin = |h: f64| (h * 187.5 * n as f64 / fs as f64).round() as usize;
        let fund = mag[bin(1.0)];
        let harm: f64 = (2..8)
            .map(|h| mag[bin(h as f64)].powi(2))
            .sum::<f64>()
            .sqrt();
        let thd = 20.0 * (harm / fund).log10();
        assert!(
            thd >= last - 0.5,
            "THD fell from {last:.1} to {thd:.1} dB at {db} dBFS"
        );
        last = thd;
    }
    assert!(
        last > -30.0,
        "+12 dBFS into the ladder is only {last:.1} dB THD"
    );
}

#[test]
fn converging_takes_few_passes_and_keeps_its_cap() {
    let fs = 192_000.0;
    let mut hist = [0usize; 9];
    let mut rng = Noise::new(99);
    let c = FilterCoefs::new(&FilterSettings {
        model: Model::Ladder,
        reso: 1.2,
        drive: 4.0,
        ..Default::default()
    });
    let mut f = VaFilter::default();
    for _ in 0..192_000 {
        f.tick(rng.next(), tan_pi(2000.0 / fs), &c, Solve::Converge);
        hist[f.last_passes as usize] += 1;
    }
    let mean: f64 = hist
        .iter()
        .enumerate()
        .map(|(p, n)| p as f64 * *n as f64)
        .sum::<f64>()
        / 192_000.0;
    println!("passes (noise at full resonance, 4×): mean {mean:.2}, histogram {hist:?}");
    assert!(mean < 6.0);
}
