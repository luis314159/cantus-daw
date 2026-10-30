//! MPE in Standard MIDI Files (round 33, ADR 0080): an expressive track is written as a
//! lower zone (its MCM and bend range, a member channel per note, each note's curves as its
//! channel's bend, pressure and CC 74) and read back as one track with the same curves; a
//! controller's file with or without an MCM is recombined; key pressure is kept.

use cantus_core::model::note_expr::{ExprKind, ExprPoint, NoteExpr};
use cantus_core::*;
use cantus_project::midi::{self, smf, Event, ExportOptions, ImportOptions, TimedEvent};

fn st(x: f64) -> i64 {
    ExprKind::Bend.store(x)
}

fn u(x: f64) -> i64 {
    ExprKind::Pressure.store(x)
}

/// Two overlapping notes, each bent and pressed its own way, and a plain one after.
fn expressive() -> Project {
    let mut p = Project::new("mpe");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Seaboard");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(8));
    let mut a = p.make_note(Ticks::ZERO, Ticks::beats(2), 60, 100);
    a.expr = NoteExpr {
        bend: vec![
            ExprPoint(0, 0),
            ExprPoint(960, st(2.0)),
            ExprPoint(1920, st(-1.0)),
        ],
        pressure: vec![ExprPoint(0, u(0.2)), ExprPoint(960, u(0.9))],
        timbre: vec![ExprPoint(0, u(0.5))],
    }
    .boxed();
    let mut b = p.make_note(Ticks::beats(1), Ticks::beats(2), 64, 90);
    b.expr = NoteExpr {
        bend: vec![ExprPoint(0, st(-12.0)), ExprPoint(480, 0)],
        ..NoteExpr::default()
    }
    .boxed();
    c.notes
        .extend([a, b, p.make_note(Ticks::beats(4), Ticks::beats(1), 67, 80)]);
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn import(bytes: &[u8]) -> midi::MidiImport {
    midi::read(bytes, &ImportOptions::default()).unwrap()
}

#[test]
fn an_expressive_track_is_written_as_an_mpe_zone() {
    let bytes = midi::export(&expressive(), &ExportOptions::default()).unwrap();
    let file = smf::read(&bytes).unwrap();
    let track = &file.tracks[1];
    // The MCM first: RPN 6 on the master, 15 members.
    let ccs: Vec<(u8, u8, u8)> = track
        .iter()
        .filter_map(|e| match e.event {
            Event::Controller {
                channel,
                controller,
                value,
            } => Some((channel, controller, value)),
            _ => None,
        })
        .collect();
    assert_eq!(
        &ccs[..3],
        &[(0, 101, 0), (0, 100, 6), (0, 6, 2)],
        "two members: two notes sound at once"
    );
    assert!(ccs.contains(&(1, 6, 48)), "the members' bend range");
    // A member channel per note; the overlapping two differ.
    let ons: Vec<(u64, u8, u8)> = track
        .iter()
        .filter_map(|e| match e.event {
            Event::NoteOn {
                channel,
                key,
                velocity,
            } if velocity > 0 => Some((e.tick, channel, key)),
            _ => None,
        })
        .collect();
    assert_eq!(ons.len(), 3);
    assert!(ons.iter().all(|o| (1..=15).contains(&o.1)));
    assert_ne!(ons[0].1, ons[1].1);
    // The second note starts a full octave down: its bend is on its channel before it.
    let ch = ons[1].1;
    let i = track
        .iter()
        .position(|e| matches!(e.event, Event::NoteOn { channel, key: 64, .. } if channel == ch))
        .unwrap();
    let before = track[..i].iter().rev().find_map(|e| match e.event {
        Event::PitchBend { channel, value } if channel == ch => Some((e.tick, value)),
        _ => None,
    });
    assert_eq!(before, Some((ons[1].0, 8192 - 2048)), "−12 of ±48");
}

#[test]
fn an_mpe_file_reads_back_as_one_track_with_its_curves() {
    let p = expressive();
    let bytes = midi::export(&p, &ExportOptions::default()).unwrap();
    let back = import(&bytes);
    assert_eq!(back.tracks.len(), 1, "one track, not one per channel");
    let notes = &back.tracks[0].notes;
    assert_eq!(notes.len(), 3);
    let orig = &p.tracks[0].clips[0].notes;
    for (n, o) in notes.iter().zip(orig.iter()) {
        assert_eq!((n.start, n.length, n.pitch), (o.start, o.length, o.pitch));
        let (Some(a), Some(e)) = (o.expr.as_deref(), n.expr.as_deref()) else {
            assert!(o.expr.is_none() && n.expr.is_none(), "{n:?}");
            continue;
        };
        for kind in ExprKind::ALL {
            if a.curve(kind).is_empty() {
                continue;
            }
            for at in (0..o.length.0).step_by(40) {
                let want = a.value_at(kind, at).unwrap();
                // A curve at rest all through comes back as none.
                let got = e.value_at(kind, at).unwrap_or(kind.read(kind.rest()));
                // A 14-bit bend at ±48 is 0.006 semitone a step; 7 bits of pressure 1/127;
                // the curve between two of the engine's grid points is a line both ways.
                let tol = if kind == ExprKind::Bend { 0.05 } else { 0.02 };
                assert!(
                    (want - got).abs() <= tol,
                    "{kind:?} at {at}: {want} vs {got}"
                );
            }
        }
    }
}

/// A controller's own file: no MCM (guessed from the member channels), notes on channels
/// 2 and 3, bends at the default ±48; and a plain track whose key pressure is kept.
#[test]
fn a_controllers_file_is_recombined_and_key_pressure_kept() {
    let ev = |tick: u64, event: Event| TimedEvent { tick, event };
    let mpe = vec![
        ev(
            0,
            Event::PitchBend {
                channel: 1,
                value: 8192,
            },
        ),
        ev(
            0,
            Event::NoteOn {
                channel: 1,
                key: 60,
                velocity: 100,
            },
        ),
        ev(
            240,
            Event::PitchBend {
                channel: 1,
                value: 8192 + 4096,
            },
        ),
        ev(
            240,
            Event::ChannelPressure {
                channel: 1,
                value: 100,
            },
        ),
        ev(
            300,
            Event::NoteOn {
                channel: 2,
                key: 67,
                velocity: 90,
            },
        ),
        ev(
            320,
            Event::PitchBend {
                channel: 2,
                value: 9000,
            },
        ),
        ev(
            350,
            Event::ChannelPressure {
                channel: 3,
                value: 30,
            },
        ),
        ev(
            350,
            Event::NoteOn {
                channel: 3,
                key: 72,
                velocity: 90,
            },
        ),
        ev(
            560,
            Event::NoteOff {
                channel: 3,
                key: 72,
                velocity: 0,
            },
        ),
        ev(
            480,
            Event::NoteOff {
                channel: 1,
                key: 60,
                velocity: 0,
            },
        ),
        ev(
            600,
            Event::NoteOff {
                channel: 2,
                key: 67,
                velocity: 0,
            },
        ),
        // A fourth, centred at its note-on: a plain note of the zone.
        ev(
            600,
            Event::PitchBend {
                channel: 1,
                value: 8192,
            },
        ),
        ev(
            600,
            Event::NoteOn {
                channel: 1,
                key: 74,
                velocity: 90,
            },
        ),
        ev(
            700,
            Event::NoteOff {
                channel: 1,
                key: 74,
                velocity: 0,
            },
        ),
        ev(700, Event::EndOfTrack),
    ];
    let plain = vec![
        ev(
            0,
            Event::NoteOn {
                channel: 0,
                key: 50,
                velocity: 100,
            },
        ),
        ev(
            100,
            Event::PolyPressure {
                channel: 0,
                key: 50,
                value: 64,
            },
        ),
        ev(
            480,
            Event::NoteOff {
                channel: 0,
                key: 50,
                velocity: 0,
            },
        ),
        ev(480, Event::EndOfTrack),
    ];
    let file = smf::Smf {
        format: 1,
        division: smf::Division::Ppq(480),
        tracks: vec![vec![ev(0, Event::EndOfTrack)], mpe, plain],
    };
    let back = import(&smf::write(&file));
    assert_eq!(
        back.tracks.len(),
        2,
        "{:?}",
        back.tracks.iter().map(|t| &t.label).collect::<Vec<_>>()
    );
    let t = &back.tracks[0];
    assert_eq!(t.notes.len(), 4);
    let a = t.notes[0].expr.as_deref().expect("the first note's curves");
    // Half the throw at ±48: +24 semitones, from the note's second beat (480 Cantus ticks).
    assert_eq!(a.value_at(ExprKind::Bend, 0), Some(0.0));
    assert_eq!(a.value_at(ExprKind::Bend, 480), Some(24.0));
    assert!(a.value_at(ExprKind::Pressure, 480).unwrap() > 0.78);
    assert!(t.notes[3].expr.is_none(), "centred at its note-on: plain");
    let k = back.tracks[1].notes[0]
        .expr
        .as_deref()
        .expect("key pressure kept");
    assert!((k.value_at(ExprKind::Pressure, 200).unwrap() - 0.5).abs() < 1e-6);
}

fn ev(tick: u64, event: Event) -> TimedEvent {
    TimedEvent { tick, event }
}

fn on(tick: u64, channel: u8, key: u8) -> TimedEvent {
    ev(
        tick,
        Event::NoteOn {
            channel,
            key,
            velocity: 90,
        },
    )
}

fn off(tick: u64, channel: u8, key: u8) -> TimedEvent {
    ev(
        tick,
        Event::NoteOff {
            channel,
            key,
            velocity: 0,
        },
    )
}

/// Review fix 2: ordinary files are never read as MPE — a type-0 file with a piano on
/// channel 1, a bass on 2 and a bent lead on 3 (each with its program), and a GM file
/// with drums on 10 and two one-note-at-a-time parts, keep a track per part, no curves.
#[test]
fn ordinary_files_are_not_guessed_as_mpe() {
    let mut piano_bass_lead = vec![
        ev(
            0,
            Event::Program {
                channel: 0,
                program: 0,
            },
        ),
        ev(
            0,
            Event::Program {
                channel: 1,
                program: 33,
            },
        ),
        ev(
            0,
            Event::Program {
                channel: 2,
                program: 80,
            },
        ),
    ];
    for i in 0..8u64 {
        let t = i * 480;
        piano_bass_lead.extend([on(t, 0, 60), on(t, 0, 64), on(t, 1, 36), on(t, 2, 72)]);
        piano_bass_lead.push(ev(
            t + 100,
            Event::PitchBend {
                channel: 2,
                value: 10000,
            },
        ));
        piano_bass_lead.extend([
            off(t + 400, 0, 60),
            off(t + 400, 0, 64),
            off(t + 400, 1, 36),
            off(t + 400, 2, 72),
        ]);
    }
    piano_bass_lead.push(ev(4000, Event::EndOfTrack));
    let file = smf::Smf {
        format: 0,
        division: smf::Division::Ppq(480),
        tracks: vec![piano_bass_lead],
    };
    let back = import(&smf::write(&file));
    assert_eq!(back.tracks.len(), 3);
    assert!(back
        .tracks
        .iter()
        .all(|t| t.notes.iter().all(|n| n.expr.is_none())));
    // GM: drums on 10, a bass and a lead, the lead bent.
    let mut gm = Vec::new();
    for i in 0..8u64 {
        let t = i * 480;
        gm.extend([on(t, 9, 36), on(t, 9, 42), on(t, 1, 40), on(t, 2, 76)]);
        gm.push(ev(
            t + 50,
            Event::PitchBend {
                channel: 2,
                value: 6000,
            },
        ));
        gm.push(ev(
            t + 50,
            Event::PitchBend {
                channel: 1,
                value: 9000,
            },
        ));
        gm.extend([
            off(t + 200, 9, 36),
            off(t + 200, 9, 42),
            off(t + 400, 1, 40),
            off(t + 400, 2, 76),
        ]);
    }
    gm.push(ev(4000, Event::EndOfTrack));
    let file = smf::Smf {
        format: 0,
        division: smf::Division::Ppq(480),
        tracks: vec![gm],
    };
    let back = import(&smf::write(&file));
    assert_eq!(back.tracks.len(), 3, "drums, bass, lead");
    assert!(back
        .tracks
        .iter()
        .all(|t| t.notes.iter().all(|n| n.expr.is_none())));
}

/// Review fix 3: beside a Firmus track and a kit, an expressive track is a lower zone that
/// avoids channel 10 and the other tracks' channels; its lanes are on its manager channel;
/// no other track's channel gets the members' bend range.
#[test]
fn an_mpe_zone_leaves_the_other_tracks_channels_alone() {
    let mut p = Project::new("three");
    p.tempo_bpm = 120.0;
    let mut a = p.make_track("Firmus");
    let mut ca = p.make_clip("a", Ticks::ZERO, Ticks::beats(4));
    ca.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(1), 48, 90));
    a.clips.push(ca);
    let mut d = p.make_track("Kit");
    d.instrument = InstrumentSpec::Pulsus(PulsusParams::default());
    let mut cd = p.make_clip("d", Ticks::ZERO, Ticks::beats(4));
    cd.notes
        .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 36, 90));
    d.clips.push(cd);
    p.tracks.extend([a, d]);
    let mut e = expressive();
    let mut lead = e.tracks.remove(0);
    lead.clips[0].lanes = vec![cantus_core::ClipLane::new(
        cantus_core::LaneKind::PitchBend,
        vec![cantus_core::AutoPoint::new(Ticks::ZERO, 0.7)],
    )];
    // Twelve notes in a row so the rotation goes round its members.
    for i in 0..12 {
        let n = p.make_note(Ticks(i * 240), Ticks(200), 50 + i as u8, 80);
        lead.clips[0].notes.push(n);
    }
    p.tracks.push(lead);
    let file = smf::read(&midi::export(&p, &ExportOptions::default()).unwrap()).unwrap();
    let chans = |t: &Vec<TimedEvent>, notes: bool| -> Vec<u8> {
        t.iter()
            .filter(|e| notes == matches!(e.event, Event::NoteOn { .. }))
            .filter_map(|e| e.event.channel())
            .collect()
    };
    let (firmus, kit, mpe) = (&file.tracks[1], &file.tracks[2], &file.tracks[3]);
    let others: Vec<u8> = chans(firmus, true)
        .into_iter()
        .chain(chans(kit, true))
        .collect();
    assert!(others.contains(&9), "the kit on channel 10");
    let lead_notes = chans(mpe, true);
    assert!(
        !lead_notes.contains(&9),
        "no member note on channel 10: {lead_notes:?}"
    );
    assert!(
        lead_notes.iter().all(|c| !others.contains(c)),
        "{lead_notes:?} vs {others:?}"
    );
    // The bend lane on the manager channel; the bend range only on the zone's members.
    assert!(mpe
        .iter()
        .any(|e| matches!(e.event, Event::PitchBend { channel: 0, .. })));
    let ranged: Vec<u8> = mpe
        .windows(3)
        .filter_map(|w| match (&w[0].event, &w[2].event) {
            (
                Event::Controller {
                    controller: 101,
                    value: 0,
                    ..
                },
                Event::Controller {
                    channel,
                    controller: 6,
                    value: 48,
                },
            ) => Some(*channel),
            _ => None,
        })
        .collect();
    assert!(
        !ranged.is_empty() && ranged.iter().all(|c| !others.contains(c) && *c != 9),
        "{ranged:?}"
    );
}

