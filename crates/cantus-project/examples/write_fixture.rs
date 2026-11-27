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
    // Schema 46 (round 33, ADR 0076): a session — three scenes (the second with a tempo, a
    // metre, a colour and a follow action), the first note track's column holding a looping
    // MIDI clip launched as a gate on a quarter with legato, a start marker, follow actions
    // and a clip envelope of its own level, the pool's first audio file looping in the next
    // slot, and an empty slot whose stop button was taken away; the session's own settings.
    if upto >= 46 {
        use cantus_core::session::*;
        let ids: Vec<SceneId> = (0..3).map(|_| SceneId(p.alloc_id())).collect();
        for (i, id) in ids.iter().enumerate() {
            let mut s = Scene::new(*id, ["Intro", "Verse", "Drop"][i]);
            if i == 1 {
                s.tempo = Some(128.0);
                s.metre = Some(TimeSignature::new(7, 8));
                s.color = Some(Color::rgb(0x4F, 0xA3, 0xC7));
                s.follow = FollowActions {
                    on: true,
                    a: FollowAction::Next,
                    b: FollowAction::Jump(ids[0]),
                    chance_a: 75,
                    after: FollowAfter::Time(Ticks::beats(16)),
                    count: 2,
                };
            }
            p.session.scenes.push(s);
        }
        p.session.settings = SessionSettings {
            launch_quantize: Quantize::Bars2,
            record_quantize: RecordQuantize::SixteenthAndTriplet,
            seed: 33,
            fixed_bars: Some(4),
        };
        let mut clip = p.make_clip("Session riff", Ticks::ZERO, Ticks::beats(8));
        clip.content_length = Some(Ticks::beats(8));
        for (i, pitch) in [62u8, 65, 69].iter().enumerate() {
            let mut n = p.make_note(Ticks::beats(i as i64 * 2), Ticks::QUARTER, *pitch, 96);
            n.start = Ticks::beats(i as i64 * 2);
            clip.notes.push(n);
        }
        let source = p.audio_pool.first().map(|s| s.id);
        let tid = p
            .tracks
            .iter()
            .find(|t| t.clips.iter().any(|c| !c.notes.is_empty()))
            .map(|t| t.id)
            .expect("a note track");
        let mut slots = Vec::new();
        let mut sc = SessionClip::new(clip);
        sc.launch = LaunchSettings {
            mode: LaunchMode::Gate,
            quantize: Quantize::Quarter,
            legato: true,
            velocity: 0.5,
        };
        sc.follow = FollowActions {
            on: true,
            a: FollowAction::Other,
            b: FollowAction::Stop,
            chance_a: 60,
            after: FollowAfter::Linked,
            count: 4,
        };
        sc.start = Some(Ticks::beats(2));
        sc.envelopes.push(cantus_core::Automation::new(
            cantus_core::AutoTarget::TrackGain(tid),
            vec![
                cantus_core::AutoPoint::new(Ticks::ZERO, 0.4),
                cantus_core::AutoPoint::new(Ticks::beats(8), 0.9),
            ],
        ));
        slots.push(Slot {
            scene: ids[0],
            clip: Some(sc),
            stop: true,
        });
        if let Some(src) = source {
            let mut a = p.make_audio_clip("Session loop", src, Ticks::ZERO, Ticks::beats(4));
            a.content_length = Some(Ticks::beats(4));
            slots.push(Slot {
                scene: ids[1],
                clip: Some(SessionClip::new(a)),
                stop: true,
            });
        }
        slots.push(Slot {
            scene: ids[2],
            clip: None,
            stop: false,
        });
        p.track_mut(tid).unwrap().slots = slots;
    }
    // Schema 47 (round 33, ADR 0080): per-note expression — the first note of the first
    // note track bent up a tone and back, pressed and brightened; the hosted instrument's
    // expression sent as MPE.
    if upto >= 47 {
        use cantus_core::model::note_expr::{ExprKind, ExprPoint, NoteExpr};
        let st = |x: f64| ExprKind::Bend.store(x);
        let u = |x: f64| ExprKind::Pressure.store(x);
        if let Some(n) = p
            .tracks
            .iter_mut()
            .flat_map(|t| t.clips.iter_mut())
            .find_map(|c| c.notes.first_mut())
        {
            n.expr = NoteExpr {
                bend: vec![ExprPoint(0, 0), ExprPoint(240, st(2.0)), ExprPoint(480, 0)],
                pressure: vec![ExprPoint(0, u(0.2)), ExprPoint(120, u(0.9))],
                timbre: vec![ExprPoint(0, u(0.5)), ExprPoint(360, u(0.75))],
            }
            .boxed();
        }
        if let Some(cantus_core::InstrumentSpec::External(x)) = p
            .tracks
            .iter_mut()
            .map(|t| &mut t.instrument)
            .find(|i| i.external().is_some())
        {
            x.expression = Some(cantus_core::external::ExprRoute::MpeMidi);
        }
    }
    // Schema 48 (round 34, ADR 0085): the first file of the pool marked as a model's
    // separation — its vocals, by HT-Demucs. // r34:stems
    if upto >= 48 {
        if let Some(id) = p.audio_pool.first().map(|s| s.id) {
            p.provenance.insert(
                id,
                cantus_core::Provenance {
                    kind: "separated".into(),
                    model: "htdemucs".into(),
                    model_name: "HT-Demucs — 4 stems".into(),
                    model_version: 1,
                    licence: "MIT".into(),
                    date: "2026-10-30".into(),
                    from: "Song".into(),
                    part: "vocals".into(),
                },
            );
        }
    }
    // Schema 52 (round 36, ADR 0096): the audio editor's marks on the
    // pool's first file — two markers, a named region and loop points. // r36:editor
    if upto >= 52 {
        if let Some(s) = p.audio_pool.first() {
            let (id, f) = (s.id, s.frames.max(4));
            p.source_marks.insert(
                id,
                cantus_core::SourceMarks {
                    markers: vec![
                        cantus_core::SourceMarker {
                            frame: f / 4,
                            name: "Verse".into(),
                        },
                        cantus_core::SourceMarker {
                            frame: f / 2,
                            name: String::new(),
                        },
                    ],
                    regions: vec![cantus_core::SourceRegion {
                        start: f / 4,
                        end: f / 2,
                        name: "Take 2".into(),
                    }],
                    loop_points: Some((f / 8, f / 2)),
                },
            );
        }
    }
    // Schema 49 (round 35, ADR 0087): a dancer — a character chosen, a few numbers moved,
    // where it shows, `move` automated to Clap over bar 3 and a note in its move map.
    // Before the last track, which the freeze block below freezes. // r35:dancer
    if upto >= 49 {
        use cantus_core::inst::dancer::{self, Move};
        let mut t = p.make_track("Dancer");
        let id = t.id;
        let mut n = NativeSpec::new(InstrumentKind::Dancer);
        n.set("energy", 0.9).unwrap();
        n.set("hue", -40.0).unwrap();
        n.set("view.mode", 1.0).unwrap();
        // The songbird by name (the default before schema 59). // r37:chars
        n.set_text(dancer::CHARACTER_KEY, Some("builtin:songbird".into()))
            .unwrap();
        t.instrument = InstrumentSpec::Native(n);
        t.icon = Some(TrackIcon::Dance);
        let clap = dancer::move_value(Move::Clap) / 16.0;
        let auto = p.make_automation_clip(
            "Moves",
            AutoTarget::Instrument {
                track: id,
                key: "move".into(),
            },
            Ticks::beats(8),
            Ticks::beats(4),
            vec![AutoPoint {
                tick: Ticks::ZERO,
                value: clap,
                tension: 0.0,
                shape: Shape::Hold,
            }],
        );
        let mut c = p.make_clip("Steps", Ticks::beats(12), Ticks::beats(4));
        let jump = dancer::BASE_KEY + Move::Jump.index() as u8;
        c.notes
            .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, jump, 110));
        t.clips.extend([auto, c]);
        let at = p.tracks.len() - 1;
        p.tracks.insert(at, t);
    }
    // Schema 50 (round 36, ADR 0093): a VCA the Keys strip follows (its fader at -3 dB), a
    // snapshot of the desk, a cue at bar 3 that recalls it, and the strip marked safe.
    if upto >= 50 {
        use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
        use cantus_core::vca::StripKind;
        let mut v = p.make_strip("Keys VCA");
        v.kind = StripKind::Vca;
        v.gain_db = -3.0;
        let vca = v.id;
        p.strips.push(v);
        p.strip_mut(strip1).unwrap().vcas.push(vca);
        p.strip_mut(strip1).unwrap().recall_safe = true;
        let state = capture(&p, &[], true);
        let id = MixSnapId(p.alloc_id());
        p.mix.snapshots.push(MixSnapshot {
            id,
            name: "Verse".into(),
            state,
        });
        let cue = MixCueId(p.alloc_id());
        p.mix.cues.push(MixCue {
            id: cue,
            at: Ticks::beats(8),
            snapshot: id,
            scope: Vec::new(),
        });
    }
    // Schema 51 (round 36, ADR 0094): an external instrument on a track of its own ("Outboard
    // synth") — notes to "Hardware Synth" channel 3, audio back on inputs 5–6, measured at 612
    // frames and trimmed 1.5 ms — and an external effect on another ("Outboard bus"), both
    // outside every region the freeze block below freezes (a frozen region cannot hold
    // outboard gear: `CannotFreeze::External`). Inserted before the last track, which that
    // block freezes. r36:external
    if upto >= 51 {
        use cantus_core::external_unit::{Measured, MidiRoute, Pair};
        let synth = p.make_track("Outboard synth");
        let bus = p.make_track("Outboard bus");
        let (synth_id, bus_id) = (synth.id, bus.id);
        let at = p.tracks.len().saturating_sub(1);
        p.tracks.insert(at, bus);
        p.tracks.insert(at, synth);
        let id = p.new_external_unit_id();
        let mut u = ExternalUnit::instrument(
            id,
            synth_id,
            MidiRoute {
                port: "Hardware Synth".into(),
                ordinal: 0,
                channel: 2,
            },
            Pair::stereo(4),
        );
        u.name = "Bass synth".into();
        u.device = "Fixture Interface".into();
        u.trim_ms = 1.5;
        u.latency = Some(Measured::bare(612, 48_000, 41.5));
        p.external_units.push(u);
        let id = p.new_external_unit_id();
        let mut e = ExternalUnit::effect(id, bus_id, Pair::stereo(2), Pair::stereo(2));
        e.name = "Outboard compressor".into();
        e.return_db = -3.0;
        p.external_units.push(e);
    }
    // Schema 53 (round 36, ADR 0097): an audio clip bound to an ARA 2 plug-in, with the
    // plug-in's archive of its modification. Before the freeze block. // r36:ara
    if upto >= 53 {
        use cantus_core::ara::{AraArchive, AraClip, AraPlugin};
        let src = p.audio_pool.first().map(|s| s.id).expect("a source");
        let mut c = p.make_audio_clip("ARA take", src, Ticks::beats(24), Ticks::beats(4));
        let mut ara = AraClip::new(AraPlugin {
            id: "41524154455354504C5547494E000001".into(),
            name: "Cantus Test ARA".into(),
            vendor: "Cantus Tests".into(),
            path: "cantus_test_ara.vst3".into(),
            factory_id: "org.cantus.test.ara.factory".into(),
        });
        ara.archive = Some(AraArchive {
            document_archive_id: "org.cantus.test.ara.document.v1".into(),
            source_id: cantus_core::ara::source_persistent_id(src),
            modification_id: cantus_core::ara::modification_persistent_id(c.id),
            data: cantus_core::PluginState::new(b"gain=0.5\n".to_vec()),
        });
        c.audio.as_mut().unwrap().ara = Some(Box::new(ara));
        let vocal = p
            .tracks
            .iter_mut()
            .find(|t| t.name == "Vocal")
            .expect("the vocal track");
        vocal.clips.push(c);
    }
    // Schema 55 (round 37, ADR 0099): a loop sliced by Sectio — an Exemplar of four slices
    // of one file, its points and settings, the third slice reversed and low-passed, the
    // last choked. // r37:piano
    if upto >= 55 {
        let s = cantus_core::sectio::Slicing {
            mode: cantus_core::sectio::SliceMode::Transients,
            sens: 0.6,
            bpm: 120.0,
            grid: 4,
            len: 96_000,
            rate: 48_000,
            points: vec![0, 24_000, 48_000, 72_000],
        };
        let mut spec = NativeSpec::new(InstrumentKind::Sampler);
        for (k, v) in s.values() {
            spec.set(&k, v);
        }
        for (n, mut z) in s.zones("loops/break.wav").into_iter().enumerate() {
            use cantus_core::inst::sampler::zf;
            if n == 2 {
                z.set(zf::REV, 1.0).set(zf::LPF, 2_400.0);
            }
            if n == 3 {
                z.set(zf::EXCL, 1.0).set(zf::OFFBY, 1.0);
            }
            for (k, v) in z.values_for(n) {
                spec.set(&k, v);
            }
            for (k, text) in z.texts_for(n) {
                let _ = spec.set_text(&k, Some(text));
            }
        }
        let mut sliced = p.make_track("Sliced loop");
        sliced.instrument = InstrumentSpec::Native(spec);
        p.tracks.push(sliced);
    }
    // Schema 56 (round 37, ADR 0100): the Drums kit split to strips — each pad that sounds
    // on a strip of its own, into the kit's, with the 0 dB pan law — and one pad strip's
    // fader down. Before the freeze block. // r37:drums
    if upto >= 56 {
        let drums = p
            .tracks
            .iter()
            .find(|t| t.name == "Drums")
            .map(|t| t.id)
            .expect("the drums track");
        let view = p.clone();
        let cmd = cantus_core::padout::split_with(&view, drums, |n| p.make_strip(n))
            .expect("the kit splits");
        cmd.apply(&mut p).expect("split");
        let first = p.track(drums).unwrap().instrument.output_strips()[0];
        p.strip_mut(first).unwrap().gain_db = -3.0;
    }
    // Schema 57 (round 37, ADR 0100 §8): the Drums kit's own gain. // r37:drums
    if upto >= 57 {
        if let Some(k) = p
            .tracks
            .iter_mut()
            .find(|t| t.name == "Drums")
            .and_then(|t| t.instrument.pulsus_mut())
        {
            k.kit.gain_db = 3.5;
        }
    }
    // Schema 58 (round 37, ADR 0104): the Chorda shown as a walnut upright — a look no
    // sample reads. // r37:scene
    if upto >= 58 {
        use cantus_core::inst::chorda_look::{BODY_KEY, FINISH_KEY};
        if let Some(n) = p
            .tracks
            .iter_mut()
            .find(|t| t.name == "Chorda")
            .and_then(|t| t.instrument.native_mut())
        {
            n.set_text(BODY_KEY, Some("upright".into())).unwrap();
            n.set_text(FINISH_KEY, Some("walnut".into())).unwrap();
        }
    }
    // Schema 60 (round 39, ADR 0112): Aula on the second strip, playing a response from a
    // file (from a pack, so its credit is kept), with its response shaped and a quarter wet.
    // r39:conv
    if upto >= 60 {
        let mut aula = p.make_effect(EffectKind::Aula);
        for (k, v) in [
            ("ir", 9.0),
            ("predelay", 18.0),
            ("stretch", 1.25),
            ("decay", 0.8),
            ("low", 120.0),
            ("damp", 0.35),
            ("mix", 0.25),
        ] {
            aula.spec.set(k, v).unwrap();
        }
        aula.spec.ir = Some(Box::new(IrFile {
            path: "irs/Hall A.wav".into(),
            pack: Some("cantus-impulse-responses".into()),
        }));
        let s = p.strips.get(1).map(|s| s.id).expect("a second strip");
        p.strip_mut(s).unwrap().effects.push(aula);
    }
    // Schema 61 (round 39, ADR 0113): two sections (an intro of 4 bars, a verse of 8 in
    // the palette's second colour), the first two tracks in a closed folder, and the first
    // track's level lane touching while the track reads. // r39:arrange
    if upto >= 61 {
        use cantus_core::arrange::{FolderId, LaneMode, Section, SectionId, TrackFolder};
        let bar = Ticks::beats(4);
        let intro = SectionId(p.alloc_id());
        let verse = SectionId(p.alloc_id());
        p.sections.push(Section {
            id: intro,
            start: Ticks::ZERO,
            length: Ticks(bar.0 * 4),
            name: "Intro".into(),
            color: None,
        });
        p.sections.push(Section {
            id: verse,
            start: Ticks(bar.0 * 4),
            length: Ticks(bar.0 * 8),
            name: "Verse".into(),
            color: Some(cantus_core::Color::PALETTE[1]),
        });
        let folder = FolderId(p.alloc_id());
        p.folders.push(TrackFolder {
            id: folder,
            name: "Drums".into(),
            color: cantus_core::Color::PALETTE[0],
            open: false,
        });
        for t in p.tracks.iter_mut().take(2) {
            t.folder = Some(folder);
        }
        if let Some(t) = p.tracks.first_mut() {
            t.lane_modes.push(LaneMode {
                target: AutoTarget::TrackGain(t.id),
                mode: cantus_core::AutoMode::Touch,
            });
            t.glyph = Some("kick".into());
        }
    }
    // Schema 62 (round 40, ADR 0118): the second track records after its strip's inserts.
    // r40:pads
    if upto >= 62 {
        if let Some(t) = p.tracks.get_mut(1) {
            t.record_tap = cantus_core::RecordTap::AfterFx;
        }
    }
    // Schema 66 (round 40 phase 3, ADR 0123): the Chorda's *Room* perspective on the
    // second strip, 3 dB down in its own mixer. // r40:p3
    if upto >= 66 {
        let to = p.strips.get(1).map(|s| s.id);
        if let (Some(to), Some(n)) = (
            to,
            p.tracks
                .iter_mut()
                .find(|t| t.name == "Chorda")
                .and_then(|t| t.instrument.native_mut()),
        ) {
            n.outputs.push(OutputSlot {
                id: OutputId(3),
                name: "Room".into(),
                family: OutputFamily::Rooms,
                strip: to,
            });
            n.set("o3.level", -3.0).unwrap();
        }
    }
    // Schema 67 (round 40 phase 3, ADR 0124): a Tympanum playing a recorded kit, a piece
    // tuned up, a bleed cell lowered, a mic turned, its snare-top mic on the first strip.
    // r40:p3 (`tymp-engine` keeps this block in step with the model)
    if upto >= 67 {
        use cantus_core::inst::tympanum::KIT_KEY;
        let to = p.strips.first().map(|s| s.id);
        let mut drums = p.make_track("Tympanum");
        let mut n = NativeSpec::new(InstrumentKind::AcousticDrums);
        n.set_text(KIT_KEY, Some("pack:crocellkit/kit".into()))
            .unwrap();
        n.set("p1.tune", 2.0).unwrap();
        n.set("b1.3.db", -6.0).unwrap();
        n.set("m2.az", 10.0).unwrap();
        if let Some(to) = to {
            n.outputs.push(OutputSlot {
                id: OutputId(1),
                name: "Snare Top".into(),
                family: OutputFamily::Snare,
                strip: to,
            });
        }
        drums.instrument = InstrumentSpec::Native(n);
        // Before the last track, which the freeze block below freezes (indices kept).
        let at = p.tracks.len().saturating_sub(1);
        p.tracks.insert(at, drums);
    }
    // Schema 68 (round 40 phase 3, ADR 0125): the Chorda playing the Salamander, one of its
    // zones kept under `rec.`. // r40:p3 (`chorda-src` keeps this block in step)
    if upto >= 68 {
        use cantus_core::inst::chorda_source::SOURCE_KEY;
        if let Some(n) = p
            .tracks
            .iter_mut()
            .find(|t| t.name == "Chorda")
            .and_then(|t| t.instrument.native_mut())
        {
            n.set_text(SOURCE_KEY, Some("pack:salamander-grand-piano/grand".into()))
                .unwrap();
            n.set("rec.z0.lo", 60.0).unwrap();
        }
    }
    // Schema 69 (round 40 phase 3, ADR 0126 amended): the Chorda's scene in a hall with an
    // ebony stage, brick walls and a blue rug. // r40:scene
    if upto >= 69 {
        use cantus_core::inst::room_look::{FLOOR_KEY, ROOM_KEY, RUG_KEY, WALLS_KEY};
        if let Some(n) = p
            .tracks
            .iter_mut()
            .find(|t| t.name == "Chorda")
            .and_then(|t| t.instrument.native_mut())
        {
            for (k, w) in [
                (ROOM_KEY, "hall"),
                (FLOOR_KEY, "ebony_stage"),
                (WALLS_KEY, "brick"),
                (RUG_KEY, "blue"),
            ] {
                n.set_text(k, Some(w.into())).unwrap();
            }
        }
    } else if let Some(n) = p
        .tracks
        .iter_mut()
        .find(|t| t.name == "Chorda")
        .and_then(|t| t.instrument.native_mut())
    {
        // An older build's Chorda had no room key (a new one writes the stage).
        n.set_text(cantus_core::inst::room_look::ROOM_KEY, None)
            .unwrap();
    }
    // Schema 70 (round 40 phase 3, ADR 0123 amended): the Tympanum's bus — the last strip,
    // named "Kit Bus" — added to the Tympanum's family on the desk. // r40:tymp-ui
    if upto >= 70 {
        let kit = p.tracks.iter().find(|t| t.name == "Tympanum").map(|t| t.id);
        if let (Some(kit), Some(bus)) = (kit, p.strips.last_mut()) {
            bus.name = "Kit Bus".into();
            bus.attached = Some(kit);
        }
    }
    // Schema 71 (round 40, ADR 0124's third amendment): the jazz kit, hybrid — a recorded
    // residual under the model. // r40:drums-hyb
    if upto >= 71 {
        use cantus_core::inst::tympanum::voice::HYBRID;
        use cantus_core::inst::tympanum::{KIT_KEY, VOICE_KEY};
        let mut drums = p.make_track("Tympanum hybrid");
        let mut n = NativeSpec::new(InstrumentKind::AcousticDrums);
        n.set_text(KIT_KEY, Some("model:jazz".into())).unwrap();
        n.set_text(VOICE_KEY, Some(HYBRID.into())).unwrap();
        drums.instrument = InstrumentSpec::Native(n);
        let at = p.tracks.len().saturating_sub(1);
        p.tracks.insert(at, drums);
    }
    // fx:contract — Schema 72 (the effects side round, ADRs 0127–0130; provisional 62 on its
    // branch): one insert of each of the five new kinds on the second
    // strip, Cribrum's output at −3 dB; each unit's own settings below.
    if upto >= 72 {
        let s = p.strips.get(1).map(|s| s.id).expect("a second strip");
        for kind in [
            EffectKind::AutoFilter,
            EffectKind::Pedal,
            EffectKind::TimeShaper,
            EffectKind::LoFi,
            EffectKind::Utility,
        ] {
            let mut e = p.make_effect(kind);
            if kind == EffectKind::AutoFilter {
                e.spec.set("out", -3.0).unwrap();
            }
            // fx:lapsus — Lapsus with a volume pattern of its own in slot 5.
            if kind == EffectKind::TimeShaper {
                use cantus_core::fx::lapsus::{Bank, Pattern, Point};
                let mut bank = Bank::default();
                bank.vol.insert(
                    4,
                    Pattern {
                        len: 1,
                        points: vec![Point::new(0.0, 1.0).shape(2), Point::new(0.5, 0.25)],
                        name: Some("Half gate".into()),
                    },
                );
                e.spec.lapsus = Some(Box::new(bank));
            }
            p.strip_mut(s).unwrap().effects.push(e);
        }
    }
    // fx3:contract — Schema 73 (the effects side round's wave 3, ADRs 0131–0134; provisional
    // 63): on the second strip a board — Fragor's green
    // overdrive on A, Delay at 250 ms on B (a split), macro 1 named Drive moving the
    // overdrive's drive from 20 to 80 %, at 25 % — then one insert of each of Saltus, Rota
    // and Arca. Each builder adds its own settings here.
    if upto >= 73 {
        use cantus_core::fx::tabula::{Board, Macro, MacroMap, Path, Pedal, PedalId};
        let s = p.strips.get(1).map(|s| s.id).expect("a second strip");
        let mut e = p.make_effect(EffectKind::Board);
        let mut fragor = EffectSpec::new(EffectKind::Pedal);
        fragor.set("s1", 1.0).unwrap();
        let mut delay = EffectSpec::new(EffectKind::Delay);
        delay.set("time_ms", 250.0).unwrap();
        e.spec.board = Some(Box::new(Board {
            pedals: vec![
                Pedal {
                    path: Path::A,
                    ..Pedal::new(PedalId(0), fragor)
                },
                Pedal {
                    path: Path::B,
                    ..Pedal::new(PedalId(1), delay)
                },
            ],
            macros: vec![Macro {
                name: Some("Drive".into()),
                maps: vec![MacroMap {
                    pedal: PedalId(0),
                    key: "od.drive".into(),
                    from: 0.2,
                    to: 0.8,
                }],
            }],
        }));
        e.spec.set("m1", 0.25).unwrap();
        p.strip_mut(s).unwrap().effects.push(e);
        for kind in [EffectKind::PitchShift, EffectKind::Rotary, EffectKind::Amp] {
            let e = p.make_effect(kind);
            p.strip_mut(s).unwrap().effects.push(e);
        }
    }
    // fxa:contract — Schema 74 (the effects side round's wave A, ADRs 0136–0138; provisional
    // 64): on the second strip one insert each of
    // Loquax (its mix up), Fractus (a repeat on the last beat, a half-strength gate on
    // beat three that plays three times in four) and Caelum (its shimmer up).
    if upto >= 74 {
        use cantus_core::fx::fractus::{Block, Pattern};
        let s = p.strips.get(1).map(|s| s.id).expect("a second strip");
        let mut loquax = p.make_effect(EffectKind::Vocoder);
        loquax.spec.set("mix", 1.0).unwrap();
        let mut fractus = p.make_effect(EffectKind::Glitch);
        let mut pat = Pattern::default();
        pat.replace(0, &[Block::new(12, 4)]);
        pat.replace(
            7,
            &[Block {
                amount: 0.5,
                prob: 0.75,
                ..Block::new(8, 2)
            }],
        );
        fractus.spec.fractus = Some(Box::new(pat));
        let mut caelum = p.make_effect(EffectKind::Shimmer);
        caelum.spec.set("shim", 0.55).unwrap();
        for e in [loquax, fractus, caelum] {
            p.strip_mut(s).unwrap().effects.push(e);
        }
    }
    // fxb:contract — Schema 75 (the effects side round's wave B, ADRs 0139–0141; provisional
    // 65 on its branch): on the second strip one insert each of
    // Vinculum (the opto at Limit, its threshold down), Lenis (its depth up, a focus band
    // on) and Cedo (on the tempo, a drawn shape of three points).
    if upto >= 75 {
        use cantus_core::fx::cedo::Shape;
        use cantus_core::fx::lapsus::Point;
        let s = p.strips.get(1).map(|s| s.id).expect("a second strip");
        let mut vinculum = p.make_effect(EffectKind::BusCompressor);
        for (k, v) in [("model", 1.0), ("char", 1.0), ("thr", -20.0)] {
            vinculum.spec.set(k, v).unwrap();
        }
        let mut lenis = p.make_effect(EffectKind::ResonanceSuppressor);
        for (k, v) in [("depth", 0.5), ("b2on", 1.0), ("b2amt", 0.5)] {
            lenis.spec.set(k, v).unwrap();
        }
        let mut cedo = p.make_effect(EffectKind::Ducker);
        for (k, v) in [("trig", 1.0), ("depth", 9.0)] {
            cedo.spec.set(k, v).unwrap();
        }
        cedo.spec.cedo = Some(Box::new(Shape {
            points: vec![
                Point::new(0.0, 1.0),
                Point::new(0.4, 0.3).bend(0.5),
                Point::new(0.7, 0.0).shape(2),
            ],
        }));
        for e in [vinculum, lenis, cedo] {
            p.strip_mut(s).unwrap().effects.push(e);
        }
    }
    // fxc:contract — Schema 76 (the effects side round's wave C, ADRs 0142–0143; provisional
    // 66 on its branch): on the second strip a Purus
    // (the de-hum on, the de-reverb on with a far microphone's Direct sound) and a
    // Comparatio with a reference track by path, matched short-term, in a list of three.
    if upto >= 76 {
        let s = p.strips.get(1).map(|s| s.id).expect("a second strip");
        let mut purus = p.make_effect(EffectKind::Repair);
        purus.spec.set("hon", 1.0).unwrap();
        purus.spec.set("von", 1.0).unwrap();
        purus.spec.set("vdirect", 0.0).unwrap();
        let mut comparatio = p.make_effect(EffectKind::Reference);
        comparatio.spec.ir = Some(Box::new(cantus_core::IrFile::new(
            "references/Reference master.flac",
        )));
        comparatio.spec.set("match", 1.0).unwrap();
        // Phase B: its list of three references (the one compared among them, with the
        // size and hash it was loaded with) and the match EQ's amount.
        {
            use cantus_core::fx::comparatio::{Reference, References};
            comparatio.spec.refs = Some(Box::new(References {
                list: vec![
                    // Owner, 2026-11-17: each reference its own playback — this one looped
                    // over its chorus while another was compared.
                    Reference {
                        playback: Some(cantus_core::fx::comparatio::Playback {
                            sync: 2.0,
                            soff: 0.0,
                            lstart: 45.0,
                            lend: 75.0,
                        }),
                        ..Reference::new("references/Other master.wav")
                    },
                    Reference {
                        path: "references/Reference master.flac".into(),
                        bytes: 31_457_280,
                        hash: "9e3779b97f4a7c15".into(),
                        playback: None,
                    },
                    Reference::new("C:/Music/Album/03 Song.flac"),
                ],
                eq: None,
            }));
        }
        comparatio.spec.set("eqamt", 0.75).unwrap();
        for e in [purus, comparatio] {
            p.strip_mut(s).unwrap().effects.push(e);
        }
    }
    // fxd:contract — Schema 77 (the effects side round's wave D, ADRs 0144–0145; provisional
    // 67 on its branch): on the master an X-Y pad named "Sweep" — its
    // X on the master's pan as it is, its Y on the master's width from 0 to 60 % bent
    // (0.4), 250 ms of inertia, a four-point smooth path over two bars, playing — and a
    // formula (FL's adjustable sine) on the first strip's fader, unipolar, its knob b
    // automated by a clip on the first track from 0.2 to 0.8 over four beats.
    if upto >= 77 {
        use cantus_core::modulator::{FormulaSource, RouteLink, RouteRange, XyPath};
        let strip1 = p.tracks[0].strip.expect("the first track's strip");
        let mut pad = Modulator::new(ModulatorId(p.alloc_id()), ModKind::XyPad);
        pad.name = "Sweep".into();
        pad.routes = vec![
            ModRoute {
                target: AutoTarget::MasterPan,
                depth: 0.5,
            },
            ModRoute {
                target: AutoTarget::MasterWidth,
                depth: 1.0,
            },
        ];
        if let ModSource::XyPad(x) = &mut pad.source {
            x.x = 0.3;
            x.y = 0.7;
            x.inertia_ms = 250.0;
            x.path = Some(XyPath {
                points: vec![[0.1, 0.1], [0.9, 0.2], [0.8, 0.9], [0.2, 0.8]],
                beats: 8.0,
                smooth: true,
            });
            x.play_path = true;
            x.links = vec![RouteLink {
                target: AutoTarget::MasterWidth,
                output: 1,
                range: Some(RouteRange {
                    from: 0.0,
                    to: 0.6,
                    curve: 0.4,
                }),
            }];
        }
        p.master.modulators.push(pad);
        let mut formula = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Formula);
        formula.source = ModSource::Formula(FormulaSource {
            text: "a*Sin(SongTime*Pi*b)+c".into(),
            inputs: [0.5, 0.25, 0.5],
            bipolar: false,
            links: Vec::new(),
            look: 0,
        });
        formula.routes.push(ModRoute {
            target: AutoTarget::StripGain(strip1),
            depth: -0.2,
        });
        let fid = formula.id;
        p.master.modulators.push(formula);
        let knob = p.make_automation_clip(
            "Knob b",
            AutoTarget::ModInput {
                modulator: fid,
                input: 1,
            },
            Ticks::beats(12),
            Ticks::beats(4),
            vec![
                AutoPoint::new(Ticks::ZERO, 0.2),
                AutoPoint::new(Ticks::beats(4), 0.8),
            ],
        );
        p.tracks[0].clips.push(knob);
    }
    // r41:contract — Schema 78 (round 41, ADRs 0150–0152): a Fides track (the resonator, its
    // stub's gain at -3 dB) before the last track; on the first strip a Nitor with its key fixed
    // to G (8) and its output at -1.5 dB; on the second strip a board of Fragor's five circuits
    // as pedals of their own, in order. The builders keep this block in step with their tables.
    if upto >= 78 {
        use cantus_core::fx::tabula::{Board, Pedal, PedalId};
        let mut fides = p.make_track("Fides");
        let mut n = NativeSpec::new(InstrumentKind::Resonator);
        n.set("gain", -3.0).unwrap();
        fides.instrument = InstrumentSpec::Native(n);
        let at = p.tracks.len().saturating_sub(1);
        p.tracks.insert(at, fides);
        let s1 = p.strips.first().map(|s| s.id).expect("a first strip");
        let mut nitor = p.make_effect(EffectKind::Enhancer);
        nitor.spec.set("key", 8.0).unwrap();
        nitor.spec.set("out", -1.5).unwrap();
        p.strip_mut(s1).unwrap().effects.push(nitor);
        let s2 = p.strips.get(1).map(|s| s.id).expect("a second strip");
        let mut board = p.make_effect(EffectKind::Board);
        let kinds = [
            EffectKind::FragorOverdrive,
            EffectKind::FragorOpAmp,
            EffectKind::FragorSilicon,
            EffectKind::FragorFuzz,
            EffectKind::FragorMuff,
        ];
        board.spec.board = Some(Box::new(Board {
            pedals: kinds
                .iter()
                .enumerate()
                .map(|(i, k)| Pedal::new(PedalId(i as u16), EffectSpec::new(*k)))
                .collect(),
            macros: Vec::new(),
        }));
        p.strip_mut(s2).unwrap().effects.push(board);
    }
    // r41:fides-input — Schema 79 (ADR 0154): the Fides track hears the third strip as its
    // input (its sidechain), set as the command sets it — so the key is one with no loop.
    if upto >= 79 {
        let fides = p
            .tracks
            .iter()
            .find(|t| t.name == "Fides")
            .map(|t| t.id)
            .expect("the Fides track");
        let key = p.strips.get(2).map(|s| s.id).expect("a third strip");
        Command::SetTrackSidechain {
            track: fides,
            source: Some(key),
        }
        .apply(&mut p)
        .expect("a key with no loop");
    }
    // r42:contract — Schema 80 (ADR 0155): the project in Werckmeister III in E flat at 415 Hz;
    // its first track in equal temperament, its second in its own 31-EDO (a Scala scale).
    if upto >= 80 {
        use cantus_core::tuning::{
            ScalaScale, ScaleDegree, Temperament, TrackTuning, Tuning, TuningSystem,
        };
        Command::SetProjectTuning {
            tuning: Box::new(Tuning {
                a4_hz: 415.0,
                system: TuningSystem::Temperament {
                    id: Temperament::WerckmeisterIii,
                    key: 3,
                    follow_song_key: false,
                },
                map: None,
            }),
        }
        .apply(&mut p)
        .expect("a tuning");
        let ids: Vec<TrackId> = p.tracks.iter().map(|t| t.id).collect();
        Command::SetTrackTuning {
            track: ids[0],
            tuning: TrackTuning::Equal,
        }
        .apply(&mut p)
        .expect("equal");
        let edo31 = ScalaScale {
            description: "31 equal divisions of the octave".to_string(),
            degrees: (1..=31)
                .map(|i| ScaleDegree::Cents(1200.0 * f64::from(i) / 31.0))
                .collect(),
        };
        Command::SetTrackTuning {
            track: ids[1],
            tuning: TrackTuning::Own {
                tuning: Tuning {
                    system: TuningSystem::Scale(edo31),
                    ..Tuning::default()
                },
            },
        }
        .apply(&mut p)
        .expect("its own");
    }
    // r42:contract — Schema 81 (ADRs 0157, 0158): an Arcus track before the last; the first
    // clip's first note pizzicato, its second legato with a down bow.
    if upto >= 81 {
        use cantus_core::note_art::{Articulation, Bow, NoteArt};
        let mut arcus = p.make_track("Arcus");
        arcus.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::BowedString));
        let at = p.tracks.len().saturating_sub(1);
        p.tracks.insert(at, arcus);
        let track = p.tracks[0].id;
        let clip = p.tracks[0].clips[0].id;
        let notes: Vec<_> = p.tracks[0].clips[0].notes.iter().map(|n| n.id).collect();
        Command::SetNoteArt {
            track,
            clip,
            arts: vec![
                (
                    notes[0],
                    Some(NoteArt {
                        articulation: Articulation::Pizzicato,
                        bow: None,
                        start: None,
                        part: None,
                        string: None,
                    }),
                ),
                (
                    notes[1],
                    Some(NoteArt {
                        articulation: Articulation::Legato,
                        bow: Some(Bow::Down),
                        start: None,
                        part: None,
                        string: None,
                    }),
                ),
            ],
        }
        .apply(&mut p)
        .expect("two notes' articulation");
    }
    // r42:strings-rec — Schema 82 (ADR 0158): a recorded-strings Exemplar before the last
    // track: the scripted legato on (crossfade 90 ms, glide 40 ms, 120 ms past the attack),
    // two zones a sustain crossfaded by the mod wheel — piano down-bow, forte up-bow — and a
    // pizzicato zone.
    if upto >= 82 {
        use cantus_core::inst::sampler::{zf, Zone};
        let mut spec = NativeSpec::new(InstrumentKind::Sampler);
        for (k, v) in [
            ("legato", 1.0),
            ("legato.xf", 90.0),
            ("legato.glide", 40.0),
            ("legato.skip", 120.0),
        ] {
            spec.set(k, v);
        }
        let mut soft = Zone::new(Some("strings/cello_p_d.wav".into()), 36, 72, 48);
        soft.set(zf::ART, 1.0)
            .set(zf::BOW, 1.0)
            .set(zf::XFCC, 1.0)
            .set(zf::XFOUTLO, 0.0)
            .set(zf::XFOUTHI, 127.0);
        let mut loud = Zone::new(Some("strings/cello_f_u.wav".into()), 36, 72, 48);
        loud.set(zf::ART, 1.0)
            .set(zf::BOW, 2.0)
            .set(zf::XFCC, 1.0)
            .set(zf::XFINLO, 0.0)
            .set(zf::XFINHI, 127.0)
            .set(zf::XFCURVE, 1.0);
        let mut pizz = Zone::new(Some("strings/cello_pizz.wav".into()), 36, 72, 48);
        pizz.set(zf::ART, 5.0);
        for (n, z) in [soft, loud, pizz].into_iter().enumerate() {
            for (k, v) in z.values_for(n) {
                spec.set(&k, v);
            }
            for (k, text) in z.texts_for(n) {
                let _ = spec.set_text(&k, Some(text));
            }
        }
        let mut strings = p.make_track("Strings");
        strings.instrument = InstrumentSpec::Native(spec);
        let at = p.tracks.len().saturating_sub(1);
        p.tracks.insert(at, strings);
        // r42 fix2-model (review 14): the project now plays Ptolemy's intense diatonic (a
        // seven-note Scala scale) on the white keys at A415 — a keyboard map whose black
        // keys are unmapped (`x`), its formal octave degree 7, key 69 at 440 Hz.
        use cantus_core::tuning::{KeyboardMap, ScalaScale, ScaleDegree, Tuning, TuningSystem};
        let r = ScaleDegree::Ratio;
        Command::SetProjectTuning {
            tuning: Box::new(Tuning {
                a4_hz: 415.0,
                system: TuningSystem::Scale(ScalaScale {
                    description: "Ptolemy's intense diatonic".to_string(),
                    degrees: vec![
                        r(9, 8),
                        r(5, 4),
                        r(4, 3),
                        r(3, 2),
                        r(5, 3),
                        r(15, 8),
                        r(2, 1),
                    ],
                }),
                map: Some(KeyboardMap {
                    size: 12,
                    first_key: 0,
                    last_key: 127,
                    middle_key: 60,
                    reference_key: 69,
                    reference_hz: 440.0,
                    octave_degree: 7,
                    mapping: [
                        Some(0),
                        None,
                        Some(1),
                        None,
                        Some(2),
                        Some(3),
                        None,
                        Some(4),
                        None,
                        Some(5),
                        None,
                        Some(6),
                    ]
                    .to_vec(),
                }),
            }),
        }
        .apply(&mut p)
        .expect("a mapped scale");
    }
    // r42 arcus-expr — Schema 83 (ADR 0162): the first clip's second note (legato, down-bow)
    // lands from the air at the tip, on the highest string.
    if upto >= 83 {
        use cantus_core::note_art::{Articulation, Bow, BowPart, BowStart, NoteArt};
        let track = p.tracks[0].id;
        let clip = p.tracks[0].clips[0].id;
        let note = p.tracks[0].clips[0].notes[1].id;
        Command::SetNoteArt {
            track,
            clip,
            arts: vec![(
                note,
                Some(NoteArt {
                    articulation: Articulation::Legato,
                    bow: Some(Bow::Down),
                    start: Some(BowStart::Air),
                    part: Some(BowPart::Tip),
                    string: Some(1),
                }),
            )],
        }
        .apply(&mut p)
        .expect("the note's bow");
    }
    // r42:chorda-c17 — Schema 84 (ADR 0098's sixth amendment): the Chorda's colour moved
    // most of the way toward a recorded grand's (0.6).
    if upto >= 84 {
        if let Some(n) = p
            .tracks
            .iter_mut()
            .find(|t| t.name == "Chorda")
            .and_then(|t| t.instrument.native_mut())
        {
            n.set("colour", 0.6);
        }
    }
    // r42 fiddle — Schema 85 (ADR 0164): the Arcus track's strings under one bow, in
    // sympathy, gritty, steel, its double stops tuned most of the way to just.
    if upto >= 85 {
        let arcus = p
            .tracks
            .iter_mut()
            .find(|t| t.name == "Arcus")
            .expect("the Arcus track");
        if let InstrumentSpec::Native(spec) = &mut arcus.instrument {
            for (k, v) in [
                ("bow.pair", 1.0),
                ("bow.tilt", 0.6),
                ("dstop.just", 0.6),
                ("bow.grit", 1.0),
                ("bow.bite", 0.7),
                ("sym", 1.2),
                ("steel", 1.0),
                ("tuning.strings", 1.0),
                ("body.mic", 1.0),
                ("body.bridge", 1.0),
            ] {
                spec.set(k, v);
            }
        }
    }
    // r42 arcus-classical — Schema 87 (ADR 0166): the Arcus track's player a fiddler by its own
    // switch, at a share its steel set (1) would not give it.
    if upto >= 87 {
        if let Some(n) = p
            .tracks
            .iter_mut()
            .find(|t| t.name == "Arcus")
            .and_then(|t| t.instrument.native_mut())
        {
            n.set("player.fiddle", 0.8);
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
