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
    match rng.below(48) {
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
        12 => {
            let metronome = rng.below(2) == 1;
            let gain_db = rng.f32(-20.0, 6.0);
            Command::Batch {
                label_key: "batch".into(),
                commands: vec![
                    Command::SetMetronome { enabled: metronome },
                    // Round 28 (ADR 0052): FL's PAT / SONG switch, from draws already made
                    // (a new draw would move every later case of the fixed seeds).
                    Command::SetPatternMode {
                        on: metronome != (gain_db > 0.0),
                    },
                    Command::SetMasterGain { gain_db },
                ],
            }
        }
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
            let kind = fuzz_kind(rng, project);
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
                Some(e) => match rng.below(5) {
                    // Round 20: swap the insert for another kind in its place, carrying
                    // over its mix, bypass, key and note source whether the new kind
                    // takes them or not (a refusal must leave nothing behind), now and
                    // then with an id already in the chain.
                    4 => {
                        let kind = fuzz_kind(rng, project);
                        let (effect, mix, bypass, sidechain, midi_from) =
                            (e.id, e.mix, e.bypass, e.sidechain, e.midi_from);
                        let taken = (rng.below(8) == 0)
                            .then(|| chain[rng.below(chain.len() as u64) as usize].id);
                        let mut slot = project.make_effect(kind);
                        slot.mix = mix;
                        slot.bypass = bypass;
                        slot.sidechain = sidechain;
                        slot.midi_from = midi_from;
                        if let Some(id) = taken {
                            slot.id = id;
                        }
                        Command::ReplaceEffect {
                            strip: host,
                            effect,
                            slot: Box::new(slot),
                        }
                    }
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
                            tap: SendTap::ALL[rng.below(2) as usize],
                        },
                    },
                    // A send put back at a place in the list (the inverse of a removal),
                    // and ones that would loop or duplicate — every refusal path too.
                    13 if rng.below(2) == 0 => Command::InsertStripSend {
                        id: st.id,
                        index: rng.below(st.sends.len() as u64 + 2) as usize,
                        send: Send {
                            to: project.strips[rng.below(strips as u64) as usize].id,
                            level: rng.f32(-0.2, 1.2),
                            enabled: rng.below(4) != 0,
                            tap: SendTap::ALL[rng.below(2) as usize],
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
            let spec = match rng.below(4) {
                0 => InstrumentSpec::Firmus(FirmusParams::default()),
                // A hosted instrument (ADR 0028), installed nowhere: the model only.
                3 => {
                    let mut plugin =
                        ExternalPlugin::new(PluginFormat::Clap, "org.example.synth", "Synth");
                    plugin.state = PluginState::new(vec![1, 2, rng.below(255) as u8]);
                    plugin.params.insert(0, -6.0);
                    InstrumentSpec::External(plugin)
                }
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
                    // Round 28 (schema 26): the click's voice and its accent, from the
                    // draws already made (a new draw would move the fixed seeds' cases).
                    let d = settings.click_db;
                    settings.click_sound = cantus_core::ClickSound::ALL[(d.abs() as usize) % 3];
                    settings.click_accent = d > -9.0;
                }
                _ => settings.click_db = f32::INFINITY,
            }
            Command::SetRecordSettings { settings }
        }
        // Automation clips (ADR 0027): made for every kind of target, a vanished one
        // included; curves replaced, broken (refused), cleared; lanes shown and hidden.
        25 if tracks > 0 => automation_command(rng, project),
        // Hosted instruments (ADR 0028): a value set and forgotten, a whole state, on any
        // track — the refused paths (not a hosted instrument, a value that is not finite)
        // are walked as well as the working ones.
        26 if tracks > 0 => {
            let track = project.tracks[rng.below(tracks as u64) as usize].id;
            match rng.below(4) {
                0 => Command::SetInstrumentPluginParam {
                    track,
                    id: rng.below(4) as u32,
                    value: Some(rng.f32(-60.0, 12.0) as f64),
                },
                1 => Command::SetInstrumentPluginParam {
                    track,
                    id: rng.below(4) as u32,
                    value: None,
                },
                2 => Command::SetInstrumentPluginParam {
                    track,
                    id: 0,
                    value: Some(f64::NAN),
                },
                _ => {
                    let mut params = std::collections::BTreeMap::new();
                    params.insert(rng.below(4) as u32, rng.f32(0.0, 1.0) as f64);
                    Command::SetInstrumentPluginState {
                        track,
                        state: PluginState::new(vec![rng.below(255) as u8; 3]),
                        params,
                    }
                }
            }
        }
        // The clipboard (ADR 0036): a paste of clips copied from anywhere (automation
        // clips and a foreign audio file included), a cut of clips or of notes, a paste of
        // notes that may grow its clip — each one Batch, one undo step.
        27 if tracks > 0 => clipboard_paste_clips(rng, project),
        28 if tracks > 0 => clipboard_cut(rng, project),
        29 if tracks > 0 => clipboard_paste_notes(rng, project),
        // A cut of a time range across every track (cmd.cut_time).
        30 if tracks > 0 => clipboard_cut_time(rng, project),
        // Linked clips (ADR 0037): linked copies (the duplicate, the Alt-drag, a linked
        // paste), Make unique, Link to, and edits made through one sibling — notes, name,
        // colour, audio settings, a curve, the whole content — which must reach them all.
        31 | 32 if tracks > 0 => linked_copies(rng, project),
        33 if tracks > 0 => make_unique(rng, project),
        34 if tracks > 0 => link_to(rng, project),
        35 if tracks > 0 => edit_through_sibling(rng, project),
        // A whole track brought in with a link group of its own clips (an MCP call), one
        // time in three with the two members disagreeing (refused).
        36 => linked_track(rng, project),
        // Round 25 (ADR 0041): every routing batch the interface issues — sends, outputs
        // and keys made, moved, removed and converted, a new return, tracks routed, a tap
        // switched — and strips and groups moved along the desk.
        37 | 38 => routing_batch(rng, project),
        39 => order_batch(rng, project),
        // Round 28 (ADR 0054): the pitch editor's edits on an audio clip — notes stored,
        // moved, split, joined, stretched, corrected, the key, A/B, cleared — and the
        // refused ones (an edit that is not valid, a clip that is not audio).
        40 if tracks > 0 => pitch_command(rng, project),
        // Round 28 (ADR 0057): modulators made, removed and edited on tracks, strips and
        // the master — sources, routes, depths, nesting — and the refused ones.
        41 => modulator_command(rng, project),
        // Round 28 (ADR 0058): comp seams moved as a unit (and refused where there is no
        // seam), seam crossfades set, alternate comps kept with their windows; schema 32:
        // a clip made a comp window and not (refused for a clip that is not there), a comp
        // window split (both halves stay the comp's), copied and pasted (never one).
        42 if tracks > 0 => comp_command(rng, project),
        // Round 29 (contract): one arm reserved per builder, each a helper in its own
        // section at the end of this file — tempo (ADR 0060), warp (0061), controllers
        // (0062), isolation (0063), freeze (0064). Until a builder fills its helper it
        // falls through to a project rename, so the draws of every other arm keep their
        // meaning.
        43 => round29_tempo_command(rng, project),
        44 if tracks > 0 => round29_warp_command(rng, project),
        45 => round29_controller_command(rng, project),
        46 => round29_hosting_command(rng, project),
        47 if tracks > 0 => round29_freeze_command(rng, project),
        _ => Command::SetProjectName {
            name: format!("P{}", rng.below(100)),
        },
    }
}

/// 1 to 3 random clips from any tracks.
fn random_clip_refs(rng: &mut Lcg, project: &Project) -> Vec<(TrackId, ClipId)> {
    let all: Vec<(TrackId, ClipId)> = project
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(move |c| (t.id, c.id)))
        .collect();
    if all.is_empty() {
        return Vec::new();
    }
    (0..1 + rng.below(3))
        .map(|_| all[rng.below(all.len() as u64) as usize])
        .collect()
}

fn clipboard_paste_clips(rng: &mut Lcg, project: &mut Project) -> Command {
    use cantus_core::clipboard::{self, ClipOrigin, ClipRow, Clipboard, Payload, SourcePlan};
    let sel = random_clip_refs(rng, project);
    let mut board = Clipboard::new(
        ClipOrigin::default(),
        project,
        clipboard::copy_clips(project, &sel),
    );
    let mut plans = std::collections::HashMap::new();
    // One in four: an audio clip from another project, whose file is added with it.
    if rng.below(4) == 0 {
        let foreign = AudioSourceId(9_000_000 + rng.below(1000));
        let mut clip = project.make_clip("far", Ticks::ZERO, Ticks::beats(1 + rng.below(4) as i64));
        clip.audio = Some(AudioRegion::of(foreign));
        if let Payload::Clips { rows, .. } = &mut board.payload {
            rows.push(ClipRow {
                row: rng.below(3) as usize,
                track: TrackId(1),
                clip,
            });
        }
        let source = AudioSource {
            id: AudioSourceId(project.alloc_id()),
            name: "far".into(),
            path: "D:/elsewhere/far.wav".into(),
            sample_rate: 44_100,
            channels: 1,
            frames: 44_100,
            sha256: None,
            recorded: false,
        };
        plans.insert(foreign, SourcePlan::Insert(source));
    }
    // A foreign payload might carry the project's own source ids: keep them only from
    // the same origin.
    let same_origin = rng.below(3) != 0;
    if same_origin {
        for s in &project.audio_pool {
            plans.entry(s.id).or_insert(SourcePlan::Reuse(s.id));
        }
    }
    let row = rng.below(project.tracks.len() as u64 + 1) as usize;
    let at = Ticks::beats(rng.below(64) as i64);
    let mut next = project.next_id;
    let mut ids = || {
        let id = next;
        next += 1;
        id
    };
    let linked = rng.below(2) == 0;
    let out = clipboard::paste_clips(
        project,
        &board,
        same_origin,
        row,
        at,
        &plans,
        linked,
        &mut ids,
    );
    project.next_id = next;
    Command::Batch {
        label_key: "cmd.paste_clips".into(),
        commands: out.commands,
    }
}

