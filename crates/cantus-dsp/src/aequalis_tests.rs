//! Aequalis's DSP tests: the matched design against the analog prototype, the three
//! processing modes against their promises (a wire when flat, the analog curve in natural
//! phase, the drawn curve in linear phase, the latency they report), dynamics in both
//! directions and from a key, the audition, the auto gain, and block-size independence
//! everywhere. `EQ_REPORT=1 cargo test -p cantus-dsp aequalis -- --nocapture` prints the
//! measured numbers the assertions are drawn from.

use super::*;
use crate::math;

fn report() -> bool {
    std::env::var("EQ_REPORT").is_ok()
}

fn proto(shape: EqShape, g: f64, q: f64) -> Proto {
    let a = math::powf(10f64, g / 40.0);
    let sa = a.sqrt();
    match shape {
        EqShape::Bell => Proto {
            b: [1.0, a / q, 1.0],
            a: [1.0, 1.0 / (a * q), 1.0],
        },
        EqShape::LowShelf => Proto {
            b: [a * a, a * sa / q, a],
            a: [1.0, sa / q, a],
        },
        EqShape::HighShelf => Proto {
            b: [a, a * sa / q, a * a],
            a: [a, sa / q, 1.0],
        },
        _ => unreachable!(),
    }
}

fn analog_db_of(shape: EqShape, f0: f64, g: f64, q: f64, hz: f64) -> f64 {
    10.0 * math::log10(proto(shape, g, q).mag2(hz / f0))
}

/// A band's analog response at `w` rad/sample, complex.
fn analog_c(b: &BandSettings, sr: f32, w: f64) -> (f64, f64) {
    let mut protos = [Proto::UNIT; MAX_SECTIONS];
    let mut kinds = [Kind::Matched; MAX_SECTIONS];
    let Some((n, w0)) = prototypes(
        b.shape,
        sr,
        b.freq,
        b.gain_db,
        b.q,
        b.slope,
        &mut protos,
        &mut kinds,
    ) else {
        return (1.0, 0.0);
    };
    let (mut re, mut im) = (1.0, 0.0);
    for p in &protos[..n] {
        let (a, bb) = p.response(w / w0);
        (re, im) = (re * a - im * bb, re * bb + im * a);
    }
    (re, im)
}

/// The DTFT of `h` at `w` rad/sample.
fn dtft(h: &[f32], w: f64) -> (f64, f64) {
    let (mut re, mut im) = (0.0, 0.0);
    for (n, x) in h.iter().enumerate() {
        let a = w * n as f64;
        re += *x as f64 * math::cos(a);
        im -= *x as f64 * math::sin(a);
    }
    (re, im)
}

fn db(c: (f64, f64)) -> f64 {
    10.0 * math::log10((c.0 * c.0 + c.1 * c.1).max(1e-30))
}

fn eq_with(bands: &[BandSettings], opts: Options, sr: f32) -> Aequalis {
    let mut eq = Aequalis::new(sr);
    eq.set_options(opts);
    let mut all = [BandSettings::default(); MAX_BANDS];
    all[..bands.len()].copy_from_slice(bands);
    eq.set(&all, opts.output_db);
    eq
}

/// The response to an impulse on both channels (or on the left only), `n` samples.
fn impulse(eq: &mut Aequalis, n: usize, left_only: bool) -> (Vec<f32>, Vec<f32>) {
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    l[0] = 1.0;
    if !left_only {
        r[0] = 1.0;
    }
    for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        eq.process(cl, cr);
    }
    (l, r)
}

fn noise(n: usize, seed: u32, amp: f32) -> Vec<f32> {
    let mut s = seed.max(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            amp * ((s as f32 / u32::MAX as f32) * 2.0 - 1.0)
        })
        .collect()
}

