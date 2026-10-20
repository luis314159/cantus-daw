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
    let held = warp::held_tempo(bus.project());
    assert!(comp::seam_window(&c, held));
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
    assert!(!comp::seam_window(&c, held));
}

/// Round 29's review: a warp that does not follow the tempo and does not transpose plays
/// the source at its natural speed — under a tempo map that moves too — so the snapshot
/// drops it (the source's own bits, no render); transposed, it stays.
#[test]
fn a_warp_that_does_not_follow_is_no_warp_under_any_tempo() {
    let (mut bus, t, a, _) = setup();
    let mut w = stretched();
    w.follow = false;
    bus.execute(
        Command::SetClipWarp {
            track: t,
            clip: a,
            warp: Some(Box::new(w.clone())),
            op: WarpOp::Set,
        },
        Origin::User,
    )
    .unwrap();
    let mut p = bus.project().clone();
    p.tempo_points = vec![
        tempo::TempoPoint {
            tick: Ticks::ZERO,
            bpm: 100.0,
            ramp: true,
        },
        tempo::TempoPoint {
            tick: Ticks::beats(8),
            bpm: 140.0,
            ramp: false,
        },
    ];
    let snap = Snapshot::compile(&p, 1);
    assert!(snap.track(t).unwrap().audio[0].warp.is_none());
    w.cents = -300;
    p.track_mut(t)
        .unwrap()
        .clip_mut(a)
        .unwrap()
        .audio
        .as_mut()
        .unwrap()
        .warp = Some(Box::new(w));
    let snap = Snapshot::compile(&p, 2);
    assert!(snap.track(t).unwrap().audio[0].warp.is_some());
}

/// The clip `out` inserts (its commands' `InsertClip`).
fn inserted(out: &clipboard::Pasted) -> Clip {
    out.commands
        .iter()
        .find_map(|c| match c {
            Command::InsertClip { clip, .. } => Some((**clip).clone()),
            _ => None,
        })
        .expect("a clip pasted")
}

/// Round 29's review: a warped clip that follows the tempo is measured in content ticks,
/// so a paste keeps its offset, length and fades whatever the tempo where it lands —
/// across projects at other tempos and inside one under a tempo track.
#[test]
fn a_pasted_warped_clip_keeps_its_ticks_at_any_tempo() {
    let (mut bus, t, a, _) = setup();
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
    let mut region = bus
        .project()
        .track(t)
        .unwrap()
        .clip(a)
        .unwrap()
        .audio
        .clone()
        .unwrap();
    region.fade_in = Ticks(240);
    region.fade_out = Ticks(480);
    bus.execute(
        Command::SetClipAudio {
            track: t,
            clip: a,
            audio: Some(region),
        },
        Origin::User,
    )
    .unwrap();
    let was = bus.project().track(t).unwrap().clip(a).unwrap().clone();
    let source = was.audio.as_ref().unwrap().source;
    let board = clipboard::Clipboard::new(
        clipboard::ClipOrigin::default(),
        bus.project(),
        clipboard::copy_clips(bus.project(), &[(t, a)]),
    );
    let mut plans = std::collections::HashMap::new();
    plans.insert(source, clipboard::SourcePlan::Reuse(source));
    let check = |c: &Clip, how: &str| {
        let (x, y) = (c.audio.as_ref().unwrap(), was.audio.as_ref().unwrap());
        assert_eq!(
            (c.offset, c.length, x.fade_in, x.fade_out),
            (was.offset, was.length, y.fade_in, y.fade_out),
            "{how}"
        );
        assert_eq!(x.warp, y.warp, "{how}");
    };
    // Into another project at 90 BPM.
    let mut other = bus.project().clone();
    other.tempo_bpm = 90.0;
    let mut n = 10_000u64;
    let out = clipboard::paste_clips(
        &other,
        &board,
        false,
        0,
        Ticks::beats(16),
        &plans,
        false,
        &mut || {
            n += 1;
            n
        },
    );
    check(&inserted(&out), "across tempos");
    // Inside one project, from a 120 section into a 60 one.
    let mut mapped = bus.project().clone();
    mapped.tempo_points = vec![
        tempo::TempoPoint {
            tick: Ticks::ZERO,
            bpm: 120.0,
            ramp: false,
        },
        tempo::TempoPoint {
            tick: Ticks::beats(16),
            bpm: 60.0,
            ramp: false,
        },
    ];
    let board = clipboard::Clipboard::new(
        clipboard::ClipOrigin::default(),
        &mapped,
        clipboard::copy_clips(&mapped, &[(t, a)]),
    );
    let out = clipboard::paste_clips(
        &mapped,
        &board,
        true,
        0,
        Ticks::beats(24),
        &plans,
        false,
        &mut || {
            n += 1;
            n
        },
    );
    check(&inserted(&out), "under a tempo track");
}

