//! Crossfades between overlapping clips and clip gain envelopes in the model (ADR 0070):
//! the automatic crossfade an edit makes (one undo step with it, never one an overlap
//! already had or lost), what the snapshot hands the engine, what split and the commands
//! keep or refuse.

use cantus_core::clip_gain::{GainEnvelope, GainPoint};
use cantus_core::crossfade::XfadeCurve;
use cantus_core::*;

fn source(p: &mut Project) -> AudioSourceId {
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "a".into(),
        path: "a.wav".into(),
        sample_rate: 48_000,
        channels: 2,
        frames: 48_000 * 60,
        sha256: None,
        recorded: false,
    });
    id
}

/// Two audio clips a bar apart on one track, the second with no crossfade.
fn two_clips() -> (CommandBus, TrackId, ClipId, ClipId) {
    let mut p = Project::new("x");
    let src = source(&mut p);
    let mut t = p.make_track("Vox");
    let a = p.make_audio_clip("a", src, Ticks::ZERO, Ticks::beats(4));
    let mut b = p.make_audio_clip("b", src, Ticks::beats(8), Ticks::beats(4));
    b.offset = Ticks::beats(8);
    let (ia, ib) = (a.id, b.id);
    t.clips.extend([a, b]);
    let tid = t.id;
    p.tracks.push(t);
    (CommandBus::new(p), tid, ia, ib)
}

fn clip(bus: &CommandBus, t: TrackId, c: ClipId) -> Clip {
    bus.project().track(t).unwrap().clip(c).unwrap().clone()
}

fn move_to(bus: &mut CommandBus, t: TrackId, c: ClipId, start: Ticks) {
    let len = clip(bus, t, c).length;
    bus.execute(
        Command::SetClipBounds {
            track: t,
            clip: c,
            start,
            length: len,
        },
        Origin::User,
    )
    .unwrap();
}

#[test]
fn an_edit_that_makes_an_overlap_crossfades_it_in_the_same_step() {
    let (mut bus, t, a, b) = two_clips();
    bus.set_auto_crossfades(Some(XfadeCurve::EqualPower));
    move_to(&mut bus, t, b, Ticks::beats(3));
    assert_eq!(
        clip(&bus, t, b).audio.unwrap().cross,
        Some(XfadeCurve::EqualPower)
    );
    assert_eq!(bus.history().len(), 1, "one step");
    // Undo takes both back; redo brings both.
    bus.undo();
    let back = clip(&bus, t, b);
    assert_eq!(back.start, Ticks::beats(8));
    assert_eq!(back.audio.unwrap().cross, None);
    bus.redo();
    let again = clip(&bus, t, b);
    assert_eq!(again.start, Ticks::beats(3));
    assert_eq!(again.audio.unwrap().cross, Some(XfadeCurve::EqualPower));
    let _ = a;
}

#[test]
fn with_automatic_crossfades_off_an_overlap_is_left_as_it_was() {
    let (mut bus, t, _, b) = two_clips();
    assert_eq!(bus.auto_crossfades(), None, "off on a bare bus");
    move_to(&mut bus, t, b, Ticks::beats(3));
    assert_eq!(clip(&bus, t, b).audio.unwrap().cross, None);
}

#[test]
fn a_crossfade_removed_stays_removed_while_the_overlap_stays() {
    let (mut bus, t, _, b) = two_clips();
    bus.set_auto_crossfades(Some(XfadeCurve::Linear));
    move_to(&mut bus, t, b, Ticks::beats(3));
    let mut a = clip(&bus, t, b).audio.unwrap();
    assert_eq!(a.cross, Some(XfadeCurve::Linear));
    a.cross = None;
    bus.execute(
        Command::SetClipAudio {
            track: t,
            clip: b,
            audio: Some(a),
        },
        Origin::User,
    )
    .unwrap();
    // Moved again, still overlapping the same clip: no new crossfade.
    move_to(&mut bus, t, b, Ticks::beats(2));
    assert_eq!(clip(&bus, t, b).audio.unwrap().cross, None);
}

