//! Review 2 of the tempo track in MIDI files (ADR 0060): markers kept are no texts
//! dropped (finding 59), an odd metre is dropped not fatal and two at one tick keep the
//! later (60), a first metre written a few ticks in is the song's (61), an SMPTE file keeps
//! no map (64), a marker at the music's end is exported (65), and a map dropped into a song
//! at a bar starts there, the song's own before it, in one undo step (27, 62).

use cantus_core::tempo::{Marker, MarkerId, MetreChange, TempoPoint};
use cantus_core::*;
use cantus_project::midi::{self, smf, Event, ExportOptions, ImportOptions, Placement};

fn ev(tick: u64, event: Event) -> smf::TimedEvent {
    smf::TimedEvent { tick, event }
}

fn metre(n: u8, pow: u8) -> Event {
    Event::TimeSignature {
        numerator: n,
        denominator_pow: pow,
        clocks: 24,
        n32: 8,
    }
}

fn note(tick: u64, len: u64, key: u8) -> [smf::TimedEvent; 2] {
    [
        ev(
            tick,
            Event::NoteOn {
                channel: 0,
                key,
                velocity: 100,
            },
        ),
        ev(
            tick + len,
            Event::NoteOff {
                channel: 0,
                key,
                velocity: 0,
            },
        ),
    ]
}

/// A type 1 file: the conductor `conductor`, one track of notes.
fn file(
    division: smf::Division,
    conductor: Vec<smf::TimedEvent>,
    notes: Vec<smf::TimedEvent>,
) -> Vec<u8> {
    smf::write(&smf::Smf {
        format: 1,
        division,
        tracks: vec![conductor, notes],
    })
}

fn pt(tick: i64, bpm: f64) -> TempoPoint {
    TempoPoint {
        tick: Ticks(tick),
        bpm,
        ramp: false,
    }
}

fn mc(tick: i64, n: u8, d: u8) -> MetreChange {
    MetreChange {
        tick: Ticks(tick),
        signature: TimeSignature::new(n, d),
    }
}

/// Import `bytes` into `project` as placed, as one batch: the bus after it.
fn import(project: Project, bytes: &[u8], place: Placement) -> CommandBus {
    let imported = midi::read(bytes, &ImportOptions::default()).expect("reads");
    let mut bus = CommandBus::new(project);
    let mut next = bus.project().next_id;
    let built = {
        let mut ids = || {
            next += 1;
            next - 1
        };
        midi::import_commands(bus.project(), &imported, &place, &mut ids, &|l| l.english())
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
    bus
}

/// Finding 59: a Cantus project's markers, tempo and metre round trip with nothing
/// reported lost.
#[test]
fn markers_kept_are_no_texts_dropped() {
    let mut p = Project::new("Round");
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(16));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100));
    t.clips.push(c);
    p.tracks.push(t);
    p.tempo_points = vec![pt(0, 120.0), pt(7680, 90.0)];
    for (i, (tick, name)) in [(0, "Intro"), (3840, "Verse"), (11_520, "Chorus")]
        .iter()
        .enumerate()
    {
        p.markers.push(Marker {
            id: MarkerId(800 + i as u64),
            tick: Ticks(*tick),
            name: name.to_string(),
            color: None,
        });
    }
    let bytes = midi::export(&p, &ExportOptions::default()).unwrap();
    let read = midi::read(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(read.markers.len(), 3);
    assert!(
        read.report.is_lossless(),
        "{:?}",
        read.report.dropped_items().collect::<Vec<_>>()
    );
}

