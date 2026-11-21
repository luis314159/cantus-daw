//! Voicing pass three (round 40, ADR 0098's third amendment): what the new voicing's parts
//! hold. The frequency-dependent loss by register is `hf_decay` itself at its default; a
//! register's level moves it alone; the output ceiling bends nothing under its knee; a
//! damped bass key rests within seconds with the damped floor on; and a voicing with every
//! part away from its default keeps the family's rules — any block size, any thread count,
//! an idle unit plays as a new one, an exact rest. Model 2's bits at the voicing beta.14
//! shipped are pinned.

use super::registers::BANDS;
use super::tests_engine::{perform, Ev};
use super::tests_hash::hash;
use super::tests_model2::piano2;
use super::voicing::Voicing;
use super::*;
use crate::math;

fn voiced(v: &Voicing) -> Chorda {
    let mut c = piano2();
    c.set_voicing(v);
    c
}

/// Model 2 at beta.14's voicing, named (since `DSP_VERSION` 7 a new Chorda plays candidate
/// 16): every part of this file is held neutral against it.
fn beta14_piano() -> Chorda {
    voiced(&Voicing::beta14())
}

/// A phrase through every part model 2 has: the bass, a loud chord, the pedal, the soft
/// pedal, a re-strike of a ringing key, the top keys without dampers.
fn phrase() -> Vec<(usize, Ev)> {
    let mut ev = vec![
        (0, Ev::On(21, 0.95, 1)),
        (40, Ev::On(33, 0.72, 2)),
        (2_000, Ev::Pedal(0.6)),
        (3_000, Ev::On(48, 1.0, 3)),
        (3_004, Ev::On(55, 0.98, 4)),
        (3_008, Ev::On(64, 0.97, 5)),
        (3_012, Ev::On(72, 1.0, 6)),
        (9_000, Ev::Off(33, 2)),
        (11_000, Ev::On(48, 0.9, 7)),
        (12_000, Ev::Soft(1.0)),
        (12_500, Ev::On(60, 0.45, 8)),
        (15_000, Ev::Pedal(0.0)),
        (15_001, Ev::Off(21, 1)),
        (16_000, Ev::Soft(0.0)),
        (18_000, Ev::On(100, 0.85, 9)),
        (18_400, Ev::Off(100, 9)),
    ];
    ev.sort_by_key(|e| e.0);
    ev
}

/// Every part of the third pass at once, away from its default.
fn third_pass() -> Voicing {
    let mut reg = [[0.0; BANDS]; 3];
    reg[0][2] = -4.0;
    reg[0][5] = 5.0;
    reg[1][4] = -3.0;
    reg[2][6] = 2.0;
    Voicing {
        reg,
        hf_bass: 0.5,
        hf_treble: 1.3,
        reg_level: [-3.0, 0.0, 2.0],
        damped_db: 70.0,
        knee: 0.85,
        ceiling: 0.97,
        uni_weight: 0.3,
        uni_skew: 0.5,
        polar: 0.2,
        polar_ct: 0.4,
        polar_yz: 0.15,
        polar_damp: 0.3,
        damper_bass: 0.5,
        itd_ms: 0.5,
        itd_jitter: 0.3,
        formant: [600.0, 6.0, 3.0],
        bridge: [5.0, 25.0, 0.6, 10.0],
        bass_hp: 45.0,
        knock_vel: 0.8,
        landing_shape: [0.12, 700.0, 0.2, 0.5],
        vmin_treble: 1.3,
        even: 1.0,
        even_thud: 0.5,
        uni_b: 0.05,
        uni_b_bass: 4.0,
        uni_loss: 0.6,
        detune_bass: 0.6,
        detune_scale: 0.8,
        felt_upper: 0.6,
        coupling_bass: 1.3,
        bass_hp_order: 4.0,
        rise: [3.0, 4.0, 5.0, 2.0, 3.0],
        top_damper: 0.2,
        landing_rise: 0.3,
        pp_trim: [2.0, 4.0, 0.0, -3.0],
        uni_rank: 1.0,
        detune_top: [84.0, 0.2],
        vel_top: [84.0, 0.7, 0.8],
        itd_ends: 0.3,
        uni_weight_bass: 0.4,
        hf_top: [84.0, 2.0],
        damper_key: 2.0,
        strike_true: 1.0,
        vmin_bass: 0.8,
        detune_low: [0.8, 84.0],
        felt_soft: [1.5, 55.0, 79.0],
        ..Voicing::beta14()
    }
}

#[test]
fn the_loss_by_register_is_hf_decay_itself_at_its_default_and_glides_off_it() {
    let v = Voicing::beta14();
    for m in 21..=108u8 {
        assert_eq!(v.hf_of(m), v.hf_decay, "key {m}");
    }
    let w = Voicing {
        hf_bass: 0.5,
        hf_treble: 2.0,
        ..v
    };
    assert!((w.hf_of(21) - 0.5 * v.hf_decay).abs() < 1e-12);
    assert!((w.hf_of(60) - v.hf_decay).abs() < 1e-12);
    assert!((w.hf_of(108) - 2.0 * v.hf_decay).abs() < 1e-12);
    for m in 21..108u8 {
        assert!(w.hf_of(m + 1) >= w.hf_of(m), "key {m}: not monotonic");
    }
}

#[test]
fn a_registers_level_moves_that_register_alone() {
    let lvl = |reg_level: [f64; 3]| Voicing {
        reg_level,
        ..Voicing::beta14()
    };
    let at = |v: &Voicing, key: u8| {
        let ev = vec![(0, Ev::On(key, 0.6, 1))];
        let (l, r) = perform(&mut voiced(v), &ev, 24_000, 256, 1);
        l.iter()
            .chain(&r)
            .map(|x| (*x as f64) * (*x as f64))
            .sum::<f64>()
    };
    let flat = Voicing::beta14();
    let bass = lvl([-6.0, 0.0, 0.0]);
    // A0 sits in the bass register alone: 6 dB down. The middle's F4 not at all.
    let d = 10.0 * math::log10(at(&bass, 21) / at(&flat, 21));
    assert!((d + 6.0).abs() < 0.5, "A0: {d:.2} dB");
    let (a, b) = (at(&bass, 65), at(&flat, 65));
    assert_eq!(a, b, "F4 moved");
    // 0 dB is not applied at all: the flat voicing's bits.
    let ev = phrase();
    let x = perform(&mut voiced(&lvl([0.0; 3])), &ev, 12_000, 256, 1);
    let y = perform(&mut beta14_piano(), &ev, 12_000, 256, 1);
    assert!(x == y);
}

#[test]
fn the_ceiling_bends_nothing_under_its_knee_and_never_passes_its_top() {
    use super::engine::ceiling;
    for i in 0..=1000 {
        let x = i as f64 / 1000.0 * 0.89;
        assert_eq!(ceiling(x, 0.89, 0.98), x);
        assert_eq!(ceiling(-x, 0.89, 0.98), -x);
    }
    let mut last = 0.0;
    for i in 0..=4000 {
        let x = 0.89 + i as f64 / 100.0;
        let y = ceiling(x, 0.89, 0.98);
        assert!(y <= 0.98 && y >= last, "{x}: {y}");
        last = y;
    }
}

#[test]
fn the_third_pass_gives_the_same_bits_at_every_block_size_and_on_threads() {
    let v = third_pass();
    let ev = phrase();
    let frames = 24_000;
    let reference = perform(&mut voiced(&v), &ev, frames, 1, 1);
    for (block, threads) in [(7, 1), (37, 1), (64, 1), (100, 4), (300, 1), (512, 3)] {
        let got = perform(&mut voiced(&v), &ev, frames, block, threads);
        assert!(got == reference, "block {block}, {threads} threads");
    }
}

#[test]
fn an_idle_voiced_unit_plays_as_a_new_one() {
    let v = third_pass();
    let ev = phrase();
    let fresh = perform(&mut voiced(&v), &ev, 20_000, 256, 1);
    let mut idle = voiced(&v);
    // Played, let ring to rest, then idle: nothing of the first strikes is left.
    let short = vec![(0, Ev::On(60, 1.0, 1)), (100, Ev::Off(60, 1))];
    perform(&mut idle, &short, 512, 512, 1);
    let mut frames: usize = 0;
    while !idle.at_rest() {
        perform(&mut idle, &[], 4_800, 512, 1);
        frames += 4_800;
        assert!(frames < 48_000 * 60, "no rest within a minute");
    }
    // Back on the grid a new unit starts at (every clock is a multiple of 3 s).
    let to = (frames + 512).div_ceil(144_000) * 144_000;
    perform(&mut idle, &[], to - frames - 512, 512, 1);
    let after = perform(&mut idle, &ev, 20_000, 256, 1);
    assert!(fresh == after, "an idle unit's attacks moved");
}

