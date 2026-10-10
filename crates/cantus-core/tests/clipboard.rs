//! The clipboard's pure builders (ADR 0036): what a copy keeps and what a paste or a cut
//! runs, each checked through a real `CommandBus` so "undo restores the project" is the
//! bus's own answer, not this file's.

use cantus_core::clipboard::{self, ClipOrigin, Clipboard, Payload, SourcePlan};
use cantus_core::*;
use std::collections::HashMap;

/// A counter for fresh ids, starting where the project's own counter is.
struct Counter(u64);
impl Counter {
    fn of(bus: &CommandBus) -> Self {
        Counter(bus.project().next_id)
    }
    fn take(&mut self) -> u64 {
        let id = self.0;
        self.0 += 1;
        id
    }
}

fn run(bus: &mut CommandBus, label: &str, commands: Vec<Command>) {
    bus.execute(
        Command::Batch {
            label_key: label.into(),
            commands,
        },
        Origin::User,
    )
    .expect("the batch applies");
}

fn normalised(p: &Project) -> Project {
    let mut p = p.clone();
    p.next_id = 0;
    p
}

/// Three tracks; a four-note clip on the first, a clip on the third.
fn fixture() -> (CommandBus, Vec<TrackId>, ClipId, ClipId) {
    let mut bus = CommandBus::default();
    let mut tracks = Vec::new();
    for i in 0..3 {
        let t = bus.new_track(format!("t{i}"));
        tracks.push(t.id);
        bus.execute(
            Command::InsertTrack {
                index: i,
                track: Box::new(t),
            },
            Origin::User,
        )
        .unwrap();
    }
    let mut a = bus.new_clip("a", Ticks::beats(4), Ticks::beats(8));
    for i in 0..4 {
        let mut n = bus.new_note(Ticks::beats(1 + i), Ticks::QUARTER, 60 + i as u8, 90);
        n.muted = i == 2;
        a.notes.push(n);
    }
    let b = bus.new_clip("b", Ticks::beats(12), Ticks::beats(4));
    let (ca, cb) = (a.id, b.id);
    for (t, c) in [(tracks[0], a), (tracks[2], b)] {
        bus.execute(
            Command::InsertClip {
                track: t,
                clip: Box::new(c),
            },
            Origin::User,
        )
        .unwrap();
    }
    (bus, tracks, ca, cb)
}

fn clip(bus: &CommandBus, t: TrackId, c: ClipId) -> Clip {
    bus.project().track(t).unwrap().clip(c).unwrap().clone()
}

fn board(bus: &CommandBus, payload: Payload) -> Clipboard {
    Clipboard::new(ClipOrigin::default(), bus.project(), payload)
}

#[test]
fn copied_notes_start_at_zero_and_keep_pitch_velocity_length_mute() {
    let (bus, t, ca, _) = fixture();
    let c = clip(&bus, t[0], ca);
    let sel: Vec<Note> = c.notes[1..].to_vec();
    let Payload::Notes { notes } = clipboard::copy_notes(&sel, Ticks::beats(2)) else {
        panic!("notes copy as notes");
    };
    assert_eq!(notes.len(), 3);
    assert_eq!(
        notes[0].start,
        Ticks::ZERO,
        "the earliest note starts at zero"
    );
    assert_eq!(notes[1].start, Ticks::beats(1));
    for (a, b) in notes.iter().zip(&sel) {
        assert_eq!(
            (a.pitch, a.velocity, a.length, a.muted),
            (b.pitch, b.velocity, b.length, b.muted)
        );
    }
    let b = board(&bus, Payload::Notes { notes });
    assert_eq!(b.format, clipboard::FORMAT);
    assert_eq!(b.ppq, PPQ);
    assert_eq!(b.len(), 3);
}