/// Round 29's review: a warp that is not valid is refused by every command that can carry
/// one — `InsertClip` and `SetClipAudio` as well as `SetClipWarp`.
#[test]
fn an_invalid_warp_is_refused_on_every_way_in() {
    let (mut bus, t, a, _) = setup();
    let mut zero = stretched();
    zero.rate = 0;
    let mut backwards = stretched();
    backwards.markers.reverse();
    for bad in [zero, backwards] {
        assert!(!bad.is_valid());
        let mut region = bus
            .project()
            .track(t)
            .unwrap()
            .clip(a)
            .unwrap()
            .audio
            .clone()
            .unwrap();
        region.warp = Some(Box::new(bad.clone()));
        let before = bus.project().clone();
        let r = bus.execute(
            Command::SetClipAudio {
                track: t,
                clip: a,
                audio: Some(region.clone()),
            },
            Origin::User,
        );
        assert!(r.is_err(), "SetClipAudio took {bad:?}");
        let mut c = bus.new_clip("bad", Ticks::beats(20), Ticks::beats(4));
        c.audio = Some(region);
        let r = bus.execute(
            Command::InsertClip {
                track: t,
                clip: Box::new(c),
            },
            Origin::User,
        );
        assert!(r.is_err(), "InsertClip took {bad:?}");
        assert_eq!(bus.project().tracks, before.tracks);
    }
}

/// Round 29's review: a comp window whose warp plays the source as it is (not following
/// the tempo, not transposed) keeps its seam's crossfade — nothing is rendered there.
#[test]
fn a_warp_that_plays_the_source_keeps_the_comp_seam() {
    let (mut bus, t, a, _) = setup();
    let mut w = stretched();
    w.follow = false;
    w.mode = WarpMode::Complex;
    bus.execute(
        Command::SetClipWarp {
            track: t,
            clip: a,
            warp: Some(Box::new(w.clone())),
            op: WarpOp::Set,
        },
        Origin::User,
    )
    .unwrap();
    let c = bus.project().track(t).unwrap().clip(a).unwrap().clone();
    let held = warp::held_tempo(bus.project());
    assert!(comp::seam_window(&c, held));
    w.cents = 200;
    let mut c2 = c.clone();
    c2.audio.as_mut().unwrap().warp = Some(Box::new(w));
    assert!(!comp::seam_window(&c2, held), "a transposed window renders");
}

/// The content tick a clip plays at timeline tick `t` (a reversed one reads its window
/// backwards), and the source frame its warp hears there.
fn heard_frame(c: &Clip, t: Ticks) -> f64 {
    let a = c.audio.as_ref().unwrap();
    let w = a.warp.as_ref().unwrap();
    let content = if a.reverse {
        c.offset.0 + (c.end().0 - t.0)
    } else {
        c.offset.0 + (t.0 - c.start.0)
    };
    w.frame_at(content as f64)
}

