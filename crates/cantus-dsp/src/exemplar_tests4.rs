//! Exemplar's DSP tests, part four: round robin **in order** (round 28, sound). A sequenced
//! note's round-robin turn is its ordinal — how many notes of its pitch come before it on
//! the track — so a roll cycles 1, 2, 3, 4, 1… with no sample twice in a row, and a render
//! that starts mid-song cycles exactly as the whole render does there.

use super::tests::source_of;
use super::*;

/// Four zones on every key, round-robin positions 1…4, each a constant of its own level
/// (0.1, 0.2, 0.3, 0.4), so the level a note plays names the zone it chose.
fn engine(in_order: bool) -> Exemplar {
    let mut e = Exemplar::new(48_000.0);
    let g = Globals {
        amp: [0.1, 0.1, 1_000.0, 1.0, 5.0],
        vel_amp: 0.0,
        rr_in_order: in_order,
        ..Globals::default()
    };
    e.set_globals(&g);
    for n in 0..4 {
        let z = ZoneParams {
            seq: n as u8 + 1,
            seqlen: 4,
            one_shot: true,
            ..ZoneParams::default()
        };
        let level = 0.1 * (n + 1) as f32;
        e.put_zone(n, &z, &source_of(vec![level; 4_800], 1, 48_000.0, false));
    }
    e
}

/// The zone (1…4) a note chose: the level of its first frames, in tenths.
fn chosen(e: &mut Exemplar, id: u64, ordinal: Option<u32>) -> usize {
    match ordinal {
        Some(o) => e.note_on_seq(60, 1.0, id, o),
        None => e.note_on(60, 1.0, id),
    }
    let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
    e.process(&mut l, &mut r);
    e.note_off(60, id, 0.0);
    e.reset();
    // The balance law keeps a centred mono zone at unity on the left.
    (l[40] * 10.0).round() as usize
}

#[test]
fn a_sequenced_note_cycles_by_its_ordinal_in_order() {
    let mut e = engine(true);
    let turns: Vec<usize> = (0..9u32)
        .map(|o| chosen(&mut e, 1_000 + o as u64 * 7_919, Some(o)))
        .collect();
    assert_eq!(turns, [1, 2, 3, 4, 1, 2, 3, 4, 1]);
}

#[test]
fn a_render_from_the_middle_cycles_as_the_whole_render_does_there() {
    // Ordinal 6 is the seventh note of its pitch: whatever played before (nothing, after
    // a seek), it takes turn 3.
    let mut e = engine(true);
    assert_eq!(chosen(&mut e, 42, Some(6)), 3);
    assert_eq!(chosen(&mut e, 43, Some(7)), 4);
}

#[test]
fn a_fast_roll_never_plays_the_same_sample_twice_in_a_row() {
    let mut e = engine(true);
    let mut last = 0;
    for o in 0..64u32 {
        let z = chosen(&mut e, 5_000 + o as u64, Some(o));
        assert_ne!(z, last, "ordinal {o} repeated zone {z}");
        last = z;
    }
}

#[test]
fn seeded_round_robin_is_what_it_always_was() {
    // `rr_in_order` off (every Exemplar saved before round 28): a sequenced note's turn is
    // drawn from its id, ordinal or not — the very zone the old engine chose.
    let mut e = engine(false);
    for id in [1u64, 2, 3, 99, 12_345, 777_777] {
        let expect = ((mix(id) >> 33) as u32 % 4) as usize + 1;
        assert_eq!(chosen(&mut e, id, Some(id as u32 * 3)), expect, "id {id}");
        assert_eq!(chosen(&mut e, id, None), expect, "id {id}");
    }
}

#[test]
fn live_notes_keep_their_strict_per_key_cycle() {
    let mut e = engine(true);
    let mut turns = Vec::new();
    for _ in 0..5 {
        e.note_on(60, 1.0, LIVE_IDS + 60);
        let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
        e.process(&mut l, &mut r);
        e.note_off(60, LIVE_IDS + 60, 0.0);
        turns.push((l[40] * 10.0).round() as usize);
        // Let the layer end without forgetting the per-key count.
        let (mut l, mut r) = (vec![0.0; 9_600], vec![0.0; 9_600]);
        e.process(&mut l, &mut r);
    }
    assert_eq!(turns, [1, 2, 3, 4, 1]);
}
