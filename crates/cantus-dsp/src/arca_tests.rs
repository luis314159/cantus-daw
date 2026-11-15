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
    let mut line: Vec<f32> = (0..len).map(|i| hash(seed * 1_000_003 + i as u64)).collect();
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
    let a1 = (c1 * r1 + c1 * r3 + c2 * r3 + c2 * r4 + c3 * r4) + m * c3 * r3 + l * (c1 * r2 + c2 * r2);
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
                o.iter().zip(&outs[0]).all(|(a, b)| a.to_bits() == b.to_bits()),
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
        println!("{m:?}: at rest after {:.2} s of silence", blocks as f32 * 256.0 / SR);
        assert!(b.at_rest());
        for _ in 0..40 {
            let (la, _) = run(&mut a, &silence, 256);
            assert!(la.iter().all(|v| v.to_bits() == 0), "{m:?}: silence at rest");
        }
        b.skip(40 * 256);
        let (la, _) = run(&mut a, &x, 100);
        let (lb, _) = run(&mut b, &x, 100);
        assert!(la.iter().zip(&lb).all(|(p, q)| p.to_bits() == q.to_bits()), "{m:?}");
        assert!(la.iter().any(|v| *v != 0.0));
    }
}