fn tone(n: usize, hz: f32, amp: f32, sr: f32) -> Vec<f32> {
    (0..n)
        .map(|i| amp * math::sin(std::f32::consts::TAU * hz * i as f32 / sr))
        .collect()
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

fn bell(freq: f32, gain: f32, q: f32) -> BandSettings {
    BandSettings {
        active: true,
        shape: EqShape::Bell,
        freq,
        gain_db: gain,
        q,
        ..BandSettings::default()
    }
}

// ---- the matched design ---------------------------------------------------------------

/// The point of the whole design: a bell high up does not cramp. Every gain shape tracks
/// its analog prototype from 20 Hz to 20 kHz at 48 kHz at least as well as the bilinear
/// transform, and far better near Nyquist, where the bilinear one is several dB out.
#[test]
fn matched_sections_follow_the_analog_curve_up_to_nyquist() {
    let sr = 48_000.0f32;
    for (shape, f0, g, q) in [
        (EqShape::Bell, 1_000.0, 9.0, 1.0),
        (EqShape::Bell, 16_000.0, 9.0, 2.0),
        (EqShape::Bell, 12_000.0, -12.0, 0.7),
        (EqShape::LowShelf, 200.0, -9.0, 0.71),
        (EqShape::HighShelf, 10_000.0, 6.0, 0.71),
        (EqShape::HighShelf, 18_000.0, 6.0, 0.71),
        (EqShape::HighShelf, 6_000.0, 6.0, 0.71),
        (EqShape::HighShelf, 8_000.0, -9.0, 0.71),
        (EqShape::HighShelf, 12_000.0, 6.0, 0.71),
        (EqShape::HighShelf, 14_000.0, -9.0, 0.71),
        (EqShape::HighShelf, 16_000.0, 6.0, 0.71),
        (EqShape::HighShelf, 12_000.0, 6.0, 2.0),
        (EqShape::LowShelf, 12_000.0, 6.0, 0.71),
        (EqShape::Bell, 8_000.0, 6.0, 1.0),
        (EqShape::Bell, 18_000.0, -6.0, 4.0),
    ] {
        let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
        let n = design(shape, sr, f0 as f32, g as f32, q as f32, 12.0, &mut c);
        assert_eq!(n, 1);
        // (Measured when this was written, ours vs bilinear of the same prototype, worst dB
        // over 20 Hz–20 kHz at 48 kHz: bell 16 kHz Q2 0.61 vs 4.27; bell 12 kHz Q0.7
        // −12 dB 2.44 vs 5.17; high shelf 14 kHz 0.27 vs 1.80; high shelf 18 kHz uses the
        // bilinear design, 1.18, where matching gave 1.81.)
        let rbj = bilinear(proto(shape, g, q), 2.0 * PI * f0 / sr as f64);
        let (mut worst, mut worst_rbj) = (0.0f64, 0.0f64);
        let mut hz = 20.0f64;
        while hz <= 20_000.0 {
            let a = analog_db_of(shape, f0, g, q, hz);
            worst = worst.max((c[0].magnitude_db(sr, hz as f32) as f64 - a).abs());
            worst_rbj = worst_rbj.max((rbj.magnitude_db(sr, hz as f32) as f64 - a).abs());
            hz *= 1.05;
        }
        if report() {
            eprintln!(
                "CASE {shape:?} {f0} Hz {g} dB q {q}: ours {worst:.2}, bilinear {worst_rbj:.2}"
            );
            continue;
        }
        assert!(
            worst <= worst_rbj + 1e-3,
            "{shape:?} {f0} Hz {g} dB q {q}: ours {worst:.2} dB, bilinear {worst_rbj:.2} dB"
        );
        if worst_rbj <= 2.0 {
            assert!(
                worst < 1.5,
                "{shape:?} {f0} Hz {g} dB q {q}: off by {worst:.2} dB"
            );
            if f0 < sr as f64 * 0.25 && q <= 1.0 {
                assert!(
                    worst < 0.5,
                    "{shape:?} {f0} Hz {g} dB q {q}: off by {worst:.2} dB"
                );
            }
        } else {
            assert!(
                worst * 2.0 < worst_rbj,
                "{shape:?} {f0} Hz {g} dB q {q}: ours {worst:.2} dB, bilinear {worst_rbj:.2} dB"
            );
        }
    }
}

#[test]
fn cuts_have_the_slope_they_say() {
    let sr = 48_000.0;
    for slope in [6.0f32, 12.0, 18.0, 24.0, 48.0, 96.0] {
        let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
        let n = design(
            EqShape::LowCut,
            sr,
            400.0,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            slope,
            &mut c,
        );
        assert_eq!(n, (slope / 12.0).ceil() as usize);
        let at = |hz: f32| c[..n].iter().map(|s| s.magnitude_db(sr, hz)).sum::<f32>();
        // −3 dB at the corner (Butterworth), and `slope` per octave well below it.
        assert!(
            (at(400.0) + 3.01).abs() < 0.3,
            "slope {slope}: corner {}",
            at(400.0)
        );
        let per_octave = at(50.0) - at(25.0);
        assert!(
            (per_octave - slope).abs() < slope * 0.08 + 0.3,
            "slope {slope}: measured {per_octave} dB/oct"
        );
        assert!(at(8_000.0).abs() < 0.05, "passband");
    }
}

#[test]
fn a_flat_or_empty_eq_is_a_wire_to_the_bit() {
    let mut eq = Aequalis::new(48_000.0);
    let mut bands = [BandSettings::default(); MAX_BANDS];
    bands[0] = BandSettings {
        active: true,
        freq: 3_000.0,
        gain_db: 0.0,
        ..BandSettings::default()
    };
    eq.set(&bands, 0.0);
    assert!(!eq.is_active());
    let src: Vec<f32> = (0..4096)
        .map(|i| ((i * 7919) % 1000) as f32 / 500.0 - 1.0)
        .collect();
    let (mut l, mut r) = (src.clone(), src.clone());
    eq.process(&mut l, &mut r);
    assert_eq!(l, src);
    assert_eq!(r, src);
}

#[test]
fn processing_matches_the_drawn_curve() {
    let sr = 48_000.0;
    let mut bands = [BandSettings::default(); MAX_BANDS];
    bands[0] = bell(2_000.0, 6.0, 1.0);
    let mut eq = Aequalis::new(sr);
    eq.set(&bands, 0.0);
    let n = 48_000;
    let mut l = tone(n, 2_000.0, 1.0, sr);
    let mut r = l.clone();
    eq.process(&mut l, &mut r);
    let measured = 20.0 * math::log10(rms(&l[n / 2..]) / 0.5f32.sqrt());
    let drawn = Aequalis::response_db(&bands, sr, 2_000.0);
    assert!((measured - drawn).abs() < 0.1, "{measured} vs {drawn}");
}

/// A dynamic band cuts a loud signal in its band and leaves a quiet one alone, and the
/// result does not depend on how the stream was cut into blocks.
#[test]
fn dynamic_bands_move_with_the_level_and_ignore_block_size() {
    let sr = 48_000.0;
    let mut bands = [BandSettings::default(); MAX_BANDS];
    bands[0] = BandSettings {
        dynamic: true,
        threshold_db: -30.0,
        range_db: -9.0,
        ratio: 8.0,
        ..bell(1_000.0, 0.001, 2.0)
    };
    let run = |src: &[f32], block: usize| -> (Vec<f32>, f32) {
        let mut eq = Aequalis::new(sr);
        eq.set(&bands, 0.0);
        let (mut l, mut r) = (src.to_vec(), src.to_vec());
        for (cl, cr) in l.chunks_mut(block).zip(r.chunks_mut(block)) {
            eq.process(cl, cr);
        }
        let d = eq.dynamic_deltas()[0];
        (l, d)
    };
    let (loud, d_loud) = run(&tone(24_000, 1_000.0, 0.5, sr), 512);
    assert!(d_loud < -8.0, "a loud tone in the band is cut: {d_loud}");
    let (_, d_quiet) = run(&tone(24_000, 1_000.0, 0.005, sr), 512);
    assert!(d_quiet > -0.5, "a quiet one is not: {d_quiet}");
    for block in [1usize, 7, 100, 4096] {
        let (other, _) = run(&tone(24_000, 1_000.0, 0.5, sr), block);
        assert_eq!(other, loud, "block size {block} changed the result");
    }
}

#[test]
fn an_all_pass_changes_phase_and_not_level() {
    let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
    design(EqShape::AllPass, 48_000.0, 1_000.0, 0.0, 1.0, 12.0, &mut c);
    for hz in [50.0, 500.0, 1_000.0, 5_000.0, 15_000.0] {
        assert!(c[0].magnitude_db(48_000.0, hz).abs() < 1e-3);
    }
}

// ---- the processing modes -------------------------------------------------------------

/// Natural phase follows the analog prototype in magnitude and phase up to 20 kHz, where
/// the zero-latency cascade — right in magnitude by design — drifts in phase near Nyquist.
#[test]
fn natural_phase_follows_the_analog_curve_in_magnitude_and_phase() {
    let sr = 48_000.0f32;
    let cases = [
        vec![bell(16_000.0, 9.0, 2.0)],
        vec![bell(12_000.0, -12.0, 0.7)],
        vec![BandSettings {
            shape: EqShape::HighShelf,
            ..bell(10_000.0, 6.0, 0.71)
        }],
        vec![
            bell(1_000.0, 6.0, 1.0),
            BandSettings {
                shape: EqShape::LowCut,
                slope: 24.0,
                ..bell(80.0, 0.0, 0.71)
            },
            bell(18_000.0, -6.0, 4.0),
        ],
    ];
    for bands in cases {
        let mut np = eq_with(
            &bands,
            Options {
                mode: Mode::NaturalPhase,
                ..Options::default()
            },
            sr,
        );
        let mut zl = eq_with(&bands, Options::default(), sr);
        let (h_np, _) = impulse(&mut np, 16_384, false);
        let (h_zl, _) = impulse(&mut zl, 16_384, false);
        let (mut np_mag, mut np_ph, mut zl_mag, mut zl_ph) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let mut hz = 20.0f64;
        while hz <= 20_000.0 {
            let w = 2.0 * PI * hz / sr as f64;
            let mut a = (1.0, 0.0);
            for b in &bands {
                let c = analog_c(b, sr, w);
                a = (a.0 * c.0 - a.1 * c.1, a.0 * c.1 + a.1 * c.0);
            }
            let pa = math::atan2(a.1, a.0);
            // Natural phase, with its delay taken out.
            let n = dtft(&h_np, w);
            let (s, c) = math::sin_cos(w * phase::NP_DELAY as f64);
            let n = (n.0 * c - n.1 * s, n.0 * s + n.1 * c);
            let z = dtft(&h_zl, w);
            let wrap = |x: f64| (x + PI).rem_euclid(2.0 * PI) - PI;
            np_mag = np_mag.max((db(n) - db(a)).abs());
            zl_mag = zl_mag.max((db(z) - db(a)).abs());
            np_ph = np_ph.max(wrap(math::atan2(n.1, n.0) - pa).abs().to_degrees());
            zl_ph = zl_ph.max(wrap(math::atan2(z.1, z.0) - pa).abs().to_degrees());
            hz *= math::powf(2f64, 1.0 / 24.0);
        }
        if report() {
            eprintln!(
                "NP {:?}: natural {np_mag:.3} dB {np_ph:.2}°, zero latency {zl_mag:.3} dB {zl_ph:.2}°",
                bands.iter().map(|b| (b.shape, b.freq)).collect::<Vec<_>>()
            );
        }
        assert!(
            np_mag < 0.1,
            "natural phase magnitude off by {np_mag:.3} dB"
        );
        assert!(np_ph < 2.0, "natural phase off by {np_ph:.2}°");
        assert!(np_ph <= zl_ph, "natural phase no closer than zero latency");
    }
}

#[test]
fn every_flat_mode_is_the_input_delayed_to_the_bit() {
    let sr = 48_000.0;
    let src = noise(40_000, 9, 0.8);
    for (mode, res) in [
        (Mode::NaturalPhase, Resolution::Medium),
        (Mode::LinearPhase, Resolution::Low),
        (Mode::LinearPhase, Resolution::Medium),
        (Mode::LinearPhase, Resolution::High),
        (Mode::LinearPhase, Resolution::VeryHigh),
    ] {
        let mut eq = eq_with(
            &[bell(1_000.0, 0.0, 1.0)],
            Options {
                mode,
                resolution: res,
                ..Options::default()
            },
            sr,
        );
        let lat = eq.latency();
        assert_eq!(lat, latency_of(mode, res, sr));
        let (mut l, mut r) = (src.clone(), src.clone());
        for (cl, cr) in l.chunks_mut(333).zip(r.chunks_mut(333)) {
            eq.process(cl, cr);
        }
        assert!(l[..lat].iter().all(|x| *x == 0.0), "{mode:?} leaked early");
        assert_eq!(&l[lat..], &src[..src.len() - lat], "{mode:?} {res:?}");
        assert_eq!(&r[lat..], &src[..src.len() - lat], "{mode:?} {res:?}");
    }
    // The latencies, stated: 64 samples; 2 560, 4 608, 8 704 and 16 896 at 48 kHz, twice
    // that at 96 kHz.
    assert_eq!(latency_of(Mode::NaturalPhase, Resolution::Low, sr), 64);
    assert_eq!(latency_of(Mode::LinearPhase, Resolution::Low, sr), 2_560);
    assert_eq!(
        latency_of(Mode::LinearPhase, Resolution::VeryHigh, sr),
        16_896
    );
    assert_eq!(
        latency_of(Mode::LinearPhase, Resolution::Medium, 96_000.0),
        9_216
    );
}

/// Linear phase: the magnitude is the drawn curve, the impulse response is symmetric about
/// the latency, and its peak is where the latency says.
#[test]
fn linear_phase_matches_the_drawn_curve_and_is_symmetric() {
    let sr = 48_000.0f32;
    let bands = [
        bell(1_000.0, 6.0, 1.0),
        BandSettings {
            shape: EqShape::LowShelf,
            ..bell(120.0, -4.0, 0.71)
        },
        BandSettings {
            shape: EqShape::HighCut,
            slope: 24.0,
            ..bell(12_000.0, 0.0, 0.71)
        },
        bell(5_000.0, -8.0, 3.0),
    ];
    for res in [Resolution::Medium, Resolution::High] {
        let mut eq = eq_with(
            &bands,
            Options {
                mode: Mode::LinearPhase,
                resolution: res,
                ..Options::default()
            },
            sr,
        );
        let lat = eq.latency();
        let (h, hr) = impulse(&mut eq, 2 * lat + 1024, false);
        // Both channels through one transform: the same filter, to rounding.
        let lr = h
            .iter()
            .zip(&hr)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(lr < 1e-6, "left and right differ by {lr}");
        let peak = h
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap()
            .0;
        assert_eq!(peak, lat, "the impulse response peaks at the latency");
        let asym = (1..lat - 1024)
            .map(|k| (h[lat + k] - h[lat - k]).abs())
            .fold(0.0f32, f32::max);
        assert!(asym < 1e-6, "not symmetric: {asym}");
        let mut worst = 0.0f64;
        let mut hz = 40.0f64;
        while hz <= 18_000.0 {
            let w = 2.0 * PI * hz / sr as f64;
            let got = db(dtft(&h, w));
            let want = Aequalis::curve_db(&bands, Mode::LinearPhase, sr, hz as f32) as f64;
            worst = worst.max((got - want).abs());
            hz *= math::powf(2f64, 1.0 / 12.0);
        }
        if report() {
            eprintln!("LP {res:?}: latency {lat}, worst {worst:.3} dB, asymmetry {asym:e}");
        }
        assert!(worst < 0.1, "{res:?}: off the drawn curve by {worst:.3} dB");
    }
}

/// Bands on L/R and on M/S together run as one 2×2 stage: the latency does not double,
/// and each output is what the matrix says.
#[test]
fn linear_phase_mixes_lr_and_ms_bands_in_one_stage() {
    let sr = 48_000.0f32;
    let left = BandSettings {
        placement: Placement::Left,
        ..bell(500.0, 6.0, 1.0)
    };
    let side = BandSettings {
        placement: Placement::Side,
        ..bell(4_000.0, -6.0, 1.0)
    };
    let mut eq = eq_with(
        &[left, side],
        Options {
            mode: Mode::LinearPhase,
            resolution: Resolution::Medium,
            ..Options::default()
        },
        sr,
    );
    let lat = eq.latency();
    assert_eq!(lat, latency_of(Mode::LinearPhase, Resolution::Medium, sr));
    let (hl, hr) = impulse(&mut eq, 2 * lat + 1024, true);
    for hz in [200.0f32, 500.0, 1_000.0, 4_000.0, 8_000.0] {
        let w = 2.0 * PI * hz as f64 / sr as f64;
        let gl = math::powf(10f64, analog_db(&left, sr, hz) as f64 / 20.0);
        let gs = math::powf(10f64, analog_db(&side, sr, hz) as f64 / 20.0);
        let want_l = 0.5 * gl * (1.0 + gs);
        let want_r = 0.5 * gl * (1.0 - gs);
        let got_l = db(dtft(&hl, w));
        assert!(
            (got_l - 20.0 * math::log10(want_l)).abs() < 0.1,
            "{hz} Hz: L {got_l} vs {}",
            20.0 * math::log10(want_l)
        );
        if want_r.abs() > 0.05 {
            let got_r = db(dtft(&hr, w));
            assert!(
                (got_r - 20.0 * math::log10(want_r.abs())).abs() < 0.2,
                "{hz} Hz: R {got_r} vs {}",
                20.0 * math::log10(want_r.abs())
            );
        }
    }
}

/// Every mode, with a static band, a dynamic band keyed from the sidechain, auto gain and
/// output gain, renders the same at every block size.
#[test]
fn every_mode_is_block_size_independent() {
    let sr = 48_000.0;
    let src = noise(30_000, 3, 0.5);
    let key = tone(30_000, 1_000.0, 0.6, sr);
    let bands = [
        bell(300.0, -4.0, 1.5),
        BandSettings {
            dynamic: true,
            external: true,
            threshold_db: -30.0,
            range_db: -8.0,
            placement: Placement::Mid,
            ..bell(1_000.0, 2.0, 2.0)
        },
        BandSettings {
            dynamic: true,
            below: true,
            threshold_db: -40.0,
            range_db: 4.0,
            placement: Placement::Side,
            ..bell(6_000.0, 0.0, 1.0)
        },
    ];
    for mode in Mode::ALL {
        let opts = Options {
            mode,
            resolution: Resolution::Low,
            auto_gain: true,
            output_db: -1.5,
            ..Options::default()
        };
        let run = |block: usize| -> Vec<f32> {
            let mut eq = eq_with(&bands, opts, sr);
            let (mut l, mut r) = (src.clone(), noise(30_000, 5, 0.5));
            let mut i = 0;
            while i < l.len() {
                let n = block.min(l.len() - i);
                eq.process_keyed(
                    &mut l[i..i + n],
                    &mut r[i..i + n],
                    Some((&key[i..i + n], &key[i..i + n])),
                );
                i += n;
            }
            l.extend(r);
            l
        };
        let reference = run(512);
        assert!(reference.iter().all(|x| x.is_finite()));
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_eq!(run(block), reference, "{mode:?} at block {block}");
        }
    }
}

