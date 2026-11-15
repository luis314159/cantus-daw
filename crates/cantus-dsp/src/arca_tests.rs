//! Arca's tests: the valves against their datasheets, the operating points against the
//! published voltages, the tone stack against Yeh & Smith's closed form, the null, block
//! size, rest, aliasing, the cabinets, the power amp's gain and sag.

use super::cab::{design_db, impulse, CabSettings, Cabinet, Mic};
use super::net::{Block, Knobs, Preamp, Taper, Tone};
use super::power::Power;
use super::tube::{AX7, AY7, EL34, EL84, L6GC, V6GT};
use super::*;
use crate::aula_kernel::{Format, Kernel, Layout};

const SR: f32 = 48_000.0;

/// A deterministic hash in −1 … 1.
fn hash(i: u64) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

/// A plucked string (Karplus–Strong): a guitar-like test note at `amp` peak (volts).
pub(super) fn pluck(hz: f32, secs: f32, amp: f32, seed: u64) -> Vec<f32> {
    let n = (secs * SR) as usize;
    let len = (SR / hz) as usize;
    let mut line: Vec<f32> = (0..len)
        .map(|i| hash(seed * 1_000_003 + i as u64))
        .collect();
    let mut out = Vec::with_capacity(n);
    let mut p = 0;
    for _ in 0..n {
        let a = line[p];
        let b = line[(p + 1) % len];
        line[p] = 0.996 * 0.5 * (a + b);
        out.push(a);
        p = (p + 1) % len;
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    out.iter().map(|v| v * amp / peak).collect()
}

pub(super) fn amp_settings(m: AmpModel) -> ArcaSettings {
    ArcaSettings {
        model: Some(m),
        master: 1.0,
        ..ArcaSettings::default()
    }
}

pub(super) fn fresh(s: &ArcaSettings) -> Arca {
    let mut a = Arca::new(SR);
    a.set(s);
    a.reset();
    a
}

pub(super) fn cab_kernel(c: Cabinet) -> Arc<Kernel> {
    let h = impulse(
        &CabSettings {
            cabinet: c,
            ..CabSettings::default()
        },
        SR,
    );
    Arc::new(Kernel::new(
        Format::Mono,
        std::slice::from_ref(&h),
        Layout::for_taps(h.len()),
    ))
}

pub(super) fn run(a: &mut Arca, x: &[f32], block: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (x.to_vec(), x.to_vec());
    for (bl, br) in l.chunks_mut(block).zip(r.chunks_mut(block)) {
        a.process(bl, br);
    }
    (l, r)
}

pub(super) fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|s| s * s).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

#[test]
fn the_valves_hold_their_datasheet_points() {
    // 12AX7, Koren's published constants: 250 V, −2 V → 1.2 mA on the datasheet; Koren's
    // fit is known to run low there (0.95 mA) — held within 25 %.
    let (_, _, ip, gm, _) = AX7.currents(-2.0, 250.0);
    println!(
        "12AX7 250 V −2 V: {:.3} mA, gm {:.2} mA/V",
        ip * 1e3,
        gm * 1e3
    );
    assert!((ip - 1.2e-3).abs() < 0.25 * 1.2e-3, "{ip}");
    assert!((gm - 1.6e-3).abs() < 0.15 * 1.6e-3, "{gm}");
    let (_, _, ip, gm, _) = AY7.currents(-4.0, 250.0);
    assert!((ip - 3.0e-3).abs() < 0.01 * 3.0e-3, "12AY7 {ip}");
    assert!((gm - 1.75e-3).abs() < 0.01 * 1.75e-3, "12AY7 {gm}");
    // No grid current below the cathode; some once past it.
    let (ig_neg, ..) = AX7.currents(-1.0, 200.0);
    let (ig_pos, ..) = AX7.currents(1.0, 200.0);
    assert!(ig_neg < 1e-8 && ig_pos > 1e-4, "{ig_neg} {ig_pos}");
    for (name, t, vg, ia, gm) in [
        ("EL34", EL34, -12.2, 0.100, 11e-3),
        ("6L6GC", L6GC, -14.0, 0.072, 6e-3),
        ("EL84", EL84, -7.3, 0.048, 11.3e-3),
        ("6V6GT", V6GT, -12.5, 0.045, 4.1e-3),
    ] {
        let (i, g, _) = t.current(vg, 250.0, 250.0);
        println!("{name}: {:.1} mA, {:.2} mA/V", i * 1e3, g * 1e3);
        assert!((i - ia).abs() < 0.01 * ia, "{name} {i}");
        assert!((g - gm).abs() < 0.01 * gm, "{name} {g}");
    }
}