/// Findings 60 and 61: a 3/64 is dropped (counted) and the import still applies; two
/// signatures that land on one Cantus tick keep the later; a first signature a few ticks
/// in is the song's own, with no 4/4 bar of ten ticks before it.
#[test]
fn odd_and_crowded_metres_are_tidied() {
    let ppq = 1920u64;
    let notes: Vec<_> = note(0, ppq, 60).into_iter().collect();
    let conductor = vec![
        ev(0, metre(4, 2)),
        // 7679 and 7680 file ticks at 1920 PPQ are both Cantus tick 3840.
        ev(7_679, metre(3, 2)),
        ev(7_680, metre(5, 2)),
        ev(4 * 4 * ppq, metre(3, 6)),
    ];
    let bytes = file(smf::Division::Ppq(ppq as u16), conductor, notes.clone());
    let read = midi::read(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(read.metre_changes, vec![mc(0, 4, 4), mc(3840, 5, 4)]);
    assert!(
        read.report.has("interop.report.odd_metres"),
        "the 3/64 is said"
    );
    let bus = import(
        Project::new("Into"),
        &bytes,
        Placement {
            apply_tempo: true,
            ..Default::default()
        },
    );
    assert_eq!(
        bus.project().metre_changes,
        vec![mc(0, 4, 4), mc(3840, 5, 4)]
    );
    // A sequencer's first signature after its setup events: 4/4 at file tick 10.
    for (n, pow) in [(4u8, 2u8), (3, 2)] {
        let bytes = file(
            smf::Division::Ppq(480),
            vec![ev(10, metre(n, pow))],
            notes.clone(),
        );
        let read = midi::read(&bytes, &ImportOptions::default()).unwrap();
        assert!(
            read.metre_changes.is_empty(),
            "{n}: {:?}",
            read.metre_changes
        );
        assert_eq!(read.time_signature, Some(TimeSignature::new(n, 1 << pow)));
    }
}

/// Finding 64: an SMPTE file's times are seconds — no tempo points, metre changes or
/// markers are taken for a map.
#[test]
fn an_smpte_file_keeps_no_map() {
    let conductor = vec![
        ev(0, metre(4, 2)),
        ev(100, metre(3, 2)),
        ev(
            200,
            Event::Text {
                kind: 0x06,
                text: "Verse".into(),
            },
        ),
    ];
    let notes: Vec<_> = note(0, 50, 60).into_iter().collect();
    let bytes = file(
        smf::Division::Smpte {
            fps: 25,
            ticks_per_frame: 40,
        },
        conductor,
        notes,
    );
    let read = midi::read(&bytes, &ImportOptions::default()).unwrap();
    assert!(read.tempo_points.is_empty());
    assert!(read.metre_changes.is_empty());
    assert!(read.markers.is_empty());
}

/// Finding 65: a marker where the music ends, and one after it, are in the file.
#[test]
fn markers_at_and_after_the_end_are_exported() {
    let mut p = Project::new("End");
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(8));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100));
    t.clips.push(c);
    p.tracks.push(t);
    for (i, (beat, name)) in [(8, "End"), (12, "Outro")].iter().enumerate() {
        p.markers.push(Marker {
            id: MarkerId(900 + i as u64),
            tick: Ticks::beats(*beat),
            name: name.to_string(),
            color: None,
        });
    }
    let bytes = midi::export(&p, &ExportOptions::default()).unwrap();
    let read = midi::read(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(
        read.markers,
        vec![
            (Ticks::beats(8), "End".to_string()),
            (Ticks::beats(12), "Outro".to_string())
        ]
    );
}

