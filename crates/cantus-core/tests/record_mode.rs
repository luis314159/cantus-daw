//! The recording mode's model (round 36, ADR 0092): what *Prepare to record* bakes, and
//! the edit lock — everything that would change the sound is refused (with the frozen
//! guard's help), the live parts stay live, and the lock holds for undo and redo too.

use cantus_core::freeze::{self, FreezeTarget};
use cantus_core::record_mode::{self, EditLock, Why};
use cantus_core::*;

struct Song {
    bus: CommandBus,
    synth: TrackId,
    synth_strip: StripId,
    drums: TrackId,
    group: StripId,
    reverb: StripId,
    lonely: TrackId,
}

fn song() -> Song {
    let mut p = Project::new("Record");
    let ids: Vec<StripId> = p.strips.iter().map(|s| s.id).collect();
    let (synth_strip, group, kick_strip, reverb) = (ids[0], ids[1], ids[2], ids[3]);
    let add = |p: &mut Project, name: &str, strip: Option<StripId>| {
        let mut t = p.make_track(name);
        t.strip = strip;
        let mut c = p.make_clip(name, Ticks::ZERO, Ticks::beats(8));
        let n = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
        c.notes.push(n);
        t.clips.push(c);
        let id = t.id;
        p.tracks.push(t);
        id
    };
    let synth = add(&mut p, "Synth", Some(synth_strip));
    let drums = add(&mut p, "Drums", Some(group));
    let _kick = add(&mut p, "Kick", Some(kick_strip));
    let lonely = add(&mut p, "Lonely", None);
    let delay = p.make_effect(EffectKind::Delay);
    p.strip_mut(synth_strip).unwrap().effects.push(delay);
    p.strip_mut(synth_strip).unwrap().sends.push(Send {
        to: reverb,
        level: 0.5,
        enabled: true,
        tap: SendTap::Post,
    });
    Song {
        bus: CommandBus::new(p),
        synth,
        synth_strip,
        drums,
        group,
        reverb,
        lonely,
    }
}

/// What the mode does once its renders are there: every planned target frozen (here
/// without files: the model's state is what the lock reads).
fn enter(s: &mut Song, mix_live: bool) -> record_mode::Plan {
    let plan = record_mode::plan(s.bus.project());
    for t in plan.bake.clone() {
        let cmd = freeze::freeze_commands(s.bus.project(), t).unwrap();
        s.bus.apply_session(cmd).unwrap();
    }
    let armed: Vec<FreezeTarget> = plan
        .live
        .iter()
        .filter(|(_, w)| matches!(w, Why::Armed(_)))
        .map(|(t, _)| *t)
        .collect();
    let lock = EditLock::new(s.bus.project(), mix_live, true, &armed);
    s.bus.set_record_lock(Some(lock));
    plan
}

fn refused(bus: &mut CommandBus, cmd: Command) -> bool {
    let before = bus.project().clone();
    match bus.execute(cmd, Origin::User) {
        Err(CommandError::RecordingMode) => {
            assert_eq!(bus.project(), &before, "a refusal changes nothing");
            true
        }
        Err(e) => panic!("refused for another reason: {e}"),
        Ok(_) => false,
    }
}