fn clipboard_cut(rng: &mut Lcg, project: &mut Project) -> Command {
    use cantus_core::clipboard;
    let with_notes: Vec<(TrackId, ClipId, Vec<NoteId>)> = project
        .tracks
        .iter()
        .flat_map(|t| {
            t.clips
                .iter()
                .filter(|c| !c.notes.is_empty())
                .map(move |c| (t.id, c.id, c.notes.iter().map(|n| n.id).collect()))
        })
        .collect();
    if rng.below(2) == 0 && !with_notes.is_empty() {
        let (track, clip, notes) = &with_notes[rng.below(with_notes.len() as u64) as usize];
        let take = 1 + rng.below(notes.len() as u64) as usize;
        return Command::Batch {
            label_key: "cmd.cut_notes".into(),
            commands: vec![Command::RemoveNotes {
                track: *track,
                clip: *clip,
                ids: notes[..take].to_vec(),
            }],
        };
    }
    let sel = random_clip_refs(rng, project);
    Command::Batch {
        label_key: "cmd.cut_clips".into(),
        commands: clipboard::cut_clips(project, &sel),
    }
}

fn clipboard_paste_notes(rng: &mut Lcg, project: &mut Project) -> Command {
    use cantus_core::clipboard;
    let notes: Vec<(TrackId, Clip)> = project
        .tracks
        .iter()
        .flat_map(|t| {
            t.clips
                .iter()
                .filter(|c| c.audio.is_none() && c.automation.is_none())
                .map(move |c| (t.id, c.clone()))
        })
        .collect();
    if notes.is_empty() {
        return Command::SetProjectName {
            name: format!("P{}", rng.below(100)),
        };
    }
    // Half the time a linked clip, so a paste that grows it grows its siblings too.
    let linked: Vec<&(TrackId, Clip)> = notes.iter().filter(|(_, c)| c.link.is_some()).collect();
    let (track, clip) = if !linked.is_empty() && rng.below(2) == 0 {
        linked[rng.below(linked.len() as u64) as usize]
    } else {
        &notes[rng.below(notes.len() as u64) as usize]
    };
    let board: Vec<Note> = (0..1 + rng.below(4))
        .map(|i| Note {
            id: NoteId(0),
            start: Ticks::beats(i as i64),
            length: Ticks::QUARTER,
            pitch: 40 + rng.below(40) as u8,
            velocity: 1 + rng.below(127) as u8,
            muted: rng.below(5) == 0,
        })
        .collect();
    // Sometimes well past the end, so the clip has to grow (or its loop lengthen).
    let at = clip.offset + Ticks::beats(rng.below(24) as i64);
    let mut next = project.next_id;
    let (mut commands, _) = clipboard::paste_notes(*track, clip, &board, at, &mut || {
        let id = next;
        next += 1;
        id
    });
    project.next_id = next;
    // As the piano roll does: the copies that showed what it showed grow with it.
    commands.extend(links::follow_window(project, (*track, clip.id), &commands));
    Command::Batch {
        label_key: "cmd.paste".into(),
        commands,
    }
}

/// A cut of a random `[from, to)` across every track, as the arrangement's Cut of a time
/// selection runs it. Two times in three the batch first lays a clip under the range that
/// the crop treats specially — a looping note clip, a looping automation clip, a reversed
/// audio clip with fades — so those paths run on every seed, whatever else is there.
fn clipboard_cut_time(rng: &mut Lcg, project: &mut Project) -> Command {
    use cantus_core::clipboard;
    let from = Ticks(rng.below(Ticks::beats(24).0 as u64) as i64);
    let to = from + Ticks(1 + rng.below(Ticks::beats(12).0 as u64) as i64);
    let tid = project.tracks[rng.below(project.tracks.len() as u64) as usize].id;
    let start = Ticks(from.0 - rng.below(Ticks::beats(6).0 as u64) as i64).max(Ticks::ZERO);
    let length = Ticks::beats(1 + rng.below(16) as i64);
    let period = Some(Ticks(1 + rng.below(Ticks::beats(5).0 as u64) as i64));
    let mut prelude = Vec::new();
    match rng.below(6) {
        0 | 1 => {
            let mut c = project.make_clip("loop", start, length);
            c.content_length = period;
            for _ in 0..1 + rng.below(5) {
                let at = Ticks(rng.below(Ticks::beats(6).0 as u64) as i64);
                c.notes
                    .push(project.make_note(at, Ticks::QUARTER, 40 + rng.below(40) as u8, 90));
            }
            // Notes are kept sorted, as every editor leaves them.
            c.notes.sort_by_key(|n| (n.start, n.pitch));
            prelude.push(Command::InsertClip {
                track: tid,
                clip: Box::new(c),
            });
        }
        2 | 3 => {
            let mut pts = random_points(rng);
            pts.push(AutoPoint::new(Ticks::ZERO, rng.f32(0.0, 1.0)));
            pts.sort_by_key(|p| p.tick);
            let target = AutoTarget::TrackGain(tid);
            let mut c = project.make_automation_clip("auto", target, start, length, pts);
            c.content_length = period;
            prelude.push(Command::InsertClip {
                track: tid,
                clip: Box::new(c),
            });
        }
        4 => {
            let source = match project.audio_pool.first() {
                Some(s) => s.id,
                None => {
                    let s = AudioSource {
                        id: AudioSourceId(project.alloc_id()),
                        name: "cut".into(),
                        path: "C:/nowhere/cut.wav".into(),
                        sample_rate: 48_000,
                        channels: 1,
                        frames: 48_000 * 60,
                        sha256: None,
                        recorded: false,
                    };
                    let id = s.id;
                    prelude.push(Command::InsertAudioSource { source: s });
                    id
                }
            };
            let mut c = project.make_audio_clip("rev", source, start, length);
            let a = c.audio.as_mut().unwrap();
            a.reverse = rng.below(2) == 0;
            a.fade_in = Ticks(rng.below(length.0 as u64) as i64);
            a.fade_out = Ticks(rng.below(length.0 as u64) as i64);
            prelude.push(Command::InsertClip {
                track: tid,
                clip: Box::new(c),
            });
        }
        _ => {}
    }
    // The crop is planned on the project as the prelude leaves it.
    let mut scratch = CommandBus::new(project.clone());
    if !prelude.is_empty()
        && scratch
            .execute(
                Command::Batch {
                    label_key: String::new(),
                    commands: prelude.clone(),
                },
                Origin::System,
            )
            .is_err()
    {
        return Command::SetProjectName {
            name: format!("P{}", rng.below(100)),
        };
    }
    let mut next = project.next_id;
    let crops = clipboard::time_crops(scratch.project(), from, to, &mut || {
        let id = next;
        next += 1;
        id
    });
    project.next_id = next;
    prelude.extend(crops.into_iter().flat_map(|c| c.cut));
    Command::Batch {
        label_key: "cmd.cut_time".into(),
        commands: prelude,
    }
}

/// A track whose two clips name one new link group; one time in three their notes differ
/// and `InsertTrack` must refuse it.
fn linked_track(rng: &mut Lcg, project: &mut Project) -> Command {
    let mut t = project.make_track(format!("L{}", rng.below(100)));
    let link = Some(LinkId(project.alloc_id()));
    let mut a = project.make_clip("la", Ticks::ZERO, Ticks::beats(4));
    a.notes.push(project.make_note(
        Ticks(rng.below(3000) as i64),
        Ticks::QUARTER,
        rng.below(128) as u8,
        1 + rng.below(127) as u8,
    ));
    a.link = link;
    let mut b = Clip {
        id: ClipId(project.alloc_id()),
        start: Ticks::beats(8),
        ..a.clone()
    };
    if rng.below(3) == 0 {
        b.notes[0].pitch = (b.notes[0].pitch + 1) % 128;
    }
    t.clips = vec![a, b];
    Command::Batch {
        label_key: "fuzz.linked_track".into(),
        commands: vec![Command::InsertTrack {
            index: rng.below(project.tracks.len() as u64 + 1) as usize,
            track: Box::new(t),
        }],
    }
}

/// The kinds the fuzz knew before round 28: the generator draws among these, so every arm
/// draws the same numbers as before (the coverage check counts on the seeds).
const KINDS_BEFORE_ROUND_28: usize = 12;

/// An effect kind for an insert or a swap: one of the older kinds by the generator, and
/// where it names the delay or the reverb (which take neither a key nor notes), every
/// other time a kind of round 28's, chosen by the next id rather than the generator.
fn fuzz_kind(rng: &mut Lcg, project: &Project) -> EffectKind {
    let kind = EffectKind::ALL[rng.below(KINDS_BEFORE_ROUND_28 as u64) as usize];
    if matches!(kind, EffectKind::Delay | EffectKind::Reverb) && project.next_id.is_multiple_of(2) {
        let new = &EffectKind::ALL[KINDS_BEFORE_ROUND_28..];
        new[(project.next_id / 2) as usize % new.len()]
    } else {
        kind
    }
}