/// A preamp stage's first triode at rest: its plate and cathode voltages.
fn plate_of(p: &Preamp, stage: usize) -> (f64, f64) {
    let ports = p.stages()[stage].c.ports();
    let (vgk, vpk) = (ports[0], ports[1]);
    // The grid sits at 0 V through its leak: the cathode is at −v_gk.
    (vpk - vgk, -vgk)
}

#[test]
fn the_operating_points_are_the_published_ones() {
    let k = Knobs::default();
    let rate = 192_000.0;
    let clean = Preamp::new(AmpModel::Clean.preamp(), SOURCE_OHMS, rate, &k);
    let (p, c) = plate_of(&clean, 0);
    println!("clean V1: plate {p:.1} V, cathode {c:.2} V");
    // The AB763's preamp plates: about 180 V (robrobinette.com, AB763 model differences).
    assert!((p - 180.0).abs() < 15.0, "{p}");
    let bass = Preamp::new(AmpModel::Bass.preamp(), SOURCE_OHMS, rate, &k);
    let (p, c) = plate_of(&bass, 1);
    println!("bass V2A: plate {p:.1} V, cathode {c:.2} V");
    // The 5F6-A's V2A plate: 181 V (ampbooks.com, the 5F6-A's analysis).
    assert!((p - 181.0).abs() < 20.0, "{p}");
    let hg = Preamp::new(AmpModel::HighGain.preamp(), SOURCE_OHMS, rate, &k);
    let (p, c) = plate_of(&hg, 2);
    let ma = c / 39e3 * 1e3;
    println!("cold clipper: plate {p:.1} V, cathode {c:.2} V, {ma:.3} mA");
    // The SLO's cold clipper: about −4 V of bias and 0.1 mA (ampbooks.com).
    assert!((c - 4.0).abs() < 0.8 && (ma - 0.1).abs() < 0.03, "{c} {ma}");
}

/// Yeh & Smith, "Discretization of the '59 Fender Bassman tone stack" (DAFx-06): |H(j2πf)|
/// for pot fractions `t`, `l`, `m` (an ideal source, no load), `v` = C1, C2, C3, R1 (treble),
/// R2 (bass), R3 (mid), R4 (slope).
pub(super) fn yeh(t: f64, l: f64, m: f64, f: f64, v: [f64; 7]) -> f64 {
    let [c1, c2, c3, r1, r2, r3, r4] = v;
    let k = c1 * c2 * c3;
    let b1 = t * c1 * r1 + m * c3 * r3 + l * (c1 * r2 + c2 * r2) + (c1 * r3 + c2 * r3);
    let b2 = t * (c1 * c2 * r1 * r4 + c1 * c3 * r1 * r4)
        - m * m * (c1 * c3 * r3 * r3 + c2 * c3 * r3 * r3)
        + m * (c1 * c3 * r1 * r3 + c1 * c3 * r3 * r3 + c2 * c3 * r3 * r3)
        + l * (c1 * c2 * r1 * r2 + c1 * c2 * r2 * r4 + c1 * c3 * r2 * r4)
        + l * m * (c1 * c3 * r2 * r3 + c2 * c3 * r2 * r3)
        + (c1 * c2 * r1 * r3 + c1 * c2 * r3 * r4 + c1 * c3 * r3 * r4);
    let b3 = l * m * (k * r1 * r2 * r3 + k * r2 * r3 * r4)
        - m * m * (k * r1 * r3 * r3 + k * r3 * r3 * r4)
        + m * (k * r1 * r3 * r3 + k * r3 * r3 * r4)
        + t * k * r1 * r3 * r4
        - t * m * k * r1 * r3 * r4
        + t * l * k * r1 * r2 * r4;
    let a1 =
        (c1 * r1 + c1 * r3 + c2 * r3 + c2 * r4 + c3 * r4) + m * c3 * r3 + l * (c1 * r2 + c2 * r2);
    let a2 = m * (c1 * c3 * r1 * r3 - c2 * c3 * r3 * r4 + c1 * c3 * r3 * r3 + c2 * c3 * r3 * r3)
        + l * m * (c1 * c3 * r2 * r3 + c2 * c3 * r2 * r3)
        - m * m * (c1 * c3 * r3 * r3 + c2 * c3 * r3 * r3)
        + l * (c1 * c2 * r2 * r4 + c1 * c2 * r1 * r2 + c1 * c3 * r2 * r4 + c2 * c3 * r2 * r4)
        + (c1 * c2 * r1 * r4
            + c1 * c3 * r1 * r4
            + c1 * c2 * r3 * r4
            + c1 * c2 * r1 * r3
            + c1 * c3 * r3 * r4
            + c2 * c3 * r3 * r4);
    let a3 = l * m * (k * r1 * r2 * r3 + k * r2 * r3 * r4)
        - m * m * (k * r1 * r3 * r3 + k * r3 * r3 * r4)
        + m * (k * r3 * r3 * r4 + k * r1 * r3 * r3 - k * r1 * r3 * r4)
        + l * k * r1 * r2 * r4
        + k * r1 * r3 * r4;
    let w = std::f64::consts::TAU * f;
    // s = jω: s² = −ω², s³ = −jω³.
    let (nr, ni) = (-b2 * w * w, b1 * w - b3 * w * w * w);
    let (dr, di) = (1.0 - a2 * w * w, a1 * w - a3 * w * w * w);
    ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt()
}