#[test]
fn the_third_pass_comes_to_an_exact_rest() {
    rests(voiced(&third_pass()));
}

fn rests(mut c: Chorda) {
    // Keys with dampers that rest within seconds (the top ones ring on by design; A0…E1
    // struck hard take 45–120 s to fall under model 2's quiet floor even at beta.14's
    // voicing — `docs/round40/chorda.md`).
    for (k, v) in [(33u8, 1.0f32), (60, 0.95), (84, 1.0)] {
        c.note_on(k, v, k as u64);
    }
    let mut l = vec![0.0f32; 512];
    let mut r = vec![0.0f32; 512];
    c.process(&mut l, &mut r);
    for k in [33u8, 60, 84] {
        c.note_off(k, k as u64);
    }
    let mut blocks = 0;
    while !c.at_rest() {
        l.fill(0.0);
        r.fill(0.0);
        c.process(&mut l, &mut r);
        blocks += 1;
        assert!(
            blocks < 48_000 * 60 / 512,
            "no rest within a minute: {:?}, {} voices",
            c.busy_parts(),
            c.active_voices()
        );
    }
    l.fill(0.0);
    r.fill(0.0);
    assert!(!c.process(&mut l, &mut r));
    assert!(l.iter().chain(&r).all(|x| *x == 0.0));
}

/// Model 2 at the voicing beta.14 shipped (round 37's), hashed: computed on the round-40
/// contract commit (f00ed550), before the third pass's parts existed — they change no bit
/// of it. Since `DSP_VERSION` 7 that voicing is named ([`Voicing::beta14`]), no longer the
/// default, and its bits are the same.
const MODEL_2_BETA14: u64 = 0x663b_f285_d17a_ebdd;

/// Model 2 as it ships since `DSP_VERSION` 11 (the technician's pass with the unison pass
/// over it, a new unit's own voicing with no `set_voicing`), on its own keyboard, hashed —
/// the same phrase as [`MODEL_2_BETA14`]'s.
const MODEL_2_SHIPPED: u64 = 0x9561_d8f8_a3ce_3a61;

/// Model 2 at the technician's pass as it shipped in `DSP_VERSION` 10 (beta.15, beta.16) —
/// the hash `MODEL_2_SHIPPED` held then; since `DSP_VERSION` 11 it is named
/// ([`Voicing::technician`]) and its bits are the same.
const MODEL_2_TECHNICIAN: u64 = 0x9561_d8f8_a3ce_3a61;

/// Model 2 at candidate 16 as it shipped from `DSP_VERSION` 7 to 9, on its own keyboard —
/// the hash `MODEL_2_SHIPPED` held then; since `DSP_VERSION` 10 it is named
/// ([`Voicing::candidate16`]) and its bits are the same.
const MODEL_2_C16: u64 = 0x717d_64b7_22b9_e71b;

/// A new model-2 unit on the keyboard `v` is designed for (one per voicing, made once), `v`
/// set on it unless it is the default (a new unit's own).
fn piano_for(v: Voicing) -> Chorda {
    use super::design::{Keyboard, Tuning};
    use std::sync::{Arc, Mutex, OnceLock};
    type Cache = Mutex<Vec<(Tuning, Arc<Keyboard>)>>;
    static K: OnceLock<Cache> = OnceLock::new();
    let t = v.tuning(Tuning::default());
    let cache = K.get_or_init(|| Mutex::new(Vec::new()));
    let found = cache
        .lock()
        .unwrap()
        .iter()
        .find(|e| e.0 == t)
        .map(|e| e.1.clone());
    let kb = found.unwrap_or_else(|| {
        let kb = Arc::new(Keyboard::compute(t, 48_000.0, 8));
        cache.lock().unwrap().push((t, kb.clone()));
        kb
    });
    let mut c = Chorda::new(48_000.0);
    c.set_keyboard(kb);
    let s = ChordaSettings {
        model: 2,
        ..ChordaSettings::default()
    };
    c.set(&s, 0.0);
    if v != Voicing::default() {
        c.set_voicing(&v);
    }
    c
}

/// A new model-2 unit on the keyboard its shipped voicing is designed for.
fn shipped_piano() -> Chorda {
    piano_for(Voicing::default())
}

/// Round 40, `DSP_VERSION` 7: candidate 16 is the lab's spec over beta.14's, its two tables,
/// the board and room it names (the ones made ahead); `DSP_VERSION` 10 shipped it with the
/// technician's pass over it (its spec, then its table on the keyboard), named since
/// `DSP_VERSION` 11 ([`Voicing::technician`]); what ships now is that with the unison pass
/// over it (its spec, then its table) — and a new unit plays it as `set_voicing` of it
/// would, to the bit.
#[test]
fn the_shipped_voicing_is_the_technicians_with_the_unison_pass_and_a_new_unit_plays_it() {
    use super::body::BodyVoicing;
    use super::room::RoomVoicing;
    let mut lab = Voicing::beta14();
    lab.partials = super::voicing::c16::PARTIALS;
    lab.levels = super::voicing::c16::LEVELS;
    assert!(
        lab.apply(super::voicing::c16::SPEC),
        "a key of the spec is not one"
    );
    assert_eq!(Voicing::candidate16(), lab);
    assert!(
        lab.apply(super::voicing::tune::SPEC),
        "a key of the technician's spec is not one"
    );
    lab.tune = super::voicing::tune::TUNE;
    assert_eq!(Voicing::technician(), lab);
    assert!(
        lab.apply(super::voicing::unison::SPEC),
        "a key of the unison spec is not one"
    );
    lab.tune = super::voicing::unison::TUNE;
    assert_eq!(Voicing::default(), lab);
    assert_eq!(Voicing::default(), Voicing::shipped());
    assert_ne!(Voicing::default(), Voicing::technician());
    assert_ne!(Voicing::default(), Voicing::candidate16());
    assert_eq!(Voicing::default().body, BodyVoicing::default());
    assert_eq!(Voicing::default().room, RoomVoicing::default());
    assert_ne!(Voicing::default(), Voicing::beta14());
    // A few of its numbers, as the lab's file has them (a later key wins).
    let v = Voicing::default();
    assert_eq!((v.decay_bass, v.coupling_bass), (0.24, 1.3));
    assert_eq!(
        (v.draws[35 - 21], v.draws[42 - 21], v.draws[50 - 21]),
        (1, 4, 1)
    );
    assert_eq!((v.reg[1][5], v.reg[0][5], v.reg[0][6]), (4.0, 11.0, 0.0));
    assert_eq!(v.hf_keys[72 - 21], 0.5);
    // The technician's pass: the three fixes on, the redrawn keys, the table on the keyboard.
    assert_eq!(
        (v.uni_center, v.bridge_tune, v.bridge_pull, v.frac_room),
        (1.0, 1.0, 2.0, 8.0)
    );
    assert_eq!((v.draws[24 - 21], v.draws[106 - 21]), (25, 31));
    // The unison pass: the pull held in hertz, each key's unison laid, the new table on the
    // keyboard.
    assert_eq!(v.bridge_pull_hz, 0.1);
    assert_eq!(v.tune[46 - 21], super::voicing::unison::TUNE[46 - 21]);
    assert_eq!(
        v.tuning(super::design::Tuning::default()).tune,
        super::voicing::unison::TUNE
    );
    // A new unit is the shipped voicing set on it.
    let ev = phrase();
    let new = perform(&mut shipped_piano(), &ev, 12_000, 256, 1);
    let mut set = shipped_piano();
    set.set_voicing(&Voicing::default());
    assert!(perform(&mut set, &ev, 12_000, 256, 1) == new);
}

