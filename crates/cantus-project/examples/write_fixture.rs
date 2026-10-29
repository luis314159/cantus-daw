//! Writes a fixture for the *current* schema version so future migrations are tested
//! against a real file: `cargo run -p cantus-project --example write_fixture`. With a
//! number (`… --example write_fixture 35`), the fixture of that schema: the blocks up to it,
//! saved as that version — for a schema whose round ended at a later one (round 29 wrote
//! 35, 36 and 37 together).
use cantus_core::*;
use std::path::Path;

fn main() {
    let upto: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(cantus_project::SCHEMA_VERSION)
        .min(cantus_project::SCHEMA_VERSION);
    assert!(upto >= 35, "the blocks before schema 35 always run");
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
    // A minor, the first (sung near A4) moved down to G4 and pulled onto its centre, the
    // second sung late and stretched, the third's formants a little down — compared with
    // the original (A/B on).
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
        edit::set_pitch(&mut e, 0, 67.0);
        edit::set_field(&mut e, &[0], edit::Field::Centre, 1.0);
        edit::set_timing(&mut e, 1, 74_400, 124_800);
        edit::set_field(&mut e, &[2], edit::Field::Formant, -2.0);
        e.bypass = true;
        a.pitch = Some(Box::new(e));
    }
    // And its pitch lane shown under it.
    p.tracks[vocal_index].pitch_open = true;
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
    // Schema 29 (round 28, ADR 0058): the vocal comp — the take to beat 8, the older take
    // after, the seam crossfading over 19 ticks (~10 ms at 96 BPM) — comp A playing and
    // comp B keeping a window of its own, pitch edits and all; the record options' seam
    // default at 15 ms and automation not recorded.
    {
        let vocal = &mut p.tracks[vocal_index];
        let older = vocal.clips.iter().find(|c| c.lane == 1).cloned();
        let edited = vocal
            .clips
            .iter()
            .find(|c| c.lane == 0)
            .and_then(|c| c.audio.as_ref())
            .and_then(|a| a.pitch.clone());
        if let (Some(left), Some(older)) = (vocal.clips.iter_mut().find(|c| c.lane == 0), older) {
            left.length = Ticks::beats(4);
            // Comp windows, as schema 32 says them (`Clip::comp`).
            left.comp = true;
            if let Some(a) = left.audio.as_mut() {
                a.fade_out = Ticks::ZERO;
            }
            let mut right = older.clone();
            right.lane = 0;
            right.start = Ticks::beats(8);
            right.length = Ticks::beats(4);
            right.offset = older.offset + Ticks::beats(4);
            right.comp = true;
            let mut kept = right.clone();
            if let Some(a) = right.audio.as_mut() {
                a.xfade = Ticks(19);
            }
            if let Some(a) = kept.audio.as_mut() {
                a.pitch = edited;
            }
            right.id = ClipId(next_id(&mut p.next_id));
            kept.id = ClipId(next_id(&mut p.next_id));
            let vocal = &mut p.tracks[vocal_index];
            vocal.clips.push(right);
            vocal.clips.sort_by_key(|c| (c.start, c.id));
            vocal.comps = vec![
                cantus_core::comp::Comp {
                    name: "A".into(),
                    windows: Vec::new(),
                },
                cantus_core::comp::Comp {
                    name: "B".into(),
                    windows: vec![kept],
                },
            ];
        }
        p.recording.comp_xfade_ms = 15.0;
        p.recording.record_automation = false;
    }
    // Schema 30 (round 28, sound): an Exemplar made now, whose two round-robin zones take
    // their turns in order (`rr.order` at its default, 1, not stored); every older Exemplar
    // reads back with 0.
    let mut rr = p.make_track("Exemplar in order");
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    for (k, v) in [
        ("z0.gen", 1.0),
        ("z0.seqlen", 2.0),
        ("z1.gen", 2.0),
        ("z1.seq", 2.0),
        ("z1.seqlen", 2.0),
    ] {
        s.set(k, v).unwrap();
    }
    rr.instrument = InstrumentSpec::Native(s);
    p.tracks.push(rr);
    // Schema 31 (round 28, review 2): the vocal's second note set by *Correct pitch* at
    // 50 %, which the note now records.
    if let Some(e) = p.tracks[vocal_index]
        .clips
        .iter_mut()
        .filter(|c| c.lane == 0)
        .find_map(|c| c.audio.as_mut().and_then(|a| a.pitch.as_mut()))
    {
        cantus_core::pitch::edit::correct_pitch(e, &[1], 0.5);
    }
    // Schema 32 (round 28, fix4 of ADR 0058): a comp window says it is one. The comp's two
    // windows and B's kept window carry the flag; a third take on lane 2 (beats 12–16)
    // has a copy pasted right over it — the same file, the same place — which is no
    // window: what schema 31 took for one.
    {
        let vocal = &mut p.tracks[vocal_index];
        if let Some(take) = vocal.clips.iter().find(|c| c.lane == 1).cloned() {
            let mut third = take.clone();
            third.name = "Vocal take 2".into();
            third.lane = 2;
            third.start = Ticks::beats(12);
            third.length = Ticks::beats(4);
            third.offset = Ticks::ZERO;
            let mut pasted = third.clone();
            pasted.name = "Vocal take 2 (copy)".into();
            pasted.lane = 0;
            pasted.comp = false;
            third.id = ClipId(next_id(&mut p.next_id));
            pasted.id = ClipId(next_id(&mut p.next_id));
            let vocal = &mut p.tracks[vocal_index];
            vocal.clips.extend([third, pasted]);
            vocal.clips.sort_by_key(|c| (c.start, c.id));
        }
    }
    // Schema 33 (round 29, tempo — ADR 0060): a tempo track — a ramp from the song's tempo
    // to 140 over four bars, holding at 140, then 100 from bar 9 — a bar of 7/8 at bar 3
    // and 3/4 from bar 4, and three markers.
    {
        use cantus_core::tempo::{Marker, MarkerId, MetreChange, TempoPoint};
        use cantus_core::TimeSignature;
        p.tempo_points = vec![
            TempoPoint {
                tick: Ticks::ZERO,
                bpm: p.tempo_bpm,
                ramp: true,
            },
            TempoPoint {
                tick: Ticks::beats(16),
                bpm: 140.0,
                ramp: false,
            },
            TempoPoint {
                tick: Ticks::beats(32),
                bpm: 100.0,
                ramp: false,
            },
        ];
        p.metre_changes = vec![
            MetreChange {
                tick: Ticks::ZERO,
                signature: p.time_signature,
            },
            MetreChange {
                tick: Ticks(7680),
                signature: TimeSignature::new(7, 8),
            },
            MetreChange {
                tick: Ticks(7680 + 3360),
                signature: TimeSignature::new(3, 4),
            },
        ];
        for (name, tick) in [("Intro", 0), ("Verse", 7680), ("Chorus", 11_040)] {
            let id = MarkerId(next_id(&mut p.next_id));
            p.markers.push(Marker {
                id,
                tick: Ticks(tick),
                name: name.into(),
                color: None,
            });
        }
    }
    // Schema 34 (round 29, warp — ADR 0061): the vocal's first playing window warped —
    // Melodic, three semitones down, its formants kept, a marker on beat 2 moved a little
    // late (the take slowed into it) — so a v34 file holds every warp field but the tail.
    {
        let tempo = p.tempo_bpm;
        let rates: Vec<(cantus_core::AudioSourceId, u32)> =
            p.audio_pool.iter().map(|s| (s.id, s.sample_rate)).collect();
        let vocal = &mut p.tracks[vocal_index];
        if let Some(a) = vocal
            .clips
            .iter_mut()
            .filter(|c| c.lane == 0)
            .find_map(|c| c.audio.as_mut())
        {
            let rate = rates
                .iter()
                .find(|(id, _)| *id == a.source)
                .map_or(48_000, |r| r.1);
            let mut w = cantus_core::warp::Warp::identity(rate, tempo, a.slip_frames);
            if let Some(m) = w.add_marker(Ticks::beats(2)) {
                w = m.move_marker(1, Ticks::beats(2) + Ticks(120));
            }
            w.mode = cantus_core::warp::WarpMode::Melodic;
            w.cents = -300;
            w.detected = Some(cantus_core::warp::Detected {
                bpm: tempo,
                confidence: 0.4,
                downbeat: 0.0,
                bars: None,
            });
            a.warp = Some(Box::new(w));
        }
    }
    // Schema 35 (round 29, controllers — ADR 0062): seven controller mappings — a knob on
    // the delay's mix with a range and a bend, a 14-bit fader on strip 1 picking up with
    // feedback, a sign-bit encoder on the tempo, a pad (the second port of its name)
    // toggling the track's mute, a button firing *play*, one whose target is gone, and a
    // knob on the hosted synth's parameter, learnt on that plug-in (review 2 of ADR 0062).
    {
        use cantus_core::controller::*;
        let keys = |ordinal| PortRef {
            name: "Keystation 49".into(),
            ordinal,
        };
        let delay = p.strip(strip1).unwrap().effects[0].id;
        let track = p.tracks[0].id;
        let src = |port, channel, control| MidiSource {
            port,
            channel,
            control,
        };
        let mut knob = Mapping::new(
            MappingId(1),
            src(keys(0), Some(0), Control::Cc(74)),
            MappingTarget::Param(AutoTarget::Effect {
                effect: delay,
                key: "mix".into(),
            }),
            MapMode::Absolute,
        );
        knob.min = 0.1;
        knob.max = 0.9;
        knob.curve = 0.4;
        let mut fader = Mapping::new(
            MappingId(2),
            src(keys(0), Some(0), Control::Cc14(7)),
            MappingTarget::Param(AutoTarget::StripGain(strip1)),
            MapMode::Absolute,
        );
        fader.feedback = true;
        let mut tempo = Mapping::new(
            MappingId(3),
            src(PortRef::any(), None, Control::Cc(16)),
            MappingTarget::Tempo,
            MapMode::Relative(Encoding::SignBit),
        );
        tempo.sensitivity = 0.5;
        tempo.takeover = Takeover::Jump;
        let pad = Mapping::new(
            MappingId(4),
            src(keys(1), Some(9), Control::Note(36)),
            MappingTarget::TrackMute(track),
            MapMode::Toggle,
        );
        let play = Mapping::new(
            MappingId(5),
            src(keys(0), Some(0), Control::Cc(41)),
            MappingTarget::Action("play-pause".into()),
            MapMode::Trigger,
        );
        let gone = Mapping::new(
            MappingId(6),
            src(keys(0), Some(0), Control::Cc(75)),
            MappingTarget::TrackSolo(TrackId(999_999)),
            MapMode::Toggle,
        );
        let synth = p
            .tracks
            .iter()
            .find(|t| t.instrument.external().is_some())
            .expect("the hosted synth of schema 20");
        let hosted_target = MappingTarget::Param(AutoTarget::PluginInstrument {
            track: synth.id,
            id: 1,
        });
        let mut hosted = Mapping::new(
            MappingId(7),
            src(keys(0), Some(0), Control::Cc(71)),
            hosted_target.clone(),
            MapMode::Absolute,
        );
        hosted.plugin = hosted_target.plugin_of(&p);
        assert!(hosted.plugin.is_some());
        // The names a learnt plug-in carries came with schema 41.
        if let Some(r) = hosted.plugin.as_mut() {
            if upto >= 41 {
                r.param = "Cutoff".into();
            } else {
                r.name.clear();
            }
        }
        p.controllers.mappings = vec![knob, fader, tempo, pad, play, gone, hosted];
    }
    // Schema 36 (round 29, isolation, ADR 0063): where a hosted plug-in runs — the absent
    // insert of schema 14 *Individually*, the hosted synth of schema 20 *Within Cantus*.
    for slot in p.all_effects_mut().filter(|_| upto >= 36) {
        if let Some(plugin) = slot.spec.external.as_deref_mut() {
            plugin.isolation = Some(cantus_core::PluginIsolation::Individually);
        }
    }
    for t in p.tracks.iter_mut().filter(|_| upto >= 36) {
        if let Some(plugin) = t.instrument.external_mut() {
            plugin.isolation = Some(cantus_core::PluginIsolation::Within);
        }
    }
    // Schema 38 (round 29, review 2 — ADR 0064 §6): the second track's audio arrives 4 608
    // frames late (a bounce in place through a linear-phase EQ keeps that in its file).
    if let Some(t) = p.tracks.get_mut(1).filter(|_| upto >= 38) {
        t.lateness = Some(cantus_core::Lateness {
            frames: 4_608,
            rate: 48_000,
        });
    }
    // Schema 39 (round 29, warp's second review — ADR 0061 §11): an audio window under the
    // tempo ramp warped to follow from the natural warp — the warp's own markers on the
    // ramp and the tempo points — with a user's marker dragged across them.
    if upto >= 39 {
        let map = p.tempo_map();
        let rates: Vec<(cantus_core::AudioSourceId, u32)> =
            p.audio_pool.iter().map(|s| (s.id, s.sample_rate)).collect();
        let clip = p
            .tracks
            .iter_mut()
            .flat_map(|t| t.clips.iter_mut())
            .filter(|c| c.lane == 0 && c.link.is_none())
            .find(|c| c.audio.as_ref().is_some_and(|a| a.warp.is_none()))
            .expect("an unwarped audio window");
        // (The vocal at bar 3: under the ramp.)
        let a = clip.audio.as_mut().unwrap();
        let rate = rates
            .iter()
            .find(|(id, _)| *id == a.source)
            .map_or(48_000, |r| r.1);
        let mut w = cantus_core::warp::Warp::natural_of(
            &map,
            clip.start,
            clip.offset,
            clip.length,
            a.slip_frames,
            rate,
        );
        assert!(w.markers.iter().any(|m| m.auto), "the ramp's own markers");
        if let Some(n) = w.add_marker(Ticks::beats(1)) {
            let i = n.marker_at(Ticks::beats(1)).unwrap();
            w = n.move_marker(i, Ticks::beats(2));
        }
        a.warp = Some(Box::new(w));
    }
    // Schema 40 (round 29, ADR 0062 review 3): a pad on strip 1's effects switch and one on
    // its record arm (the mixer's FX bulb and record dot).
    if upto >= 40 {
        use cantus_core::controller::*;
        let pad = |id, note, target| {
            Mapping::new(
                MappingId(id),
                MidiSource {
                    port: PortRef {
                        name: "Keystation 49".into(),
                        ordinal: 0,
                    },
                    channel: Some(9),
                    control: Control::Note(note),
                },
                target,
                MapMode::Toggle,
            )
        };
        p.controllers
            .mappings
            .push(pad(8, 40, MappingTarget::StripFx(strip1)));
        p.controllers
            .mappings
            .push(pad(9, 41, MappingTarget::StripArm(strip1)));
    }
    // Schema 42 (round 31, ADR 0068): the Keys track writes in Touch and the master in
    // Latch; its first clip bends up and back, opens the mod wheel, and its notes carry
    // release velocities (the last none, as every older note).
    if upto >= 42 {
        use cantus_core::clip_lanes::{ClipLane, LaneKind};
        p.master.auto_mode = AutoMode::Latch;
        let keys = &mut p.tracks[0];
        keys.auto_mode = AutoMode::Touch;
        let clip = &mut keys.clips[0];
        let mut up = AutoPoint::new(Ticks::beats(1), 0.75);
        up.tension = 0.4;
        clip.lanes = vec![
            ClipLane::new(
                LaneKind::PitchBend,
                vec![
                    AutoPoint::new(Ticks::ZERO, 0.5),
                    up,
                    AutoPoint::new(Ticks::beats(2), 0.5),
                ],
            ),
            ClipLane::new(
                LaneKind::Cc(1),
                vec![
                    AutoPoint::new(Ticks::ZERO, 0.0),
                    AutoPoint::new(Ticks::beats(6), 1.0),
                ],
            ),
        ];
        for (i, n) in clip.notes.iter_mut().enumerate().take(3) {
            n.release = 40 + 20 * i as u8;
        }
    }
    // Schema 43 (round 31, ADR 0069): the song in D minor, the first track's note effects
    // (chords from the key into an up-down arpeggiator in 1/8 triplets over two octaves,
    // then a scale that drops what leaves the key), a groove extracted into the pool and
    // the first clip swung by it (timing and accents), the second by a built-in 16th swing.
    if upto >= 43 {
        use cantus_core::groove::{ClipGroove, Groove, GrooveId, GrooveSource, SwingGrid};
        use cantus_core::notefx::{
            ArpMode, ChordShape, NoteEffect, NoteFxId, NoteFxKind, NoteFxType, ScaleMode,
        };
        p.key = Some(cantus_core::Key::new(2, cantus_core::ScaleKind::Minor));
        let mut chord = NoteEffect::new(NoteFxId(p.alloc_id()), NoteFxType::Chord);
        if let NoteFxKind::Chord(c) = &mut chord.kind {
            c.shape = ChordShape::ScaleSeventh;
            c.inversion = 1;
        }
        let mut arp = NoteEffect::new(NoteFxId(p.alloc_id()), NoteFxType::Arp);
        if let NoteFxKind::Arp(a) = &mut arp.kind {
            a.mode = ArpMode::UpDown;
            a.octaves = 2;
            a.rate = 11; // 1/8T
            a.gate = 0.6;
            a.swing = 0.58;
        }
        let mut scale = NoteEffect::new(NoteFxId(p.alloc_id()), NoteFxType::Scale);
        if let NoteFxKind::Scale(s) = &mut scale.kind {
            s.mode = ScaleMode::Drop;
        }
        scale.enabled = false;
        let gid = GrooveId(p.alloc_id());
        let hits: Vec<_> = (0..16)
            .map(|k| {
                let late = if k % 2 == 1 { 48 } else { 0 };
                (
                    Ticks(k * 240 + late),
                    if k % 4 == 0 { 120.0 } else { 80.0 },
                    Ticks(200),
                )
            })
            .collect();
        let mut groove = Groove::extract(gid, "Played".into(), &hits, Ticks(1920), Ticks(240))
            .expect("a groove");
        groove.name = "Fixture feel".into();
        p.grooves.push(groove);
        if let Some(t) = p.tracks.first_mut() {
            t.note_fx = vec![chord, arp, scale];
            if let Some(c) = t.clips.first_mut() {
                c.groove = Some(ClipGroove {
                    source: GrooveSource::Pool { id: gid },
                    timing: 0.8,
                    velocity: 0.5,
                    length: 0.0,
                });
            }
            if let Some(c) = t.clips.get_mut(1).filter(|c| c.audio.is_none()) {
                c.groove = Some(ClipGroove::new(GrooveSource::Swing {
                    grid: SwingGrid::Sixteenths,
                    percent: 62,
                }));
                c.skip_note_fx = true;
            }
        }
    }
    // Schema 44 (round 31, ADR 0070): a track of two clips of the vocal's file overlapping
    // by a beat with an S-curve crossfade, the first with a gain envelope (a dip, a hold
    // and a bend).
    if upto >= 44 {
        use cantus_core::clip_gain::{GainEnvelope, GainPoint};
        let src = p.audio_pool[0].id;
        let mut t = p.make_track("Doubles");
        let mut a = p.make_audio_clip("Double A", src, Ticks::beats(16), Ticks::beats(4));
        let mut points = vec![
            GainPoint::new(Ticks::ZERO, 0.0),
            GainPoint::new(Ticks::beats(1), -9.5),
            GainPoint::new(Ticks::beats(2), -9.5),
            GainPoint::new(Ticks::beats(3), 2.0),
        ];
        points[1].shape = cantus_core::Shape::Hold;
        points[2].tension = 0.4;
        if let Some(r) = a.audio.as_mut() {
            r.gain_env = Some(Box::new(GainEnvelope::new(points)));
        }
        let mut b = p.make_audio_clip("Double B", src, Ticks::beats(19), Ticks::beats(4));
        b.offset = Ticks::beats(3);
        if let Some(r) = b.audio.as_mut() {
            r.cross = Some(cantus_core::crossfade::XfadeCurve::SCurve);
        }
        t.clips.extend([a, b]);
        p.tracks.push(t);
    }
    // Schema 45 (round 32, ADR 0075): the first note track's score settings — a grand
    // staff split at D4, written a tone up, dynamics from velocity, read in triplets.
    if upto >= 45 {
        use cantus_core::notation::{ReadingGrid, ScoreSettings, StaffChoice};
        if let Some(t) = p
            .tracks
            .iter_mut()
            .find(|t| t.clips.iter().any(|c| !c.notes.is_empty()))
        {
            t.score = ScoreSettings {
                staves: StaffChoice::Grand,
                split: Some(62),
                transpose: 2,
                dynamics: true,
                grid: ReadingGrid::EighthTriplet,
                name: Some("Piano".into()),
                ..Default::default()
            };
        }
    }
    // Schema 37 (round 29, freeze — ADR 0064): the first strip frozen (everything it
    // gathers and its chain up to the width) and the last track's own source, each with
    // the content key of what it holds — computed last, after every block above.
    if upto >= 37 {
        use cantus_core::freeze::{content_key, FreezeTarget, Frozen};
        if let Some(s) = p.strips.first().map(|s| s.id) {
            let content = content_key(&p, FreezeTarget::Strip(s)).expect("a strip");
            p.strip_mut(s).unwrap().frozen = Some(Frozen { content });
        }
        if let Some(t) = p.tracks.last().map(|t| t.id) {
            let content = content_key(&p, FreezeTarget::Track(t)).expect("a track");
            p.track_mut(t).unwrap().frozen = Some(Frozen { content });
        }
    }
    let name = format!("v{upto}.cantus");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    cantus_project::save(&p, &path, "fixture").expect("save fixture");
    if upto < cantus_project::SCHEMA_VERSION {
        // Saved as that schema: what its build wrote (the later blocks left out above).
        let text = std::fs::read_to_string(&path).expect("read back");
        let from = format!("\"schema_version\": {}", cantus_project::SCHEMA_VERSION);
        assert!(text.contains(&from));
        let text = text.replacen(&from, &format!("\"schema_version\": {upto}"), 1);
        std::fs::write(&path, text).expect("write back");
    }
    println!("wrote {}", path.display());
}

/// A fresh id, as `Project::alloc_id` gives it, while the project is borrowed elsewhere.
fn next_id(next: &mut u64) -> u64 {
    let id = *next;
    *next += 1;
    id
}
