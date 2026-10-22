//! Review 4 of the tempo track's audio rule (ADR 0060 §4): a tempo edit re-anchors the
//! windows of one take **together** — one shift for every window that shares the take's
//! content origin and fine offset, measured at its earliest window — so a split, a comp
//! seam or an alternate comp never opens a gap or a repeat, whether the edit is before the
//! take or inside it, with a tempo track or with the song's one tempo. A reversed line keeps
//! the frame it lands on (its end). A looping window keeps its ticks; a window whose file is
//! gone does not stop the edit. A gesture (a drag, a knob, a wheel) is measured from where
//! it began: a hundred frames are one edit, a drag back to the start leaves everything as
//! it was, its undo step holds one fix, and a widened gesture redoes what it did.

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

/// A window: `(start, length, offset)` in ticks, its lane, reversed or not.
type W = (i64, i64, i64, u8, bool);

/// One track, a 48 kHz source, the tempo `points` (none: the song's 120), the windows.
fn song(points: Vec<TempoPoint>, windows: &[W]) -> (CommandBus, TrackId, Vec<ClipId>) {
    let mut p = Project::new("groups");
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
    let mut ids = Vec::new();
    for &(start, length, offset, lane, reverse) in windows {
        let mut c = p.make_audio_clip("w", sid, Ticks(start), Ticks(length));
        c.offset = Ticks(offset);
        c.lane = lane;
        let a = c.audio.as_mut().unwrap();
        a.slip_frames = 37;
        a.reverse = reverse;
        ids.push(c.id);
        t.clips.push(c);
    }
    let tid = t.id;
    p.tracks.push(t);
    (CommandBus::new(p), tid, ids)
}

const B: i64 = 960;

fn clip(bus: &CommandBus, t: TrackId, id: ClipId) -> Clip {
    bus.project().track(t).unwrap().clip(id).unwrap().clone()
}

/// The frame a window plays at `tick` (backwards when reversed), as the engine plays it.
fn frame(map: &TempoMap, c: &Clip, tick: f64) -> f64 {
    let a = c.audio.as_ref().unwrap();
    let slip = f64::from(a.slip_frames);
    if a.reverse {
        let first = audio_time::frame_at(map, c.start, c.offset, slip, RATE, c.start.0 as f64);
        first + map.seconds_between_f64(tick, c.end().0 as f64) * RATE
    } else {
        audio_time::frame_at(map, c.start, c.offset, slip, RATE, tick)
    }
}

/// Run `cmd` as one step and check undo and redo give each state back.
fn run(bus: &mut CommandBus, cmd: Command) {
    let start = bus.project().clone();
    bus.execute(cmd, Origin::User).expect("the edit applies");
    let edited = bus.project().clone();
    bus.undo().unwrap();
    assert_eq!(*bus.project(), start, "undo");
    bus.redo().unwrap();
    assert_eq!(*bus.project(), edited, "redo");
}

/// Every window keeps one origin and one fine offset, and each split plays on to the frame.
fn one_take(bus: &CommandBus, t: TrackId, ids: &[ClipId]) {
    let map = bus.project().tempo_map();
    let cs: Vec<Clip> = ids.iter().map(|i| clip(bus, t, *i)).collect();
    let origin = cs[0].start - cs[0].offset;
    for c in &cs {
        assert_eq!(c.start - c.offset, origin, "one origin: {:?}", c.id);
        assert_eq!(
            c.audio.as_ref().unwrap().slip_frames,
            cs[0].audio.as_ref().unwrap().slip_frames,
            "one fine offset"
        );
    }
    for p in cs.windows(2) {
        if p[0].end() == p[1].start && p[0].lane == p[1].lane {
            let held = cantus_core::warp::held_tempo(bus.project());
            assert!(!comp::is_seam(&p[0], &p[1], held), "a split, not a seam");
            let (x, y) = (
                frame(&map, &p[0], p[0].end().0 as f64),
                frame(&map, &p[1], p[1].start.0 as f64),
            );
            assert_eq!(x, y, "the split plays on");
        }
    }
}

