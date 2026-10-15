//! Writes a fixture for the *current* schema version so future migrations are tested
//! against a real file: `cargo run -p cantus-project --example write_fixture`.
use cantus_core::*;
use std::path::Path;

fn main() {
    let mut p = Project::new("Fixture");
    p.tempo_bpm = 96.0;
    p.loop_enabled = true;
    let mut t = p.make_track("Keys");
    // Schema 9: the mixer has strips of its own. The Keys track feeds strip 1, which holds
    // the fader, the pan and the delay; the track's own gain and pan stay at unity.
    let strip1 = t.strip.expect("a new track takes the first free strip");
    let f = t.instrument.firmus_mut().expect("Firmus");
    f.cutoff_hz = 900.0;
    f.osc1_wave = Waveform::Square;
    let mut delay = p.make_effect(EffectKind::Delay);
    delay.spec.set("mix", 0.2).unwrap();
    {
        let s = p.strip_mut(strip1).unwrap();
        s.gain_db = -4.5;
        s.pan = 0.25;
        s.effects.push(delay);
        s.name = "Keys bus".to_string();
    }
    let mut c = p.make_clip("Intro", Ticks::beats(4), Ticks::beats(8));
    for (i, pitch) in [60u8, 64, 67, 71].iter().enumerate() {
        c.notes.push(p.make_note(
            Ticks::beats(i as i64 * 2),
            Ticks::QUARTER,
            *pitch,
            80 + i as u8,
        ));
    }
    t.clips.push(c);
    // A second clip that exercises everything schema 4 added, so the 3→4 migration and
    // every future one are tested against a file that actually uses the fields.
    let mut looped = p.make_clip("Loop", Ticks::beats(16), Ticks::beats(16));
    looped.content_length = Some(Ticks::beats(4));
    looped.offset = Ticks::beats(2);
    looped.muted = true;
    looped.color = Some(Color::PALETTE[2]);
    looped
        .notes
        .push(p.make_note(Ticks::beats(2), Ticks::QUARTER, 55, 90));
    t.clips.push(looped);
    p.tracks.push(t);
    // Schema 6: a Pulsus track, with a pad renamed, a pad muted and a pad panned, so the
    // 5→6 migration and every future one are tested against a file that actually uses the
    // kit rather than one that only mentions it.
    let mut drums = p.make_track("Drums");
    let mut kit = PulsusParams::default();
    kit.pads[0].name = "Bombo".to_string();
    kit.pads[0].level_db = -1.5;
    kit.pads[2].pan = -0.3;
    kit.pads[3].muted = true;
    // Schema 8: a sampled pad. The path is deliberately one that does not exist, so the
    // fixture also proves that a missing sample never stops a project from opening.
    kit.pads[1].voice = PadVoice::Sample;
    kit.pads[1].sample = Some("samples/rim.wav".to_string());
    // Schema 17: Pulsus II — the kit is model 2 (what a new kit is), with a seed, and its
    // kick holds two sample layers in one velocity zone chosen in turn. The files do not
    // exist, on purpose: a missing layer is silence, never a project that will not open.
    kit.kit.seed = 17;
    kit.pads[0].round_robin = RoundRobin::Cycle;
    kit.pads[0].layers = ["samples/kick-a.wav", "samples/kick-b.wav"]
        .iter()
        .map(|s| PadLayer {
            sample: s.to_string(),
            vel_lo: 1,
            vel_hi: 127,
            xfade: 0,
            tune: 0.0,
            level_db: 0.0,
            pan: 0.0,
            start_ms: 0.0,
            reverse: false,
        })
        .collect();
    drums.instrument = InstrumentSpec::Pulsus(kit);
    // Schema 7: a track that is taller than the rest and carries an icon it did not have
    // to derive, so both fields are exercised by a real file.
    drums.height = 1.75;
    drums.icon = Some(TrackIcon::Drums);
    let mut beat = p.make_clip("Beat", Ticks::ZERO, Ticks::beats(4));
    beat.content_length = Some(Ticks::beats(2));
    for (i, pitch) in [(0i64, 36u8), (2, 42), (4, 38), (6, 42)] {
        beat.notes
            .push(p.make_note(Ticks::EIGHTH * i, Ticks::SIXTEENTH, pitch, 100));
    }
    drums.clips.push(beat);
    p.tracks.push(drums);

    let mut bass = p.make_track("Bass");
    bass.muted = true;
    bass.height = 0.5;
    bass.icon = Some(TrackIcon::Strings);
    // Schema 5's width and polarity now live on the bass's strip; and the bass's strip is
    // routed into the drums' strip, so the file holds a bus.
    let bass_strip = bass.strip.expect("strip");
    let drums_strip = p.tracks[1].strip.expect("drums strip");
    {
        let s = p.strip_mut(bass_strip).unwrap();
        s.width = 0.0;
        s.phase_invert = true;
        s.output = Some(drums_strip);
    }
    p.master.pan = -0.25;
    p.master.width = 1.5;
    p.tracks.push(bass);
    // Schema 10: a send from the Keys strip into an empty strip that carries a reverb —
    // a return — with a slot at half dry/wet; a soloed, swapped strip with its FX off
    // and a band of EQ on it. Every new field written, so the 9→10 migration and every
    // later one are tested against a file that uses them.
    let ret = p.strips[5].id;
    let mut verb = p.make_effect(EffectKind::Reverb);
    verb.mix = 0.5;
    {
        let r = p.strip_mut(ret).unwrap();
        r.name = "Return".to_string();
        r.effects.push(verb);
        r.eq.set(0, EqField::Gain, -3.0);
        r.eq.set(2, EqField::Freq, 6_000.0);
    }
    p.strip_mut(strip1).unwrap().sends.push(Send {
        to: ret,
        level: 0.4,
        enabled: true,
        tap: cantus_core::SendTap::Post,
    });
    p.strip_mut(strip1).unwrap().sends.push(Send {
        to: drums_strip,
        level: 1.0,
        enabled: false,
        tap: cantus_core::SendTap::Post,
    });
    {
        let d = p.strip_mut(drums_strip).unwrap();
        d.soloed = true;
        d.swap_channels = true;
        d.fx_enabled = false;
    }
    // Schema 11: the return docked on the left, and the Keys and Drums strips in a
    // collapsed group of their own colour.
    p.strip_mut(ret).unwrap().dock = Dock::Left;
    let mut rhythm = p.make_strip_group("Rhythm", Color::PALETTE[3]);
    rhythm.collapsed = true;
    let gid = rhythm.id;
    p.strip_groups.push(rhythm);
    p.strip_mut(strip1).unwrap().group = Some(gid);
    p.strip_mut(drums_strip).unwrap().group = Some(gid);
    // Schema 12: an Aequalis with a bell, a low cut and a dynamic band on the Keys strip,
    // and a Comprimo on it keyed from the Drums strip — a duck.
    let mut eq = p.make_effect(EffectKind::Eq);
    for (k, v) in [
        ("b0.used", 1.0),
        ("b0.freq", 2_500.0),
        ("b0.gain", 3.0),
        ("b1.used", 1.0),
        ("b1.shape", 3.0),
        ("b1.freq", 90.0),
        ("b1.slope", 24.0),
        ("b4.used", 1.0),
        ("b4.freq", 6_000.0),
        ("b4.gain", 0.5),
        ("b4.dyn", 1.0),
        ("b4.place", 3.0),
    ] {
        eq.spec.set(k, v).unwrap();
    }
    let mut comp = p.make_effect(EffectKind::Compressor);
    comp.spec.set("thr", -30.0).unwrap();
    comp.spec.set("style", 2.0).unwrap();
    comp.sidechain = Some(drums_strip);
    p.strip_mut(strip1).unwrap().effects.push(eq);
    p.strip_mut(strip1).unwrap().effects.push(comp);
    // Schema 14: a hosted plug-in (ADR 0019) on the return, keyed from the Drums strip,
    // with a state and three parameter values. It is not installed anywhere — which is the
    // point: the fixture proves a project with a missing plug-in opens, and keeps its data.
    let mut hosted = ExternalPlugin::new(PluginFormat::Clap, "org.cantus.fixture.absent", "Absent");
    hosted.vendor = "Cantus Fixtures".to_string();
    hosted.sidechain = true;
    hosted.state = PluginState::new(b"CTG1 opaque state \x00\x01\xfe\xff".to_vec());
    hosted.params.insert(0, -6.25);
    hosted.params.insert(1, 128.0);
    hosted.params.insert(1024, 0.1 + 0.2);
    let mut slot = p.make_effect(EffectKind::External);
    slot.spec = EffectSpec::external(hosted);
    slot.sidechain = Some(drums_strip);
    slot.mix = 0.75;
    p.strip_mut(ret).unwrap().effects.push(slot);
    // Schema 15: a Porta on the Drums strip, opened by the Keys track's notes (ADR 0022).
    let mut gate = p.make_effect(EffectKind::Gate);
    gate.spec.set("trig", 1.0).unwrap();
    gate.spec.set("vel", 12.0).unwrap();
    gate.midi_from = Some(p.tracks[0].id);
    p.strip_mut(drums_strip).unwrap().effects.push(gate);
    // Schema 16: a native instrument (ADR 0023), stored sparsely — a number moved from its
    // default, a macro with a name, and a modulation connection from it.
    let mut organ = p.make_track("Organ");
    let mut n = NativeSpec::new(InstrumentKind::Organ);
    n.set("gain", -3.0).unwrap();
    n.set("macro1", 0.5).unwrap();
    n.set_text("macro1.name", Some("Drive".into())).unwrap();
    n.set_text("mod1.src", Some("macro1".into())).unwrap();
    n.set_text("mod1.dst", Some("gain".into())).unwrap();
    n.set("mod1.amt", 0.25).unwrap();
    organ.instrument = InstrumentSpec::Native(n);
    p.tracks.push(organ);
    // Schema 18 (ADR 0025): an audio track with a stereo input, armed, a clip with gain and
    // fades on the playing lane and an older take on lane 1, recording settings; and
    // (ADR 0026) a limiter on the master. The file is not shipped — a missing source must
    // open, and the clip plays silence.
    let source = AudioSource {
        id: AudioSourceId(p.alloc_id()),
        name: "Vocal take 1".into(),
        path: "Audio/Vocal take 1.wav".into(),
        sample_rate: 48_000,
        channels: 2,
        frames: 48_000 * 8,
        sha256: None,
        recorded: true,
    };
    let mut vocal = p.make_track("Vocal");
    vocal.input = Some(TrackInput::stereo(0));
    vocal.armed = true;
    vocal.takes_open = true;
    let mut take = p.make_audio_clip("Vocal take 1", source.id, Ticks::beats(4), Ticks::beats(8));
    if let Some(a) = take.audio.as_mut() {
        a.gain_db = -2.5;
        a.fade_in = Ticks::SIXTEENTH;
        a.fade_out = Ticks::QUARTER;
        a.fade_out_curve = FadeCurve::EqualPower;
    }
    let mut older = p.make_audio_clip("Vocal take 0", source.id, Ticks::beats(4), Ticks::beats(8));
    older.lane = 1;
    older.offset = Ticks::beats(8);
    vocal.clips.push(take);
    vocal.clips.push(older);
    let vocal_index = p.tracks.len();
    p.tracks.push(vocal);
    p.audio_pool.push(source);
    p.recording.count_in_bars = 1;
    p.recording.punch = Some(LoopRegion {
        start: Ticks::beats(4),
        end: Ticks::beats(12),
    });
    p.recording.take_mode = TakeMode::Stack;
    p.recording.click = ClickMode::Recording;
    let mut master_lim = p.make_effect(EffectKind::Limiter);
    master_lim.spec.set("ceiling", -0.3).unwrap();
    p.master.effects.push(master_lim);
    // Schema 19 (ADR 0027): automation clips under the Keys track — the Keys strip's
    // fader ramping in with a bent curve, the delay's mix stepping, looped, and the
    // master's width on a sine — and the Drums track with its lanes hidden.
    let delay_id = p.strip(strip1).unwrap().effects[0].id;
    let mut fade = p.make_automation_clip(
        "Fade in",
        AutoTarget::StripGain(strip1),
        Ticks::ZERO,
        Ticks::beats(8),
        vec![
            AutoPoint {
                tick: Ticks::ZERO,
                value: 0.0,
                tension: 0.4,
                shape: Shape::Curve,
            },
            AutoPoint::new(Ticks::beats(8), 0.8),
        ],
    );
    fade.color = Some(Color::PALETTE[5]);
    let mut steps = p.make_automation_clip(
        "Delay mix",
        AutoTarget::Effect {
            effect: delay_id,
            key: "mix".into(),
        },
        Ticks::beats(8),
        Ticks::beats(16),
        vec![
            AutoPoint {
                tick: Ticks::ZERO,
                value: 0.1,
                tension: 0.0,
                shape: Shape::Hold,
            },
            AutoPoint::new(Ticks::beats(2), 0.6),
        ],
    );
    steps.content_length = Some(Ticks::beats(4));
    let wide = p.make_automation_clip(
        "Width",
        AutoTarget::MasterWidth,
        Ticks::beats(16),
        Ticks::beats(8),
        cantus_core::automation::edit::lfo(
            cantus_core::automation::edit::Lfo::Sine,
            Ticks::ZERO,
            Ticks::beats(8),
            2,
            0.3,
            0.7,
            0,
        ),
    );
    p.tracks[0].clips.extend([fade, steps, wide]);
    p.tracks[1].automation_open = false;
    // Schema 20 (ADR 0028): a hosted CLAP instrument installed nowhere — it opens, plays
    // silence, and its state and values (0.1 + 0.2 to the bit) survive a round trip.
    let mut synth = p.make_track("Synth");
    let mut plugin = ExternalPlugin::new(
        PluginFormat::Clap,
        "org.cantus.test.synth",
        "Cantus Test Synth",
    );
    plugin.vendor = "Cantus Tests".into();
    plugin.state = PluginState::new(b"CTS1 opaque instrument state".to_vec());
    plugin.params.insert(0, -4.5);
    plugin.params.insert(7, 0.1 + 0.2);
    synth.instrument = InstrumentSpec::External(plugin);
    let mut riff = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(4));
    riff.notes
        .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 48, 100));
    // Schema 22 (ADR 0037): the riff is a linked pattern — a copy a bar later on its own
    // track, and a third on the Organ track, trimmed and slipped (where each copy sits is
    // its own) — and the vocal take has a linked copy later in the song.
    let riff_link = LinkId(p.alloc_id());
    riff.link = Some(riff_link);
    let mut again = riff.clone();
    again.id = ClipId(p.alloc_id());
    again.start = Ticks::beats(4);
    let mut far = riff.clone();
    far.id = ClipId(p.alloc_id());
    far.start = Ticks::beats(8);
    far.length = Ticks::beats(2);
    far.offset = Ticks::beats(1);
    synth.clips.push(riff);
    synth.clips.push(again);
    p.tracks.push(synth);
    p.tracks
        .iter_mut()
        .find(|t| t.name == "Organ")
        .expect("the organ track")
        .clips
        .push(far);
    let take_link = LinkId(p.alloc_id());
    let vocal = p
        .tracks
        .iter_mut()
        .find(|t| t.name == "Vocal")
        .expect("the vocal track");
    let take = vocal
        .clips
        .iter_mut()
        .find(|c| c.lane == 0)
        .expect("the take");
    take.link = Some(take_link);
    let mut later = take.clone();
    later.start = Ticks::beats(16);
    vocal.clips.push(later);
    let id = ClipId(p.alloc_id());
    p.tracks
        .iter_mut()
        .find(|t| t.name == "Vocal")
        .unwrap()
        .clips
        .last_mut()
        .unwrap()
        .id = id;
    assert!(cantus_core::links::check(&p).is_ok());
    // Schema 21 (ADR 0039): a Lamina made now (its trim not stored: the calibration) and
    // an Exemplar saved before the calibration (its trim written down as 0 dB).
    let mut epiano = p.make_track("Lamina");
    epiano.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::ElectricPiano));
    p.tracks.push(epiano);
    let mut sampler = p.make_track("Exemplar");
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    s.set(cantus_core::inst::TRIM_KEY, 0.0).unwrap();
    sampler.instrument = InstrumentSpec::Native(s);
    p.tracks.push(sampler);
    // Schema 23 (ADR 0041): a pre-fader send at half level, from the Bass strip into the
    // return — a cue-style send that keeps its level when the bass is faded out.
    p.strip_mut(bass_strip).unwrap().sends.push(Send {
        level: 0.5,
        tap: SendTap::Pre,
        ..Send::new(ret)
    });
    // Schema 24 (round 26): an Exemplar whose second zone the sustain pedal starts — a
    // pedal-down noise (CC 64 crossing into 126…127), on the built-in noise.
    let mut pedal = p.make_track("Exemplar pedal");
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    for (k, v) in [
        ("z0.gen", 1.0),
        ("z1.gen", 5.0),
        ("z1.trig", 5.0),
        ("z1.cc", 64.0),
        ("z1.cclo", 126.0),
        ("z1.gain", -20.0),
    ] {
        s.set(k, v).unwrap();
    }
    pedal.instrument = InstrumentSpec::Native(s);
    p.tracks.push(pedal);
    // Schema 25 (round 28): one insert of each of the thirteen new kinds, on two strips of
    // their own (a strip holds ten), the transient shaper keyed from the drums and the
    // tuner hearing the Keys track's notes; and a Chorda track.
    let round_28 = [
        EffectKind::Chorus,
        EffectKind::Flanger,
        EffectKind::Phaser,
        EffectKind::Tremolo,
        EffectKind::Vibrato,
        EffectKind::RingMod,
        EffectKind::FreqShift,
        EffectKind::VolumeShaper,
        EffectKind::Clipper,
        EffectKind::Transient,
        EffectKind::TransientSplit,
        EffectKind::Analyzer,
        EffectKind::AutoTune,
    ];
    let keys = p.tracks[0].id;
    for (name, kinds) in [
        ("Modulation", &round_28[..8]),
        ("Dynamics and pitch", &round_28[8..]),
    ] {
        let t = p.make_track(name);
        let strip = t.strip.expect("a strip");
        p.tracks.push(t);
        for kind in kinds {
            let mut slot = p.make_effect(*kind);
            if *kind == EffectKind::Transient {
                slot.sidechain = Some(drums_strip);
            }
            if *kind == EffectKind::AutoTune {
                slot.midi_from = Some(keys);
            }
            if *kind == EffectKind::Chorus {
                slot.spec.set("out", -3.0).unwrap();
            }
            p.strip_mut(strip).unwrap().effects.push(slot);
        }
    }
    let mut piano = p.make_track("Chorda");
    piano.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Chorda));
    p.tracks.push(piano);
    // Schema 26 (round 28, ADR 0052): the woodblock click without its accent, and the song
    // saved in pattern mode.
    p.recording.click_sound = cantus_core::ClickSound::Wood;
    p.recording.click_accent = false;
    p.pattern_mode = true;
    // Schema 27 (round 28, ADR 0054): the vocal take's pitch edits — three notes in
    // A minor, the first moved onto A4 and pulled onto its centre, the second sung late
    // and stretched, the third's formants a little down.
    if let Some(a) = p.tracks[vocal_index]
        .clips
        .iter_mut()
        .find(|c| c.lane == 0)
        .and_then(|c| c.audio.as_mut())
    {
        use cantus_core::pitch::{edit, PitchEdit, PitchKey, PitchNote};
        let mut e = PitchEdit {
            rate: 48_000,
            frames: 48_000 * 8,
            analyser: 1,
            key: PitchKey { tonic: 9, scale: 2 },
            notes: vec![
                PitchNote::sung(0, 24_000, 60_000, 68.7, -14.0),
                PitchNote::sung(1, 72_000, 120_000, 72.1, -12.5),
                PitchNote::sung(2, 132_000, 180_000, 76.0, -16.0),
            ],
            bypass: false,
        };
        edit::set_pitch(&mut e, 0, 69.0);
        edit::set_field(&mut e, &[0], edit::Field::Centre, 1.0);
        edit::set_timing(&mut e, 1, 74_400, 124_800);
        edit::set_field(&mut e, &[2], edit::Field::Formant, -2.0);
        a.pitch = Some(Box::new(e));
    }
    // Schema 28 (round 28, ADR 0057): modulators. On the Keys track, an LFO (a dotted
    // eighth, triangle) on the delay's mix and, inverted, on the hosted return's parameter
    // 1024; on the Keys strip, an envelope from the Keys notes ducking its fader; on the
    // master, a follower listening to the Drums strip on the master's width; on the Synth
    // track, a step sequence on its hosted instrument's parameter 7; a random source
    // nested into the LFO's amount; and a curve on the hosted return's parameter 0.
    {
        use cantus_core::modulator::*;
        let hosted_fx = p
            .strip(ret)
            .unwrap()
            .effects
            .iter()
            .find(|e| e.spec.external.is_some())
            .unwrap()
            .id;
        let synth_id = p.tracks.iter().find(|t| t.name == "Synth").unwrap().id;
        let mut lfo = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
        lfo.name = "Wobble".into();
        if let ModSource::Lfo(l) = &mut lfo.source {
            l.shape = LfoShape::Triangle;
            l.rate = ModRate::Sync { division: 9 };
            l.phase = 0.25;
        }
        lfo.routes = vec![
            ModRoute {
                target: AutoTarget::Effect {
                    effect: delay_id,
                    key: "mix".into(),
                },
                depth: 0.3,
            },
            ModRoute {
                target: AutoTarget::Plugin {
                    effect: hosted_fx,
                    id: 1024,
                },
                depth: -0.4,
            },
        ];
        let lfo_id = lfo.id;
        p.tracks[0].modulators.push(lfo);
        let mut env = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Envelope);
        env.routes.push(ModRoute {
            target: AutoTarget::StripGain(strip1),
            depth: -0.25,
        });
        p.strip_mut(strip1).unwrap().modulators.push(env);
        let mut follow = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Follower);
        if let ModSource::Follower(f) = &mut follow.source {
            f.listen = Listen::Strip(drums_strip);
            f.gain_db = 6.0;
        }
        follow.routes.push(ModRoute {
            target: AutoTarget::MasterWidth,
            depth: -0.5,
        });
        p.master.modulators.push(follow);
        let mut steps = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Step);
        if let ModSource::Step(s) = &mut steps.source {
            s.glide = 0.25;
        }
        steps.routes.push(ModRoute {
            target: AutoTarget::PluginInstrument {
                track: synth_id,
                id: 7,
            },
            depth: 0.5,
        });
        let mut random = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Random);
        if let ModSource::Random(r) = &mut random.source {
            r.smooth = true;
            r.seed = 28;
        }
        random.amount = 0.8;
        random.routes.push(ModRoute {
            target: AutoTarget::ModAmount(lfo_id),
            depth: -0.5,
        });
        let synth = p.tracks.iter_mut().find(|t| t.name == "Synth").unwrap();
        synth.modulators.push(steps);
        synth.modulators.push(random);
        let curve = p.make_automation_clip(
            "Hosted 0",
            AutoTarget::Plugin {
                effect: hosted_fx,
                id: 1024,
            },
            Ticks::beats(4),
            Ticks::beats(4),
            vec![
                AutoPoint::new(Ticks::ZERO, 0.2),
                AutoPoint::new(Ticks::beats(4), 0.9),
            ],
        );
        p.tracks[0].clips.push(curve);
    }
    let name = format!("v{}.cantus", cantus_project::SCHEMA_VERSION);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    cantus_project::save(&p, &path, "fixture").expect("save fixture");
    println!("wrote {}", path.display());
}
