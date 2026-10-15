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

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn unit(s: &PhasisSettings) -> Phasis {
    let mut p = Phasis::new(SR);
    p.set(s);
    p.reset();
    p
}

fn drive(p: &mut Phasis, l: &mut [f32], r: &mut [f32], mut split: impl FnMut() -> usize) {
    let n = l.len();
    let mut at = 0;
    while at < n {
        let b = split().clamp(1, n - at);
        p.set_transport(&Transport {
            playing: true,
            tempo_bpm: 128.0,
            samples: at as i64,
        });
        p.process(&mut l[at..at + b], &mut r[at..at + b]);
        at += b;
    }
}

fn busy() -> PhasisSettings {
    PhasisSettings {
        stages: 6,
        mix: 0.5,
        lo_hz: 150.0,
        hi_hz: 4_000.0,
        feedback: -0.6,
        lfo: LfoSettings {
            rate: Rate::Beats(0.5),
            shape: Shape::Triangle,
            ..LfoSettings::default()
        },
        spread: 0.25,
        out_db: -1.0,
    }
}

/// Held still (a rate of 0 Hz keeps the sine at its middle), the stages sit at the
/// geometric mean of the bounds.
fn held(stages: usize) -> PhasisSettings {
    PhasisSettings {
        stages,
        mix: 0.5,
        lo_hz: 250.0,
        hi_hz: 4_000.0,
        lfo: LfoSettings {
            rate: Rate::Hz(0.0),
            ..LfoSettings::default()
        },
        ..PhasisSettings::default()
    }
}

fn level_db(s: &PhasisSettings, f: f64) -> f32 {
    let mut p = unit(s);
    let n = 24_000;
    let w = std::f64::consts::TAU * f / f64::from(SR);
    let x: Vec<f32> = (0..n)
        .map(|i| crate::math::sin((w * i as f64) % std::f64::consts::TAU) as f32)
        .collect();
    let (mut l, mut r) = (x.clone(), x);
    p.process(&mut l, &mut r);
    let peak = l[12_000..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
    20.0 * crate::math::log10(peak.max(1e-9))
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
    unit(&PhasisSettings::default()).process(&mut l, &mut r);
    assert!(same(&l, &x) && same(&r, &x), "the defaults");
}

/// A notch for every two stages, where the cascade turns by an odd multiple of 180°:
/// each stage turns `−2·atan(f/fc)`.
#[test]
fn every_two_stages_make_a_notch_where_the_phase_says() {
    let fc = (250.0f64 * 4_000.0).sqrt();
    for stages in [2usize, 4, 6, 12] {
        let s = held(stages);
        let pairs = stages / 2;
        for k in 0..pairs {
            let angle = std::f64::consts::PI * (2 * k + 1) as f64 / (2 * stages) as f64;
            let f = warped(fc, angle);
            if f > 20_000.0 {
                continue;
            }
            let db = level_db(&s, f);
            assert!(
                db < -30.0,
                "{stages} stages, notch {k} at {f:.0} Hz: {db:.1} dB"
            );
        }
        // Where the cascade turns a whole circle the two add: +6 dB (two stages turn a
        // whole circle only at DC and Nyquist).
        if stages == 2 {
            continue;
        }
        let peak = level_db(&s, warped(fc, std::f64::consts::PI / stages as f64));
        assert!((peak - 6.02).abs() < 0.2, "{stages} stages peak {peak:.2}");
    }
}

#[test]
fn feedback_deepens_the_peaks_either_way_and_stays_bounded() {
    let mut s = held(4);
    let fc = (250.0f64 * 4_000.0).sqrt();
    let f = fc * crate::math::tan(std::f64::consts::FRAC_PI_4);
    let open = level_db(&s, f);
    s.feedback = 0.8;
    let resonant = level_db(&s, f);
    assert!(resonant > open + 3.0, "{open:.1} → {resonant:.1}");
    let x = noise(48_000, 3);
    for fb in [0.95f32, -0.95] {
        let mut b = busy();
        b.feedback = fb;
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut unit(&b), &mut l, &mut r, || 512);
        assert!(l.iter().chain(&r).all(|v| v.is_finite() && v.abs() < 50.0));
    }
}

#[test]
fn the_sweep_moves_between_its_bounds() {
    let mut s = busy();
    s.lfo.rate = Rate::Hz(2.0);
    let mut p = unit(&s);
    let (mut lo, mut hi) = (f32::MAX, 0.0f32);
    let mut t = [0.0f32; 64];
    for _ in 0..1_000 {
        let (mut l, mut r) = ([0.1f32; 32], [0.1f32; 32]);
        p.process(&mut l, &mut r);
        p.telemetry(&mut t);
        lo = lo.min(t[T_FC_L]);
        hi = hi.max(t[T_FC_L]);
    }
    assert!(
        (lo - 150.0).abs() < 2.0 && (hi - 4_000.0).abs() < 40.0,
        "{lo} … {hi}"
    );
    assert!((sweep_hz(150.0, 4_000.0, 0.0) - (150.0f32 * 4_000.0).sqrt()).abs() < 0.5);
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
    let mut s = busy();
    s.feedback = 0.95;
    let mut a = unit(&s);
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

/// Where a TPT stage tuned to `fc` turns by `2·angle`: its phase is
/// `−2·atan(tan(πf/fs)/tan(πfc/fs))` — the analog curve, prewarped.
fn warped(fc: f64, angle: f64) -> f64 {
    let fs = f64::from(SR);
    let g = crate::math::tan(std::f64::consts::PI * fc / fs);
    fs / std::f64::consts::PI * crate::math::atan(g * crate::math::tan(angle))
}

#[test]
fn fewer_stages_strand_nothing_and_the_unit_comes_to_rest() {
    let x = noise(10_000, 21);
    let mut s = busy();
    s.stages = 12;
    let mut a = unit(&s);
    let (mut l, mut r) = (x.clone(), x);
    drive(&mut a, &mut l, &mut r, || 512);
    s.stages = 4;
    a.set(&s);
    let mut silent = 0;
    while !a.at_rest() && silent < 480_000 {
        let (mut zl, mut zr) = (vec![0.0f32; 64], vec![0.0f32; 64]);
        a.process(&mut zl, &mut zr);
        silent += 64;
    }
    assert!(a.at_rest(), "{:?}", a.rest_blocker());
}
