//! Nitor's contract: the null to the bit, block-size independence at tolerance 0, exact
//! rest, and what each part is for.

use super::testkit::*;
use super::*;
use crate::math;

const BLOCKS: [&[usize]; 5] = [&[1], &[37], &[64], &[512], &[17, 1024, 3, 256, 333]];

fn every_character_at_zero() -> Vec<NitorSettings> {
    let mut v = vec![NitorSettings::default()];
    for c in Character::ALL {
        for (m, mix) in [(true, 1.0), (false, 0.5)] {
            v.push(NitorSettings {
                character: c,
                level_match: m,
                mix,
                sub_mode: SubMode::Tonic,
                tonic: 9,
                xover_hz: 180.0,
                color: 0.9,
                ..NitorSettings::default()
            });
        }
    }
    v
}

#[test]
fn every_default_and_every_character_at_amount_zero_is_a_wire_to_the_bit() {
    let mut input = programme(2.0, 1.0, 3);
    // −0.0 and the smallest numbers survive too.
    input.0[100] = -0.0;
    input.1[101] = f32::MIN_POSITIVE;
    input.0[102] = 1e-42;
    for s in every_character_at_zero() {
        for blocks in BLOCKS {
            let mut u = Nitor::new(SR);
            u.set(&s);
            let out = render(&mut u, &input, blocks, &[]);
            assert_eq!(first_difference(&out, &input), None, "{s:?} {blocks:?}");
            assert!(u.at_rest(), "{:?}", u.rest_blocker());
        }
    }
}

#[test]
fn a_part_turned_back_to_zero_is_a_wire_again_to_the_bit() {
    let input = programme(3.0, 1.0, 5);
    let mut u = Nitor::new(SR);
    u.set(&NitorSettings::default());
    let changes = [(0, heavy()), (48_000, NitorSettings::default())];
    let out = render(&mut u, &input, &[256], &changes);
    // Within a glide and the match's way back (0.1 s), it is the input again.
    let from = 48_000 + 9_600;
    for i in from..input.0.len() {
        assert_eq!(out.0[i].to_bits(), input.0[i].to_bits(), "at {i}");
        assert_eq!(out.1[i].to_bits(), input.1[i].to_bits(), "at {i}");
    }
}

#[test]
fn heavy_settings_do_not_depend_on_the_block_size_at_tolerance_zero() {
    let input = programme(3.0, 1.0, 7);
    let mut light = heavy();
    light.character = Character::Air;
    light.amount = 0.4;
    light.sub_mode = SubMode::Tonic;
    light.xover_hz = 70.0;
    light.excite = [0.0, 0.3, 0.0];
    // Changes at samples every block pattern reaches by its own cuts.
    let changes = [
        (0, heavy()),
        (40_000, light),
        (70_001, NitorSettings::default()),
        (90_000, heavy()),
    ];
    let mut first = None;
    for blocks in BLOCKS {
        let mut u = Nitor::new(SR);
        let out = render(&mut u, &input, blocks, &changes);
        assert!(out.0.iter().chain(&out.1).all(|v| v.is_finite()));
        match &first {
            None => first = Some(out),
            Some(f) => assert_eq!(first_difference(f, &out), None, "{blocks:?}"),
        }
    }
}

#[test]
fn offline_equals_real_time_a_fork_resumes_to_the_bit() {
    let input = programme(2.0, 1.0, 11);
    let mut a = Nitor::new(SR);
    a.set(&heavy());
    let whole = render(&mut a, &input, &[480], &[]);
    let mut b = Nitor::new(SR);
    b.set(&heavy());
    let half = 48_000;
    let head = (input.0[..half].to_vec(), input.1[..half].to_vec());
    let _ = render(&mut b, &head, &[100], &[]);
    let mut c = b.clone();
    let tail = (input.0[half..].to_vec(), input.1[half..].to_vec());
    let out = render(&mut c, &tail, &[77], &[]);
    assert_eq!(out.0, whole.0[half..].to_vec());
    assert_eq!(out.1, whole.1[half..].to_vec());
}