/// A linear response's gain at `hz` (dB): the DFT of its impulse response `h` (the
/// impulse `amp` volts) at `rate`.
pub(super) fn gain_db(h: &[f64], amp: f64, rate: f64, hz: f64) -> f64 {
    let w = std::f64::consts::TAU * hz / rate;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, v) in h.iter().enumerate() {
        let (s, c) = crate::math::sin_cos(w * i as f64);
        re += v * c;
        im -= v * s;
    }
    20.0 * crate::math::log10((re * re + im * im).sqrt() / amp)
}

/// A preamp's impulse response at `rate`, `len` samples (the impulse 10 µV: far below
/// anything that clips).
pub(super) fn preamp_ir(p: &mut Preamp, len: usize) -> (Vec<f64>, f64) {
    let amp = 1e-5;
    let h = (0..len)
        .map(|i| p.step(if i == 0 { amp } else { 0.0 }))
        .collect();
    (h, amp)
}

#[test]
fn the_tone_stack_is_yeh_and_smiths() {
    let vals = [250e-12, 20e-9, 20e-9, 250e3, 1e6, 25e3, 56e3];
    let tone = Tone::Tmb {
        slope: 56e3,
        c1: 250e-12,
        c2: 20e-9,
        c3: 20e-9,
        treble: (250e3, Taper::Lin),
        bass: (1e6, Taper::Lin),
        mid: (25e3, Taper::Lin),
    };
    let blocks = [Block::Tone(tone), Block::Leak(1e12)];
    let rate = 192_000.0;
    let mut worst = 0.0f64;
    for (t, l, m) in [
        (0.5, 0.5, 0.5),
        (0.0, 1.0, 0.2),
        (1.0, 0.0, 1.0),
        (0.8, 0.3, 0.0),
        (0.2, 0.9, 0.7),
    ] {
        let k = Knobs {
            gain: 0.5,
            bass: l,
            mid: m,
            treble: t,
        };
        let mut p = Preamp::new(&blocks, 1e-3, rate, &k);
        let (h, amp) = preamp_ir(&mut p, 96_000);
        for hz in [40.0, 100.0, 300.0, 1_000.0, 3_000.0, 8_000.0] {
            let ours = gain_db(&h, amp, rate, hz);
            let theirs = 20.0 * crate::math::log10(yeh(t, l, m, hz, vals));
            worst = worst.max((ours - theirs).abs());
            assert!(
                (ours - theirs).abs() < 0.1,
                "t {t} l {l} m {m} at {hz} Hz: ours {ours:.2} dB, Yeh {theirs:.2} dB"
            );
        }
    }
    println!("worst against Yeh & Smith: {worst:.3} dB");
}

