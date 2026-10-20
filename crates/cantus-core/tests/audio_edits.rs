//! Review 2 (findings 0, 3, 43): an unwarped audio clip's edits under a tempo track. Its
//! `offset` is read at the tempo where it starts (ADR 0060 §5), so an edit that moves its
//! start — a split's right half, a reversed clip's halves, a time-range crop — re-derives
//! the offset in seconds and carries the rounding into `slip_frames`: every piece plays
//! the frames the whole clip played there, to within one frame.

use cantus_core::audio_time;
use cantus_core::clipboard;
use cantus_core::tempo::TempoPoint;
use cantus_core::*;

const RATE: f64 = 48_000.0;

fn run(bus: &mut CommandBus, commands: Vec<Command>) {
    bus.execute(
        Command::Batch {
            label_key: "edit".into(),
            commands,
        },
        Origin::User,
    )
    .expect("the batch applies");
}

fn point(beat: i64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick: Ticks::beats(beat),
        bpm,
        ramp,
    }
}

/// One track, the tempo `points`, and an audio clip from beat 0 to beat 16 on a 48 kHz
/// source (reversed with `reverse`), offset `offset`, slip 37 frames.
fn fixture(points: Vec<TempoPoint>, offset: Ticks, reverse: bool) -> (CommandBus, TrackId, ClipId) {
    let mut bus = CommandBus::default();
    let t = bus.new_track("audio");
    let tid = t.id;
    let sid = AudioSourceId(bus.alloc_id());
    let mut c = bus.new_clip("wave", Ticks::ZERO, Ticks::beats(16));
    let mut region = AudioRegion::of(sid);
    region.reverse = reverse;
    region.slip_frames = 37;
    c.audio = Some(region);
    c.offset = offset;
    let cid = c.id;
    let bpm = points.first().map_or(120.0, |p| p.bpm);
    run(
        &mut bus,
        vec![
            Command::InsertTrack {
                index: 0,
                track: Box::new(t),
            },
            Command::SetTempoTrack { bpm, points },
            Command::InsertAudioSource {
                source: AudioSource {
                    id: sid,
                    name: "take".into(),
                    path: "C:/take.wav".into(),
                    sample_rate: RATE as u32,
                    channels: 1,
                    frames: 4_800_000,
                    sha256: None,
                    recorded: true,
                },
            },
            Command::InsertClip {
                track: tid,
                clip: Box::new(c),
            },
        ],
    );
    (bus, tid, cid)
}

/// The source frame a clip plays at `tick`, as the engine plays it: forward from its start's
/// frame, or — reversed — backwards from its end.
fn frame(map: &tempo::TempoMap, c: &Clip, tick: f64) -> f64 {
    let a = c.audio.as_ref().unwrap();
    let slip = a.slip_frames as f64;
    if a.reverse {
        let first = audio_time::frame_at(map, c.start, c.offset, slip, RATE, c.start.0 as f64);
        first + map.seconds_between_f64(tick, c.end().0 as f64) * RATE
    } else {
        audio_time::frame_at(map, c.start, c.offset, slip, RATE, tick)
    }
}

fn clips(bus: &CommandBus, t: TrackId) -> Vec<Clip> {
    bus.project().track(t).unwrap().clips.clone()
}

/// Every piece of `after` plays, at the ticks it covers, what `whole` played there.
fn same_frames(map: &tempo::TempoMap, whole: &Clip, after: &[Clip]) {
    for c in after {
        let (a, b) = (c.start.0 as f64, c.end().0 as f64);
        for t in [a, a + 0.25 * (b - a), 0.5 * (a + b), b - 1.0] {
            let (want, got) = (frame(map, whole, t), frame(map, c, t));
            assert!(
                (want - got).abs() <= 1.0,
                "clip {:?} at tick {t}: the whole clip played frame {want}, the piece {got}",
                c.id
            );
        }
    }
}

fn split(bus: &CommandBus, t: TrackId, c: &Clip, at: Ticks) -> Vec<Command> {
    let mut next = bus.project().next_id;
    let mut ids = || {
        next += 1;
        next - 1
    };
    clipboard::split(bus.project(), t, c, at, &mut ids).expect("inside the clip")
}

fn split_case(points: Vec<TempoPoint>, offset: Ticks, reverse: bool, at: Ticks) {
    let (mut bus, t, c) = fixture(points, offset, reverse);
    let whole = bus.project().track(t).unwrap().clip(c).unwrap().clone();
    let map = bus.project().tempo_map();
    let cmds = split(&bus, t, &whole, at);
    run(&mut bus, cmds);
    let after = clips(&bus, t);
    assert_eq!(after.len(), 2);
    same_frames(&map, &whole, &after);
    // One undo gives the clip back exactly.
    bus.undo().unwrap();
    assert_eq!(clips(&bus, t), vec![whole]);
}

#[test]
fn a_split_across_a_tempo_change_plays_on_without_a_jump() {
    let pts = vec![point(0, 120.0, false), point(8, 60.0, false)];
    split_case(pts.clone(), Ticks::ZERO, false, Ticks::beats(12));
    split_case(pts.clone(), Ticks::beats(3), false, Ticks(11_520 + 7));
    // Through a ramp.
    let ramp = vec![point(0, 90.0, false), point(4, 90.0, true), point(12, 140.0, false)];
    split_case(ramp, Ticks(333), false, Ticks(7_000));
}

