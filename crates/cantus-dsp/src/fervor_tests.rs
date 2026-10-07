//! Fervor's tests: the null first, then block-size independence, determinism, silence,
//! headroom, the oversamplers' filters, the level match and the harmonics each model makes.

use super::*;
use crate::math;
use crate::spectrum::fft;

const SR: f32 = 48_000.0;

/// A test signal: noise, two sines, and passages past full scale (a float bus can be).
fn signal(n: usize, seed: u32) -> (Vec<f32>, Vec<f32>) {
    let mut s = seed.max(1);
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
        let loud = if (i / 3_000) % 4 == 1 { 1.6 } else { 0.4 };
        let sine = math::sin(t * 220.0 * std::f32::consts::TAU) * 0.5
            + math::sin(t * 5_100.0 * std::f32::consts::TAU) * 0.2;
        l.push((noise() * 0.3 + sine) * loud);
        r.push((noise() * 0.25 + sine * 0.8) * loud);
    }
    (l, r)
}

fn run(f: &mut Fervor, l: &[f32], r: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (l.to_vec(), r.to_vec());
    for (a, b) in l.chunks_mut(block).zip(r.chunks_mut(block)) {
        f.process(a, b);
    }
    (l, r)
}

fn with(s: &SatSettings) -> Fervor {
    let mut f = Fervor::new(SR);
    f.set(s);
    f.reset();
    f
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// Settings that make every part of the path work.
fn busy(model: Model) -> SatSettings {
    SatSettings {
        model,
        drive_db: 18.0,
        character: 0.7,
        bias: 0.3,
        lo_cut_hz: 60.0,
        hi_cut_hz: 12_000.0,
        tone_db: 3.0,
        oversampling: 4,
        mix: 0.8,
        out_db: -2.0,
        curve: drawn(),
        ..SatSettings::default()
    }
}

/// A drawn curve with a soft knee and a kink.
fn drawn() -> CurveSpec {
    let mut c = CurveSpec::default();
    c.points[0] = Some(Point {
        x: 0.3,
        y: 0.5,
        t: 0.0,
    });
    c.points[3] = Some(Point {
        x: 0.7,
        y: 0.8,
        t: 0.6,
    });
    c.hi = 0.9;
    c
}

// ---- the null ------------------------------------------------------------------------

#[test]
fn at_its_defaults_it_is_a_wire_to_the_bit() {
    let (l, r) = signal(20_000, 3);
    for block in [1, 16, 64, 333, 4096] {
        let mut f = Fervor::new(SR);
        let (ol, or) = run(&mut f, &l, &r, block);
        assert!(same(&ol, &l) && same(&or, &r), "block {block}");
        assert_eq!(f.latency(), 0);
    }
}

#[test]
fn without_drive_every_analytic_model_is_a_wire_whatever_its_character_and_bias() {
    let (l, r) = signal(10_000, 5);
    for model in Model::ALL.into_iter().filter(|m| *m != Model::Curve) {
        for (ch, bias) in [(0.0, 0.0), (1.0, -1.0), (0.3, 0.8)] {
            let mut f = with(&SatSettings {
                model,
                character: ch,
                bias,
                mix: 0.4,
                ..SatSettings::default()
            });
            let (ol, or) = run(&mut f, &l, &r, 100);
            assert!(same(&ol, &l) && same(&or, &r), "{model:?} {ch} {bias}");
        }
    }
}

#[test]
fn the_output_gain_alone_is_a_plain_gain() {
    let (l, r) = signal(4_000, 9);
    let mut f = with(&SatSettings {
        out_db: -6.0,
        ..SatSettings::default()
    });
    let (ol, _) = run(&mut f, &l, &r, 64);
    let g = math::powf(10f32, -6.0 / 20.0);
    for (a, b) in ol.iter().zip(&l) {
        assert_eq!(a.to_bits(), (b * g).to_bits());
    }
}

#[test]
fn at_no_mix_it_is_the_dry_signal_delayed_by_its_latency_to_the_bit() {
    let (l, r) = signal(12_000, 11);
    for (factor, minimum) in [(1, false), (2, false), (4, false), (8, false), (8, true)] {
        let mut s = busy(Model::Fuzz);
        s.oversampling = factor;
        s.minimum_phase = minimum;
        s.mix = 0.0;
        s.out_db = 0.0;
        let mut f = with(&s);
        let lat = f.latency();
        let (ol, or) = run(&mut f, &l, &r, 256);
        for i in 0..l.len() {
            let want = if i < lat { 0.0 } else { l[i - lat] };
            assert_eq!(ol[i].to_bits(), want.to_bits(), "{factor}× at {i}");
            let want = if i < lat { 0.0 } else { r[i - lat] };
            assert_eq!(or[i].to_bits(), want.to_bits(), "{factor}× at {i}");
        }
    }
}

#[test]
fn drive_glided_back_to_zero_is_the_wire_again() {
    let (l, r) = signal(40_000, 13);
    let mut f = with(&SatSettings {
        drive_db: 20.0,
        ..SatSettings::default()
    });
    let (mut a, mut b) = (l[..8_000].to_vec(), r[..8_000].to_vec());
    f.process(&mut a, &mut b);
    f.set(&SatSettings::default());
    let (mut a, mut b) = (l[8_000..].to_vec(), r[8_000..].to_vec());
    f.process(&mut a, &mut b);
    // A 20 ms glide lands within 0.005 dB after about eight time constants, then exact.
    let settled = 12_000;
    assert!(same(&a[settled..], &l[8_000 + settled..]));
    assert!(same(&b[settled..], &r[8_000 + settled..]));
}

// ---- independence, determinism, silence, headroom -----------------------------------

#[test]
fn the_output_does_not_depend_on_the_block_size() {
    let (l, r) = signal(9_000, 17);
    let mut cases: Vec<SatSettings> = Model::ALL.into_iter().map(busy).collect();
    for (factor, minimum) in [(1, false), (2, true), (8, false)] {
        let mut s = busy(Model::Tube);
        s.oversampling = factor;
        s.minimum_phase = minimum;
        cases.push(s);
    }
    let mut crush = busy(Model::Clip);
    crush.bits = 6.5;
    crush.rate_hz = 7_000.0;
    crush.channels = Channels::Mid;
    cases.push(crush);
    let mut side = busy(Model::Drive);
    side.channels = Channels::Side;
    side.boost = true;
    cases.push(side);
    for s in &cases {
        let (wl, wr) = run(&mut with(s), &l, &r, 4096);
        for block in [1, 7, 64, 1000] {
            let (ol, or) = run(&mut with(s), &l, &r, block);
            assert!(
                same(&ol, &wl) && same(&or, &wr),
                "{:?} block {block}",
                s.model
            );
        }
    }
}

#[test]
fn a_change_lands_on_the_grid_whatever_the_blocks() {
    // Settings changed between blocks at a grid point give the same output for any
    // block pattern that has a boundary there.
    let (l, r) = signal(6_400, 19);
    let at = 1_600;
    let render = |blocks: &[usize]| {
        let mut f = with(&busy(Model::Tape));
        let (mut ol, mut or) = (l.clone(), r.clone());
        let mut pos = 0;
        let mut changed = false;
        for &b in blocks.iter().cycle() {
            if pos >= l.len() {
                break;
            }
            if pos >= at && !changed {
                assert_eq!(pos, at);
                f.set(&busy(Model::Fold));
                changed = true;
            }
            let end = (pos + b).min(l.len());
            f.process(&mut ol[pos..end], &mut or[pos..end]);
            pos = end;
        }
        (ol, or)
    };
    let (al, ar) = render(&[16]);
    for pattern in [&[32usize][..], &[64], &[400, 1200, 16, 48]] {
        let (bl, br) = render(pattern);
        assert!(same(&al, &bl) && same(&ar, &br), "{pattern:?}");
    }
}

#[test]
fn two_units_with_the_same_settings_make_the_same_sound() {
    let (l, r) = signal(5_000, 23);
    let s = busy(Model::Curve);
    let a = run(&mut with(&s), &l, &r, 128);
    let b = run(&mut with(&s), &l, &r, 128);
    assert!(same(&a.0, &b.0) && same(&a.1, &b.1));
}

#[test]
fn silence_stays_exactly_silent() {
    for model in Model::ALL {
        for factor in [1, 2, 4, 8] {
            let mut s = busy(model);
            s.oversampling = factor;
            s.boost = true;
            s.bias = -0.9;
            let mut f = with(&s);
            let (ol, or) = run(&mut f, &vec![0.0; 3_000], &vec![0.0; 3_000], 512);
            assert!(
                ol.iter().chain(&or).all(|v| *v == 0.0),
                "{model:?} {factor}×"
            );
        }
    }
}

#[test]
fn a_full_scale_square_at_the_most_drive_stays_finite_and_bounded() {
    let n = 6_000;
    let sq: Vec<f32> = (0..n)
        .map(|i| if (i / 40) % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    for model in Model::ALL {
        for factor in [1, 8] {
            for auto in [true, false] {
                let mut s = busy(model);
                s.drive_db = 36.0;
                s.boost = true;
                s.oversampling = factor;
                s.auto_gain = auto;
                s.mix = 1.0;
                s.out_db = 0.0;
                let mut f = with(&s);
                let (ol, or) = run(&mut f, &sq, &sq, 480);
                let peak = ol.iter().chain(&or).fold(0.0f32, |m, v| {
                    assert!(v.is_finite(), "{model:?}");
                    m.max(v.abs())
                });
                // A curve's ceiling is about full scale; the overdrive's low end, taken
                // down before the curve and restored after it, can come out above it.
                assert!(peak < 8.0, "{model:?} {factor}× auto {auto}: {peak}");
            }
        }
    }
}

#[test]
fn no_denormals_in_the_state_after_a_signal_dies_away() {
    let (mut l, mut r) = signal(4_000, 29);
    l.extend(std::iter::repeat_n(0.0, 48_000));
    r.extend(std::iter::repeat_n(0.0, 48_000));
    let mut f = with(&busy(Model::Tube));
    let (ol, or) = run(&mut f, &l, &r, 512);
    let tail = &ol[ol.len() - 1_000..];
    assert!(tail
        .iter()
        .chain(&or[or.len() - 1_000..])
        .all(|v| *v == 0.0 || v.is_normal()));
}

// ---- the oversamplers -------------------------------------------------------------------

/// Up and straight back down: what the filters do on their own.
fn round_trip(factor: usize, minimum: bool, x: &[f32]) -> Vec<f32> {
    let mut o = os::Oversampler::new(factor, minimum);
    x.iter()
        .map(|v| {
            let mut buf = [0.0; 8];
            o.up(*v, &mut buf);
            o.down(&mut buf)
        })
        .collect()
}

fn sine(n: usize, hz: f32, amp: f32) -> Vec<f32> {
    (0..n)
        .map(|i| amp * math::sin(std::f32::consts::TAU * hz * i as f32 / SR))
        .collect()
}

#[test]
fn the_latencies_are_whole_samples_and_measured() {
    assert_eq!(os::latency(1, false), 0);
    assert_eq!(os::latency(2, false), 47);
    assert_eq!(os::latency(4, false), 55);
    assert_eq!(os::latency(8, false), 57);
    // Minimum phase: the all-passes' group delay at low frequencies, rounded.
    assert_eq!(os::latency(2, true), 5);
    assert_eq!(os::latency(4, true), 7);
    assert_eq!(os::latency(8, true), 8);
    for factor in [2, 4, 8] {
        // The impulse response's centre of mass is the latency (linear phase: symmetric).
        let mut x = vec![0.0f32; 512];
        x[10] = 1.0;
        let y = round_trip(factor, false, &x);
        let (mut m, mut w) = (0.0f64, 0.0f64);
        for (i, v) in y.iter().enumerate() {
            m += i as f64 * *v as f64;
            w += *v as f64;
        }
        let centre = m / w - 10.0;
        assert!(
            (centre - os::latency(factor, false) as f64).abs() < 1e-3,
            "{factor}×: {centre}"
        );
        assert!((w - 1.0).abs() < 1e-4, "{factor}× DC gain {w}");
        // Minimum phase: a few samples at most.
        let l = os::latency(factor, true);
        assert!((1..=8).contains(&l), "{factor}× minimum phase reports {l}");
    }
}

#[test]
fn the_pass_band_is_flat_to_20k_for_both_filter_kinds() {
    for factor in [2, 4, 8] {
        for minimum in [false, true] {
            for hz in [100.0, 1_000.0, 10_000.0, 18_000.0, 20_000.0] {
                // RMS, not the sampled peak: a phase shift moves where the samples fall.
                let y = round_trip(factor, minimum, &sine(8_192, hz, 0.5));
                let rms = (y[4_096..].iter().map(|v| v * v).sum::<f32>() / 4_096.0).sqrt();
                let db = 20.0 * math::log10(rms * std::f32::consts::SQRT_2 / 0.5);
                assert!(
                    db.abs() < 0.1,
                    "{factor}× min {minimum} at {hz} Hz: {db:.3} dB"
                );
            }
        }
    }
}

/// Energy of `x` (a whole number of cycles of everything in it is not assumed: a Hann
/// window) at bins not near multiples of `f0`, relative to the total, dB.
fn non_harmonic_db(x: &[f32], f0: f32) -> f32 {
    let n = x.len();
    let mut re: Vec<f32> = x
        .iter()
        .enumerate()
        .map(|(i, v)| v * (0.5 - 0.5 * math::cos(std::f32::consts::TAU * i as f32 / n as f32)))
        .collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    let (mut harm, mut other) = (0.0f64, 0.0f64);
    for k in 1..n / 2 {
        let hz = k as f32 * SR / n as f32;
        let p = (re[k] * re[k] + im[k] * im[k]) as f64;
        let h = (hz / f0).round();
        if h >= 1.0 && (hz - h * f0).abs() < 4.0 * SR / n as f32 {
            harm += p;
        } else {
            other += p;
        }
    }
    10.0 * math::log10(other / harm) as f32
}

#[test]
fn oversampling_keeps_the_aliasing_out() {
    let f0 = 3_217.0;
    let x = sine(16_384, f0, 0.5);
    let mut alias = Vec::new();
    for factor in [1, 2, 4, 8] {
        let s = SatSettings {
            model: Model::Clip,
            drive_db: 18.0,
            character: 0.1,
            oversampling: factor,
            ..SatSettings::default()
        };
        let (y, _) = run(&mut with(&s), &x, &x, 512);
        alias.push(non_harmonic_db(&y[4_096..12_288], f0));
    }
    // Each doubling folds back less; 8× is far below 1×.
    assert!(alias[1] < alias[0] - 10.0, "{alias:?}");
    assert!(alias[3] < alias[0] - 30.0, "{alias:?}");
    assert!(alias[3] < -60.0, "{alias:?}");
}

// ---- the curves ----------------------------------------------------------------------

/// Harmonic levels of a 1 kHz-ish sine (an exact number of cycles in 4096), dB re the
/// fundamental: [h2, h3].
fn harmonics(s: &SatSettings, amp: f32) -> [f32; 2] {
    let n = 4_096;
    let k = 85; // cycles in the window: 996 Hz
    let f0 = k as f32 * SR / n as f32;
    let x = sine(n * 4, f0, amp);
    let (y, _) = run(&mut with(s), &x, &x, 512);
    let mut re = y[n * 3..].to_vec();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    let mag = |b: usize| (re[b] * re[b] + im[b] * im[b]).sqrt();
    let h1 = mag(k);
    [
        20.0 * math::log10(mag(2 * k) / h1),
        20.0 * math::log10(mag(3 * k) / h1),
    ]
}

#[test]
fn symmetric_models_make_odd_harmonics_and_bias_or_asymmetry_even_ones() {
    let base = |model| SatSettings {
        model,
        drive_db: 18.0,
        oversampling: 4,
        ..SatSettings::default()
    };
    let tape = harmonics(&base(Model::Tape), 0.3);
    assert!(tape[1] > -40.0, "tape makes a third: {tape:?}");
    assert!(tape[0] < -90.0, "and no second: {tape:?}");
    let tube = harmonics(&base(Model::Tube), 0.3);
    assert!(tube[0] > -40.0, "tube makes a second: {tube:?}");
    let biased = harmonics(
        &SatSettings {
            bias: 0.5,
            ..base(Model::Tape)
        },
        0.3,
    );
    assert!(biased[0] > -40.0, "a bias makes a second: {biased:?}");
}

#[test]
fn more_drive_more_harmonics() {
    let mut last = -200.0;
    for drive in [3.0, 9.0, 18.0, 30.0] {
        let h = harmonics(
            &SatSettings {
                model: Model::Tape,
                drive_db: drive,
                oversampling: 4,
                ..SatSettings::default()
            },
            0.2,
        );
        assert!(h[1] > last, "{drive} dB: {h:?}");
        last = h[1];
    }
}

#[test]
fn the_level_match_keeps_a_mix_level_signal_at_its_level() {
    for model in Model::ALL {
        let s = SatSettings {
            model,
            drive_db: 24.0,
            character: 0.3,
            curve: drawn(),
            ..SatSettings::default()
        };
        let x = sine(24_000, 997.0, shape::REF_PEAK as f32);
        let (y, _) = run(&mut with(&s), &x, &x, 480);
        let rms = |v: &[f32]| (v.iter().map(|a| a * a).sum::<f32>() / v.len() as f32).sqrt();
        let db = 20.0 * math::log10(rms(&y[12_000..]) / rms(&x[12_000..]));
        // Within a dB: the emphasis and the DC block move it a little from the static
        // curve's measure.
        assert!(db.abs() < 1.0, "{model:?}: {db:.2} dB");
    }
}

// ---- the drawn curve, and changes while running --------------------------------------

#[test]
fn the_drawn_curve_passes_through_its_points_and_mirrors_when_symmetric() {
    let c = drawn();
    assert_eq!(c.eval(0.0), 0.0);
    assert!((c.eval(0.3) - 0.5).abs() < 1e-6);
    assert!((c.eval(0.7) - 0.8).abs() < 1e-6);
    assert!((c.eval(1.0) - 0.9).abs() < 1e-6);
    assert!((c.eval(-0.3) + 0.5).abs() < 1e-6, "mirrored");
    assert!(
        (c.eval(2.0) - 0.9).abs() < 1e-6,
        "the end holds beyond the domain"
    );
    // A straight line with no points is the identity.
    let line = CurveSpec::default();
    for x in [-1.0f32, -0.4, 0.0, 0.25, 1.0] {
        assert!((line.eval(x) - x).abs() < 1e-6);
    }
    // Not symmetric: from (−1, lo).
    let asym = CurveSpec {
        symmetric: false,
        lo: 1.0,
        points: {
            let mut p = [None; 16];
            p[5] = Some(Point {
                x: 0.0,
                y: 0.0,
                t: 0.0,
            });
            p
        },
        ..CurveSpec::default()
    };
    assert!((asym.eval(-0.5) - 0.5).abs() < 1e-6, "a rectifier");
    assert!((asym.eval(0.5) - 0.5).abs() < 1e-6);
    // The table the DSP reads agrees with the curve.
    let mut lut = [0.0f32; LUT_N + 1];
    c.table(&mut lut);
    for i in 0..=200 {
        let x = -1.0 + i as f32 / 100.0;
        let t = shape::lut_read(&lut, x as f64);
        assert!((t - c.eval(x)).abs() < 2e-3, "{x}: {t} vs {}", c.eval(x));
    }
}

#[test]
fn tension_bends_a_segment_up_or_down_and_zero_is_straight() {
    assert_eq!(shape::warp(0.5, 0.0), 0.5);
    assert!(shape::warp(0.5, 0.5) > 0.5);
    assert!(shape::warp(0.5, -0.5) < 0.5);
    for t in [-1.0, -0.3, 0.0, 0.7, 1.0] {
        assert_eq!(shape::warp(0.0, t), 0.0);
        assert!((shape::warp(1.0, t) - 1.0).abs() < 1e-12);
    }
}

/// The largest step between neighbouring samples.
fn max_step(x: &[f32]) -> f32 {
    x.windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max)
}

#[test]
fn a_change_of_model_or_drive_while_playing_does_not_click() {
    let x = sine(24_000, 110.0, 0.3);
    let steady = {
        let (y, _) = run(
            &mut with(&SatSettings {
                drive_db: 12.0,
                ..SatSettings::default()
            }),
            &x,
            &x,
            480,
        );
        max_step(&y[2_000..])
    };
    for next in [
        SatSettings {
            model: Model::Fuzz,
            drive_db: 12.0,
            ..SatSettings::default()
        },
        SatSettings {
            drive_db: 30.0,
            ..SatSettings::default()
        },
        SatSettings {
            model: Model::Curve,
            drive_db: 12.0,
            curve: drawn(),
            ..SatSettings::default()
        },
    ] {
        let mut f = with(&SatSettings {
            drive_db: 12.0,
            ..SatSettings::default()
        });
        let (mut a, mut b) = (x[..12_000].to_vec(), x[..12_000].to_vec());
        f.process(&mut a, &mut b);
        f.set(&next);
        let (mut c, mut d) = (x[12_000..].to_vec(), x[12_000..].to_vec());
        f.process(&mut c, &mut d);
        let mut all = a;
        all.extend_from_slice(&c);
        let around = max_step(&all[11_000..14_000]);
        // A fuzz at 110 Hz has steep edges of its own; what is ruled out is a jump far
        // beyond what either sound makes in steady state.
        let (after, _) = run(&mut with(&next), &x, &x, 480);
        let limit = steady.max(max_step(&after[2_000..])) * 1.5;
        assert!(
            around <= limit,
            "{:?}: {around} against {limit}",
            next.model
        );
    }
}