#[test]
fn every_strip_that_feeds_the_master_is_baked_and_an_armed_one_stays_live() {
    let s = song();
    let plan = record_mode::plan(s.bus.project());
    // The synth's strip, the drums' bus, the kick's strip and the reverb return, and the
    // track that goes straight to the master.
    assert!(plan.bake.contains(&FreezeTarget::Strip(s.synth_strip)));
    assert!(plan.bake.contains(&FreezeTarget::Strip(s.group)));
    assert!(plan.bake.contains(&FreezeTarget::Strip(s.reverb)));
    assert!(plan.bake.contains(&FreezeTarget::Track(s.lonely)));
    assert!(plan.live.is_empty());

    let mut s = song();
    s.bus
        .execute(
            Command::SetTrackArmed {
                id: s.synth,
                armed: true,
            },
            Origin::User,
        )
        .unwrap();
    let plan = record_mode::plan(s.bus.project());
    // The armed track's strip is live — and the return it sends into, which gathers it.
    assert!(!plan.bake.contains(&FreezeTarget::Strip(s.synth_strip)));
    assert!(!plan.bake.contains(&FreezeTarget::Strip(s.reverb)));
    assert!(plan
        .live
        .contains(&(FreezeTarget::Strip(s.synth_strip), Why::Armed(s.synth))));
    assert!(plan.bake.contains(&FreezeTarget::Strip(s.group)));
    // An armed track that goes straight to the master is live too.
    s.bus
        .execute(
            Command::SetTrackArmed {
                id: s.lonely,
                armed: true,
            },
            Origin::User,
        )
        .unwrap();
    let plan = record_mode::plan(s.bus.project());
    assert!(plan
        .live
        .contains(&(FreezeTarget::Track(s.lonely), Why::Armed(s.lonely))));
}

#[test]
fn what_the_user_froze_is_kept_and_not_planned_again() {
    let mut s = song();
    let t = FreezeTarget::Strip(s.group);
    let cmd = freeze::freeze_commands(s.bus.project(), t).unwrap();
    s.bus.execute(cmd, Origin::User).unwrap();
    let plan = record_mode::plan(s.bus.project());
    assert_eq!(plan.already, vec![t]);
    assert!(!plan.bake.contains(&t));
}

#[test]
fn a_soloed_bus_member_keeps_the_bus_live_and_says_why() {
    let mut s = song();
    // Two tracks on the bus: a solo inside it is lost in one render.
    s.bus
        .execute(
            Command::SetTrackStrip {
                id: s.lonely,
                strip: Some(s.group),
            },
            Origin::User,
        )
        .unwrap();
    s.bus
        .execute(
            Command::SetTrackSolo {
                id: s.drums,
                soloed: true,
            },
            Origin::User,
        )
        .unwrap();
    let plan = record_mode::plan(s.bus.project());
    assert!(plan
        .live
        .iter()
        .any(|(t, w)| *t == FreezeTarget::Strip(s.group) && *w == Why::Soloed));
}

#[test]
fn the_lock_refuses_every_edit_that_changes_the_sound_and_leaves_the_rest() {
    let mut s = song();
    s.bus
        .execute(
            Command::SetTrackArmed {
                id: s.lonely,
                armed: true,
            },
            Origin::User,
        )
        .unwrap();
    enter(&mut s, true);
    // Project-wide.
    assert!(refused(&mut s.bus, Command::SetTempo { bpm: 99.0 }));
    assert!(refused(&mut s.bus, Command::SetPatternMode { on: true }));
    assert!(refused(
        &mut s.bus,
        freeze::unfreeze_command(FreezeTarget::Strip(s.group))
    ));
    // Inside a baked region: the mode says it, not the freeze card.
    let clip = s.bus.project().track(s.drums).unwrap().clips[0].id;
    assert!(refused(
        &mut s.bus,
        Command::SetClipMute {
            track: s.drums,
            clip,
            muted: true
        }
    ));
    // A baked strip's own fader is a live mix control (the bus sends nowhere).
    assert!(!refused(
        &mut s.bus,
        Command::SetStripGain {
            id: s.group,
            gain_db: -3.0
        }
    ));
    // Structure.
    let t = s.bus.project().clone().make_track("New");
    assert!(refused(
        &mut s.bus,
        Command::InsertTrack {
            index: 0,
            track: Box::new(t)
        }
    ));
    // Not sound: a name, a colour, the loop, the metronome, a marker.
    assert!(!refused(
        &mut s.bus,
        Command::SetTrackName {
            id: s.drums,
            name: "Toms".into()
        }
    ));
    assert!(!refused(
        &mut s.bus,
        Command::SetMetronome { enabled: true }
    ));
    // The musician's own (armed, live) track edits as ever.
    let lone = s.bus.project().track(s.lonely).unwrap().clips[0].id;
    assert!(!refused(
        &mut s.bus,
        Command::SetClipMute {
            track: s.lonely,
            clip: lone,
            muted: true
        }
    ));
    // The master is live: its fader moves.
    assert!(!refused(
        &mut s.bus,
        Command::SetMasterGain { gain_db: -1.0 }
    ));
}

