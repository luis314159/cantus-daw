//! Review 3 (findings 0, 13, 37): a tempo edit never changes which source frame plays at
//! an unwarped audio window's start (ADR 0060 §4) — a take recorded after play from bar 1,
//! loop passes over the loop, comp windows of one take, a punch-in's pre-roll, a reversed
//! window, a linked one and an alternate comp's — while the windows keep their content
//! origin's tick arithmetic. The command bus adds the window edits to the tempo edit's
//! undo step; undo and redo give each state back exactly.

use cantus_core::audio_time;
use cantus_core::comp::Comp;
use cantus_core::tempo::TempoPoint;
use cantus_core::*;

const RATE: f64 = 48_000.0;

fn pt(beat: i64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick: Ticks::beats(beat),
        bpm,
        ramp,
    }
}

/// One track, a 48 kHz source, the tempo `points`, and audio windows `(start beat, offset
/// beat, lane, reversed)`.
fn song(
    points: Vec<TempoPoint>,
    windows: &[(i64, i64, u8, bool)],
) -> (CommandBus, TrackId, AudioSourceId) {
    let mut p = Project::new("keep");
    p.tempo_bpm = points.first().map_or(120.0, |q| q.bpm);
    p.tempo_points = points;
    let mut t = p.make_track("Vocal");
    let sid = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: sid,
        name: "take".into(),
        path: "C:/take.wav".into(),
        sample_rate: RATE as u32,
        channels: 1,
        frames: 48_000_000,
        sha256: None,
        recorded: true,
    });
    for &(start, offset, lane, reverse) in windows {
        let mut c = p.make_audio_clip("w", sid, Ticks::beats(start), Ticks::beats(2));
        c.offset = Ticks::beats(offset);
        c.lane = lane;
        let a = c.audio.as_mut().unwrap();
        a.slip_frames = 37;
        a.reverse = reverse;
        t.clips.push(c);
    }
    let tid = t.id;
    p.tracks.push(t);
    (CommandBus::new(p), tid, sid)
}

/// The source frame at each window's start (and the window's content origin).
fn frames(bus: &CommandBus, t: TrackId) -> Vec<(f64, Ticks)> {
    let p = bus.project();
    let map = p.tempo_map();
    p.track(t)
        .unwrap()
        .clips
        .iter()
        .map(|c| {
            let a = c.audio.as_ref().unwrap();
            let s = c.start.0 as f64;
            (
                audio_time::frame_at(&map, c.start, c.offset, f64::from(a.slip_frames), RATE, s),
                c.start - c.offset,
            )
        })
        .collect()
}

fn same(before: &[(f64, Ticks)], after: &[(f64, Ticks)]) {
    assert_eq!(before.len(), after.len());
    for (i, (a, b)) in before.iter().zip(after).enumerate() {
        assert!(
            (a.0 - b.0).abs() <= 0.5,
            "window {i}: frame {} became {}",
            a.0,
            b.0
        );
    }
}

/// Run `cmd`, check the frames are kept, then undo and redo exactly.
fn edit_keeps(mut bus: CommandBus, t: TrackId, cmd: Command) -> Vec<(f64, Ticks)> {
    let before = frames(&bus, t);
    let start = bus.project().clone();
    bus.execute(cmd, Origin::User).expect("the edit applies");
    let after = frames(&bus, t);
    same(&before, &after);
    let edited = bus.project().clone();
    bus.undo().unwrap();
    assert_eq!(*bus.project(), start, "one undo gives the song back");
    bus.redo().unwrap();
    assert_eq!(*bus.project(), edited, "redo gives the edit back");
    after
}

/// Play from bar 1, record from bar 17: the take's hidden pre-roll is 64 beats. The intro's
/// tempo edited, bar 17 still plays the frame it did.
#[test]
fn a_take_recorded_after_play_from_bar_one_keeps_its_frame() {
    let (bus, t, _) = song(
        vec![pt(0, 100.0, false), pt(64, 120.0, false)],
        &[(64, 64, 0, false)],
    );
    edit_keeps(
        bus,
        t,
        Command::SetTempoPoint {
            index: 0,
            point: pt(0, 90.0, false),
        },
    );
}

/// Three loop passes over beats 64–72, their origins walked back through the passes before
/// (as loop recording lays them), and a tempo edit before the loop: every pass keeps its
/// frame, so they still agree.
#[test]
fn loop_passes_keep_their_frames_after_an_edit_before_the_loop() {
    let w = [(64, 2, 3, false), (64, 10, 2, false), (64, 18, 0, false)];
    let (bus, t, _) = song(vec![pt(0, 100.0, false), pt(64, 120.0, false)], &w);
    edit_keeps(
        bus,
        t,
        Command::InsertTempoPoint {
            index: 1,
            point: pt(16, 80.0, true),
        },
    );
}