/// A take on lane 1 from beat 8 (its count-in the first 8 beats), cut into three comp
/// windows on lane 0: an edit in the count-in and an edit inside the take keep the first
/// window's frame, and every window keeps the take's origin — no split jumps.
#[test]
fn a_take_cut_in_three_stays_whole_through_edits_before_and_inside_it() {
    let w = [
        (8 * B, 24 * B, 8 * B, 1, false),
        (8 * B, 8 * B, 8 * B, 0, false),
        (16 * B, 8 * B, 16 * B, 0, false),
        (24 * B, 8 * B, 24 * B, 0, false),
    ];
    let (mut bus, t, ids) = song(vec![pt(0, 120.0, false), pt(4, 120.0, false)], &w);
    let first = |bus: &CommandBus| {
        let c = clip(bus, t, ids[1]);
        frame(&bus.project().tempo_map(), &c, c.start.0 as f64)
    };
    let at = first(&bus);
    // Before the take: the count-in at 75.
    run(
        &mut bus,
        Command::SetTempoPoint {
            index: 1,
            point: pt(4, 75.0, true),
        },
    );
    assert!((first(&bus) - at).abs() <= 0.5, "the take's entry is kept");
    one_take(&bus, t, &ids);
    // Inside it: a ramp from beat 18 to 110 at beat 21.
    let at = first(&bus);
    run(
        &mut bus,
        Command::InsertTempoPoint {
            index: 2,
            point: TempoPoint {
                tick: Ticks(18 * B + 17),
                bpm: 75.0,
                ramp: true,
            },
        },
    );
    run(
        &mut bus,
        Command::InsertTempoPoint {
            index: 3,
            point: pt(21, 110.0, false),
        },
    );
    assert!((first(&bus) - at).abs() <= 0.5, "the take's entry is kept");
    one_take(&bus, t, &ids);
}

/// The song's one tempo changed in the top bar (no tempo track): a take recorded from bar
/// 17 and split at bar 25 keeps its entry and its split.
#[test]
fn the_songs_tempo_keeps_a_split_take_whole() {
    let w = [
        (64 * B, 32 * B, 64 * B, 0, false),
        (96 * B, 32 * B, 96 * B, 0, false),
    ];
    let (mut bus, t, ids) = song(vec![], &w);
    let c = clip(&bus, t, ids[0]);
    let at = frame(&bus.project().tempo_map(), &c, c.start.0 as f64);
    run(&mut bus, Command::SetTempo { bpm: 101.3 });
    let c = clip(&bus, t, ids[0]);
    assert!((frame(&bus.project().tempo_map(), &c, c.start.0 as f64) - at).abs() <= 0.5);
    one_take(&bus, t, &ids);
}

/// A reversed take split in two (as `reanchor_reversed` cuts it, one tempo over it), then a
/// tempo point inside its right half: the frame it lands on (its end) is kept, and the
/// halves still play one reversed line.
#[test]
fn a_reversed_take_keeps_the_frame_it_lands_on_and_its_split() {
    let w = [
        (16 * B, 2 * B, 14 * B, 0, true),
        (18 * B, 2 * B, 12 * B, 0, true),
    ];
    let (mut bus, t, ids) = song(vec![pt(0, 120.0, false), pt(8, 120.0, false)], &w);
    let map = bus.project().tempo_map();
    let (l, r) = (clip(&bus, t, ids[0]), clip(&bus, t, ids[1]));
    assert_eq!(
        frame(&map, &l, l.end().0 as f64),
        frame(&map, &r, r.start.0 as f64),
        "one line to begin with"
    );
    let lands = frame(&map, &r, r.end().0 as f64);
    run(
        &mut bus,
        Command::InsertTempoPoint {
            index: 2,
            point: TempoPoint {
                tick: Ticks(19 * B + 5),
                bpm: 83.0,
                ramp: false,
            },
        },
    );
    let map = bus.project().tempo_map();
    let (l, r) = (clip(&bus, t, ids[0]), clip(&bus, t, ids[1]));
    assert!(
        (frame(&map, &r, r.end().0 as f64) - lands).abs() <= 0.5,
        "it lands where it did"
    );
    let (x, y) = (
        frame(&map, &l, l.end().0 as f64),
        frame(&map, &r, r.start.0 as f64),
    );
    assert!((x - y).abs() <= 1.0, "the split plays on: {x} then {y}");
}

