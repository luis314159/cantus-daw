//! Freezing (round 29, ADR 0064): the content key, the freeze commands and their exact
//! inverses, and the edit lock — what a frozen sound refuses and what stays live.

use cantus_core::freeze::{self, CannotFreeze, FreezeTarget};
use cantus_core::*;

/// A song: a synth on its own strip with a delay; a drum track and a bass track into a bus
/// strip with a compressor keyed from a "kick" strip; a reverb return fed by a send from
/// the synth strip; a lonely track straight to the master.
struct Song {
    bus: CommandBus,
    synth: TrackId,
    synth_strip: StripId,
    delay: EffectId,
    drums: TrackId,
    bass: TrackId,
    group: StripId,
    comp: EffectId,
    kick_strip: StripId,
    kick: TrackId,
    reverb: StripId,
    lonely: TrackId,
}

fn song() -> Song {
    let mut p = Project::new("Freeze");
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
    let bass = add(&mut p, "Bass", Some(group));
    let kick = add(&mut p, "Kick", Some(kick_strip));
    let lonely = add(&mut p, "Lonely", None);
    let delay = p.make_effect(EffectKind::Delay);
    let delay_id = delay.id;
    p.strip_mut(synth_strip).unwrap().effects.push(delay);
    let mut comp = p.make_effect(EffectKind::Compressor);
    comp.sidechain = Some(kick_strip);
    let comp_id = comp.id;
    p.strip_mut(group).unwrap().effects.push(comp);
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
        delay: delay_id,
        drums,
        bass,
        group,
        comp: comp_id,
        kick_strip,
        kick,
        reverb,
        lonely,
    }
}

fn key(bus: &CommandBus, t: FreezeTarget) -> String {
    freeze::content_key(bus.project(), t).unwrap()
}

fn freeze(bus: &mut CommandBus, t: FreezeTarget) {
    let cmd = freeze::freeze_commands(bus.project(), t).unwrap();
    bus.execute(cmd, Origin::User).unwrap();
    assert!(bus.project().frozen(t).is_some());
}

fn refused_by(bus: &mut CommandBus, cmd: Command) -> Option<FreezeTarget> {
    let before = bus.project().clone();
    match bus.execute(cmd, Origin::User) {
        Err(CommandError::Frozen(t)) => {
            assert_eq!(bus.project(), &before, "a refusal changes nothing");
            Some(t)
        }
        Err(e) => panic!("refused for another reason: {e}"),
        Ok(_) => None,
    }
}