#[test]
fn the_tone_knobs_turn_the_right_way() {
    let rate = 96_000.0;
    for m in AmpModel::ALL {
        let at = |k: Knobs, hz: f64| {
            let mut p = Preamp::new(m.preamp(), SOURCE_OHMS, rate, &k);
            let (h, amp) = preamp_ir(&mut p, 96_000);
            gain_db(&h, amp, rate, hz)
        };
        let mid = Knobs::default();
        let base_lo = at(mid, 80.0);
        let base_hi = at(mid, 4_000.0);
        let lo = at(Knobs { bass: 1.0, ..mid }, 80.0) - base_lo;
        let hi = at(Knobs { treble: 1.0, ..mid }, 4_000.0) - base_hi;
        let g = at(Knobs { gain: 1.0, ..mid }, 1_000.0) - at(mid, 1_000.0);
        println!("{m:?}: bass up {lo:+.1} dB at 80 Hz, treble up {hi:+.1} dB at 4 kHz, gain up {g:+.1} dB");
        assert!(lo > 1.0 && hi > 1.0 && g > 3.0, "{m:?}");
    }
}

#[test]
fn no_amp_and_no_cabinet_is_a_null() {
    let x = pluck(196.0, 0.5, 0.8, 3);
    let mut a = fresh(&ArcaSettings::default());
    assert_eq!(a.latency(), 0);
    let (l, r) = run(&mut a, &x, 64);
    assert!(l.iter().zip(&x).all(|(a, b)| a.to_bits() == b.to_bits()));
    assert!(r.iter().zip(&x).all(|(a, b)| a.to_bits() == b.to_bits()));
    assert!(a.at_rest() || a.rest_blocker() == Some("the dry line holds sound"));
}

/// Every model with its cabinet, its knobs moving mid-way: any block size, the same bits.
#[test]
fn the_block_size_changes_nothing() {
    let x = pluck(110.0, 0.4, 0.6, 9);
    for m in [AmpModel::Crunch, AmpModel::Chime] {
        let mut s = amp_settings(m);
        s.cab = true;
        s.gain = 0.7;
        let mut outs = vec![];
        for block in [1, 37, 64, 512] {
            let mut a = Arca::with_cab(SR, cab_kernel(Cabinet::FourTwelveVintage));
            a.set(&s);
            a.reset();
            let (mut l, mut r) = (x.clone(), x.clone());
            let half = x.len() / 2;
            for (i, (bl, br)) in l[..half]
                .chunks_mut(block)
                .zip(r[..half].chunks_mut(block))
                .enumerate()
            {
                let _ = i;
                a.process(bl, br);
            }
            let mut s2 = s;
            s2.treble = 0.9;
            s2.gain = 0.4;
            a.set(&s2);
            for (bl, br) in l[half..].chunks_mut(block).zip(r[half..].chunks_mut(block)) {
                a.process(bl, br);
            }
            outs.push(l);
        }
        for o in &outs[1..] {
            assert!(
                o.iter()
                    .zip(&outs[0])
                    .all(|(a, b)| a.to_bits() == b.to_bits()),
                "{m:?}"
            );
        }
    }
}

/// Each model's level for a guitar note (a pluck, 0.3 V peak) at low and high gain, with
/// its cabinet: printed (the presets' levels are fitted from it).
#[test]
fn levels_by_model() {
    let x = pluck(110.0, 1.0, 0.3, 5);
    for m in AmpModel::ALL {
        for gain in [0.2, 0.8] {
            let mut s = amp_settings(m);
            s.gain = gain;
            s.cab = true;
            let mut a = Arca::with_cab(SR, cab_kernel(Cabinet::FourTwelveVintage));
            a.set(&s);
            a.reset();
            let (l, _) = run(&mut a, &x, 256);
            let peak = l.iter().fold(0.0f32, |p, v| p.max(v.abs()));
            assert!(l.iter().all(|v| v.is_finite()), "{m:?}");
            println!(
                "{m:?} gain {gain}: rms {:.1} dB, peak {:.1} dB, sag {:.1} V",
                20.0 * crate::math::log10(rms(&l)),
                20.0 * crate::math::log10(peak),
                a.meters().0
            );
        }
    }
}