#[test]
fn pasted_notes_land_at_the_anchor_with_fresh_ids_and_undo_in_one_step() {
    let (mut bus, t, ca, _) = fixture();
    let before = normalised(bus.project());
    let c = clip(&bus, t[0], ca);
    let Payload::Notes { notes } = clipboard::copy_notes(&c.notes[..2], Ticks::beats(1)) else {
        unreachable!()
    };
    let mut ids = Counter::of(&bus);
    let (cmds, made) =
        clipboard::paste_notes(t[0], &c, &notes, Ticks::beats(3), &mut || ids.take());
    bus.reserve_ids(ids.0);
    assert_eq!(made.len(), 2);
    run(&mut bus, "cmd.paste", cmds);
    let after = clip(&bus, t[0], ca);
    assert_eq!(after.notes.len(), 6);
    for id in &made {
        let n = after.note(*id).expect("the pasted note is there");
        assert!(c.note(*id).is_none(), "a pasted note reuses an id");
        assert!(n.start >= Ticks::beats(3));
    }
    let starts: Vec<Ticks> = made
        .iter()
        .map(|id| after.note(*id).unwrap().start)
        .collect();
    assert_eq!(starts, vec![Ticks::beats(3), Ticks::beats(4)]);
    assert_eq!(
        after.length, c.length,
        "a paste inside the clip does not grow it"
    );
    assert_eq!(bus.undo_label_key(), Some("cmd.paste"));
    bus.undo();
    assert_eq!(
        normalised(bus.project()),
        before,
        "one undo takes the paste away"
    );
}

#[test]
fn pasting_past_the_clip_end_grows_the_window_and_undo_shrinks_it() {
    let (mut bus, t, ca, _) = fixture();
    let before = normalised(bus.project());
    let c = clip(&bus, t[0], ca);
    let Payload::Notes { notes } = clipboard::copy_notes(&c.notes, Ticks::beats(1)) else {
        unreachable!()
    };
    let mut ids = Counter::of(&bus);
    let (cmds, _) = clipboard::paste_notes(t[0], &c, &notes, Ticks::beats(7), &mut || ids.take());
    run(&mut bus, "cmd.paste", cmds);
    let after = clip(&bus, t[0], ca);
    // The last note ends at 7 + 3 + ¼ beats in content ticks.
    assert!(after.content_end() >= Ticks::beats(10) + Ticks::QUARTER);
    assert_eq!(after.start, c.start, "growing moved the clip");
    bus.undo();
    assert_eq!(normalised(bus.project()), before);
}

#[test]
fn pasting_into_a_looping_clip_grows_its_period_so_the_notes_sound() {
    let (mut bus, t, ca, _) = fixture();
    run(
        &mut bus,
        "loop",
        vec![Command::SetClipLoop {
            track: t[0],
            clip: ca,
            content_length: Some(Ticks::beats(2)),
        }],
    );
    let c = clip(&bus, t[0], ca);
    let note = bus.new_note(Ticks::ZERO, Ticks::beats(1), 70, 100);
    let mut ids = Counter::of(&bus);
    let (cmds, made) =
        clipboard::paste_notes(t[0], &c, &[note], Ticks::beats(3), &mut || ids.take());
    run(&mut bus, "cmd.paste", cmds);
    let after = clip(&bus, t[0], ca);
    assert_eq!(
        after.period(),
        Some(Ticks::beats(4)),
        "the period covers the paste"
    );
    let s = Snapshot::compile(bus.project(), 0);
    let id = made[0];
    assert!(
        s.tracks
            .iter()
            .flat_map(|t| &t.events)
            .any(|e| e.note == id),
        "the pasted note is never heard"
    );
}

/// Note-ons heard, as (tick, pitch), straight from the snapshot.
fn heard_notes(bus: &CommandBus) -> Vec<(i64, u8)> {
    let s = Snapshot::compile(bus.project(), 0);
    let mut v: Vec<(i64, u8)> = s
        .tracks
        .iter()
        .flat_map(|t| t.events.iter())
        .filter(|e| e.is_on())
        .map(|e| (e.tick.0, e.pitch))
        .collect();
    v.sort_unstable();
    v
}

