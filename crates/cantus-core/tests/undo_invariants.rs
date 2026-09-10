//! Invariant from idea.md §6: for any command sequence, undo-all must restore the exact
//! initial state, and redo-all must restore the final state. Commands are generated
//! randomly (deterministic seed) from every origin, including AI and MCP.

use cantus_core::*;

/// Tiny deterministic PRNG so the test needs no external crate and failures reproduce.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() % 10_000) as f32 / 10_000.0 * (hi - lo)
    }
}

fn random_command(rng: &mut Lcg, project: &mut Project) -> Command {
    let tracks = project.tracks.len();
    match rng.below(18) {
        0 => Command::SetTempo {
            bpm: rng.f32(20.0, 300.0) as f64,
        },
        1 => {
            let t = project.make_track(format!("T{}", project.next_id));
            Command::InsertTrack {
                index: rng.below(tracks as u64 + 1) as usize,
                track: Box::new(t),
            }
        }
        2 if tracks > 0 => Command::RemoveTrack {
            id: project.tracks[rng.below(tracks as u64) as usize].id,
        },
        3 if tracks > 0 => {
            let id = project.tracks[rng.below(tracks as u64) as usize].id;
            Command::SetTrackGain {
                id,
                gain_db: rng.f32(-96.0, 12.0),
            }
        }
        4 if tracks > 0 => {
            let id = project.tracks[rng.below(tracks as u64) as usize].id;
            Command::SetTrackSolo {
                id,
                soloed: rng.below(2) == 1,
            }
        }
        5 if tracks > 0 => {
            let id = project.tracks[rng.below(tracks as u64) as usize].id;
            let clip = project.make_clip(
                "c",
                Ticks::beats(rng.below(32) as i64),
                Ticks::beats(1 + rng.below(8) as i64),
            );
            Command::InsertClip {
                track: id,
                clip: Box::new(clip),
            }
        }
        6..=8 if tracks > 0 => {
            let t = &project.tracks[rng.below(tracks as u64) as usize];
            let tid = t.id;
            if t.clips.is_empty() {
                return Command::SetTrackPan {
                    id: tid,
                    pan: rng.f32(-1.0, 1.0),
                };
            }
            let c = &t.clips[rng.below(t.clips.len() as u64) as usize];
            let cid = c.id;
            match rng.below(3) {
                0 => {
                    let n = (0..1 + rng.below(4))
                        .map(|_| {
                            let start = Ticks(rng.below(4000) as i64);
                            let len = Ticks(1 + rng.below(2000) as i64);
                            let pitch = rng.below(128) as u8;
                            let vel = 1 + rng.below(127) as u8;
                            project.make_note(start, len, pitch, vel)
                        })
                        .collect();
                    Command::InsertNotes {
                        track: tid,
                        clip: cid,
                        notes: n,
                    }
                }
                1 if !c.notes.is_empty() => {
                    let ids = c.notes.iter().take(2).map(|n| n.id).collect();
                    Command::RemoveNotes {
                        track: tid,
                        clip: cid,
                        ids,
                    }
                }
                _ if !c.notes.is_empty() => {
                    let mut n = c.notes[0].clone();
                    n.start = Ticks(rng.below(4000) as i64);
                    n.pitch = rng.below(128) as u8;
                    Command::SetNotes {
                        track: tid,
                        clip: cid,
                        notes: vec![n],
                    }
                }
                _ => Command::SetClipName {
                    track: tid,
                    clip: cid,
                    name: "x".into(),
                },
            }
        }
        9 if tracks > 0 => {
            let id = project.tracks[rng.below(tracks as u64) as usize].id;
            let param = FirmusParam::ALL[rng.below(FirmusParam::ALL.len() as u64) as usize];
            Command::SetInstrumentParam {
                track: id,
                param,
                value: rng.f32(-100.0, 30_000.0),
            }
        }
        10 if tracks > 1 => {
            let id = project.tracks[rng.below(tracks as u64) as usize].id;
            Command::MoveTrack {
                id,
                index: rng.below(tracks as u64) as usize,
            }
        }
        11 => Command::SetLoopRegion {
            region: Some(LoopRegion {
                start: Ticks::beats(rng.below(8) as i64),
                end: Ticks::beats(8 + rng.below(8) as i64),
            }),
        },
        12 => Command::Batch {
            label_key: "batch".into(),
            commands: vec![
                Command::SetMetronome {
                    enabled: rng.below(2) == 1,
                },
                Command::SetMasterGain {
                    gain_db: rng.f32(-20.0, 6.0),
                },
            ],
        },
        13 if tracks > 0 => {
            let id = project.tracks[rng.below(tracks as u64) as usize].id;
            let kind = EffectKind::ALL[rng.below(2) as usize];
            let slot = project.make_effect(kind);
            let len = project.track(id).unwrap().effects.len();
            Command::InsertEffect {
                track: id,
                index: rng.below(len as u64 + 1) as usize,
                slot: Box::new(slot),
            }
        }
        14 if tracks > 0 => {
            let t = &project.tracks[rng.below(tracks as u64) as usize];
            match t
                .effects
                .get(rng.below(t.effects.len().max(1) as u64) as usize)
            {
                Some(e) => match rng.below(4) {
                    0 => Command::RemoveEffect {
                        track: t.id,
                        effect: e.id,
                    },
                    1 => Command::SetEffectBypass {
                        track: t.id,
                        effect: e.id,
                        bypass: rng.below(2) == 1,
                    },
                    2 => Command::MoveEffect {
                        track: t.id,
                        effect: e.id,
                        index: rng.below(t.effects.len() as u64) as usize,
                    },
                    _ => {
                        let infos = e.spec.kind.params();
                        let info = infos[rng.below(infos.len() as u64) as usize];
                        Command::SetEffectParam {
                            track: t.id,
                            effect: e.id,
                            key: info.key.to_string(),
                            value: rng.f32(info.min - 1.0, info.max + 1.0),
                        }
                    }
                },
                None => Command::SetTrackMute {
                    id: t.id,
                    muted: rng.below(2) == 1,
                },
            }
        }
        // Schema 4's clip fields: a clip is a window onto its content, and every one of
        // these has to be as reversible as the rest.
        15..=16 if tracks > 0 => {
            let ti = rng.below(tracks as u64) as usize;
            let t = &project.tracks[ti];
            let tid = t.id;
            let Some(c) = t.clips.get(rng.below(t.clips.len().max(1) as u64) as usize) else {
                return Command::SetTrackMute {
                    id: tid,
                    muted: rng.below(2) == 1,
                };
            };
            let cid = c.id;
            match rng.below(6) {
                0 => Command::SetClipOffset {
                    track: tid,
                    clip: cid,
                    offset: Ticks(rng.below(8000) as i64 - 4000),
                },
                1 => Command::SetClipLoop {
                    track: tid,
                    clip: cid,
                    content_length: (rng.below(2) == 1).then(|| Ticks(1 + rng.below(4000) as i64)),
                },
                2 => Command::SetClipMute {
                    track: tid,
                    clip: cid,
                    muted: rng.below(2) == 1,
                },
                3 => Command::SetClipColor {
                    track: tid,
                    clip: cid,
                    color: (rng.below(2) == 1).then(|| Color::PALETTE[rng.below(8) as usize]),
                },
                4 if tracks > 1 => Command::MoveClipToTrack {
                    from: tid,
                    to: project.tracks[rng.below(tracks as u64) as usize].id,
                    clip: cid,
                },
                _ => Command::SetClipBounds {
                    track: tid,
                    clip: cid,
                    start: Ticks(rng.below(20_000) as i64),
                    length: Ticks(1 + rng.below(8000) as i64),
                },
            }
        }
        _ => Command::SetProjectName {
            name: format!("P{}", rng.below(100)),
        },
    }
}

