//! The chain's logic in ticks (through [`bake`]): what each effect plays, step by step.

use super::bake::{bake, BakedNote};
use super::*;
use cantus_core::notefx::{Arp, ArpMode, Chord, ChordShape, NoteEffect, ScaleFx, ScaleMode};
use cantus_core::scale::ScaleKind;
use cantus_core::{NoteEvent, NoteId, Ticks};

fn fx(id: u64, kind: NoteFxKind) -> NoteEffect {
    NoteEffect {
        id: NoteFxId(id),
        enabled: true,
        kind,
    }
}

/// Notes `(start, length, pitch)` at velocity 100 as a snapshot's events.
fn events(notes: &[(i64, i64, u8)]) -> Vec<NoteEvent> {
    let mut ev = Vec::new();
    for (i, &(s, l, p)) in notes.iter().enumerate() {
        let note = NoteId(i as u64 + 1);
        ev.push(NoteEvent {
            tick: Ticks(s),
            pitch: p,
            velocity: 100,
            note,
            ordinal: 0,
            release: 0,
        });
        ev.push(NoteEvent {
            tick: Ticks(s + l),
            pitch: p,
            velocity: 0,
            note,
            ordinal: 0,
            release: 0,
        });
    }
    ev.sort_by_key(|e| (e.tick, e.is_on(), e.pitch, e.note));
    ev
}

fn n(start: i64, length: i64, pitch: u8) -> BakedNote {
    BakedNote {
        start: Ticks(start),
        length: Ticks(length),
        pitch,
        velocity: 100,
        release: 0,
    }
}

fn arp(f: impl FnOnce(&mut Arp)) -> NoteEffect {
    let mut a = Arp {
        gate: 0.5,
        ..Arp::default()
    };
    f(&mut a);
    fx(1, NoteFxKind::Arp(a))
}

/// A C major triad held for a beat.
fn triad() -> Vec<NoteEvent> {
    events(&[(0, 960, 60), (0, 960, 64), (0, 960, 67)])
}

#[test]
fn no_effect_or_a_switched_off_one_plays_the_notes_as_they_are() {
    let ev = triad();
    let plain = bake(&[], None, &ev, &[], Ticks(960));
    assert_eq!(plain, vec![n(0, 960, 60), n(0, 960, 64), n(0, 960, 67)]);
    let mut off = arp(|_| {});
    off.enabled = false;
    assert_eq!(bake(&[off], None, &ev, &[], Ticks(960)), plain);
}

#[test]
fn the_arpeggiator_plays_up_down_and_both_ways_in_sixteenths() {
    let ev = triad();
    let up = bake(&[arp(|_| {})], None, &ev, &[], Ticks(960));
    assert_eq!(
        up,
        vec![
            n(0, 120, 60),
            n(240, 120, 64),
            n(480, 120, 67),
            n(720, 120, 60)
        ]
    );
    let down = bake(
        &[arp(|a| a.mode = ArpMode::Down)],
        None,
        &ev,
        &[],
        Ticks(960),
    );
    let p: Vec<u8> = down.iter().map(|x| x.pitch).collect();
    assert_eq!(p, [67, 64, 60, 67]);
    // Up-down over two octaves: C E G C' E' G' E' C' G E | C …, the ends played once.
    let ud = bake(
        &[arp(|a| {
            a.mode = ArpMode::UpDown;
            a.octaves = 2;
            a.rate = 15; // 1/32
        })],
        None,
        &ev,
        &[],
        Ticks(960),
    );
    let p: Vec<u8> = ud.iter().map(|x| x.pitch).collect();
    assert_eq!(p, [60, 64, 67, 72, 76, 79, 76, 72]);
    let du = bake(
        &[arp(|a| a.mode = ArpMode::DownUp)],
        None,
        &ev,
        &[],
        Ticks(960),
    );
    let p: Vec<u8> = du.iter().map(|x| x.pitch).collect();
    assert_eq!(p, [67, 64, 60, 64]);
}