#[test]
fn heavy_settings_come_to_rest_in_silence_and_resting_is_exact() {
    let sound = programme(1.5, 1.0, 13);
    let silence = 20 * SR as usize;
    let mut input = (sound.0.clone(), sound.1.clone());
    input.0.resize(sound.0.len() + silence, 0.0);
    input.1.resize(sound.1.len() + silence, 0.0);
    let mut u = Nitor::new(SR);
    u.set(&heavy());
    let out = render(&mut u, &input, &[512], &[]);
    assert!(u.at_rest(), "{:?}", u.rest_blocker());
    assert!(out.0[out.0.len() - 4800..].iter().all(|v| v.to_bits() == 0));
    // Skipping now, against processing silence, then sound: the same, and the same as a
    // new unit (skipped as far).
    let after = programme(1.0, 0.7, 17);
    let mut skipped = u.clone();
    skipped.skip(12_345);
    let mut processed = u.clone();
    let quiet = (vec![0.0f32; 12_345], vec![0.0f32; 12_345]);
    let q = render(&mut processed, &quiet, &[512], &[]);
    assert!(q.0.iter().chain(&q.1).all(|v| v.to_bits() == 0));
    assert!(processed.at_rest());
    let a = render(&mut skipped, &after, &[256], &[]);
    let b = render(&mut processed, &after, &[256], &[]);
    assert_eq!(first_difference(&a, &b), None);
    let mut fresh = Nitor::new(SR);
    fresh.set(&heavy());
    fresh.skip((u.clock() + 12_345) as usize);
    let c = render(&mut fresh, &after, &[256], &[]);
    assert_eq!(first_difference(&a, &c), None);
}

#[test]
fn silence_in_is_exact_silence_out_and_a_new_unit_rests() {
    for s in [NitorSettings::default(), heavy()] {
        let mut u = Nitor::new(SR);
        u.set(&s);
        assert!(u.at_rest(), "{:?}", u.rest_blocker());
        let z = (vec![0.0f32; 9_000], vec![0.0f32; 9_000]);
        let out = render(&mut u, &z, &[333], &[]);
        assert!(out.0.iter().chain(&out.1).all(|v| v.to_bits() == 0));
        assert!(u.at_rest(), "{:?}", u.rest_blocker());
        u.reset();
        assert!(u.at_rest(), "after a reset: {:?}", u.rest_blocker());
        let mut v = Nitor::new(SR);
        v.set(&s);
        v.reset();
        assert!(v.at_rest(), "set, then reset: {:?}", v.rest_blocker());
    }
}

#[test]
fn the_output_gain_alone_is_a_plain_gain() {
    let input = programme(0.5, 1.0, 19);
    let mut u = Nitor::new(SR);
    u.set(&NitorSettings {
        out_db: -6.0,
        ..NitorSettings::default()
    });
    let out = render(&mut u, &input, &[64], &[]);
    let g = math::db_to_gain(-6.0);
    for i in 0..input.0.len() {
        assert_eq!(out.0[i], input.0[i] * g);
    }
}

#[test]
fn the_tonic_sub_sounds_on_the_song_s_tonic_and_moves_with_it() {
    let bass = sine(98.0, 0.3, 96_000);
    let input = (bass.clone(), bass);
    for (tonic, hz) in [(7u8, 49.0f32), (2, 36.71)] {
        let mut u = Nitor::new(SR);
        u.set(&NitorSettings {
            sub: 1.0,
            sub_mode: SubMode::Tonic,
            tonic,
            level_match: false,
            ..NitorSettings::default()
        });
        let out = render(&mut u, &input, &[128], &[]);
        let sub: Vec<f32> = out.0.iter().zip(&input.0).map(|(o, i)| o - i).collect();
        let at = tone(&sub, hz, 24_000);
        let other = tone(&sub, if tonic == 7 { 36.71 } else { 49.0 }, 24_000);
        assert!(db(at) > -16.0, "tonic {tonic}: {} dB at {hz}", db(at));
        assert!(db(at) - db(other) > 30.0, "{} vs {}", db(at), db(other));
        assert!((u.meter().sub_hz - hz).abs() < 0.05, "{}", u.meter().sub_hz);
    }
}

#[test]
fn the_octave_sub_is_an_octave_under_the_bass() {
    let bass = sine(110.0, 0.3, 96_000);
    let input = (bass.clone(), bass);
    let mut u = Nitor::new(SR);
    u.set(&NitorSettings {
        sub: 1.0,
        level_match: false,
        ..NitorSettings::default()
    });
    let out = render(&mut u, &input, &[128], &[]);
    let sub: Vec<f32> = out.0.iter().zip(&input.0).map(|(o, i)| o - i).collect();
    assert!(db(tone(&sub, 55.0, 24_000)) > -16.0);
    assert!(db(tone(&sub, 55.0, 24_000)) - db(tone(&sub, 49.0, 24_000)) > 20.0);
}