/// Twins: one plays silence through to the end, the other skips it once at rest; then
/// both play the same note. The same bits.
#[test]
fn rest_is_exact() {
    let x = pluck(147.0, 0.3, 0.5, 11);
    for m in [AmpModel::Clean, AmpModel::HighGain, AmpModel::Chime] {
        let mut s = amp_settings(m);
        s.cab = true;
        let mk = || {
            let mut a = Arca::with_cab(SR, cab_kernel(Cabinet::OneTwelveOpen));
            a.set(&s);
            a.reset();
            a
        };
        let (mut a, mut b) = (mk(), mk());
        run(&mut a, &x, 128);
        run(&mut b, &x, 128);
        let silence = vec![0.0f32; 256];
        let mut blocks = 0;
        while !a.at_rest() {
            let (la, _) = run(&mut a, &silence, 256);
            let (lb, _) = run(&mut b, &silence, 256);
            assert_eq!(la, lb);
            blocks += 1;
            assert!(blocks < 2_000, "{m:?} never rests: {:?}", a.rest_blocker());
        }
        println!(
            "{m:?}: at rest after {:.2} s of silence",
            blocks as f32 * 256.0 / SR
        );
        assert!(b.at_rest());
        for _ in 0..40 {
            let (la, _) = run(&mut a, &silence, 256);
            assert!(
                la.iter().all(|v| v.to_bits() == 0),
                "{m:?}: silence at rest"
            );
        }
        b.skip(40 * 256);
        let (la, _) = run(&mut a, &x, 100);
        let (lb, _) = run(&mut b, &x, 100);
        assert!(
            la.iter().zip(&lb).all(|(p, q)| p.to_bits() == q.to_bits()),
            "{m:?}"
        );
        assert!(la.iter().any(|v| *v != 0.0));
    }
}

/// A 2.5 kHz sine (on a bin) through the high-gain amp, no cabinet: what is not a
/// harmonic of it (aliasing, folded back) against what is.
#[test]
fn aliasing_is_measured() {
    let n = 16_384;
    let k0 = 853; // 853 · 48 000 / 16 384 ≈ 2 499 Hz
    let mut res = vec![];
    for (m, os) in [
        (AmpModel::Crunch, 2),
        (AmpModel::Crunch, 4),
        (AmpModel::Crunch, 8),
        (AmpModel::HighGain, 2),
        (AmpModel::HighGain, 4),
        (AmpModel::HighGain, 8),
    ] {
        let mut s = amp_settings(m);
        s.gain = 0.6;
        s.oversampling = os;
        let mut a = fresh(&s);
        let w = std::f64::consts::TAU * k0 as f64 / n as f64;
        let x: Vec<f32> = (0..3 * n)
            .map(|i| (0.1 * crate::math::sin(w * i as f64)) as f32)
            .collect();
        let (y, _) = run(&mut a, &x, 512);
        let y = &y[2 * n..];
        let (mut harm, mut other) = (0.0f64, 0.0f64);
        for bin in 1..n / 2 {
            let wb = std::f64::consts::TAU * bin as f64 / n as f64;
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, v) in y.iter().enumerate() {
                let (si, co) = crate::math::sin_cos(wb * i as f64);
                re += *v as f64 * co;
                im += *v as f64 * si;
            }
            let p = re * re + im * im;
            if bin % k0 == 0 {
                harm += p;
            } else {
                other += p;
            }
        }
        let db = 10.0 * crate::math::log10(other / harm);
        println!("{m:?} {os}x: aliasing {db:.1} dB below the harmonics");
        res.push(db);
    }
    assert!(res[0] > res[1] && res[1] > res[2], "{res:?}");
    assert!(res[3] > res[4] && res[4] > res[5], "{res:?}");
    assert!(res[1] < -40.0 && res[4] < -28.0, "4x: {res:?}");
}

