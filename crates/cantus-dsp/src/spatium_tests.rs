//! Spatium's own tests: the null tests of the house rule, and the reverb measured the way
//! a room is measured. Every number the tests hold is printed with `--nocapture`.

use super::model::{DecayShape, ER_OFF_DB};
use super::*;
use crate::nulltest::assert_null;
use crate::spatium::measure::{
    band_t60_averaged, correlation, echo_density, expected_band_t60, max_autocorrelation,
};

const FS: f32 = 48_000.0;

fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Pink noise (Kellet's filter on white), about 0.3 RMS.
fn pink(n: usize, seed: u32) -> Vec<f32> {
    let mut seed = seed;
    let mut b = [0.0f32; 7];
    (0..n)
        .map(|_| {
            let w = noise(&mut seed);
            b[0] = 0.99886 * b[0] + w * 0.0555179;
            b[1] = 0.99332 * b[1] + w * 0.0750759;
            b[2] = 0.96900 * b[2] + w * 0.153_852;
            b[3] = 0.86650 * b[3] + w * 0.3104856;
            b[4] = 0.55000 * b[4] + w * 0.5329522;
            b[5] = -0.7616 * b[5] - w * 0.0168980;
            let out = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + w * 0.5362;
            b[6] = w * 0.115926;
            out * 0.11
        })
        .collect()
}

/// A test signal: bursts of noise and tones with silences between.
fn program(n: usize) -> (Vec<f32>, Vec<f32>) {
    let mut seed = 9u32;
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    for i in 0..n {
        let on = (i / 9_000) % 3 != 2;
        let t = i as f32 / FS;
        if on {
            l[i] = 0.4 * noise(&mut seed) + 0.3 * (t * 440.0 * std::f32::consts::TAU).sin();
            r[i] = 0.4 * noise(&mut seed) + 0.2 * (t * 330.0 * std::f32::consts::TAU).sin();
        }
    }
    (l, r)
}

fn render(s: &SpatiumSettings, block: usize, l: &[f32], r: &[f32], key: bool) -> Vec<f32> {
    let mut v = Spatium::new(FS);
    v.set(s);
    v.reset();
    let (mut l, mut r) = (l.to_vec(), r.to_vec());
    // A key that pulses on its own schedule.
    let kl: Vec<f32> = (0..l.len())
        .map(|i| if (i / 6_000) % 2 == 0 { 0.8 } else { 0.0 })
        .collect();
    let mut pos = 0;
    while pos < l.len() {
        let e = (pos + block).min(l.len());
        let k = if key {
            Some((&kl[pos..e], &kl[pos..e]))
        } else {
            None
        };
        v.process(&mut l[pos..e], &mut r[pos..e], k);
        pos = e;
    }
    l.into_iter().chain(r).collect()
}

/// Settings that exercise everything: a decay band, post EQ, ducking, modulation.
fn busy() -> SpatiumSettings {
    let mut s = SpatiumSettings {
        mix: 0.6,
        character: 0.8,
        duck_db: 12.0,
        duck_thr_db: -24.0,
        width: 1.2,
        er_size: 1.4,
        ..SpatiumSettings::default()
    };
    s.decay_bands[0] = DecayBand {
        active: true,
        shape: DecayShape::Bell,
        freq: 300.0,
        mult: 2.0,
        q: 1.0,
    };
    s.post_bands[0] = BandSettings {
        active: true,
        shape: EqShape::LowCut,
        freq: 150.0,
        q: 0.7,
        slope: 24.0,
        ..BandSettings::default()
    };
    s
}

#[test]
fn mix_zero_is_a_wire_to_the_bit() {
    let (l, r) = program(48_000);
    let dry: Vec<f32> = l.iter().chain(r.iter()).copied().collect();
    for style in Style::ALL {
        for freeze in [false, true] {
            let s = SpatiumSettings {
                mix: 0.0,
                style,
                freeze,
                ..busy()
            };
            assert_null(&dry, &render(&s, 256, &l, &r, true), 0.0, "mix 0");
        }
    }
}

#[test]
fn the_block_size_changes_nothing() {
    let (l, r) = program(40_000);
    for style in Style::ALL {
        let s = SpatiumSettings { style, ..busy() };
        let reference = render(&s, 512, &l, &r, true);
        assert!(reference.iter().all(|x| x.is_finite()));
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render(&s, block, &l, &r, true),
                0.0,
                &format!("{style:?} at block {block}"),
            );
        }
    }
}