/// Review fix 4: a member channel used again is put back to rest — sixteen notes in a row,
/// the first bent up a tone, the rest plain: none of the others comes back bent.
#[test]
fn a_reused_channel_starts_at_rest() {
    let mut p = Project::new("reuse");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("t");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(16));
    for i in 0..16 {
        let mut n = p.make_note(Ticks::beats(i), Ticks::QUARTER, 60, 90);
        if i == 0 {
            n.expr = NoteExpr {
                bend: vec![ExprPoint(0, 0), ExprPoint(480, st(2.0))],
                ..NoteExpr::default()
            }
            .boxed();
        }
        c.notes.push(n);
    }
    t.clips.push(c);
    p.tracks.push(t);
    let back = import(&midi::export(&p, &ExportOptions::default()).unwrap());
    let notes = &back.tracks[0].notes;
    assert_eq!(notes.len(), 16);
    assert!(notes[0].expr.is_some());
    for (i, n) in notes.iter().enumerate().skip(1) {
        assert!(n.expr.is_none(), "note {i}: {:?}", n.expr);
    }
}

/// Review fix 6: a long played note keeps its whole curve — 20 s, 10 000 bends rising —
/// thinned under the point limit, its last value the file's last.
#[test]
fn a_long_played_note_keeps_its_whole_curve() {
    let mut evs = vec![
        ev(
            0,
            Event::Controller {
                channel: 0,
                controller: 101,
                value: 0,
            },
        ),
        ev(
            0,
            Event::Controller {
                channel: 0,
                controller: 100,
                value: 6,
            },
        ),
        ev(
            0,
            Event::Controller {
                channel: 0,
                controller: 6,
                value: 15,
            },
        ),
        on(0, 1, 60),
    ];
    // 20 s at 120 BPM is 40 beats: 19 200 ticks at 480.
    for i in 0..10_000u64 {
        let v = 8192 + (i * 8191 / 9_999) as u16;
        evs.push(ev(
            1 + i * 19_198 / 9_999,
            Event::PitchBend {
                channel: 1,
                value: v.min(16383),
            },
        ));
    }
    evs.extend([off(19_200, 1, 60), ev(19_200, Event::EndOfTrack)]);
    let file = smf::Smf {
        format: 0,
        division: smf::Division::Ppq(480),
        tracks: vec![evs],
    };
    let back = import(&smf::write(&file));
    let n = &back.tracks[0].notes[0];
    let e = n.expr.as_deref().unwrap();
    assert!(
        e.bend.len() <= cantus_core::model::note_expr::MAX_POINTS,
        "{}",
        e.bend.len()
    );
    let want = cantus_core::model::note_expr::midi2::bend_of_14(16383, 48);
    assert_eq!(
        e.bend.last().unwrap().1,
        want,
        "the end of the curve is there"
    );
}