/// A looping window repeats its period in ticks: an edit in the intro leaves it as it is
/// (each repetition plays what it did; only the first's hidden part reaches the intro).
#[test]
fn a_looping_window_keeps_its_ticks() {
    let (mut bus, t, ids) = song(
        vec![pt(0, 100.0, false), pt(32, 120.0, false)],
        &[(32 * B, 16 * B, B / 2, 0, false)],
    );
    bus.execute(
        Command::SetClipLoop {
            track: t,
            clip: ids[0],
            content_length: Some(Ticks::beats(4)),
        },
        Origin::User,
    )
    .unwrap();
    let was = clip(&bus, t, ids[0]);
    run(
        &mut bus,
        Command::SetTempoPoint {
            index: 0,
            point: pt(0, 90.0, false),
        },
    );
    assert_eq!(clip(&bus, t, ids[0]), was);
}

/// A window whose file is gone from the pool plays nothing: a tempo edit is never refused
/// for it.
#[test]
fn a_window_without_its_file_never_refuses_a_tempo_edit() {
    let (mut bus, t, ids) = song(
        vec![pt(0, 100.0, false), pt(8, 120.0, false)],
        &[(16 * B, 2 * B, 13 * B + 7, 0, false)],
    );
    let mut c = clip(&bus, t, ids[0]);
    c.id = ClipId(bus.alloc_id());
    c.start = Ticks(20 * B);
    c.audio.as_mut().unwrap().source = AudioSourceId(9_998);
    bus.execute(
        Command::InsertClip {
            track: t,
            clip: Box::new(c),
        },
        Origin::User,
    )
    .unwrap();
    run(
        &mut bus,
        Command::SetTempoPoint {
            index: 0,
            point: pt(0, 87.0, true),
        },
    );
}