#[test]
fn two_instances_agree() {
    let (l, r) = program(30_000);
    let s = busy();
    assert_null(
        &render(&s, 333, &l, &r, false),
        &render(&s, 333, &l, &r, false),
        0.0,
        "determinism",
    );
}

/// The wet impulse response, late field only: no reflections, no pre-delay. `side`
/// chooses the input: both sides, the left, or the right.
fn impulse_from(s: &SpatiumSettings, seconds: f32, side: usize) -> (Vec<f32>, Vec<f32>) {
    let mut v = Spatium::new(FS);
    let s = SpatiumSettings {
        mix: 1.0,
        predelay_ms: 0.0,
        er_db: ER_OFF_DB,
        ..*s
    };
    v.set(&s);
    v.reset();
    let n = (seconds * FS) as usize;
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    if side != 2 {
        l[0] = 1.0;
    }
    if side != 1 {
        r[0] = 1.0;
    }
    v.process(&mut l, &mut r, None);
    (l, r)
}

fn impulse(s: &SpatiumSettings, seconds: f32) -> (Vec<f32>, Vec<f32>) {
    impulse_from(s, seconds, 0)
}

/// The decay the tail really has, measured the way acousticians measure a room — octave
/// bands, Schroeder's backward integration, a line fitted from −5 to −25 dB, energy-
/// averaged over four source/receiver pairs (left and right in, left and right out) —
/// against what the same measurement reads on an ideal decay that follows the curve
/// exactly (`expected_band_t60`: where the curve is steep an octave band reads its slow
/// side). 125 Hz … 8 kHz, five settings. Tolerance 8 %: the 5 % a listener notices plus
/// the scatter of a finite number of modes in a band. The numbers are printed.
#[test]
fn the_measured_decay_follows_the_curve() {
    let mut eq = SpatiumSettings::default();
    eq.decay_bands[0] = DecayBand {
        active: true,
        shape: DecayShape::LowShelf,
        freq: 200.0,
        mult: 2.0,
        q: 0.7,
    };
    eq.decay_bands[1] = DecayBand {
        active: true,
        shape: DecayShape::Bell,
        freq: 2_000.0,
        mult: 0.5,
        q: 0.8,
    };
    let cases = [
        ("default room", SpatiumSettings::default()),
        (
            "dark hall",
            SpatiumSettings {
                space: 0.85,
                decay_s: 4.0,
                brightness: 0.2,
                ..SpatiumSettings::default()
            },
        ),
        (
            "small bright room",
            SpatiumSettings {
                space: 0.15,
                decay_s: 0.6,
                brightness: 0.8,
                character: 0.0,
                ..SpatiumSettings::default()
            },
        ),
        ("drawn curve", SpatiumSettings { decay_s: 1.5, ..eq }),
        (
            "plate",
            SpatiumSettings {
                style: Style::Plate,
                decay_s: 2.5,
                ..SpatiumSettings::default()
            },
        ),
    ];
    let mut worst = 0.0f32;
    for (name, s) in cases {
        let curve = s.target_curve();
        let longest = [125.0, 1_000.0, 8_000.0]
            .iter()
            .map(|f| curve.t60(*f))
            .fold(0.0f32, f32::max);
        let seconds = (longest * 0.9).max(0.8) + 0.3;
        let (ll, lr) = impulse_from(&s, seconds, 1);
        let (rl, rr) = impulse_from(&s, seconds, 2);
        for hz in [125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0] {
            let want = expected_band_t60(|f| curve.t60(f), FS, hz).unwrap();
            let got = band_t60_averaged(&[&ll, &lr, &rl, &rr], FS, hz).expect("a decay");
            let err = got / want - 1.0;
            eprintln!(
                "{name:>18} {hz:>6} Hz: {got:.3} s for {want:.3} s (curve {:.3} s) ({:+.1} %)",
                curve.t60(hz),
                err * 100.0
            );
            worst = worst.max(err.abs());
        }
    }
    eprintln!("worst {:.1} %", worst * 100.0);
    assert!(worst < 0.08, "worst {:.1} %", worst * 100.0);
}

