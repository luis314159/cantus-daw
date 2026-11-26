//! r42 arcus-expr — the player's tests (ADR 0162): its null (off, pass 6's instrument to the
//! bit — `upper_tests::the_player_off_keeps_pass_six_s_bits`), the bow along the stick, the
//! starts from the string and the air, the fingering and the vibrato it decides.

use super::bow::{self, Part, Start};
use super::tables::Instrument;
use super::*;

const SR: f32 = 48_000.0;

fn arcus(s: ArcusSettings) -> Arcus {
    let mut a = Arcus::new(SR);
    a.set(&s);
    a
}

/// Run `a` for `secs` in blocks of 256, calling `each` after every block with the time.
fn run(a: &mut Arcus, secs: f32, mut each: impl FnMut(&Arcus, f32)) -> Vec<f32> {
    let frames = (secs * SR) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for at in (0..frames).step_by(256) {
        let n = 256.min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
        each(a, (at + n) as f32 / SR);
    }
    l
}

fn voice(a: &Arcus) -> &voice::Voice {
    a.voices
        .iter()
        .find(|v| v.active())
        .expect("a sounding voice")
}

#[test]
fn a_down_bow_moves_from_the_frog_to_the_tip() {
    for inst in Instrument::ALL {
        let mut a = arcus(ArcusSettings {
            instrument: inst,
            ..ArcusSettings::default()
        });
        let key = inst.spec().strings[1].open + 4;
        a.note_on(key, 0.5, 1);
        let mut xs = Vec::new();
        let mut dirs = Vec::new();
        run(&mut a, 8.0, |a, _| {
            let v = voice(a);
            xs.push(v.bow_x());
            dirs.push(v.bow_velocity() >= 0.0);
        });
        assert!(
            xs[0] < 0.2,
            "{inst:?}: a down-bow from silence starts at the frog"
        );
        assert!(xs.iter().all(|x| (0.0..=1.0).contains(x)));
        // The automatic change is off (`bow::CHANGE_M`): the bow stays down, at its end.
        let changes = dirs.windows(2).filter(|w| w[0] != w[1]).count();
        assert_eq!(changes, 0, "{inst:?}: no change by itself");
        assert!(
            xs.iter().cloned().fold(0.0, f32::max) > 0.9,
            "{inst:?}: the whole bow is used"
        );
    }
}

#[test]
fn the_arm_weighs_more_at_the_frog_than_at_the_tip() {
    assert!(bow::weight(0.0, 1.0) > 1.07 && bow::weight(1.0, 1.0) < 0.93);
    assert_eq!(bow::weight(0.3, 0.0), 1.0);
    // In a stroke: the force at a steady dynamic falls as a down-bow nears the tip.
    let mut a = arcus(ArcusSettings {
        vib_depth: 0.0,
        ..ArcusSettings::default()
    });
    a.controller(0, 0.5);
    a.note_on(50, 0.4, 1);
    let mut at = Vec::new();
    run(&mut a, 2.4, |a, t| {
        let v = voice(a);
        if v.bow_velocity() > 0.0 && t > 0.4 {
            at.push((v.bow_x(), v.bow_force() / v.limits().1.max(1e-6)));
        }
    });
    let near = |lo: f32, hi: f32| {
        let v: Vec<f32> = at
            .iter()
            .filter(|(x, _)| (lo..hi).contains(x))
            .map(|p| p.1)
            .collect();
        v.iter().sum::<f32>() / v.len().max(1) as f32
    };
    // The player makes up most of the fall (`bow::COMPENSATE`): a little lighter at the tip.
    let (frog, tip) = (near(0.2, 0.4), near(0.65, 0.85));
    assert!(
        frog > tip * 1.003 && frog < tip * 1.03,
        "frog {frog:.3} tip {tip:.3}"
    );
    assert!(bow::stroke_weight(0.0, 1.0) < 1.02 && bow::stroke_weight(1.0, 1.0) > 0.98);
}

/// The first and the later bow speed of a note started `start`, at `velocity`.
fn speeds(start: Option<Start>, velocity: f32, t_s: f32) -> (f32, f32) {
    let mut a = arcus(ArcusSettings::default());
    a.note_how(
        1,
        NoteHow {
            start,
            ..NoteHow::default()
        },
    );
    a.note_on(50, velocity, 1);
    let mut first = None;
    let mut later = 0.0;
    run(&mut a, t_s + 0.01, |a, t| {
        let v = voice(a);
        if first.is_none() {
            first = Some(v.bow_velocity());
        }
        if t <= t_s {
            later = v.bow_velocity();
        }
    });
    (first.unwrap(), later)
}

