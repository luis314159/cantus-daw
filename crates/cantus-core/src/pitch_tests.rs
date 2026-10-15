use super::curves::{decompose, margin, time_points, Curves, Frames};
use super::edit::{self, Field};
use super::midi::{self, Interval};
use super::*;

const SR: u32 = 48_000;
const HOP: usize = 256;

/// Three notes a quarter second apart, sung slightly off: 60.3, 64.0 and 67.2 (C, E, G
/// with the first and last out of tune), each 0.5 s, gaps of 0.25 s.
fn take() -> (PitchEdit, Vec<f32>) {
    let len = SR as u64 * 3;
    let spans = [
        (12_000u64, 36_000u64, 60.3f32),
        (48_000, 72_000, 64.0),
        (84_000, 108_000, 67.2),
    ];
    let mut midi = vec![f32::NAN; (len as usize).div_ceil(HOP) + 1];
    for (a, b, m) in spans {
        for (i, v) in midi.iter_mut().enumerate() {
            let t = (i * HOP) as u64;
            if t >= a && t < b {
                *v = m;
            }
        }
    }
    let notes = spans
        .iter()
        .enumerate()
        .map(|(i, (a, b, m))| PitchNote::sung(i as u32, *a, *b, *m, -12.0))
        .collect();
    (
        PitchEdit {
            rate: SR,
            frames: len,
            analyser: 1,
            key: PitchKey { tonic: 0, scale: 1 },
            notes,
            bypass: false,
        },
        midi,
    )
}

fn frames(midi: &[f32]) -> Frames<'_> {
    Frames { hop: HOP, midi }
}

#[test]
fn an_untouched_edit_asks_for_nothing_exactly() {
    let (e, midi) = take();
    assert!(e.is_valid());
    assert!(!e.edited() && !e.audible());
    let c = Curves::new(&e, &frames(&midi));
    for t in (0..e.frames).step_by(7) {
        let t = t as f64;
        assert_eq!(c.shift_at(t), 0.0, "{t}");
        assert_eq!(c.formant_at(t), 0.0);
        assert_eq!(c.gain_db_at(t), 0.0);
    }
    assert!(time_points(&e).is_empty(), "the identity");
}

#[test]
fn a_moved_note_shifts_itself_and_never_its_neighbours() {
    let (mut e, midi) = take();
    assert!(edit::set_pitch(&mut e, 1, 66.0));
    assert!(e.audible());
    let c = Curves::new(&e, &frames(&midi));
    // Inside: the whole move.
    for t in (48_000..72_000).step_by(101) {
        assert_eq!(c.shift_at(t as f64), 2.0);
    }
    // The neighbours, and a margin beside them: nothing.
    let m = margin(SR);
    for t in (12_000..36_000 + m).chain(84_000 - m..108_000) {
        assert_eq!(c.shift_at(t as f64), 0.0, "{t}");
    }
    // The gaps ramp to it: no jump bigger than a step of the ramp.
    let mut last = 0.0f32;
    for t in 36_000..48_000 {
        let s = c.shift_at(t as f64);
        assert!((s - last).abs() < 0.01, "{t}: {last} → {s}");
        last = s;
    }
    assert_eq!(last, 2.0);
}

#[test]
fn the_centre_pulls_onto_the_note_and_a_move_keeps_the_deviation() {
    let (mut e, _) = take();
    let n = e.note(0).unwrap().clone();
    assert_eq!(n.home(), 60.0);
    assert_eq!(n.out_centre(), 60.3, "untouched: as sung");
    edit::set_field(&mut e, &[0], Field::Centre, 1.0);
    assert!((e.note(0).unwrap().out_centre() - 60.0).abs() < 1e-5);
    edit::set_field(&mut e, &[0], Field::Centre, 0.5);
    assert!((e.note(0).unwrap().out_centre() - 60.15).abs() < 1e-5);
    edit::set_field(&mut e, &[0], Field::Centre, 0.0);
    edit::set_pitch(&mut e, 0, 62.0);
    assert!(
        (e.note(0).unwrap().out_centre() - 62.3).abs() < 1e-5,
        "moved, as sung"
    );
    // Clamped: a move is at most two octaves; a centre 0 … 1.
    edit::set_pitch(&mut e, 0, 200.0);
    assert_eq!(e.note(0).unwrap().pitch, 60.0 + MAX_MOVE);
    edit::set_field(&mut e, &[0], Field::Centre, 3.0);
    assert_eq!(e.note(0).unwrap().centre, 1.0);
    assert!(e.is_valid());
}