/// Round 29's review: a warped clip split at a tick that is no marker — forwards and
/// reversed — gives two halves that hear what the whole did, with valid warps.
#[test]
fn a_split_warped_clip_plays_what_the_whole_did() {
    for reverse in [false, true] {
        let (mut bus, t, a, _) = setup();
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
        if reverse {
            let mut r = bus
                .project()
                .track(t)
                .unwrap()
                .clip(a)
                .unwrap()
                .audio
                .clone()
                .unwrap();
            r.reverse = true;
            bus.execute(
                Command::SetClipAudio {
                    track: t,
                    clip: a,
                    audio: Some(r),
                },
                Origin::User,
            )
            .unwrap();
        }
        let whole = bus.project().track(t).unwrap().clip(a).unwrap().clone();
        let at = Ticks::beats(3) + Ticks(77);
        let mut next = bus.project().next_id + 1_000;
        let cmds = clipboard::split(bus.project(), t, &whole, at, &mut || {
            next += 1;
            next
        })
        .unwrap();
        bus.execute(
            Command::Batch {
                label_key: "cmd.split".into(),
                commands: cmds,
            },
            Origin::User,
        )
        .unwrap();
        let halves: Vec<Clip> = bus
            .project()
            .track(t)
            .unwrap()
            .clips
            .iter()
            .filter(|c| c.audio.is_some())
            .cloned()
            .collect();
        assert_eq!(halves.len(), 2);
        for h in &halves {
            assert!(h.audio.as_ref().unwrap().warp.as_ref().unwrap().is_valid());
            for k in 0..8 {
                let tick = h.start + Ticks(h.length.0 * k / 8);
                let d = heard_frame(h, tick) - heard_frame(&whole, tick);
                assert!(d.abs() < 1e-6, "reverse {reverse}, {tick:?}: {d}");
            }
        }
    }
}

/// Round 29's second review (finding 8): a following warp that maps exactly as the unwarped
/// window under the one tempo held plays the source (the snapshot drops it), so its seam
/// keeps the crossfade — one rule for both; under a tempo that changes it renders, and
/// takes none.
#[test]
fn an_identity_following_warp_keeps_the_comp_seam_under_one_tempo() {
    let (mut bus, t, a, _) = setup();
    let c = bus.project().track(t).unwrap().clip(a).unwrap().clone();
    let w = Warp::natural_of(
        &bus.project().tempo_map(),
        c.start,
        c.offset,
        c.length,
        0,
        44_100,
    );
    assert!(w.follow && w.is_identity(bus.project().tempo_bpm, 0));
    bus.execute(
        Command::SetClipWarp {
            track: t,
            clip: a,
            warp: Some(Box::new(w)),
            op: WarpOp::Set,
        },
        Origin::User,
    )
    .unwrap();
    let p = bus.project();
    let c = p.track(t).unwrap().clip(a).unwrap().clone();
    let held = warp::held_tempo(p);
    assert!(held.is_some());
    let snap = Snapshot::compile(p, 2);
    assert!(
        snap.track(t).unwrap().audio[0].warp.is_none(),
        "it plays the source"
    );
    assert!(comp::seam_window(&c, held), "so it takes its seam");
    // Under a tempo that changes, the same warp renders and takes none.
    assert!(!comp::seam_window(&c, None));
}

/// Round 29's second review (finding 40): a track inserted with a clip whose warp
/// `SetClipWarp` refuses is refused, and so is a kept comp window holding one.
#[test]
fn an_invalid_warp_never_comes_in_with_a_track_or_a_comp() {
    let (mut bus, t, a, _) = setup();
    let mut bad = stretched();
    bad.rate = 0;
    let mut clip = bus.project().track(t).unwrap().clip(a).unwrap().clone();
    clip.audio.as_mut().unwrap().warp = Some(Box::new(bad.clone()));
    let mut track = bus.new_track("more");
    clip.id = ClipId(bus.project().next_id + 500);
    track.clips.push(clip.clone());
    let before = bus.project().clone();
    let r = bus.execute(
        Command::InsertTrack {
            index: 0,
            track: Box::new(track),
        },
        Origin::User,
    );
    assert!(r.is_err(), "InsertTrack took a bad warp");
    let mut kept = clip;
    kept.comp = true;
    kept.id = ClipId(bus.project().next_id + 600);
    let r = bus.execute(
        Command::SetTrackComps {
            track: t,
            comps: vec![
                comp::Comp {
                    name: "A".into(),
                    windows: Vec::new(),
                },
                comp::Comp {
                    name: "B".into(),
                    windows: vec![kept],
                },
            ],
            active: 0,
        },
        Origin::User,
    );
    assert!(r.is_err(), "SetTrackComps took a bad warp");
    assert_eq!(bus.project().tracks, before.tracks);
}
