//! Model 2's perspective (round 37, voicing pass two): the microphones, the pair's width,
//! the lid and the room's size are each exact at their defaults, glide when moved (no
//! zipper), keep every family rule (any block size, an exact rest) and do what they say —
//! measured on the output, not on the gains.

use super::stage::{self, Stage};
use super::tests_engine::{perform, Ev};
use super::tests_model2::piano2;
use super::*;

fn with(f: impl FnOnce(&mut ChordaSettings)) -> Chorda {
    let mut c = piano2();
    let mut s = ChordaSettings {
        model: 2,
        ..ChordaSettings::default()
    };
    f(&mut s);
    c.set(&s, 0.0);
    c
}

fn chord() -> Vec<(usize, Ev)> {
    vec![
        (0, Ev::On(48, 0.7, 1)),
        (3, Ev::On(60, 0.7, 2)),
        (6, Ev::On(64, 0.7, 3)),
        (9, Ev::On(79, 0.7, 4)),
        (30_000, Ev::Off(48, 1)),
        (30_000, Ev::Off(60, 2)),
        (30_000, Ev::Off(64, 3)),
        (30_000, Ev::Off(79, 4)),
    ]
}

fn energy(x: &[f32]) -> f64 {
    x.iter().map(|v| (*v as f64) * (*v as f64)).sum()
}

/// The energy above or below `hz`: two cascaded second-order filters (RBJ, Q 0.707), f64.
fn band(x: &[f32], hz: f64, high: bool) -> f64 {
    let w = std::f64::consts::TAU * hz / 48_000.0;
    let (c, al) = (crate::math::cos(w), crate::math::sin(w) / (2.0 * 0.707));
    let a0 = 1.0 + al;
    let (b0, b1, b2) = if high {
        ((1.0 + c) / 2.0 / a0, -(1.0 + c) / a0, (1.0 + c) / 2.0 / a0)
    } else {
        ((1.0 - c) / 2.0 / a0, (1.0 - c) / a0, (1.0 - c) / 2.0 / a0)
    };
    let (a1, a2) = (-2.0 * c / a0, (1.0 - al) / a0);
    let mut st = [[0.0f64; 4]; 2];
    let mut e = 0.0;
    for v in x {
        let mut y = *v as f64;
        for s in st.iter_mut() {
            let out = b0 * y + b1 * s[0] + b2 * s[1] - a1 * s[2] - a2 * s[3];
            *s = [y, s[0], out, s[2]];
            y = out;
        }
        e += y * y;
    }
    e
}

/// The energy above 5 kHz.
fn top(x: &[f32]) -> f64 {
    band(x, 5_000.0, true)
}

#[test]
fn every_control_at_its_stored_default_plays_the_voiced_piano_to_the_bit() {
    // A parameter stores single precision: 0.35 reads back as 0.349999994 — still the default.
    // (The width also spreads the keys, model 1's way, at whatever it reads.)
    let ev = chord();
    let reference = perform(&mut with(|s| s.width = 0.7f32 as f64), &ev, 40_000, 512, 1);
    let mut c = with(|s| {
        s.persp = 0.35f32 as f64;
        s.width = 0.7f32 as f64;
        s.lid = 1.0;
        s.size = 0.4f32 as f64;
    });
    assert!(perform(&mut c, &ev, 40_000, 512, 1) == reference);
}

#[test]
fn moved_controls_keep_every_block_size_and_the_threads_to_the_bit() {
    let ev = chord();
    let set = |s: &mut ChordaSettings| {
        s.persp = 0.1;
        s.width = 0.3;
        s.lid = 0.4;
        s.size = 0.9;
    };
    let reference = perform(&mut with(set), &ev, 40_000, 512, 1);
    for block in [1, 37, 64, 300] {
        assert!(
            perform(&mut with(set), &ev, 40_000, block, 1) == reference,
            "{block}"
        );
    }
    assert!(perform(&mut with(set), &ev, 40_000, 333, 4) == reference);
}