/// CPU per model, a plucked note, release build: `--ignored --nocapture`.
#[test]
#[ignore]
fn cpu_per_model() {
    let x = pluck(110.0, 1.0, 0.4, 4);
    for os in [2, 4, 8] {
        for m in AmpModel::ALL {
            let mut s = amp_settings(m);
            s.oversampling = os;
            s.cab = true;
            let mut a = Arca::with_cab(SR, cab_kernel(Cabinet::FourTwelveVintage));
            a.set(&s);
            a.reset();
            let mut best = f64::MAX;
            for _ in 0..3 {
                let (mut l, mut r) = (x.clone(), x.clone());
                let t = std::time::Instant::now();
                for (bl, br) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
                    a.process(bl, br);
                }
                best = best.min(t.elapsed().as_secs_f64());
            }
            println!(
                "{os}x {m:?}: {:.2} % of a core (48 kHz, with its cabinet)",
                best * 100.0
            );
        }
    }
}

/// The power amp's small-signal gain, inverter input to speaker, against the closed loop
/// worked by hand: the open loop `A = A_pi·n·g_m·Raa/(1 + n·g_p·Raa/2)·√(R_spk/Raa)` from
/// the pentodes' conductances at idle, closed by the feedback `A / (1 + A·β)`.
#[test]
fn the_power_amp_closes_its_loop() {
    let rate = 192_000.0;
    for m in AmpModel::ALL {
        let spec = m.power();
        let mut p = Power::new(spec, rate);
        let (vb, b) = p.bias_and_supply();
        let vk = p.idle_cathode();
        let (_, gm, gp) = spec.tube.current(vb - vk, b - vk, b - vk);
        let turns = (spec.r_speaker / spec.raa).sqrt();
        // Each side's plate conductance shares the load: V = n·g_m·Raa·v_g / (1 + n·g_p·Raa/2).
        let n = spec.per_side;
        // The inverter's output resistance against the grid leak: a divider.
        let div = spec.r_grid / (spec.pi_r + spec.r_grid);
        let a = spec.pi_gain * div * n * gm * spec.raa / (1.0 + n * gp * spec.raa / 2.0) * turns;
        // A cathode-biased pair's shared cathode is bypassed: no degeneration at 1 kHz.
        // At 1 kHz (s = jω), presence at noon: the feedback through its presence network,
        // or the top-boost amp's cut between the inverter's outputs.
        let jw = std::f64::consts::TAU * 1_000.0;
        let (mut hr, mut hi) = (0.0, 0.0);
        if let Some(f) = spec.feedback {
            let share = (f.tap / spec.r_speaker).sqrt();
            let (rf, rt) = (f.r_fb, f.r_tail);
            match f.presence {
                Some((pot, c)) => {
                    let rp = 1.0 + 0.5 * pot;
                    // rt(1 + jωC·rp) / ((rf + rt) + jωC(rf(rt + rp) + rt·rp)).
                    let (nr, ni) = (rt, rt * jw * c * rp);
                    let (dr, di) = (rf + rt, jw * c * (rf * (rt + rp) + rt * rp));
                    let d = dr * dr + di * di;
                    hr = share * (nr * dr + ni * di) / d;
                    hi = share * (ni * dr - nr * di) / d;
                }
                None => hr = share * rt / (rf + rt),
            }
        }
        let mut gain = a;
        if let Some((pot, c)) = spec.cut {
            let rc = 1.0 + 0.5 * pot;
            let (nr, ni) = (1.0, jw * rc * c);
            let (dr, di) = (1.0, jw * (rc + 2.0 * spec.pi_r) * c);
            gain *= ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt();
        }
        // |A / (1 + A·H)|.
        let (er, ei) = (1.0 + gain * hr, gain * hi);
        let closed = gain / (er * er + ei * ei).sqrt();
        // A 1 kHz sine of 1 mV at the inverter, read at the tap after it settles.
        let amp = 1e-3;
        let w = std::f64::consts::TAU * 1_000.0 / rate;
        let n = 192_000;
        let y: Vec<f64> = (0..n)
            .map(|i| p.step(amp * crate::math::sin(w * i as f64)))
            .collect();
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in y[n / 2..].iter().enumerate() {
            let (s, c) = crate::math::sin_cos(w * (i + n / 2) as f64);
            re += v * s;
            im += v * c;
        }
        let got = 2.0 * (re * re + im * im).sqrt() / (n / 2) as f64;
        let full = (2.0 * spec.watts * spec.r_speaker).sqrt();
        let got_v = got * full / amp;
        let d = 20.0 * crate::math::log10(got_v / closed);
        println!(
            "{m:?}: bias {vb:.1} V, cathode {vk:.1} V, supply {b:.0} V, gm {:.1} mA/V; gain {got_v:.2} against {closed:.2} ({d:+.2} dB), feedback {:.1} dB",
            gm * 1e3,
            20.0 * crate::math::log10(1.0 + gain * p.beta())
        );
        assert!(d.abs() < 0.5, "{m:?}: {d:.2} dB");
    }
}