fn strip_at(rng: &mut Lcg, project: &Project) -> StripId {
    project.strips[rng.below(project.strips.len() as u64) as usize].id
}

/// 1 to 3 strips, now and then the master or a stale id among them (refused or skipped).
fn some_strips(rng: &mut Lcg, project: &Project) -> Vec<StripId> {
    (0..1 + rng.below(3))
        .map(|_| match rng.below(12) {
            0 => StripId::MASTER,
            1 => StripId(77_777),
            _ => strip_at(rng, project),
        })
        .collect()
}

/// A routing batch built by `routing::plan` from random strips and routes — what the
/// cables, the matrix, the menus and the route-here plate issue. When the plan says there
/// is nothing to do, a bare send is tried instead (applied or refused).
fn routing_batch(rng: &mut Lcg, project: &mut Project) -> Command {
    use cantus_core::routing::{self, plan, End, Route};
    let a = strip_at(rng, project);
    let b = strip_at(rng, project);
    let edges = routing::edges(project);
    let route = edges
        .get(rng.below(edges.len() as u64) as usize)
        .map(|e| match e.kind {
            routing::EdgeKind::Output => Route::output(e.from, e.to),
            routing::EdgeKind::Send { .. } => Route::send(e.from, e.to),
            routing::EdgeKind::Key { effect } => Route::key(e.from, e.to, effect),
        });
    let cmd = match rng.below(11) {
        0 => {
            let sources = some_strips(rng, project);
            plan::add_sends(project, &sources, b)
        }
        1 => {
            let sources = some_strips(rng, project);
            let to = if rng.below(4) == 0 {
                StripId::MASTER
            } else {
                b
            };
            plan::replace_outputs(project, &sources, to)
        }
        2 => {
            let host = if rng.below(5) == 0 {
                StripId::MASTER
            } else {
                b
            };
            let chain = project.effects_of(host).cloned().unwrap_or_default();
            chain
                .get(rng.below(chain.len().max(1) as u64) as usize)
                .and_then(|e| plan::key(project, a, host, e.id))
        }
        3 => {
            let kind = [EffectKind::Compressor, EffectKind::Gate, EffectKind::Delay]
                [rng.below(3) as usize];
            let slot = project.make_effect(kind);
            let label = if kind == EffectKind::Gate {
                "cmd.gate_key"
            } else {
                "cmd.comp_duck"
            };
            plan::key_with(project, a, b, slot, label)
        }
        4 => route.and_then(|r| {
            let end = if rng.below(2) == 0 {
                End::Source
            } else {
                End::Destination
            };
            let to = if rng.below(6) == 0 {
                StripId::MASTER
            } else {
                b
            };
            plan::move_route(project, r, end, to)
        }),
        5 => route.and_then(|r| plan::remove_route(project, r)),
        6 => {
            let to = project.strip(a).and_then(|s| s.sends.first()).map(|x| x.to);
            to.and_then(|to| plan::send_to_output(project, a, to))
        }
        7 => {
            // A strip routed somewhere other than the master, when there is one.
            let routed: Vec<StripId> = project
                .strips
                .iter()
                .filter(|s| s.output.is_some())
                .map(|s| s.id)
                .collect();
            let from = routed
                .get(rng.below(routed.len().max(1) as u64) as usize)
                .copied()
                .unwrap_or(a);
            plan::output_to_send(project, from)
        }
        8 => {
            let ret = project.make_strip("Return");
            let effect = match rng.below(3) {
                0 => None,
                1 => Some(project.make_effect(EffectKind::Space)),
                _ => Some(project.make_effect(EffectKind::Delay)),
            };
            let sources = some_strips(rng, project);
            plan::new_return(project, &sources, ret, effect)
        }
        9 if !project.tracks.is_empty() => {
            let n = project.tracks.len() as u64;
            let tracks: Vec<TrackId> = (0..1 + rng.below(2))
                .map(|_| project.tracks[rng.below(n) as usize].id)
                .collect();
            let to = (rng.below(4) != 0).then_some(b);
            plan::route_tracks(project, &tracks, to)
        }
        _ => {
            let to = project.strip(a).and_then(|s| s.sends.last()).map(|x| x.to);
            let tap = SendTap::ALL[rng.below(2) as usize];
            to.and_then(|to| plan::set_send_tap(project, a, to, tap))
        }
    };
    cmd.unwrap_or(Command::SetStripSend {
        id: a,
        send: Send::new(b),
    })
}

/// Strips and groups moved along the desk by `routing::order`: drags of a strip or a
/// selection to a spot in any dock, a group dragged by its separator, Alt+←/→.
fn order_batch(rng: &mut Lcg, project: &mut Project) -> Command {
    use cantus_core::routing::order::{self, Spot};
    let dock = [Dock::Left, Dock::Middle, Dock::Right][rng.below(3) as usize];
    let in_dock: Vec<StripId> = project
        .strips
        .iter()
        .filter(|s| s.dock == dock)
        .map(|s| s.id)
        .collect();
    let spot = match in_dock.get(rng.below(in_dock.len() as u64 + 1) as usize) {
        Some(id) => Spot::before(dock, *id),
        None => Spot::end(dock),
    };
    // A group with members when there is one (an empty group moves nothing).
    let groups: Vec<GroupId> = project
        .strip_groups
        .iter()
        .filter(|g| project.strips_in_group(g.id).next().is_some())
        .map(|g| g.id)
        .collect();
    if groups.is_empty() || rng.below(10) == 0 {
        // A group of 1 to 3 strips (the desk's *Create group*), so the group moves have
        // something to move.
        let g = project.make_strip_group("grp", Color::PALETTE[rng.below(8) as usize]);
        let id = g.id;
        let mut commands = vec![Command::InsertStripGroup {
            index: project.strip_groups.len(),
            group: Box::new(g),
        }];
        let start = rng.below(project.strips.len() as u64) as usize;
        for s in project
            .strips
            .iter()
            .skip(start)
            .take(1 + rng.below(3) as usize)
        {
            commands.push(Command::SetStripGroup {
                id: s.id,
                group: Some(id),
            });
        }
        return Command::Batch {
            label_key: "fuzz.group".into(),
            commands,
        };
    }
    let group = groups.get(rng.below(groups.len() as u64) as usize).copied();
    let delta = [-2, -1, 1, 2][rng.below(4) as usize];
    let cmd = match rng.below(6) {
        0 | 1 => {
            let sel = some_strips(rng, project);
            order::move_strips(project, &sel, spot)
        }
        2 | 5 => group.and_then(|g| order::move_group(project, g, spot)),
        3 => {
            let sel = match (rng.below(3), group) {
                (0, Some(g)) => project.strips_in_group(g).map(|s| s.id).collect(),
                _ => some_strips(rng, project),
            };
            order::nudge(project, &sel, delta)
        }
        _ => group.and_then(|g| order::nudge_group(project, g, delta)),
    };
    cmd.unwrap_or(Command::MoveStrip {
        id: strip_at(rng, project),
        index: rng.below(project.strips.len() as u64) as usize,
    })
}

/// Which routing batch a generated command is, for the coverage check (round 25).
fn routing_kind(cmd: &Command) -> Option<usize> {
    let Command::Batch { label_key, .. } = cmd else {
        return None;
    };
    [
        "cmd.add_send",
        "cmd.replace_output",
        "cmd.key_sidechain",
        "cmd.gate_key",
        "cmd.comp_duck",
        "cmd.move_route",
        "cmd.remove_route",
        "cmd.send_to_output",
        "cmd.output_to_send",
        "cmd.new_return",
        "cmd.route_tracks",
        "cmd.set_send_tap",
        "cmd.move_strips",
        "cmd.move_group",
    ]
    .iter()
    .position(|k| k == label_key)
}

/// Fresh ids from the project's counter, written back when the closure is done.
fn with_ids<R>(project: &mut Project, f: impl FnOnce(&Project, &mut dyn FnMut() -> u64) -> R) -> R {
    let mut next = project.next_id;
    let out = f(project, &mut || {
        let id = next;
        next += 1;
        id
    });
    project.next_id = next;
    out
}

/// Linked copies of 1 to 3 clips, on their own track or one below (the duplicate and the
/// Alt-drag with linking on). One time in eight, a clip that does not match the group it
/// names is slipped in, and the whole batch must be refused.
fn linked_copies(rng: &mut Lcg, project: &mut Project) -> Command {
    let sel = random_clip_refs(rng, project);
    let shift = Ticks::beats(1 + rng.below(16) as i64);
    let below = rng.below(2) == 1;
    let spoil = rng.below(8) == 0;
    let mut commands = with_ids(project, |p, ids| {
        let mut linker = links::Linker::new();
        let mut out = Vec::new();
        for (t, c) in &sel {
            let Some(index) = p.track_index(*t) else {
                continue;
            };
            let clip = p.tracks[index].clip(*c).expect("listed");
            let to = if below && clip.automation.is_none() {
                p.tracks[(index + 1).min(p.tracks.len() - 1)].id
            } else {
                *t
            };
            let (pre, copy) = linker.copy(*t, clip, clip.start + shift, ids);
            out.extend(pre);
            out.push(Command::InsertClip {
                track: to,
                clip: Box::new(copy),
            });
        }
        out
    });
    if spoil {
        if let Some(Command::InsertClip { clip, .. }) = commands.last_mut() {
            clip.name.push('!');
        }
    }
    Command::Batch {
        label_key: "cmd.duplicate_clips".into(),
        commands,
    }
}