#[test]
fn model_2_at_its_shipped_voicing_renders_to_the_bits_it_does() {
    let (l, r) = perform(&mut shipped_piano(), &phrase(), 24_000, 256, 1);
    let h = hash(&l, &r);
    eprintln!("model 2 (shipped voicing, the technician's + the unison pass) phrase hash: {h:#018x}");
    assert!(l.iter().chain(&r).any(|x| *x != 0.0));
    if std::env::var_os("CANTUS_GOLDEN_DUMP").is_some() {
        // The size of the change from the technician's pass (`DSP_VERSION` 10 → 11).
        let (cl, cr) = perform(
            &mut piano_for(Voicing::technician()),
            &phrase(),
            24_000,
            256,
            1,
        );
        let d: Vec<f64> = l
            .iter()
            .zip(&cl)
            .chain(r.iter().zip(&cr))
            .map(|(a, b)| (*a - *b) as f64)
            .collect();
        let max = d.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        let rms = (d.iter().map(|x| x * x).sum::<f64>() / d.len() as f64).sqrt();
        let rms_of = |a: &[f32], b: &[f32]| {
            (a.iter()
                .chain(b)
                .map(|x| (*x as f64) * (*x as f64))
                .sum::<f64>()
                / d.len() as f64)
                .sqrt()
        };
        let (was, now) = (rms_of(&cl, &cr), rms_of(&l, &r));
        eprintln!(
            "against DSP_VERSION 10: max |Δ| {max:.4}, RMS Δ {rms:.5} ({:.1} dBFS); the phrase's RMS {was:.4} -> {now:.4} ({:+.2} dB)",
            20.0 * crate::math::log10(rms.max(1e-12)),
            20.0 * crate::math::log10(now / was)
        );
    }
    assert_eq!(
        h, MODEL_2_SHIPPED,
        "model 2 at its shipped voicing moved: {h:#018x}"
    );
}

#[test]
fn model_2_at_the_technicians_pass_renders_to_the_bits_it_did() {
    let (l, r) = perform(
        &mut piano_for(Voicing::technician()),
        &phrase(),
        24_000,
        256,
        1,
    );
    let h = hash(&l, &r);
    eprintln!("model 2 (the technician's pass, DSP_VERSION 10) phrase hash: {h:#018x}");
    assert_eq!(
        h, MODEL_2_TECHNICIAN,
        "model 2 at the technician's pass moved: {h:#018x}"
    );
}

#[test]
fn model_2_at_candidate_16_renders_to_the_bits_it_did() {
    let (l, r) = perform(
        &mut piano_for(Voicing::candidate16()),
        &phrase(),
        24_000,
        256,
        1,
    );
    let h = hash(&l, &r);
    eprintln!("model 2 (candidate 16) phrase hash: {h:#018x}");
    assert_eq!(h, MODEL_2_C16, "model 2 at candidate 16 moved: {h:#018x}");
}

#[test]
fn model_2_at_beta14s_voicing_renders_to_the_bits_it_did() {
    let (l, r) = perform(&mut beta14_piano(), &phrase(), 24_000, 256, 1);
    let h = hash(&l, &r);
    eprintln!("model 2 (beta.14's voicing) phrase hash: {h:#018x}");
    assert_eq!(
        h, MODEL_2_BETA14,
        "model 2 at beta.14's voicing moved: {h:#018x}"
    );
}

/// Round 40: a hard-struck A0…E1, released, rests within seconds once its damper lands —
/// with the damped floor on. At beta.14's voicing (off) a 1.4 kHz loop mode of the lowest
/// strings rang on at −88 dBFS for minutes.
#[test]
fn a_damped_bass_key_rests_within_seconds() {
    let v = Voicing {
        damped_db: 70.0,
        ..Voicing::beta14()
    };
    for k in [21u8, 24, 28] {
        let mut c = voiced(&v);
        c.note_on(k, 1.0, 1);
        let mut l = vec![0.0f32; 4_800];
        let mut r = vec![0.0f32; 4_800];
        let mut tenths = 0;
        while !c.at_rest() {
            if tenths == 10 {
                c.note_off(k, 1);
            }
            l.fill(0.0);
            r.fill(0.0);
            c.process(&mut l, &mut r);
            tenths += 1;
            assert!(
                tenths < 100,
                "key {k}: no rest 9 s after its release: {:?}",
                c.busy_parts()
            );
        }
    }
    // Off, the same key still sounds ten seconds on.
    let mut c = beta14_piano();
    c.note_on(21, 1.0, 1);
    perform(&mut c, &[(48_000, Ev::Off(21, 1))], 48_000 * 10, 512, 1);
    assert!(!c.at_rest());
}

/// Round 40: the dampers' hold lengthens what a released middle C still sounds (the
/// reference's sounds 17 dB more 0.3–1.5 s after its release than beta.14's), and leaves
/// the held note alone.
#[test]
fn the_dampers_hold_lengthens_a_release_and_leaves_the_held_note_alone() {
    let held = |t60: f64| {
        let v = Voicing {
            damper_t60: t60,
            ..Voicing::beta14()
        };
        let ev = vec![(0, Ev::On(60, 0.6, 1)), (48_000, Ev::Off(60, 1))];
        let (l, _) = perform(&mut voiced(&v), &ev, 120_000, 256, 1);
        let e = |a: usize, b: usize| {
            10.0 * math::log10(
                l[a..b]
                    .iter()
                    .map(|x| (*x as f64) * (*x as f64))
                    .sum::<f64>()
                    + 1e-30,
            )
        };
        (e(0, 48_000), e(62_400, 120_000))
    };
    let (h1, t1) = held(1.0);
    let (h3, t3) = held(3.0);
    assert_eq!(h1, h3, "the held note moved");
    assert!(t3 > t1 + 6.0, "the release: {t1:.1} → {t3:.1} dB");
}

/// Round 40's string parts: the helpers are exact at their defaults.
#[test]
fn the_string_parts_are_nothing_at_their_defaults() {
    let v = Voicing::beta14();
    for m in 21..=108u8 {
        assert_eq!(v.damper_of(m), v.damper_t60, "key {m}");
        assert_eq!(v.lag_of(m, 48_000.0), 0, "key {m}");
    }
    let w = Voicing { itd_ms: 0.6, ..v };
    // The bass reaches the left microphone first (the right later), the treble the right.
    assert!(w.lag_of(21, 48_000.0) > 0 && w.lag_of(108, 48_000.0) < 0);
    assert!(w.lag_of(21, 48_000.0) <= 30, "{}", w.lag_of(21, 48_000.0));
    let d = Voicing {
        damper_bass: 0.5,
        ..v
    };
    assert!((d.damper_of(21) - 0.5 * v.damper_t60).abs() < 1e-12);
    assert_eq!(d.damper_of(60), v.damper_t60);
}

/// The level (dB) of `x[a..b]`.
fn level(x: &[f32], a: usize, b: usize) -> f64 {
    let e = x[a..b]
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        / (b - a) as f64;
    10.0 * math::log10(e + 1e-30)
}

/// A held A0 (one string): with the second polarisation it sings on — its late decay slower
/// than its early one, where the string alone falls at one rate.
#[test]
fn the_second_polarisation_gives_a_single_string_its_aftersound() {
    let held = |polar: f64| {
        let v = Voicing {
            polar,
            polar_ct: 0.3,
            polar_yz: 0.0,
            ..Voicing::beta14()
        };
        let ev = vec![(0, Ev::On(21, 0.7, 1))];
        let (l, _) = perform(&mut voiced(&v), &ev, 48_000 * 8, 512, 1);
        let early = level(&l, 4_800, 28_800) - level(&l, 48_000, 72_000);
        let late = level(&l, 4 * 48_000, 5 * 48_000) - level(&l, 7 * 48_000, 8 * 48_000);
        (early, late / 3.0)
    };
    let (e0, l0) = held(0.0);
    let (e1, l1) = held(0.5);
    eprintln!(
        "A0 decay (dB/s): alone early {e0:.2} late {l0:.2}; polarised early {e1:.2} late {l1:.2}"
    );
    // (The whole note's level: its upper partials, lost alike in both loops, weigh on it.)
    assert!(l1 < l0 - 0.2, "the aftersound: late {l0:.2} → {l1:.2} dB/s");
}

/// A three-string key whose last string the damper grips less: its release falls fast and
/// then lingers (the release's second stage), where every string gripped alike falls at once.
#[test]
fn a_loose_unison_string_gives_the_release_its_second_stage() {
    let tail = |loose: f64| {
        let v = Voicing {
            loose_last: loose,
            polar_damp: 0.25,
            ..Voicing::beta14()
        };
        let ev = vec![(0, Ev::On(60, 0.6, 1)), (48_000, Ev::Off(60, 1))];
        let (l, _) = perform(&mut voiced(&v), &ev, 48_000 * 3, 512, 1);
        (
            level(&l, 24_000, 48_000),
            level(&l, 48_000 + 24_000, 48_000 + 48_000),
        )
    };
    let (h0, t0) = tail(0.0);
    let (h1, t1) = tail(1.0);
    assert!((h0 - h1).abs() < 0.5, "the held note: {h0:.1} → {h1:.1} dB");
    assert!(
        t1 > t0 + 6.0,
        "0.5-1 s after the release: {t0:.1} → {t1:.1} dB"
    );
}