/// A punch-in (a take whose window starts after its pre-roll) and comp windows cut from one
/// take: an edit in the pre-roll keeps every frame, and the windows keep one content origin.
#[test]
fn comp_windows_of_one_take_keep_their_frames_and_their_origin() {
    // The take on lane 1 from beat 8 (a count-in of 8 beats), windows at beats 10 and 12.
    let w = [(8, 8, 1, false), (10, 10, 0, false), (12, 12, 0, false)];
    // The edit is in the count-in alone: 120 again from beat 8.
    let (bus, t, _) = song(
        vec![
            pt(0, 120.0, false),
            pt(4, 120.0, false),
            pt(8, 120.0, false),
        ],
        &w,
    );
    let after = edit_keeps(
        bus,
        t,
        Command::SetTempoPoint {
            index: 1,
            point: pt(4, 75.0, false),
        },
    );
    assert_eq!(after[0].1, after[1].1, "one origin");
    assert_eq!(after[1].1, after[2].1, "one origin");
}

/// A reversed window keeps the frame its start reads; the whole tempo track replaced, and
/// the song's tempo changed.
#[test]
fn a_reversed_window_and_whole_track_edits_keep_their_frames() {
    let (bus, t, _) = song(
        vec![pt(0, 120.0, false), pt(8, 90.0, false)],
        &[(16, 12, 0, true)],
    );
    edit_keeps(
        bus,
        t,
        Command::SetTempoTrack {
            bpm: 140.0,
            points: vec![pt(0, 140.0, true), pt(12, 60.0, false)],
        },
    );
    let (bus, t, _) = song(vec![], &[(16, 12, 0, false)]);
    edit_keeps(bus, t, Command::SetTempo { bpm: 97.0 });
}

/// A linked window keeps its group's fine offset: its offset alone moves, to within half a
/// tick at its tempo.
#[test]
fn a_linked_window_moves_its_offset_alone() {
    let (mut bus, t, _) = song(
        vec![pt(0, 120.0, false), pt(8, 120.0, false)],
        &[(16, 12, 0, false)],
    );
    let c = bus.project().track(t).unwrap().clips[0].id;
    bus.execute(
        Command::SetClipLink {
            track: t,
            clip: c,
            link: Some(LinkId(9_999)),
        },
        Origin::User,
    )
    .unwrap();
    let before = frames(&bus, t);
    bus.execute(
        Command::SetTempoPoint {
            index: 1,
            point: pt(8, 70.0, false),
        },
        Origin::User,
    )
    .unwrap();
    let after = frames(&bus, t);
    let c = &bus.project().track(t).unwrap().clips[0];
    assert_eq!(
        c.audio.as_ref().unwrap().slip_frames,
        37,
        "the group's fine offset"
    );
    let half_tick = 0.5 * 60.0 / (70.0 * 960.0) * RATE;
    assert!((before[0].0 - after[0].0).abs() <= half_tick + 1e-9);
}

/// An alternate comp's kept windows are kept too.
#[test]
fn an_alternate_comps_windows_keep_their_frames() {
    let (mut bus, t, _) = song(vec![pt(0, 120.0, false), pt(4, 120.0, false)], &[]);
    let src = bus.project().audio_pool[0].id;
    let mut win = bus.new_clip("b", Ticks::beats(12), Ticks::beats(2));
    win.audio = Some(AudioRegion::of(src));
    win.offset = Ticks::beats(12);
    win.comp = true;
    let comps = vec![
        Comp {
            name: "A".into(),
            windows: Vec::new(),
        },
        Comp {
            name: "B".into(),
            windows: vec![win.clone()],
        },
    ];
    bus.execute(
        Command::SetTrackComps {
            track: t,
            comps,
            active: 0,
        },
        Origin::User,
    )
    .unwrap();
    let map = bus.project().tempo_map();
    let at = |p: &Project, map: &TempoMap| {
        let c = &p.track(t).unwrap().comps[1].windows[0];
        audio_time::frame_at(map, c.start, c.offset, 0.0, RATE, c.start.0 as f64)
    };
    let before = at(bus.project(), &map);
    bus.execute(
        Command::SetTempoPoint {
            index: 1,
            point: pt(4, 60.0, false),
        },
        Origin::User,
    )
    .unwrap();
    let map = bus.project().tempo_map();
    assert!((at(bus.project(), &map) - before).abs() <= 0.5);
}

/// A window the same batch moved is the batch's: the edit leaves it where it was put.
#[test]
fn a_window_the_edit_moves_itself_is_left_as_it_put_it() {
    let (mut bus, t, _) = song(
        vec![pt(0, 120.0, false), pt(4, 120.0, false)],
        &[(16, 12, 0, false)],
    );
    let c = bus.project().track(t).unwrap().clips[0].id;
    bus.execute(
        Command::Batch {
            label_key: "both".into(),
            commands: vec![
                Command::SetTempoPoint {
                    index: 1,
                    point: pt(4, 60.0, false),
                },
                Command::SetClipOffset {
                    track: t,
                    clip: c,
                    offset: Ticks::beats(3),
                },
            ],
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(
        bus.project().track(t).unwrap().clips[0].offset,
        Ticks::beats(3)
    );
}