/// A sung note: 0.6 semitone of slow drift up across it and ±0.5 of vibrato at 5.5 Hz.
fn drifting(len_s: f64) -> (PitchNote, Vec<f32>, Vec<f32>, Vec<f32>) {
    let n_frames = (len_s * SR as f64 / HOP as f64) as usize;
    let (mut midi, mut d, mut v) = (vec![], vec![], vec![]);
    for i in 0..n_frames {
        let t = (i * HOP) as f64 / SR as f64;
        let drift = 0.6 * (t / len_s) - 0.3;
        let vib = 0.5 * cantus_dsp::math::sin(2.0 * std::f64::consts::PI * 5.5 * t);
        midi.push((62.0 + drift + vib) as f32);
        d.push(drift as f32);
        v.push(vib as f32);
    }
    let note = PitchNote::sung(0, 0, (n_frames * HOP) as u64, 62.0, -12.0);
    (note, midi, d, v)
}

#[test]
fn a_notes_pitch_comes_apart_into_drift_and_vibrato() {
    let (note, midi, d, v) = drifting(1.5);
    let p = decompose(&note, &frames(&midi), SR);
    assert_eq!(p.first, 0);
    // Away from the ends (half a window), each part is what was put in.
    let edge = (0.2 * SR as f64 / HOP as f64) as usize;
    let (mut ed, mut ev) = (0.0f32, 0.0f32);
    for i in edge..midi.len() - edge {
        ed = ed.max((p.drift[i] - d[i]).abs());
        ev = ev.max((p.vibrato[i] - v[i]).abs());
        // Together they are the sung pitch, to the float.
        assert!((62.0 + p.drift[i] + p.vibrato[i] - midi[i]).abs() < 1e-4);
    }
    assert!(ed < 0.05, "drift within 5 cents: {ed}");
    assert!(ev < 0.06, "vibrato within 6 cents: {ev}");
}

#[test]
fn drift_and_vibrato_scale_their_own_part() {
    let (note, midi, _, _) = drifting(1.5);
    let mk = |drift: f32, vibrato: f32| {
        let mut n = note.clone();
        n.drift = drift;
        n.vibrato = vibrato;
        PitchEdit {
            rate: SR,
            frames: n.src_end + SR as u64,
            analyser: 1,
            key: PitchKey::default(),
            notes: vec![n],
            bypass: false,
        }
    };
    let f = frames(&midi);
    let p = decompose(&note, &f, SR);
    let flat = Curves::new(&mk(0.0, 1.0), &f);
    let still = Curves::new(&mk(1.0, 0.0), &f);
    for i in (40..midi.len() - 40).step_by(5) {
        let t = (i * HOP) as f64;
        assert!(
            (flat.shift_at(t) + p.drift[i]).abs() < 1e-5,
            "drift removed"
        );
        assert!(
            (still.shift_at(t) + p.vibrato[i]).abs() < 1e-5,
            "vibrato removed"
        );
    }
}

#[test]
fn formant_and_gain_stay_inside_their_note() {
    let (mut e, midi) = take();
    edit::set_field(&mut e, &[1], Field::Formant, 3.0);
    edit::set_field(&mut e, &[1], Field::Gain, -6.0);
    let c = Curves::new(&e, &frames(&midi));
    assert_eq!(c.formant_at(60_000.0), 3.0);
    assert_eq!(c.gain_db_at(60_000.0), -6.0);
    let m = margin(SR);
    for t in (0..36_000 + m).chain(84_000 - m..e.frames) {
        assert_eq!(c.formant_at(t as f64), 0.0, "{t}");
    }
    // The neighbours and the margins beside them: untouched.
    for t in (0..36_000 + m).chain(84_000 - m..e.frames) {
        assert_eq!(c.gain_db_at(t as f64), 0.0, "{t}");
    }
}

/// The largest step of the gain (dB) from one output sample to the next over `range`.
fn gain_step(c: &Curves, range: std::ops::Range<u64>) -> f32 {
    let mut last = c.gain_db_at(range.start as f64);
    let mut most = 0.0f32;
    for t in range {
        let g = c.gain_db_at(t as f64);
        most = most.max((g - last).abs());
        last = g;
    }
    most
}