/// The usual looping clip: Loop switched on makes the period the clip's length, so the
/// window holds exactly one repeat. A paste past its end must be heard — not sit outside
/// the window — and must not drag the repeats off the bar: the loop grows by whole loops,
/// repeating what it played, and the window grows to show the new loop once.
#[test]
fn pasting_past_a_loop_as_long_as_its_clip_grows_both_by_whole_loops_and_keeps_what_played() {
    let mut bus = CommandBus::default();
    let t = bus.new_track("t");
    let tid = t.id;
    bus.execute(
        Command::InsertTrack {
            index: 0,
            track: Box::new(t),
        },
        Origin::User,
    )
    .unwrap();
    let mut c = bus.new_clip("loop", Ticks::beats(8), Ticks::beats(4));
    c.content_length = Some(Ticks::beats(4));
    for (b, pitch) in [(0, 50u8), (2, 52)] {
        c.notes
            .push(bus.new_note(Ticks::beats(b), Ticks::QUARTER, pitch, 90));
    }
    let cid = c.id;
    bus.execute(
        Command::InsertClip {
            track: tid,
            clip: Box::new(c),
        },
        Origin::User,
    )
    .unwrap();
    let before = normalised(bus.project());
    let played = heard_notes(&bus);
    let c = clip(&bus, tid, cid);
    // A two-beat phrase pasted on the loop's last beat.
    let phrase = vec![
        bus.new_note(Ticks::ZERO, Ticks::beats(1), 70, 100),
        bus.new_note(Ticks::beats(1), Ticks::beats(1), 71, 100),
    ];
    let mut ids = Counter::of(&bus);
    let (cmds, made) =
        clipboard::paste_notes(tid, &c, &phrase, Ticks::beats(3), &mut || ids.take());
    run(&mut bus, "cmd.paste", cmds);
    let after = clip(&bus, tid, cid);
    assert_eq!(
        after.period(),
        Some(Ticks::beats(8)),
        "the loop grows by whole loops (4 → 8 beats), not to the paste's end (5)"
    );
    assert!(
        after.length >= Ticks::beats(8),
        "the window does not show the grown loop: {} ticks",
        after.length.0
    );
    let s = Snapshot::compile(bus.project(), 0);
    for id in &made {
        assert!(
            s.tracks
                .iter()
                .flat_map(|t| &t.events)
                .any(|e| e.note == *id),
            "a pasted note is never heard"
        );
    }
    let now = heard_notes(&bus);
    for p in &played {
        assert!(
            now.contains(p),
            "{p:?} was heard before the paste, not after"
        );
    }
    // The loop's own notes repeat in the room the paste made (beats 12 and 14).
    for p in [(Ticks::beats(12).0, 50u8), (Ticks::beats(14).0, 52)] {
        assert!(
            now.contains(&p),
            "{p:?}: the grown loop is silent where it repeated"
        );
    }
    bus.undo();
    assert_eq!(normalised(bus.project()), before, "undo is exact");
}

#[test]
fn copied_clips_keep_rows_and_spacing() {
    let (bus, t, ca, cb) = fixture();
    let Payload::Clips { rows, span } =
        clipboard::copy_clips(bus.project(), &[(t[2], cb), (t[0], ca)])
    else {
        panic!("clips copy as clips");
    };
    assert_eq!(span, None);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[0].row, rows[0].track),
        (0, t[0]),
        "rows sorted from the top"
    );
    assert_eq!((rows[1].row, rows[1].track), (2, t[2]));
    assert_eq!(rows[0].clip.start, Ticks::ZERO);
    assert_eq!(rows[1].clip.start, Ticks::beats(8));
}

