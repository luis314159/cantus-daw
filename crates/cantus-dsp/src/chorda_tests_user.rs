//! Chorda's voicing by hand (round 42, `chorda-presets`): every control at its default is
//! nothing — the shipped voicing to the bit, through a unit too; each moved does something;
//! with all of them moved a unit keeps every block size and thread count, plays after idling
//! as a new one, and comes to rest; moved and brought back, the bits return.

use super::tests_engine::{perform, Ev};
use super::tests_model2::piano2;
use super::user::UserVoicing;
use super::voicing::Voicing;
use super::Chorda;
use super::ChordaSettings;

/// Model 2 with the player's controls `u`.
fn unit(u: &UserVoicing) -> Chorda {
    let mut c = piano2();
    let s = ChordaSettings {
        model: 2,
        user: *u,
        ..ChordaSettings::default()
    };
    c.set(&s, 0.0);
    c
}

/// Every register struck, the pedal down and up, keys released (the dampers, the floor and
/// the key's return are heard).
fn phrase() -> Vec<(usize, Ev)> {
    let mut ev = vec![
        (0, Ev::On(28, 0.8, 1)),
        (200, Ev::On(60, 0.6, 2)),
        (400, Ev::On(96, 0.7, 3)),
        (3_000, Ev::Pedal(0.9)),
        (3_200, Ev::On(45, 0.5, 4)),
        (6_000, Ev::Off(60, 2)),
        (7_000, Ev::Pedal(0.0)),
        (9_000, Ev::Off(28, 1)),
        (9_000, Ev::Off(96, 3)),
        (12_000, Ev::Off(45, 4)),
    ];
    ev.sort_by_key(|e| e.0);
    ev
}

const FRAMES: usize = 19_200;

/// Every control away from its default (each inside its range).
fn moved() -> UserVoicing {
    UserVoicing {
        felt_bass: 0.4,
        felt_treble: -0.3,
        mass: 1.3,
        decay_bass: 0.7,
        decay_mid: 1.4,
        decay_treble: 0.6,
        hf: 1.8,
        hf_spread: 0.5,
        uneven: 1.7,
        polar: 2.0,
        formant_hz: Some(420.0),
        formant_db: Some(6.0),
        formant_q: Some(1.5),
        knock: 2.0,
        knock_tone: -0.8,
        key_return: 1.8,
        damper: 0.6,
        damper_bass: 1.5,
        damped_db: Some(50.0),
        itd: 2.0,
        top_hz: Some(6_000.0),
        amount: 0.3,
    }
}

/// One control of [`moved`] alone, by its index (0 … [`CONTROLS`]).
fn one(i: usize) -> UserVoicing {
    let m = moved();
    let mut u = UserVoicing::NEUTRAL;
    match i {
        0 => u.felt_bass = m.felt_bass,
        1 => u.felt_treble = m.felt_treble,
        2 => u.mass = m.mass,
        3 => u.decay_bass = m.decay_bass,
        4 => u.decay_mid = m.decay_mid,
        5 => u.decay_treble = m.decay_treble,
        6 => u.hf = m.hf,
        7 => u.hf_spread = m.hf_spread,
        8 => u.uneven = m.uneven,
        9 => u.polar = m.polar,
        10 => u.formant_hz = m.formant_hz,
        11 => u.formant_db = m.formant_db,
        12 => u.formant_q = m.formant_q,
        13 => u.knock = m.knock,
        14 => u.knock_tone = m.knock_tone,
        15 => u.key_return = m.key_return,
        16 => u.damper = m.damper,
        17 => u.damper_bass = m.damper_bass,
        18 => u.damped_db = m.damped_db,
        19 => u.itd = m.itd,
        20 => u.top_hz = m.top_hz,
        _ => u.amount = m.amount,
    }
    u
}

const CONTROLS: usize = 22;

#[test]
fn every_control_at_its_default_leaves_the_voicing_to_the_bit() {
    for base in [Voicing::shipped(), Voicing::c17(), Voicing::beta14()] {
        assert_eq!(UserVoicing::NEUTRAL.over(&base), base);
        assert_eq!(UserVoicing::default(), UserVoicing::NEUTRAL);
    }
    assert!(UserVoicing::NEUTRAL.is_neutral() && !moved().is_neutral());
    // Each control alone touches the voicing (none is lost on its way).
    let base = Voicing::shipped();
    for i in 0..CONTROLS {
        assert_ne!(one(i).over(&base), base, "control {i} touched nothing");
    }
}