/// Findings 27 and 62: a file with its own map dropped into a song at bar 5 (a beat past
/// the bar line: it goes to the line) — the song keeps its tempo track and metre before it,
/// the file's start there, the song's markers stay and the file's join them; one undo gives
/// the song back. Dropped at the start, the file's map is the song's.
#[test]
fn a_map_dropped_into_a_song_starts_at_its_bar() {
    let ppq = 480u64;
    // 3/4 at 90 BPM, 4/4 from the file's bar 3 (beat 6), 150 BPM from beat 4; a marker at
    // beat 3; a note on beat 0.
    let conductor = vec![
        ev(0, Event::Tempo(666_667)),
        ev(0, metre(3, 2)),
        ev(
            3 * ppq,
            Event::Text {
                kind: 0x06,
                text: "Hook".into(),
            },
        ),
        ev(4 * ppq, Event::Tempo(400_000)),
        ev(6 * ppq, metre(4, 2)),
    ];
    let notes: Vec<_> = note(0, ppq, 64).into_iter().collect();
    let bytes = file(smf::Division::Ppq(ppq as u16), conductor, notes);
    let mut song = Project::new("Song");
    song.tempo_bpm = 100.0;
    song.tempo_points = vec![pt(0, 100.0), pt(3840, 120.0)];
    song.markers.push(Marker {
        id: MarkerId(700),
        tick: Ticks::ZERO,
        name: "A".into(),
        color: None,
    });
    let before = song.clone();
    let bar5 = 4 * 3840;
    let mut bus = import(
        song,
        &bytes,
        Placement {
            at: Ticks(bar5 + 960),
            apply_tempo: true,
            ..Default::default()
        },
    );
    let p = bus.project();
    assert_eq!(
        p.tempo_points,
        vec![
            pt(0, 100.0),
            pt(3840, 120.0),
            pt(bar5, 90.0),
            pt(bar5 + 3840, 150.0)
        ]
    );
    assert_eq!(p.tempo_bpm, 100.0, "the song's start keeps its tempo");
    assert_eq!(
        p.metre_changes,
        vec![mc(0, 4, 4), mc(bar5, 3, 4), mc(bar5 + 5760, 4, 4)]
    );
    assert_eq!(p.time_signature, TimeSignature::new(4, 4));
    let marks: Vec<(Ticks, &str)> = p
        .markers
        .iter()
        .map(|m| (m.tick, m.name.as_str()))
        .collect();
    assert_eq!(
        marks,
        vec![(Ticks::ZERO, "A"), (Ticks(bar5 + 2880), "Hook")]
    );
    let notes: Vec<Ticks> = p
        .tracks
        .iter()
        .flat_map(|t| {
            t.clips
                .iter()
                .flat_map(|c| c.notes.iter().map(move |n| c.start - c.offset + n.start))
        })
        .collect();
    assert_eq!(
        notes,
        vec![Ticks(bar5)],
        "the note on the bar the map starts at"
    );
    assert!(
        p.tempo_map().bbt(Ticks(bar5 + 5760)).1 == 1,
        "4/4 on a bar line"
    );
    bus.undo().unwrap();
    assert_eq!(bus.project().tempo_points, before.tempo_points);
    assert_eq!(bus.project().metre_changes, before.metre_changes);
    assert_eq!(bus.project().markers, before.markers);
    assert_eq!(bus.project().tracks, before.tracks);
    // At the start: the file's map is the song's, the song's markers stay.
    let mut song = before.clone();
    song.metre_changes = vec![mc(0, 4, 4), mc(7680, 7, 8)];
    let bus = import(
        song,
        &bytes,
        Placement {
            apply_tempo: true,
            ..Default::default()
        },
    );
    let p = bus.project();
    assert_eq!(p.tempo_points, vec![pt(0, 90.0), pt(3840, 150.0)]);
    assert_eq!(p.tempo_bpm, 90.0);
    assert_eq!(p.metre_changes, vec![mc(0, 3, 4), mc(5760, 4, 4)]);
    assert_eq!(p.markers.len(), 2);
}

/// Review 3 (finding 5): a file of one tempo dropped at bar 5 into a song with a later
/// tempo point: the file's tempo holds from bar 5 on — the song's later point is gone.
#[test]
fn a_one_tempo_file_dropped_later_replaces_the_songs_later_points() {
    let notes: Vec<_> = note(0, 480, 60).into_iter().collect();
    let bytes = file(
        smf::Division::Ppq(480),
        vec![ev(0, Event::Tempo(500_000))],
        notes,
    );
    let mut song = Project::new("Song");
    song.tempo_points = vec![pt(0, 120.0), pt(30_720, 140.0)];
    let bus = import(
        song,
        &bytes,
        Placement {
            at: Ticks(15_360),
            apply_tempo: true,
            ..Default::default()
        },
    );
    assert!(
        bus.project().tempo_points.is_empty(),
        "{:?}",
        bus.project().tempo_points
    );
    assert_eq!(bus.project().tempo_bpm, 120.0);
}

