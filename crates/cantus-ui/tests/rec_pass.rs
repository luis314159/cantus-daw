//! A recording pass becomes the right sources, clips, lanes and windows (ADR 0025 §5–§7),
//! as one undo step — tested with `FinishedTake`s built by hand, as the engine will hand
//! them back.

use cantus_core::audio::{AudioRegion, AudioSource, TakeMode, TrackInput};
use cantus_core::{
    AudioSourceId, Clip, Command, CommandBus, Origin, Project, Snapshot, Ticks, TrackId,
};
use cantus_engine::FinishedTake;
use cantus_ui::record_pass::{self, PassPlan};
use cantus_ui::recording::RecordedNote;
use std::path::PathBuf;

const SR: u32 = 48_000;
/// At 120 BPM a beat is half a second: 24 000 frames, 960 ticks.
const BEAT_FRAMES: u64 = 24_000;

fn beats(n: i64) -> Ticks {
    Ticks::beats(n)
}

fn project() -> (Project, TrackId) {
    let mut p = Project {
        tempo_bpm: 120.0,
        ..Default::default()
    };
    let mut t = p.make_track("Vocal");
    t.input = Some(TrackInput::mono(0));
    t.armed = true;
    let id = t.id;
    p.tracks.push(t);
    (p, id)
}

fn take(track: TrackId, start_tick: f64, frames: u64, wraps: Vec<u64>) -> FinishedTake {
    FinishedTake {
        track,
        path: PathBuf::from(format!("C:/rec/Vocal {}.wav", frames)),
        sample_rate: SR,
        channels: 1,
        frames,
        start_tick,
        wraps,
        wrap_shifts: Vec::new(),
        dropped: 0,
    }
}

fn plan(track: TrackId, record_start: Ticks) -> PassPlan {
    let mut p = PassPlan::new(record_start);
    p.audio_tracks = vec![track];
    p
}

/// Run a pass through a bus: the project after it, and the bus to undo it with.
fn run(
    p: Project,
    plan: &PassPlan,
    takes: &[FinishedTake],
    notes: &[RecordedNote],
    latency: i64,
) -> CommandBus {
    let mut bus = CommandBus::new(p);
    let mut next = bus.project().next_id;
    let commands = {
        let mut ids = || {
            let id = next;
            next += 1;
            id
        };
        record_pass::build(bus.project(), plan, takes, notes, latency, &mut ids).expect("build")
    };
    bus.reserve_ids(next);
    let before = bus.history().len();
    bus.execute(
        Command::Batch {
            label_key: "cmd.record_take".into(),
            commands,
        },
        Origin::User,
    )
    .expect("the batch applies");
    assert_eq!(bus.history().len(), before + 1, "one undo step per pass");
    bus
}

/// (start, length, offset, lane) of every clip on the track, by lane then start.
fn layout(p: &Project, t: TrackId) -> Vec<(i64, i64, i64, u8)> {
    let mut v: Vec<_> = p
        .track(t)
        .unwrap()
        .clips
        .iter()
        .map(|c| (c.start.0, c.length.0, c.offset.0, c.lane))
        .collect();
    v.sort_by_key(|x| (x.3, x.0));
    v
}

fn audio_clip(p: &mut Project, t: TrackId, name: &str, start: Ticks, len: Ticks) -> AudioSourceId {
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: name.into(),
        path: format!("C:/old/{name}.wav"),
        sample_rate: SR,
        channels: 1,
        frames: 10 * SR as u64,
        sha256: None,
        recorded: true,
    });
    let clip = p.make_audio_clip(name, id, start, len);
    p.track_mut(t).unwrap().clips.push(clip);
    id
}

