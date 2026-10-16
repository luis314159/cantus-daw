//! Comping (ADR 0025 §5): a swipe across a take lane makes that range of the playing lane
//! a window on the take, in one undo step; promote, delete unused takes, explode.

use cantus_core::audio::AudioSource;
use cantus_core::{AudioSourceId, Command, CommandBus, Origin, Project, Ticks, TrackId};
use cantus_ui::comping;

fn beats(n: i64) -> Ticks {
    Ticks::beats(n)
}

fn source(p: &mut Project, name: &str) -> AudioSourceId {
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: name.into(),
        path: format!("C:/rec/{name}.wav"),
        sample_rate: 48_000,
        channels: 1,
        frames: 480_000,
        sha256: None,
        recorded: true,
    });
    id
}

/// A track with take A on lane 1, take B on lane 2, and B playing on lane 0 — the state a
/// two-pass loop recording leaves, over bars 1–2.
fn takes() -> (Project, TrackId, AudioSourceId, AudioSourceId) {
    let mut p = Project::default();
    let t = p.make_track("Vocal");
    let tid = t.id;
    p.tracks.push(t);
    let a = source(&mut p, "A");
    let b = source(&mut p, "B");
    for (src, lane) in [(a, 1u8), (b, 2), (b, 0)] {
        let mut c = p.make_audio_clip("take", src, Ticks::ZERO, beats(8));
        c.lane = lane;
        // B plays over the takes: the comp's, as the recording lays it (schema 32).
        c.comp = lane == 0;
        p.track_mut(tid).unwrap().clips.push(c);
    }
    (p, tid, a, b)
}