/// Review 1: a gain at a legato joint, or across a short gap, never steps (a click).
#[test]
fn a_notes_gain_never_steps_at_a_legato_joint_or_a_short_gap() {
    let (mut e, midi) = take();
    // Legato: the middle note split; its second half at −12 dB.
    assert!(edit::split(&mut e, 1, 60_000, None));
    let right = e.notes[2].id;
    edit::set_field(&mut e, &[right], Field::Gain, -12.0);
    let c = Curves::new(&e, &frames(&midi));
    let step = gain_step(&c, 40_000..80_000);
    assert!(step < 0.05, "a {step} dB step at the legato joint");
    // A short gap: both halves at −12 dB, the second moved 100 frames later.
    let left = e.notes[1].id;
    edit::set_field(&mut e, &[left], Field::Gain, -12.0);
    let (s, en) = (e.notes[2].start, e.notes[2].end);
    assert!(edit::set_timing(&mut e, right, s + 100, en));
    let c = Curves::new(&e, &frames(&midi));
    let step = gain_step(&c, 40_000..80_000);
    assert!(step < 0.05, "a {step} dB step across the short gap");
    assert_eq!(
        c.gain_db_at(60_050.0),
        -12.0,
        "the gap keeps the notes' gain"
    );
}

/// Review 1: correct pitch below 100 % pulls toward the key's note, never past it, and at
/// 0 % leaves the note alone; applied again at another amount, it is as if applied once.
#[test]
fn correct_pitch_below_full_never_overshoots() {
    let (mut e, _) = take();
    // C major; a C# sung 40 cents sharp (61.4): its target is D (62).
    e.notes
        .push(PitchNote::sung(9, 120_000, 130_000, 61.4, -12.0));
    let at = |e: &PitchEdit| e.note(9).unwrap().out_centre();
    let mut z = e.clone();
    edit::correct_pitch(&mut z, &[9], 0.0);
    assert!((at(&z) - 61.4).abs() < 1e-4, "0 %: untouched, {}", at(&z));
    let mut h = e.clone();
    edit::correct_pitch(&mut h, &[9], 0.5);
    assert!(
        (at(&h) - 61.7).abs() < 1e-4,
        "50 %: half way to D, {}",
        at(&h)
    );
    let mut f = e.clone();
    edit::correct_pitch(&mut f, &[9], 1.0);
    assert!((at(&f) - 62.0).abs() < 1e-4);
    edit::correct_pitch(&mut f, &[9], 0.5);
    assert!((at(&f) - 61.7).abs() < 1e-4, "100 % then 50 %: {}", at(&f));
    assert!(f.is_valid());
}

/// Review 2: which pitch is the macro's is stored on the note, so a key change between two
/// runs re-targets it (it was left a quarter tone off the key), and a hand move onto the
/// key's note is kept (it was taken back).
#[test]
fn correct_pitch_follows_a_key_change_and_keeps_a_hand_move() {
    let (mut e, _) = take();
    e.notes
        .push(PitchNote::sung(9, 120_000, 130_000, 61.4, -12.0));
    let at = |e: &PitchEdit| e.note(9).unwrap().out_centre();
    let mut k = e.clone();
    // C major at 50 %: half way to D; then D major, where C# is the key's note.
    edit::correct_pitch(&mut k, &[9], 0.5);
    k.key.tonic = 2;
    edit::correct_pitch(&mut k, &[9], 1.0);
    assert!((at(&k) - 61.0).abs() < 1e-4, "on C#: {}", at(&k));
    assert!(k.note(9).unwrap().corrected);
    // Moved by hand onto D: a later run keeps D as its target.
    let mut h = e.clone();
    edit::set_pitch(&mut h, 9, 62.0);
    assert!(!h.note(9).unwrap().corrected);
    edit::correct_pitch(&mut h, &[9], 0.5);
    assert_eq!(h.note(9).unwrap().pitch, 62.0, "the hand's move kept");
    assert!(!h.note(9).unwrap().corrected);
    // A macro note moved by hand afterwards is the hand's from then on.
    edit::correct_pitch(&mut k, &[9], 0.5);
    edit::set_pitch(&mut k, 9, 63.0);
    edit::correct_pitch(&mut k, &[9], 1.0);
    assert_eq!(k.note(9).unwrap().pitch, 63.0);
}