/// A loud chord pulls the supply down; it recovers once the chord stops. A tube
/// rectifier (the clean amp's) sags more than the stiff solid-state one (the crunch's),
/// and the sag setting scales it.
#[test]
fn the_supply_sags_and_recovers() {
    let rate = 192_000.0;
    let sag_of = |m: AmpModel, f: f64| {
        let mut p = Power::new(m.power(), rate);
        p.set_sag(f);
        let idle = p.supply();
        let w = std::f64::consts::TAU * 220.0 / rate;
        let mut low = idle;
        for i in 0..(0.3 * rate) as usize {
            p.step(3.0 * crate::math::sin(w * i as f64));
            low = low.min(p.supply());
        }
        for _ in 0..(0.5 * rate) as usize {
            p.step(0.0);
        }
        (idle - low, (p.supply() - idle).abs())
    };
    let (tube, back) = sag_of(AmpModel::Clean, 1.0);
    let (stiff, _) = sag_of(AmpModel::Crunch, 1.0);
    let (soft, _) = sag_of(AmpModel::Clean, 3.0);
    println!("clean (tube rectifier) sags {tube:.1} V (soft {soft:.1} V), crunch {stiff:.1} V; back within {back:.3} V");
    assert!(
        tube > 2.0 * stiff && soft > tube && back < 0.5,
        "{tube} {stiff} {soft} {back}"
    );
}

/// The factory cabinets: start at once (no latency), are minimum phase (their energy
/// up front), follow their design within 1.5 dB from 80 Hz to 6 kHz, and fall as a guitar
/// speaker does past its break-up.
#[test]
fn the_cabinets_are_what_they_were_designed_to_be() {
    let rate = 48_000.0f64;
    for c in Cabinet::ALL {
        for mic in Mic::ALL {
            let s = CabSettings {
                cabinet: c,
                mic,
                position: 0.3,
                distance_cm: 2.0,
            };
            let h = impulse(&s, SR);
            assert_eq!(h.len(), 2_048);
            let e: Vec<f64> = h.iter().map(|v| (*v as f64) * (*v as f64)).collect();
            let total: f64 = e.iter().sum();
            let first: f64 = e[..240].iter().sum();
            let peak = h.iter().enumerate().fold((0, 0.0f32), |b, (i, v)| {
                if v.abs() > b.1 {
                    (i, v.abs())
                } else {
                    b
                }
            });
            assert!(peak.0 < 48, "{c:?} {mic:?}: its peak at {}", peak.0);
            assert!(
                first / total > 0.9,
                "{c:?} {mic:?}: {:.2} in 5 ms",
                first / total
            );
            let hd: Vec<f64> = h.iter().map(|v| *v as f64).collect();
            let mut mean = 0.0;
            let pts = 64;
            for k in 0..pts {
                let f = 100.0 * crate::math::powf(50.0f64, k as f64 / (pts - 1) as f64);
                mean += design_db(&s, f as f32) as f64;
            }
            mean /= pts as f64;
            for hz in [80.0, 150.0, 400.0, 1_000.0, 2_500.0, 4_000.0, 6_000.0] {
                let got = gain_db(&hd, 1.0, rate, hz);
                let want = design_db(&s, hz as f32) as f64 - mean;
                assert!(
                    (got - want).abs() < 1.5,
                    "{c:?} {mic:?} at {hz} Hz: {got:.1} against {want:.1}"
                );
            }
            let bite = gain_db(&hd, 1.0, rate, 2_500.0);
            let top = gain_db(&hd, 1.0, rate, 9_000.0);
            assert!(bite - top > 15.0, "{c:?} {mic:?}: {bite:.1} → {top:.1}");
        }
    }
}