#[test]
fn the_perspective_moves_from_the_hammers_to_the_audience() {
    let ev = chord();
    let near = perform(&mut with(|s| s.persp = 0.0), &ev, 48_000, 512, 1);
    let far = perform(&mut with(|s| s.persp = 1.0), &ev, 48_000, 512, 1);
    // Far: the room's share grows (what rings on after the keys are let go, against what
    // sounded while they were held).
    let after = |x: &[f32]| energy(&x[40_000..]) / energy(&x[..30_000]);
    let (an, af) = (after(&near.0), after(&far.0));
    assert!(af > 2.0 * an, "the room's tail: near {an}, far {af}");
}

/// The air: the same sound through the stage, its top (over 5 kHz) against its bottom (under
/// 1 kHz) — under the player's in the audience, over it at the hammers.
#[test]
fn the_air_darkens_the_far_perspective_and_brightens_the_near() {
    let share = |p: f64| {
        let mut st = Stage::new(48_000.0);
        st.set(p, stage::WIDTH, 1.0);
        st.snap();
        let mut seed = 0x1234_5678u32;
        let mut out = Vec::with_capacity(48_000);
        for _ in 0..48_000 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let x = (seed >> 8) as f64 / (1u32 << 24) as f64 - 0.5;
            let mut o = st.mix([x, x], [0.0, 0.0]);
            st.lid(&mut o);
            let o = st.wet(o, [0.0, 0.0], 0.0);
            out.push(o[0] as f32);
        }
        top(&out) / band(&out, 1_000.0, false)
    };
    let (near, player, far) = (share(0.0), share(stage::PERSP), share(1.0));
    let db = |r: f64| 10.0 * crate::math::log10(r);
    eprintln!(
        "top's share against the player's: near {:+.1} dB, far {:+.1} dB",
        db(near / player),
        db(far / player)
    );
    assert!(db(far / player) < -4.0);
    assert!(db(near / player) > 1.0);
}

#[test]
fn a_closed_lid_takes_the_top_away() {
    let ev = chord();
    let open = perform(&mut with(|s| s.lid = 1.0), &ev, 30_000, 512, 1);
    let shut = perform(&mut with(|s| s.lid = 0.0), &ev, 30_000, 512, 1);
    let bright = |x: &[f32]| top(x) / energy(x);
    let ratio = bright(&shut.0) / bright(&open.0);
    assert!(ratio < 0.5, "closed against open, the top's share: {ratio}");
}

#[test]
fn the_width_never_touches_the_mono_sum_of_the_board_and_the_room() {
    let mut a = Stage::new(48_000.0);
    let mut b = Stage::new(48_000.0);
    a.set(stage::PERSP, 0.0, 1.0);
    b.set(stage::PERSP, 1.0, 1.0);
    a.snap();
    b.snap();
    for i in 0..2_000 {
        let x = [
            crate::math::sin((i as f64) * 0.013),
            crate::math::cos((i as f64) * 0.029),
        ];
        let d = [0.3 * x[0], 0.3 * x[1]];
        let (oa, ob) = (a.mix(d, x), b.mix(d, x));
        assert!(((oa[0] + oa[1]) - (ob[0] + ob[1])).abs() < 1e-12);
        let (wa, wb) = (a.wet(oa, x, 0.5), b.wet(ob, x, 0.5));
        assert!(((wa[0] + wa[1]) - (wb[0] + wb[1])).abs() < 1e-12);
    }
}

#[test]
fn a_larger_room_rings_longer_and_still_comes_to_rest() {
    let ev = [(0, Ev::On(60, 0.9, 1)), (2_400, Ev::Off(60, 1))];
    let small = perform(&mut with(|s| s.size = 0.0), &ev, 96_000, 512, 1);
    let mut big_unit = with(|s| s.size = 1.0);
    let big = perform(&mut big_unit, &ev, 96_000, 512, 1);
    let tail = |x: &[f32]| energy(&x[48_000..]);
    assert!(tail(&big.0) > 4.0 * tail(&small.0));
    let (mut l, mut r) = (vec![0.0; 4_800], vec![0.0; 4_800]);
    let mut n = 0;
    while !big_unit.at_rest() && n < 600 {
        big_unit.process(&mut l, &mut r);
        n += 1;
    }
    assert!(big_unit.at_rest(), "rests within a minute");
}