#[test]
fn the_content_key_is_sha256_hex_and_stable() {
    let s = song();
    let k = key(&s.bus, FreezeTarget::Strip(s.group));
    assert_eq!(k.len(), 64);
    assert!(k
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    assert_eq!(k, key(&s.bus, FreezeTarget::Strip(s.group)));
    // A copy of the project (through its file) keys the same.
    let json = serde_json::to_string(s.bus.project()).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(
        freeze::content_key(&back, FreezeTarget::Strip(s.group)).unwrap(),
        k
    );
    assert_ne!(k, key(&s.bus, FreezeTarget::Strip(s.synth_strip)));
    assert_ne!(
        key(&s.bus, FreezeTarget::Track(s.synth)),
        key(&s.bus, FreezeTarget::Strip(s.synth_strip))
    );
}

/// Apply `cmd` to a copy and say whether `target`'s key moved.
fn moves(s: &Song, target: FreezeTarget, cmd: Command) -> bool {
    let mut p = s.bus.project().clone();
    let before = freeze::content_key(&p, target).unwrap();
    cmd.apply(&mut p).expect("applies");
    freeze::content_key(&p, target).unwrap() != before
}

#[test]
fn the_key_changes_for_what_changes_the_sound_and_for_nothing_else() {
    let s = song();
    let group = FreezeTarget::Strip(s.group);
    // What the strip gathers and its chain: the key moves.
    assert!(moves(
        &s,
        group,
        Command::SetTrackGain {
            id: s.drums,
            gain_db: -3.0
        }
    ));
    assert!(moves(
        &s,
        group,
        Command::SetTrackMute {
            id: s.bass,
            muted: true
        }
    ));
    assert!(moves(
        &s,
        group,
        Command::SetInstrumentParam {
            track: s.bass,
            param: FirmusParam::Cutoff.into(),
            value: 900.0,
        }
    ));
    assert!(moves(
        &s,
        group,
        Command::SetEffectParam {
            strip: s.group,
            effect: s.comp,
            key: "ratio".into(),
            value: 8.0,
        }
    ));
    assert!(moves(
        &s,
        group,
        Command::SetStripPhaseInvert {
            id: s.group,
            invert: true
        }
    ));
    // The key's strip, through the compressor's sidechain.
    assert!(moves(
        &s,
        group,
        Command::SetStripGain {
            id: s.kick_strip,
            gain_db: -6.0
        }
    ));
    assert!(moves(
        &s,
        group,
        Command::SetTrackGain {
            id: s.kick,
            gain_db: -6.0
        }
    ));
    assert!(moves(&s, group, Command::SetTempo { bpm: 97.0 }));
    // The strip's own mix stays live: its fader, pan, width, mute, solo, where it goes.
    for cmd in [
        Command::SetStripGain {
            id: s.group,
            gain_db: -6.0,
        },
        Command::SetStripPan {
            id: s.group,
            pan: 0.4,
        },
        Command::SetStripWidth {
            id: s.group,
            width: 0.5,
        },
        Command::SetStripMute {
            id: s.group,
            muted: true,
        },
        Command::SetStripSolo {
            id: s.group,
            soloed: true,
        },
        Command::SetStripOutput {
            id: s.group,
            output: Some(s.reverb),
        },
        Command::SetStripName {
            id: s.group,
            name: "Rhythm".into(),
        },
        Command::SetStripColor {
            id: s.group,
            color: Some(Color::PALETTE[3]),
        },
        Command::SetTrackName {
            id: s.drums,
            name: "Kit".into(),
        },
        Command::SetTrackColor {
            id: s.bass,
            color: Color::PALETTE[5],
        },
        Command::SetTrackHeight {
            id: s.bass,
            height: 2.0,
        },
        Command::SetTrackSolo {
            id: s.bass,
            soloed: true,
        },
        // Elsewhere.
        Command::SetTrackGain {
            id: s.lonely,
            gain_db: -9.0,
        },
        Command::SetStripGain {
            id: s.synth_strip,
            gain_db: -9.0,
        },
        Command::SetEffectParam {
            strip: s.synth_strip,
            effect: s.delay,
            key: "feedback".into(),
            value: 0.7,
        },
        Command::SetMasterGain { gain_db: -2.0 },
        Command::SetLoopEnabled { enabled: true },
        Command::SetMetronome { enabled: true },
    ] {
        let label = format!("{cmd:?}");
        assert!(!moves(&s, group, cmd), "{label} moved the key");
    }
    // A frozen track's own level and balance are live, its notes are not.
    let synth = FreezeTarget::Track(s.synth);
    assert!(!moves(
        &s,
        synth,
        Command::SetTrackGain {
            id: s.synth,
            gain_db: -4.0
        }
    ));
    assert!(!moves(
        &s,
        synth,
        Command::SetTrackPan {
            id: s.synth,
            pan: -0.3
        }
    ));
    assert!(!moves(
        &s,
        synth,
        Command::SetTrackStrip {
            id: s.synth,
            strip: None
        }
    ));
    assert!(!moves(
        &s,
        synth,
        Command::SetEffectParam {
            strip: s.synth_strip,
            effect: s.delay,
            key: "feedback".into(),
            value: 0.7,
        }
    ));
    let clip = s.bus.project().track(s.synth).unwrap().clips[0].id;
    assert!(moves(
        &s,
        synth,
        Command::SetClipMute {
            track: s.synth,
            clip,
            muted: true
        }
    ));
}

#[test]
fn freezing_and_unfreezing_are_one_undo_step_each_and_exact() {
    let mut s = song();
    let before = s.bus.project().clone();
    let target = FreezeTarget::Strip(s.group);
    freeze(&mut s.bus, target);
    assert_eq!(
        s.bus.project().frozen(target).unwrap().content,
        freeze::content_key(&before, target).unwrap()
    );
    assert!(freeze::consistent(s.bus.project()).is_ok());
    s.bus
        .execute(freeze::unfreeze_command(target), Origin::User)
        .unwrap();
    assert!(s.bus.project().frozen(target).is_none());
    s.bus.undo().unwrap();
    assert!(
        s.bus.project().frozen(target).is_some(),
        "the undo of an unfreeze freezes"
    );
    s.bus.undo().unwrap();
    assert_eq!(s.bus.project(), &before);
    // A key that is not the target's is refused.
    let wrong = Command::SetStripFrozen {
        strip: s.group,
        frozen: Some(Frozen {
            content: "0".repeat(64),
        }),
    };
    assert!(matches!(
        s.bus.execute(wrong, Origin::User),
        Err(CommandError::InvalidValue(_))
    ));
    assert_eq!(
        s.bus.project().frozen(target),
        None,
        "nothing frozen by a wrong key"
    );
    // A frozen state goes to the file and comes back; a live one writes nothing.
    let live = serde_json::to_string(s.bus.project()).unwrap();
    assert!(
        !live.contains("frozen"),
        "a project without freezing writes no field"
    );
    freeze(&mut s.bus, target);
    let json = serde_json::to_string(s.bus.project()).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(&back, s.bus.project());
}

#[test]
fn a_frozen_strip_refuses_what_would_change_its_sound_and_names_itself() {
    let mut s = song();
    let target = FreezeTarget::Strip(s.group);
    freeze(&mut s.bus, target);
    let clip = s.bus.project().track(s.drums).unwrap().clips[0].id;
    let note = s.bus.project().track(s.drums).unwrap().clips[0].notes[0].clone();
    let refused = [
        Command::SetTrackGain {
            id: s.drums,
            gain_db: -3.0,
        },
        Command::SetTrackMute {
            id: s.bass,
            muted: true,
        },
        Command::SetTrackSolo {
            id: s.bass,
            soloed: true,
        },
        Command::SetInstrumentParam {
            track: s.bass,
            param: FirmusParam::Cutoff.into(),
            value: 900.0,
        },
        Command::SetEffectParam {
            strip: s.group,
            effect: s.comp,
            key: "ratio".into(),
            value: 8.0,
        },
        Command::SetEffectBypass {
            strip: s.group,
            effect: s.comp,
            bypass: true,
        },
        Command::SetStripEq {
            id: s.group,
            band: 0,
            field: EqField::Gain,
            value: 3.0,
        },
        Command::SetStripFxEnabled {
            id: s.group,
            enabled: false,
        },
        Command::SetNotes {
            track: s.drums,
            clip,
            notes: vec![Note {
                velocity: 30,
                ..note
            }],
        },
        Command::SetClipMute {
            track: s.drums,
            clip,
            muted: true,
        },
        Command::SetTrackStrip {
            id: s.lonely,
            strip: Some(s.group),
        },
        Command::SetTrackStrip {
            id: s.drums,
            strip: None,
        },
        Command::SetTrackArmed {
            id: s.bass,
            armed: true,
        },
        // The key's strip and its track.
        Command::SetStripGain {
            id: s.kick_strip,
            gain_db: -6.0,
        },
        Command::SetTrackGain {
            id: s.kick,
            gain_db: -6.0,
        },
        // A send into it.
        Command::InsertStripSend {
            id: s.synth_strip,
            index: 0,
            send: Send {
                to: s.group,
                level: 0.3,
                enabled: true,
                tap: SendTap::Post,
            },
        },
        // Project-wide: checked after, undone.
        Command::SetTempo { bpm: 97.0 },
    ];
    for cmd in refused {
        let label = format!("{cmd:?}");
        assert_eq!(
            refused_by(&mut s.bus, cmd),
            Some(target),
            "{label} was not refused"
        );
    }
    assert!(freeze::consistent(s.bus.project()).is_ok());
    // Live: the strip's own mix, and everything outside.
    let allowed = [
        Command::SetStripGain {
            id: s.group,
            gain_db: -6.0,
        },
        Command::SetStripPan {
            id: s.group,
            pan: 0.4,
        },
        Command::SetStripWidth {
            id: s.group,
            width: 0.5,
        },
        Command::SetStripMute {
            id: s.group,
            muted: true,
        },
        Command::SetStripSolo {
            id: s.group,
            soloed: true,
        },
        Command::InsertStripSend {
            id: s.group,
            index: 0,
            send: Send {
                to: s.reverb,
                level: 0.3,
                enabled: true,
                tap: SendTap::Post,
            },
        },
        Command::SetStripName {
            id: s.group,
            name: "Rhythm".into(),
        },
        Command::SetTrackName {
            id: s.drums,
            name: "Kit".into(),
        },
        Command::SetTrackSolo {
            id: s.lonely,
            soloed: true,
        },
        Command::SetTrackGain {
            id: s.synth,
            gain_db: -2.0,
        },
        Command::SetEffectParam {
            strip: s.synth_strip,
            effect: s.delay,
            key: "feedback".into(),
            value: 0.7,
        },
        Command::SetMasterGain { gain_db: -1.0 },
        Command::SetLoopEnabled { enabled: true },
    ];
    for cmd in allowed {
        let label = format!("{cmd:?}");
        assert_eq!(refused_by(&mut s.bus, cmd), None, "{label} was refused");
    }
    assert!(freeze::consistent(s.bus.project()).is_ok());
    // Unfrozen, the same edits go through.
    s.bus
        .execute(freeze::unfreeze_command(target), Origin::User)
        .unwrap();
    assert_eq!(
        refused_by(
            &mut s.bus,
            Command::SetTrackGain {
                id: s.drums,
                gain_db: -3.0
            }
        ),
        None
    );
    assert_eq!(
        refused_by(&mut s.bus, Command::SetTempo { bpm: 97.0 }),
        None
    );
}

#[test]
fn a_frozen_track_keeps_its_level_live_and_locks_its_source() {
    let mut s = song();
    let target = FreezeTarget::Track(s.synth);
    freeze(&mut s.bus, target);
    let clip = s.bus.project().track(s.synth).unwrap().clips[0].id;
    for cmd in [
        Command::SetClipBounds {
            track: s.synth,
            clip,
            start: Ticks::beats(1),
            length: Ticks::beats(4),
        },
        Command::SetInstrumentParam {
            track: s.synth,
            param: FirmusParam::Cutoff.into(),
            value: 900.0,
        },
        Command::SetTrackArmed {
            id: s.synth,
            armed: true,
        },
    ] {
        let label = format!("{cmd:?}");
        assert_eq!(refused_by(&mut s.bus, cmd), Some(target), "{label}");
    }
    for cmd in [
        Command::SetTrackGain {
            id: s.synth,
            gain_db: -4.0,
        },
        Command::SetTrackPan {
            id: s.synth,
            pan: 0.3,
        },
        Command::SetTrackMute {
            id: s.synth,
            muted: true,
        },
        Command::SetTrackSolo {
            id: s.synth,
            soloed: true,
        },
        Command::SetTrackStrip {
            id: s.synth,
            strip: Some(s.group),
        },
        // The strip's inserts are after the track's source.
        Command::SetEffectParam {
            strip: s.synth_strip,
            effect: s.delay,
            key: "feedback".into(),
            value: 0.7,
        },
    ] {
        let label = format!("{cmd:?}");
        assert_eq!(refused_by(&mut s.bus, cmd), None, "{label}");
    }
}

#[test]
fn freezing_disarms_the_region_in_the_same_step_and_refuses_a_live_input() {
    let mut s = song();
    s.bus
        .execute(
            Command::SetTrackArmed {
                id: s.bass,
                armed: true,
            },
            Origin::User,
        )
        .unwrap();
    let before = s.bus.project().clone();
    let target = FreezeTarget::Strip(s.group);
    freeze(&mut s.bus, target);
    assert!(!s.bus.project().track(s.bass).unwrap().armed);
    s.bus.undo().unwrap();
    assert_eq!(s.bus.project(), &before, "one undo step");
    let mut input = TrackInput::mono(0);
    input.monitor = MonitorMode::On;
    s.bus
        .execute(
            Command::SetTrackInput {
                id: s.drums,
                input: Some(input),
            },
            Origin::User,
        )
        .unwrap();
    assert_eq!(
        freeze::freeze_commands(s.bus.project(), target),
        Err(CannotFreeze::Monitoring(s.drums))
    );
}

#[test]
fn freeze_on_a_track_freezes_its_own_strip_or_its_source() {
    let s = song();
    let p = s.bus.project();
    assert_eq!(
        p.freeze_target_of_track(s.synth),
        Some(FreezeTarget::Strip(s.synth_strip))
    );
    assert_eq!(
        p.freeze_target_of_track(s.drums),
        Some(FreezeTarget::Track(s.drums))
    );
    assert_eq!(
        p.freeze_target_of_track(s.lonely),
        Some(FreezeTarget::Track(s.lonely))
    );
}

#[test]
fn what_rests_is_the_region_but_what_a_live_insert_listens_to() {
    let mut s = song();
    freeze(&mut s.bus, FreezeTarget::Strip(s.group));
    let p = s.bus.project();
    let d = freeze::dormant(p, &|_| true);
    assert!(d.roots.contains(&s.group));
    assert!(d.tracks.contains(&s.drums) && d.tracks.contains(&s.bass));
    // The kick strip keys the group but plays to the master too: it runs (review 1).
    assert!(!d.strips.contains(&s.kick_strip) && !d.tracks.contains(&s.kick));
    assert!(!d.tracks.contains(&s.synth) && !d.tracks.contains(&s.lonely));
    // Nothing rests while the render is not at hand.
    assert!(freeze::dormant(p, &|_| false).is_empty());
    // Played into the group only, the kick is heard nowhere but in the render: it rests.
    let mut p = p.clone();
    p.strip_mut(s.kick_strip).unwrap().output = Some(s.group);
    let d = freeze::dormant(&p, &|_| true);
    assert!(d.strips.contains(&s.kick_strip) && d.tracks.contains(&s.kick));
    // It also keys the synth strip's insert, which runs: the kick runs.
    let mut gate = p.make_effect(EffectKind::Gate);
    gate.sidechain = Some(s.kick_strip);
    p.strip_mut(s.synth_strip).unwrap().effects.push(gate);
    let d = freeze::dormant(&p, &|_| true);
    assert!(!d.strips.contains(&s.kick_strip) && !d.tracks.contains(&s.kick));
    assert!(d.tracks.contains(&s.drums));
    // Pattern mode plays every chain live.
    p.pattern_mode = true;
    assert!(freeze::dormant(&p, &|_| true).is_empty());
}

#[test]
fn a_playing_frozen_strip_loses_its_chain_in_the_snapshot_and_its_region_rests() {
    let mut s = song();
    freeze(&mut s.bus, FreezeTarget::Strip(s.group));
    s.bus
        .execute(
            Command::SetStripPhaseInvert {
                id: s.synth_strip,
                invert: true,
            },
            Origin::User,
        )
        .unwrap();
    let p = s.bus.project();
    let mut snap = Snapshot::compile(p, 1);
    let gi = p.strip_index(s.group).unwrap();
    assert!(snap.strips[gi].frozen.is_some());
    assert!(
        !snap.strips[gi].effects.is_empty(),
        "live until its render is attached"
    );
    // Not attached: settling changes nothing.
    let live = snap.clone();
    freeze::settle(p, &mut snap);
    assert_eq!(snap, live);
    // Attached.
    snap.strips[gi].frozen.as_mut().unwrap().audio =
        Some(std::sync::Arc::new(freeze::FrozenAudio {
            key: "k".into(),
            rate: 48_000,
            frames: 10,
            reader: StreamRef(std::sync::Arc::new(())),
        }));
    freeze::settle(p, &mut snap);
    assert!(snap.strips[gi].effects.is_empty() && snap.strips[gi].eq.is_flat());
    let si = p.strip_index(s.synth_strip).unwrap();
    assert!(
        snap.strips[si].phase_invert,
        "another strip keeps its chain"
    );
    let ti = |t: TrackId| p.track_index(t).unwrap();
    assert!(snap.tracks[ti(s.drums)].rest && snap.tracks[ti(s.bass)].rest);
    assert!(!snap.tracks[ti(s.synth)].rest);
    // The kick keys the group but plays to the master: it runs.
    let ki = p.strip_index(s.kick_strip).unwrap();
    assert!(!snap.strips[ki].rest && !snap.tracks[ti(s.kick)].rest);
    assert!(!snap.strips[gi].rest);
}

#[test]
fn a_solo_elsewhere_silences_a_playing_frozen_strip() {
    let mut s = song();
    freeze(&mut s.bus, FreezeTarget::Strip(s.group));
    s.bus
        .execute(
            Command::SetTrackSolo {
                id: s.lonely,
                soloed: true,
            },
            Origin::User,
        )
        .unwrap();
    let p = s.bus.project();
    let mut snap = Snapshot::compile(p, 1);
    let gi = p.strip_index(s.group).unwrap();
    assert!(
        !snap.strips[gi].muted,
        "live, the solo silences its tracks instead"
    );
    snap.strips[gi].frozen.as_mut().unwrap().audio =
        Some(std::sync::Arc::new(freeze::FrozenAudio {
            key: "k".into(),
            rate: 48_000,
            frames: 10,
            reader: StreamRef(std::sync::Arc::new(())),
        }));
    freeze::settle(p, &mut snap);
    assert!(snap.strips[gi].muted);
}

#[test]
fn pattern_mode_plays_frozen_chains_live() {
    let mut s = song();
    freeze(&mut s.bus, FreezeTarget::Strip(s.group));
    let p = cantus_core::pattern::audition(s.bus.project(), None);
    assert!(!p.any_frozen());
}

#[test]
fn a_modulator_driving_a_frozen_insert_is_locked_and_one_following_a_region_strip_wakes_it() {
    let mut s = song();
    let mut lfo = Modulator::new(ModulatorId(s.bus.alloc_id()), ModKind::Lfo);
    lfo.routes.push(ModRoute {
        target: AutoTarget::Effect {
            effect: s.comp,
            key: "ratio".into(),
        },
        depth: 0.2,
    });
    let lfo_id = lfo.id;
    s.bus
        .execute(
            Command::InsertModulator {
                host: ModHost::Strip(s.group),
                index: 0,
                modulator: Box::new(lfo.clone()),
            },
            Origin::User,
        )
        .unwrap();
    freeze(&mut s.bus, FreezeTarget::Strip(s.group));
    let mut faster = lfo.clone();
    faster.amount = 0.5;
    assert_eq!(
        refused_by(
            &mut s.bus,
            Command::SetModulator {
                modulator: Box::new(faster)
            }
        ),
        Some(FreezeTarget::Strip(s.group))
    );
    assert_eq!(
        refused_by(&mut s.bus, Command::RemoveModulator { id: lfo_id }),
        Some(FreezeTarget::Strip(s.group))
    );
    // A follower of the bass's strip driving the synth's delay (live): the bus runs.
    let mut f = Modulator::new(ModulatorId(s.bus.alloc_id()), ModKind::Follower);
    if let ModSource::Follower(src) = &mut f.source {
        src.listen = cantus_core::modulator::Listen::Strip(s.group);
    }
    f.routes.push(ModRoute {
        target: AutoTarget::Effect {
            effect: s.delay,
            key: "feedback".into(),
        },
        depth: 0.3,
    });
    let mut p = s.bus.project().clone();
    p.strip_mut(s.synth_strip).unwrap().modulators.push(f);
    let d = freeze::dormant(&p, &|_| true);
    assert!(
        d.roots.contains(&s.group),
        "the frozen strip itself plays its render"
    );
    assert!(d.tracks.contains(&s.drums), "its region still rests");
}

// ---- review 1 ---------------------------------------------------------------------------

/// A follower on `host` listening to `listen`, driving `target`.
fn follower(
    bus: &mut CommandBus,
    host: ModHost,
    listen: cantus_core::modulator::Listen,
    target: AutoTarget,
) {
    let mut f = Modulator::new(ModulatorId(bus.alloc_id()), ModKind::Follower);
    if let ModSource::Follower(src) = &mut f.source {
        src.listen = listen;
    }
    f.routes.push(ModRoute { target, depth: 0.3 });
    bus.execute(
        Command::InsertModulator {
            host,
            index: 0,
            modulator: Box::new(f),
        },
        Origin::User,
    )
    .unwrap();
}

#[test]
fn a_frozen_return_or_keyed_bus_never_silences_what_also_plays_live() {
    // A return fed by a send from a strip that plays to the master.
    let mut s = song();
    freeze(&mut s.bus, FreezeTarget::Strip(s.reverb));
    let d = freeze::dormant(s.bus.project(), &|_| true);
    assert!(d.roots.contains(&s.reverb));
    assert!(!d.strips.contains(&s.synth_strip) && !d.tracks.contains(&s.synth));
    // A vocoder's note source that plays to the master on its own.
    let mut s = song();
    let mut p = s.bus.project().clone();
    let mut voc = p.make_effect(EffectKind::Gate);
    voc.midi_from = Some(s.lonely);
    p.strip_mut(s.group).unwrap().effects.push(voc);
    s.bus = CommandBus::new(p);
    freeze(&mut s.bus, FreezeTarget::Strip(s.group));
    assert!(freeze::region(s.bus.project(), FreezeTarget::Strip(s.group))
        .tracks
        .contains(&s.lonely));
    let d = freeze::dormant(s.bus.project(), &|_| true);
    assert!(!d.tracks.contains(&s.lonely), "the note source plays on");
    assert!(d.tracks.contains(&s.drums), "what only the group hears rests");
}

#[test]
fn a_live_follower_of_a_region_strip_wakes_it_and_what_it_gathers() {
    let mut s = song();
    // A percussion strip into the group, with a track of its own.
    let mut p = s.bus.project().clone();
    let perc_strip = p.strips[4].id;
    p.strip_mut(perc_strip).unwrap().output = Some(s.group);
    let mut perc = p.make_track("Perc");
    perc.strip = Some(perc_strip);
    let perc_id = perc.id;
    p.tracks.push(perc);
    s.bus = CommandBus::new(p);
    freeze(&mut s.bus, FreezeTarget::Strip(s.group));
    let d = freeze::dormant(s.bus.project(), &|_| true);
    assert!(d.strips.contains(&perc_strip) && d.tracks.contains(&perc_id));
    // A follower on the synth strip (live) listens to the percussion strip.
    let delay = s.delay;
    follower(
        &mut s.bus,
        ModHost::Strip(s.synth_strip),
        cantus_core::modulator::Listen::Strip(perc_strip),
        AutoTarget::Effect {
            effect: delay,
            key: "feedback".into(),
        },
    );
    let d = freeze::dormant(s.bus.project(), &|_| true);
    assert!(!d.strips.contains(&perc_strip), "the strip it follows runs");
    assert!(!d.tracks.contains(&perc_id), "and what that strip gathers");
    assert!(d.tracks.contains(&s.drums), "the rest of the region still rests");
}

#[test]
fn a_frozen_tracks_region_takes_in_the_audio_its_modulators_follow() {
    let mut s = song();
    let drums = s.drums;
    follower(
        &mut s.bus,
        ModHost::Track(drums),
        cantus_core::modulator::Listen::Strip(s.kick_strip),
        AutoTarget::Instrument {
            track: drums,
            key: "cutoff".into(),
        },
    );
    let target = FreezeTarget::Track(drums);
    let r = freeze::region(s.bus.project(), target);
    assert!(r.strips.contains(&s.kick_strip) && r.tracks.contains(&s.kick));
    assert!(!r.hears_root);
    // The kick's notes are in its sound.
    let kick_clip = s.bus.project().track(s.kick).unwrap().clips[0].id;
    let note = s.bus.project().track(s.kick).unwrap().clips[0].notes[0].clone();
    let mut moved = note.clone();
    moved.pitch = 61;
    let edit = Command::SetNotes {
        track: s.kick,
        clip: kick_clip,
        notes: vec![moved],
    };
    assert!(moves(&s, target, edit.clone()));
    freeze(&mut s.bus, target);
    assert_eq!(refused_by(&mut s.bus, edit), Some(target));
    assert_eq!(
        refused_by(
            &mut s.bus,
            Command::SetTrackGain {
                id: s.kick,
                gain_db: -4.0
            }
        ),
        Some(target)
    );
    // Its own level stays live.
    assert_eq!(
        refused_by(
            &mut s.bus,
            Command::SetTrackGain {
                id: drums,
                gain_db: -4.0
            }
        ),
        None
    );
    assert!(freeze::consistent(s.bus.project()).is_ok());
}

#[test]
fn a_follower_of_the_frozen_strip_itself_locks_its_fader() {
    let mut s = song();
    let comp = s.comp;
    follower(
        &mut s.bus,
        ModHost::Strip(s.group),
        cantus_core::modulator::Listen::Strip(s.group),
        AutoTarget::Effect {
            effect: comp,
            key: "threshold".into(),
        },
    );
    let target = FreezeTarget::Strip(s.group);
    assert!(freeze::region(s.bus.project(), target).hears_root);
    let fader = Command::SetStripGain {
        id: s.group,
        gain_db: -6.0,
    };
    assert!(moves(&s, target, fader.clone()), "the fader is heard");
    freeze(&mut s.bus, target);
    assert_eq!(refused_by(&mut s.bus, fader), Some(target));
    assert!(freeze::consistent(s.bus.project()).is_ok());
}

#[test]
fn a_frozen_strips_only_track_keeps_its_mute_and_solo_live() {
    let mut s = song();
    let target = FreezeTarget::Strip(s.synth_strip);
    assert_eq!(freeze::region(s.bus.project(), target).own, Some(s.synth));
    assert_eq!(
        freeze::region(s.bus.project(), FreezeTarget::Strip(s.group)).own,
        None
    );
    freeze(&mut s.bus, target);
    let key_before = key(&s.bus, target);
    assert_eq!(
        refused_by(
            &mut s.bus,
            Command::SetTrackMute {
                id: s.synth,
                muted: true,
            }
        ),
        None
    );
    assert_eq!(key(&s.bus, target), key_before);
    assert!(freeze::consistent(s.bus.project()).is_ok());
    // Muted: the strip is silenced while it plays its render.
    assert!(freeze::silenced(s.bus.project(), s.synth_strip));
    s.bus.undo().unwrap();
    assert!(!freeze::silenced(s.bus.project(), s.synth_strip));
    // A solo elsewhere silences it; its own solo does not.
    s.bus
        .execute(
            Command::SetTrackSolo {
                id: s.lonely,
                soloed: true,
            },
            Origin::User,
        )
        .unwrap();
    assert!(freeze::silenced(s.bus.project(), s.synth_strip));
    assert_eq!(
        refused_by(
            &mut s.bus,
            Command::SetTrackSolo {
                id: s.synth,
                soloed: true,
            }
        ),
        None
    );
    assert!(!freeze::silenced(s.bus.project(), s.synth_strip));
    assert_eq!(key(&s.bus, target), key_before);
    // Its level is inside the render.
    assert_eq!(
        refused_by(
            &mut s.bus,
            Command::SetTrackGain {
                id: s.synth,
                gain_db: -3.0
            }
        ),
        Some(target)
    );
    // The render is made unmuted and unsoloed.
    let mut p = s.bus.project().clone();
    p.track_mut(s.synth).unwrap().muted = true;
    freeze::neutral(&mut p, target);
    assert!(!p.track(s.synth).unwrap().muted && !p.any_solo());
}

#[test]
fn freezing_a_region_with_a_solo_inside_is_refused_and_a_solo_elsewhere_is_not() {
    let mut s = song();
    let target = FreezeTarget::Strip(s.group);
    s.bus
        .execute(
            Command::SetTrackSolo {
                id: s.drums,
                soloed: true,
            },
            Origin::User,
        )
        .unwrap();
    assert_eq!(
        freeze::freeze_commands(s.bus.project(), target),
        Err(CannotFreeze::SoloedTrack(s.drums))
    );
    s.bus.undo().unwrap();
    s.bus
        .execute(
            Command::SetStripSolo {
                id: s.kick_strip,
                soloed: true,
            },
            Origin::User,
        )
        .unwrap();
    assert_eq!(
        freeze::freeze_commands(s.bus.project(), target),
        Err(CannotFreeze::SoloedStrip(s.kick_strip))
    );
    s.bus.undo().unwrap();
    // Elsewhere, or the frozen strip itself: allowed (the render is made unsoloed).
    for cmd in [
        Command::SetTrackSolo {
            id: s.lonely,
            soloed: true,
        },
        Command::SetStripSolo {
            id: s.group,
            soloed: true,
        },
    ] {
        s.bus.execute(cmd, Origin::User).unwrap();
    }
    assert!(freeze::freeze_commands(s.bus.project(), target).is_ok());
}

#[test]
fn moving_strips_and_tracks_keeps_every_key_or_is_refused() {
    let mut s = song();
    let target = FreezeTarget::Strip(s.group);
    freeze(&mut s.bus, target);
    // The frozen strip itself moved on the desk: nothing to hear.
    let before = key(&s.bus, target);
    assert_eq!(
        refused_by(
            &mut s.bus,
            Command::MoveStrip {
                id: s.group,
                index: 6
            }
        ),
        None
    );
    assert_eq!(key(&s.bus, target), before);
    // A track elsewhere moved.
    assert_eq!(
        refused_by(
            &mut s.bus,
            Command::MoveTrack {
                id: s.lonely,
                index: 0
            }
        ),
        None
    );
    assert!(freeze::consistent(s.bus.project()).is_ok());
    // A strip of the region moved (the order of the group's sums): refused.
    assert_eq!(
        refused_by(
            &mut s.bus,
            Command::MoveStrip {
                id: s.kick_strip,
                index: 8
            }
        ),
        Some(target)
    );
    // Two automation clips of one target on two tracks: their tracks' order is nothing
    // to hear unless they start together.
    let mut p = s.bus.project().clone();
    let comp = s.comp;
    let auto = |p: &mut Project, at: Ticks, v: f32| {
        let mut c = p.make_clip("ratio", at, Ticks::beats(4));
        c.automation = Some(Automation::new(
            AutoTarget::Effect {
                effect: comp,
                key: "ratio".into(),
            },
            vec![AutoPoint::new(Ticks::ZERO, v)],
        ));
        c
    };
    let a = auto(&mut p, Ticks::ZERO, 0.5);
    let b = auto(&mut p, Ticks::beats(8), 0.5);
    p.track_mut(s.lonely).unwrap().clips.push(a);
    p.track_mut(s.synth).unwrap().clips.push(b);
    let swap = |p: &mut Project| {
        let x = p.track_index(s.lonely).unwrap();
        let y = p.track_index(s.synth).unwrap();
        p.tracks.swap(x, y);
    };
    let k1 = freeze::content_key(&p, target).unwrap();
    swap(&mut p);
    assert_eq!(freeze::content_key(&p, target).unwrap(), k1);
    // Starting together, the later track wins: their order is in the key.
    let c = auto(&mut p, Ticks::ZERO, 0.9);
    p.track_mut(s.synth).unwrap().clips.push(c);
    let k2 = freeze::content_key(&p, target).unwrap();
    swap(&mut p);
    assert_ne!(freeze::content_key(&p, target).unwrap(), k2);
}