#[test]
fn the_exciter_adds_harmonics_even_or_odd_by_its_colour() {
    let x = sine(2_000.0, 0.25, 48_000);
    let input = (x.clone(), x);
    let run = |color: f32| {
        let mut u = Nitor::new(SR);
        u.set(&NitorSettings {
            excite: [0.0, 1.0, 0.0],
            color,
            level_match: false,
            ..NitorSettings::default()
        });
        render(&mut u, &input, &[64], &[])
    };
    let even = run(0.0);
    let odd = run(1.0);
    let h = |o: &(Vec<f32>, Vec<f32>), hz| db(tone(&o.0, hz, 12_000));
    // The fundamental is kept; the colour picks the harmonics.
    assert!((h(&even, 2_000.0) - db(0.25)).abs() < 0.5);
    assert!(h(&even, 4_000.0) > -40.0 && h(&even, 4_000.0) - h(&even, 6_000.0) > 20.0);
    assert!(h(&odd, 6_000.0) > -40.0 && h(&odd, 6_000.0) - h(&odd, 4_000.0) > 20.0);
}

#[test]
fn the_exciter_adds_the_same_share_at_any_level() {
    let share = |amp: f32| {
        let x = sine(2_000.0, amp, 48_000);
        let input = (x.clone(), x);
        let mut u = Nitor::new(SR);
        u.set(&NitorSettings {
            excite: [0.0, 1.0, 0.0],
            color: 0.0,
            level_match: false,
            ..NitorSettings::default()
        });
        let o = render(&mut u, &input, &[64], &[]);
        db(tone(&o.0, 4_000.0, 12_000)) - db(f64::from(amp))
    };
    let loud = share(0.5);
    let quiet = share(0.005);
    assert!((loud - quiet).abs() < 0.5, "{loud} vs {quiet}");
}

#[test]
fn lift_raises_the_tonic_and_its_octave() {
    for (hz, want) in [(49.0f32, low::LIFT_DB), (98.0, 0.5 * low::LIFT_DB), (300.0, 0.0)] {
        let x = sine(hz, 0.2, 72_000);
        let input = (x.clone(), x);
        let mut u = Nitor::new(SR);
        u.set(&NitorSettings {
            lift: 1.0,
            tonic: 7,
            level_match: false,
            ..NitorSettings::default()
        });
        let o = render(&mut u, &input, &[64], &[]);
        let g = db(tone(&o.0, hz, 24_000)) - db(0.2);
        assert!((g - f64::from(want)).abs() < 1.0, "{hz} Hz: {g} dB, want {want}");
    }
}

#[test]
fn tight_shortens_the_low_end_s_decay_and_keeps_its_hit() {
    // A 50 Hz note struck every half second, decaying slowly.
    let n = 96_000;
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = (i % 24_000) as f32 / SR;
            0.4 * math::exp(-t * 2.0) * math::sin(core::f32::consts::TAU * 50.0 * t)
        })
        .collect();
    let input = (x.clone(), x);
    let mut u = Nitor::new(SR);
    u.set(&NitorSettings {
        tight: 1.0,
        level_match: false,
        ..NitorSettings::default()
    });
    let o = render(&mut u, &input, &[64], &[]);
    let hit = |s: &[f32]| rms(&s[48_000..48_000 + 1_200]);
    let tail = |s: &[f32]| rms(&s[48_000 + 12_000..48_000 + 23_000]);
    let h = db(hit(&o.0)) - db(hit(&input.0));
    let t = db(tail(&o.0)) - db(tail(&input.0));
    assert!(h > -1.5, "the hit kept: {h:.2} dB (the tail {t:.2})");
    assert!(t < -6.0, "the tail shortened: {t:.2} dB (the hit {h:.2})");
}

#[test]
fn the_level_match_holds_the_loudness_of_every_character() {
    let input = programme(8.0, 1.0, 23);
    let dry = loudness(&input, 3 * SR as usize);
    for c in Character::ALL {
        let mut u = Nitor::new(SR);
        u.set(&NitorSettings {
            amount: 1.0,
            character: c,
            ..NitorSettings::default()
        });
        let o = render(&mut u, &input, &[256], &[]);
        let wet = loudness(&o, 3 * SR as usize);
        assert!((wet - dry).abs() < 0.5, "{c:?}: {wet:.2} against {dry:.2} LUFS");
        assert!(o.0.iter().chain(&o.1).all(|v| v.is_finite()));
    }
}

#[test]
fn a_snap_on_a_new_unit_changes_nothing() {
    // A host snaps every unit at the start of a stream (the exported plug-in does).
    let input = programme(1.0, 1.0, 31);
    for s in [heavy(), NitorSettings { amount: 0.35, ..NitorSettings::default() }] {
        let mut a = Nitor::new(SR);
        a.set(&s);
        a.reset();
        let mut b = a.clone();
        b.snap();
        let x = render(&mut a, &input, &[256], &[]);
        let y = render(&mut b, &input, &[256], &[]);
        assert_eq!(first_difference(&x, &y), None);
    }
}