#[test]
fn the_mix_controls_are_live_or_locked_as_chosen() {
    let mut live = song();
    enter(&mut live, true);
    let strip = live.group;
    let group = live.synth_strip;
    assert!(!refused(
        &mut live.bus,
        Command::SetStripGain {
            id: strip,
            gain_db: -4.0
        }
    ));
    assert!(!refused(
        &mut live.bus,
        Command::SetStripPan {
            id: strip,
            pan: 0.3
        }
    ));
    assert!(!refused(
        &mut live.bus,
        Command::SetStripMute {
            id: strip,
            muted: true
        }
    ));
    // Where a baked strip goes is not a mix control.
    assert!(refused(
        &mut live.bus,
        Command::SetStripOutput {
            id: strip,
            output: Some(group)
        }
    ));

    let mut locked = song();
    enter(&mut locked, false);
    assert!(refused(
        &mut locked.bus,
        Command::SetStripGain {
            id: strip,
            gain_db: -4.0
        }
    ));
    let lonely = locked.lonely;
    assert!(refused(
        &mut locked.bus,
        Command::SetTrackGain {
            id: lonely,
            gain_db: -2.0
        }
    ));
    // The master stays live in both.
    assert!(!refused(
        &mut locked.bus,
        Command::SetMasterGain { gain_db: -1.0 }
    ));
}

#[test]
fn the_mode_is_never_in_the_history_and_undo_cannot_change_the_sound() {
    let mut s = song();
    // A sound edit made before the mode: undoing it in the mode would change a render.
    s.bus
        .execute(Command::SetTempo { bpm: 101.0 }, Origin::User)
        .unwrap();
    let steps = s.bus.history().len();
    assert!(!s.bus.is_dirty() || steps > 0);
    s.bus.mark_saved();
    let plan = enter(&mut s, true);
    assert_eq!(s.bus.history().len(), steps, "entering is no undo step");
    assert!(
        !s.bus.is_dirty(),
        "the mode is session state: the project stays clean"
    );
    let before = s.bus.project().clone();
    assert!(
        s.bus.undo().is_none(),
        "undoing the tempo in the mode is refused"
    );
    assert_eq!(s.bus.project(), &before);
    // A live edit made in the mode undoes and redoes as ever.
    s.bus
        .execute(Command::SetMasterGain { gain_db: -2.0 }, Origin::User)
        .unwrap();
    assert!(s.bus.undo().is_some());
    assert!(s.bus.redo().is_some());
    assert!(s.bus.undo().is_some());
    // Leaving: no step either, and the undo after it never freezes anything again.
    s.bus.set_record_lock(None);
    for t in &plan.bake {
        s.bus.apply_session(freeze::unfreeze_command(*t)).unwrap();
    }
    assert_eq!(s.bus.history().len(), steps);
    assert!(!s.bus.project().any_frozen());
    assert!(s.bus.undo().is_some(), "the tempo edit undoes");
    assert!(!s.bus.project().any_frozen(), "and nothing is frozen by it");
}

#[test]
fn a_batch_is_refused_whole_when_one_of_its_commands_is() {
    let mut s = song();
    enter(&mut s, true);
    let before = s.bus.project().clone();
    let batch = Command::Batch {
        label_key: "cmd.test".into(),
        commands: vec![
            Command::SetMasterGain { gain_db: -1.0 },
            Command::SetTempo { bpm: 80.0 },
        ],
    };
    assert!(refused(&mut s.bus, batch));
    assert_eq!(s.bus.project(), &before);
}