// ---- dynamics, audition, auto gain, key -------------------------------------------------

#[test]
fn a_band_that_acts_below_its_threshold_moves_when_the_signal_is_quiet() {
    let sr = 48_000.0;
    let band = BandSettings {
        dynamic: true,
        below: true,
        threshold_db: -30.0,
        range_db: 6.0,
        ratio: 8.0,
        ..bell(1_000.0, 0.001, 2.0)
    };
    let delta = |amp: f32| {
        let mut eq = eq_with(&[band], Options::default(), sr);
        let (mut l, mut r) = (
            tone(24_000, 1_000.0, amp, sr),
            tone(24_000, 1_000.0, amp, sr),
        );
        eq.process(&mut l, &mut r);
        eq.dynamic_deltas()[0]
    };
    let quiet = delta(0.003);
    let loud = delta(0.5);
    assert!(quiet > 5.5, "a quiet signal is lifted: {quiet}");
    assert!(loud.abs() < 0.1, "a loud one is left alone: {loud}");
}

#[test]
fn a_band_keyed_from_the_sidechain_follows_the_key_not_its_own_signal() {
    let sr = 48_000.0;
    let band = BandSettings {
        dynamic: true,
        external: true,
        threshold_db: -30.0,
        range_db: -9.0,
        ratio: 20.0,
        ..bell(1_000.0, 0.001, 2.0)
    };
    let run = |own: f32, key: f32| {
        let mut eq = eq_with(&[band], Options::default(), sr);
        let (mut l, mut r) = (
            tone(24_000, 1_000.0, own, sr),
            tone(24_000, 1_000.0, own, sr),
        );
        let k = tone(24_000, 1_000.0, key, sr);
        eq.process_keyed(&mut l, &mut r, Some((&k, &k)));
        eq.dynamic_deltas()[0]
    };
    assert!(
        run(0.5, 0.0).abs() < 0.1,
        "a loud own signal with a silent key"
    );
    assert!(run(0.0, 0.5) < -8.5, "a loud key moves the band");
    // With no key routed, the band falls back to its own signal.
    let mut eq = eq_with(&[band], Options::default(), sr);
    let (mut l, mut r) = (
        tone(24_000, 1_000.0, 0.5, sr),
        tone(24_000, 1_000.0, 0.5, sr),
    );
    eq.process(&mut l, &mut r);
    assert!(eq.dynamic_deltas()[0] < -8.5);
}