/// Review 1: a step of the key from a note sung off it lands on the key's next note.
#[test]
fn a_scale_step_from_an_off_key_note_lands_on_the_key() {
    let c_major = PitchKey { tonic: 0, scale: 1 };
    assert_eq!(c_major.step(61.0, 1), 62.0, "C# up: D");
    assert_eq!(c_major.step(61.0, -1), 60.0, "C# down: C");
    assert_eq!(c_major.step(60.0, 1), 62.0, "C up: D");
    assert_eq!(c_major.step(64.0, 1), 65.0, "E up: F");
    assert!((c_major.step(60.3, 1) - 62.3).abs() < 1e-5, "cents kept");
    assert_eq!(c_major.steps_to(61.0, 62.0), 1);
    assert_eq!(c_major.steps_to(61.0, 60.0), -1);
    assert_eq!(c_major.steps_to(60.0, 64.0), 2);
}

/// Review 1 (§3c): quantise timing to another track's notes — each note's start to the
/// nearest onset within reach, its length kept.
#[test]
fn quantise_to_onsets_moves_notes_onto_another_tracks_notes() {
    let (mut e, _) = take();
    let ids: Vec<u32> = e.notes.iter().map(|n| n.id).collect();
    // Onsets near the first two notes; none near the third (out of reach).
    let onsets = [11_000u64, 49_500, 200_000];
    assert!(edit::quantise_to(&mut e, &ids, &onsets, 4_800));
    let starts: Vec<u64> = e.notes.iter().map(|n| n.start).collect();
    assert_eq!(starts, vec![11_000, 49_500, 84_000]);
    assert_eq!(e.notes[0].end - e.notes[0].start, 24_000);
    assert!(e.is_valid());
}

#[test]
fn a_timing_edit_maps_only_its_note_and_the_gaps_beside_it() {
    let (mut e, _) = take();
    // The middle note 0.1 s later and 20 % longer.
    assert!(edit::set_timing(&mut e, 1, 52_800, 81_600));
    assert!(e.is_valid());
    let pts = time_points(&e);
    let map = cantus_dsp::psola::TimeMap::new(pts.clone()).expect("a valid map");
    // The neighbours read themselves, sample for sample.
    for t in (0..36_000).chain(84_000..e.frames) {
        assert_eq!(map.source(t as f64), t as f64, "{t}");
    }
    // And a margin beside them.
    let m = margin(SR);
    assert_eq!(map.source((36_000 + m) as f64), (36_000 + m) as f64);
    assert_eq!(map.source((84_000 - m) as f64), (84_000 - m) as f64);
    // The note reads its own source, stretched.
    assert_eq!(map.source(52_800.0), 48_000.0);
    assert_eq!(map.source(81_600.0), 72_000.0);
}

#[test]
fn timing_stops_at_the_neighbours_and_at_the_shortest_note() {
    let (mut e, _) = take();
    edit::set_timing(&mut e, 1, 0, 200_000);
    let n = e.note(1).unwrap();
    assert_eq!(
        (n.start, n.end),
        (36_001, 83_999),
        "strictly between, gaps kept"
    );
    edit::set_timing(&mut e, 1, 60_000, 60_000);
    let n = e.note(1).unwrap();
    assert_eq!(n.end - n.start, e.min_len());
    // A shift keeps the length.
    let (mut e, _) = take();
    edit::shift_timing(&mut e, 1, 1_000_000);
    let n = e.note(1).unwrap();
    assert_eq!((n.start, n.end), (83_999 - 24_000, 83_999));
    assert!(e.is_valid());
}

#[test]
fn split_and_join_give_the_note_back() {
    let (mut e, midi) = take();
    edit::set_pitch(&mut e, 1, 65.0);
    let before = e.clone();
    assert!(edit::split(&mut e, 1, 60_000, Some(&frames(&midi))));
    assert_eq!(e.notes.len(), 4);
    let (l, r) = (&e.notes[1], &e.notes[2]);
    assert_eq!((l.id, l.src_end, r.src_start, r.id), (1, 60_000, 60_000, 3));
    assert_eq!(
        (l.pitch, r.pitch),
        (65.0, 65.0),
        "the move kept on both halves"
    );
    assert!(e.is_valid());
    // Too near an end: refused.
    assert!(!edit::split(&mut e, 1, 48_100, None));
    assert!(edit::join(&mut e, &[1, 3]));
    assert_eq!(e.notes.len(), 3);
    let j = &e.notes[1];
    let b = &before.notes[1];
    assert_eq!(
        (j.src_start, j.src_end, j.start, j.end),
        (b.src_start, b.src_end, b.start, b.end)
    );
    assert!((j.sung - b.sung).abs() < 1e-5);
    assert_eq!(j.pitch, b.pitch);
    // Notes that are not neighbours do not join.
    assert!(!edit::join(&mut e, &[0, 2]));
}