/// Review 3 (findings 43, 51): notes kept at their seconds ('Keep the timing', every SMPTE
/// file) go through the song's tempo map: 120 to beat 8 (4 s), then 60 — a note at 6 s in
/// the file sounds 6 s into the song.
#[test]
fn notes_kept_at_their_seconds_go_through_the_songs_map() {
    let ppq = 480u64;
    // 120 BPM: 6 s is beat 12.
    let notes: Vec<_> = note(12 * ppq, ppq / 2, 60).into_iter().collect();
    let bytes = file(
        smf::Division::Ppq(ppq as u16),
        vec![ev(0, Event::Tempo(500_000))],
        notes,
    );
    let mut song = Project::new("Song");
    song.tempo_points = vec![pt(0, 120.0), pt(7680, 60.0)];
    let opts = ImportOptions {
        tempo_mode: midi::TempoMode::Seconds,
        target_bpm: Some(song.tempo_bpm),
    };
    let imported = midi::read(&bytes, &opts).expect("reads");
    let mut bus = CommandBus::new(song);
    let mut next = bus.project().next_id;
    let built = {
        let mut ids = || {
            next += 1;
            next - 1
        };
        midi::import_commands(
            bus.project(),
            &imported,
            &Placement::default(),
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
    let p = bus.project();
    let c = &p.tracks.last().unwrap().clips[0];
    let n = &c.notes[0];
    let at = c.start - c.offset + n.start;
    let secs = p.tempo_map().seconds_between(Ticks::ZERO, at);
    assert!((secs - 6.0).abs() < 1e-3, "the note sounds at {secs} s");
}

/// Review 3 (finding 52): a tempo map with no clips — a click map — exports and comes back.
#[test]
fn a_map_without_clips_round_trips() {
    let mut p = Project::new("Map");
    p.tempo_points = vec![pt(0, 120.0), pt(7680, 90.0), pt(15_360, 140.0)];
    p.metre_changes = vec![mc(0, 4, 4), mc(15_360, 7, 8)];
    let bytes = midi::export(&p, &ExportOptions::default()).unwrap();
    let read = midi::read(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(read.tempo_points, p.tempo_points);
    assert_eq!(read.metre_changes, p.metre_changes);
}

/// Review 3 (finding 53): a first tempo a few ticks in is the song's, whatever PPQ.
#[test]
fn a_first_tempo_a_few_ticks_in_is_the_songs() {
    let notes: Vec<_> = note(0, 480, 60).into_iter().collect();
    let bytes = file(
        smf::Division::Ppq(480),
        vec![ev(5, Event::Tempo(600_000))],
        notes.clone(),
    );
    let read = midi::read(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(read.tempo_bpm, 100.0);
    assert!(read.tempo_points.is_empty(), "{:?}", read.tempo_points);
    let notes: Vec<_> = note(0, 3840, 60).into_iter().collect();
    let conductor = vec![
        ev(1, Event::Tempo(600_000)),
        ev(4 * 3840, Event::Tempo(400_000)),
    ];
    let bytes = file(smf::Division::Ppq(3840), conductor, notes);
    let read = midi::read(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(read.tempo_bpm, 100.0);
    assert_eq!(read.tempo_points, vec![pt(0, 100.0), pt(3840, 150.0)]);
    let bus = import(
        Project::new("Into"),
        &bytes,
        Placement {
            apply_tempo: true,
            ..Default::default()
        },
    );
    assert_eq!(
        bus.project().tempo_points,
        vec![pt(0, 100.0), pt(3840, 150.0)]
    );
}

/// Review 3 (finding 54): a pickup — 4/4 from the start, a 3/4 at beat 2 — keeps its bar
/// lines; only a signature within a sixteenth of the start is the song's.
#[test]
fn a_pickups_metre_keeps_its_place() {
    let notes: Vec<_> = note(0, 480, 60).into_iter().collect();
    let bytes = file(
        smf::Division::Ppq(480),
        vec![ev(2 * 480, metre(3, 2))],
        notes,
    );
    let read = midi::read(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(read.metre_changes, vec![mc(0, 4, 4), mc(1920, 3, 4)]);
}

/// Review 4 (finding 50): a file with no time signature, imported at bar 1 with the song's
/// tempo set from it, leaves the song's metre changes as they are (as at a later bar).
#[test]
fn a_file_without_a_metre_leaves_the_songs_metre_changes() {
    let mut song = Project::new("Bridge");
    song.metre_changes = vec![mc(0, 4, 4), mc(16 * 3840, 3, 4)];
    let notes: Vec<_> = note(0, 480, 60).into_iter().collect();
    let bytes = file(
        smf::Division::Ppq(480),
        vec![ev(0, Event::Tempo(500_000))],
        notes,
    );
    for at in [Ticks::ZERO, Ticks(3840)] {
        let bus = import(
            song.clone(),
            &bytes,
            Placement {
                at,
                apply_tempo: true,
                ..Default::default()
            },
        );
        assert_eq!(bus.project().metre_changes, song.metre_changes, "at {at:?}");
    }
}

/// Review 4 (finding 51): a file in 4/4 with a 7/8 from bar 2 gives clips on the song's bar
/// lines through the 7/8, not in whole bars of 4/4.
#[test]
fn imported_clips_end_on_the_songs_bar_lines() {
    let ppq = 480u64;
    let mut notes: Vec<smf::TimedEvent> = Vec::new();
    // Bar 1 (4/4: 1 920 file ticks), then bars of 7/8 (1 680 file ticks): the last note
    // ends inside the third bar of 7/8.
    notes.extend(note(0, ppq, 60));
    notes.extend(note(1920 + 2 * 1680 + 100, 200, 62));
    let bytes = file(
        smf::Division::Ppq(ppq as u16),
        vec![ev(0, metre(4, 2)), ev(1920, metre(7, 3))],
        notes,
    );
    let bus = import(
        Project::new("Odd"),
        &bytes,
        Placement {
            apply_tempo: true,
            ..Default::default()
        },
    );
    let p = bus.project();
    let map = p.tempo_map();
    let c = &p.tracks.last().unwrap().clips[0];
    assert_eq!(map.bar_start(c.start), c.start, "starts on a bar line");
    assert_eq!(map.bar_start(c.end()), c.end(), "ends on a bar line");
    // 3 840 + 3 × 3 360: the end of the third bar of 7/8.
    assert_eq!(c.end(), Ticks(3840 + 3 * 3360));
}

/// Review 5 (finding 13): a file with no metre dropped at bar 6 of a song with a 3/4 from
/// bar 5, the song's tempo kept: the clip ends on the song's 3/4 bar lines.
#[test]
fn a_clip_dropped_into_a_songs_metre_change_ends_on_its_bars() {
    let mut song = Project::new("Waltz from bar 5");
    song.metre_changes = vec![mc(0, 4, 4), mc(4 * 3840, 3, 4)];
    let notes: Vec<_> = note(0, 480, 60)
        .into_iter()
        .chain(note(4 * 480, 240, 62))
        .collect();
    let bytes = file(smf::Division::Ppq(480), vec![], notes);
    let bar6 = Ticks(4 * 3840 + 2880);
    let bus = import(
        song,
        &bytes,
        Placement {
            at: bar6,
            apply_tempo: false,
            ..Default::default()
        },
    );
    let p = bus.project();
    let map = p.tempo_map();
    let c = &p.tracks.last().unwrap().clips[0];
    assert_eq!(c.start, bar6);
    assert_eq!(map.bar_start(c.end()), c.end(), "on a bar line");
    // The notes reach 4 320 ticks past bar 6 (the file's beat 4 and an eighth): into the
    // second bar of 3/4 after it, which ends 2 × 2 880 on.
    assert_eq!(c.end(), bar6 + Ticks(2 * 2880));
}