#[test]
fn a_take_keeps_its_count_in_as_a_handle() {
    let (p, t) = project();
    let before = p.clone();
    // A bar of count-in: the transport started at 0, recording asked for bar 2.
    let plan = plan(t, beats(4));
    let tk = take(t, 0.0, 8 * BEAT_FRAMES, vec![]);
    let mut bus = run(p, &plan, &[tk], &[], 0);
    let pr = bus.project();
    assert_eq!(layout(pr, t), vec![(3840, 3840, 3840, 0)]);
    let src = &pr.audio_pool[0];
    assert!(src.recorded, "a recording is moved on save, not copied");
    assert_eq!(
        (src.frames, src.sample_rate, src.channels),
        (192_000, SR, 1)
    );
    let clip = &pr.track(t).unwrap().clips[0];
    assert_eq!(clip.audio, Some(AudioRegion::of(src.id)));
    assert_eq!(clip.name, "Vocal \u{b7} Take 1");
    // What the engine would play: one audio clip.
    let snap = Snapshot::compile(pr, 0);
    assert_eq!(snap.tracks[0].audio.len(), 1);
    bus.undo();
    assert_eq!(bus.project().tracks, before.tracks);
    assert!(
        bus.project().audio_pool.is_empty(),
        "undo takes the source out too"
    );
}

#[test]
fn latency_moves_the_content_not_the_window() {
    let (p, t) = project();
    // The engine says frame 0 belongs 480 frames (10 ms, 19.2 ticks) before bar 2.
    let start_tick = 3840.0 - 19.2;
    let bus = run(
        p,
        &plan(t, beats(4)),
        &[take(t, start_tick, BEAT_FRAMES * 4, vec![])],
        &[],
        480,
    );
    let l = layout(bus.project(), t);
    assert_eq!(
        (l[0].0, l[0].2),
        (3840, 19),
        "the window at bar 2, the content 19 ticks in"
    );
}

#[test]
fn a_loop_recording_is_a_clip_per_pass_and_the_last_plays() {
    let (p, t) = project();
    let mut plan = plan(t, Ticks::ZERO);
    plan.loop_region = Some((Ticks::ZERO, beats(4)));
    let pass = 4 * BEAT_FRAMES;
    // Three passes; the third stopped after 2.5 beats.
    let tk = take(t, 0.0, 2 * pass + 60_000, vec![pass, 2 * pass]);
    let bus = run(p, &plan, &[tk], &[], 0);
    assert_eq!(
        layout(bus.project(), t),
        vec![(0, 2400, 7680, 0), (0, 3840, 0, 1), (0, 3840, 3840, 2),]
    );
    let names: Vec<_> = bus
        .project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .map(|c| c.name.clone())
        .collect();
    assert!(
        names.contains(&"Vocal \u{b7} Take 3".to_string()),
        "{names:?}"
    );
}

#[test]
fn a_loop_pass_stopped_in_its_first_quarter_is_not_a_take() {
    let (p, t) = project();
    let mut plan = plan(t, Ticks::ZERO);
    plan.loop_region = Some((Ticks::ZERO, beats(4)));
    let pass = 4 * BEAT_FRAMES;
    let tk = take(t, 0.0, 2 * pass + 10_000, vec![pass, 2 * pass]);
    let bus = run(p, &plan, &[tk], &[], 0);
    assert_eq!(
        layout(bus.project(), t),
        vec![(0, 3840, 3840, 0), (0, 3840, 0, 1)]
    );
}

#[test]
fn later_passes_are_lined_up_by_the_latency_too() {
    let (p, t) = project();
    let mut plan = plan(t, Ticks::ZERO);
    plan.loop_region = Some((Ticks::ZERO, beats(4)));
    let pass = 4 * BEAT_FRAMES;
    let tk = take(t, -19.2, 2 * pass, vec![pass]);
    let bus = run(p, &plan, &[tk], &[], 480);
    let l = layout(bus.project(), t);
    // Pass 1 shows the file from the wrap plus the latency: 3840 + 19.2 ticks; it ends where
    // the file does, the latency short of the stop (what was heard after it was not yet in).
    assert_eq!(l[0], (0, 3821, 3859, 0));
    assert_eq!(l[1], (0, 3840, 19, 1));
}