#[test]
fn soloing_a_band_plays_only_the_region_it_acts_on() {
    let sr = 48_000.0;
    for mode in Mode::ALL {
        let opts = Options {
            mode,
            resolution: Resolution::Low,
            solo: Some(0),
            ..Options::default()
        };
        let level = |hz: f32, place: Placement| {
            let b = BandSettings {
                placement: place,
                ..bell(1_000.0, 6.0, 2.0)
            };
            let mut eq = eq_with(&[b], opts, sr);
            let (mut l, mut r) = (tone(48_000, hz, 0.5, sr), tone(48_000, hz, 0.5, sr));
            for (cl, cr) in l.chunks_mut(500).zip(r.chunks_mut(500)) {
                eq.process(cl, cr);
            }
            (
                20.0 * math::log10(rms(&l[24_000..]) / rms(&tone(24_000, hz, 0.5, sr))),
                rms(&r[24_000..]),
            )
        };
        let (at, _) = level(1_000.0, Placement::Stereo);
        let (away, _) = level(100.0, Placement::Stereo);
        assert!(
            at.abs() < 0.5,
            "{mode:?}: the band's own frequency at unity: {at}"
        );
        assert!(away < -20.0, "{mode:?}: far from it, gone: {away}");
        let (_, right) = level(1_000.0, Placement::Left);
        assert!(
            right < 1e-4,
            "{mode:?}: a left band's audition silences the right"
        );
    }
}