#[test]
fn played_order_chord_mode_and_fixed_velocity() {
    // G, then C, then E: played in that order.
    let ev = events(&[(0, 960, 67), (10, 950, 60), (20, 940, 64)]);
    let played = bake(
        &[arp(|a| a.mode = ArpMode::Played)],
        None,
        &ev,
        &[],
        Ticks(960),
    );
    let p: Vec<u8> = played.iter().map(|x| x.pitch).collect();
    // The first step plays with the first note alone (it starts the arpeggio).
    assert_eq!(p, [67, 60, 64, 67]);
    let chord = bake(
        &[arp(|a| {
            a.mode = ArpMode::Chord;
            a.octaves = 2;
            a.rate = 9; // 1/8
            a.fixed_velocity = true;
            a.velocity = 64;
        })],
        None,
        &triad(),
        &[],
        Ticks(960),
    );
    let at: Vec<(i64, u8, u8)> = chord
        .iter()
        .map(|x| (x.start.0, x.pitch, x.velocity))
        .collect();
    assert_eq!(
        at,
        [
            (0, 60, 64),
            (0, 64, 64),
            (0, 67, 64),
            (480, 72, 64),
            (480, 76, 64),
            (480, 79, 64)
        ]
    );
}

#[test]
fn random_is_the_same_every_time_and_follows_the_seed() {
    let a = |seed: u32| {
        bake(
            &[arp(|a| {
                a.mode = ArpMode::Random;
                a.seed = seed;
                a.rate = 18;
            })],
            None,
            &triad(),
            &[],
            Ticks(960),
        )
    };
    assert_eq!(a(7), a(7));
    assert_ne!(a(7), a(8));
    assert_eq!(a(7).len(), 16);
}

#[test]
fn swing_delays_every_second_step_and_gate_sets_the_length() {
    let sw = bake(
        &[arp(|a| {
            a.swing = 0.75;
            a.gate = 1.0;
        })],
        None,
        &triad(),
        &[],
        Ticks(960),
    );
    let starts: Vec<i64> = sw.iter().map(|x| x.start.0).collect();
    // 1/16 at 75 %: the second of each pair a 16th's half … 240 late → 0, 360, 480, 840.
    assert_eq!(starts, [0, 360, 480, 840]);
    assert_eq!(sw[0].length, Ticks(240));
}

#[test]
fn hold_latches_until_a_new_chord() {
    // C E G for one 16th, then nothing for a beat, then D alone.
    let ev = events(&[(0, 100, 60), (0, 100, 64), (0, 100, 67), (1920, 100, 62)]);
    let held = bake(&[arp(|a| a.hold = true)], None, &ev, &[], Ticks(2400));
    let p: Vec<u8> = held.iter().map(|x| x.pitch).collect();
    // Eight steps of C E G (the keys long up), then D replaces them.
    assert_eq!(&p[..8], &[60, 64, 67, 60, 64, 67, 60, 64]);
    assert!(p[8..].iter().all(|x| *x == 62), "{p:?}");
    let free = bake(&[arp(|_| {})], None, &ev, &[], Ticks(2400));
    // Without hold the arpeggio stops with the keys: one step of the chord, one of D.
    assert_eq!(free.len(), 2);
}

#[test]
fn chords_build_on_each_note_and_release_what_they_built() {
    let c = fx(
        2,
        NoteFxKind::Chord(Chord {
            shape: ChordShape::Minor7,
            inversion: 1,
            ..Chord::default()
        }),
    );
    let out = bake(&[c], None, &events(&[(0, 480, 57)]), &[], Ticks(960));
    // Am7, first inversion: C E G A.
    let p: Vec<u8> = out.iter().map(|x| x.pitch).collect();
    assert_eq!(p, [60, 64, 67, 69]);
    assert!(out
        .iter()
        .all(|x| x.start == Ticks(0) && x.length == Ticks(480)));
}

