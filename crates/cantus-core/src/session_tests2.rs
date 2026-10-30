//! The session's clips seen by every clip command, the launch grid, what a clip plays and
//! the builders (ADR 0076).

use super::*;

#[test]
fn a_session_clip_is_edited_by_the_clip_commands() {
    let (mut bus, tracks, scenes) = fixture();
    let t = tracks[0];
    let id = bus
        .project()
        .slot_clip(SlotRef::new(t, scenes[0]))
        .unwrap()
        .id();
    assert!(bus.project().track(t).unwrap().clip(id).is_some());
    let n = bus.new_note(Ticks::beats(3), Ticks::QUARTER, 72, 90);
    bus.execute(
        Command::InsertNotes {
            track: t,
            clip: id,
            notes: vec![n],
        },
        Origin::User,
    )
    .unwrap();
    bus.execute(
        Command::SetClipName {
            track: t,
            clip: id,
            name: "Lead".into(),
        },
        Origin::User,
    )
    .unwrap();
    let c = bus.project().slot_clip(SlotRef::new(t, scenes[0])).unwrap();
    assert_eq!(c.clip.name, "Lead");
    assert_eq!(c.clip.notes.len(), 9);
    // What a slot cannot hold is refused, the project unchanged.
    let before = bus.project().clone();
    for cmd in [
        Command::SetClipLink {
            track: t,
            clip: id,
            link: Some(crate::model::LinkId(77)),
        },
        Command::SetClipLane {
            track: t,
            clip: id,
            lane: 2,
        },
        Command::SetClipBounds {
            track: t,
            clip: id,
            start: BAR,
            length: BAR,
        },
        Command::MoveClipToTrack {
            from: t,
            to: tracks[1],
            clip: id,
        },
        Command::RemoveClip { track: t, clip: id },
    ] {
        assert!(bus.execute(cmd.clone(), Origin::User).is_err(), "{cmd:?}");
        assert_eq!(bus.project(), &before, "{cmd:?}");
    }
    // Its window is the loop panel's: a shorter loop is taken.
    bus.execute(
        Command::SetClipLoop {
            track: t,
            clip: id,
            content_length: Some(BAR),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(
        bus.project()
            .slot_clip(SlotRef::new(t, scenes[0]))
            .unwrap()
            .end(),
        BAR
    );
}

#[test]
fn removing_a_track_or_a_scene_takes_its_slots_and_undo_brings_them_back() {
    let (mut bus, tracks, scenes) = fixture();
    let before = bus.project().clone();
    bus.execute(Command::RemoveTrack { id: tracks[0] }, Origin::User)
        .unwrap();
    bus.undo().unwrap();
    assert_eq!(bus.project(), &before);
    bus.execute(
        Command::Session(SessionCommand::RemoveScene { id: scenes[0] }),
        Origin::User,
    )
    .unwrap();
    assert!(bus.project().track(tracks[0]).unwrap().slots.is_empty());
    bus.undo().unwrap();
    assert_eq!(bus.project(), &before);
}

#[test]
fn a_source_or_groove_a_session_clip_plays_is_in_use() {
    let (mut bus, tracks, scenes) = fixture();
    let sid = crate::model::AudioSourceId(bus.alloc_id());
    bus.execute(
        Command::InsertAudioSource {
            source: crate::audio::AudioSource {
                id: sid,
                name: "loop".into(),
                path: "C:/loop.wav".into(),
                sample_rate: 48_000,
                channels: 2,
                frames: 96_000,
                sha256: None,
                recorded: false,
            },
        },
        Origin::User,
    )
    .unwrap();
    let mut a = note_clip(&mut bus, 1);
    a.clip.notes.clear();
    a.clip.audio = Some(crate::audio::AudioRegion::of(sid));
    bus.execute(
        Command::Session(SessionCommand::InsertSlotClip {
            slot: SlotRef::new(tracks[1], scenes[1]),
            clip: Box::new(a),
        }),
        Origin::User,
    )
    .unwrap();
    assert!(bus
        .execute(Command::RemoveAudioSource { id: sid }, Origin::User)
        .is_err());
    // A pooled groove a session clip plays with (review of delivery 1, #2).
    let slot = SlotRef::new(tracks[0], scenes[0]);
    let gid = crate::groove::GrooveId(bus.alloc_id());
    let g = crate::groove::Groove::from_clip(
        gid,
        &bus.project().slot_clip(slot).unwrap().clip,
        Ticks::beats(4),
        Ticks::SIXTEENTH,
    )
    .expect("a groove of its notes");
    bus.execute(
        Command::InsertGroove {
            index: 0,
            groove: Box::new(g),
        },
        Origin::User,
    )
    .unwrap();
    let id = bus.project().slot_clip(slot).unwrap().id();
    bus.execute(
        Command::SetClipGroove {
            track: tracks[0],
            clip: id,
            groove: Some(crate::groove::ClipGroove::new(
                crate::groove::GrooveSource::Pool { id: gid },
            )),
        },
        Origin::User,
    )
    .unwrap();
    assert!(bus
        .execute(Command::RemoveGroove { id: gid }, Origin::User)
        .is_err());
}

/// Review of delivery 1, #1: a track comes in with its column only when the column is one
/// a command could have made — scenes there, in order, clips valid, ids its own.
#[test]
fn a_track_comes_in_only_with_a_column_the_project_can_hold() {
    let (mut bus, tracks, scenes) = fixture();
    let mut copy = bus.project().track(tracks[0]).unwrap().clone();
    copy.id = TrackId(bus.alloc_id());
    let index = bus.project().tracks.len();
    let insert = |t: &crate::model::Track| Command::InsertTrack {
        index,
        track: Box::new(t.clone()),
    };
    // Its slot's clip is the original track's (same id): refused.
    assert!(bus.execute(insert(&copy), Origin::User).is_err());
    // A slot of a scene that is not there: refused.
    let mut ghost = copy.clone();
    ghost.slots[0].clip.as_mut().unwrap().clip.id = crate::model::ClipId(bus.alloc_id());
    ghost.slots[0].scene = SceneId(999_999);
    assert!(bus.execute(insert(&ghost), Origin::User).is_err());
    // With a clip of its own in a real scene: taken.
    let mut own = copy.clone();
    own.slots[0].clip.as_mut().unwrap().clip.id = crate::model::ClipId(bus.alloc_id());
    own.slots[0].scene = scenes[1];
    bus.execute(insert(&own), Origin::User).unwrap();
}

#[test]
fn a_frozen_tracks_column_is_locked() {
    let (mut bus, tracks, scenes) = fixture();
    let t = tracks[0];
    let content =
        crate::freeze::content_key(bus.project(), crate::freeze::FreezeTarget::Track(t)).unwrap();
    bus.execute(
        Command::SetTrackFrozen {
            track: t,
            frozen: Some(crate::freeze::Frozen { content }),
        },
        Origin::User,
    )
    .unwrap();
    let r = bus.execute(
        Command::Session(SessionCommand::SetSlotLaunch {
            slot: SlotRef::new(t, scenes[0]),
            launch: LaunchSettings {
                legato: true,
                ..Default::default()
            },
        }),
        Origin::User,
    );
    assert!(
        matches!(r, Err(crate::command::CommandError::Frozen(_))),
        "{r:?}"
    );
    // A scene's name is nothing a frozen sound hears.
    bus.execute(
        Command::Session(SessionCommand::SetSceneName {
            id: scenes[0],
            name: "Verse".into(),
        }),
        Origin::User,
    )
    .unwrap();
}

#[test]
fn launch_boundaries_follow_the_bar_lines() {
    let map = crate::tempo::TempoMap::constant(120.0, TimeSignature::default());
    let b = |t: Ticks, g| boundary_at_or_after(&map, t, g);
    assert_eq!(b(Ticks::ZERO, QuantGrid::Bars(1)), Ticks::ZERO);
    assert_eq!(b(Ticks(1), QuantGrid::Bars(1)), BAR);
    assert_eq!(b(BAR, QuantGrid::Bars(2)), BAR * 2);
    assert_eq!(b(BAR * 2, QuantGrid::Bars(2)), BAR * 2);
    assert_eq!(b(BAR * 3 + Ticks(5), QuantGrid::Bars(8)), BAR * 8);
    assert_eq!(b(Ticks(1), QuantGrid::Ticks(PPQ)), Ticks::QUARTER);
    assert_eq!(b(Ticks(17), QuantGrid::None), Ticks(17));
    // A triplet grid restarts at each bar line.
    assert_eq!(
        b(Ticks(1), QuantGrid::Ticks(PPQ * 4 / 3)),
        Ticks(PPQ * 4 / 3)
    );
    // Through a metre change: 4/4 for two bars, then 3/4.
    let map = crate::tempo::TempoMap::new(
        120.0,
        TimeSignature::default(),
        &[],
        &[MetreChange {
            tick: BAR * 2,
            signature: TimeSignature::new(3, 4),
        }],
    );
    let three = Ticks::beats(3);
    assert_eq!(
        boundary_at_or_after(&map, BAR * 2 + Ticks(1), QuantGrid::Bars(1)),
        BAR * 2 + three
    );
    assert_eq!(
        boundary_at_or_after(&map, BAR * 2 + Ticks(1), QuantGrid::Bars(2)),
        BAR * 2 + three * 2
    );
    // Global resolves to the session's own, and a session saying Global reads as a bar.
    assert_eq!(
        Quantize::Global.grid(Quantize::Quarter),
        QuantGrid::Ticks(PPQ)
    );
    assert_eq!(Quantize::Global.grid(Quantize::Global), QuantGrid::Bars(1));
}

#[test]
fn what_a_clip_plays_from_its_start_marker() {
    let (bus, tracks, scenes) = fixture();
    let mut c = bus
        .project()
        .slot_clip(SlotRef::new(tracks[0], scenes[0]))
        .unwrap()
        .clone();
    // Loop [1 bar, 2 bars), start marker at beat 2: 2..8 then the loop.
    c.clip.offset = BAR;
    c.clip.content_length = Some(BAR);
    c.start = Some(Ticks::beats(2));
    assert_eq!(c.content_at(Ticks::ZERO), Some(Ticks::beats(2)));
    assert_eq!(c.content_at(Ticks::beats(6)), Some(BAR));
    assert_eq!(c.content_at(Ticks::beats(11)), Some(BAR + Ticks::beats(1)));
    assert_eq!(c.first_pass(), Ticks::beats(6));
    // Without the loop it plays to its end marker and ends.
    c.clip.content_length = None;
    c.clip.length = BAR;
    assert_eq!(c.content_at(Ticks::beats(5)), Some(Ticks::beats(7)));
    assert_eq!(c.content_at(Ticks::beats(6)), None);
    // The pieces it becomes in the arrangement play the same content at every tick.
    for looped in [true, false] {
        c.clip.content_length = looped.then_some(BAR);
        let ps = edit::pieces(&c, c.play_from(), BAR * 10, BAR * 5);
        for d in 0..(BAR * 5).0 {
            let t = BAR * 10 + Ticks(d);
            let want = c.content_at(Ticks(d));
            let got = ps.iter().find_map(|p| {
                Clip {
                    start: p.start,
                    length: p.length,
                    offset: p.offset,
                    content_length: p.period,
                    ..c.clip.clone()
                }
                .content_shown_at(t)
            });
            assert_eq!(got, want, "looped {looped}, tick {d}");
        }
    }
}

#[test]
fn record_quantisation_and_follow_chances() {
    assert_eq!(RecordQuantize::Sixteenth.snap(Ticks(130)), Ticks(PPQ / 4));
    assert_eq!(RecordQuantize::None.snap(Ticks(130)), Ticks(130));
    assert_eq!(
        RecordQuantize::EighthAndTriplet.snap(Ticks(330)),
        Ticks(PPQ / 3)
    );
    let f = FollowActions {
        on: true,
        a: FollowAction::Next,
        b: FollowAction::Stop,
        chance_a: 30,
        ..FollowActions::off()
    };
    assert_eq!(f.pick(29), FollowAction::Next);
    assert_eq!(f.pick(30), FollowAction::Stop);
    assert!(f.is_active() && !FollowActions::off().is_active());
    assert_eq!(LaunchSettings::default().gain(0.2), 1.0);
    let half = LaunchSettings {
        velocity: 0.5,
        ..Default::default()
    };
    assert_eq!(half.gain(0.0), 0.5);
}

#[test]
fn duplicate_and_capture_copy_with_fresh_ids_in_one_step() {
    let (mut bus, tracks, scenes) = fixture();
    let before = bus.project().clone();
    let p = bus.project().clone();
    let cmd = edit::duplicate_scene(&p, scenes[0], &mut ids_of(&mut bus)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let s = &bus.project().session.scenes;
    assert_eq!(s.len(), 4);
    let dup = s[1].id;
    let t = bus.project().track(tracks[0]).unwrap();
    assert_ne!(
        t.slot_clip(dup).unwrap().id(),
        t.slot_clip(scenes[0]).unwrap().id()
    );
    assert_eq!(t.slot_clip(dup).unwrap().clip.notes.len(), 8);
    bus.undo().unwrap();
    assert_eq!(bus.project().session, before.session);
    let p = bus.project().clone();
    let playing = [SlotRef::new(tracks[0], scenes[0])];
    let cmd = edit::capture_scene(&p, &playing, 3, "Captured", &mut ids_of(&mut bus)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let last = bus.project().session.scenes[3].id;
    assert!(bus
        .project()
        .track(tracks[0])
        .unwrap()
        .slot_clip(last)
        .is_some());
    assert!(bus.project().track(tracks[1]).unwrap().slot(last).is_none());
    assert_eq!(bus.undo_label_key(), Some("cmd.session.capture_scene"));
}

#[test]
fn arrangement_clips_go_into_slots_and_back() {
    let (mut bus, tracks, scenes) = fixture();
    // Three arrangement clips on the bass track: they fill its column from scene 0 down,
    // and a fourth row is made for the third.
    let mut cs = Vec::new();
    for i in 0..4 {
        let c = bus.new_clip("Arr", BAR * (i * 2), BAR);
        cs.push((tracks[1], c.id));
        bus.execute(
            Command::InsertClip {
                track: tracks[1],
                clip: Box::new(c),
            },
            Origin::User,
        )
        .unwrap();
    }
    let p = bus.project().clone();
    let cmd = edit::slots_from_arrangement(&p, &cs, 1, &mut ids_of(&mut bus)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let pr = bus.project();
    assert_eq!(pr.session.scenes.len(), 5);
    let col = &pr.track(tracks[1]).unwrap().slots;
    assert_eq!(col.len(), 4);
    assert!(pr.track(tracks[1]).unwrap().slot(scenes[0]).is_none());
    // A slot's clip into the arrangement: what was there is cut where it is laid.
    let p = bus.project().clone();
    let slot = SlotRef::new(tracks[0], scenes[0]);
    let cmd = edit::slot_to_arrangement(&p, slot, BAR * 20, Some(BAR * 3), &mut ids_of(&mut bus))
        .unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let keys = bus.project().track(tracks[0]).unwrap();
    let total: i64 = keys.clips.iter().map(|c| c.length.0).sum();
    assert_eq!(Ticks(total), BAR * 3);
}

#[test]
fn a_performance_commits_as_one_step_that_plays_what_was_heard() {
    let (mut bus, tracks, scenes) = fixture();
    // An arrangement clip under where the session will play, longer on both sides.
    let mut arr = bus.new_clip("Under", Ticks::ZERO, BAR * 12);
    arr.content_length = Some(BAR * 3);
    let under = arr.clone();
    bus.execute(
        Command::InsertClip {
            track: tracks[0],
            clip: Box::new(arr),
        },
        Origin::User,
    )
    .unwrap();
    let before = bus.project().clone();
    let slot = SlotRef::new(tracks[0], scenes[0]);
    let perf = Performance {
        held: vec![(tracks[0], BAR * 4, BAR * 7)],
        played: vec![Played {
            slot,
            at: BAR * 4,
            from: Ticks::beats(3),
            until: BAR * 7,
            gain: 1.0,
            velocity: 1.0,
            origin: BAR * 4 - Ticks::beats(3),
        }],
        tempos: vec![(BAR * 4, 140.0)],
        metres: vec![],
    };
    let cmd = edit::commit_performance(&before, &perf).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let sc = before.slot_clip(slot).unwrap().clone();
    let t = bus.project().track(tracks[0]).unwrap();
    // Every tick plays what was heard: the arrangement outside, the clip inside.
    for d in (0..(BAR * 12).0).step_by(37) {
        let tick = Ticks(d);
        let shown: Vec<Ticks> = t
            .clips
            .iter()
            .filter_map(|c| c.content_shown_at(tick))
            .collect();
        let want = if tick >= BAR * 4 && tick < BAR * 7 {
            sc.content_at(tick - BAR * 4 + Ticks::beats(3) - sc.play_from())
        } else {
            under.content_shown_at(tick)
        };
        assert_eq!(shown, want.into_iter().collect::<Vec<_>>(), "tick {d}");
    }
    assert_eq!(bus.project().tempo_points.len(), 2);
    assert_eq!(bus.undo_label_key(), Some("cmd.session.commit_performance"));
    bus.undo().unwrap();
    let (mut now, mut want) = (bus.project().clone(), before);
    now.next_id = 0;
    want.next_id = 0;
    assert_eq!(now, want);
}

#[test]
fn a_column_and_a_stretch_of_time_consolidate() {
    let (mut bus, tracks, scenes) = fixture();
    let second = note_clip(&mut bus, 1);
    bus.execute(
        Command::Session(SessionCommand::InsertSlotClip {
            slot: SlotRef::new(tracks[0], scenes[2]),
            clip: Box::new(second),
        }),
        Origin::User,
    )
    .unwrap();
    let p = bus.project().clone();
    let cmd = edit::consolidate_column(
        &p,
        tracks[0],
        &[scenes[0], scenes[2]],
        &mut ids_of(&mut bus),
    )
    .unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let t = bus.project().track(tracks[0]).unwrap();
    let c = t.slot_clip(scenes[0]).unwrap();
    assert_eq!(c.clip.notes.len(), 12);
    assert_eq!(c.end(), BAR * 3);
    assert!(t.slot_clip(scenes[2]).is_none());
    // An arrangement stretch to a new scene.
    let mut arr = bus.new_clip("Arr", BAR, BAR * 2);
    let n = bus.new_note(Ticks::beats(1), Ticks::QUARTER, 50, 90);
    arr.notes.push(n);
    bus.execute(
        Command::InsertClip {
            track: tracks[1],
            clip: Box::new(arr),
        },
        Origin::User,
    )
    .unwrap();
    let p = bus.project().clone();
    let cmd = edit::consolidate_time(&p, BAR, BAR * 2, 0, "Taken", &mut ids_of(&mut bus)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let first = bus.project().session.scenes[0].id;
    let c = bus
        .project()
        .track(tracks[1])
        .unwrap()
        .slot_clip(first)
        .unwrap();
    assert_eq!(c.clip.notes.len(), 1);
    assert_eq!(c.clip.notes[0].start, Ticks::beats(1));
}

/// Review of delivery 1, #3–#4: the timeline's edits read the arrangement only — a cut of a
/// selection holding a session clip cuts the arrangement clips and applies; pattern mode
/// has no pattern around a session clip; a session clip is no link target.
#[test]
fn timeline_edits_read_the_arrangement_only() {
    let (mut bus, tracks, scenes) = fixture();
    let arr = bus.new_clip("Arr", Ticks::ZERO, BAR);
    let arr_id = arr.id;
    bus.execute(
        Command::InsertClip {
            track: tracks[0],
            clip: Box::new(arr),
        },
        Origin::User,
    )
    .unwrap();
    let slot_id = bus
        .project()
        .slot_clip(SlotRef::new(tracks[0], scenes[0]))
        .unwrap()
        .id();
    let cut =
        crate::clipboard::cut_clips(bus.project(), &[(tracks[0], arr_id), (tracks[0], slot_id)]);
    assert_eq!(cut.len(), 1);
    bus.execute(
        Command::Batch {
            label_key: "cmd.cut".into(),
            commands: cut,
        },
        Origin::User,
    )
    .unwrap();
    assert!(bus
        .project()
        .slot_clip(SlotRef::new(tracks[0], scenes[0]))
        .is_some());
    assert!(crate::pattern::PatternRegion::of(bus.project(), tracks[0], slot_id).is_none());
    assert!(
        crate::links::would_link(bus.project(), &[(tracks[1], slot_id)], (tracks[0], slot_id))
            .is_err()
    );
}

/// Review of delivery 1, #6 and #10: a clip moved (or copied) to another track takes its
/// envelopes of its track's level along; what its envelopes drive is touched by every
/// command that carries them (a frozen strip's level is locked).
#[test]
fn envelopes_follow_their_clip_and_are_touched_with_it() {
    let (mut bus, tracks, scenes) = fixture();
    let a = SlotRef::new(tracks[0], scenes[0]);
    let gain = Automation::new(
        AutoTarget::TrackGain(tracks[0]),
        vec![AutoPoint::new(Ticks::ZERO, 0.3)],
    );
    bus.execute(
        Command::Session(SessionCommand::SetSlotEnvelopes {
            slot: a,
            envelopes: vec![gain],
        }),
        Origin::User,
    )
    .unwrap();
    let b = SlotRef::new(tracks[1], scenes[1]);
    bus.execute(
        Command::Session(SessionCommand::MoveSlotClip { from: a, to: b }),
        Origin::User,
    )
    .unwrap();
    let moved = bus.project().slot_clip(b).unwrap();
    assert_eq!(moved.envelopes[0].target, AutoTarget::TrackGain(tracks[1]));
    bus.undo().unwrap();
    let back = bus.project().slot_clip(a).unwrap();
    assert_eq!(back.envelopes[0].target, AutoTarget::TrackGain(tracks[0]));
    let p = bus.project().clone();
    let dup = edit::duplicate_slot(&p, a, b, &mut ids_of(&mut bus)).unwrap();
    bus.execute(dup, Origin::User).unwrap();
    let copy = bus.project().slot_clip(b).unwrap();
    assert_eq!(copy.envelopes[0].target, AutoTarget::TrackGain(tracks[1]));
    // An envelope of the bass track's level, which the frozen bass strip gathers: taking the
    // clip out touches it, and is refused.
    let strip = bus.project().track(tracks[1]).unwrap().strip.unwrap();
    bus.execute(
        Command::Session(SessionCommand::SetSlotEnvelopes {
            slot: a,
            envelopes: vec![Automation::new(
                AutoTarget::TrackGain(tracks[1]),
                vec![AutoPoint::new(Ticks::ZERO, 0.3)],
            )],
        }),
        Origin::User,
    )
    .unwrap();
    let content =
        crate::freeze::content_key(bus.project(), crate::freeze::FreezeTarget::Strip(strip))
            .unwrap();
    bus.execute(
        Command::SetStripFrozen {
            strip,
            frozen: Some(crate::freeze::Frozen { content }),
        },
        Origin::User,
    )
    .unwrap();
    let r = bus.execute(
        Command::Session(SessionCommand::RemoveSlotClip { slot: a }),
        Origin::User,
    );
    assert!(
        matches!(r, Err(crate::command::CommandError::Frozen(_))),
        "{r:?}"
    );
}

/// Review of delivery 1, #8: clips that would not play as they did joined (one with
/// envelopes of its own) are not consolidated.
#[test]
fn a_column_with_envelopes_is_not_consolidated() {
    let (mut bus, tracks, scenes) = fixture();
    let mut second = note_clip(&mut bus, 1);
    second.envelopes.push(Automation::new(
        AutoTarget::TrackGain(tracks[0]),
        vec![AutoPoint::new(Ticks::ZERO, 0.3)],
    ));
    bus.execute(
        Command::Session(SessionCommand::InsertSlotClip {
            slot: SlotRef::new(tracks[0], scenes[1]),
            clip: Box::new(second),
        }),
        Origin::User,
    )
    .unwrap();
    let p = bus.project().clone();
    assert!(edit::consolidate_column(
        &p,
        tracks[0],
        &[scenes[0], scenes[1]],
        &mut ids_of(&mut bus)
    )
    .is_none());
}