#[test]
fn auto_gain_matches_the_loudness_the_eq_changed() {
    let sr = 48_000.0;
    let shelf = BandSettings {
        shape: EqShape::HighShelf,
        ..bell(2_000.0, 9.0, 0.71)
    };
    let mut eq = eq_with(
        &[shelf],
        Options {
            auto_gain: true,
            ..Options::default()
        },
        sr,
    );
    let n = 48_000 * 8;
    let src = noise(n, 11, 0.25);
    let (mut l, mut r) = (src.clone(), noise(n, 12, 0.25));
    let r_src = r.clone();
    for (cl, cr) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
        eq.process(cl, cr);
    }
    // Measure both sides the way the auto gain does (K-weighted), over the last 2 s.
    let loud = |a: &[f32], b: &[f32]| {
        let mut k = KWeight::new(sr);
        let tail = a.len() - 2 * 48_000;
        let mut sum = 0.0f64;
        for i in 0..a.len() {
            let mut z = 0.0;
            k.power(&a[i..i + 1], &b[i..i + 1], &mut z, 1.0);
            if i >= tail {
                sum += z;
            }
        }
        10.0 * math::log10(sum / (a.len() - tail) as f64)
    };
    let diff = loud(&l, &r) - loud(&src, &r_src);
    if report() {
        eprintln!(
            "auto gain {:.2} dB, loudness difference {diff:.2} dB",
            eq.auto_gain_db()
        );
    }
    assert!(eq.auto_gain_db() < -3.0, "it turned the shelf down");
    assert!(
        diff.abs() < 0.5,
        "loudness out differs from in by {diff:.2} dB"
    );
}

