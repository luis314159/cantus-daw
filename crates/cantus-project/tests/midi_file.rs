//! Standard MIDI Files, out and back in (ADR 0021): notes survive exactly at every PPQ
//! that can hold them, a hand-built type 0 file with running status is split by channel,
//! the tempo map is honoured, and what is written is well formed to the byte.

use cantus_core::*;
use cantus_project::midi::{self, smf, Event, ExportOptions, ImportOptions, Placement, Scope};

/// A project with one Firmus track of notes at awkward positions and lengths, a muted
/// note, a muted clip, and a kit track with a pad off the GM map.
fn awkward_project(positions: &[(i64, i64)]) -> Project {
    let mut p = Project::new("Awkward");
    p.tempo_bpm = 113.5;
    p.time_signature = TimeSignature::new(7, 8);
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(64));
    for (i, (start, len)) in positions.iter().enumerate() {
        let n = p.make_note(
            Ticks(*start),
            Ticks(*len),
            40 + (i % 40) as u8,
            1 + (i * 7 % 127) as u8,
        );
        c.notes.push(n);
    }
    let mut muted = p.make_note(Ticks(5), Ticks(100), 100, 90);
    muted.muted = true;
    c.notes.push(muted);
    t.clips.push(c);
    let mut silent = p.make_clip("Muted clip", Ticks::beats(80), Ticks::beats(4));
    silent.muted = true;
    silent
        .notes
        .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 72, 100));
    t.clips.push(silent);
    p.tracks.push(t);
    p
}

/// What the engine would play, without the note ids: `(tick, on, pitch, velocity)`.
fn played(p: &Project) -> Vec<Vec<(i64, bool, u8, u8)>> {
    Snapshot::compile(p, 0)
        .tracks
        .iter()
        .map(|t| {
            let mut v: Vec<_> = t
                .events
                .iter()
                .map(|e| (e.tick.0, e.is_on(), e.pitch, e.velocity))
                .collect();
            v.sort();
            v
        })
        .collect()
}

/// Import bytes into an empty project the way the interface does, as one batch.
fn import_into_empty(bytes: &[u8], opts: &ImportOptions, apply_tempo: bool) -> Project {
    let import = midi::read(bytes, opts).expect("reads");
    let mut bus = CommandBus::new(Project::new("Imported"));
    let mut next = bus.project().next_id;
    let built = {
        let mut ids = || {
            let id = next;
            next += 1;
            id
        };
        midi::import_commands(
            bus.project(),
            &import,
            &Placement {
                apply_tempo,
                ..Default::default()
            },
            &mut ids,
            &|l| l.english(),
        )
    };
    bus.reserve_ids(next);
    bus.execute(
        Command::Batch {
            label_key: "cmd.import_midi".into(),
            commands: built.commands,
        },
        Origin::User,
    )
    .expect("the import applies");
    bus.project().clone()
}

/// A grid of odd positions and odd lengths: primes and near-primes, none on any beat.
fn odd_grid() -> Vec<(i64, i64)> {
    (0..120)
        .map(|i| {
            let start = 1 + i * 97 + (i * i) % 13;
            let len = 1 + (i * 37) % 1901;
            (start, len)
        })
        .collect()
}

#[test]
fn export_then_import_round_trips_every_note_at_every_ppq_that_can_hold_it() {
    let p = awkward_project(&odd_grid());
    let want = played(&p);
    assert!(!want[0].is_empty());
    // 960 is Cantus's own resolution; the others are multiples of it, so every tick is on
    // the file's grid.
    for ppq in [960u16, 1920, 3840, 7680, 9600] {
        let bytes = midi::export(
            &p,
            &ExportOptions {
                scope: Scope::Song,
                ppq,
            },
        )
        .unwrap();
        let back = import_into_empty(&bytes, &ImportOptions::default(), true);
        assert_eq!(played(&back), want, "PPQ {ppq}");
        assert_eq!(back.tempo_bpm, 113.5, "PPQ {ppq}");
        assert_eq!(back.time_signature, TimeSignature::new(7, 8), "PPQ {ppq}");
    }
}