#[test]
fn correct_pitch_puts_the_take_in_its_key() {
    let (mut e, _) = take();
    // C major: 60.3 → C, 64.0 → E, 67.2 → G; and a C# sung 61.4 goes to D.
    e.notes
        .push(PitchNote::sung(9, 120_000, 130_000, 61.4, -12.0));
    let ids: Vec<u32> = e.notes.iter().map(|n| n.id).collect();
    assert!(edit::correct_pitch(&mut e, &ids, 1.0));
    let got: Vec<f32> = e.notes.iter().map(|n| n.out_centre()).collect();
    for (g, want) in got.iter().zip([60.0, 64.0, 67.0, 62.0]) {
        assert!((g - want).abs() < 1e-4, "{got:?}");
    }
    // Half way: half the deviation left.
    let (mut e, _) = take();
    edit::correct_pitch(&mut e, &[0], 0.5);
    assert!((e.note(0).unwrap().out_centre() - 60.15).abs() < 1e-5);
    // Drift.
    edit::correct_drift(&mut e, &[0, 1], 0.75);
    assert_eq!(e.note(1).unwrap().drift, 0.25);
}

#[test]
fn quantise_moves_whole_notes_to_the_grid() {
    let (mut e, _) = take();
    // A grid every 10 000 frames.
    let snap = |f: u64| (f + 5_000) / 10_000 * 10_000;
    let ids: Vec<u32> = e.notes.iter().map(|n| n.id).collect();
    assert!(edit::quantise(&mut e, &ids, snap));
    let starts: Vec<(u64, u64)> = e.notes.iter().map(|n| (n.start, n.end - n.start)).collect();
    assert_eq!(
        starts,
        vec![(10_000, 24_000), (50_000, 24_000), (80_000, 24_000)]
    );
    assert!(e.is_valid());
}

#[test]
fn reset_takes_every_edit_back() {
    let (mut e, _) = take();
    let clean = e.clone();
    edit::set_pitch(&mut e, 0, 64.0);
    edit::set_field(&mut e, &[0, 1], Field::Vibrato, 0.0);
    edit::set_timing(&mut e, 1, 50_000, 70_000);
    assert!(edit::reset(&mut e, &[0, 1, 2]));
    assert_eq!(e, clean);
}

#[test]
fn the_key_snaps_and_counts_degrees() {
    let c_major = PitchKey { tonic: 0, scale: 1 };
    assert_eq!(c_major.nearest(61.4), 62.0);
    assert_eq!(c_major.nearest(61.5), 62.0);
    assert_eq!(c_major.nearest(60.4), 60.0);
    assert_eq!(c_major.degrees_from(60.0, 2), 64.0, "C → E");
    assert_eq!(c_major.degrees_from(62.0, 2), 65.0, "D → F");
    assert_eq!(c_major.degrees_from(64.0, 4), 71.0, "E → B");
    let a_minor = PitchKey { tonic: 9, scale: 2 };
    assert!(a_minor.contains(69) && a_minor.contains(72) && !a_minor.contains(73));
}

#[test]
fn midi_out_and_the_double() {
    let (mut e, _) = take();
    edit::correct_pitch(&mut e, &[0, 1, 2], 1.0);
    let m = midi::notes(&e);
    let pitches: Vec<u8> = m.iter().map(|n| n.pitch).collect();
    assert_eq!(pitches, vec![60, 64, 67]);
    assert_eq!((m[0].start, m[0].end), (12_000, 36_000));
    assert!(m.iter().all(|n| n.velocity > 64 && n.velocity < 127));
    let third = midi::double(&e, Interval::Third);
    let outs: Vec<f32> = third.notes.iter().map(|n| n.out_centre().round()).collect();
    assert_eq!(
        outs,
        vec![64.0, 67.0, 71.0],
        "a diatonic third above in C major"
    );
    let fifth = midi::double(&e, Interval::Fifth);
    let outs: Vec<f32> = fifth.notes.iter().map(|n| n.out_centre().round()).collect();
    assert_eq!(outs, vec![67.0, 71.0, 74.0]);
    assert!(third.is_valid() && fifth.is_valid());
}

