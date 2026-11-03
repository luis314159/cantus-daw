//! What each clip operation means to an ARA plug-in (ADR 0097): the document of a project,
//! the updates between two states, and undo.

use cantus_ara::document::{apply, diff, Document, Update};
use cantus_core::audio::AudioSource;
use cantus_core::bus::CommandBus;
use cantus_core::command::{Command, Origin};
use cantus_core::model::{AudioSourceId, ClipId, Project, TrackId};
use cantus_core::time::Ticks;

fn source(id: u64, frames: u64) -> AudioSource {
    AudioSource {
        id: AudioSourceId(id),
        name: format!("take {id}"),
        path: format!("Audio/take{id}.wav"),
        sample_rate: 48_000,
        channels: 1,
        frames,
        sha256: Some(format!("{id:064x}")),
        recorded: false,
    }
}

/// One track with one audio clip of two beats at beat 1 (120 BPM: 0.5 s per beat).
fn fixture() -> (CommandBus, TrackId, ClipId) {
    let mut bus = CommandBus::new(Project::new("ara"));
    let mut track = bus.new_track("Vocal");
    let sid = AudioSourceId(bus.alloc_id());
    let clip = bus
        .project()
        .clone()
        .make_audio_clip("take", sid, Ticks::beats(1), Ticks::beats(2));
    // `make_audio_clip` on a clone allocated from the clone's counter: take a fresh id here.
    let mut clip = clip;
    clip.id = ClipId(bus.alloc_id());
    let (tid, cid) = (track.id, clip.id);
    track.clips.clear();
    bus.execute(
        Command::InsertTrack {
            index: 0,
            track: Box::new(track),
        },
        Origin::User,
    )
    .unwrap();
    bus.execute(
        Command::InsertAudioSource {
            source: source(sid.0, 480_000),
        },
        Origin::User,
    )
    .unwrap();
    bus.execute(
        Command::InsertClip {
            track: tid,
            clip: Box::new(clip),
        },
        Origin::User,
    )
    .unwrap();
    (bus, tid, cid)
}

fn doc(bus: &CommandBus) -> Document {
    Document::of_project(bus.project())
}

fn run(bus: &mut CommandBus, c: Command) {
    bus.execute(c, Origin::User).unwrap();
}

#[test]
fn a_clip_is_a_region_in_seconds_and_its_source_a_source() {
    let (bus, t, c) = fixture();
    let d = doc(&bus);
    assert_eq!(d.sources.len(), 1);
    let r = &d.regions[&c];
    assert_eq!(r.track, t);
    assert!((r.start - 0.5).abs() < 1e-9, "{}", r.start);
    assert!((r.duration - 1.0).abs() < 1e-9);
    assert!(
        r.source_start.abs() < 1e-9,
        "an untrimmed clip starts at the source's start"
    );
    assert!(!r.muted);
}

#[test]
fn a_project_with_no_audio_clip_is_an_empty_document() {
    let d = Document::of_project(&Project::new("empty"));
    assert!(d.sources.is_empty() && d.regions.is_empty());
}

#[test]
fn identical_states_give_no_update() {
    let (bus, _, _) = fixture();
    assert!(diff(&doc(&bus), &doc(&bus)).is_empty());
}

#[test]
fn a_new_clip_adds_its_source_before_its_region() {
    let (bus, _, _) = fixture();
    let updates = diff(&Document::default(), &doc(&bus));
    assert!(matches!(updates[0], Update::AddSource(_)));
    assert!(matches!(updates[1], Update::AddRegion(_)));
    assert_eq!(updates.len(), 2);
}

#[test]
fn moving_and_trimming_update_the_region_and_nothing_else() {
    let (mut bus, t, c) = fixture();
    let before = doc(&bus);
    run(
        &mut bus,
        Command::SetClipBounds {
            track: t,
            clip: c,
            start: Ticks::beats(2),
            length: Ticks::beats(1),
        },
    );
    let after = doc(&bus);
    let u = diff(&before, &after);
    assert_eq!(u.len(), 1);
    let Update::UpdateRegion(r) = &u[0] else {
        panic!("{u:?}")
    };
    assert!((r.start - 1.0).abs() < 1e-9 && (r.duration - 0.5).abs() < 1e-9);
    let mut copy = before.clone();
    apply(&mut copy, &u);
    assert_eq!(copy, after);
}

#[test]
fn slipping_moves_the_source_under_the_region_not_the_region() {
    let (mut bus, t, c) = fixture();
    let before = doc(&bus);
    run(
        &mut bus,
        Command::SetClipOffset {
            track: t,
            clip: c,
            offset: Ticks::beats(1),
        },
    );
    let after = doc(&bus);
    let (a, b) = (&before.regions[&c], &after.regions[&c]);
    assert_eq!((a.start, a.duration), (b.start, b.duration));
    assert!((b.source_start - 0.5).abs() < 1e-9, "{}", b.source_start);
    assert_eq!(diff(&before, &after).len(), 1);
}