/// No ringing and no flutter: after 100 ms the tail is as dense as noise (normalised echo
/// density at least 0.9 on average over 20 ms windows, none under 0.7) and nothing in it
/// repeats (the largest
/// autocorrelation at any lag from 2 to 250 ms stays under 0.2 — a metallic loop shows a
/// peak near 1 at its period).
#[test]
fn the_tail_is_dense_and_does_not_repeat() {
    for (name, s) in [
        ("default", SpatiumSettings::default()),
        (
            "still",
            SpatiumSettings {
                character: 0.0,
                ..SpatiumSettings::default()
            },
        ),
        (
            "small room, long decay",
            SpatiumSettings {
                space: 0.1,
                decay_s: 3.0,
                ..SpatiumSettings::default()
            },
        ),
    ] {
        let (l, _) = impulse(&s, 1.2);
        let tail = &l[(0.1 * FS) as usize..(1.0 * FS) as usize];
        // Undo the decay so every part of the tail weighs the same.
        let t60 = s.target_curve().t60(1_000.0);
        let flat: Vec<f32> = tail
            .iter()
            .enumerate()
            .map(|(i, x)| x * 10f32.powf(3.0 * i as f32 / (FS * t60)))
            .collect();
        let ned = echo_density(&flat, FS, 20.0);
        let min_ned = ned.iter().copied().fold(f32::MAX, f32::min);
        let mean_ned = ned.iter().sum::<f32>() / ned.len() as f32;
        let (peak, lag) = max_autocorrelation(&flat, FS, 2.0, 250.0);
        eprintln!(
            "{name}: echo density {mean_ned:.2} (lowest window {min_ned:.2}); strongest repetition {peak:.3} at {lag:.1} ms"
        );
        // A 20 ms window of noise reads 1 +- 0.05; the lowest of 45 windows sits lower.
        assert!(mean_ned >= 0.9, "{name}: sparse tail {mean_ned}");
        assert!(min_ned >= 0.7, "{name}: a sparse window {min_ned}");
        assert!(peak < 0.2, "{name}: repeats {peak} at {lag} ms");
    }
}

/// A mono impulse comes out as two different tails: correlation under 0.15 at full width;
/// at width 0 the sides are identical.
#[test]
fn the_sides_are_decorrelated_and_width_narrows_them() {
    let (l, r) = impulse(&SpatiumSettings::default(), 1.0);
    let rho = correlation(&l[4_800..], &r[4_800..]);
    eprintln!("L/R correlation at width 1: {rho:.3}");
    assert!(rho.abs() < 0.15, "{rho}");
    let (l, r) = impulse(
        &SpatiumSettings {
            width: 0.0,
            ..SpatiumSettings::default()
        },
        0.5,
    );
    assert!((correlation(&l[4_800..], &r[4_800..]) - 1.0).abs() < 1e-4);
}

/// Freeze keeps what the tank holds and lets nothing new in.
#[test]
fn freeze_holds_the_tail_and_shuts_the_door() {
    let run = |late_input: bool| -> Vec<f32> {
        let mut v = Spatium::new(FS);
        let s = SpatiumSettings {
            mix: 1.0,
            ..SpatiumSettings::default()
        };
        v.set(&s);
        v.reset();
        let (mut l, mut r) = program(24_000);
        v.process(&mut l, &mut r, None);
        v.set(&SpatiumSettings { freeze: true, ..s });
        let mut out = Vec::new();
        for block in 0..600 {
            let mut l = vec![0.0; 480];
            let mut r = vec![0.0; 480];
            if late_input && block > 100 {
                l.iter_mut().for_each(|x| *x = 0.5);
            }
            v.process(&mut l, &mut r, None);
            out.extend_from_slice(&l);
        }
        out
    };
    let frozen = run(false);
    let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
    let early = rms(&frozen[48_000..96_000]);
    let late = rms(&frozen[240_000..288_000]);
    let drop = 20.0 * (late / early).log10();
    eprintln!("frozen level after 5 s: {drop:+.2} dB");
    assert!(drop.abs() < 1.0, "{drop} dB");
    // Input after freezing changes nothing that the reverb holds.
    let fed = run(true);
    let d = frozen[48_000..]
        .iter()
        .zip(&fed[48_000..])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(d < 1e-6, "input leaked into a frozen tank: {d}");
}