#[test]
fn defaults_are_not_written_and_everything_round_trips() {
    let (mut e, _) = take();
    let json = serde_json::to_string(&e).unwrap();
    for absent in ["centre", "drift", "vibrato", "formant", "gain_db", "bypass"] {
        assert!(!json.contains(absent), "{absent} in {json}");
    }
    edit::set_field(&mut e, &[0], Field::Vibrato, 0.5);
    edit::set_field(&mut e, &[1], Field::Formant, -2.0);
    e.bypass = true;
    let json = serde_json::to_string(&e).unwrap();
    assert_eq!(serde_json::from_str::<PitchEdit>(&json).unwrap(), e);
}

#[test]
fn invalid_edits_are_told_apart() {
    let (e, _) = take();
    let bad = |f: &dyn Fn(&mut PitchEdit)| {
        let mut x = e.clone();
        f(&mut x);
        !x.is_valid()
    };
    assert!(bad(&|x| x.notes[1].start = 30_000), "overlaps the first");
    assert!(bad(&|x| x.notes[1].src_end = 90_000), "source overlap");
    assert!(bad(&|x| x.notes[2].end = 10_000_000), "past the take");
    assert!(bad(&|x| x.notes[0].drift = f32::NAN));
    assert!(bad(&|x| x.notes[0].gain_db = 40.0));
    assert!(bad(&|x| x.notes[2].id = 0), "a duplicate id");
    assert!(
        bad(&|x| x.notes[1].start = 36_000),
        "a source gap closed to nothing"
    );
    assert!(bad(&|x| x.rate = 0));
}

/// Random edits through every function keep an edit valid (what a command requires).
#[test]
fn every_edit_keeps_the_edit_valid() {
    let (base, midi) = take();
    let f = frames(&midi);
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut rnd = |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n.max(1)
    };
    for _ in 0..40 {
        let mut e = base.clone();
        for _ in 0..30 {
            let ids: Vec<u32> = e.notes.iter().map(|n| n.id).collect();
            let id = ids[rnd(ids.len() as u64) as usize];
            match rnd(9) {
                0 => {
                    edit::set_pitch(&mut e, id, 50.0 + rnd(30) as f32 * 0.7);
                }
                1 => {
                    let field = Field::ALL[rnd(5) as usize];
                    edit::set_field(&mut e, &[id], field, rnd(100) as f32 * 0.3 - 15.0);
                }
                2 => {
                    let a = rnd(e.frames);
                    edit::set_timing(&mut e, id, a, a + rnd(60_000));
                }
                3 => {
                    edit::shift_timing(&mut e, id, rnd(40_000) as i64 - 20_000);
                }
                4 => {
                    let n = e.note(id).unwrap();
                    let at = n.src_start + rnd(n.src_end - n.src_start);
                    edit::split(&mut e, id, at, Some(&f));
                }
                5 => {
                    let i = e.notes.iter().position(|n| n.id == id).unwrap();
                    if let Some(next) = e.notes.get(i + 1).map(|n| n.id) {
                        edit::join(&mut e, &[id, next]);
                    }
                }
                6 => {
                    edit::correct_pitch(&mut e, &ids, rnd(100) as f32 / 100.0);
                }
                7 => {
                    edit::quantise(&mut e, &[id], |x| x / 7_000 * 7_000);
                }
                _ => {
                    edit::reset(&mut e, &[id]);
                }
            }
            assert!(e.is_valid(), "{e:#?}");
            assert!(cantus_dsp::psola::TimeMap::new(time_points(&e)).is_some());
        }
    }
}

/// Review 1: a valid edit sent to a clip that is not audio is refused, and nothing moves.
#[test]
fn a_pitch_edit_on_a_note_clip_is_refused() {
    use crate::{Command, CommandBus, Origin, Ticks};
    let mut bus = CommandBus::default();
    let mut p = bus.project().clone();
    let mut t = p.make_track("Keys");
    let c = p.make_clip("notes", Ticks::ZERO, Ticks::beats(4));
    let (tid, cid) = (t.id, c.id);
    t.clips.push(c);
    bus.execute(
        Command::InsertTrack {
            index: 0,
            track: Box::new(t),
        },
        Origin::User,
    )
    .unwrap();
    let before = bus.project().clone();
    let (e, _) = take();
    assert!(e.is_valid());
    let r = bus.execute(
        Command::SetClipPitch {
            track: tid,
            clip: cid,
            pitch: Some(Box::new(e)),
            op: PitchOp::Notes,
        },
        Origin::User,
    );
    assert!(r.is_err(), "not an audio clip");
    assert_eq!(bus.project(), &before);
}