#[test]
fn a_start_from_the_air_lands_moving_and_one_from_the_string_settles_slower() {
    let (air0, _) = speeds(Some(Start::Air), 0.5, 0.05);
    let (str0, _) = speeds(Some(Start::String), 0.5, 0.05);
    assert!(
        air0 > 0.03,
        "from the air the bow is moving at once: {air0}"
    );
    assert!(
        str0 < air0 * 0.5,
        "from the string it starts from rest: {str0}"
    );
    // The player's choice: a hard attack from the air, a soft one from the string.
    assert_eq!(bow::auto_start(0.9, false), Start::String);
    assert_eq!(bow::auto_start(0.4, false), Start::String);
    assert_eq!(bow::auto_start(0.4, true), Start::Air);
    // From the string the stroke's force settles over its contour: slower than the classic
    // attack's (the player off), so the spectrum takes its time to settle.
    let force_at = |player: f32, t_s: f32| {
        let mut a = arcus(ArcusSettings {
            player,
            vib_depth: 0.0,
            ..ArcusSettings::default()
        });
        a.note_on(50, 0.6, 1);
        let mut f = 0.0;
        run(&mut a, t_s, |a, _| f = voice(a).bow_force());
        f
    };
    let (on, off) = (force_at(1.0, 0.12), force_at(0.0, 0.12));
    let (on_l, off_l) = (force_at(1.0, 1.0), force_at(0.0, 1.0));
    // A firm attack's force eases to the stroke's: at 120 ms it is still further from it.
    assert!(
        (on - on_l).abs() > 1.15 * (off - off_l).abs(),
        "at 120 ms {on:.3} / {off:.3}, settled {on_l:.3} / {off_l:.3}"
    );
}

#[test]
fn a_part_of_the_bow_puts_the_stroke_there() {
    assert!(bow::start_x(BowDir::Down, Some(Part::Tip), false) > 0.6);
    assert!(bow::start_x(BowDir::Up, Some(Part::Frog), false) < 0.4);
    assert!((bow::start_x(BowDir::Down, Some(Part::Middle), false) - 0.4).abs() < 1e-6);
    let mut a = arcus(ArcusSettings::default());
    a.note_how(
        7,
        NoteHow {
            part: Some(Part::Tip),
            ..NoteHow::default()
        },
    );
    a.note_on(55, 0.5, 7);
    let mut x0 = None;
    run(&mut a, 0.02, |a, _| {
        x0.get_or_insert(voice(a).bow_x());
    });
    assert!(x0.unwrap() > 0.6);
}

#[test]
fn the_fingering_honours_a_named_string_and_names_a_shift() {
    let cello = Instrument::Cello.spec();
    // The open A (57): first position, open; asked for on the D string, a fourth finger.
    let open = bow::finger(cello, 57.0, None, true, None);
    assert_eq!((open.string, open.open), (3, true));
    let stopped = bow::finger(cello, 57.0, Some(2), true, None);
    assert_eq!((stopped.string, stopped.open), (2, false));
    // Sul D: the note asks for its string (0 the lowest), where it reaches.
    assert_eq!(bow::finger(cello, 62.0, Some(2), true, None).string, 2);
    assert_eq!(bow::finger(cello, 45.0, Some(3), true, None).string, 1);
    // A slur up a fifth on the A string is a shift; a step is not.
    let up = bow::finger(cello, 64.0, None, true, Some((59.0, 3)));
    assert!(up.shift && up.string == 3);
    let step = bow::finger(cello, 61.0, None, true, Some((59.0, 3)));
    assert!(!step.shift);
}

#[test]
fn an_open_string_in_a_passage_has_no_vibrato() {
    let mut a = arcus(ArcusSettings::default());
    // A fast détaché, then the open A.
    let mut most = 0.0f32;
    for (i, key) in [59u8, 60, 59, 57].iter().enumerate() {
        a.note_on(*key, 0.5, i as u64 + 1);
        let last = i == 3;
        run(&mut a, if last { 1.2 } else { 0.15 }, |a, _| {
            if last {
                most = most.max(voice(a).vibrato().abs());
            }
        });
        if !last {
            a.note_off(*key, i as u64 + 1);
        }
    }
    assert!(most < 0.5, "the open A vibrates {most:.2} cents");
}

#[test]
fn with_the_player_off_a_note_s_bow_marks_change_nothing() {
    let render = |how: NoteHow| {
        let mut a = arcus(ArcusSettings {
            player: 0.0,
            ..ArcusSettings::default()
        });
        a.note_how(3, how);
        a.note_on(57, 0.6, 3);
        run(&mut a, 0.6, |_, _| {})
    };
    let plain = render(NoteHow::default());
    let marked = render(NoteHow {
        start: Some(Start::Air),
        part: Some(Part::Tip),
        string: Some(1),
        ..NoteHow::default()
    });
    assert!(plain
        .iter()
        .zip(&marked)
        .all(|(x, y)| x.to_bits() == y.to_bits()));
}