#[test]
fn the_lock_off_changes_nothing_of_the_buses_behaviour() {
    let mut s = song();
    assert!(s.bus.record_lock().is_none());
    s.bus
        .execute(Command::SetTempo { bpm: 99.0 }, Origin::User)
        .unwrap();
    assert!(s.bus.undo().is_some());
}

#[test]
fn what_stays_live_without_being_armed_is_locked_too() {
    let mut s = song();
    // A track straight to the master that monitors its input always: it stays live, and
    // locked; and a track with no clip (planned nowhere).
    let mut input = TrackInput::mono(0);
    input.monitor = MonitorMode::On;
    let lonely = s.lonely;
    s.bus
        .execute(
            Command::SetTrackInput {
                id: lonely,
                input: Some(input),
            },
            Origin::User,
        )
        .unwrap();
    let mut p = s.bus.project().clone();
    let empty = p.make_track("Empty");
    let empty_id = empty.id;
    let n = s.bus.project().tracks.len();
    s.bus
        .execute(
            Command::InsertTrack {
                index: n,
                track: Box::new(empty),
            },
            Origin::User,
        )
        .unwrap();
    let plan = enter(&mut s, true);
    assert!(plan
        .live
        .iter()
        .any(|(t, w)| *t == FreezeTarget::Track(lonely) && matches!(w, Why::Monitoring(_))));
    let clip = s.bus.project().track(lonely).unwrap().clips[0].id;
    assert!(refused(
        &mut s.bus,
        Command::SetClipMute {
            track: lonely,
            clip,
            muted: true
        }
    ));
    let c = p.make_clip("new", Ticks::ZERO, Ticks::beats(4));
    assert!(refused(
        &mut s.bus,
        Command::InsertClip {
            track: empty_id,
            clip: Box::new(c)
        }
    ));
}

#[test]
fn a_modulators_amount_is_judged_by_every_route() {
    let mut s = song();
    // A modulator on the master driving the drums bus fader.
    let group = s.group;
    let mid = ModulatorId(s.bus.alloc_id());
    let mut m = cantus_core::modulator::Modulator::new(mid, cantus_core::modulator::ModKind::Lfo);
    m.routes.push(cantus_core::modulator::ModRoute {
        target: cantus_core::automation::AutoTarget::StripGain(group),
        depth: 0.5,
    });
    let id = m.id;
    s.bus
        .execute(
            Command::InsertModulator {
                host: cantus_core::modulator::ModHost::Master,
                index: 0,
                modulator: Box::new(m),
            },
            Origin::User,
        )
        .unwrap();
    enter(&mut s, false);
    let amount = Command::SetModulatorAmount { id, amount: 0.2 };
    assert!(refused(&mut s.bus, amount));
}

/// A VCA strip following nothing yet (ADR 0093).
fn vca(s: &mut Song) -> StripId {
    let mut p = s.bus.project().clone();
    let mut v = p.make_strip("VCA");
    v.kind = cantus_core::vca::StripKind::Vca;
    let id = v.id;
    let n = s.bus.project().strips.len();
    s.bus
        .execute(
            Command::InsertStrip {
                index: n,
                strip: Box::new(v),
            },
            Origin::User,
        )
        .unwrap();
    id
}

fn follow(s: &mut Song, strip: StripId, vcas: Vec<StripId>) {
    s.bus
        .execute(
            Command::Mix(cantus_core::mix_cmd::MixCommand::SetStripVcas { id: strip, vcas }),
            Origin::User,
        )
        .unwrap();
}