/// One SMF track as a file of its own (type 0).
fn file_of(track: Vec<TimedEvent>) -> Vec<u8> {
    smf::write(&smf::Smf {
        format: 0,
        division: smf::Division::Ppq(480),
        tracks: vec![track],
    })
}

/// A note on `channel` with its own bend at its note-on (and one while it sounds) — how a
/// controller hands each note a channel.
fn played(t: u64, channel: u8, key: u8, len: u64, bend: u16) -> Vec<TimedEvent> {
    vec![
        ev(
            t,
            Event::PitchBend {
                channel,
                value: 8192,
            },
        ),
        on(t, channel, key),
        ev(
            t + len / 2,
            Event::PitchBend {
                channel,
                value: bend,
            },
        ),
        off(t + len, channel, key),
    ]
}

fn sorted(mut v: Vec<TimedEvent>) -> Vec<TimedEvent> {
    v.sort_by_key(|e| e.tick);
    let end = v.last().map_or(0, |e| e.tick);
    v.push(ev(end, Event::EndOfTrack));
    v
}

/// Review D6: MPE without an MCM is told by its notes' own setup, wherever the rotation
/// starts and whatever it skips, and a one-note-at-a-time line is MPE too; three parts that
/// bend now and then are not.
#[test]
fn mpe_is_told_by_each_notes_setup_not_by_how_many_sound() {
    // A staccato lead rotating over channels 2–8, one note at a time, every note bent.
    let mut line = Vec::new();
    for i in 0..14u64 {
        line.extend(played(
            i * 240,
            1 + (i % 7) as u8,
            60 + (i % 5) as u8,
            200,
            9000,
        ));
    }
    let back = import(&file_of(sorted(line)));
    assert_eq!(back.tracks.len(), 1, "one line, not seven tracks");
    assert!(back.tracks[0].notes.iter().all(|n| n.expr.is_some()));
    // A rotation that starts on channel 4.
    let mut high = Vec::new();
    for i in 0..8u64 {
        high.extend(played(i * 200, 3 + (i % 3) as u8, 64, 300, 10000));
    }
    assert_eq!(import(&file_of(sorted(high))).tracks.len(), 1);
    // A LinnStrument's channel per row: channels 2, 4 and 6, overlapping.
    let mut rows = Vec::new();
    for i in 0..9u64 {
        rows.extend(played(
            i * 150,
            [1, 3, 5][(i % 3) as usize],
            50 + i as u8,
            400,
            7000,
        ));
    }
    assert_eq!(import(&file_of(sorted(rows))).tracks.len(), 1);
    // Three parts on channels 2–4, no programs, two of them bent now and then, overlapping.
    let mut parts = Vec::new();
    for i in 0..8u64 {
        let t = i * 480;
        for (c, k) in [(1u8, 40u8), (2, 64), (3, 72)] {
            parts.extend([on(t, c, k), off(t + 400, c, k)]);
        }
        if i % 3 == 1 {
            parts.push(ev(
                t + 200,
                Event::PitchBend {
                    channel: 2,
                    value: 10000,
                },
            ));
            parts.push(ev(
                t + 220,
                Event::PitchBend {
                    channel: 3,
                    value: 6000,
                },
            ));
        }
    }
    let back = import(&file_of(sorted(parts)));
    assert_eq!(back.tracks.len(), 3);
    assert!(back
        .tracks
        .iter()
        .all(|t| t.notes.iter().all(|n| n.expr.is_none())));
}