#[test]
fn coarser_files_hold_their_own_grid_exactly_and_round_everything_else_to_it() {
    for ppq in [96u16, 120, 192, 240, 480] {
        let step = (PPQ / ppq as i64).max(1);
        // Odd multiples of the file's tick: on its grid, off every beat.
        let on_grid: Vec<(i64, i64)> = (0..80)
            .map(|i| ((1 + 2 * i * 7) * step, (1 + 2 * (i % 9)) * step))
            .collect();
        let p = awkward_project(&on_grid);
        let bytes = midi::export(
            &p,
            &ExportOptions {
                scope: Scope::Song,
                ppq,
            },
        )
        .unwrap();
        let back = import_into_empty(&bytes, &ImportOptions::default(), true);
        assert_eq!(played(&back), played(&p), "on-grid notes at PPQ {ppq}");

        // Off the grid: each position within half a file tick, and no note lost.
        let p = awkward_project(&odd_grid());
        let bytes = midi::export(
            &p,
            &ExportOptions {
                scope: Scope::Song,
                ppq,
            },
        )
        .unwrap();
        let back = import_into_empty(&bytes, &ImportOptions::default(), true);
        let (a, b) = (played(&p), played(&back));
        assert_eq!(a[0].len(), b[0].len(), "no note lost at PPQ {ppq}");
        let ons = |v: &Vec<(i64, bool, u8, u8)>| {
            let mut o: Vec<(u8, i64)> = v.iter().filter(|e| e.1).map(|e| (e.2, e.0)).collect();
            o.sort();
            o
        };
        for ((pa, ta), (pb, tb)) in ons(&a[0]).iter().zip(ons(&b[0]).iter()) {
            assert_eq!(pa, pb);
            assert!(
                (ta - tb).abs() * 2 <= step,
                "PPQ {ppq}: {ta} came back as {tb}, more than half a file tick away"
            );
        }
    }
}