#[test]
fn a_unit_with_every_control_at_its_default_plays_the_shipped_bits() {
    let ev = phrase();
    let plain = perform(&mut piano2(), &ev, FRAMES, 256, 1);
    let neutral = perform(&mut unit(&UserVoicing::NEUTRAL), &ev, FRAMES, 256, 1);
    assert!(neutral == plain);
    // Moved and brought back before the phrase: the same bits.
    let mut c = unit(&moved());
    let s = ChordaSettings {
        model: 2,
        ..ChordaSettings::default()
    };
    c.set(&s, 0.0);
    assert!(perform(&mut c, &ev, FRAMES, 256, 1) == plain);
}

#[test]
fn each_control_moves_the_sound() {
    let ev = phrase();
    let plain = perform(&mut piano2(), &ev, FRAMES, 256, 1);
    let mut still = Vec::new();
    for i in 0..CONTROLS {
        let got = perform(&mut unit(&one(i)), &ev, FRAMES, 256, 1);
        if got == plain {
            still.push(i);
        }
        assert!(
            got.0
                .iter()
                .chain(&got.1)
                .all(|x| x.is_finite() && x.abs() < 1.0),
            "control {i}: not finite or past full scale"
        );
    }
    assert!(still.is_empty(), "controls {still:?} moved no sample");
}

#[test]
fn every_control_moved_gives_the_same_bits_at_every_block_size_and_on_threads() {
    let ev = phrase();
    let u = moved();
    let reference = perform(&mut unit(&u), &ev, FRAMES, 1, 1);
    for (block, threads) in [(7, 1), (37, 1), (64, 1), (100, 4), (300, 1), (512, 3)] {
        let got = perform(&mut unit(&u), &ev, FRAMES, block, threads);
        assert!(got == reference, "block {block}, {threads} threads");
    }
}

#[test]
fn every_control_moved_idles_as_new_and_comes_to_rest() {
    let ev = phrase();
    let u = moved();
    let fresh = perform(&mut unit(&u), &ev, FRAMES, 256, 1);
    let mut idle = unit(&u);
    perform(
        &mut idle,
        &[(0, Ev::On(100, 1.0, 1)), (100, Ev::Off(100, 1))],
        512,
        512,
        1,
    );
    let mut frames: usize = 0;
    while !idle.at_rest() {
        perform(&mut idle, &[], 4_800, 512, 1);
        frames += 4_800;
        assert!(frames < 48_000 * 60, "no rest within a minute");
    }
    // The board's quarter-rate phase and the grid: start where a new unit's would.
    let to = (frames + 512).div_ceil(144_000) * 144_000;
    perform(&mut idle, &[], to - frames - 512, 512, 1);
    assert!(perform(&mut idle, &ev, FRAMES, 256, 1) == fresh);
}

#[test]
fn the_formant_and_the_top_move_on_a_ringing_unit_without_a_break() {
    // Moved while a chord rings (the audio thread's case): finite, and back again the filters
    // are the shipped ones.
    let mut c = piano2();
    perform(
        &mut c,
        &[(0, Ev::On(36, 0.9, 1)), (0, Ev::On(60, 0.9, 2))],
        4_800,
        256,
        1,
    );
    let mut s = ChordaSettings {
        model: 2,
        user: moved(),
        ..ChordaSettings::default()
    };
    c.set(&s, 0.0);
    let (l, r) = perform(&mut c, &[], 4_800, 256, 1);
    assert!(l.iter().chain(&r).all(|x| x.is_finite()));
    assert!(l.iter().any(|x| *x != 0.0));
    // The top roll-off switched off and on again keeps the unit coming to rest.
    s.user.top_hz = Some(0.0);
    s.user.formant_db = Some(0.0);
    c.set(&s, 0.0);
    perform(
        &mut c,
        &[(0, Ev::Off(36, 1)), (0, Ev::Off(60, 2))],
        4_800,
        256,
        1,
    );
    s.user = UserVoicing::NEUTRAL;
    c.set(&s, 0.0);
    let mut frames: usize = 0;
    while !c.at_rest() {
        perform(&mut c, &[], 4_800, 512, 1);
        frames += 4_800;
        assert!(frames < 48_000 * 60, "no rest within a minute");
    }
}