#[test]
fn without_loop_takes_only_the_last_pass_is_kept() {
    let (p, t) = project();
    let mut plan = plan(t, Ticks::ZERO);
    plan.loop_region = Some((Ticks::ZERO, beats(4)));
    plan.loop_takes = false;
    let pass = 4 * BEAT_FRAMES;
    let bus = run(
        p,
        &plan,
        &[take(t, 0.0, 3 * pass, vec![pass, 2 * pass])],
        &[],
        0,
    );
    assert_eq!(layout(bus.project(), t), vec![(0, 3840, 7680, 0)]);
}

#[test]
fn punch_sets_the_window_and_keeps_both_handles() {
    let (p, t) = project();
    let mut plan = plan(t, Ticks::ZERO);
    plan.punch = Some((beats(2), beats(3)));
    let bus = run(p, &plan, &[take(t, 0.0, 8 * BEAT_FRAMES, vec![])], &[], 0);
    assert_eq!(layout(bus.project(), t), vec![(1920, 960, 1920, 0)]);
}

#[test]
fn a_take_that_misses_the_punch_leaves_nothing() {
    let (p, t) = project();
    let mut plan = plan(t, Ticks::ZERO);
    plan.punch = Some((beats(20), beats(24)));
    let bus = run(p, &plan, &[take(t, 0.0, 4 * BEAT_FRAMES, vec![])], &[], 0);
    assert!(bus.project().track(t).unwrap().clips.is_empty());
    assert!(bus.project().audio_pool.is_empty(), "no clip, no source");
}

#[test]
fn stack_moves_what_the_take_covers_to_a_take_lane() {
    let (mut p, t) = project();
    audio_clip(&mut p, t, "Old", Ticks::ZERO, beats(8));
    let mut plan = plan(t, beats(2));
    plan.punch = Some((beats(2), beats(6)));
    let bus = run(p, &plan, &[take(t, 0.0, 8 * BEAT_FRAMES, vec![])], &[], 0);
    assert_eq!(
        layout(bus.project(), t),
        vec![
            (0, 1920, 0, 0),
            (1920, 3840, 1920, 0),
            (5760, 1920, 5760, 0),
            (1920, 3840, 1920, 1),
        ]
    );
    // Lane 0 plays the old clip around the new take; the take lane is silent.
    let snap = Snapshot::compile(bus.project(), 0);
    assert_eq!(snap.tracks[0].audio.len(), 3);
}

#[test]
fn replace_cuts_away_what_the_take_covers() {
    let (mut p, t) = project();
    audio_clip(&mut p, t, "Old", Ticks::ZERO, beats(8));
    let mut plan = plan(t, beats(2));
    plan.punch = Some((beats(2), beats(6)));
    plan.mode = TakeMode::Replace;
    let mut bus = run(
        p.clone(),
        &plan,
        &[take(t, 0.0, 8 * BEAT_FRAMES, vec![])],
        &[],
        0,
    );
    assert_eq!(
        layout(bus.project(), t),
        vec![
            (0, 1920, 0, 0),
            (1920, 3840, 1920, 0),
            (5760, 1920, 5760, 0)
        ]
    );
    bus.undo();
    assert_eq!(
        bus.project().tracks,
        p.tracks,
        "one step restores the old clip"
    );
}

#[test]
fn a_loop_over_old_material_stacks_it_first_then_the_passes() {
    let (mut p, t) = project();
    audio_clip(&mut p, t, "Old", Ticks::ZERO, beats(4));
    let mut plan = plan(t, Ticks::ZERO);
    plan.loop_region = Some((Ticks::ZERO, beats(4)));
    let pass = 4 * BEAT_FRAMES;
    let bus = run(p, &plan, &[take(t, 0.0, 2 * pass, vec![pass])], &[], 0);
    let tr = bus.project().track(t).unwrap();
    let lane_of = |name: &str| tr.clips.iter().find(|c| c.name == name).map(|c| c.lane);
    assert_eq!(
        lane_of("Old"),
        Some(1),
        "the oldest material is the first take lane"
    );
    assert_eq!(lane_of("Vocal \u{b7} Take 1"), Some(2));
    assert_eq!(lane_of("Vocal \u{b7} Take 2"), Some(0));
}