#[test]
fn a_reversed_split_across_a_tempo_change_plays_on_without_a_jump() {
    let pts = vec![point(0, 120.0, false), point(8, 60.0, false)];
    split_case(pts.clone(), Ticks::ZERO, true, Ticks::beats(12));
    split_case(pts, Ticks::beats(2), true, Ticks::beats(5));
    let ramp = vec![point(0, 90.0, false), point(4, 90.0, true), point(12, 140.0, false)];
    split_case(ramp, Ticks(333), true, Ticks(7_000));
}

/// Inside one held tempo a split is the old tick arithmetic exactly (ADR 0025): the right
/// half's offset is the left's plus its length, the slip untouched.
#[test]
fn a_split_inside_one_tempo_keeps_the_tick_arithmetic() {
    let pts = vec![point(0, 120.0, false), point(14, 60.0, false)];
    let (bus, t, c) = fixture(pts, Ticks::beats(1), false);
    let whole = bus.project().track(t).unwrap().clip(c).unwrap().clone();
    let cmds = split(&bus, t, &whole, Ticks::beats(5));
    let right = cmds
        .iter()
        .find_map(|c| match c {
            Command::InsertClip { clip, .. } => Some(clip.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(right.offset, Ticks::beats(6));
    assert_eq!(right.audio.unwrap().slip_frames, 37);
}

/// A time-range cut is built on the split: what stays on either side of the hole plays
/// what it played.
#[test]
fn a_time_range_cut_across_a_tempo_change_leaves_the_rest_in_place() {
    let pts = vec![point(0, 120.0, false), point(8, 60.0, false)];
    let (mut bus, t, c) = fixture(pts, Ticks::ZERO, false);
    let whole = bus.project().track(t).unwrap().clip(c).unwrap().clone();
    let map = bus.project().tempo_map();
    let mut next = bus.project().next_id;
    let mut ids = || {
        next += 1;
        next - 1
    };
    let crops = clipboard::time_crops(bus.project(), Ticks::beats(10), Ticks::beats(12), &mut ids);
    let cut: Vec<Command> = crops.into_iter().flat_map(|c| c.cut).collect();
    run(&mut bus, cut);
    let after = clips(&bus, t);
    assert_eq!(after.len(), 2, "{after:?}");
    same_frames(&map, &whole, &after);
}

/// A comp seam moved across a tempo change: the right window plays on what it played
/// (its offset re-derived where it starts), and undo and redo give back each state
/// exactly — the inverse carries the window's offset and fine offset.
#[test]
fn a_seam_moved_across_a_tempo_change_keeps_the_right_windows_audio() {
    let pts = vec![point(0, 120.0, false), point(8, 60.0, false)];
    let (mut bus, t, c) = fixture(pts, Ticks::ZERO, false);
    let whole = bus.project().track(t).unwrap().clip(c).unwrap().clone();
    let sid = AudioSourceId(bus.alloc_id());
    let mut src = bus.project().audio_pool[0].clone();
    src.id = sid;
    src.path = "C:/other.wav".into();
    // The left window: another take's audio to beat 6; the right: the fixture's clip from
    // beat 6 on.
    let mut left = bus.new_clip("left", Ticks::ZERO, Ticks::beats(6));
    left.audio = Some(AudioRegion::of(sid));
    left.comp = true;
    let lid = left.id;
    let cmds = vec![
        Command::InsertAudioSource { source: src },
        Command::SetClipComp {
            track: t,
            clip: c,
            comp: true,
        },
    ];
    run(&mut bus, cmds);
    let map = bus.project().tempo_map();
    let mut ids_next = bus.project().next_id;
    let mut ids = || {
        ids_next += 1;
        ids_next - 1
    };
    let crop = clipboard::split(bus.project(), t, &whole, Ticks::beats(6), &mut ids).unwrap();
    run(&mut bus, crop);
    let left_half = clips(&bus, t)
        .into_iter()
        .find(|x| x.id == c)
        .unwrap();
    run(
        &mut bus,
        vec![
            Command::RemoveClip { track: t, clip: left_half.id },
            Command::InsertClip {
                track: t,
                clip: Box::new(left),
            },
        ],
    );
    let right = clips(&bus, t).into_iter().find(|x| x.id != lid).unwrap();
    let before = bus.project().clone();
    run(
        &mut bus,
        vec![Command::SetCompSeam {
            track: t,
            left: lid,
            right: right.id,
            at: Ticks::beats(10),
            window: None,
        }],
    );
    let moved = clips(&bus, t).into_iter().find(|x| x.id == right.id).unwrap();
    assert_eq!(moved.start, Ticks::beats(10));
    same_frames(&map, &whole, &[moved]);
    let after = bus.project().clone();
    bus.undo().unwrap();
    assert_eq!(*bus.project(), before);
    bus.redo().unwrap();
    assert_eq!(*bus.project(), after);
}