fn run(bus: &mut CommandBus, f: impl FnOnce(&Project, &mut dyn FnMut() -> u64) -> Vec<Command>) {
    let mut next = bus.project().next_id;
    let commands = {
        let mut ids = || {
            let id = next;
            next += 1;
            id
        };
        f(bus.project(), &mut ids)
    };
    bus.reserve_ids(next);
    assert!(!commands.is_empty(), "the operation did something");
    let before = bus.history().len();
    bus.execute(
        Command::Batch {
            label_key: "cmd.comp".into(),
            commands,
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(bus.history().len(), before + 1, "one undo step");
}

/// Lane 0 as (start, end, source).
fn lane0(p: &Project, t: TrackId) -> Vec<(i64, i64, AudioSourceId)> {
    let mut v: Vec<_> = p
        .track(t)
        .unwrap()
        .clips
        .iter()
        .filter(|c| c.lane == 0)
        .map(|c| (c.start.0, c.end().0, c.audio.as_ref().unwrap().source))
        .collect();
    v.sort();
    v
}

#[test]
fn a_swipe_makes_that_range_a_window_on_the_take_and_undoes_in_one_step() {
    let (p, t, a, b) = takes();
    let mut bus = CommandBus::new(p.clone());
    run(&mut bus, |p, ids| {
        comping::swipe(p, t, 1, beats(2), beats(4), ids)
    });
    assert_eq!(
        lane0(bus.project(), t),
        vec![(0, 1920, b), (1920, 3840, a), (3840, 7680, b)]
    );
    // Every lane-0 window shows its take at the same place: the comp lines up.
    for c in bus
        .project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .filter(|c| c.lane == 0)
    {
        assert_eq!(c.start - c.offset, Ticks::ZERO);
    }
    // A second stroke on the same take next to the first grows that window.
    run(&mut bus, |p, ids| {
        comping::swipe(p, t, 1, beats(4), beats(5), ids)
    });
    assert_eq!(
        lane0(bus.project(), t),
        vec![(0, 1920, b), (1920, 4800, a), (4800, 7680, b)]
    );
    // The take lane knows which of its ranges are in the comp.
    let tr = bus.project().track(t).unwrap();
    let take_a = tr.clips.iter().find(|c| c.lane == 1).unwrap();
    let mut lit = Vec::new();
    comping::for_each_comped(tr, take_a, |x, y| lit.push((x.0, y.0)));
    assert_eq!(lit, vec![(1920, 4800)]);
    bus.undo();
    bus.undo();
    assert_eq!(bus.project().tracks, p.tracks);
}

#[test]
fn promoting_a_take_makes_it_the_whole_comp() {
    let (p, t, a, _) = takes();
    let mut bus = CommandBus::new(p);
    run(&mut bus, |p, ids| comping::promote_lane(p, t, 1, ids));
    assert_eq!(lane0(bus.project(), t), vec![(0, 7680, a)]);
}

#[test]
fn deleting_unused_takes_closes_the_lanes_and_empties_the_pool() {
    let (p, t, a, b) = takes();
    let mut bus = CommandBus::new(p);
    // The comp is all A: B's take lane and B's lane-0 window go once nothing plays B.
    run(&mut bus, |p, ids| comping::promote_lane(p, t, 1, ids));
    run(&mut bus, |p, _| comping::delete_unused_takes(p, t));
    let tr = bus.project().track(t).unwrap();
    let lanes: Vec<_> = tr.clips.iter().map(|c| c.lane).collect();
    assert_eq!(lanes.iter().filter(|l| **l == 1).count(), 1);
    assert!(lanes.iter().all(|l| *l <= 1), "lane 2 is gone: {lanes:?}");
    let pool: Vec<_> = bus.project().audio_pool.iter().map(|s| s.id).collect();
    assert_eq!(
        pool,
        vec![a],
        "B is played by nothing and leaves the pool ({b:?})"
    );
}

#[test]
fn a_take_with_part_of_it_in_the_comp_is_kept() {
    let (p, t, _, _) = takes();
    let mut bus = CommandBus::new(p);
    run(&mut bus, |p, ids| {
        comping::swipe(p, t, 1, beats(2), beats(4), ids)
    });
    let commands = comping::delete_unused_takes(bus.project(), t);
    assert!(
        commands.is_empty(),
        "both takes are in the comp: {commands:?}"
    );
}

#[test]
fn exploding_takes_makes_muted_tracks_and_changes_nothing_heard() {
    let (mut p, t, _, _) = takes();
    p.track_mut(t).unwrap().takes_open = true;
    let mut bus = CommandBus::new(p);
    let heard = cantus_core::Snapshot::compile(bus.project(), 0).tracks[0]
        .audio
        .len();
    run(&mut bus, |p, ids| comping::explode_takes(p, t, "Take", ids));
    let pr = bus.project();
    assert_eq!(pr.tracks.len(), 3);
    assert_eq!(pr.tracks[1].name, "Vocal \u{b7} Take 1");
    assert!(pr.tracks[1].clips.iter().all(|c| c.lane == 0 && c.muted));
    assert!(pr.track(t).unwrap().clips.iter().all(|c| c.lane == 0));
    assert!(!pr.track(t).unwrap().takes_open);
    assert_eq!(
        pr.tracks[1].strip,
        pr.track(t).unwrap().strip,
        "same channel"
    );
    let snap = cantus_core::Snapshot::compile(pr, 0);
    let total: usize = snap.tracks.iter().map(|t| t.audio.len()).sum();
    assert_eq!(total, heard);
}

#[test]
fn midi_takes_comp_by_sharing_their_notes() {
    let mut p = Project::default();
    let t = p.make_track("Keys");
    let tid = t.id;
    p.tracks.push(t);
    let mut take = p.make_clip("Keys \u{b7} Take 1", Ticks::ZERO, beats(4));
    take.notes
        .push(p.make_note(Ticks::ZERO, Ticks(480), 60, 100));
    take.notes.push(p.make_note(beats(2), Ticks(480), 64, 100));
    take.lane = 1;
    let first = take.notes[1].id;
    p.track_mut(tid).unwrap().clips.push(take);
    let mut bus = CommandBus::new(p);
    run(&mut bus, |p, ids| {
        comping::swipe(p, tid, 1, beats(2), beats(3), ids)
    });
    let tr = bus.project().track(tid).unwrap();
    let w = tr.clips.iter().find(|c| c.lane == 0).unwrap();
    assert_eq!(
        (w.start, w.length, w.offset),
        (beats(2), beats(1), beats(2))
    );
    assert!(
        w.notes.iter().any(|n| n.id == first),
        "the same notes, by id"
    );
    let snap = cantus_core::Snapshot::compile(bus.project(), 0);
    let pitches: Vec<_> = snap.tracks[0].events.iter().map(|e| e.pitch).collect();
    assert!(
        pitches.contains(&64) && !pitches.contains(&60),
        "{pitches:?}"
    );
}