/// An alternate comp's window keeps its frame through a re-anchor that leaves a rest: its
/// own fine offset moves.
#[test]
fn an_alternate_comps_window_keeps_its_frame_with_its_fine_offset() {
    let (mut bus, t, _) = song(vec![pt(0, 120.0, false), pt(4, 120.0, false)], &[]);
    let src = bus.project().audio_pool[0].id;
    let mut win = bus.new_clip("b", Ticks::beats(12), Ticks::beats(2));
    win.audio = Some(AudioRegion::of(src));
    win.audio.as_mut().unwrap().slip_frames = 11;
    win.offset = Ticks(12 * B - 3);
    win.comp = true;
    let comps = vec![
        Comp {
            name: "A".into(),
            windows: Vec::new(),
        },
        Comp {
            name: "B".into(),
            windows: vec![win],
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
    let at = |bus: &CommandBus| {
        let c = bus.project().track(t).unwrap().comps[1].windows[0].clone();
        (frame(&bus.project().tempo_map(), &c, c.start.0 as f64), c)
    };
    let (before, _) = at(&bus);
    run(
        &mut bus,
        Command::SetTempoPoint {
            index: 1,
            point: pt(4, 61.7, true),
        },
    );
    let (after, c) = at(&bus);
    assert!((after - before).abs() <= 0.5, "{before} became {after}");
    assert_ne!(
        c.audio.as_ref().unwrap().slip_frames,
        11,
        "the rest is in its fine offset"
    );
}

// --- Gestures (review 4: findings 1, 8, 16, 23, 33, 41, 47; 2, 14, 19, 26, 48; 0, 9, 17,
// 24, 32, 44) ---------------------------------------------------------------------------

/// A trimmed window, a linked trimmed window and a split take, at the song's one tempo.
fn gesture_song() -> (CommandBus, TrackId) {
    let w = [
        (16 * B, 4 * B, 19, 0, false),
        (24 * B, 4 * B, 240, 0, false),
        (32 * B, 8 * B, 32 * B, 0, false),
        (40 * B, 8 * B, 40 * B, 0, false),
    ];
    let (mut bus, t, ids) = song(vec![], &w);
    bus.execute(
        Command::SetClipLink {
            track: t,
            clip: ids[1],
            link: Some(LinkId(9_999)),
        },
        Origin::User,
    )
    .unwrap();
    (bus, t)
}

fn windows(bus: &CommandBus, t: TrackId) -> Vec<(Ticks, Ticks, i32)> {
    bus.project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .map(|c| (c.start, c.offset, c.audio.as_ref().unwrap().slip_frames))
        .collect()
}

/// A hundred frames of a tempo drag (0.2 BPM a frame, as the top bar sends them) leave the
/// windows where one edit to the last tempo puts them, and the step holds one fix.
#[test]
fn a_hundred_frame_drag_is_one_edit() {
    let (mut bus, t) = gesture_song();
    let (mut once, _) = gesture_song();
    let key = MergeKey::of(&"tempo");
    for i in 1..=100 {
        let bpm = 120.0 + 0.2 * f64::from(i);
        bus.execute_merging(Command::SetTempo { bpm }, Origin::User, key)
            .unwrap();
    }
    bus.end_gesture();
    once.execute(Command::SetTempo { bpm: 140.0 }, Origin::User)
        .unwrap();
    assert_eq!(windows(&bus, t), windows(&once, t));
    let steps = bus.history().len();
    assert_eq!(steps, once.history().len(), "one step");
    let size = |c: &Command| match c {
        Command::Batch { commands, .. } => commands
            .iter()
            .map(|c| match c {
                Command::Batch { commands, .. } => commands.len(),
                _ => 1,
            })
            .sum(),
        _ => 1,
    };
    let top = &bus.history()[steps - 1];
    assert!(size(&top.command) <= 1 + 2 * 4, "one fix, not a hundred");
    // Undo gives the song back; redo the drag.
    let done = bus.project().clone();
    bus.undo().unwrap();
    assert_eq!(windows(&bus, t), windows(&gesture_song().0, t));
    bus.redo().unwrap();
    assert_eq!(*bus.project(), done);
}

/// A drag that goes up fast and comes back slowly to the tempo it started at — the top
/// bar's tempo, a tempo point on the lane — leaves every window as it was, to the bit
/// (measured frame by frame, the slow steps were each under a tick and never moved back).
#[test]
fn a_drag_back_to_the_start_leaves_everything_as_it_was() {
    let (mut bus, _) = gesture_song();
    let start = bus.project().clone();
    let key = MergeKey::of(&"tempo");
    let up = (1..=10).map(|i| 120.0 + 2.0 * f64::from(i));
    let down = (0..200).rev().map(|i| 120.0 + 0.1 * f64::from(i));
    for bpm in up.chain(down) {
        bus.execute_merging(Command::SetTempo { bpm }, Origin::User, key)
            .unwrap();
    }
    assert!(
        bus.project().tracks == start.tracks,
        "the windows as they were"
    );
    assert_eq!(bus.project().tempo_bpm, 120.0);
    // The lane: point 1 dragged down and back.
    let (mut bus, _) = gesture_song();
    bus.execute(
        Command::SetTempoTrack {
            bpm: 120.0,
            points: vec![pt(0, 120.0, false), pt(8, 120.0, true), pt(12, 96.0, false)],
        },
        Origin::User,
    )
    .unwrap();
    let start = bus.project().clone();
    let key = MergeKey::of(&("tempo-point", 1u64));
    let down = (1..=8).map(|i| 120.0 - 3.0 * f64::from(i));
    let back = (0..80).rev().map(|i| 120.0 - 0.3 * f64::from(i));
    for bpm in down.chain(back) {
        let point = pt(8, bpm, true);
        bus.execute_merging(
            Command::SetTempoPoint { index: 1, point },
            Origin::User,
            key,
        )
        .unwrap();
    }
    assert!(*bus.project() == start, "the song as it was");
}

/// A controller gesture that moved the tempo, then widened by a second knob: undo gives the
/// song back, redo gives back what the gesture did.
#[test]
fn a_widened_tempo_gesture_redoes_what_it_did() {
    let (mut bus, _) = gesture_song();
    let start = bus.project().clone();
    let (a, ab) = (MergeKey::of(&"knob a"), MergeKey::of(&"knobs a b"));
    for i in 1..=20 {
        let bpm = 120.0 - 0.7 * f64::from(i);
        bus.execute_merging(Command::SetTempo { bpm }, Origin::User, a)
            .unwrap();
    }
    let both = |pan: f32| Command::Batch {
        label_key: "cmd.controllers.move".into(),
        commands: vec![
            Command::SetTempo { bpm: 106.0 },
            Command::SetMasterPan { pan },
        ],
    };
    bus.widen_merging(both(0.1), Origin::User, a, ab).unwrap();
    bus.execute_merging(both(0.3), Origin::User, ab).unwrap();
    let done = bus.project().clone();
    assert_ne!(done.tracks, start.tracks, "the gesture moved the windows");
    bus.undo().unwrap();
    assert!(*bus.project() == start, "undo");
    bus.redo().unwrap();
    assert!(*bus.project() == done, "redo is what the gesture did");
    // And it equals the last frame as one edit.
    let (mut once, _) = gesture_song();
    once.execute(both(0.3), Origin::User).unwrap();
    assert!(once.project().tracks == done.tracks, "one edit");
}

/// Review 5 (findings 0, 9, 20): a take cut in three whose middle piece is linked (a copy
/// made with *Link copies* on) through two tempo edits, the first leaving a rest: the
/// linked piece, moved by whole ticks, stays in its take, and every seam plays on within
/// half a tick (and a frame).
#[test]
fn a_linked_piece_stays_in_its_take_through_two_edits() {
    let w = [
        (8 * B, 24 * B, 8 * B, 1, false),
        (8 * B, 8 * B, 8 * B, 0, false),
        (16 * B, 8 * B, 16 * B, 0, false),
        (24 * B, 8 * B, 24 * B, 0, false),
    ];
    let (mut bus, t, ids) = song(vec![], &w);
    bus.execute(
        Command::SetClipLink {
            track: t,
            clip: ids[2],
            link: Some(LinkId(9_999)),
        },
        Origin::User,
    )
    .unwrap();
    let first = |bus: &CommandBus| {
        let c = clip(bus, t, ids[1]);
        frame(&bus.project().tempo_map(), &c, c.start.0 as f64)
    };
    let at = first(&bus);
    for bpm in [121.3, 128.0] {
        run(&mut bus, Command::SetTempo { bpm });
        let map = bus.project().tempo_map();
        assert!((first(&bus) - at).abs() <= 0.5, "{bpm}: the take's entry");
        let half_tick = 0.5 * 60.0 / (bpm * 960.0) * 48_000.0;
        for pair in [(ids[1], ids[2]), (ids[2], ids[3])] {
            let (l, r) = (clip(&bus, t, pair.0), clip(&bus, t, pair.1));
            let (x, y) = (
                frame(&map, &l, l.end().0 as f64),
                frame(&map, &r, r.start.0 as f64),
            );
            assert!(
                (x - y).abs() <= half_tick + 1.0,
                "{bpm}: the seam at {:?} plays {x} then {y}",
                l.end()
            );
        }
    }
}

/// Review 5 (finding 12): a top-bar drag over a take sliced into 2 000 windows (one
/// track, one origin) — sixty merged frames, every window re-anchored each frame — stays
/// within a frame's budget a frame, best of three.
#[test]
fn a_drag_over_two_thousand_windows_keeps_to_its_budget() {
    let w: Vec<W> = (0..2000)
        .map(|i| (64 * B + i * 240, 240, 64 * B + i * 240, 0, false))
        .collect();
    let mut best = f64::MAX;
    for _ in 0..3 {
        let (mut bus, _, _) = song(vec![], &w);
        let key = MergeKey::of(&"tempo");
        let t0 = std::time::Instant::now();
        for i in 1..=60 {
            let bpm = 120.0 + 0.37 * f64::from(i);
            bus.execute_merging(Command::SetTempo { bpm }, Origin::User, key)
                .unwrap();
        }
        best = best.min(t0.elapsed().as_secs_f64() / 60.0);
    }
    println!("a drag frame over 2 000 windows: {:.2} ms", best * 1e3);
    assert!(best < 0.003, "{:.2} ms a frame", best * 1e3);
}

/// Review 6 (findings 0, 4, 10): a linked piece's rounding is half a tick where its offset
/// is rounded — at its content origin — so a take whose origin lies in a slower section
/// keeps its linked piece through two point edits: the seam plays on within half a tick at
/// the origin (and a frame).
#[test]
fn a_linked_piece_stays_in_its_take_when_its_origin_is_slower() {
    for (points, take, split, edits) in [
        (
            vec![pt(0, 60.0, false), pt(16, 120.0, false)],
            (8 * B, 7 * B),
            24 * B,
            [(0usize, pt(0, 61.4, false)), (1, pt(16, 110.0, false))],
        ),
        (
            vec![pt(0, 80.0, false), pt(32, 140.0, false)],
            (0, 0),
            48 * B,
            [(1, pt(32, 141.0, false)), (1, pt(32, 142.0, false))],
        ),
    ] {
        let (start, origin) = take;
        let w = [
            (start, split - start, start - origin, 0, false),
            (split, 8 * B, split - origin, 0, false),
        ];
        let (mut bus, t, ids) = song(points, &w);
        bus.execute(
            Command::SetClipLink {
                track: t,
                clip: ids[1],
                link: Some(LinkId(9_999)),
            },
            Origin::User,
        )
        .unwrap();
        for (index, point) in edits {
            run(&mut bus, Command::SetTempoPoint { index, point });
            let map = bus.project().tempo_map();
            let o = origin as f64;
            let half_tick = 0.5 * map.seconds_between_f64(o, o + 1.0) * RATE;
            let (l, r) = (clip(&bus, t, ids[0]), clip(&bus, t, ids[1]));
            let (x, y) = (
                frame(&map, &l, l.end().0 as f64),
                frame(&map, &r, r.start.0 as f64),
            );
            assert!(
                (x - y).abs() <= half_tick + 1.0,
                "{:?}: the seam plays {x} then {y}",
                point
            );
        }
    }
}

/// Review 6 (findings 2, 5): a take whose first piece is linked is laid on its unlinked
/// pieces' line, never on the linked one's rounding: through six tempo edits the unlinked
/// piece plays what it would with no link (within a frame or two of rounding).
#[test]
fn a_linked_first_piece_never_drags_its_take() {
    let w = [
        (16 * B, 16 * B, 16 * B - 3 * B / 2, 0, false),
        (32 * B, 32 * B, 32 * B - 3 * B / 2, 0, false),
    ];
    let (mut linked, t, ids) = song(vec![], &w);
    let (mut plain, _, _) = song(vec![], &w);
    linked
        .execute(
            Command::SetClipLink {
                track: t,
                clip: ids[0],
                link: Some(LinkId(9_999)),
            },
            Origin::User,
        )
        .unwrap();
    let at = |bus: &CommandBus| {
        let c = clip(bus, t, ids[1]);
        frame(&bus.project().tempo_map(), &c, c.start.0 as f64)
    };
    for bpm in [121.3, 122.7, 119.1, 125.9, 117.3, 128.6] {
        for bus in [&mut linked, &mut plain] {
            bus.execute(Command::SetTempo { bpm }, Origin::User)
                .unwrap();
        }
        let (a, b) = (at(&linked), at(&plain));
        assert!(
            (a - b).abs() <= 2.0,
            "{bpm}: {a} with a linked head, {b} without"
        );
    }
}

/// Final review (finding A1): laying a large selection — 4 000 windows of one take, every
/// other one linked, as a drag preview lays it across a tempo change — stays within its
/// budget (best of three): the linked pieces join their take by line in one sorted pass,
/// not each against every window. The take still moves as one.
#[test]
fn laying_a_large_selection_keeps_to_its_budget() {
    use cantus_core::audio_time::{lay_chains, Member};
    let mut p = Project::new("big");
    p.tempo_points = vec![pt(0, 120.0, false), pt(8, 100.0, false)];
    let sid = AudioSourceId(p.alloc_id());
    let clips: Vec<Clip> = (0..4000i64)
        .map(|i| {
            let mut c = p.make_audio_clip("w", sid, Ticks(1500 + i * 240), Ticks(240));
            c.offset = Ticks(1500 + i * 240);
            if i % 2 == 1 {
                c.link = Some(LinkId(9_999));
                // Off the line by a few ticks of rounding, as a re-anchor leaves it.
                c.offset += Ticks(i % 3 - 1);
            }
            c
        })
        .collect();
    let members: Vec<Member> = clips
        .iter()
        .map(|c| Member {
            clip: c,
            row: 0,
            from: c.start,
            rate: RATE,
        })
        .collect();
    let map = p.tempo_map();
    let mut best = f64::MAX;
    let mut laid = Vec::new();
    for _ in 0..3 {
        let t0 = std::time::Instant::now();
        laid = lay_chains(&map, &map, Ticks::beats(16), &members);
        best = best.min(t0.elapsed().as_secs_f64());
    }
    println!("laying 4 000 windows: {:.2} ms", best * 1e3);
    assert!(
        laid.iter().all(Option::is_some),
        "one take, every window laid with it"
    );
    assert!(best < 0.05, "{:.1} ms", best * 1e3);
}
