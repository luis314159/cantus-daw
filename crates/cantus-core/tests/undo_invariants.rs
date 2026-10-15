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
    match rng.below(40) {
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
                    settings.click_sound =
                        cantus_core::ClickSound::ALL[(d.abs() as usize) % 3];
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
            let kind = clipboard_kind(&cmd);
            let route = routing_kind(&cmd);
            let ok = bus.execute(cmd, origin).is_ok();
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