/// An expressive track: `keys` sounding together, each bent its own way.
fn chord(p: &mut Project, name: &str, keys: &[u8]) -> Track {
    let mut t = p.make_track(name);
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    for (i, k) in keys.iter().enumerate() {
        let mut n = p.make_note(Ticks::ZERO, Ticks::beats(2), *k, 90);
        n.expr = NoteExpr {
            bend: vec![ExprPoint(0, st(i as f64 + 1.0))],
            ..NoteExpr::default()
        }
        .boxed();
        c.notes.push(n);
    }
    t.clips.push(c);
    t
}

/// Review D4: two expressive tracks, the second a six-note chord, each note bent its own way:
/// zones sized to what sounds at once, so every note comes back with its own bend.
#[test]
fn each_chord_note_keeps_its_own_bend_through_a_file() {
    let mut p = Project::new("chords");
    p.tempo_bpm = 120.0;
    let a = chord(&mut p, "Pad", &[48, 55]);
    let b = chord(&mut p, "Keys", &[60, 62, 64, 65, 67, 69]);
    p.tracks.extend([a, b]);
    let back = import(&midi::export(&p, &ExportOptions::default()).unwrap());
    assert_eq!(back.tracks.len(), 2);
    for (orig, got) in p.tracks.iter().zip(&back.tracks) {
        for o in &orig.clips[0].notes {
            let n = got
                .notes
                .iter()
                .find(|n| n.pitch == o.pitch)
                .expect("the note");
            assert_eq!(n.length, o.length, "{}: not cut", o.pitch);
            let want = o
                .expr
                .as_deref()
                .unwrap()
                .value_at(ExprKind::Bend, 0)
                .unwrap();
            let have = n
                .expr
                .as_deref()
                .map(|e| e.value_at(ExprKind::Bend, 0).unwrap());
            assert!(
                have.is_some_and(|h| (h - want).abs() < 0.01),
                "{}: {have:?} for {want}",
                o.pitch
            );
        }
    }
}