#[test]
fn multitrack_takes_land_on_their_own_tracks() {
    let (mut p, a) = project();
    let mut t2 = p.make_track("Guitar");
    t2.input = Some(TrackInput::stereo(2));
    t2.armed = true;
    let b = t2.id;
    p.tracks.push(t2);
    let mut plan = plan(a, Ticks::ZERO);
    plan.audio_tracks.push(b);
    let takes = [
        take(a, 0.0, 4 * BEAT_FRAMES, vec![]),
        FinishedTake {
            channels: 2,
            ..take(b, 0.0, 4 * BEAT_FRAMES, vec![])
        },
    ];
    let bus = run(p, &plan, &takes, &[], 0);
    assert_eq!(bus.project().audio_pool.len(), 2);
    assert_eq!(layout(bus.project(), a).len(), 1);
    assert_eq!(layout(bus.project(), b).len(), 1);
    assert_eq!(bus.project().audio_pool[1].channels, 2);
}

fn note(start: i64, len: i64, pitch: u8, pass: u32) -> RecordedNote {
    RecordedNote {
        start: Ticks(start),
        length: Ticks(len),
        pitch,
        velocity: 100,
        pass,
    }
}

fn midi_project() -> (Project, TrackId) {
    let mut p = Project {
        tempo_bpm: 120.0,
        ..Default::default()
    };
    let mut t = p.make_track("Keys");
    t.armed = true;
    let id = t.id;
    p.tracks.push(t);
    (p, id)
}

fn notes_of(c: &Clip) -> Vec<(i64, u8)> {
    c.notes.iter().map(|n| (n.start.0, n.pitch)).collect()
}

#[test]
fn midi_loop_passes_are_takes_too() {
    let (p, t) = midi_project();
    let mut plan = PassPlan::new(Ticks::ZERO);
    plan.midi_tracks = vec![t];
    plan.loop_region = Some((Ticks::ZERO, beats(4)));
    plan.last_pass = 1;
    plan.stop_at = beats(2);
    let notes = [note(0, 480, 60, 0), note(960, 480, 62, 1)];
    let bus = run(p, &plan, &[], &notes, 0);
    let tr = bus.project().track(t).unwrap();
    let lane0: Vec<_> = tr.clips.iter().filter(|c| c.lane == 0).collect();
    let lane1: Vec<_> = tr.clips.iter().filter(|c| c.lane == 1).collect();
    assert_eq!((lane0.len(), lane1.len()), (1, 1));
    assert_eq!(notes_of(lane0[0]), vec![(960, 62)]);
    assert_eq!(notes_of(lane1[0]), vec![(0, 60)]);
    // A MIDI take is a clip from bar line to bar line.
    assert_eq!((lane1[0].start, lane1[0].length), (Ticks::ZERO, beats(4)));
}

#[test]
fn midi_overdub_merges_into_the_clip_under_the_record_start() {
    let (mut p, t) = midi_project();
    let clip = p.make_clip("Pattern", Ticks::ZERO, beats(4));
    let cid = clip.id;
    p.track_mut(t).unwrap().clips.push(clip);
    let mut plan = PassPlan::new(Ticks::ZERO);
    plan.midi_tracks = vec![t];
    plan.mode = TakeMode::Overdub;
    plan.loop_region = Some((Ticks::ZERO, beats(4)));
    plan.last_pass = 1;
    let notes = [note(0, 240, 36, 0), note(1920, 240, 38, 1)];
    let bus = run(p, &plan, &[], &notes, 0);
    let tr = bus.project().track(t).unwrap();
    assert_eq!(tr.clips.len(), 1, "both passes went into the pattern");
    assert_eq!(notes_of(tr.clip(cid).unwrap()), vec![(0, 36), (1920, 38)]);
}