#[test]
fn muted_notes_and_clips_are_not_written_and_every_track_ends_properly() {
    let mut p = awkward_project(&[(0, 480), (480, 480)]);
    let mut drums = p.make_track("Kit");
    // The shipped kit with its kick moved off the GM map, to C#6.
    let mut kit = PulsusParams::default();
    kit.pads.retain(|pad| pad.pitch != 36);
    kit.pads
        .push(DrumPad::new(90, "Stray kick", PadVoice::Kick));
    drums.instrument = InstrumentSpec::Pulsus(PulsusParams::of(kit.pads));
    let mut c = p.make_clip("Beat", Ticks::ZERO, Ticks::beats(4));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::EIGHTH, 90, 110));
    c.notes
        .push(p.make_note(Ticks::QUARTER, Ticks::EIGHTH, 38, 100));
    drums.clips.push(c);
    p.tracks.push(drums);

    let bytes = midi::export(&p, &ExportOptions::default()).unwrap();
    // The chunks, read by hand rather than by the reader under test.
    assert_eq!(&bytes[0..4], b"MThd");
    assert_eq!(u32::from_be_bytes(bytes[4..8].try_into().unwrap()), 6);
    assert_eq!(u16::from_be_bytes([bytes[8], bytes[9]]), 1, "type 1");
    assert_eq!(
        u16::from_be_bytes([bytes[10], bytes[11]]),
        3,
        "conductor + 2"
    );
    assert_eq!(u16::from_be_bytes([bytes[12], bytes[13]]), 960);
    let mut at = 14;
    let mut chunks = 0;
    while at < bytes.len() {
        assert_eq!(&bytes[at..at + 4], b"MTrk", "chunk {chunks}");
        let len = u32::from_be_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &bytes[at + 8..at + 8 + len];
        assert_eq!(
            &body[len - 3..],
            &[0xFF, 0x2F, 0x00],
            "chunk {chunks} ends with EOT"
        );
        at += 8 + len;
        chunks += 1;
    }
    assert_eq!(at, bytes.len(), "the lengths add up to the file");
    assert_eq!(chunks, 3);

    let file = smf::read(&bytes).unwrap();
    let song_end = to_file(p.content_end());
    for (i, track) in file.tracks.iter().enumerate() {
        let last = track.last().unwrap();
        assert_eq!(last.event, Event::EndOfTrack);
        assert_eq!(last.tick, song_end, "track {i} ends where the song does");
        assert_eq!(
            track
                .iter()
                .filter(|e| e.event == Event::EndOfTrack)
                .count(),
            1
        );
    }
    let notes = |t: usize| -> Vec<(u8, u8)> {
        file.tracks[t]
            .iter()
            .filter_map(|e| match e.event {
                Event::NoteOn {
                    channel,
                    key,
                    velocity,
                    ..
                } if velocity > 0 => Some((channel, key)),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        notes(1).len(),
        2,
        "the muted note and the muted clip are not there"
    );
    assert!(notes(1).iter().all(|(ch, _)| *ch == 0));
    assert!(!notes(1).iter().any(|(_, k)| *k == 100 || *k == 72));
    // The kit is on channel 10, and the stray kick is written on GM 36.
    assert_eq!(notes(2), vec![(9, 36), (9, 38)]);
    assert_eq!(
        file.tracks[2][0].event,
        Event::TrackName("Kit".into()),
        "tracks carry their names"
    );
    assert_eq!(file.tracks[0][0].event, Event::TrackName("Awkward".into()));
    assert!(file.tracks[0].contains(&smf::TimedEvent {
        tick: 0,
        event: Event::Tempo(528_634)
    }));
    assert!(file.tracks[0].contains(&smf::TimedEvent {
        tick: 0,
        event: Event::TimeSignature {
            numerator: 7,
            denominator_pow: 3,
            clocks: 12,
            n32: 8
        }
    }));
}

fn to_file(t: Ticks) -> u64 {
    midi::to_file_ticks(t, 960)
}

#[test]
fn a_clip_exports_from_its_own_start_and_plays_what_its_window_shows() {
    let mut p = Project::new("Clip");
    let mut t = p.make_track("Keys");
    // A window from beat 8, slipped by a beat, repeating every two beats, three bars long.
    let mut c = p.make_clip("Loop", Ticks::beats(8), Ticks::beats(12));
    c.offset = Ticks::beats(1);
    c.content_length = Some(Ticks::beats(2));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100)); // before the window
    c.notes
        .push(p.make_note(Ticks::beats(1), Ticks::EIGHTH, 62, 90));
    c.notes
        .push(p.make_note(Ticks::beats(2) + Ticks(7), Ticks::beats(3), 64, 80));
    let (tid, cid) = (t.id, c.id);
    t.clips.push(c);
    p.tracks.push(t);
    let bytes = midi::export(
        &p,
        &ExportOptions {
            scope: Scope::Clip {
                track: tid,
                clip: cid,
            },
            ppq: 960,
        },
    )
    .unwrap();
    let back = import_into_empty(&bytes, &ImportOptions::default(), false);
    // The same notes as the arrangement plays, a bar-and-a-half earlier.
    let shift = Ticks::beats(8).0;
    let want: Vec<_> = played(&p)[0]
        .iter()
        .map(|(t, on, k, v)| (t - shift, *on, *k, *v))
        .collect();
    assert_eq!(played(&back)[0], want);
    assert_eq!(
        want.iter().filter(|e| e.1).count(),
        12,
        "six repeats of two notes"
    );
    let file = smf::read(&bytes).unwrap();
    assert_eq!(
        file.tracks[1].last().unwrap().tick,
        to_file(Ticks::beats(12))
    );
}