/// Round 40: the board's onset makes its response rise over milliseconds and leaves its
/// energy as voiced; at 0 the response is the old one, to the bit.
#[test]
fn the_boards_onset_rises_and_keeps_the_responses_energy() {
    use super::body::{responses, BodyVoicing};
    let v = BodyVoicing::beta14();
    let [l0, _] = responses(&v, 48_000.0);
    let [l1, _] = responses(&BodyVoicing { onset_ms: 0.0, ..v }, 48_000.0);
    assert_eq!(l0, l1);
    let [ls, _] = responses(&BodyVoicing { onset_ms: 3.0, ..v }, 48_000.0);
    let e = |x: &[f64], a: usize, b: usize| x[a..b].iter().map(|y| y * y).sum::<f64>();
    let total = |x: &[f64]| e(x, 0, x.len());
    assert!((total(&ls) - total(&l0)).abs() < 1e-9 * total(&l0));
    // The first millisecond holds far less of it.
    assert!(
        e(&ls, 0, 48) < 0.3 * e(&l0, 0, 48),
        "{} vs {}",
        e(&ls, 0, 48),
        e(&l0, 0, 48)
    );
}

/// Round 40: a heavier hammer stays longer on the strings — a mezzo-forte middle C's upper
/// partials fall against its first ones.
#[test]
fn a_heavier_hammer_darkens_a_mezzo_forte_strike() {
    let tilt = |mass: f64| {
        let v = Voicing {
            mass,
            ..Voicing::beta14()
        };
        let ev = vec![(0, Ev::On(60, 0.6, 1))];
        let (l, _) = perform(&mut voiced(&v), &ev, 24_000, 512, 1);
        // A first difference weighs the top: its energy against the plain signal's.
        let d: f64 = l
            .windows(2)
            .map(|w| ((w[1] - w[0]) as f64) * ((w[1] - w[0]) as f64))
            .sum();
        let e: f64 = l.iter().map(|x| (*x as f64) * (*x as f64)).sum();
        10.0 * math::log10(d / e)
    };
    let (light, heavy) = (tilt(1.0), tilt(2.0));
    assert!(
        heavy < light - 1.0,
        "the top against the whole: {light:.2} → {heavy:.2} dB"
    );
}

/// Round 40: a key's own partial voicing moves the partial it names and leaves the
/// fundamental where it was.
#[test]
fn a_keys_partial_voicing_moves_its_partial_alone() {
    use super::voice::PARTIALS;
    let at = |gain: f32| {
        let mut v = Voicing {
            partials_amount: 1.0,
            ..Voicing::beta14()
        };
        // Middle C's third partial (the table's second entry: partials 2 …) 12 dB down.
        v.partials[60 - 21] = [0.0; PARTIALS];
        v.partials[60 - 21][1] = gain;
        let ev = vec![(0, Ev::On(60, 0.6, 1))];
        let (l, _) = perform(&mut voiced(&v), &ev, 48_000, 512, 1);
        l
    };
    // The level at a frequency, by a single-bin correlation over 0.2-0.8 s.
    let bin = |x: &[f32], f: f64| {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, s) in x[9_600..38_400].iter().enumerate() {
            let w = std::f64::consts::TAU * f * i as f64 / 48_000.0;
            re += *s as f64 * math::cos(w);
            im += *s as f64 * math::sin(w);
        }
        10.0 * math::log10(re * re + im * im + 1e-30)
    };
    let (a, b) = (at(0.0), at(-12.0));
    let k = super::tables::keyboard(440.0, 1.0, 1.0)[60 - 21];
    let (f1, f3) = (k.partial(1), k.partial(3));
    assert!(
        (bin(&a, f1) - bin(&b, f1)).abs() < 1.0,
        "the fundamental moved"
    );
    assert!(
        bin(&b, f3) < bin(&a, f3) - 6.0,
        "the third partial: {:.1} → {:.1} dB",
        bin(&a, f3),
        bin(&b, f3)
    );
}

/// Round 40: the treble's coupling scale is the coupling itself at its default and grows
/// toward C8 off it.
#[test]
fn the_treble_coupling_is_the_coupling_at_its_default() {
    let v = Voicing {
        coupling: 1.2,
        ..Voicing::beta14()
    };
    for m in 21..=108u8 {
        assert_eq!(v.coupling_of(m), 1.2, "key {m}");
    }
    let w = Voicing {
        coupling_treble: 2.0,
        ..v
    };
    assert_eq!(w.coupling_of(60), 1.2);
    assert!((w.coupling_of(108) - 2.4).abs() < 1e-12);
}

/// Round 40: an undamped key's return thump sounds as it comes up, and only then.
#[test]
fn an_undamped_key_thumps_softly_as_it_comes_up() {
    let render = |key_return: f64| {
        let v = Voicing {
            key_return,
            ..Voicing::beta14()
        };
        // C7 has no damper: its strings ring on after the key comes up at 0.5 s.
        let ev = vec![(0, Ev::On(96, 0.6, 1)), (24_000, Ev::Off(96, 1))];
        perform(&mut voiced(&v), &ev, 36_000, 512, 1).0
    };
    let (a, b) = (render(0.0), render(0.5));
    assert!(
        a[..24_000] == b[..24_000],
        "it sounded before the key came up"
    );
    assert!(a[24_000..] != b[24_000..], "no thump");
}

/// Round 40: the output's top roll-off takes the air above its corner and nothing at 0.
#[test]
fn the_top_roll_off_takes_the_top_octave() {
    let top = |hz: f64| {
        let v = Voicing {
            top_hz: hz,
            ..Voicing::beta14()
        };
        let ev = vec![(0, Ev::On(100, 0.9, 1))];
        let (l, _) = perform(&mut voiced(&v), &ev, 24_000, 512, 1);
        // Its energy (the level of the whole: the roll-off takes it all above the corner).
        let e: f64 = l.iter().map(|x| (*x as f64) * (*x as f64)).sum();
        10.0 * math::log10(e)
    };
    // E7 (2.6 kHz) under a 1.5 kHz corner: its whole tone above it.
    assert!(
        top(1_500.0) < top(0.0) - 3.0,
        "{:.1} vs {:.1}",
        top(1_500.0),
        top(0.0)
    );
}

/// Round 40: a body's fine-grid spectrum is its responses' transform — against a direct
/// DFT, the direct path added as a complex sum, a flat response smoothed as itself.
#[test]
fn the_boards_spectrum_is_its_responses_transform() {
    use super::body::Spectrum;
    let fs = 48_000.0;
    let x: Vec<f64> = (0..300)
        .map(|i| {
            let t = i as f64 / fs;
            math::exp(-t * 400.0) * math::sin(std::f64::consts::TAU * 1_234.5 * t)
        })
        .collect();
    let sp = Spectrum::of(&[x.clone(), x.clone()], fs);
    let n = 1024.0;
    for k in [3usize, 10, 26, 27, 40, 90] {
        let f = k as f64 * fs / n;
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, v) in x.iter().enumerate() {
            let w = -std::f64::consts::TAU * k as f64 * i as f64 / n;
            re += v * math::cos(w);
            im += v * math::sin(w);
        }
        let (raw, _) = sp.power(f, 0.0, 1.0).expect("below the top");
        let want = re * re + im * im;
        assert!(
            (raw - want).abs() <= 1e-4 * want.max(1e-3),
            "{k}: {raw} against {want}"
        );
        let d = 0.3;
        let (with, _) = sp.power(f, d, 0.7).expect("below the top");
        let want = (d + 0.7 * re) * (d + 0.7 * re) + (0.7 * im) * (0.7 * im);
        assert!(
            (with - want).abs() <= 1e-4 * want.max(1e-3),
            "{k}: {with} against {want}"
        );
    }
    let mut impulse = vec![0.0; 1_000];
    impulse[0] = 1.0;
    let flat = Spectrum::of(&[impulse.clone(), impulse], fs);
    for f in [50.0, 440.0, 3_000.0, 5_900.0] {
        let (raw, smooth) = flat.power(f, 0.0, 1.0).expect("below the top");
        assert!(
            (raw - 1.0).abs() < 1e-6 && (smooth - 1.0).abs() < 1e-6,
            "{f}"
        );
    }
    assert!(flat.power(7_000.0, 0.0, 1.0).is_none());
}