/// The cabinet alone (no amp): an impulse comes out at its first sample (no latency).
#[test]
fn the_cabinet_alone_has_no_latency() {
    let s = ArcaSettings {
        cab: true,
        ..ArcaSettings::default()
    };
    let mut a = Arca::with_cab(SR, cab_kernel(Cabinet::FourTwelveModern));
    a.set(&s);
    a.reset();
    assert_eq!(a.latency(), 0);
    let mut x = vec![0.0f32; 1_024];
    x[0] = 1.0;
    let (l, _) = run(&mut a, &x, 64);
    let h = impulse(
        &CabSettings {
            cabinet: Cabinet::FourTwelveModern,
            ..CabSettings::default()
        },
        SR,
    );
    for i in 0..1_024 {
        assert!((l[i] - h[i]).abs() < 1e-5, "{i}: {} against {}", l[i], h[i]);
    }
    // And with an amp the latency is the oversampler's, the dry lined up with it.
    let mut s = amp_settings(AmpModel::Clean);
    s.mix = 0.0;
    let mut a = fresh(&s);
    assert_eq!(a.latency(), crate::fervor::os::latency(4, false));
    let (l, _) = run(&mut a, &x, 64);
    let lat = a.latency();
    assert_eq!(l[lat], 1.0);
    assert!(l.iter().enumerate().all(|(i, v)| i == lat || *v == 0.0));
}

/// Each part's cost at 4× and the Newton iterations a sample per circuit:
/// `--ignored --nocapture`, release.
#[test]
#[ignore]
fn cpu_by_part() {
    let rate = 192_000.0;
    // Parts' cost.
    for m in AmpModel::ALL {
        let mut p = Preamp::new(m.preamp(), SOURCE_OHMS, rate, &Knobs::default());
        let mut pw = Power::new(m.power(), rate);
        let x = pluck(110.0, 0.25, 0.4, 4);
        let t = std::time::Instant::now();
        let mut acc = 0.0;
        for v in &x {
            for _ in 0..4 {
                acc += p.step(*v as f64);
            }
        }
        let tp = t.elapsed().as_secs_f64() * 4.0;
        let t = std::time::Instant::now();
        for v in &x {
            for _ in 0..4 {
                acc += pw.step(*v as f64);
            }
        }
        let tw = t.elapsed().as_secs_f64() * 4.0;
        let its: Vec<String> = p
            .stages()
            .iter()
            .map(|s| format!("{:.2}", s.c.iterations_per_step()))
            .collect();
        println!(
            "{m:?}: preamp {:.1} %, power {:.1} % ({acc:.1}); Newton per step {its:?}",
            tp * 100.0,
            tw * 100.0
        );
    }
}

/// What is not a harmonic of a sine on bin `k0` of `n`, against what is (dB).
pub(super) fn inharmonic_db(y: &[f32], n: usize, k0: usize) -> f64 {
    let (mut harm, mut other) = (0.0f64, 0.0f64);
    for bin in 1..n / 2 {
        let wb = std::f64::consts::TAU * bin as f64 / n as f64;
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, v) in y.iter().enumerate() {
            let (si, co) = crate::math::sin_cos(wb * i as f64);
            re += *v as f64 * co;
            im += *v as f64 * si;
        }
        let p = re * re + im * im;
        if bin % k0 == 0 {
            harm += p;
        } else {
            other += p;
        }
    }
    10.0 * crate::math::log10(other / harm)
}

/// Overdriven far past clipping (a 34 V near-square into the inverter), every power amp
/// stays periodic: what is not a harmonic stays 35 dB down (a hard floor under the plate
/// trapped Newton in a two-cycle and made it −14 dB).
#[test]
fn an_overdriven_power_amp_stays_periodic() {
    let n = 32_768;
    let k0 = 427; // 427 · 192 000 / 32 768 ≈ 2 502 Hz
    let w = std::f64::consts::TAU * k0 as f64 / n as f64;
    for m in AmpModel::ALL {
        let mut pw = Power::new(m.power(), 192_000.0);
        let y: Vec<f32> = (0..3 * n)
            .map(|i| {
                pw.step(34.0 * crate::math::tanh(10.0 * crate::math::sin(w * i as f64))) as f32
            })
            .collect();
        let db = inharmonic_db(&y[2 * n..], n, k0);
        println!("{m:?}: {db:.1} dB");
        assert!(db < -35.0, "{m:?}: {db:.1} dB");
    }
}