/// Written by hand, byte by byte: type 0, 96 PPQ, 100 BPM, 3/4, a fingered bass on
/// channel 1 in **running status** with note-on velocity 0 as its note-offs, a kit on
/// channel 10 with a crash the shipped kit does not have, a controller and a pitch bend.
fn hand_built_type_0() -> Vec<u8> {
    let track: Vec<u8> = vec![
        0x00, 0xFF, 0x03, 0x04, b'D', b'e', b'm', b'o', // sequence name
        0x00, 0xFF, 0x51, 0x03, 0x09, 0x27, 0xC0, // 600 000 µs = 100 BPM
        0x00, 0xFF, 0x58, 0x04, 0x03, 0x02, 0x18, 0x08, // 3/4
        0x00, 0xC0, 0x21, // program 33, fingered bass
        0x00, 0xB0, 0x07, 0x64, // volume: a controller, reported
        0x00, 0x90, 0x28, 0x50, // E1 on
        0x30, 0x28, 0x00, // running status: E1 off (velocity 0) at 48
        0x00, 0x2B, 0x60, // running status: G1 on
        0x00, 0x99, 0x24, 0x70, // kick on channel 10
        0x00, 0x31, 0x68, // running status: crash (GM 49) on
        0x18, 0x89, 0x24, 0x40, // kick off at 72 (a real note-off)
        0x00, 0x31, 0x40, // running status: crash off
        0x18, 0xE0, 0x00, 0x50, // pitch bend on channel 1, reported
        0x00, 0x90, 0x2B, 0x00, // G1 off at 96
        0x00, 0xFF, 0x2F, 0x00,
    ];
    let mut bytes = b"MThd".to_vec();
    bytes.extend_from_slice(&6u32.to_be_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 1, 0, 96]);
    bytes.extend_from_slice(b"MTrk");
    bytes.extend_from_slice(&(track.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&track);
    bytes
}