/// The mezzo-forte level (dB, 30-500 ms) of each key of `keys` struck alone at `v`.
fn key_levels(v: &Voicing, keys: &[u8]) -> Vec<f64> {
    let gap = 24_000;
    let mut ev = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        ev.push((i * gap, Ev::On(*k, 0.6, i as u64 + 1)));
        ev.push((i * gap + 22_000, Ev::Off(*k, i as u64 + 1)));
    }
    let (l, r) = perform(&mut voiced(v), &ev, keys.len() * gap, 512, 1);
    (0..keys.len())
        .map(|i| {
            let (a, b) = (i * gap + 1_440, i * gap + 21_000);
            10.0 * math::log10(
                math::powf(10.0, level(&l, a, b) / 10.0) + math::powf(10.0, level(&r, a, b) / 10.0),
            )
        })
        .collect()
}

/// Each key's level against the mean of its two neighbours either side: the keyboard's
/// unevenness, root mean square (dB).
fn unevenness(x: &[f64]) -> f64 {
    let d: Vec<f64> = (0..x.len())
        .map(|i| {
            let nb: Vec<f64> = (i.saturating_sub(2)..(i + 3).min(x.len()))
                .filter(|j| *j != i)
                .map(|j| x[j])
                .collect();
            x[i] - nb.iter().sum::<f64>() / nb.len() as f64
        })
        .collect();
    (d.iter().map(|v| v * v).sum::<f64>() / d.len() as f64).sqrt()
}

/// Round 40: the board's modes make some keys loud and some weak; evened, the keyboard is
/// the smoother for it, and at `even` 0 a key's strike is what it was (to the bit: the
/// pinned hash above).
#[test]
fn the_evening_evens_the_keyboard() {
    let keys: Vec<u8> = (55..=79).collect();
    let off = key_levels(&Voicing::beta14(), &keys);
    let on = key_levels(
        &Voicing {
            even: 1.0,
            ..Voicing::beta14()
        },
        &keys,
    );
    let (a, b) = (unevenness(&off), unevenness(&on));
    eprintln!("unevenness {a:.2} → {b:.2} dB");
    assert!(a > 1.5, "the board makes the keyboard uneven: {a:.2} dB");
    assert!(b < 0.7 * a, "evened: {a:.2} → {b:.2} dB");
    // The keyboard's overall level stays (within a decibel).
    let mean = |x: &[f64]| x.iter().sum::<f64>() / x.len() as f64;
    assert!(
        (mean(&on) - mean(&off)).abs() < 1.0,
        "{:.2} → {:.2}",
        mean(&off),
        mean(&on)
    );
}

#[test]
#[ignore]
fn print_the_evening() {
    let c = voiced(&Voicing::beta14());
    let kb = super::tables::keyboard(440.0, 1.0, 1.0);
    for hz in [800.0, 4_000.0] {
        let v = Voicing {
            even: 1.0,
            even_hz: hz,
            ..Voicing::beta14()
        };
        let line: Vec<String> = (55..=79u8)
            .map(|m| {
                let k = &kb[(m - 21) as usize];
                let g = v.even_of(k, &[0.0; super::voice::PARTIALS], |f| c.body_power(f));
                format!("{:+.1}", 20.0 * math::log10(g))
            })
            .collect();
        eprintln!("{hz}: {}", line.join(" "));
    }
}

