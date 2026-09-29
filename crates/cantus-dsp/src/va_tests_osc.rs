//! Oscillators, drift, cards and envelopes.

use super::testkit::*;
use super::*;
use crate::comprimo::oversample::Oversampler;

/// Render one oscillator at `factor`× the base rate `fs` and bring it down: `n` base
/// samples, after a settling second.
fn render_osc(shape: Shape, f0: f32, fs: f32, factor: usize, kernel: Kernel, n: usize) -> Vec<f32> {
    let fs_os = fs * factor as f32;
    let mut os = Oversampler::new();
    os.set_factor(factor);
    let mut blep = Blep::new(kernel);
    let mut osc = VaOsc::new(0.123);
    let dt = f0 as f64 / fs_os as f64;
    let settle = 4096;
    let mut out = Vec::with_capacity(n);
    for i in 0..settle + n {
        let mut buf = [0f32; 4];
        for s in buf.iter_mut().take(factor) {
            blep.begin();
            osc.tick(dt, &shape, 1.0, &mut blep, None);
            *s = blep.end();
        }
        let y = if factor == 1 {
            buf[0]
        } else {
            os.down(0, &buf)
        };
        if i >= settle {
            out.push(y);
        }
    }
    out
}

const SAW: Shape = Shape {
    saw: 1.0,
    pulse: 0.0,
    tri: 0.0,
    pw: 0.5,
    curve: 0.0,
    round: 0.0,
};

#[test]
fn a_saw_is_on_pitch() {
    let fs = 96_000.0;
    for f0 in [27.5f32, 110.0, 440.0, 1760.0, 7040.0] {
        let y = render_osc(SAW, f0, fs, 1, Kernel::Wide, 1 << 16);
        let f = frequency(&y, fs).unwrap();
        let err = cents(f, f0 as f64);
        assert!(err.abs() < 0.01, "{f0} Hz saw is {err:+.4} cents off");
    }
}

#[test]
fn the_aliasing_floor_by_kernel_and_rate() {
    let fs = 48_000.0;
    let n = 1 << 16;
    let pulse = Shape {
        saw: 0.0,
        pulse: 1.0,
        pw: 0.1,
        ..SAW
    };
    let tri = Shape {
        saw: 0.0,
        tri: 1.0,
        ..SAW
    };
    // (name, factor, kernel, worst allowed at 1.2 kHz, at 4.3 kHz, at 7.9 kHz): what was
    // measured over saw, 10 % pulse and triangle, with 3 dB to spare.
    let setups = [
        ("1× two-point", 1usize, Kernel::Narrow, -27.0, -24.0, -23.0),
        ("draft 1× four-point", 1, Kernel::Wide, -38.0, -37.0, -37.0),
        ("good 2× four-point", 2, Kernel::Wide, -76.0, -68.0, -67.0),
        ("best 4× four-point", 4, Kernel::Wide, -89.0, -89.0, -89.0),
    ];
    for (name, factor, kernel, a1, a4, a8) in setups {
        let mut line = format!("{name:>20}:");
        for (wave, shape) in [("saw", SAW), ("pulse 10%", pulse), ("tri", tri)] {
            for (f0, allowed) in [(1234.5f32, a1), (4321.0, a4), (7919.0, a8)] {
                let y = render_osc(shape, f0, fs, factor, kernel, n);
                let floor = aliasing_floor_db(&y, fs, f0, 20.0, 20_000.0);
                line.push_str(&format!(" {wave} {f0}: {floor:6.1}"));
                assert!(
                    floor <= allowed,
                    "{name} {wave} at {f0} Hz: aliasing floor {floor:.1} dB (allowed {allowed})"
                );
            }
        }
        println!("{line}");
    }
}

#[test]
fn hard_sync_and_cross_mod_stay_clean() {
    // A slave synced to a 220 Hz master at 2.37× its pitch: the floor holds at 2×.
    let fs = 48_000.0;
    let factor = 4;
    let fs_os = fs * factor as f32;
    let mut os = Oversampler::new();
    os.set_factor(factor);
    let mut blep = Blep::new(Kernel::Wide);
    let (mut master, mut slave) = (VaOsc::new(0.0), VaOsc::new(0.3));
    let silent = Shape { saw: 0.0, ..SAW };
    let n = 1 << 16;
    let mut out = Vec::new();
    for i in 0..4096 + n {
        let mut buf = [0f32; 4];
        for s in buf.iter_mut() {
            blep.begin();
            let c = master.tick(220.0 / fs_os as f64, &silent, 0.0, &mut blep, None);
            slave.tick(2.37 * 220.0 / fs_os as f64, &SAW, 1.0, &mut blep, c.wrap);
            *s = blep.end();
        }
        let y = os.down(0, &buf);
        if i >= 4096 {
            out.push(y);
        }
    }
    let floor = aliasing_floor_db(&out, fs, 220.0, 20.0, 20_000.0);
    println!("hard sync 2.37× over 220 Hz at 4×: aliasing floor {floor:.1} dB");
    assert!(floor < -85.0, "sync floor {floor:.1} dB");
    assert!(frequency(&out, fs).is_some());
}