#[test]
fn a_split_is_one_update_and_one_new_region_on_the_same_source() {
    let (mut bus, t, c) = fixture();
    let before = doc(&bus);
    let mut right = bus.project().track(t).unwrap().clip(c).unwrap().clone();
    right.id = ClipId(bus.alloc_id());
    right.start = Ticks::beats(2);
    right.length = Ticks::beats(1);
    right.offset = Ticks::beats(1);
    run(
        &mut bus,
        Command::SetClipBounds {
            track: t,
            clip: c,
            start: Ticks::beats(1),
            length: Ticks::beats(1),
        },
    );
    run(
        &mut bus,
        Command::InsertClip {
            track: t,
            clip: Box::new(right),
        },
    );
    let after = doc(&bus);
    let u = diff(&before, &after);
    assert_eq!(u.len(), 2, "{u:?}");
    assert!(u.iter().any(|x| matches!(x, Update::UpdateRegion(_))));
    assert!(u.iter().any(|x| matches!(x, Update::AddRegion(_))));
    assert_eq!(after.sources.len(), 1);
    // The right half plays the source from where the left half stops: no gap, no overlap.
    let mut rs: Vec<_> = after.regions.values().collect();
    rs.sort_by(|a, b| a.start.total_cmp(&b.start));
    assert!((rs[0].start + rs[0].duration - rs[1].start).abs() < 1e-9);
    assert!((rs[0].source_start + rs[0].duration - rs[1].source_start).abs() < 1e-9);
    let mut copy = before;
    apply(&mut copy, &u);
    assert_eq!(copy, after);
}

#[test]
fn mute_is_an_update_and_a_deleted_clip_removes_its_region() {
    let (mut bus, t, c) = fixture();
    let d0 = doc(&bus);
    run(
        &mut bus,
        Command::SetClipMute {
            track: t,
            clip: c,
            muted: true,
        },
    );
    let d1 = doc(&bus);
    let u = diff(&d0, &d1);
    assert!(matches!(&u[..], [Update::UpdateRegion(r)] if r.muted));
    run(&mut bus, Command::RemoveClip { track: t, clip: c });
    let d2 = doc(&bus);
    // The source leaves the document with its last region (nothing uses it).
    assert_eq!(
        diff(&d1, &d2),
        vec![
            Update::RemoveRegion(c),
            Update::RemoveSource(AudioSourceId(d1.sources.keys().next().unwrap().0))
        ]
    );
}

#[test]
fn undo_returns_to_the_previous_document_exactly() {
    let (mut bus, t, c) = fixture();
    let start = doc(&bus);
    run(
        &mut bus,
        Command::SetClipBounds {
            track: t,
            clip: c,
            start: Ticks::beats(3),
            length: Ticks::beats(1),
        },
    );
    run(&mut bus, Command::RemoveClip { track: t, clip: c });
    // The plug-in's copy follows the project through the edits and back through two undos.
    let mut plugin = start.clone();
    let mut last = start.clone();
    let mut check = |bus: &CommandBus| {
        let now = doc(bus);
        apply(&mut plugin, &diff(&last, &now));
        assert_eq!(plugin, now);
        last = now.clone();
        now
    };
    let removed = check(&bus);
    assert!(removed.regions.is_empty());
    bus.undo().unwrap();
    assert_eq!(check(&bus).regions[&c].start, 1.5);
    bus.undo().unwrap();
    assert_eq!(check(&bus), start);
}

#[test]
fn a_replaced_file_removes_its_regions_and_source_and_adds_them_again() {
    let (_, _, c) = fixture();
    let (bus, _, _) = fixture();
    let old = doc(&bus);
    let mut new = old.clone();
    for s in new.sources.values_mut() {
        s.sha256 = Some("f".repeat(64));
    }
    let u = diff(&old, &new);
    let kinds: Vec<&str> = u
        .iter()
        .map(|x| match x {
            Update::RemoveRegion(_) => "rr",
            Update::RemoveSource(_) => "rs",
            Update::AddSource(_) => "as",
            Update::AddRegion(_) => "ar",
            Update::UpdateRegion(_) => "ur",
        })
        .collect();
    assert_eq!(kinds, ["rr", "rs", "as", "ar"]);
    let _ = c;
    let mut copy = old;
    apply(&mut copy, &u);
    assert_eq!(copy, new);
}

#[test]
fn take_lanes_are_never_heard_so_never_regions() {
    let (mut bus, t, c) = fixture();
    run(
        &mut bus,
        Command::SetClipLane {
            track: t,
            clip: c,
            lane: 1,
        },
    );
    assert!(doc(&bus).regions.is_empty());
}

#[test]
fn an_edit_in_a_tempo_change_moves_the_seconds_with_the_map() {
    let (mut bus, _, c) = fixture();
    let slow = doc(&bus).regions[&c].clone();
    let mut p = bus.project().clone();
    p.tempo_bpm = 60.0;
    let fast = Document::of_project(&p).regions[&c].clone();
    assert!((fast.start - 2.0 * slow.start).abs() < 1e-9);
    assert!((fast.duration - 2.0 * slow.duration).abs() < 1e-9);
    let _ = &mut bus;
}