#[test]
#[ignore]
fn print_the_evening_against_the_levels() {
    let keys: Vec<u8> = (55..=79).collect();
    let v = Voicing::beta14();
    let lv = key_levels(&v, &keys);
    let c = voiced(&v);
    let kb = super::tables::keyboard(440.0, 1.0, 1.0);
    let e = Voicing { even: 1.0, ..v };
    let pred: Vec<f64> = keys
        .iter()
        .map(|m| {
            let k = &kb[(m - 21) as usize];
            -20.0 * math::log10(e.even_of(k, &[0.0; super::voice::PARTIALS], |f| c.body_power(f)))
        })
        .collect();
    let dev = |x: &[f64]| -> Vec<f64> {
        (0..x.len())
            .map(|i| {
                let nb: Vec<f64> = (i.saturating_sub(2)..(i + 3).min(x.len()))
                    .filter(|j| *j != i)
                    .map(|j| x[j])
                    .collect();
                x[i] - nb.iter().sum::<f64>() / nb.len() as f64
            })
            .collect()
    };
    let f = |x: Vec<f64>| {
        x.iter()
            .map(|v| format!("{v:+.1}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    eprintln!("measured  {}", f(dev(&lv)));
    eprintln!("predicted {}", f(pred));
    let nobody = key_levels(
        &Voicing {
            body: super::body::BodyVoicing {
                direct: 0.999,
                ..v.body
            },
            ..v
        },
        &keys,
    );
    eprintln!("direct    {}", f(dev(&nobody)));
}

/// Round 40: a key's own level moves that key alone, by what it says.
#[test]
fn a_keys_own_level_moves_that_key_alone() {
    let keys = [84u8, 87, 90];
    let mut v = Voicing {
        levels_amount: 1.0,
        ..Voicing::beta14()
    };
    let before = key_levels(&v, &keys);
    v.levels[87 - 21] = 6.0;
    let after = key_levels(&v, &keys);
    assert!(
        (after[0] - before[0]).abs() < 0.05,
        "{before:?} → {after:?}"
    );
    assert!(
        (after[2] - before[2]).abs() < 0.05,
        "{before:?} → {after:?}"
    );
    assert!(
        (after[1] - before[1] - 6.0).abs() < 0.3,
        "{before:?} → {after:?}"
    );
    assert_eq!(Voicing::beta14().level_of(87), 1.0);
}

/// Round 40: the bass coupling scale is the coupling itself at its default, leaves C4 and
/// above alone, and grows toward A0 off it.
#[test]
fn the_bass_coupling_leaves_the_middle_and_treble_alone() {
    let v = Voicing {
        coupling: 1.2,
        coupling_treble: 1.6,
        ..Voicing::beta14()
    };
    let b = Voicing {
        coupling_bass: 1.5,
        ..v
    };
    for m in 60..=108u8 {
        assert_eq!(
            b.coupling_of(m).to_bits(),
            v.coupling_of(m).to_bits(),
            "{m}"
        );
    }
    assert!((b.coupling_of(21) - 1.5 * v.coupling_of(21)).abs() < 1e-12);
    assert!(b.coupling_of(40) > v.coupling_of(40));
    assert_eq!(
        Voicing::beta14().coupling_of(30),
        Voicing::beta14().coupling
    );
}

/// Round 40: the bridge's resonances keep the loop passive — a loud held chord with strong
/// resonances decays, never grows — and move the sound only when asked.
#[test]
fn the_bridges_resonances_stay_passive() {
    let ev = vec![
        (0, Ev::On(45, 1.0, 1)),
        (0, Ev::On(57, 1.0, 2)),
        (0, Ev::On(69, 1.0, 3)),
    ];
    let render = |v: &Voicing| perform(&mut voiced(v), &ev, 3 * 48_000, 512, 1).0;
    let plain = render(&Voicing::beta14());
    let strong = render(&Voicing {
        bridge: [8.0, 40.0, 3.0, 12.0],
        ..Voicing::beta14()
    });
    assert!(strong.iter().all(|x| x.is_finite()));
    let (a, b, c) = (
        level(&strong, 4_800, 24_000),
        level(&strong, 72_000, 96_000),
        level(&strong, 120_000, 144_000),
    );
    assert!(b < a && c < b, "decays: {a:.1} {b:.1} {c:.1} dB");
    assert!(plain != strong, "the resonances move the sound");
}

/// Round 40: the bass register's high-pass takes A0's fundamental down and leaves its
/// fourth partial.
#[test]
fn the_bass_high_pass_takes_the_fundamental_and_leaves_the_partials() {
    let bin = |x: &[f32], f: f64| {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, s) in x[9_600..57_600].iter().enumerate() {
            let w = std::f64::consts::TAU * f * i as f64 / 48_000.0;
            re += *s as f64 * math::cos(w);
            im += *s as f64 * math::sin(w);
        }
        10.0 * math::log10(re * re + im * im + 1e-30)
    };
    let ev = vec![(0, Ev::On(21, 0.6, 1))];
    let at = |hz: f64| {
        let v = Voicing {
            bass_hp: hz,
            ..Voicing::beta14()
        };
        perform(&mut voiced(&v), &ev, 60_000, 512, 1).0
    };
    let (a, b) = (at(0.0), at(50.0));
    let k = super::tables::keyboard(440.0, 1.0, 1.0)[0];
    let (f1, f4) = (k.partial(1), k.partial(4));
    assert!(
        bin(&b, f1) < bin(&a, f1) - 6.0,
        "{:.1} → {:.1}",
        bin(&a, f1),
        bin(&b, f1)
    );
    assert!(
        (bin(&b, f4) - bin(&a, f4)).abs() < 2.0,
        "{:.1} → {:.1}",
        bin(&a, f4),
        bin(&b, f4)
    );
}

/// Round 40: the treble's lowest speed scale leaves C4 to the bit and lifts a soft C7.
#[test]
fn the_trebles_soft_speed_leaves_the_middle_alone() {
    let render =
        |m: u8, v: &Voicing| perform(&mut voiced(v), &[(0, Ev::On(m, 0.2, 1))], 24_000, 512, 1).0;
    let up = Voicing {
        vmin_treble: 1.5,
        ..Voicing::beta14()
    };
    assert_eq!(render(60, &up), render(60, &Voicing::beta14()));
    assert!(
        level(&render(96, &up), 0, 24_000)
            > level(&render(96, &Voicing::beta14()), 0, 24_000) + 1.0
    );
}

/// Round 40 (candidate 13): the soft felt moves a soft middle C, not a fortissimo one nor a
/// key two octaves away.
#[test]
fn the_soft_felt_leaves_the_fortissimo_and_the_far_keys_alone() {
    let render = |m: u8, vel: f32, v: &Voicing| {
        perform(&mut voiced(v), &[(0, Ev::On(m, vel, 1))], 24_000, 512, 1).0
    };
    let base = Voicing::beta14();
    let soft = Voicing {
        felt_soft: [1.6, 55.0, 79.0],
        ..base
    };
    assert_ne!(render(60, 0.5, &soft), render(60, 0.5, &base));
    assert_eq!(render(60, 0.95, &soft), render(60, 0.95, &base));
    assert_eq!(render(96, 0.5, &soft), render(96, 0.5, &base));
    assert_eq!(render(36, 0.5, &soft), render(36, 0.5, &base));
}

/// Round 40 (candidate 14): a key's own loss moves that key alone.
#[test]
fn a_keys_own_loss_moves_that_key_alone() {
    let render =
        |m: u8, v: &Voicing| perform(&mut voiced(v), &[(0, Ev::On(m, 0.6, 1))], 24_000, 512, 1).0;
    let base = Voicing::beta14();
    let mut lossy = base;
    assert!(lossy.set("hf_key.60", 0.5));
    assert!(!lossy.set("hf_key.60", 0.0));
    assert_ne!(render(60, &lossy), render(60, &base));
    for m in [59u8, 61, 72] {
        assert_eq!(render(m, &lossy), render(m, &base), "key {m}");
    }
}

/// Round 40 (candidate 13): a key's draw re-voices that key alone; at 0, its own.
#[test]
fn a_keys_draw_moves_that_key_alone() {
    let render =
        |m: u8, v: &Voicing| perform(&mut voiced(v), &[(0, Ev::On(m, 0.6, 1))], 24_000, 512, 1).0;
    let base = Voicing {
        uni_weight: 0.45,
        uni_rank: 1.0,
        uni_skew: 0.7,
        uni_b: 0.05,
        uni_b_hold: 1.0,
        ..Voicing::beta14()
    };
    let mut drawn = base;
    assert!(drawn.set("draw.39", 3.0));
    assert!(!drawn.set("draw.12", 3.0));
    assert_ne!(render(39, &drawn), render(39, &base));
    for m in [38u8, 40, 60] {
        assert_eq!(render(m, &drawn), render(m, &base), "key {m}");
    }
}

/// Round 40 (candidate 12): the middle's detune scale leaves the keys from its end alone
/// and moves a middle C.
#[test]
fn the_middles_detune_leaves_the_top_alone() {
    let render =
        |m: u8, v: &Voicing| perform(&mut voiced(v), &[(0, Ev::On(m, 0.6, 1))], 24_000, 512, 1).0;
    let base = Voicing {
        detune_scale: 1.48,
        ..Voicing::beta14()
    };
    let low = Voicing {
        detune_low: [0.8, 84.0],
        ..base
    };
    for m in [84u8, 96, 108] {
        assert_eq!(render(m, &low), render(m, &base), "key {m}");
    }
    assert_ne!(render(60, &low), render(60, &base));
}

/// Round 40 (candidate 11): the bass's soft speed leaves middle C and above alone and slows
/// a soft A1.
#[test]
fn the_bass_soft_speed_leaves_the_middle_alone() {
    let render =
        |m: u8, v: &Voicing| perform(&mut voiced(v), &[(0, Ev::On(m, 0.2, 1))], 24_000, 512, 1).0;
    let down = Voicing {
        vmin_bass: 0.6,
        ..Voicing::beta14()
    };
    assert_eq!(render(60, &down), render(60, &Voicing::beta14()));
    assert_eq!(render(72, &down), render(72, &Voicing::beta14()));
    assert!(
        level(&render(33, &down), 0, 24_000)
            < level(&render(33, &Voicing::beta14()), 0, 24_000) - 1.0
    );
}

/// Round 40: each unison string's own stiffness and losses move the sound only when asked,
/// and a unison so scattered still decays.
#[test]
fn a_unisons_own_strings_move_the_sound_only_when_asked() {
    let ev = vec![(0, Ev::On(57, 0.7, 1)), (0, Ev::On(33, 0.7, 2))];
    let render = |v: &Voicing| perform(&mut voiced(v), &ev, 2 * 48_000, 512, 1).0;
    let plain = render(&Voicing::beta14());
    for v in [
        Voicing {
            uni_b: 0.05,
            ..Voicing::beta14()
        },
        Voicing {
            uni_loss: 0.8,
            ..Voicing::beta14()
        },
    ] {
        let x = render(&v);
        assert!(x != plain);
        assert!(level(&x, 72_000, 96_000) < level(&x, 4_800, 24_000));
    }
    assert_eq!(Voicing::bass_scale(30, 1.0), 1.0);
    assert!((Voicing::bass_scale(21, 4.0) - 4.0).abs() < 1e-12);
    assert_eq!(Voicing::bass_scale(60, 4.0), 1.0);
    let f = Voicing {
        felt_upper: 0.5,
        ..Voicing::beta14()
    };
    assert_eq!(f.felt_of(60), Voicing::beta14().felt_of(60));
    assert!((f.felt_of(78) - 0.5 * Voicing::beta14().felt_of(78)).abs() < 1e-9);
}

/// Round 40: the strings' rise — a struck middle C's tone builds up over its time constant
/// (its first two milliseconds far under the plain strike's, its sound from 200 ms on — the
/// board's tail holding a little of the strike's first milliseconds — within a dB of the
/// plain one's), the lowest keys untouched, nothing at the default.
#[test]
fn the_strings_rise_slows_the_attack_and_leaves_the_tone() {
    let v0 = Voicing::beta14();
    for m in 21..=108u8 {
        assert_eq!(v0.rise_of(m), 0.0);
    }
    let quiet = Voicing {
        knock: 0.0,
        blow: 0.0,
        ..Voicing::beta14()
    };
    let up = Voicing {
        rise: [5.0, 6.0, 5.0, 3.0, 4.0],
        ..quiet
    };
    assert_eq!(up.rise_of(30), 0.0);
    assert_eq!(up.rise_of(60), 6.0);
    assert_eq!(up.rise_of(66), 5.5);
    assert_eq!(up.rise_of(100), 4.0);
    let render =
        |m: u8, v: &Voicing| perform(&mut voiced(v), &[(0, Ev::On(m, 0.6, 1))], 24_000, 512, 1).0;
    let (a, b) = (render(60, &quiet), render(60, &up));
    let first = level(&b, 0, 144) - level(&a, 0, 144);
    let after = level(&b, 9_600, 24_000) - level(&a, 9_600, 24_000);
    assert!(first < -6.0, "the first 3 ms: {first:.1} dB");
    assert!(after.abs() < 1.0, "from 200 ms: {after:.2} dB");
    assert_eq!(render(28, &quiet), render(28, &up), "A0's strike moved");
}

/// Round 40: the top keys' release — a released C7 (no damper) falls faster with it, the
/// pedal down holds it as before, and a damped key is untouched.
#[test]
fn the_top_keys_release_fades_an_undamped_key_unless_the_pedal_holds_it() {
    let damp = Voicing {
        top_damper: 0.15,
        ..Voicing::beta14()
    };
    let render = |m: u8, v: &Voicing, pedal: f32| {
        let ev = vec![
            (0, Ev::Pedal(pedal)),
            (10, Ev::On(m, 0.6, 1)),
            (12_000, Ev::Off(m, 1)),
        ];
        perform(&mut voiced(v), &ev, 72_000, 512, 1).0
    };
    let (a, b) = (render(96, &Voicing::beta14(), 0.0), render(96, &damp, 0.0));
    assert!(a[..12_000] == b[..12_000], "it moved before the release");
    let fall = level(&b, 48_000, 72_000) - level(&a, 48_000, 72_000);
    assert!(fall < -10.0, "a second after the release: {fall:.1} dB");
    assert_eq!(render(96, &Voicing::beta14(), 1.0), render(96, &damp, 1.0));
    assert_eq!(render(60, &Voicing::beta14(), 0.0), render(60, &damp, 0.0));
}

/// Round 40: the fourth-order bass high-pass takes A0's fundamental further than the second
/// order and still leaves its fourth partial.
#[test]
fn the_fourth_order_bass_high_pass_is_steeper() {
    let bin = |x: &[f32], f: f64| {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, s) in x[9_600..57_600].iter().enumerate() {
            let w = std::f64::consts::TAU * f * i as f64 / 48_000.0;
            re += *s as f64 * math::cos(w);
            im += *s as f64 * math::sin(w);
        }
        10.0 * math::log10(re * re + im * im + 1e-30)
    };
    let ev = vec![(0, Ev::On(21, 0.6, 1))];
    let at = |order: f64| {
        let v = Voicing {
            bass_hp: 40.0,
            bass_hp_order: order,
            ..Voicing::beta14()
        };
        perform(&mut voiced(&v), &ev, 60_000, 512, 1).0
    };
    let (a, b) = (at(2.0), at(4.0));
    let k = super::tables::keyboard(440.0, 1.0, 1.0)[0];
    let (f1, f4) = (k.partial(1), k.partial(4));
    assert!(
        bin(&b, f1) < bin(&a, f1) - 4.0,
        "{:.1} → {:.1}",
        bin(&a, f1),
        bin(&b, f1)
    );
    assert!((bin(&b, f4) - bin(&a, f4)).abs() < 2.0);
    // Every path the strings reach the output by takes it — the sympathetic strings' bank
    // passes its drive, the unvoiced strings, through as well (round 40: A0's fundamental
    // went around the bass register's high-pass that way).
    let off = perform(&mut voiced(&Voicing::beta14()), &ev, 60_000, 512, 1).0;
    assert!(
        bin(&b, f1) < bin(&off, f1) - 15.0,
        "{:.1} → {:.1}",
        bin(&off, f1),
        bin(&b, f1)
    );
}

/// Round 40: the damper landing's rise takes its click away — the first millisecond after a
/// key comes up quieter, the breath after it still there.
#[test]
fn the_landings_rise_takes_the_click_away() {
    let render = |landing_rise: f64| {
        let v = Voicing {
            landing_shape: [0.12, 700.0, 0.0, 0.0],
            landing: 4.0,
            landing_rise,
            ..Voicing::beta14()
        };
        let ev = vec![(0, Ev::On(60, 0.6, 1)), (24_000, Ev::Off(60, 1))];
        perform(&mut voiced(&v), &ev, 36_000, 512, 1).0
    };
    let (a, b) = (render(0.0), render(0.3));
    assert!(a[..24_000] == b[..24_000]);
    let d: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x - y).collect();
    assert!(
        d[24_000..].iter().any(|x| *x != 0.0),
        "the rise did nothing"
    );
    let mut t = super::noise::Thud::new(48_000.0, [95.0, 240.0], [0.05, 0.035], 200.0, 700.0, 0.12);
    t.set_rise(48_000.0, 0.12, 0.3);
    assert!(t.rises());
    t.set_rise(48_000.0, 0.12, 0.0);
    assert!(!t.rises());
}