#[test]
fn chords_from_the_key_and_a_scale_that_snaps_or_drops() {
    let key = Some(Key::new(0, ScaleKind::Major));
    let chord = fx(
        3,
        NoteFxKind::Chord(Chord {
            shape: ChordShape::ScaleTriad,
            ..Chord::default()
        }),
    );
    let ev = events(&[(0, 480, 62), (480, 480, 67)]);
    let out = bake(&[chord], key, &ev, &[], Ticks(960));
    let p: Vec<u8> = out.iter().map(|x| x.pitch).collect();
    // ii (D F A) then V (G B D).
    assert_eq!(p, [62, 65, 69, 67, 71, 74]);
    let snap = fx(4, NoteFxKind::Scale(ScaleFx::default()));
    let out = bake(&[snap], key, &events(&[(0, 480, 61)]), &[], Ticks(960));
    assert_eq!(out, vec![n(0, 480, 60)]);
    let drop = fx(
        5,
        NoteFxKind::Scale(ScaleFx {
            mode: ScaleMode::Drop,
            ..ScaleFx::default()
        }),
    );
    assert!(bake(
        std::slice::from_ref(&drop),
        key,
        &events(&[(0, 480, 61)]),
        &[],
        Ticks(960)
    )
    .is_empty());
    // No key anywhere: a scale lets every note through.
    assert_eq!(
        bake(&[drop], None, &events(&[(0, 480, 61)]), &[], Ticks(960)).len(),
        1
    );
}

#[test]
fn effects_chain_in_order_chord_into_arpeggio() {
    let chain = [
        fx(
            1,
            NoteFxKind::Chord(Chord {
                shape: ChordShape::Major,
                ..Chord::default()
            }),
        ),
        arp(|a| a.rate = 9),
    ];
    // Moved: the arpeggio's id must differ from the chord's.
    let chain = [
        chain[0].clone(),
        NoteEffect {
            id: NoteFxId(9),
            ..chain[1].clone()
        },
    ];
    let out = bake(&chain, None, &events(&[(0, 1920, 60)]), &[], Ticks(1920));
    let p: Vec<u8> = out.iter().map(|x| x.pitch).collect();
    assert_eq!(p, [60, 64, 67, 60]);
    // The other way round, the chord voices each arpeggio step.
    let rev = [chain[1].clone(), chain[0].clone()];
    let out = bake(&rev, None, &events(&[(0, 960, 60)]), &[], Ticks(960));
    assert_eq!(out.len(), 3 * 2);
}

#[test]
fn a_baked_clip_passes_the_chain() {
    let ev = triad();
    let direct = vec![true; ev.len()];
    let out = bake(&[arp(|_| {})], None, &ev, &direct, Ticks(960));
    assert_eq!(out, bake(&[], None, &ev, &[], Ticks(960)));
}

#[test]
fn the_chain_never_allocates_once_built() {
    // A smoke check of the shape: `configure` with a new chain, inputs and steps, a reset,
    // all with a chain built once (the engine's `rt_memory` counts it on the audio thread).
    let mut c = Chain::new();
    let chain = [arp(|_| {})];
    assert!(!c.configure(&chain, None));
    let now = Now {
        rolling: true,
        pos: 0.0,
        tps: 1.0,
    };
    c.input(now, Msg::on(60, 100, 1));
    c.run_due(now, |d| d.pos <= 0.0);
    assert_eq!(c.take().len(), 1);
    assert!(c.sounding());
    assert!(
        c.configure(&[], None),
        "a changed shape lets go of what sounded"
    );
    assert!(!c.is_active());
}

/// A live note into a stopped chain at engine sample `s`.
fn live(on: bool, pitch: u8) -> Msg {
    Msg {
        on,
        pitch,
        velocity: if on { 100 } else { 0 },
        release: 0,
        tag: u64::MAX,
        ordinal: 0,
        live: true,
    }
}

fn stopped(s: f64) -> Now {
    Now {
        rolling: false,
        pos: s,
        tps: 0.04,
    }
}

/// Run a stopped chain from sample `from` to `to`, firing what is due at each sample it
/// names: the ons it played.
fn ons_between(c: &mut Chain, from: f64, to: f64) -> usize {
    let mut ons = 0;
    let mut t = from;
    while t < to {
        c.run_due(stopped(t), |d| d.pos <= t);
        ons += c.take().iter().filter(|(_, m)| m.on).count();
        t = match c.next_due() {
            Some(d) if d.pos > t => d.pos.ceil(),
            _ => t + 1.0,
        };
    }
    ons
}