fn make_unique(rng: &mut Lcg, project: &mut Project) -> Command {
    let linked: Vec<(TrackId, ClipId)> = project
        .tracks
        .iter()
        .flat_map(|t| {
            t.clips
                .iter()
                .filter(|c| c.link.is_some())
                .map(move |c| (t.id, c.id))
        })
        .collect();
    let sel = if linked.is_empty() || rng.below(4) == 0 {
        random_clip_refs(rng, project)
    } else {
        vec![linked[rng.below(linked.len() as u64) as usize]]
    };
    Command::Batch {
        label_key: "cmd.make_unique".into(),
        commands: links::make_unique(project, &sel),
    }
}

/// Link 1 to 3 clips to another: they take its content. One time in six a bare
/// `SetClipLink` into a group the clip does not match is tried instead (refused).
fn link_to(rng: &mut Lcg, project: &mut Project) -> Command {
    let sel = random_clip_refs(rng, project);
    let all = random_clip_refs(rng, project);
    let (Some(target), Some(first)) = (all.first().copied(), sel.first().copied()) else {
        return Command::SetProjectName {
            name: format!("P{}", rng.below(100)),
        };
    };
    if rng.below(6) == 0 {
        let link = project
            .track(target.0)
            .and_then(|t| t.clip(target.1))
            .and_then(|c| c.link)
            .unwrap_or(LinkId(project.next_id + 7));
        return Command::SetClipLink {
            track: first.0,
            clip: first.1,
            link: Some(link),
        };
    }
    // Half the time the target is one the *Link to…* picker offers (round 26): an entry of
    // `links::groups`, by its first member. Either way `would_link` — what the picker dims
    // a row by — must say beforehand exactly whether the batch links anything.
    let target = match links::groups(project) {
        // Chosen from the next id rather than the generator, so every other arm draws the
        // same numbers as before (the coverage check counts on the seeds).
        g if !g.is_empty() && project.next_id.is_multiple_of(2) => {
            g[(project.next_id / 2) as usize % g.len()].first
        }
        _ => target,
    };
    let commands = with_ids(project, |p, ids| links::link_to(p, &sel, target, ids));
    assert_eq!(
        links::would_link(project, &sel, target).is_ok(),
        !commands.is_empty(),
        "would_link disagrees with link_to: {sel:?} → {target:?}"
    );
    // And the picker's summary of the selection answers every entry as `would_link` does.
    let summary = links::Selection::of(project, &sel);
    for g in links::groups(project) {
        assert_eq!(
            summary.would_link(&g),
            links::would_link(project, &sel, g.first),
            "the selection summary disagrees: {sel:?} → {g:?}"
        );
    }
    Command::Batch {
        label_key: "cmd.link_clips".into(),
        commands,
    }
}

/// An edit through one member of a group of two or more.
fn edit_through_sibling(rng: &mut Lcg, project: &mut Project) -> Command {
    let sizes = links::sizes(project);
    let shared: Vec<(TrackId, Clip)> = project
        .tracks
        .iter()
        .flat_map(|t| {
            t.clips
                .iter()
                .filter(|c| c.link.is_some_and(|l| sizes[&l] > 1))
                .map(move |c| (t.id, c.clone()))
        })
        .collect();
    if shared.is_empty() {
        return linked_copies(rng, project);
    }
    let (track, c) = shared[rng.below(shared.len() as u64) as usize].clone();
    let clip = c.id;
    let cmd = match rng.below(7) {
        0 => Command::InsertNotes {
            track,
            clip,
            notes: vec![project.make_note(
                Ticks(rng.below(4000) as i64),
                Ticks(1 + rng.below(900) as i64),
                rng.below(128) as u8,
                1 + rng.below(127) as u8,
            )],
        },
        1 if !c.notes.is_empty() => Command::RemoveNotes {
            track,
            clip,
            ids: vec![c.notes[rng.below(c.notes.len() as u64) as usize].id],
        },
        2 if !c.notes.is_empty() => {
            let mut n = c.notes[rng.below(c.notes.len() as u64) as usize].clone();
            n.start = Ticks(rng.below(4000) as i64);
            n.velocity = 1 + rng.below(127) as u8;
            Command::SetNotes {
                track,
                clip,
                notes: vec![n],
            }
        }
        3 => Command::SetClipColor {
            track,
            clip,
            color: Some(Color::PALETTE[rng.below(Color::PALETTE.len() as u64) as usize]),
        },
        4 if c.audio.is_some() => {
            let mut a = c.audio.clone().unwrap();
            a.gain_db = rng.f32(-24.0, 6.0);
            a.fade_in = Ticks(rng.below(900) as i64);
            a.reverse = rng.below(2) == 0;
            Command::SetClipAudio {
                track,
                clip,
                audio: Some(a),
            }
        }
        4 if c.automation.is_some() => {
            let mut a = c.automation.clone().unwrap();
            a.points = random_points(rng);
            Command::SetClipAutomation {
                track,
                clip,
                automation: Some(a),
            }
        }
        5 => {
            let mut content = links::ClipContent::of(&c);
            content.name = format!("n{}", rng.below(100));
            if c.audio.is_none() && c.automation.is_none() {
                content.notes = vec![project.make_note(
                    Ticks(rng.below(2000) as i64),
                    Ticks::QUARTER,
                    rng.below(128) as u8,
                    90,
                )];
            }
            Command::SetClipContent {
                track,
                clip,
                content: Box::new(content),
            }
        }
        _ => Command::SetClipName {
            track,
            clip,
            name: format!("s{}", rng.below(100)),
        },
    };
    Command::Batch {
        label_key: "fuzz.sibling_edit".into(),
        commands: vec![cmd],
    }
}

fn random_points(rng: &mut Lcg) -> Vec<AutoPoint> {
    let mut pts: Vec<AutoPoint> = (0..rng.below(6))
        .map(|_| AutoPoint {
            tick: Ticks(rng.below(4000) as i64),
            value: rng.f32(0.0, 1.0),
            tension: rng.f32(-1.0, 1.0),
            shape: Shape::ALL[rng.below(Shape::ALL.len() as u64) as usize],
        })
        .collect();
    pts.sort_by_key(|p| p.tick);
    pts
}