#[test]
fn midi_punch_keeps_only_the_notes_inside_it() {
    let (p, t) = midi_project();
    let mut plan = PassPlan::new(Ticks::ZERO);
    plan.midi_tracks = vec![t];
    plan.punch = Some((beats(4), beats(8)));
    plan.stop_at = beats(12);
    let notes = [
        note(960, 240, 60, 0),
        note(4800, 240, 64, 0),
        note(9600, 240, 67, 0),
    ];
    let bus = run(p, &plan, &[], &notes, 0);
    let tr = bus.project().track(t).unwrap();
    assert_eq!(tr.clips.len(), 1);
    let c = &tr.clips[0];
    assert_eq!((c.start, c.length), (beats(4), beats(4)));
    let heard: Vec<_> = c
        .notes
        .iter()
        .map(|n| c.start + n.start - c.offset)
        .collect();
    assert_eq!(heard, vec![Ticks(4800)]);
}

#[test]
fn quick_record_overdubs_the_selected_clip() {
    let (mut p, t) = midi_project();
    p.track_mut(t).unwrap().armed = false;
    let clip = p.make_clip("Riff", beats(4), beats(4));
    let cid = clip.id;
    p.track_mut(t).unwrap().clips.push(clip);
    let mut plan = PassPlan::new(beats(4));
    plan.quick = Some((t, Some(cid)));
    plan.mode = TakeMode::Stack; // quick record always merges
    let notes = [note(4800, 480, 60, 0), note(4 * 960 + 16 * 960, 480, 62, 0)];
    let bus = run(p, &plan, &[], &notes, 0);
    let c = bus.project().track(t).unwrap().clip(cid).unwrap().clone();
    assert_eq!(c.notes.len(), 2);
    assert!(
        c.end() >= Ticks(20 * 960 + 480),
        "the clip grew to hold the take"
    );
}

/// Run comping's commands on the bus as one step.
fn comp_step(
    bus: &mut CommandBus,
    f: impl FnOnce(&Project, &mut dyn FnMut() -> u64) -> Vec<Command>,
) {
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
    assert!(!commands.is_empty());
    bus.execute(
        Command::Batch {
            label_key: "cmd.comp".into(),
            commands,
        },
        Origin::User,
    )
    .expect("applies");
}

/// Fix4 (schema 32): a pass laid over takes is the comp's window — the loop's last pass
/// over its earlier passes, a stacked take over what it moved to a take lane — and lights
/// no take it does not play (the loop's earlier passes are other stretches of its file).
/// A first recording on an empty track, and what a stack leaves playing around the take,
/// are plain clips.
#[test]
fn a_pass_laid_over_takes_is_the_comps_window() {
    let (p, t) = project();
    let mut lp = plan(t, Ticks::ZERO);
    lp.loop_region = Some((Ticks::ZERO, beats(4)));
    let pass = 4 * BEAT_FRAMES;
    let bus = run(
        p,
        &lp,
        &[take(t, 0.0, 2 * pass + 60_000, vec![pass, 2 * pass])],
        &[],
        0,
    );
    let tr = bus.project().track(t).unwrap();
    let windows: Vec<(i64, i64)> = cantus_ui::comping::comp_windows(tr)
        .iter()
        .map(|c| (c.start.0, c.offset.0))
        .collect();
    assert_eq!(windows, vec![(0, 7680)], "the last pass is the comp's");
    for take in tr.clips.iter().filter(|c| c.lane > 0) {
        let mut lit = Vec::new();
        cantus_ui::comping::for_each_comped(tr, take, |a, b| lit.push((a, b)));
        assert!(lit.is_empty(), "lane {} is not played: {lit:?}", take.lane);
    }

    let (p, t) = project();
    let first = run(
        p,
        &plan(t, Ticks::ZERO),
        &[take(t, 0.0, 4 * BEAT_FRAMES, vec![])],
        &[],
        0,
    );
    assert!(
        first
            .project()
            .track(t)
            .unwrap()
            .clips
            .iter()
            .all(|c| !c.comp),
        "a first recording is no comp"
    );

    let (mut p, t) = project();
    audio_clip(&mut p, t, "Old", Ticks::ZERO, beats(8));
    let mut sp = plan(t, beats(2));
    sp.punch = Some((beats(2), beats(6)));
    let mut bus = run(p, &sp, &[take(t, 0.0, 8 * BEAT_FRAMES, vec![])], &[], 0);
    let flags = |bus: &CommandBus| -> Vec<(i64, u8, bool)> {
        let mut v: Vec<_> = bus
            .project()
            .track(t)
            .unwrap()
            .clips
            .iter()
            .map(|c| (c.start.0, c.lane, c.comp))
            .collect();
        v.sort_by_key(|x| (x.1, x.0));
        v
    };
    assert_eq!(
        flags(&bus),
        vec![
            (0, 0, false),
            (1920, 0, true),
            (5760, 0, false),
            (1920, 1, false)
        ]
    );
    // So alternate comps work after a stacked take: comp B swipes the old take in, and
    // comp A comes back with the new take whole.
    comp_step(&mut bus, |p, ids| {
        cantus_ui::comping::duplicate_comp(p, t, ids)
    });
    comp_step(&mut bus, |p, ids| {
        cantus_ui::comping::swipe(p, t, 1, beats(2), beats(4), ids)
    });
    comp_step(&mut bus, |p, _| cantus_ui::comping::switch_comp(p, t, 0));
    assert_eq!(
        layout(bus.project(), t)
            .into_iter()
            .filter(|c| c.3 == 0)
            .collect::<Vec<_>>(),
        vec![
            (0, 1920, 0, 0),
            (1920, 3840, 1920, 0),
            (5760, 1920, 5760, 0)
        ],
        "comp A: the new take whole"
    );
}