/// Round 40: a unison's stiffness scatter held at its fundamentals — a middle C's first
/// partial stays where the plain unison has it (within a fraction of a cent) while the
/// unheld scatter moves it further.
#[test]
fn the_held_stiffness_scatter_leaves_the_fundamental() {
    let k = super::tables::keyboard(440.0, 1.0, 1.0)[60 - 21];
    let f1 = k.partial(1);
    let peak = |v: &Voicing| {
        let x = perform(&mut voiced(v), &[(0, Ev::On(60, 0.6, 1))], 96_000, 512, 1).0;
        let mut best = (0.0, f64::MIN);
        for i in -100..=100 {
            let f = f1 * math::powf(2.0f64, i as f64 * 0.1 / 1200.0);
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (n, s) in x[24_000..96_000].iter().enumerate() {
                let w = std::f64::consts::TAU * f * n as f64 / 48_000.0;
                re += *s as f64 * math::cos(w);
                im += *s as f64 * math::sin(w);
            }
            if re * re + im * im > best.1 {
                best = (i as f64 * 0.1, re * re + im * im);
            }
        }
        best.0
    };
    let plain = peak(&Voicing::beta14());
    let scatter = |hold: f64| Voicing {
        uni_b: 0.08,
        uni_b_hold: hold,
        ..Voicing::beta14()
    };
    let (held, loose) = (peak(&scatter(1.0)), peak(&scatter(0.0)));
    eprintln!("first partial (cents): plain {plain:.2}, held {held:.2}, unheld {loose:.2}");
    assert!(
        (held - plain).abs() < 0.3,
        "held {held:.2} against {plain:.2}"
    );
    assert!((loose - plain).abs() > (held - plain).abs());
}

/// Round 40: the soft strikes' trim — nothing from velocity 75 up, its anchor at 30, none
/// at the default.
#[test]
fn the_soft_trim_leaves_the_mezzo_forte() {
    assert_eq!(Voicing::beta14().pp_gain(60, 0.1), 1.0);
    let t = Voicing {
        pp_trim: [2.0, 4.0, 0.0, -3.0],
        ..Voicing::beta14()
    };
    assert_eq!(t.pp_gain(60, 75.0 / 127.0), 1.0);
    assert_eq!(t.pp_gain(60, 0.9), 1.0);
    assert!((t.pp_gain(60, 30.0 / 127.0) - math::powf(10.0f64, -4.0 / 20.0)).abs() < 1e-12);
    assert!(t.pp_gain(108, 30.0 / 127.0) > 1.0);
    let render = |v: &Voicing, vel: f32| {
        perform(&mut voiced(v), &[(0, Ev::On(60, vel, 1))], 24_000, 512, 1).0
    };
    assert_eq!(render(&t, 0.8), render(&Voicing::beta14(), 0.8));
    assert!(
        level(&render(&t, 0.2), 0, 24_000)
            < level(&render(&Voicing::beta14(), 0.2), 0, 24_000) - 2.0
    );
}

/// The arrival-time difference's curve (`itd_shape`): at 0 the straight line to the sample;
/// shaped, it keeps its ends, falls key by key and is steeper in the middle.
#[test]
fn the_lag_curve_keeps_its_ends_and_is_the_line_at_zero() {
    let fs = 48_000.0;
    let line = Voicing {
        itd_ms: 0.6,
        ..Voicing::beta14()
    };
    for m in 21..=108u8 {
        let x = (m as f64 - 64.5) / 43.5;
        assert_eq!(
            line.lag_of(m, fs),
            (-0.6 * x * fs / 1000.0).round() as i64,
            "key {m}"
        );
    }
    let curve = Voicing {
        itd_shape: 0.8,
        ..line
    };
    assert_eq!(curve.lag_of(21, fs), line.lag_of(21, fs));
    assert_eq!(curve.lag_of(108, fs), line.lag_of(108, fs));
    for m in 22..=108u8 {
        assert!(curve.lag_of(m, fs) <= curve.lag_of(m - 1, fs), "key {m}");
    }
    assert!(curve.lag_of(76, fs) < line.lag_of(76, fs));
    assert!(curve.lag_of(53, fs) > line.lag_of(53, fs));
}