#[test]
fn undo_all_restores_initial_state_for_random_sequences() {
    let origins = [
        Origin::User,
        Origin::Script,
        Origin::Ai,
        Origin::Mcp,
        Origin::System,
    ];
    for seed in 1..=40u64 {
        let mut rng = Lcg(seed);
        let mut bus = CommandBus::default();
        let mut checkpoints: Vec<Project> = vec![bus.project().clone()];
        let mut applied = 0;
        for _ in 0..120 {
            // Id allocation mutates `next_id` outside the command system; keep the two in
            // sync by cloning the project only to *generate* the command, then re-applying
            // the allocation through a real project copy.
            let mut scratch = bus.project().clone();
            let cmd = random_command(&mut rng, &mut scratch);
            // Keep the bus id counter in sync with the ids the generator handed out.
            while bus.project().next_id < scratch.next_id {
                bus.new_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
            }
            let origin = origins[rng.below(5) as usize];
            let ok = bus.execute(cmd, origin).is_ok();
            if ok {
                applied += 1;
                // next_id bookkeeping is not undoable; normalise it out of the comparison.
                let mut snap = bus.project().clone();
                snap.next_id = 0;
                checkpoints.push(snap);
            }
        }
        assert!(
            applied > 90,
            "seed {seed}: too few commands applied ({applied})"
        );
        assert!(bus.project().tracks.len() + 1 >= 2 || applied > 0);

        // Walk back: each undo must yield the previous checkpoint.
        for i in (1..checkpoints.len()).rev() {
            assert!(bus.undo().is_some(), "seed {seed}: undo {i} failed");
            let mut now = bus.project().clone();
            now.next_id = 0;
            let mut expected = checkpoints[i - 1].clone();
            expected.next_id = 0;
            assert_eq!(
                now,
                expected,
                "seed {seed}: mismatch after undo to checkpoint {}",
                i - 1
            );
        }
        assert!(!bus.can_undo());

        // Walk forward again.
        #[allow(clippy::needless_range_loop)]
        for i in 1..checkpoints.len() {
            assert!(bus.redo().is_some(), "seed {seed}: redo {i} failed");
            let mut now = bus.project().clone();
            now.next_id = 0;
            assert_eq!(
                now, checkpoints[i],
                "seed {seed}: mismatch after redo to checkpoint {i}"
            );
        }
        assert!(!bus.can_redo());
    }
}

#[test]
fn commands_serialize_roundtrip() {
    let mut p = Project::default();
    let t = p.make_track("x");
    let cmd = Command::Batch {
        label_key: "k".into(),
        commands: vec![
            Command::InsertTrack {
                index: 0,
                track: Box::new(t),
            },
            Command::SetTempo { bpm: 133.3 },
            Command::SetInstrumentParam {
                track: TrackId(1),
                param: FirmusParam::Cutoff,
                value: 800.0,
            },
        ],
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let back: Command = serde_json::from_str(&json).unwrap();
    assert_eq!(back, cmd);
}