#[test]
fn a_hand_built_type_0_file_is_split_by_channel_into_a_bass_and_a_kit() {
    let bytes = hand_built_type_0();
    let import = midi::read(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(import.format, 0);
    assert_eq!(import.tempo_bpm, 100.0);
    assert_eq!(import.time_signature, Some(TimeSignature::new(3, 4)));
    assert_eq!(import.name.as_deref(), Some("Demo"));
    assert_eq!(import.tracks.len(), 2);
    let bass = &import.tracks[0];
    assert_eq!(bass.label, midi::TrackLabel::Program("Fingered Bass"));
    assert!(!bass.drums);
    let notes: Vec<_> = bass
        .notes
        .iter()
        .map(|n| (n.start.0, n.length.0, n.pitch, n.velocity))
        .collect();
    // 96 PPQ → 960: every file tick is ten of ours.
    assert_eq!(notes, vec![(0, 480, 40, 0x50), (480, 480, 43, 0x60)]);
    let kit = &import.tracks[1];
    assert!(kit.drums);
    assert_eq!(kit.label, midi::TrackLabel::Drums);
    assert_eq!(kit.notes.len(), 2);
    assert!(import.report.has("interop.report.controllers"));
    assert!(import.report.has("interop.report.pitch_bends"));
    assert!(import.report.has("interop.report.programs"));

    let p = import_into_empty(&bytes, &ImportOptions::default(), true);
    assert_eq!(p.tempo_bpm, 100.0);
    assert_eq!(p.time_signature, TimeSignature::new(3, 4));
    assert_eq!(p.tracks.len(), 2);
    assert_eq!(p.tracks[0].name, "Fingered Bass");
    assert_eq!(p.tracks[1].name, "Drums");
    let kit = p.tracks[1]
        .instrument
        .pulsus()
        .expect("channel 10 is a kit");
    let crash = kit
        .pads
        .iter()
        .find(|pad| pad.pitch == 49)
        .expect("a crash pad");
    assert_eq!(crash.name, "Crash cymbal 1");
    assert_eq!(crash.voice, PadVoice::HiHat);
    // Each track feeds a strip of its own, and each clip spans whole bars of 3/4.
    assert_ne!(p.tracks[0].strip, p.tracks[1].strip);
    assert!(p.tracks.iter().all(|t| t.strip.is_some()));
    let clip = &p.tracks[0].clips[0];
    assert_eq!((clip.start, clip.length), (Ticks::ZERO, Ticks::beats(3)));
}

/// 120 BPM for a bar, then 60: a note on beat 6 is 4 s in.
fn tempo_map_file() -> Vec<u8> {
    let ppq = 480u64;
    let smf = smf::Smf {
        format: 1,
        division: smf::Division::Ppq(ppq as u16),
        tracks: vec![
            vec![
                smf::TimedEvent {
                    tick: 0,
                    event: Event::Tempo(500_000),
                },
                smf::TimedEvent {
                    tick: 4 * ppq,
                    event: Event::Tempo(1_000_000),
                },
            ],
            vec![
                smf::TimedEvent {
                    tick: 6 * ppq,
                    event: Event::NoteOn {
                        channel: 0,
                        key: 60,
                        velocity: 100,
                    },
                },
                smf::TimedEvent {
                    tick: 7 * ppq,
                    event: Event::NoteOff {
                        channel: 0,
                        key: 60,
                        velocity: 0,
                    },
                },
            ],
        ],
    };
    smf::write(&smf)
}

#[test]
fn the_tempo_map_is_honoured_by_beats_or_by_seconds_and_never_in_silence() {
    let bytes = tempo_map_file();
    // By beats: the note stays on beat 6 and the report says a change was not kept.
    let by_beats = midi::read(
        &bytes,
        &ImportOptions {
            tempo_mode: midi::TempoMode::Beats,
            target_bpm: None,
        },
    )
    .unwrap();
    assert_eq!(by_beats.tempo_bpm, 120.0);
    let n = by_beats.tracks[0].notes[0];
    assert_eq!((n.start, n.length), (Ticks::beats(6), Ticks::beats(1)));
    let item = by_beats
        .report
        .get("interop.report.tempo_changes")
        .expect("the change is reported");
    assert_eq!(item.arg("n"), Some("1"));
    assert!(!by_beats.report.is_lossless());

    // By seconds, at 120: the note sounds 4 s in, which is beat 8, and lasts 1 s.
    let by_time = midi::read(
        &bytes,
        &ImportOptions {
            tempo_mode: midi::TempoMode::Seconds,
            target_bpm: Some(120.0),
        },
    )
    .unwrap();
    let n = by_time.tracks[0].notes[0];
    assert_eq!((n.start, n.length), (Ticks::beats(8), Ticks::beats(2)));
    assert!(by_time.report.has("interop.report.tempo_retimed"));
    // And at 90 the same four seconds are six beats.
    let at_90 = midi::read(
        &bytes,
        &ImportOptions {
            tempo_mode: midi::TempoMode::Seconds,
            target_bpm: Some(90.0),
        },
    )
    .unwrap();
    assert_eq!(at_90.tracks[0].notes[0].start, Ticks::beats(6));

    // Export writes the tempo the way the standard counts it.
    let mut p = Project::new("Tempo");
    p.tempo_bpm = 140.0;
    let bytes = midi::export(&p, &ExportOptions::default()).unwrap();
    let back = midi::read(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(back.tempo_bpm, 140.0, "428 571 µs reads back as 140");
}

#[test]
fn an_import_is_one_step_of_the_undo_history() {
    let mut bus = CommandBus::new(Project::default());
    let before = bus.project().clone();
    let import = midi::read(&hand_built_type_0(), &ImportOptions::default()).unwrap();
    let mut next = bus.project().next_id;
    let built = {
        let mut ids = || {
            let id = next;
            next += 1;
            id
        };
        midi::import_commands(
            bus.project(),
            &import,
            &Placement {
                at: Ticks::beats(8),
                apply_tempo: true,
                ..Default::default()
            },
            &mut ids,
            &|l| l.english(),
        )
    };
    bus.reserve_ids(next);
    bus.execute(
        Command::Batch {
            label_key: "cmd.import_midi".into(),
            commands: built.commands,
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(bus.project().tracks.len(), 2);
    assert_eq!(bus.project().tempo_bpm, 100.0);
    assert_eq!(bus.project().tracks[0].clips[0].start, Ticks::beats(8));
    bus.undo();
    let mut after = bus.project().clone();
    after.next_id = before.next_id;
    assert_eq!(after, before, "one undo takes the whole import back");
}

/// Round 29 (ADR 0060): the whole tempo map goes out and comes back — every tempo point (a
/// ramp in sixteenths, each lasting exactly as long as it plays), every metre change, every
/// marker — and a file's map imported into an empty project plays its notes at the
/// seconds the file does.
#[test]
fn the_tempo_map_round_trips_through_a_midi_file() {
    use cantus_core::tempo::{Marker, MarkerId, MetreChange, TempoPoint};
    let mut p = awkward_project(&[(0, 480), (9_600, 960), (20_000, 240)]);
    p.tempo_bpm = 90.0;
    p.time_signature = TimeSignature::new(4, 4);
    let pt = |tick: i64, bpm: f64, ramp: bool| TempoPoint {
        tick: Ticks(tick),
        bpm,
        ramp,
    };
    p.tempo_points = vec![
        pt(0, 90.0, true),
        pt(7680, 140.0, false),
        pt(15_360, 110.0, false),
    ];
    p.metre_changes = vec![
        MetreChange {
            tick: Ticks::ZERO,
            signature: TimeSignature::new(4, 4),
        },
        MetreChange {
            tick: Ticks(7680),
            signature: TimeSignature::new(7, 8),
        },
        MetreChange {
            tick: Ticks(7680 + 3360),
            signature: TimeSignature::new(3, 4),
        },
    ];
    for (i, (tick, name)) in [(0, "Intro"), (7680, "Verse"), (11_040, "Chorus")]
        .iter()
        .enumerate()
    {
        p.markers.push(Marker {
            id: MarkerId(900 + i as u64),
            tick: Ticks(*tick),
            name: name.to_string(),
            color: None,
        });
    }
    let bytes = midi::export(&p, &ExportOptions::default()).unwrap();
    let back = import_into_empty(&bytes, &ImportOptions::default(), true);
    let (a, b) = (p.tempo_map(), back.tempo_map());
    // On the sixteenths the ramp is written in (between them a step holds its average).
    for tick in [0.0, 960.0, 3_120.0, 7_680.0, 9_000.0, 15_360.0, 20_000.0] {
        let (x, y) = (a.seconds_at(tick), b.seconds_at(tick));
        assert!((x - y).abs() < 1e-4, "{tick}: {x} vs {y}");
    }
    // The ramp is in sixteenths: 32 steps over its 8 beats.
    assert!(back.tempo_points.len() >= 32, "{}", back.tempo_points.len());
    assert_eq!(back.metre_changes, p.metre_changes);
    let names: Vec<(Ticks, &str)> = back
        .markers
        .iter()
        .map(|m| (m.tick, m.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            (Ticks(0), "Intro"),
            (Ticks(7680), "Verse"),
            (Ticks(11_040), "Chorus")
        ]
    );
    assert_eq!(played(&back), played(&p), "every note where it was");
    // The file of `tempo_map_file`, imported keeping its map: the note on beat 6 sounds
    // 4 s in, as in the file.
    let file = import_into_empty(&tempo_map_file(), &ImportOptions::default(), true);
    let m = file.tempo_map();
    assert!((m.seconds_at(Ticks::beats(6).0 as f64) - 4.0).abs() < 1e-9);
}