#[test]
fn a_vca_fader_is_a_mix_control_judged_by_every_member() {
    // The drums' bus follows a VCA: its fader moves the bus, a baked root with a live mix.
    let mut s = song();
    let v = vca(&mut s);
    let group = s.group;
    follow(&mut s, group, vec![v]);
    enter(&mut s, true);
    assert!(!refused(
        &mut s.bus,
        Command::SetStripGain {
            id: v,
            gain_db: -4.0
        }
    ));
    assert!(!refused(
        &mut s.bus,
        Command::SetStripMute { id: v, muted: true }
    ));
    // The same with the mix locked: refused.
    let mut s = song();
    let v = vca(&mut s);
    follow(&mut s, group, vec![v]);
    enter(&mut s, false);
    assert!(refused(
        &mut s.bus,
        Command::SetStripGain {
            id: v,
            gain_db: -4.0
        }
    ));
    // A member whose mix another baked strip holds (the synth sends into the reverb): the
    // VCA's move is refused whole.
    let mut s = song();
    let v = vca(&mut s);
    let synth = s.synth_strip;
    follow(&mut s, group, vec![v]);
    follow(&mut s, synth, vec![v]);
    enter(&mut s, true);
    assert!(refused(
        &mut s.bus,
        Command::SetStripGain {
            id: v,
            gain_db: -4.0
        }
    ));
}

#[test]
fn a_snapshot_recall_into_a_baked_region_is_refused_and_one_of_live_strips_is_not() {
    use cantus_core::mix_cmd::MixCommand;
    let mut s = song();
    let lonely = s.lonely;
    s.bus
        .execute(
            Command::SetTrackArmed {
                id: lonely,
                armed: true,
            },
            Origin::User,
        )
        .unwrap();
    // The lonely track gets a strip of its own, armed: a live strip.
    let mut p = s.bus.project().clone();
    let own = p.make_strip("Lonely strip");
    let own_id = own.id;
    let n = s.bus.project().strips.len();
    s.bus
        .execute(
            Command::InsertStrip {
                index: n,
                strip: Box::new(own),
            },
            Origin::User,
        )
        .unwrap();
    s.bus
        .execute(
            Command::SetTrackStrip {
                id: lonely,
                strip: Some(own_id),
            },
            Origin::User,
        )
        .unwrap();
    enter(&mut s, true);
    // A desk state that changes a baked strip: refused whole.
    let mut all = cantus_core::mix_snap::capture(s.bus.project(), &[], false);
    let synth = s.synth_strip;
    if let Some(st) = all.strips.iter_mut().find(|x| x.id == synth) {
        st.phase_invert = true;
    }
    assert!(refused(
        &mut s.bus,
        Command::Mix(MixCommand::Apply {
            state: Box::new(all)
        })
    ));
    // Only the live strip's state: it goes through.
    let mut mine = cantus_core::mix_snap::capture(s.bus.project(), &[own_id], false);
    if let Some(st) = mine.strips.iter_mut().find(|x| x.id == own_id) {
        st.gain_db = -2.0;
    }
    assert!(!refused(
        &mut s.bus,
        Command::Mix(MixCommand::Apply {
            state: Box::new(mine)
        })
    ));
}

#[test]
fn a_pre_fader_sender_keeps_its_fader_and_pan_live_in_the_mode() {
    let mut s = song();
    let synth = s.synth_strip;
    let mut send = s.bus.project().strip(synth).unwrap().sends[0];
    send.tap = SendTap::Pre;
    s.bus
        .execute(Command::SetStripSend { id: synth, send }, Origin::User)
        .unwrap();
    let reverb = s.reverb;
    enter(&mut s, true);
    let key = freeze::content_key(s.bus.project(), FreezeTarget::Strip(reverb)).unwrap();
    assert!(!refused(
        &mut s.bus,
        Command::SetStripGain {
            id: synth,
            gain_db: -6.0
        }
    ));
    assert!(!refused(
        &mut s.bus,
        Command::SetStripPan {
            id: synth,
            pan: 0.4
        }
    ));
    assert_eq!(
        freeze::content_key(s.bus.project(), FreezeTarget::Strip(reverb)).unwrap(),
        key
    );
    assert!(freeze::consistent(s.bus.project()).is_ok());
    // Its width and mute shape or silence the send: locked.
    assert!(refused(
        &mut s.bus,
        Command::SetStripWidth {
            id: synth,
            width: 0.5
        }
    ));
    assert!(refused(
        &mut s.bus,
        Command::SetStripMute {
            id: synth,
            muted: true
        }
    ));
}