/// Fix5: a pass recorded away from a track's takes lies over none, so it is no comp
/// window — it plays under every comp, as under schema 31 (before, any take lane anywhere
/// on the track flagged it and a switch took it away). One recorded over its own loop's
/// earlier passes, far from the old takes, is still the comp's.
#[test]
fn a_pass_recorded_away_from_the_takes_plays_under_every_comp() {
    let (mut p, t) = project();
    audio_clip(&mut p, t, "Old", Ticks::ZERO, beats(8));
    let mut sp = plan(t, beats(2));
    sp.punch = Some((beats(2), beats(6)));
    let mut bus = run(p, &sp, &[take(t, 0.0, 8 * BEAT_FRAMES, vec![])], &[], 0);
    // Comp B: the old take swiped back in.
    comp_step(&mut bus, |p, ids| {
        cantus_ui::comping::duplicate_comp(p, t, ids)
    });
    comp_step(&mut bus, |p, ids| {
        cantus_ui::comping::swipe(p, t, 1, beats(2), beats(4), ids)
    });
    // A chorus recorded at beat 20, where no take lies.
    let p = bus.project().clone();
    let far = run(
        p,
        &plan(t, beats(20)),
        &[take(t, 20.0 * 960.0, 4 * BEAT_FRAMES, vec![])],
        &[],
        0,
    );
    let at = |bus: &CommandBus| -> Vec<(u8, bool)> {
        bus.project()
            .track(t)
            .unwrap()
            .clips
            .iter()
            .filter(|c| c.start == beats(20))
            .map(|c| (c.lane, c.comp))
            .collect()
    };
    assert_eq!(
        at(&far),
        vec![(0, false)],
        "a pass over no take is no window"
    );
    let mut far = far;
    comp_step(&mut far, |p, _| cantus_ui::comping::switch_comp(p, t, 0));
    assert_eq!(at(&far), vec![(0, false)], "comp A plays the chorus");
    comp_step(&mut far, |p, _| cantus_ui::comping::switch_comp(p, t, 1));
    assert_eq!(at(&far), vec![(0, false)], "comp B plays the chorus");

    // A loop recorded at beat 20 on the same track: its last pass lies over its own
    // earlier passes, so it is the comp's.
    let p = bus.project().clone();
    let mut lp = plan(t, beats(20));
    lp.loop_region = Some((beats(20), beats(24)));
    let pass = 4 * BEAT_FRAMES;
    let looped = run(
        p,
        &lp,
        &[take(t, 20.0 * 960.0, 2 * pass, vec![pass])],
        &[],
        0,
    );
    assert!(
        at(&looped).contains(&(0, true)),
        "the loop's last pass is the comp's: {:?}",
        at(&looped)
    );
}