#[test]
fn pasted_clips_keep_relative_track_offsets() {
    let (mut bus, t, ca, cb) = fixture();
    let b = board(
        &bus,
        clipboard::copy_clips(bus.project(), &[(t[0], ca), (t[2], cb)]),
    );
    // A fourth and fifth track so the block fits from row 1.
    for i in 3..5 {
        let tr = bus.new_track(format!("t{i}"));
        bus.execute(
            Command::InsertTrack {
                index: i,
                track: Box::new(tr),
            },
            Origin::User,
        )
        .unwrap();
    }
    let before = normalised(bus.project());
    let mut ids = Counter::of(&bus);
    let out = clipboard::paste_clips(
        bus.project(),
        &b,
        true,
        1,
        Ticks::beats(32),
        &HashMap::new(),
        false,
        &mut || ids.take(),
    );
    bus.reserve_ids(ids.0);
    assert_eq!(out.made.len(), 2);
    run(&mut bus, "cmd.paste_clips", out.commands);
    let p = bus.project();
    assert_eq!(p.track_index(out.made[0].0), Some(1));
    assert_eq!(p.track_index(out.made[1].0), Some(3));
    let first = p.track(out.made[0].0).unwrap().clip(out.made[0].1).unwrap();
    let second = p.track(out.made[1].0).unwrap().clip(out.made[1].1).unwrap();
    assert_eq!(first.start, Ticks::beats(32));
    assert_eq!(second.start, Ticks::beats(40));
    assert_eq!(first.notes.len(), 4);
    bus.undo();
    assert_eq!(normalised(bus.project()), before);
}

#[test]
fn a_block_below_the_last_track_moves_up_whole_instead_of_squashing() {
    let (bus, t, ca, cb) = fixture();
    let b = board(
        &bus,
        clipboard::copy_clips(bus.project(), &[(t[0], ca), (t[2], cb)]),
    );
    let mut ids = Counter::of(&bus);
    let out = clipboard::paste_clips(
        bus.project(),
        &b,
        true,
        2,
        Ticks::ZERO,
        &HashMap::new(),
        false,
        &mut || ids.take(),
    );
    let rows: Vec<usize> = out
        .made
        .iter()
        .map(|r| bus.project().track_index(r.0).unwrap())
        .collect();
    assert_eq!(
        rows,
        vec![0, 2],
        "the block was squashed instead of moved up"
    );
}

#[test]
fn a_pasted_clip_shares_no_ids_with_its_original_and_lands_on_lane_0() {
    let (mut bus, t, ca, _) = fixture();
    run(
        &mut bus,
        "lane",
        vec![Command::SetClipLane {
            track: t[0],
            clip: ca,
            lane: 2,
        }],
    );
    let b = board(&bus, clipboard::copy_clips(bus.project(), &[(t[0], ca)]));
    let mut ids = Counter::of(&bus);
    let out = clipboard::paste_clips(
        bus.project(),
        &b,
        true,
        0,
        Ticks::beats(20),
        &HashMap::new(),
        false,
        &mut || ids.take(),
    );
    run(&mut bus, "cmd.paste_clips", out.commands);
    let original = clip(&bus, t[0], ca);
    let copy = clip(&bus, out.made[0].0, out.made[0].1);
    assert_ne!(copy.id, original.id);
    assert_eq!(copy.lane, 0, "a paste plays");
    for (a, b) in copy.notes.iter().zip(&original.notes) {
        assert_ne!(a.id, b.id, "a pasted note shares its id with the original");
        assert_eq!((a.start, a.pitch, a.muted), (b.start, b.pitch, b.muted));
    }
}

#[test]
fn automation_clips_go_back_to_their_track_in_the_same_project_and_are_skipped_from_another() {
    let (mut bus, t, _, _) = fixture();
    let auto = {
        let p = &mut Project::default();
        p.next_id = bus.project().next_id;
        let c = p.make_automation_clip(
            "auto",
            AutoTarget::TrackGain(t[1]),
            Ticks::beats(2),
            Ticks::beats(4),
            vec![AutoPoint::new(Ticks::ZERO, 0.5)],
        );
        bus.reserve_ids(p.next_id);
        c
    };
    let aid = auto.id;
    run(
        &mut bus,
        "auto",
        vec![Command::InsertClip {
            track: t[1],
            clip: Box::new(auto),
        }],
    );
    let b = board(&bus, clipboard::copy_clips(bus.project(), &[(t[1], aid)]));
    let mut ids = Counter::of(&bus);
    // The same project: back under its own track whatever row is asked for.
    let out = clipboard::paste_clips(
        bus.project(),
        &b,
        true,
        0,
        Ticks::beats(16),
        &HashMap::new(),
        false,
        &mut || ids.take(),
    );
    assert_eq!(out.skipped_automation, 0);
    assert_eq!(out.made.len(), 1);
    assert_eq!(out.made[0].0, t[1], "the automation clip left its track");
    run(&mut bus, "cmd.paste_clips", out.commands);
    let pasted = clip(&bus, t[1], out.made[0].1);
    assert_eq!(
        pasted.automation.as_ref().unwrap().target,
        AutoTarget::TrackGain(t[1])
    );
    // Another project (or another run): what it automates is not here.
    let out = clipboard::paste_clips(
        bus.project(),
        &b,
        false,
        0,
        Ticks::beats(16),
        &HashMap::new(),
        false,
        &mut || ids.take(),
    );
    assert_eq!(out.skipped_automation, 1);
    assert!(out.made.is_empty() && out.commands.is_empty());
}