#[test]
fn a_strip_that_plays_outboard_gear_stays_live_and_says_why() {
    use cantus_core::external_unit::{ExternalUnit, ExternalUnitId, Pair};
    let s = song();
    let mut p = s.bus.project().clone();
    let drums = s.drums;
    let lonely = s.lonely;
    // An outboard effect on the drums (inside the bus) and one on the lonely track.
    for (i, t) in [drums, lonely].into_iter().enumerate() {
        p.external_units.push(ExternalUnit::effect(
            ExternalUnitId(900 + i as u64),
            t,
            Pair::stereo(2),
            Pair::stereo(2),
        ));
    }
    let plan = record_mode::plan(&p);
    assert!(plan
        .live
        .contains(&(FreezeTarget::Strip(s.group), Why::External(drums))));
    assert!(plan
        .live
        .contains(&(FreezeTarget::Track(lonely), Why::External(lonely))));
    assert!(!plan.bake.contains(&FreezeTarget::Strip(s.group)));
    assert!(!plan.bake.contains(&FreezeTarget::Track(lonely)));
    // Everything planned can be frozen: nothing fails for want of the gear.
    for t in &plan.bake {
        assert!(freeze::freeze_commands(&p, *t).is_ok(), "{t:?}");
    }
    // A unit switched off is no gear: the strip bakes.
    for u in &mut p.external_units {
        u.enabled = false;
    }
    let plan = record_mode::plan(&p);
    assert!(plan.bake.contains(&FreezeTarget::Strip(s.group)));
}

fn cue_of(
    p: &mut Project,
    beat: i64,
    edit: impl Fn(&mut cantus_core::mix_snap::MixState),
) -> cantus_core::mix_snap::MixSnapId {
    use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
    let mut state = capture(p, &[], false);
    edit(&mut state);
    let sid = MixSnapId(p.alloc_id());
    p.mix.snapshots.push(MixSnapshot {
        id: sid,
        name: "s".into(),
        state,
    });
    let cid = MixCueId(p.alloc_id());
    p.mix.cues.push(MixCue {
        id: cid,
        at: Ticks::beats(beat),
        snapshot: sid,
        scope: Vec::new(),
    });
    sid
}