#[test]
fn pulse_width_keeps_no_dc() {
    let fs = 48_000.0;
    for pw in [0.05f32, 0.25, 0.5, 0.8, 0.95] {
        let shape = Shape {
            saw: 0.0,
            pulse: 1.0,
            pw,
            ..SAW
        };
        let y = render_osc(shape, 187.5, fs, 1, Kernel::Wide, 25_600);
        let mean = y.iter().map(|v| *v as f64).sum::<f64>() / y.len() as f64;
        assert!(mean.abs() < 1e-3, "pw {pw}: DC {mean}");
    }
}

#[test]
fn the_sub_divides_oscillator_one() {
    let fs = 48_000.0;
    for (shape, div) in [
        (SubShape::Square1, 2.0),
        (SubShape::Square2, 4.0),
        (SubShape::Pulse1, 2.0),
    ] {
        let mut blep = Blep::new(Kernel::Wide);
        let mut osc = VaOsc::new(0.0);
        let mut sub = SubOsc::default();
        let silent = Shape { saw: 0.0, ..SAW };
        let y: Vec<f32> = (0..48_000)
            .map(|_| {
                blep.begin();
                let c = osc.tick(440.0 / fs as f64, &silent, 0.0, &mut blep, None);
                sub.tick(shape, 1.0, c, osc.phase, &mut blep);
                blep.end()
            })
            .collect();
        let f = frequency(&y, fs).unwrap();
        assert!(cents(f, 440.0 / div).abs() < 0.5, "{shape:?}: {f} Hz");
    }
}

// ---- drift, cards --------------------------------------------------------------------------

#[test]
fn drift_is_a_pure_function_of_seed_and_time() {
    let a: Vec<f32> = (0..2000)
        .map(|i| drift::drift(7, i as f64 * 0.01, 0.5))
        .collect();
    let b: Vec<f32> = (0..2000)
        .map(|i| drift::drift(7, i as f64 * 0.01, 0.5))
        .collect();
    let c: Vec<f32> = (0..2000)
        .map(|i| drift::drift(8, i as f64 * 0.01, 0.5))
        .collect();
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert!(a.iter().all(|v| v.abs() <= 1.05));
    // Smooth: 10 ms apart it moves a little, never jumps.
    let step = a
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0f32, f32::max);
    assert!(step < 0.1, "drift jumped {step} in 10 ms");
    let spread = a.iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!(spread > 0.2, "drift barely moves ({spread})");
}

#[test]
fn cards_differ_repeatably_and_a_reroll_changes_one() {
    let v = Variance {
        detune_cents: 4.0,
        cutoff_st: 0.3,
        env: 0.05,
        pw: 0.01,
        glide: 0.05,
        tracking_cents: 0.5,
        pan: 0.5,
    };
    let cards: Vec<_> = (0..CARDS).map(|c| card_trims(42, c, 0, &v)).collect();
    assert_eq!(cards[3], card_trims(42, 3, 0, &v));
    assert_ne!(
        cards[3],
        card_trims(42, 3, 1, &v),
        "a re-roll draws the card again"
    );
    assert_ne!(cards[3].osc_cents, cards[4].osc_cents);
    for c in &cards {
        assert!(c.osc_cents.iter().all(|x| x.abs() <= 4.0));
        assert!(c.cutoff_st.abs() <= 0.3 && (c.env_scale - 1.0).abs() <= 0.05);
    }
    let calibrated = card_trims(42, 3, 0, &Variance::default());
    assert_eq!(calibrated.osc_cents, [0.0; 3]);
    assert_eq!((calibrated.cutoff_st, calibrated.env_scale), (0.0, 1.0));
}

// ---- envelopes -------------------------------------------------------------------------------

#[test]
fn the_rc_attack_reaches_its_peak_on_time() {
    let fs = 48_000.0;
    for (model, attack) in [
        (EnvModel::Rc, 0.010f32),
        (EnvModel::Rc, 0.2),
        (EnvModel::Digital, 0.05),
    ] {
        let s = EnvSettings {
            attack,
            model,
            ..Default::default()
        };
        let c = EnvCoefs::new(&s, fs, 1.0);
        let mut e = VaEnv::default();
        e.gate_on();
        let n = (0..10 * fs as usize)
            .position(|_| {
                e.tick(&c);
                e.stage != EnvStage::Attack
            })
            .unwrap() as f32
            + 1.0;
        let want = attack * fs;
        assert!(
            (n - want).abs() <= 0.05 * want,
            "{model:?} {attack}s: {n} samples"
        );
    }
}

#[test]
fn a_release_lands_on_exact_zero_and_a_retrigger_never_jumps() {
    let fs = 48_000.0;
    for model in [
        EnvModel::Rc,
        EnvModel::Ads,
        EnvModel::Digital,
        EnvModel::OneShot,
    ] {
        let s = EnvSettings {
            model,
            ..Default::default()
        };
        let c = EnvCoefs::new(&s, fs, 1.0);
        let mut e = VaEnv::default();
        e.gate_on();
        let mut last = 0.0;
        let mut worst = 0f32;
        for i in 0..fs as usize * 4 {
            if i == 2000 {
                e.gate_off(&c);
            }
            if i == 3000 {
                e.gate_on(); // retrigger mid-release
            }
            if i == 30_000 {
                e.gate_off(&c);
            }
            let v = e.tick(&c);
            worst = worst.max((v - last).abs());
            last = v;
        }
        assert!(worst < 0.1, "{model:?} stepped by {worst}");
        assert_eq!(last, 0.0, "{model:?}");
        assert!(e.is_idle(), "{model:?}");
    }
}
