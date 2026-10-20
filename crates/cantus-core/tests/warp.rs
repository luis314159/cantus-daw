//! `SetClipWarp` (ADR 0061): exact inverse, refusals, a drag merged into one step, linked
//! copies sharing the warp, and what the snapshot hands the engine.

use cantus_core::warp::{Warp, WarpMode, WarpOp};
use cantus_core::*;

fn setup() -> (CommandBus, TrackId, ClipId, ClipId) {
    let mut bus = CommandBus::default();
    let t = bus.new_track("loop");
    let tid = t.id;
    bus.execute(
        Command::InsertTrack {
            index: 0,
            track: Box::new(t),
        },
        Origin::User,
    )
    .unwrap();
    let src = AudioSource {
        id: AudioSourceId(bus.project().next_id + 100),
        name: "drums".into(),
        path: "C:/nowhere/drums 92.wav".into(),
        sample_rate: 44_100,
        channels: 2,
        frames: 44_100 * 6,
        sha256: None,
        recorded: false,
    };
    let sid = src.id;
    bus.execute(Command::InsertAudioSource { source: src }, Origin::User)
        .unwrap();
    let mut au = bus.new_clip("drums", Ticks::ZERO, Ticks::beats(8));
    au.audio = Some(AudioRegion::of(sid));
    let aid = au.id;
    bus.execute(
        Command::InsertClip {
            track: tid,
            clip: Box::new(au),
        },
        Origin::User,
    )
    .unwrap();
    let notes = bus.new_clip("notes", Ticks::beats(8), Ticks::beats(4));
    let nid = notes.id;
    bus.execute(
        Command::InsertClip {
            track: tid,
            clip: Box::new(notes),
        },
        Origin::User,
    )
    .unwrap();
    (bus, tid, aid, nid)
}

fn warp_of(bus: &CommandBus, t: TrackId, c: ClipId) -> Option<Box<Warp>> {
    bus.project()
        .track(t)
        .and_then(|t| t.clip(c))
        .and_then(|c| c.audio.as_ref())
        .and_then(|a| a.warp.clone())
}

fn stretched() -> Warp {
    let w = Warp::identity(44_100, 120.0, 0)
        .add_marker(Ticks::beats(4))
        .unwrap();
    let mut w = w.move_marker(1, Ticks::beats(5));
    w.mode = WarpMode::Rhythm;
    w
}

#[test]
fn a_warp_is_set_and_undone_exactly() {
    let (mut bus, t, a, _) = setup();
    let before = bus.project().clone();
    bus.execute(
        Command::SetClipWarp {
            track: t,
            clip: a,
            warp: Some(Box::new(stretched())),
            op: WarpOp::Set,
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(warp_of(&bus, t, a).as_deref(), Some(&stretched()));
    assert_eq!(bus.undo_label_key(), Some("cmd.warp.set"));
    bus.undo().unwrap();
    assert_eq!(bus.project(), &before);
    bus.redo().unwrap();
    assert_eq!(warp_of(&bus, t, a).as_deref(), Some(&stretched()));
}

#[test]
fn a_warp_on_no_audio_or_not_valid_is_refused() {
    let (mut bus, t, a, n) = setup();
    let r = bus.execute(
        Command::SetClipWarp {
            track: t,
            clip: n,
            warp: Some(Box::new(stretched())),
            op: WarpOp::Set,
        },
        Origin::User,
    );
    assert!(r.is_err());
    let mut bad = stretched();
    bad.markers[1].tick = Ticks::ZERO;
    let r = bus.execute(
        Command::SetClipWarp {
            track: t,
            clip: a,
            warp: Some(Box::new(bad)),
            op: WarpOp::Set,
        },
        Origin::User,
    );
    assert!(r.is_err());
    assert!(warp_of(&bus, t, a).is_none());
}

#[test]
fn a_marker_drag_is_one_undo_step() {
    let (mut bus, t, a, _) = setup();
    let base = stretched();
    bus.execute(
        Command::SetClipWarp {
            track: t,
            clip: a,
            warp: Some(Box::new(base.clone())),
            op: WarpOp::Set,
        },
        Origin::User,
    )
    .unwrap();
    let key = MergeKey::of(&("warp", a));
    for k in 1..20 {
        let w = base.move_marker(1, Ticks::beats(5) + Ticks(k * 10));
        bus.execute_merging(
            Command::SetClipWarp {
                track: t,
                clip: a,
                warp: Some(Box::new(w)),
                op: WarpOp::MoveMarker,
            },
            Origin::User,
            key,
        )
        .unwrap();
    }
    bus.end_gesture();
    assert_eq!(
        warp_of(&bus, t, a).unwrap().markers[1].tick,
        Ticks::beats(5) + Ticks(190)
    );
    bus.undo().unwrap();
    assert_eq!(warp_of(&bus, t, a).as_deref(), Some(&base), "one step back");
}

#[test]
fn the_snapshot_carries_a_warp_unless_it_plays_as_the_source() {
    let (mut bus, t, a, _) = setup();
    let identity = Warp::identity(44_100, bus.project().tempo_bpm, 0);
    bus.execute(
        Command::SetClipWarp {
            track: t,
            clip: a,
            warp: Some(Box::new(identity)),
            op: WarpOp::Set,
        },
        Origin::User,
    )
    .unwrap();
    let snap = Snapshot::compile(bus.project(), 1);
    let clip = &snap.track(t).unwrap().audio[0];
    assert!(clip.warp.is_none(), "an identity warp is no warp");
    assert_eq!(clip.source_rate, 44_100);
    bus.execute(
        Command::SetClipWarp {
            track: t,
            clip: a,
            warp: Some(Box::new(stretched())),
            op: WarpOp::Set,
        },
        Origin::User,
    )
    .unwrap();
    let snap = Snapshot::compile(bus.project(), 2);
    let clip = &snap.track(t).unwrap().audio[0];
    assert_eq!(clip.warp.as_deref(), Some(&stretched()));
}

#[test]
fn a_warped_window_takes_no_comp_seam() {
    let (mut bus, t, a, _) = setup();
    let c = bus.project().track(t).unwrap().clip(a).unwrap().clone();
    assert!(comp::seam_window(&c));
    bus.execute(
        Command::SetClipWarp {
            track: t,
            clip: a,
            warp: Some(Box::new(stretched())),
            op: WarpOp::Set,
        },
        Origin::User,
    )
    .unwrap();
    let c = bus.project().track(t).unwrap().clip(a).unwrap().clone();
    assert!(!comp::seam_window(&c));
}