#[test]
fn a_cue_that_reaches_a_frozen_region_is_in_its_key_and_refused_while_frozen() {
    use cantus_core::mix_cmd::MixCommand;
    let s = song();
    let group = s.group;
    let mut p = s.bus.project().clone();
    let before = freeze::content_key(&p, FreezeTarget::Strip(group)).unwrap();
    // A cue that flips the bus: in the bus key.
    let sid = cue_of(&mut p, 4, move |st| {
        if let Some(x) = st.strips.iter_mut().find(|x| x.id == group) {
            x.phase_invert = true;
        }
    });
    assert_ne!(
        before,
        freeze::content_key(&p, FreezeTarget::Strip(group)).unwrap(),
        "a cue that flips the bus is in its key"
    );
    // Frozen, the bus refuses a retake of the snapshot the cue plays (its key would move).
    let mut bus = CommandBus::new(p);
    let cmd = freeze::freeze_commands(bus.project(), FreezeTarget::Strip(group)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let retake = |bus: &CommandBus| {
        let mut state = bus.project().mix.snapshot(sid).unwrap().state.clone();
        if let Some(x) = state.strips.iter_mut().find(|x| x.id == group) {
            x.swap_channels = !x.swap_channels;
        }
        Command::Mix(MixCommand::UpdateSnapshot {
            id: sid,
            state: Box::new(state),
        })
    };
    let r = bus.execute(retake(&bus), Origin::User);
    assert!(matches!(r, Err(CommandError::Frozen(_))), "{r:?}");
    assert!(freeze::consistent(bus.project()).is_ok());
    // In the mode: refused as the mode does.
    bus.set_record_lock(Some(EditLock::new(bus.project(), true, true, &[])));
    let r = bus.execute(retake(&bus), Origin::User);
    assert!(matches!(r, Err(CommandError::RecordingMode)), "{r:?}");
    // Unfrozen, it goes through and the key moves: entering again renders the bus again.
    bus.set_record_lock(None);
    let k1 = freeze::content_key(bus.project(), FreezeTarget::Strip(group)).unwrap();
    bus.execute(
        freeze::unfreeze_command(FreezeTarget::Strip(group)),
        Origin::User,
    )
    .unwrap();
    bus.execute(retake(&bus), Origin::User).unwrap();
    assert_ne!(
        freeze::content_key(bus.project(), FreezeTarget::Strip(group)).unwrap(),
        k1
    );
}

#[test]
fn a_recall_of_baked_faders_is_a_mix_control() {
    use cantus_core::mix_cmd::MixCommand;
    let fader = |s: &Song, db: f32| {
        let mut st = cantus_core::mix_snap::capture(s.bus.project(), &[s.group], false);
        st.strips[0].gain_db = db;
        Command::Mix(MixCommand::Apply {
            state: Box::new(st),
        })
    };
    let mut live = song();
    enter(&mut live, true);
    let cmd = fader(&live, -5.0);
    assert!(!refused(&mut live.bus, cmd), "the bus fader is live");
    let mut locked = song();
    enter(&mut locked, false);
    let cmd = fader(&locked, -5.0);
    assert!(refused(&mut locked.bus, cmd), "the mix is locked");
    // A recall that flips a baked strip polarity is no mix control.
    let mut s = song();
    enter(&mut s, true);
    let mut st = cantus_core::mix_snap::capture(s.bus.project(), &[s.group], false);
    st.strips[0].phase_invert = true;
    assert!(refused(
        &mut s.bus,
        Command::Mix(MixCommand::Apply {
            state: Box::new(st)
        })
    ));
}

#[test]
fn a_cue_that_solos_a_strip_inside_a_region_keeps_it_from_being_frozen() {
    use cantus_core::mix_cmd::MixCommand;
    let s = song();
    let synth = s.synth_strip;
    let reverb = s.reverb;
    let mut p = s.bus.project().clone();
    // The synth sends into the reverb: a cue that solos the synth solos inside the
    // reverb's region.
    cue_of(&mut p, 4, move |st| {
        if let Some(x) = st.strips.iter_mut().find(|x| x.id == synth) {
            x.soloed = true;
        }
    });
    assert_eq!(
        freeze::freeze_commands(&p, FreezeTarget::Strip(reverb)).err(),
        Some(freeze::CannotFreeze::CueSolo(synth))
    );
    let plan = record_mode::plan(&p);
    assert!(plan
        .live
        .contains(&(FreezeTarget::Strip(reverb), Why::CueSolo(synth))));
    assert!(!plan.bake.contains(&FreezeTarget::Strip(reverb)));
    // The synth itself (its own solo is live, after its render) bakes.
    assert!(freeze::freeze_commands(&p, FreezeTarget::Strip(synth)).is_ok());
    // A cue that would solo inside a frozen region: refused (the region's key moves).
    let mut q = s.bus.project().clone();
    let sid = cue_of(&mut q, 4, |_| {});
    let mut bus = CommandBus::new(q);
    let cmd = freeze::freeze_commands(bus.project(), FreezeTarget::Strip(reverb)).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    let mut state = bus.project().mix.snapshot(sid).unwrap().state.clone();
    if let Some(x) = state.strips.iter_mut().find(|x| x.id == synth) {
        x.soloed = true;
    }
    let r = bus.execute(
        Command::Mix(MixCommand::UpdateSnapshot {
            id: sid,
            state: Box::new(state),
        }),
        Origin::User,
    );
    assert!(matches!(r, Err(CommandError::Frozen(_))), "{r:?}");
    assert!(freeze::consistent(bus.project()).is_ok());
}