#[test]
fn the_keys_spectrum_shows_a_key_tone_at_its_level() {
    let sr = 48_000.0;
    let mut eq = Aequalis::new(sr);
    eq.set(&[BandSettings::default(); MAX_BANDS], 0.0);
    let key = tone(16_384, 1_000.0, 0.5, sr);
    let (mut l, mut r) = (vec![0.0; 16_384], vec![0.0; 16_384]);
    eq.process_keyed(&mut l, &mut r, Some((&key, &key)));
    let levels = eq.key_levels();
    let k = (0..KEY_BANDS)
        .min_by(|a, b| {
            (key_band_hz(*a) - 1_000.0)
                .abs()
                .total_cmp(&(key_band_hz(*b) - 1_000.0).abs())
        })
        .unwrap();
    assert!(
        (levels[k] + 6.0).abs() < 1.5,
        "1 kHz at −6 dB: {}",
        levels[k]
    );
    assert!(levels[k - 6] < -40.0 && levels[k + 6] < -40.0);
    let t = eq.telemetry();
    assert_eq!(t[telemetry::KEY + k], levels[k]);
    assert_eq!(t[telemetry::KEY_ON], 1.0);
    // Untouched audio: the EQ is empty.
    assert!(l.iter().all(|x| *x == 0.0));
}