#[test]
fn the_snapshot_crosses_an_overlap_over_its_whole_length_on_the_later_clips_curve() {
    let (mut bus, t, a, b) = two_clips();
    bus.set_auto_crossfades(Some(XfadeCurve::SCurve));
    move_to(&mut bus, t, b, Ticks::beats(3));
    let s = Snapshot::compile(bus.project(), 1);
    let audio = &s.tracks[0].audio;
    let left = audio.iter().find(|c| c.clip == a).unwrap();
    let right = audio.iter().find(|c| c.clip == b).unwrap();
    let stretch = Some((Ticks::beats(3), Ticks::beats(4)));
    assert_eq!(left.xout, stretch);
    assert_eq!(right.xin, stretch);
    assert_eq!(left.xout_curve, XfadeCurve::SCurve);
    assert_eq!(right.xin_curve, XfadeCurve::SCurve);
    // Neither window moves: an overlap is crossed where it is (a seam overlaps its two
    // windows; an overlap already is one).
    assert_eq!((left.start, left.end), (Ticks::ZERO, Ticks::beats(4)));
    assert_eq!((right.start, right.end), (Ticks::beats(3), Ticks::beats(7)));
}

#[test]
fn a_flat_envelope_is_no_envelope_in_the_snapshot() {
    let (mut bus, t, a, _) = two_clips();
    let mut r = clip(&bus, t, a).audio.unwrap();
    r.gain_env = Some(Box::new(GainEnvelope::new(vec![
        GainPoint::new(Ticks::ZERO, 0.0),
        GainPoint::new(Ticks::beats(1), 0.0),
    ])));
    bus.execute(
        Command::SetClipAudio {
            track: t,
            clip: a,
            audio: Some(r.clone()),
        },
        Origin::User,
    )
    .unwrap();
    let s = Snapshot::compile(bus.project(), 1);
    assert!(s.tracks[0].audio.iter().all(|c| c.gain_env.is_none()));
    r.gain_env.as_mut().unwrap().points[1].db = -6.0;
    bus.execute(
        Command::SetClipAudio {
            track: t,
            clip: a,
            audio: Some(r),
        },
        Origin::User,
    )
    .unwrap();
    let s = Snapshot::compile(bus.project(), 1);
    let c = s.tracks[0].audio.iter().find(|c| c.clip == a).unwrap();
    assert!(c.gain_env.is_some());
}

#[test]
fn a_bad_envelope_is_refused() {
    let (mut bus, t, a, _) = two_clips();
    let mut r = clip(&bus, t, a).audio.unwrap();
    r.gain_env = Some(Box::new(GainEnvelope {
        points: vec![
            GainPoint::new(Ticks(5), 0.0),
            GainPoint::new(Ticks(1), 99.0),
        ],
    }));
    assert!(bus
        .execute(
            Command::SetClipAudio {
                track: t,
                clip: a,
                audio: Some(r.clone()),
            },
            Origin::User,
        )
        .is_err());
    // Nor does a clip carrying one get in.
    let mut c = clip(&bus, t, a);
    c.id = ClipId(9_999);
    c.audio = Some(r);
    assert!(bus
        .execute(
            Command::InsertClip {
                track: t,
                clip: Box::new(c),
            },
            Origin::User,
        )
        .is_err());
}