/// Review D5: two expressive tracks and a plain one — the plain track gets a channel of its
/// own, never a zone's member or manager.
#[test]
fn a_plain_track_beside_two_zones_has_a_channel_of_its_own() {
    let mut p = Project::new("three");
    p.tempo_bpm = 120.0;
    let a = chord(&mut p, "Lead", &[60, 64, 67, 71, 74, 77, 79, 83]);
    let b = chord(&mut p, "Pad", &[48, 52, 55, 59, 62]);
    let mut piano = p.make_track("Piano");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(1), 36, 90));
    piano.clips.push(c);
    p.tracks.extend([a, b, piano]);
    let file = smf::read(&midi::export(&p, &ExportOptions::default()).unwrap()).unwrap();
    let zone_channels: Vec<u8> = file.tracks[1..3]
        .iter()
        .flat_map(|t| t.iter().filter_map(|e| e.event.channel()))
        .collect();
    let plain: Vec<u8> = file.tracks[3]
        .iter()
        .filter_map(|e| e.event.channel())
        .collect();
    assert!(!plain.is_empty());
    assert!(
        plain.iter().all(|c| !zone_channels.contains(c)),
        "{plain:?} in {zone_channels:?}"
    );
}

/// Review D-nit: a bend stamped on the very tick a note lets go of its channel is that
/// note's last, not the next note's first.
#[test]
fn a_value_on_the_release_tick_stays_with_the_note_released() {
    let evs = vec![
        ev(
            0,
            Event::Controller {
                channel: 0,
                controller: 101,
                value: 0,
            },
        ),
        ev(
            0,
            Event::Controller {
                channel: 0,
                controller: 100,
                value: 6,
            },
        ),
        ev(
            0,
            Event::Controller {
                channel: 0,
                controller: 6,
                value: 15,
            },
        ),
        on(0, 1, 60),
        ev(
            480,
            Event::PitchBend {
                channel: 1,
                value: 12000,
            },
        ),
        off(480, 1, 60),
        on(600, 1, 62),
        off(900, 1, 62),
        ev(900, Event::EndOfTrack),
    ];
    let back = import(&file_of(evs));
    let notes = &back.tracks[0].notes;
    assert_eq!(notes.len(), 2);
    assert!(notes[1].expr.is_none(), "{:?}", notes[1].expr);
}
