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
    match rng.below(25) {
        0 => Command::SetTempo {
            bpm: rng.f32(20.0, 300.0) as f64,
        },
        1 => {
            let mut t = project.make_track(format!("T{}", project.next_id));
            // Every third new track is a kit, so the pad commands below have something to
            // act on and the two instrument variants are both exercised by the fuzz.
            if rng.below(3) == 0 {
                t.instrument = InstrumentSpec::Pulsus(PulsusParams::default());
            }
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
                param: param.into(),
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
        // Inserts live on mixer strips. The strip is picked at random from all of them,
        // fed or not: an insert on an empty strip has to be as reversible as any other.
        13 => {
            let strips = project.strips.len();
            let id = project.strips[rng.below(strips as u64) as usize].id;
            // A quarter of them on the master's chain (ADR 0026).
            let id = if rng.below(4) == 0 {
                StripId::MASTER
            } else {
                id
            };
            let kind = EffectKind::ALL[rng.below(EffectKind::ALL.len() as u64) as usize];
            let mut slot = project.make_effect(kind);
            if rng.below(4) == 0 {
                // A hosted plug-in (ADR 0019): a record with a state and a value.
                let mut plugin = ExternalPlugin::new(PluginFormat::Clap, "org.fuzz.plugin", "Fuzz");
                plugin.sidechain = rng.below(2) == 1;
                plugin.state = PluginState::new(vec![7; rng.below(5) as usize]);
                plugin.params.insert(0, 0.5);
                slot.spec = EffectSpec::external(plugin);
            }
            let len = project.effects_of(id).unwrap().len();
            Command::InsertEffect {
                strip: id,
                index: rng.below(len as u64 + 1) as usize,
                slot: Box::new(slot),
            }
        }
        14 => {
            let strips = project.strips.len();
            let st = &project.strips[rng.below(strips as u64) as usize];
            // The master's chain as well as the strips' (ADR 0026): every effect command
            // addresses it as `StripId::MASTER`.
            let on_master = rng.below(4) == 0;
            let (host, chain) = if on_master {
                (StripId::MASTER, &project.master.effects)
            } else {
                (st.id, &st.effects)
            };
            match chain.get(rng.below(chain.len().max(1) as u64) as usize) {
                Some(e) => match rng.below(4) {
                    0 => Command::RemoveEffect {
                        strip: host,
                        effect: e.id,
                    },
                    1 => Command::SetEffectBypass {
                        strip: host,
                        effect: e.id,
                        bypass: rng.below(2) == 1,
                    },
                    2 => Command::MoveEffect {
                        strip: host,
                        effect: e.id,
                        index: rng.below(chain.len() as u64) as usize,
                    },
                    _ => {
                        if on_master && rng.below(8) == 0 {
                            return Command::SetMasterFxEnabled {
                                enabled: rng.below(2) == 1,
                            };
                        }
                        if rng.below(6) == 0 {
                            // Keys: from any strip, itself and loops included, onto any
                            // effect, kinds that take none included.
                            return Command::SetEffectSidechain {
                                strip: host,
                                effect: e.id,
                                source: (rng.below(3) != 0).then(|| {
                                    // The master as a key is always refused (a loop).
                                    if rng.below(8) == 0 {
                                        StripId::MASTER
                                    } else {
                                        project.strips[rng.below(strips as u64) as usize].id
                                    }
                                }),
                            };
                        }
                        if rng.below(8) == 0 {
                            // Notes into effects: from any track, a missing one included,
                            // onto any effect, kinds that take none included (ADR 0022).
                            let source = match rng.below(4) {
                                0 => None,
                                1 => Some(TrackId(9_999)),
                                _ if !project.tracks.is_empty() => Some(
                                    project.tracks[rng.below(project.tracks.len() as u64) as usize]
                                        .id,
                                ),
                                _ => None,
                            };
                            return Command::SetEffectMidiSource {
                                strip: host,
                                effect: e.id,
                                source,
                            };
                        }
                        if rng.below(3) == 0 {
                            return Command::SetEffectMix {
                                strip: host,
                                effect: e.id,
                                mix: rng.f32(-0.5, 1.5),
                            };
                        }
                        if e.spec.kind == EffectKind::External || rng.below(20) == 0 {
                            // A hosted plug-in's values by id — new, changed, forgotten —
                            // and whole states; on a native slot both are refused.
                            let id = rng.below(4) as u32;
                            if rng.below(4) == 0 {
                                let mut params = std::collections::BTreeMap::new();
                                params.insert(id, f64::from(rng.f32(-2.0, 2.0)));
                                return Command::SetPluginState {
                                    strip: host,
                                    effect: e.id,
                                    state: PluginState::new(vec![
                                        rng.below(256) as u8;
                                        rng.below(8) as usize
                                    ]),
                                    params,
                                };
                            }
                            return Command::SetPluginParam {
                                strip: host,
                                effect: e.id,
                                id,
                                value: (rng.below(5) != 0).then(|| f64::from(rng.f32(-2.0, 2.0))),
                            };
                        }
                        let infos = e.spec.kind.params();
                        let info = infos[rng.below(infos.len() as u64) as usize];
                        if rng.below(5) == 0 {
                            return Command::ClearEffectParam {
                                strip: host,
                                effect: e.id,
                                key: info.key.to_string(),
                            };
                        }
                        Command::SetEffectParam {
                            strip: host,
                            effect: e.id,
                            key: info.key.to_string(),
                            value: rng.f32(info.min - 1.0, info.max + 1.0),
                        }
                    }
                },
                None if on_master => Command::SetMasterFxEnabled {
                    enabled: rng.below(2) == 1,
                },
                // Strip controls and routing, including routes the command must refuse
                // (cycles, a strip feeding itself) and removals it must refuse (a strip
                // still fed): a refused command leaves nothing behind.
                None => match rng.below(20) {
                    // Docks and groups: a view of the desk, stored in the project, so as
                    // reversible as anything that renders — refused removals included.
                    15 => Command::SetStripDock {
                        id: st.id,
                        dock: Dock::ALL[rng.below(3) as usize],
                    },
                    16 => {
                        let color = Color::PALETTE[rng.below(8) as usize];
                        let g = project.make_strip_group("grp", color);
                        Command::InsertStripGroup {
                            index: rng.below(project.strip_groups.len() as u64 + 1) as usize,
                            group: Box::new(g),
                        }
                    }
                    17 => Command::SetStripGroup {
                        id: st.id,
                        group: project
                            .strip_groups
                            .get(rng.below(project.strip_groups.len() as u64 + 1) as usize)
                            .map(|g| g.id),
                    },
                    18 => match project
                        .strip_groups
                        .get(rng.below(project.strip_groups.len().max(1) as u64) as usize)
                    {
                        Some(g) => match rng.below(4) {
                            0 => Command::RemoveStripGroup { id: g.id },
                            1 => Command::SetStripGroupName {
                                id: g.id,
                                name: format!("G{}", rng.below(100)),
                            },
                            2 => Command::SetStripGroupColor {
                                id: g.id,
                                color: Color::PALETTE[rng.below(8) as usize],
                            },
                            _ => Command::SetStripGroupCollapsed {
                                id: g.id,
                                collapsed: rng.below(2) == 1,
                            },
                        },
                        None => Command::RemoveStripGroup { id: GroupId(9_999) },
                    },
                    19 => Command::SetStripDock {
                        id: st.id,
                        dock: Dock::Right,
                    },
                    8 => Command::SetStripSolo {
                        id: st.id,
                        soloed: rng.below(2) == 1,
                    },
                    9 => Command::SetStripSwapChannels {
                        id: st.id,
                        swap: rng.below(2) == 1,
                    },
                    10 => Command::SetStripFxEnabled {
                        id: st.id,
                        enabled: rng.below(2) == 1,
                    },
                    11 => Command::SetStripEq {
                        id: st.id,
                        band: rng.below(4) as u8,
                        field: [EqField::Freq, EqField::Gain, EqField::Q][rng.below(3) as usize],
                        value: rng.f32(-30.0, 30_000.0),
                    },
                    // Sends: new ones, changes to existing ones, ones that would loop, and
                    // ones to the strip itself — every refusal path as well as the work.
                    12 => Command::SetStripSend {
                        id: st.id,
                        send: Send {
                            to: project.strips[rng.below(strips as u64) as usize].id,
                            level: rng.f32(-0.2, 1.2),
                            enabled: rng.below(4) != 0,
                        },
                    },
                    13 => Command::RemoveStripSend {
                        id: st.id,
                        to: st
                            .sends
                            .get(rng.below(st.sends.len().max(1) as u64) as usize)
                            .map(|s| s.to)
                            .unwrap_or(project.strips[rng.below(strips as u64) as usize].id),
                    },
                    14 => Command::MoveStrip {
                        id: st.id,
                        index: rng.below(strips as u64 + 1) as usize,
                    },
                    0 => Command::SetStripGain {
                        id: st.id,
                        gain_db: rng.f32(-96.0, 12.0),
                    },
                    1 => Command::SetStripPan {
                        id: st.id,
                        pan: rng.f32(-1.0, 1.0),
                    },
                    2 => Command::SetStripWidth {
                        id: st.id,
                        width: rng.f32(0.0, 2.0),
                    },
                    3 => Command::SetStripMute {
                        id: st.id,
                        muted: rng.below(2) == 1,
                    },
                    4 => Command::SetStripOutput {
                        id: st.id,
                        output: (rng.below(3) != 0)
                            .then(|| project.strips[rng.below(strips as u64) as usize].id),
                    },
                    5 if tracks > 0 => Command::SetTrackStrip {
                        id: project.tracks[rng.below(tracks as u64) as usize].id,
                        strip: (rng.below(4) != 0).then_some(st.id),
                    },
                    6 => {
                        let strip = project.make_strip("bus");
                        Command::InsertStrip {
                            index: rng.below(strips as u64 + 1) as usize,
                            strip: Box::new(strip),
                        }
                    }
                    _ => Command::RemoveStrip { id: st.id },
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
        // Pulsus: pad parameters and pad names. `SetInstrumentParam` is deliberately
        // aimed at *any* track, kit or not, so the "not applicable" path is exercised as
        // well as the working one — a command that fails must leave nothing behind.
        18..=19 if tracks > 0 => {
            let ti = rng.below(tracks as u64) as usize;
            let id = project.tracks[ti].id;
            // Ids 1…9: a kit of two or seven pads has some of them and not others.
            let pad = PadId(1 + rng.below(9) as u16);
            let roll = rng.below(8);
            if roll == 0 {
                Command::SetPadName {
                    track: id,
                    pad,
                    name: format!("Pad{}", rng.below(100)),
                }
            } else if roll == 1 {
                // Pulsus II: a pad's layers replaced, emptied or re-zoned.
                let n = rng.below(4) as usize;
                let layers = (0..n)
                    .map(|i| {
                        let mut l = PadLayer::new(format!("l{i}.wav"));
                        l.vel_lo = 1 + rng.below(64) as u8;
                        l.xfade = rng.below(20) as u8;
                        l.tune = rng.f32(-12.0, 12.0);
                        l
                    })
                    .collect();
                Command::SetPadLayers {
                    track: id,
                    pad,
                    layers,
                }
            } else if roll == 2 {
                let field = KitField::ALL[rng.below(KitField::ALL.len() as u64) as usize];
                Command::SetInstrumentParam {
                    track: id,
                    param: InstrumentParam::PulsusKit(field),
                    value: rng.f32(-10.0, 70_000.0),
                }
            } else {
                let field = PadField::ALL[rng.below(PadField::ALL.len() as u64) as usize];
                Command::SetInstrumentParam {
                    track: id,
                    param: InstrumentParam::Pulsus { pad, field },
                    value: rng.f32(-100.0, 30_000.0),
                }
            }
        }
        // Instruments come and go, and pads come and go inside them. `RemovePad` on a
        // pitch that is not there and `InsertPad` on one that is both have to fail cleanly.
        20 if tracks > 0 => {
            let id = project.tracks[rng.below(tracks as u64) as usize].id;
            let spec = match rng.below(3) {
                0 => InstrumentSpec::Firmus(FirmusParams::default()),
                1 => InstrumentSpec::Pulsus(PulsusParams::of(vec![
                    DrumPad::kick(),
                    DrumPad::snare(),
                ])),
                _ => {
                    let kinds = InstrumentKind::ALL;
                    let kind = kinds[rng.below(kinds.len() as u64) as usize];
                    InstrumentSpec::Native(NativeSpec::new(kind))
                }
            };
            Command::SetInstrument {
                track: id,
                spec: Box::new(spec),
            }
        }
        21 if tracks > 0 => {
            let id = project.tracks[rng.below(tracks as u64) as usize].id;
            let pitch = 35 + rng.below(6) as u8;
            if rng.below(2) == 0 {
                let mut pad = DrumPad::closed_hat();
                pad.pitch = pitch;
                Command::InsertPad {
                    track: id,
                    pad: Box::new(pad),
                }
            } else {
                let pad = PadId(1 + rng.below(9) as u16);
                Command::RemovePad { track: id, pad }
            }
        }
        // Native instruments (ADR 0023): numbers set and cleared, texts set and removed,
        // on any track — so the refused paths (not a native instrument, a key the kind
        // does not have) are walked as well as the working ones.
        22 if tracks > 0 => {
            let track = project.tracks[rng.below(tracks as u64) as usize].id;
            const NUMBERS: [&str; 6] =
                ["gain", "macro1", "macro8", "mod1.amt", "mod2.curve", "nope"];
            const TEXTS: [&str; 6] = [
                "mod1.src",
                "mod1.dst",
                "macro2.name",
                "sample",
                "mod9.via",
                "bad",
            ];
            match rng.below(3) {
                0 => Command::SetInstrumentValue {
                    track,
                    key: NUMBERS[rng.below(6) as usize].to_string(),
                    value: rng.f32(-60.0, 20.0),
                },
                1 => Command::ClearInstrumentValue {
                    track,
                    key: NUMBERS[rng.below(6) as usize].to_string(),
                },
                _ => Command::SetInstrumentText {
                    track,
                    key: TEXTS[rng.below(6) as usize].to_string(),
                    text: if rng.below(3) == 0 {
                        None
                    } else {
                        Some(format!("t{}", rng.below(10)))
                    },
                },
            }
        }
        // Recording (ADR 0025): arm, input, take lanes, the pool, a clip's lane — including
        // the refused paths (a lane on a clip that is not there, a trim that is not finite).
        23 if tracks > 0 => {
            let t = &project.tracks[rng.below(tracks as u64) as usize];
            let id = t.id;
            let clip =
                (!t.clips.is_empty()).then(|| t.clips[rng.below(t.clips.len() as u64) as usize].id);
            match rng.below(6) {
                0 => Command::SetTrackArmed {
                    id,
                    armed: rng.below(2) == 1,
                },
                1 => Command::SetTrackInput {
                    id,
                    input: match rng.below(4) {
                        0 => None,
                        1 => Some(TrackInput {
                            trim_db: f32::NAN,
                            ..TrackInput::mono(0)
                        }),
                        k => {
                            let mut i = if k == 2 {
                                TrackInput::mono(rng.below(4) as u16)
                            } else {
                                TrackInput::stereo(2 * rng.below(2) as u16)
                            };
                            i.monitor = [MonitorMode::Off, MonitorMode::Auto, MonitorMode::On]
                                [rng.below(3) as usize];
                            i.trim_db = rng.f32(-24.0, 24.0);
                            Some(i)
                        }
                    },
                },
                2 => Command::SetTrackTakesOpen {
                    id,
                    open: rng.below(2) == 1,
                },
                3 => Command::SetClipLane {
                    track: id,
                    clip: clip.unwrap_or(ClipId(9_999)),
                    lane: rng.below(4) as u8,
                },
                4 => {
                    let sid = AudioSourceId(project.alloc_id());
                    Command::InsertAudioSource {
                        source: AudioSource {
                            id: sid,
                            name: format!("take {}", sid.0),
                            path: format!("C:/rec/take {}.wav", sid.0),
                            sample_rate: 48_000,
                            channels: 1 + rng.below(2) as u8,
                            frames: rng.below(1_000_000),
                            sha256: None,
                            recorded: true,
                        },
                    }
                }
                _ => {
                    // An audio clip on a take lane, over a source in the pool (or a refused
                    // one over a source that is not).
                    let source = project
                        .audio_pool
                        .first()
                        .map(|s| s.id)
                        .unwrap_or(AudioSourceId(9_998));
                    let mut c = project.make_audio_clip(
                        "take",
                        source,
                        Ticks::beats(rng.below(16) as i64),
                        Ticks::beats(1 + rng.below(4) as i64),
                    );
                    c.lane = rng.below(3) as u8;
                    // A fine offset under the tick, as a recorded take has (round 19).
                    if let Some(a) = c.audio.as_mut() {
                        a.slip_frames = rng.below(25) as i32 - 12;
                    }
                    Command::InsertClip {
                        track: id,
                        clip: Box::new(c),
                    }
                }
            }
        }
        24 => {
            let mut settings = project.recording.clone();
            match rng.below(5) {
                0 => settings.count_in_bars = [0, 1, 2, 4][rng.below(4) as usize],
                1 => {
                    let a = Ticks::beats(rng.below(16) as i64);
                    settings.punch = Some(LoopRegion {
                        start: a,
                        end: a + Ticks::beats(1 + rng.below(8) as i64),
                    });
                    settings.punch_enabled = rng.below(2) == 1;
                }
                2 => {
                    settings.take_mode = [TakeMode::Stack, TakeMode::Replace, TakeMode::Overdub]
                        [rng.below(3) as usize];
                    settings.loop_takes = rng.below(2) == 1;
                }
                3 => {
                    settings.click = [
                        ClickMode::Always,
                        ClickMode::Recording,
                        ClickMode::CountInOnly,
                    ][rng.below(3) as usize];
                    settings.click_db = rng.f32(-24.0, 6.0);
                }
                _ => settings.click_db = f32::INFINITY,
            }
            Command::SetRecordSettings { settings }
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
            applied > 80,
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
                param: FirmusParam::Cutoff.into(),
                value: 800.0,
            },
            Command::SetInstrumentParam {
                track: TrackId(1),
                param: InstrumentParam::Pulsus {
                    pad: PadId(2),
                    field: PadField::Decay,
                },
                value: 0.2,
            },
            Command::SetPadName {
                track: TrackId(1),
                pad: PadId(2),
                name: "Caja".into(),
            },
            Command::SetPadLayers {
                track: TrackId(1),
                pad: PadId(2),
                layers: vec![PadLayer::new("snare.wav")],
            },
        ],
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let back: Command = serde_json::from_str(&json).unwrap();
    assert_eq!(back, cmd);
}