#[test]
fn a_split_gives_both_halves_the_line_and_the_crossfade_to_the_left() {
    let (mut bus, t, _, b) = two_clips();
    bus.set_auto_crossfades(Some(XfadeCurve::EqualPower));
    move_to(&mut bus, t, b, Ticks::beats(3));
    let mut r = clip(&bus, t, b).audio.unwrap();
    let line = GainEnvelope::new(vec![GainPoint::new(Ticks::beats(9), -3.0)]);
    r.gain_env = Some(Box::new(line.clone()));
    bus.execute(
        Command::SetClipAudio {
            track: t,
            clip: b,
            audio: Some(r),
        },
        Origin::User,
    )
    .unwrap();
    let whole = clip(&bus, t, b);
    let mut next = 50_000u64;
    let mut ids = || {
        next += 1;
        next
    };
    let cmds =
        clipboard::split(bus.project(), t, &whole, Ticks::beats(5), &mut ids).expect("a split");
    bus.execute(
        Command::Batch {
            label_key: "cmd.split".into(),
            commands: cmds,
        },
        Origin::User,
    )
    .unwrap();
    let track = bus.project().track(t).unwrap();
    let halves: Vec<&Clip> = track
        .clips
        .iter()
        .filter(|c| c.audio.as_ref().is_some_and(|a| a.gain_env.is_some()))
        .collect();
    assert_eq!(halves.len(), 2);
    for h in &halves {
        assert_eq!(h.audio.as_ref().unwrap().gain_env.as_deref(), Some(&line));
    }
    let left = halves.iter().find(|c| c.id == b).unwrap();
    let right = halves.iter().find(|c| c.id != b).unwrap();
    assert_eq!(
        left.audio.as_ref().unwrap().cross,
        Some(XfadeCurve::EqualPower)
    );
    assert_eq!(right.audio.as_ref().unwrap().cross, None);
}

/// Review 1: a copy carries no crossfade of an overlap it is not in.
#[test]
fn a_copy_leaves_its_crossfade_behind() {
    let (mut bus, t, _, b) = two_clips();
    bus.set_auto_crossfades(Some(XfadeCurve::EqualPower));
    move_to(&mut bus, t, b, Ticks::beats(3));
    let c = clip(&bus, t, b);
    assert!(c.audio.as_ref().unwrap().cross.is_some());
    let copy = clipboard::copy_of(&c, Ticks::beats(20), &mut || 77_777);
    assert_eq!(copy.audio.unwrap().cross, None);
}

/// Review 1: a clip moved out of its overlap drops its curve even with automatic
/// crossfades off, so it never crosses what it is laid over next; undo brings it back.
#[test]
fn a_clip_moved_out_of_its_overlap_drops_its_curve_in_the_same_step() {
    let (mut bus, t, _, b) = two_clips();
    bus.set_auto_crossfades(Some(XfadeCurve::Linear));
    move_to(&mut bus, t, b, Ticks::beats(3));
    bus.set_auto_crossfades(None);
    move_to(&mut bus, t, b, Ticks::beats(10));
    assert_eq!(clip(&bus, t, b).audio.unwrap().cross, None);
    move_to(&mut bus, t, b, Ticks::beats(3));
    assert_eq!(
        clip(&bus, t, b).audio.unwrap().cross,
        None,
        "not crossed again"
    );
    bus.undo();
    bus.undo();
    assert_eq!(
        clip(&bus, t, b).audio.unwrap().cross,
        Some(XfadeCurve::Linear)
    );
}

/// Review 1: a snapshot of thousands of crossed clips costs what sorting them does.
#[test]
fn a_snapshot_of_thousands_of_crossed_clips_is_quick() {
    let mut p = Project::new("many");
    let src = source(&mut p);
    let mut t = p.make_track("Many");
    for k in 0..5_000i64 {
        let mut c = p.make_audio_clip("c", src, Ticks(k * 960), Ticks(1_200));
        c.offset = Ticks(k * 7);
        c.audio.as_mut().unwrap().cross = Some(XfadeCurve::EqualPower);
        t.clips.push(c);
    }
    p.tracks.push(t);
    let t0 = std::time::Instant::now();
    let s = Snapshot::compile(&p, 1);
    assert!(
        t0.elapsed() < std::time::Duration::from_secs(3),
        "{:?}",
        t0.elapsed()
    );
    let crossed = s.tracks[0].audio.iter().filter(|c| c.xin.is_some()).count();
    assert_eq!(crossed, 4_999);
}