fn source(id: u64, path: &str) -> AudioSource {
    AudioSource {
        id: AudioSourceId(id),
        name: "loop".into(),
        path: path.into(),
        sample_rate: 48_000,
        channels: 2,
        frames: 96_000,
        sha256: None,
        recorded: true,
    }
}

#[test]
fn audio_sources_are_reused_or_inserted_before_the_clip_and_undo_in_a_legal_order() {
    let (mut bus, t, _, _) = fixture();
    let sid = AudioSourceId(bus.alloc_id());
    let mut clip_a = bus.new_clip("wave", Ticks::beats(4), Ticks::beats(4));
    clip_a.audio = Some(AudioRegion::of(sid));
    let caid = clip_a.id;
    run(
        &mut bus,
        "import",
        vec![
            Command::InsertAudioSource {
                source: source(sid.0, "C:/audio/loop.wav"),
            },
            Command::InsertClip {
                track: t[0],
                clip: Box::new(clip_a),
            },
        ],
    );
    let b = board(&bus, clipboard::copy_clips(bus.project(), &[(t[0], caid)]));
    assert_eq!(b.audio_source_ids(), vec![sid]);

    // Reuse: no new source.
    let mut ids = Counter::of(&bus);
    let mut plans = HashMap::new();
    plans.insert(sid, SourcePlan::Reuse(sid));
    let out = clipboard::paste_clips(
        bus.project(),
        &b,
        false,
        1,
        Ticks::ZERO,
        &plans,
        false,
        &mut || ids.take(),
    );
    assert!(!out
        .commands
        .iter()
        .any(|c| matches!(c, Command::InsertAudioSource { .. })));
    assert_eq!(out.made.len(), 1);

    // Insert: a fresh source first, never "recorded" (a save must not move another
    // project's file), and the clip plays it.
    let before = normalised(bus.project());
    let fresh = AudioSourceId(ids.take());
    let mut s = source(fresh.0, "D:/other/loop.wav");
    s.recorded = false;
    plans.insert(sid, SourcePlan::Insert(s));
    let out = clipboard::paste_clips(
        bus.project(),
        &b,
        false,
        1,
        Ticks::ZERO,
        &plans,
        false,
        &mut || ids.take(),
    );
    bus.reserve_ids(ids.0);
    assert!(matches!(out.commands[0], Command::InsertAudioSource { .. }));
    run(&mut bus, "cmd.paste_clips", out.commands);
    let pasted = clip(&bus, out.made[0].0, out.made[0].1);
    assert_eq!(pasted.audio.as_ref().unwrap().source, fresh);
    assert!(!bus.project().audio_source(fresh).unwrap().recorded);
    bus.undo();
    assert_eq!(
        normalised(bus.project()),
        before,
        "undo left the source or the clip"
    );
    // No plan at all: the clip is left out rather than playing a stranger's id.
    let out = clipboard::paste_clips(
        bus.project(),
        &b,
        false,
        1,
        Ticks::ZERO,
        &HashMap::new(),
        false,
        &mut || ids.take(),
    );
    assert_eq!(out.skipped_audio, 1);
    assert!(out.made.is_empty());
}