/// Ducking: a loud key pulls the wet down by the range; it comes back after release.
#[test]
fn ducking_follows_the_key() {
    let mut v = Spatium::new(FS);
    let s = SpatiumSettings {
        mix: 1.0,
        duck_db: 12.0,
        duck_thr_db: -30.0,
        duck_att_ms: 5.0,
        duck_rel_ms: 100.0,
        freeze: true,
        ..SpatiumSettings::default()
    };
    // Fill the tank, then freeze it so its level is steady.
    v.set(&SpatiumSettings {
        freeze: false,
        duck_db: 0.0,
        ..s
    });
    v.reset();
    let (mut l, mut r) = program(24_000);
    v.process(&mut l, &mut r, None);
    v.set(&s);
    let level = |v: &mut Spatium, key: f32| -> f32 {
        let mut l = vec![0.0; 24_000];
        let mut r = vec![0.0; 24_000];
        let k = vec![key; 24_000];
        v.process(&mut l, &mut r, Some((&k, &k)));
        let tail = &l[12_000..];
        (tail.iter().map(|x| x * x).sum::<f32>() / tail.len() as f32).sqrt()
    };
    let open = level(&mut v, 0.0);
    let ducked = level(&mut v, 1.0);
    let back = level(&mut v, 0.0);
    let depth = 20.0 * (ducked / open).log10();
    eprintln!(
        "ducked by {depth:.2} dB (range 12), meter {:.2}",
        v.meter().duck_db
    );
    assert!((depth + 12.0).abs() < 1.0, "{depth}");
    assert!((20.0 * (back / open).log10()).abs() < 1.0);
}

/// The late field's level: steady pink noise (music's long-term slope) at a 2 s decay comes
/// out at the level it went in (within 1 dB) — less the distance's designed offset — in
/// every style.
#[test]
fn the_wet_level_is_calibrated() {
    let mut levels = Vec::new();
    for style in Style::ALL {
        let mut v = Spatium::new(FS);
        let s = SpatiumSettings {
            style,
            mix: 1.0,
            decay_s: 2.0,
            er_db: ER_OFF_DB,
            ..SpatiumSettings::default()
        };
        v.set(&s);
        v.reset();
        let n = 6 * 48_000;
        let mut l = pink(n, 3);
        let mut r = pink(n, 4);
        let inp = (l.iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
        v.process(&mut l, &mut r, None);
        let tail = &l[4 * 48_000..];
        let out = (tail.iter().map(|x| x * x).sum::<f32>() / tail.len() as f32).sqrt();
        // The late field sits `late_level_db` from its unit by design (distance).
        let db = 20.0 * (out / inp).log10() - super::model::late_level_db(s.distance);
        eprintln!("{style:?}: wet {db:+.2} dB from its unit");
        levels.push((style, db));
    }
    for (style, db) in levels {
        assert!(db.abs() < 1.0, "{style:?}: {db} dB");
    }
}

/// A long silence after a burst ends in exact zeros: nothing lingers in the denormal
/// range, where the arithmetic would slow to a crawl.
#[test]
fn silence_comes_back_to_exact_zero() {
    let mut v = Spatium::new(FS);
    v.set(&SpatiumSettings {
        decay_s: 0.5,
        mix: 1.0,
        ..SpatiumSettings::default()
    });
    v.reset();
    let (mut l, mut r) = program(9_000);
    v.process(&mut l, &mut r, None);
    let mut last = 1.0f32;
    for _ in 0..(20 * 48_000 / 4_800) {
        let mut l = vec![0.0; 4_800];
        let mut r = vec![0.0; 4_800];
        v.process(&mut l, &mut r, None);
        assert!(l.iter().chain(&r).all(|x| x.is_finite()));
        last = l.iter().chain(&r).fold(0.0f32, |m, x| m.max(x.abs()));
    }
    assert_eq!(last, 0.0, "the tail never reached zero");
}

/// How fast it runs: ten seconds of stereo noise through each style, at 512 and at 1
/// sample per block. `cargo test --release -p cantus-dsp spatium_speed -- --ignored
/// --nocapture`. Timing depends on the machine, so this only prints.
#[test]
#[ignore]
fn spatium_speed() {
    for style in Style::ALL {
        for block in [512usize, 1] {
            let mut v = Spatium::new(FS);
            v.set(&SpatiumSettings { style, ..busy() });
            v.reset();
            let n = 10 * 48_000;
            let (l, r) = program(n);
            let (mut l, mut r) = (l, r);
            let t0 = std::time::Instant::now();
            let mut pos = 0;
            while pos < n {
                let e = (pos + block).min(n);
                v.process(&mut l[pos..e], &mut r[pos..e], None);
                pos = e;
            }
            let secs = t0.elapsed().as_secs_f64();
            eprintln!(
                "{style:?} block {block}: {:.1}x real time ({:.2} % of a core)",
                10.0 / secs,
                secs / 10.0 * 100.0
            );
        }
    }
}