/// A control moved while a chord rings glides: the difference it makes grows over its
/// glide, never in a step (a zipper or a click).
#[test]
fn a_moved_control_glides_without_a_step() {
    let ev = chord();
    let base = perform(&mut piano2(), &ev, 20_000, 512, 1);
    for change in 0..4 {
        let mut c = piano2();
        perform(&mut c, &ev, 10_000, 512, 1);
        let mut s = ChordaSettings {
            model: 2,
            ..ChordaSettings::default()
        };
        match change {
            0 => s.persp = 0.0,
            1 => s.width = 0.0,
            2 => s.lid = 0.0,
            _ => s.size = 1.0,
        }
        c.set(&s, 0.0);
        let (mut l, mut r) = (vec![0.0f32; 10_000], vec![0.0f32; 10_000]);
        c.process(&mut l, &mut r);
        let diff: Vec<f64> = l
            .iter()
            .zip(&base.0[10_000..])
            .map(|(a, b)| (*a - *b) as f64)
            .collect();
        let early: f64 = diff[..24].iter().map(|d| d * d).sum::<f64>() / 24.0;
        let later: f64 = diff[2_400..4_800].iter().map(|d| d * d).sum::<f64>() / 2_400.0;
        assert!(later > 0.0, "change {change} changed nothing");
        assert!(
            early < 0.05 * later,
            "change {change}: a step ({early} against {later})"
        );
    }
}

/// The body's two responses' correlation per third-octave (zero lag, each band filtered by
/// its own bins of one long FFT), for a voicing with and without the band's coherence held.
#[test]
#[ignore = "a measurement: prints the bands"]
fn measure_the_bodys_band_correlation() {
    use super::body::{responses, BodyVoicing};
    let fs = 48_000.0;
    for coh in [0.0, 200.0] {
        let v = BodyVoicing {
            even: 1.0,
            coh_hz: coh,
            ..BodyVoicing::default()
        };
        let [l, r] = responses(&v, fs);
        let n = l.len().next_power_of_two() * 2;
        let dft = |x: &[f64], k: usize| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, v) in x.iter().enumerate() {
                let a = -std::f64::consts::TAU * (k * i) as f64 / n as f64;
                re += v * crate::math::cos(a);
                im += v * crate::math::sin(a);
            }
            (re, im)
        };
        let mut line = format!("coh {coh:5.0}:");
        let mut c = 100.0f64;
        while c < 6_000.0 {
            let (k0, k1) = (
                (c / 1.122 * n as f64 / fs) as usize,
                (c * 1.122 * n as f64 / fs) as usize,
            );
            let (mut x, mut el, mut er) = (0.0, 0.0, 0.0);
            for k in k0..=k1 {
                let (a, b) = (dft(&l, k), dft(&r, k));
                x += a.0 * b.0 + a.1 * b.1;
                el += a.0 * a.0 + a.1 * a.1;
                er += b.0 * b.0 + b.1 * b.1;
            }
            line += &format!(" {c:.0}:{:+.2}", x / (el * er).sqrt());
            c *= 1.26;
        }
        eprintln!("{line}");
    }
}

/// Model 2's width moves only the microphones' side signal (review 2 #7): the outer keys
/// keep their places, so the mono sum of a phrase on them is the same at any width.
#[test]
fn model_2s_width_never_moves_the_mono_sum_of_the_outer_keys() {
    let ev = vec![
        // Soft: the output ceiling (per channel, past −4.4 dBFS) stays out of it.
        (0, Ev::On(21, 0.3, 1)),
        (5, Ev::On(108, 0.3, 2)),
        (9_000, Ev::Off(21, 1)),
        (9_000, Ev::Off(108, 2)),
    ];
    let narrow = perform(&mut with(|s| s.width = 0.1), &ev, 20_000, 512, 1);
    let wide = perform(&mut with(|s| s.width = 1.0), &ev, 20_000, 512, 1);
    let peak = narrow
        .0
        .iter()
        .fold(0.0f32, |m, v| m.max(v.abs()))
        .max(1e-9);
    let worst = (0..narrow.0.len())
        .map(|i| ((narrow.0[i] + narrow.1[i]) - (wide.0[i] + wide.1[i])).abs())
        .fold(0.0f32, f32::max);
    assert!(
        worst < 1e-5 * peak,
        "mono sum moved by {worst} (peak {peak})"
    );
    assert!(narrow.0 != wide.0, "the width still widens");
}