fn automation_command(rng: &mut Lcg, project: &mut Project) -> Command {
    let tracks = project.tracks.len() as u64;
    let t = &project.tracks[rng.below(tracks) as usize];
    let tid = t.id;
    let autos: Vec<ClipId> = t
        .clips
        .iter()
        .filter(|c| c.automation.is_some())
        .map(|c| c.id)
        .collect();
    match rng.below(4) {
        0 | 1 if !autos.is_empty() => {
            let clip = autos[rng.below(autos.len() as u64) as usize];
            let mut auto = project
                .track(tid)
                .and_then(|t| t.clip(clip))
                .and_then(|c| c.automation.clone())
                .unwrap();
            match rng.below(4) {
                0 => auto.points = random_points(rng),
                // Out of range and out of order: refused, and nothing changes.
                1 => {
                    auto.points = vec![AutoPoint::new(Ticks(9), 3.0), AutoPoint::new(Ticks(1), 0.5)]
                }
                2 => auto.target = AutoTarget::TrackPan(tid),
                _ => {
                    return Command::SetClipAutomation {
                        track: tid,
                        clip,
                        automation: None,
                    }
                }
            }
            Command::SetClipAutomation {
                track: tid,
                clip,
                automation: Some(auto),
            }
        }
        2 => Command::SetTrackAutomationOpen {
            id: tid,
            open: rng.below(2) == 1,
        },
        _ => {
            let strip = project.strips[rng.below(project.strips.len() as u64) as usize].id;
            let effect = project.all_effects().next().map(|(_, e)| e.id);
            let target = match rng.below(7) {
                0 => AutoTarget::StripGain(strip),
                // A send's level (round 25: a moved send takes its clips along).
                6 => match project
                    .strips
                    .iter()
                    .find_map(|s| s.sends.first().map(|x| (s.id, x.to)))
                {
                    Some((strip, to)) => AutoTarget::Send { strip, to },
                    None => AutoTarget::StripPan(strip),
                },
                1 => AutoTarget::MasterWidth,
                2 => AutoTarget::TrackGain(tid),
                3 => match effect {
                    Some(effect) => AutoTarget::Effect {
                        effect,
                        key: "mix".into(),
                    },
                    None => AutoTarget::MasterPan,
                },
                4 => AutoTarget::Effect {
                    effect: EffectId(77_777),
                    key: "nothing".into(),
                },
                _ => AutoTarget::Firmus {
                    track: tid,
                    param: FirmusParam::Cutoff,
                },
            };
            let pts = random_points(rng);
            let clip = project.make_automation_clip(
                "auto",
                target,
                Ticks::beats(rng.below(32) as i64),
                Ticks::beats(1 + rng.below(8) as i64),
                pts,
            );
            Command::InsertClip {
                track: tid,
                clip: Box::new(clip),
            }
        }
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
    let mut exercised = [0usize; 14];
    let mut routed = [0usize; 14];
    // Pitch edits applied (ADR 0054): set, and cleared.
    let mut pitched = [0usize; 2];
    // Modulators (ADR 0057): inserted, removed, set; comping (ADR 0058): a seam moved, a
    // comps list keeping windows, a clip's comp flag set (schema 32), a comp window split
    // or copied.
    let mut modded = [0usize; 3];
    let mut comped = [0usize; 4];
    // Round 29, warp (ADR 0061): a warp set, cleared, and a marker edit applied.
    let mut warped = [0usize; 3];
    // The tempo track, metre and markers (ADR 0060): one count per command kind.
    let mut tempoed = [0usize; 12];
    for seed in 1..=60u64 {
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
            let kind = clipboard_kind(&cmd);
            let route = routing_kind(&cmd);
            let pitch = match &cmd {
                Command::SetClipPitch { pitch, .. } => Some(usize::from(pitch.is_none())),
                _ => None,
            };
            let modulator = match &cmd {
                Command::InsertModulator { .. } => Some(0),
                Command::RemoveModulator { .. } => Some(1),
                Command::SetModulator { .. } => Some(2),
                _ => None,
            };
            let warp = match &cmd {
                Command::SetClipWarp { warp: None, .. } => Some(1),
                Command::SetClipWarp {
                    op: cantus_core::warp::WarpOp::AddMarker | cantus_core::warp::WarpOp::MoveMarker,
                    ..
                } => Some(2),
                Command::SetClipWarp { .. } => Some(0),
                _ => None,
            };
            let comp = match &cmd {
                Command::Batch { commands, .. }
                    if matches!(commands.last(), Some(Command::SetCompSeam { .. })) =>
                {
                    Some(0)
                }
                Command::SetTrackComps { comps, .. }
                    if comps.iter().any(|c| !c.windows.is_empty()) =>
                {
                    Some(1)
                }
                Command::SetClipComp { .. } => Some(2),
                Command::Batch { label_key, .. } if label_key == "cmd.comp_window_copies" => {
                    Some(3)
                }
                _ => None,
            };
            // The clips a comp window's copies batch inserts, and whether each is the comp's:
            // the split's half (the first) is, the copies are not.
            let made: Vec<(TrackId, ClipId, bool)> = match &cmd {
                Command::Batch {
                    label_key,
                    commands,
                } if label_key == "cmd.comp_window_copies" => commands
                    .iter()
                    .filter_map(|c| match c {
                        Command::InsertClip { track, clip } => Some((*track, clip.id)),
                        _ => None,
                    })
                    .enumerate()
                    .map(|(i, (t, c))| (t, c, i == 0))
                    .collect(),
                _ => Vec::new(),
            };
            let tempo = tempo_kind(&cmd);
            let ok = bus.execute(cmd, origin).is_ok();
            // Schema 32: a split comp window's half is the comp's; a copy or a paste is not.
            if ok {
                for (t, c, half) in &made {
                    let clip = bus.project().track(*t).and_then(|t| t.clip(*c)).unwrap();
                    assert_eq!(clip.comp, *half, "seed {seed}: clip {c:?}");
                }
            }
            // Every group's members share their content, whatever was applied or refused.
            links::check(bus.project())
                .unwrap_or_else(|e| panic!("seed {seed}: link invariant broken: {e}"));
            if ok {
                applied += 1;
                if let Some(k) = kind {
                    exercised[k] += 1;
                }
                if let Some(k) = route {
                    routed[k] += 1;
                }
                if let Some(k) = pitch {
                    pitched[k] += 1;
                }
                if let Some(k) = modulator {
                    modded[k] += 1;
                }
                if let Some(k) = warp {
                    warped[k] += 1;
                }
                if let Some(k) = comp {
                    comped[k] += 1;
                }
                if let Some(k) = tempo {
                    tempoed[k] += 1;
                }
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
            links::check(bus.project())
                .unwrap_or_else(|e| panic!("seed {seed}: link invariant broken by undo: {e}"));
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
    // The clipboard arms must really run: pastes of clips, ones that bring a file with
    // them, cuts of clips and of notes, and note pastes that grow their clip; and the link
    // arms: linked copies, Make unique, Link to (content replaced), an edit through a
    // sibling of a real group, a linked paste.
    for (k, n) in exercised.iter().enumerate() {
        assert!(*n > 0, "clipboard case {k} never applied: {exercised:?}");
    }
    // Every routing batch and every desk move must really apply somewhere (round 25).
    for (k, n) in routed.iter().enumerate() {
        assert!(*n > 0, "routing batch {k} never applied: {routed:?}");
    }
    assert!(
        pitched.iter().all(|n| *n > 0),
        "pitch edits never applied: {pitched:?}"
    );
    assert!(
        modded.iter().all(|n| *n > 0),
        "modulator commands never applied: {modded:?}"
    );
    assert!(
        comped.iter().all(|n| *n > 0),
        "comp commands never applied: {comped:?}"
    );
    assert!(
        warped.iter().all(|n| *n > 0),
        "warp commands never applied: {warped:?}"
    );
    assert!(
        tempoed.iter().all(|n| *n > 0),
        "tempo commands never applied: {tempoed:?}"
    );
}

/// Whether `commands` change the window or loop of a clip other than `clip`.
fn has_other_window(commands: &[Command], clip: Option<ClipId>) -> bool {
    commands.iter().any(|c| match c {
        Command::SetClipBounds { clip: id, .. }
        | Command::SetClipOffset { clip: id, .. }
        | Command::SetClipLoop { clip: id, .. } => Some(*id) != clip,
        _ => false,
    })
}

/// Which clipboard case a generated command is, for the coverage check: 0 a clip paste,
/// 1 a paste that inserts a file, 2 a cut of clips, 3 a cut of notes, 4 a note paste
/// that grows its clip, 5 a time cut that cuts something, 6 one that leaves a piece of
/// a looping automation clip (the loop-phase crop); 7 a linked copy, 8 a Make unique, 9 a
/// Link to that replaced a content, 10 an edit through a sibling, 11 a linked paste; 12 a
/// note paste that grew a linked clip's siblings with it, 13 a track bringing its own link
/// group.
fn clipboard_kind(cmd: &Command) -> Option<usize> {
    let Command::Batch {
        label_key,
        commands,
    } = cmd
    else {
        return None;
    };
    let has = |f: fn(&Command) -> bool| commands.iter().any(f);
    match label_key.as_str() {
        "cmd.paste_clips"
            if has(|c| matches!(c, Command::InsertClip { clip, .. } if clip.link.is_some())) =>
        {
            Some(11)
        }
        "cmd.duplicate_clips" if !commands.is_empty() => Some(7),
        "cmd.make_unique" if !commands.is_empty() => Some(8),
        "cmd.link_clips" if has(|c| matches!(c, Command::SetClipContent { .. })) => Some(9),
        "fuzz.sibling_edit" => Some(10),
        "fuzz.linked_track" => Some(13),
        "cmd.paste"
            if {
                let pasted_into = commands.iter().find_map(|c| match c {
                    Command::InsertNotes { clip, .. } => Some(*clip),
                    _ => None,
                });
                has_other_window(commands, pasted_into)
            } =>
        {
            Some(12)
        }
        "cmd.paste_clips" if has(|c| matches!(c, Command::InsertAudioSource { .. })) => Some(1),
        "cmd.paste_clips" if has(|c| matches!(c, Command::InsertClip { .. })) => Some(0),
        "cmd.cut_clips" if !commands.is_empty() => Some(2),
        "cmd.cut_notes" => Some(3),
        "cmd.paste"
            if has(|c| {
                matches!(
                    c,
                    Command::SetClipBounds { .. } | Command::SetClipLoop { .. }
                )
            }) =>
        {
            Some(4)
        }
        "cmd.cut_time"
            if has(|c| {
                matches!(c, Command::InsertClip { clip, .. }
                    if clip.automation.is_some() && clip.content_length.is_none())
            }) =>
        {
            Some(6)
        }
        "cmd.cut_time"
            if has(|c| {
                matches!(
                    c,
                    Command::RemoveClip { .. } | Command::SetClipBounds { .. }
                )
            }) =>
        {
            Some(5)
        }
        _ => None,
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

/// One pitch edit (ADR 0054) on an audio clip, made the way the editor makes it: the
/// current edit (or the notes of a pretend analysis), one of the edit functions, one
/// `SetClipPitch`. One time in eight it is refused: not valid, or not an audio clip.
fn pitch_command(rng: &mut Lcg, project: &mut Project) -> Command {
    use cantus_core::pitch::{edit, PitchEdit, PitchKey, PitchNote, PitchOp};
    let audio: Vec<(TrackId, Clip)> = project
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(move |c| (t.id, c.clone())))
        .filter(|(_, c)| c.audio.is_some())
        .collect();
    let Some((track, clip)) =
        (!audio.is_empty()).then(|| audio[rng.below(audio.len() as u64) as usize].clone())
    else {
        // No audio clip yet: a source and a clip over it, in one step.
        let tid = project.tracks[rng.below(project.tracks.len() as u64) as usize].id;
        let s = AudioSource {
            id: AudioSourceId(project.alloc_id()),
            name: "voice".into(),
            path: "C:/rec/voice.wav".into(),
            sample_rate: 48_000,
            channels: 1,
            frames: 48_000 * 8,
            sha256: None,
            recorded: true,
        };
        let c = project.make_audio_clip("voice", s.id, Ticks::ZERO, Ticks::beats(16));
        return Command::Batch {
            label_key: "fuzz.voice".into(),
            commands: vec![
                Command::InsertAudioSource { source: s },
                Command::InsertClip {
                    track: tid,
                    clip: Box::new(c),
                },
            ],
        };
    };
    let fresh = || {
        let mut notes = Vec::new();
        let mut at = 2_000 + rng.below(20_000);
        for id in 0..1 + rng.below(6) as u32 {
            let len = 4_800 + rng.below(40_000);
            let end = (at + len).min(48_000 * 8);
            if end <= at + 1_000 {
                break;
            }
            let sung = 48.0 + rng.f32(0.0, 24.0);
            notes.push(PitchNote::sung(id, at, end, sung, rng.f32(-40.0, -6.0)));
            at = end + rng.below(12_000);
        }
        PitchEdit {
            rate: 48_000,
            frames: 48_000 * 8,
            analyser: 1,
            key: PitchKey {
                tonic: rng.below(12) as u8,
                scale: rng.below(13) as u8,
            },
            notes,
            bypass: false,
        }
    };
    let current = clip.audio.as_ref().and_then(|a| a.pitch.clone());
    let mut e = current.map(|b| *b).unwrap_or_else(fresh);
    let ids: Vec<u32> = e.notes.iter().map(|n| n.id).collect();
    let id = ids.first().copied().unwrap_or(0);
    let op = match rng.below(13) {
        12 => {
            return Command::SetTrackPitchOpen {
                id: track,
                open: rng.below(2) == 0,
            };
        }
        0 => PitchOp::Notes,
        1 => {
            edit::move_pitch(&mut e, &ids, rng.f32(-5.0, 5.0));
            PitchOp::Pitch
        }
        2 => {
            let f = edit::Field::ALL[rng.below(5) as usize];
            edit::set_field(&mut e, &ids, f, rng.f32(-12.0, 12.0));
            f.op()
        }
        3 => {
            if let Some(n) = e.note(id).cloned() {
                edit::set_timing(
                    &mut e,
                    id,
                    n.start + rng.below(4_000),
                    n.end + rng.below(9_000),
                );
            }
            PitchOp::Timing
        }
        4 => {
            if let Some(n) = e.note(id).cloned() {
                let at = n.src_start + rng.below((n.src_end - n.src_start).max(1));
                edit::split(&mut e, id, at, None);
            }
            PitchOp::Split
        }
        5 => {
            edit::join(&mut e, &ids[..ids.len().min(2)]);
            PitchOp::Join
        }
        6 => {
            edit::correct_pitch(&mut e, &ids, rng.f32(0.0, 1.0));
            PitchOp::CorrectPitch
        }
        7 => {
            edit::quantise(&mut e, &ids, |f| f / 6_000 * 6_000);
            PitchOp::Quantise
        }
        8 => {
            e.key.tonic = rng.below(12) as u8;
            PitchOp::Key
        }
        9 => {
            e.bypass = !e.bypass;
            PitchOp::Compare
        }
        10 => {
            return Command::SetClipPitch {
                track,
                clip: clip.id,
                pitch: None,
                op: PitchOp::Reset,
            };
        }
        _ => {
            // Refused: a valid edit sent to a clip that is not audio (the guard that
            // refuses it is the one under test), or an invalid edit on the audio clip.
            let other = project
                .tracks
                .iter()
                .flat_map(|t| t.clips.iter().map(move |c| (t.id, c)))
                .find(|(_, c)| c.audio.is_none())
                .map(|(t, c)| (t, c.id));
            let (target, edit) = match other.filter(|_| rng.below(2) == 0) {
                Some(t) => (t, e),
                None => {
                    if let Some(n) = e.notes.get_mut(0) {
                        n.end = e.frames + 1;
                    } else {
                        e.rate = 0;
                    }
                    ((track, clip.id), e)
                }
            };
            return Command::SetClipPitch {
                track: target.0,
                clip: target.1,
                pitch: Some(Box::new(edit)),
                op: PitchOp::Notes,
            };
        }
    };
    Command::SetClipPitch {
        track,
        clip: clip.id,
        pitch: Some(Box::new(e)),
        op,
    }
}

/// A target a modulator can drive in `project` (or, one time in ten, a stale one).
fn mod_target(rng: &mut Lcg, project: &Project) -> AutoTarget {
    let strip = || project.strips.first().map(|s| s.id);
    match rng.below(10) {
        0 => AutoTarget::MasterGain,
        1 => AutoTarget::MasterWidth,
        2 if !project.tracks.is_empty() => {
            AutoTarget::TrackPan(project.tracks[rng.below(project.tracks.len() as u64) as usize].id)
        }
        3 if !project.strips.is_empty() => AutoTarget::StripGain(strip_at(rng, project)),
        4 if !project.strips.is_empty() => AutoTarget::StripPan(strip_at(rng, project)),
        5 => match project.all_modulators().nth(rng.below(4) as usize) {
            Some((_, m)) => AutoTarget::ModAmount(m.id),
            None => AutoTarget::MasterPan,
        },
        6 => AutoTarget::Plugin {
            effect: EffectId(rng.below(50)),
            id: rng.below(8) as u32,
        },
        7 => match strip() {
            Some(s) => AutoTarget::Send {
                strip: s,
                to: strip_at(rng, project),
            },
            None => AutoTarget::MasterPan,
        },
        9 => AutoTarget::TrackGain(TrackId(9_999_999)),
        _ => AutoTarget::MasterPan,
    }
}

/// One modulator command, as the Modulators panel, *Modulate…* and a depth drag issue
/// them: a new modulator on a track, a strip or the master with routes; one removed; one
/// edited (a source setting, a route added or removed, a depth, muted, renamed). One time
/// in eight the edit is not valid, or names a modulator that is not there (refused).
fn modulator_command(rng: &mut Lcg, project: &mut Project) -> Command {
    use cantus_core::modulator::*;
    let existing: Vec<Modulator> = project.all_modulators().map(|(_, m)| m.clone()).collect();
    let pick = rng.below(4);
    if pick == 0 || existing.is_empty() {
        let host = match rng.below(3) {
            0 if !project.tracks.is_empty() => {
                ModHost::Track(project.tracks[rng.below(project.tracks.len() as u64) as usize].id)
            }
            1 if !project.strips.is_empty() => ModHost::Strip(strip_at(rng, project)),
            _ => ModHost::Master,
        };
        let kind = ModKind::ALL[rng.below(5) as usize];
        let mut m = Modulator::new(ModulatorId(project.alloc_id()), kind);
        for _ in 0..rng.below(4) {
            let target = mod_target(rng, project);
            if m.route(&target).is_none() && target != AutoTarget::ModAmount(m.id) {
                m.routes.push(ModRoute {
                    target,
                    depth: rng.f32(-1.0, 1.0),
                });
            }
        }
        let len = project.modulators(host).map_or(0, |l| l.len());
        return Command::InsertModulator {
            host,
            index: rng.below(len as u64 + 1) as usize,
            modulator: Box::new(m),
        };
    }
    let m = &existing[rng.below(existing.len() as u64) as usize];
    if pick == 1 {
        return Command::RemoveModulator {
            id: if rng.below(8) == 0 {
                ModulatorId(8_888_888)
            } else {
                m.id
            },
        };
    }
    let mut e = m.clone();
    match rng.below(7) {
        0 => {
            let target = mod_target(rng, project);
            if e.route(&target).is_none() && target != AutoTarget::ModAmount(e.id) {
                e.routes.push(ModRoute {
                    target,
                    depth: rng.f32(-1.0, 1.0),
                });
            }
        }
        1 if !e.routes.is_empty() => {
            let i = rng.below(e.routes.len() as u64) as usize;
            e.routes.remove(i);
        }
        2 if !e.routes.is_empty() => {
            let i = rng.below(e.routes.len() as u64) as usize;
            e.routes[i].depth = rng.f32(-1.0, 1.0);
        }
        3 => e.muted = !e.muted,
        4 => e.amount = rng.f32(0.0, 1.0),
        5 => e.name = format!("M{}", rng.below(100)),
        _ => {
            e.source = ModKind::ALL[rng.below(5) as usize].source();
            match &mut e.source {
                ModSource::Lfo(l) => {
                    l.rate = if rng.below(2) == 0 {
                        ModRate::Hz {
                            hz: rng.f32(0.05, 20.0),
                        }
                    } else {
                        ModRate::Sync {
                            division: rng.below(17) as u8,
                        }
                    };
                    l.retrigger = if rng.below(2) == 0 {
                        Retrigger::Note
                    } else {
                        Retrigger::Song
                    };
                }
                ModSource::Step(s) => {
                    s.steps = (0..1 + rng.below(32)).map(|_| rng.f32(-1.0, 1.0)).collect();
                    s.glide = rng.f32(0.0, 1.0);
                }
                ModSource::Envelope(env) => env.sustain = rng.f32(0.0, 1.0),
                ModSource::Follower(f) => f.release_ms = rng.f32(1.0, 2000.0),
                ModSource::Random(r) => r.seed = rng.below(1000) as u32,
            }
        }
    }
    if rng.below(8) == 0 {
        e.amount = 3.0; // not valid: refused
    }
    Command::SetModulator {
        modulator: Box::new(e),
    }
}

/// One comping command: a real seam made and moved — two playing-lane audio windows made
/// to meet (bounds set, or two new windows on a source) and their seam moved, one batch —
/// or, one time in four, a seam asked of two clips that do not meet (refused); a window's
/// seam crossfade set (a ninth of them beyond the most: refused); or the track's comps
/// replaced — a copy of a lane-0 clip kept under a fresh id in a second comp, or an empty
/// list.
fn comp_command(rng: &mut Lcg, project: &mut Project) -> Command {
    use cantus_core::comp::{letter, seam_window, Comp};
    let ti = rng.below(project.tracks.len() as u64) as usize;
    let track = project.tracks[ti].clone();
    let lane0: Vec<&Clip> = track.clips.iter().filter(|c| c.lane == 0).collect();
    match rng.below(5) {
        0 if rng.below(4) == 0 => {
            // Two clips that do not meet (or one with itself): refused.
            let c = lane0.first().map_or(ClipId(8_888_887), |c| c.id);
            Command::SetCompSeam {
                track: track.id,
                left: c,
                right: c,
                at: Ticks::beats(1),
            }
        }
        0 => {
            let windows: Vec<&Clip> = lane0.iter().copied().filter(|c| seam_window(c)).collect();
            let pair = windows
                .windows(2)
                .find(|w| w[0].start < w[1].start)
                .map(|w| (w[0].clone(), w[1].clone()));
            let mut commands = Vec::new();
            let (l, r) = match pair {
                Some((mut l, r)) => {
                    commands.push(Command::SetClipBounds {
                        track: track.id,
                        clip: l.id,
                        start: l.start,
                        length: r.start - l.start,
                    });
                    l.length = r.start - l.start;
                    (l, r)
                }
                None => {
                    let src = match project.audio_pool.first() {
                        Some(s) => s.id,
                        None => {
                            let id = AudioSourceId(project.alloc_id());
                            commands.push(Command::InsertAudioSource {
                                source: AudioSource {
                                    id,
                                    name: "take".into(),
                                    path: "take.wav".into(),
                                    sample_rate: 48_000,
                                    channels: 1,
                                    frames: 48_000 * 60,
                                    sha256: None,
                                    recorded: true,
                                },
                            });
                            id
                        }
                    };
                    let at = Ticks::beats(rng.below(16) as i64);
                    let l = project.make_audio_clip("a", src, at, Ticks::beats(2));
                    let mut r =
                        project.make_audio_clip("b", src, at + Ticks::beats(2), Ticks::beats(2));
                    r.offset = Ticks::beats(2);
                    for c in [&l, &r] {
                        commands.push(Command::InsertClip {
                            track: track.id,
                            clip: Box::new(c.clone()),
                        });
                    }
                    (l, r)
                }
            };
            let lo = l.start.0 + 1;
            let hi = r.end().0.max(lo + 1);
            commands.push(Command::SetCompSeam {
                track: track.id,
                left: l.id,
                right: r.id,
                at: Ticks(lo + rng.below((hi - lo) as u64) as i64),
            });
            Command::Batch {
                label_key: "cmd.set_comp_seam".into(),
                commands,
            }
        }
        3 => match lane0.get(rng.below(lane0.len() as u64 + 1) as usize) {
            // A clip made a comp window, or no longer one.
            Some(c) => Command::SetClipComp {
                track: track.id,
                clip: c.id,
                comp: rng.below(3) != 0,
            },
            None => Command::SetClipComp {
                track: track.id,
                clip: ClipId(8_888_886),
                comp: true,
            },
        },
        4 => {
            // A comp window split (its right half the comp's), duplicated and pasted —
            // linked and not, over itself (never a window).
            let Some(c) = lane0
                .iter()
                .find(|c| c.length > Ticks(1) && c.automation.is_none())
            else {
                return Command::SetTrackComps {
                    track: track.id,
                    comps: Vec::new(),
                    active: 0,
                };
            };
            let mut w = (*c).clone();
            let mut commands = Vec::new();
            if !w.comp {
                w.comp = true;
                commands.push(Command::SetClipComp {
                    track: track.id,
                    clip: w.id,
                    comp: true,
                });
            }
            let mut ids = || project.alloc_id();
            let at = w.start + Ticks(1 + rng.below((w.length.0 - 1) as u64) as i64);
            // Strictly inside the window: the split is always made (its half is the batch's
            // first clip inserted).
            commands.extend(clipboard::split(track.id, &w, at, &mut ids).unwrap());
            let copy = clipboard::copy_of(&w, w.start, &mut ids);
            commands.push(Command::InsertClip {
                track: track.id,
                clip: Box::new(copy),
            });
            let (pre, linked) = links::Linker::new().copy(track.id, &w, w.end(), &mut ids);
            commands.extend(pre);
            commands.push(Command::InsertClip {
                track: track.id,
                clip: Box::new(linked),
            });
            Command::Batch {
                label_key: "cmd.comp_window_copies".into(),
                commands,
            }
        }
        1 => match lane0.iter().find(|c| c.audio.is_some()) {
            Some(c) => {
                let mut a = c.audio.clone().unwrap();
                a.xfade = if rng.below(9) == 0 {
                    cantus_core::audio::MAX_XFADE + Ticks(1)
                } else {
                    Ticks(rng.below(cantus_core::audio::MAX_XFADE.0 as u64 + 1) as i64)
                };
                Command::SetClipAudio {
                    track: track.id,
                    clip: c.id,
                    audio: Some(a),
                }
            }
            None => Command::SetTrackComps {
                track: track.id,
                comps: Vec::new(),
                active: 0,
            },
        },
        _ => {
            let mut comps = vec![Comp {
                name: letter(0),
                windows: Vec::new(),
            }];
            if let Some(c) = lane0.first() {
                let mut w = (*c).clone();
                w.id = ClipId(project.alloc_id());
                // An alternate comp keeps comp windows only (schema 32).
                w.comp = true;
                comps.push(Comp {
                    name: letter(1),
                    windows: vec![w],
                });
            }
            Command::SetTrackComps {
                track: track.id,
                active: if rng.below(8) == 0 { 7 } else { 0 },
                comps,
            }
        }
    }
}

// --- Round 29: one section per builder (the contract's placeholders) --------------------

/// Round 29, tempo (ADR 0060): tempo points, ramps, metre changes and markers — and the
/// song's metre (no arm drew `SetTimeSignature` before).
fn round29_tempo_command(rng: &mut Lcg, project: &mut Project) -> Command {
    // One draw from the shared generator, as the contract's placeholder took, so the other
    // arms' draws stay where the coverage checks count on them; the rest from our own.
    let rng = &mut Lcg(rng.below(1 << 30) ^ 0x7e3_9a11);
    // The model's invariant: a tempo track starts at tick 0 at the song's tempo, and the
    // metre list at the song's metre.
    if let Some(p) = project.tempo_points.first() {
        assert_eq!((p.tick, p.bpm), (Ticks::ZERO, project.tempo_bpm));
    }
    if let Some(m) = project.metre_changes.first() {
        assert_eq!((m.tick, m.signature), (Ticks::ZERO, project.time_signature));
    }
    let point = |rng: &mut Lcg, tick: i64| TempoPoint {
        tick: Ticks(tick),
        bpm: f64::from(rng.f32(40.0, 240.0)),
        ramp: rng.below(2) == 0,
    };
    let sigs = [(4, 4), (3, 4), (7, 8), (5, 4), (6, 8), (2, 2)];
    let sig = |rng: &mut Lcg| {
        let (n, d) = sigs[rng.below(sigs.len() as u64) as usize];
        TimeSignature::new(n, d)
    };
    let pts = project.tempo_points.clone();
    let len = pts.len() as u64;
    let map = project.tempo_map();
    let changes = project.metre_changes.clone();
    let markers = project.markers.clone();
    // Draw among the kinds the project can take now, so each is applied often enough.
    let can = |k: u64| match k {
        1 | 3 => !pts.is_empty(),
        2 => pts.len() > 1,
        5 | 6 => changes.len() > 1,
        9 | 10 => !markers.is_empty(),
        _ => true,
    };
    let open: Vec<u64> = (0..12).filter(|k| can(*k)).collect();
    match open[rng.below(open.len() as u64) as usize] {
        0 if pts.is_empty() || rng.below(4) != 0 => {
            let bpm = project.tempo_bpm;
            let mut points = vec![TempoPoint {
                tick: Ticks::ZERO,
                bpm,
                ramp: rng.below(2) == 0,
            }];
            let mut tick = 0;
            for _ in 0..1 + rng.below(3) {
                tick += 960 + rng.below(30_000) as i64;
                points.push(point(rng, tick));
            }
            Command::SetTempoTrack { bpm, points }
        }
        0 => Command::SetTempoTrack {
            bpm: project.tempo_bpm,
            points: Vec::new(),
        },
        1 => {
            let index = 1 + rng.below(len.max(1)) as usize;
            let prev = pts.get(index - 1).map_or(0, |p| p.tick.0);
            let next = pts.get(index).map_or(prev + 20_000, |p| p.tick.0);
            let tick = prev + 1 + rng.below((next - prev - 1).max(1) as u64) as i64;
            Command::InsertTempoPoint {
                index,
                point: point(rng, tick),
            }
        }
        2 => Command::RemoveTempoPoint {
            index: 1 + rng.below(len.max(2) - 1) as usize,
        },
        3 => {
            let index = rng.below(len.max(1)) as usize;
            let tick = if index == 0 {
                0
            } else {
                let prev = pts[index - 1].tick.0;
                let next = pts.get(index + 1).map_or(prev + 20_000, |p| p.tick.0);
                prev + 1 + rng.below((next - prev - 1).max(1) as u64) as i64
            };
            Command::SetTempoPoint {
                index,
                point: point(rng, tick),
            }
        }
        4 => Command::SetMetreAt {
            tick: map.bar_tick(1 + rng.below(8) as i64),
            signature: sig(rng),
        },
        5 => Command::RemoveMetreAt {
            tick: changes
                .get(1 + rng.below(changes.len().max(2) as u64 - 1) as usize)
                .map_or(Ticks::ZERO, |m| m.tick),
        },
        6 => {
            let i = 1 + rng.below(changes.len().max(2) as u64 - 1) as usize;
            let from = changes.get(i).map_or(Ticks::ZERO, |m| m.tick);
            let mut without = changes.clone();
            if i < without.len() {
                without.remove(i);
            }
            let other = TempoMap::new(120.0, project.time_signature, &[], &without);
            let (bar, _, _) = other.bbt(from);
            Command::MoveMetre {
                from,
                to: other.bar_tick(bar - 1 + rng.below(3) as i64),
            }
        }
        7 => Command::SetMetreTrack {
            signature: project.time_signature,
            metres: if rng.below(2) == 0 {
                Vec::new()
            } else {
                vec![MetreChange {
                    tick: Ticks::ZERO,
                    signature: project.time_signature,
                }]
            },
        },
        8 => Command::AddMarker {
            marker: Marker {
                id: MarkerId(project.alloc_id()),
                tick: Ticks(rng.below(40_000) as i64),
                name: format!("M{}", rng.below(100)),
                color: (rng.below(2) == 0).then(|| Color::PALETTE[rng.below(8) as usize]),
            },
        },
        9 => Command::RemoveMarker {
            id: markers
                .get(rng.below(markers.len().max(1) as u64) as usize)
                .map_or(MarkerId(u64::MAX), |m| m.id),
        },
        10 => {
            let Some(m) = markers.get(rng.below(markers.len().max(1) as u64) as usize) else {
                return Command::RemoveMarker {
                    id: MarkerId(u64::MAX),
                };
            };
            Command::SetMarker {
                marker: Marker {
                    tick: Ticks(rng.below(40_000) as i64),
                    name: format!("N{}", rng.below(100)),
                    ..m.clone()
                },
            }
        }
        _ => Command::SetTimeSignature {
            time_signature: sig(rng),
        },
    }
}

/// Which tempo command a generated command is, for the coverage check (ADR 0060).
fn tempo_kind(cmd: &Command) -> Option<usize> {
    Some(match cmd {
        Command::SetTempoTrack { .. } => 0,
        Command::InsertTempoPoint { .. } => 1,
        Command::RemoveTempoPoint { .. } => 2,
        Command::SetTempoPoint { .. } => 3,
        Command::SetMetreAt { .. } => 4,
        Command::RemoveMetreAt { .. } => 5,
        Command::MoveMetre { .. } => 6,
        Command::SetMetreTrack { .. } => 7,
        Command::AddMarker { .. } => 8,
        Command::RemoveMarker { .. } => 9,
        Command::SetMarker { .. } => 10,
        Command::SetTimeSignature { .. } => 11,
        _ => return None,
    })
}

/// Round 29, warp (ADR 0061): warp markers, modes and transposition on audio clips —
/// `SetClipWarp` with every edit, cleared, and one time in nine refused (an invalid warp,
/// or a valid one sent to a clip that is not audio).
fn round29_warp_command(rng: &mut Lcg, project: &mut Project) -> Command {
    use cantus_core::warp::{Detected, Warp, WarpMode, WarpOp, WarpTail};
    let audio: Vec<(TrackId, Clip)> = project
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(move |c| (t.id, c.clone())))
        .filter(|(_, c)| c.audio.is_some())
        .collect();
    let Some((track, clip)) =
        (!audio.is_empty()).then(|| audio[rng.below(audio.len() as u64) as usize].clone())
    else {
        // No audio clip yet: a loop and a clip over it, in one step.
        let tid = project.tracks[rng.below(project.tracks.len() as u64) as usize].id;
        let s = AudioSource {
            id: AudioSourceId(project.alloc_id()),
            name: "loop".into(),
            path: "C:/loops/drums 92.wav".into(),
            sample_rate: 44_100,
            channels: 2,
            frames: 44_100 * 6,
            sha256: None,
            recorded: false,
        };
        let c = project.make_audio_clip("loop", s.id, Ticks::ZERO, Ticks::beats(8));
        return Command::Batch {
            label_key: "fuzz.loop".into(),
            commands: vec![
                Command::InsertAudioSource { source: s },
                Command::InsertClip {
                    track: tid,
                    clip: Box::new(c),
                },
            ],
        };
    };
    let rate = clip
        .audio
        .as_ref()
        .and_then(|a| project.audio_source(a.source))
        .map_or(48_000, |s| s.sample_rate.max(1));
    let slip = clip.audio.as_ref().map_or(0, |a| a.slip_frames);
    let w = clip
        .audio
        .as_ref()
        .and_then(|a| a.warp.clone())
        .map(|b| *b)
        .unwrap_or_else(|| Warp::identity(rate, project.tempo_bpm, slip));
    let n = w.markers.len().max(1) as u64;
    let i = rng.below(n) as usize;
    let (next, op) = match rng.below(14) {
        0 => (
            w.add_marker(Ticks(rng.below(16 * 960) as i64)).unwrap_or(w),
            WarpOp::AddMarker,
        ),
        1 => (
            w.move_marker(i, Ticks(rng.below(16 * 960) as i64 - 960)),
            WarpOp::MoveMarker,
        ),
        2 => {
            let f = w.markers.get(i).map_or(0.0, |m| m.frame) + rng.f32(-900.0, 900.0) as f64;
            (w.move_content(i, f), WarpOp::MoveContent)
        }
        3 => (w.remove_marker(i), WarpOp::RemoveMarker),
        4 => {
            let hits: Vec<f64> = (0..8)
                .map(|k| k as f64 * 15_000.0 + rng.below(900) as f64)
                .collect();
            (
                w.quantise(
                    &hits,
                    Ticks(480),
                    rng.f32(0.0, 1.0),
                    Ticks::ZERO,
                    Ticks::beats(16),
                ),
                WarpOp::Quantise,
            )
        }
        5 => {
            let mut v = w;
            v.mode = WarpMode::ALL[rng.below(4) as usize];
            (v, WarpOp::Mode)
        }
        6 => {
            let mut v = w;
            v.cents = rng.below(4_801) as i32 - 2_400;
            (v, WarpOp::Transpose)
        }
        7 => {
            let mut v = w;
            v.formants = !v.formants;
            v.tail = WarpTail::ALL[rng.below(3) as usize];
            (
                v,
                if rng.below(2) == 0 {
                    WarpOp::Formants
                } else {
                    WarpOp::Tail
                },
            )
        }
        8 => {
            let mut v = w;
            v.follow = !v.follow;
            (v, WarpOp::Follow)
        }
        9 => (w.with_tempo(60.0 + rng.below(120) as f64), WarpOp::Tempo),
        10 => {
            let per = rate as f64 * 60.0 / 92.0;
            let beats: Vec<f64> = (0..16).map(|k| 1_000.0 + k as f64 * per).collect();
            let d = Detected {
                bpm: 92.0,
                confidence: rng.f32(0.0, 1.0),
                downbeat: 1_000.0,
                bars: Some(4),
            };
            (Warp::from_grid(rate, 92.0, &beats, 0, 4, d), WarpOp::Set)
        }
        11 => {
            return Command::SetClipWarp {
                track,
                clip: clip.id,
                warp: None,
                op: WarpOp::Reset,
            };
        }
        12 => (w.reset(), WarpOp::Reset),
        _ => {
            // Refused: a valid warp on a clip that is not audio, or an invalid warp.
            let other = project
                .tracks
                .iter()
                .flat_map(|t| t.clips.iter().map(move |c| (t.id, c)))
                .find(|(_, c)| c.audio.is_none())
                .map(|(t, c)| (t, c.id));
            let (target, warp) = match other.filter(|_| rng.below(2) == 0) {
                Some(t) => (t, w),
                None => {
                    let mut v = w;
                    v.bpm = 0.0;
                    ((track, clip.id), v)
                }
            };
            return Command::SetClipWarp {
                track: target.0,
                clip: target.1,
                warp: Some(Box::new(warp)),
                op: WarpOp::Set,
            };
        }
    };
    Command::SetClipWarp {
        track,
        clip: clip.id,
        warp: Some(Box::new(next)),
        op,
    }
}

/// Round 29, controllers (ADR 0062): controller mappings stored in the project.
fn round29_controller_command(rng: &mut Lcg, _project: &mut Project) -> Command {
    Command::SetProjectName {
        name: format!("P{}", rng.below(100)),
    }
}

/// Round 29, isolation (ADR 0063): a hosted plug-in's isolation choice.
fn round29_hosting_command(rng: &mut Lcg, _project: &mut Project) -> Command {
    Command::SetProjectName {
        name: format!("P{}", rng.below(100)),
    }
}

/// Round 29, freeze (ADR 0064): freezing, unfreezing and bouncing in place.
fn round29_freeze_command(rng: &mut Lcg, _project: &mut Project) -> Command {
    Command::SetProjectName {
        name: format!("P{}", rng.below(100)),
    }
}