#[test]
fn switching_hold_off_lets_a_latched_arpeggio_go() {
    let mut c = Chain::new();
    let mut a = arp(|a| a.hold = true);
    c.configure(std::slice::from_ref(&a), None);
    c.input(stopped(0.0), live(true, 60));
    c.input(stopped(0.0), live(true, 64));
    c.input(stopped(10.0), live(false, 60));
    c.input(stopped(10.0), live(false, 64));
    // Latched: the keys are up and it plays on (a 16th is 6 000 samples at 0.04 tps).
    assert!(ons_between(&mut c, 0.0, 30_000.0) >= 4);
    // Hold off, the same shape: the params change in place, and the keys up are let go.
    if let NoteFxKind::Arp(x) = &mut a.kind {
        x.hold = false;
    }
    assert!(!c.configure(std::slice::from_ref(&a), None));
    assert_eq!(ons_between(&mut c, 30_000.0, 90_000.0), 0, "it stops");
    assert!(
        c.next_due().is_none_or(|d| d.pos < 32_000.0),
        "nothing left to step"
    );
}

#[test]
fn two_live_keys_snapped_to_one_note_are_two_voices() {
    let key = Some(Key::new(0, ScaleKind::Major));
    let mut c = Chain::new();
    c.configure(&[fx(4, NoteFxKind::Scale(ScaleFx::default()))], key);
    c.input(stopped(0.0), live(true, 60));
    c.input(stopped(0.0), live(true, 61)); // C#: snapped to C
    let ons: Vec<Msg> = c.take().iter().map(|(_, m)| *m).collect();
    assert_eq!(ons.len(), 2);
    assert!(ons.iter().all(|m| m.pitch == 60));
    assert_ne!(ons[0].tag, ons[1].tag, "two voices, not one");
    // Letting C# go ends its voice, not the held C.
    c.input(stopped(5.0), live(false, 61));
    let offs: Vec<Msg> = c.take().iter().map(|(_, m)| *m).collect();
    assert_eq!(offs.len(), 1);
    assert_eq!(offs[0].tag, ons[1].tag);
    // The same through chords from the key: D's triad and F's share A, each its own.
    let mut c = Chain::new();
    let triad = Chord {
        shape: ChordShape::ScaleTriad,
        ..Chord::default()
    };
    c.configure(&[fx(5, NoteFxKind::Chord(triad))], key);
    c.input(stopped(0.0), live(true, 62));
    c.input(stopped(0.0), live(true, 65));
    let a: Vec<u64> = c
        .take()
        .iter()
        .filter(|(_, m)| m.pitch == 69)
        .map(|(_, m)| m.tag)
        .collect();
    assert_eq!(a.len(), 2);
    assert_ne!(a[0], a[1]);
}

/// Round 31 (ADR 0068): a note's release velocity rides to the notes the chain makes of it —
/// each note of a chord, an arpeggio's steps once the key is up — and none given is none.
#[test]
fn a_notes_release_rides_to_the_notes_it_makes() {
    let released = |notes: &[(i64, i64, u8)], release: u8| {
        let mut ev = events(notes);
        for e in ev.iter_mut().filter(|e| !e.is_on()) {
            e.release = release;
        }
        ev
    };
    let major = fx(
        2,
        NoteFxKind::Chord(Chord {
            shape: ChordShape::Major,
            ..Chord::default()
        }),
    );
    let out = bake(
        std::slice::from_ref(&major),
        None,
        &released(&[(0, 480, 60)], 90),
        &[],
        Ticks(960),
    );
    assert_eq!(out.len(), 3);
    assert!(out.iter().all(|x| x.release == 90), "{out:?}");
    let none = bake(&[major], None, &events(&[(0, 480, 60)]), &[], Ticks(960));
    assert!(none.iter().all(|x| x.release == 0), "{none:?}");
    let held = bake(
        &[arp(|a| a.hold = true)],
        None,
        &released(&[(0, 240, 60)], 77),
        &[],
        Ticks(960),
    );
    assert!(
        held.iter()
            .any(|x| x.start >= Ticks(240) && x.release == 77),
        "{held:?}"
    );
}