#[test]
fn an_audio_clip_pasted_at_another_tempo_plays_the_same_part_of_its_file() {
    let (mut bus, t, _, _) = fixture();
    let sid = AudioSourceId(bus.alloc_id());
    let mut c = bus.new_clip("wave", Ticks::beats(4), Ticks::beats(4));
    let mut region = AudioRegion::of(sid);
    region.fade_in = Ticks::beats(1);
    c.audio = Some(region);
    c.offset = Ticks::beats(2);
    let cid = c.id;
    run(
        &mut bus,
        "import",
        vec![
            Command::InsertAudioSource {
                source: source(sid.0, "C:/a.wav"),
            },
            Command::InsertClip {
                track: t[0],
                clip: Box::new(c),
            },
        ],
    );
    let mut b = board(&bus, clipboard::copy_clips(bus.project(), &[(t[0], cid)]));
    // Copied from a project at half this one's tempo.
    b.tempo_bpm = 60.0;
    let mut plans = HashMap::new();
    plans.insert(sid, SourcePlan::Reuse(sid));
    let mut ids = Counter::of(&bus);
    let out = clipboard::paste_clips(
        bus.project(),
        &b,
        false,
        0,
        Ticks::ZERO,
        &plans,
        false,
        &mut || ids.take(),
    );
    let Command::InsertClip { clip: pasted, .. } = &out.commands[0] else {
        panic!("an InsertClip");
    };
    // 4 beats at 60 BPM are 4 seconds: 8 beats at 120.
    assert_eq!(pasted.length, Ticks::beats(8));
    assert_eq!(pasted.offset, Ticks::beats(4));
    assert_eq!(pasted.audio.as_ref().unwrap().fade_in, Ticks::beats(2));
}

#[test]
fn cut_clips_then_undo_restores_the_project_exactly() {
    let (mut bus, t, ca, cb) = fixture();
    let before = normalised(bus.project());
    let cmds = clipboard::cut_clips(
        bus.project(),
        &[(t[0], ca), (t[2], cb), (t[1], ClipId(999_999))],
    );
    assert_eq!(cmds.len(), 2, "a clip that is not there is not cut");
    run(&mut bus, "cmd.cut_clips", cmds);
    assert!(bus.project().tracks.iter().all(|t| t.clips.is_empty()));
    assert_eq!(bus.undo_label_key(), Some("cmd.cut_clips"));
    bus.undo();
    assert_eq!(normalised(bus.project()), before);
}

#[test]
fn notes_pasted_into_the_arrangement_make_a_clip_of_whole_bars() {
    let (bus, t, ca, _) = fixture();
    let c = clip(&bus, t[0], ca);
    let Payload::Notes { notes } = clipboard::copy_notes(&c.notes, Ticks::ZERO) else {
        unreachable!()
    };
    let bar = bus.project().time_signature.ticks_per_bar();
    let mut ids = Counter::of(&bus);
    let made = clipboard::notes_clip("n", &notes, Ticks::beats(8), bar, &mut || ids.take());
    assert_eq!(made.start, Ticks::beats(8));
    // The notes end at 4 and a quarter beats: two 4/4 bars.
    assert_eq!(made.length, Ticks(bar.0 * 2));
    assert_eq!(made.notes.len(), 4);
    assert!(made.notes.iter().all(|n| c.note(n.id).is_none()));
    assert_eq!(made.notes[0].start, Ticks::beats(1));
}

#[test]
fn clipboard_serde_roundtrip() {
    let (bus, t, ca, cb) = fixture();
    let mut b = board(
        &bus,
        clipboard::copy_clips(bus.project(), &[(t[0], ca), (t[2], cb)]),
    );
    b.sources.push(source(5, "C:/x.wav"));
    let json = serde_json::to_string(&b).unwrap();
    assert!(
        json.starts_with("{\"format\":\"cantus-clip/2\""),
        "the format tag must come first: {}",
        &json[..40.min(json.len())]
    );
    let back: Clipboard = serde_json::from_str(&json).unwrap();
    assert_eq!(back, b);
    let notes = board(
        &bus,
        clipboard::copy_notes(&clip(&bus, t[0], ca).notes, Ticks::ZERO),
    );
    let back: Clipboard = serde_json::from_str(&serde_json::to_string(&notes).unwrap()).unwrap();
    assert_eq!(back, notes);
}