/// Round 40 (candidate 7): the top's unison detune and velocity curve, the bass's extra
/// unevenness and the ends' extra lag — nothing at their defaults, and each moves its own
/// register alone: a middle C to the bit, the keys they are for audibly.
#[test]
fn the_top_and_bass_laws_move_their_registers_alone() {
    let d = Voicing::beta14();
    for m in 21..=108u8 {
        assert_eq!(Voicing::top_scale(m, d.detune_top[0], d.detune_top[1]), 1.0);
        assert_eq!(d.weight_bass(m), 0.0);
    }
    assert_eq!(Voicing::top_scale(84, 84.0, 0.2), 1.0);
    assert!((Voicing::top_scale(108, 84.0, 0.2) - 0.2).abs() < 1e-12);
    let base = Voicing {
        uni_weight: 0.3,
        uni_rank: 1.0,
        ..Voicing::beta14()
    };
    let render = |m: u8, vel: f32, v: &Voicing| {
        perform(&mut voiced(v), &[(0, Ev::On(m, vel, 1))], 48_000, 512, 1).0
    };
    let moved = [
        (
            Voicing {
                detune_top: [84.0, 0.2],
                ..base
            },
            105,
        ),
        (
            Voicing {
                vel_top: [84.0, 0.7, 1.0],
                ..base
            },
            105,
        ),
        (
            Voicing {
                vel_top: [84.0, 1.0, 0.7],
                ..base
            },
            105,
        ),
        (
            Voicing {
                uni_weight_bass: 0.4,
                ..base
            },
            33,
        ),
        (
            Voicing {
                hf_top: [84.0, 2.0],
                ..base
            },
            105,
        ),
    ];
    for (v, m) in &moved {
        assert_eq!(render(60, 0.6, v), render(60, 0.6, &base), "middle C moved");
        assert!(
            render(*m, 0.6, v) != render(*m, 0.6, &base),
            "key {m} did not move"
        );
    }
    // The narrower range lifts a mezzo-forte top key and leaves the hardest strike.
    let narrow = &moved[1].0;
    let lift =
        level(&render(105, 0.59, narrow), 0, 48_000) - level(&render(105, 0.59, &base), 0, 48_000);
    assert!(lift > 1.0, "a mezzo-forte A7 rose {lift:.2} dB");
    let top =
        level(&render(105, 1.0, narrow), 0, 48_000) - level(&render(105, 1.0, &base), 0, 48_000);
    assert!(top.abs() < 0.05, "the hardest A7 moved {top:.3} dB");
    // The ends' extra lag: the line at its ends wider, the middle as it was.
    let fs = 48_000.0;
    let line = Voicing {
        itd_ms: 0.6,
        ..Voicing::beta14()
    };
    let ends = Voicing {
        itd_ends: 0.3,
        ..line
    };
    assert_eq!(ends.lag_of(64, fs), line.lag_of(64, fs));
    assert!(ends.lag_of(21, fs) > line.lag_of(21, fs));
    assert!(ends.lag_of(108, fs) < line.lag_of(108, fs));
    for m in 22..=108u8 {
        assert!(ends.lag_of(m, fs) <= ends.lag_of(m - 1, fs), "key {m}");
    }
}

/// Round 40 (candidate 9): no key leaves its pitch for its unison's stiffness scatter — every
/// key from A0 to C8 struck with candidate 7's scatter (the bass's six times the middle's)
/// sounds its first partial within 3 cents (6 under C2, where a window holds fewer periods)
/// of the same key struck without it. Candidate 7's clamp left D3 46 cents flat.
#[test]
fn no_key_is_detuned_by_its_unisons_stiffness_scatter() {
    let plain = Voicing {
        uni_weight: 0.45,
        uni_rank: 1.0,
        detune_scale: 1.48,
        ..Voicing::beta14()
    };
    let scattered = Voicing {
        uni_b: 0.054,
        uni_b_bass: 6.0,
        uni_b_hold: 1.0,
        ..plain
    };
    let fs = 48_000.0;
    let render = |m: u8, v: &Voicing| {
        perform(&mut voiced(v), &[(0, Ev::On(m, 0.6, 1))], 44_000, 512, 1)
            .0
            .iter()
            .map(|x| *x as f64)
            .collect::<Vec<f64>>()
    };
    let mut worst = (0u8, 0.0f64);
    for m in 21..=108u8 {
        let near = 440.0 * math::powf(2.0f64, (m as f64 - 69.0) / 12.0);
        let hz =
            |x: &[f64]| super::testkit::partial_hz(x, fs, near * 1.01, near * 0.04, 4_800, 32_768);
        let (a, b) = (hz(&render(m, &plain)), hz(&render(m, &scattered)));
        let ct = 1200.0 * math::log2(b / a);
        if ct.abs() > worst.1.abs() {
            worst = (m, ct);
        }
        let limit = if m < 36 { 6.0 } else { 3.0 };
        assert!(ct.abs() < limit, "key {m}: {ct:+.1} cents off its pitch");
    }
    eprintln!(
        "the scatter's worst key: {} at {:+.2} cents",
        worst.0, worst.1
    );
}

/// Round 40 (candidate 10): the key's own damper — a released middle C (the pedal up) rings
/// on longer with a slower key damper; a note released under the pedal and damped when the
/// pedal comes up is the pedal's, untouched; nothing at the default.
#[test]
fn the_keys_damper_is_apart_from_the_pedals() {
    let base = Voicing::beta14();
    let slow = Voicing {
        damper_key: base.damper_t60 * 3.0,
        ..base
    };
    let key_up = vec![(0, Ev::On(60, 0.6, 1)), (12_000, Ev::Off(60, 1))];
    let pedal_up = vec![
        (0, Ev::Pedal(1.0)),
        (10, Ev::On(60, 0.6, 1)),
        (6_000, Ev::Off(60, 1)),
        (12_000, Ev::Pedal(0.0)),
    ];
    let render = |v: &Voicing, ev: &[(usize, Ev)]| perform(&mut voiced(v), ev, 48_000, 512, 1).0;
    assert_eq!(
        render(
            &Voicing {
                damper_key: 0.0,
                ..base
            },
            &key_up
        ),
        render(&base, &key_up)
    );
    assert!(
        level(&render(&slow, &key_up), 24_000, 36_000)
            > level(&render(&base, &key_up), 24_000, 36_000) + 3.0
    );
    assert_eq!(render(&slow, &pedal_up), render(&base, &pedal_up));
}

/// `DSP_VERSION` 7: the shipped voicing keeps every block size over a long random phrase —
/// notes, releases, the pedal — where voices end inside a piece. (With the arrival-time
/// difference on, a later side's last samples drained in a long piece and were dropped in a
/// short one: blocks of 1 and 7 differed from 512 at −115 dBFS.)
#[test]
fn the_shipped_voicing_keeps_every_block_size_over_a_long_phrase() {
    let mut ev = Vec::new();
    let mut seed = 12345u64;
    let mut rnd = |n: u64| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) % n
    };
    let mut held: Vec<(u8, u64)> = Vec::new();
    let mut id = 1u64;
    let mut f = 0usize;
    while f < 140_000 {
        let roll = rnd(100);
        if roll < 45 && held.len() < 16 {
            let k = 24 + rnd(72) as u8;
            if !held.iter().any(|h| h.0 == k) {
                ev.push((f, Ev::On(k, (1 + rnd(126)) as f32 / 127.0, id)));
                held.push((k, id));
                id += 1;
            }
        } else if roll < 75 && !held.is_empty() {
            let (k, i) = held.remove(rnd(held.len() as u64) as usize);
            ev.push((f, Ev::Off(k, i)));
        } else if roll < 85 {
            ev.push((f, Ev::Pedal(if rnd(2) == 0 { 1.0 } else { 0.0 })));
        }
        f += 1 + rnd(4_000) as usize;
    }
    for (k, i) in held {
        ev.push((f, Ev::Off(k, i)));
    }
    ev.push((f, Ev::Pedal(0.0)));
    let frames = 192_000;
    let reference = perform(&mut shipped_piano(), &ev, frames, 512, 1);
    assert!(reference.0.iter().any(|x| *x != 0.0));
    for block in [1usize, 7, 64] {
        let got = perform(&mut shipped_piano(), &ev, frames, block, 1);
        assert!(got == reference, "blocks of {block}");
    }
}