#[test]
fn nothing_is_ever_not_a_number() {
    let sr = 48_000.0;
    let bands = [
        bell(10.0, 30.0, 40.0),
        bell(21_000.0, -30.0, 0.025),
        BandSettings {
            shape: EqShape::LowCut,
            slope: 96.0,
            ..bell(20_000.0, 0.0, 40.0)
        },
        BandSettings {
            shape: EqShape::HighShelf,
            ..bell(30_000.0, 30.0, 40.0)
        },
        BandSettings {
            dynamic: true,
            attack_ms: 0.1,
            release_ms: 5.0,
            range_db: 30.0,
            ratio: 20.0,
            threshold_db: -80.0,
            ..bell(3_000.0, 30.0, 40.0)
        },
    ];
    for mode in Mode::ALL {
        let mut eq = eq_with(
            &bands,
            Options {
                mode,
                resolution: Resolution::Low,
                auto_gain: true,
                ..Options::default()
            },
            sr,
        );
        let (mut l, mut r) = (noise(20_000, 1, 4.0), noise(20_000, 2, 4.0));
        eq.process(&mut l, &mut r);
        assert!(
            l.iter().chain(r.iter()).all(|x| x.is_finite()),
            "{mode:?} produced a sample that is not a number"
        );
    }
}

/// A dynamic band's release comes home exactly (round 21 at-rest's finding 3): its delta
/// lands on +0 dB, and in linear phase — where the movement runs as sections only while it
/// moves — the sections it no longer uses are left clean, as a new unit's are. What is left
/// after that is the detector's envelope, which reaches zero only with denormals flushed.
#[test]
fn a_dynamic_band_comes_home_exactly() {
    for mode in Mode::ALL {
        let mut eq = Aequalis::new(48_000.0);
        eq.set_options(Options {
            mode,
            ..Options::default()
        });
        let mut bands = [BandSettings::default(); MAX_BANDS];
        for (b, band) in bands.iter_mut().enumerate().take(4) {
            *band = BandSettings {
                active: true,
                freq: 120.0 * (1u32 << (2 * b)) as f32,
                gain_db: 3.0,
                dynamic: b % 2 == 1,
                ..BandSettings::default()
            };
        }
        eq.set(&bands, 0.0);
        let mut s = 0x1234u32;
        for blk in 0..400 {
            let (mut l, mut r) = (vec![0f32; 512], vec![0f32; 512]);
            if blk < 40 {
                for (x, y) in l.iter_mut().zip(r.iter_mut()) {
                    s ^= s << 13;
                    s ^= s >> 17;
                    s ^= s << 5;
                    *x = (s as f32 / u32::MAX as f32 - 0.5) * 0.8;
                    *y = *x * 0.7;
                }
            }
            eq.process(&mut l, &mut r);
            if blk == 39 {
                assert!(
                    eq.bands.iter().any(|b| b.delta_db != 0.0),
                    "{mode:?}: no movement"
                );
            }
        }
        for b in eq.bands.iter().filter(|b| b.target.active) {
            assert_eq!(
                b.delta_db.to_bits(),
                0,
                "{mode:?}: the release ends at +0 dB"
            );
            if mode == Mode::LinearPhase {
                assert!(b
                    .state
                    .iter()
                    .flatten()
                    .all(|st| st.z1.to_bits() == 0 && st.z2.to_bits() == 0));
            }
        }
        assert!(
            matches!(eq.rest_blocker(), None | Some("detectors")),
            "{mode:?}: {:?}",
            eq.rest_blocker()
        );
    }
}
