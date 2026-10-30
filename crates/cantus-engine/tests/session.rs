//! The session in the engine (ADR 0076 §7): a performance is played as its projection — the
//! song with what the session plays written into the arrangement — so it is sample-accurate,
//! block-size independent and parallel-equal by construction, a song whose session never
//! plays sounds as it did to the bit, a live performance (projections installed as the
//! conductor makes them, while playing) sounds as its offline render, and the arrangement a
//! performance is recorded into sounds as the performance did.

use cantus_core::session::play::{plan, project_play, Input, Timed};
use cantus_core::session::*;
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

const SR: f64 = 48_000.0;
const BAR: Ticks = Ticks::beats(4);

/// Keys and bass on Firmus; a session of two scenes — the keys' first clip loops a bar with
/// follow actions (next or any) and an envelope of its track's level; their second is
/// legato; the bass loops two bars; the second scene sets 132 BPM.
fn song() -> Project {
    let mut p = Project::new("live");
    let keys = p.make_track("Keys");
    let bass = p.make_track("Bass");
    let (k, b) = (keys.id, bass.id);
    p.tracks.push(keys);
    p.tracks.push(bass);
    // Something in the arrangement under the session, to be replaced and handed back.
    let mut arr = p.make_clip("Arr", Ticks::ZERO, BAR * 12);
    for i in 0..48 {
        let n = p.make_note(Ticks::beats(i), Ticks::QUARTER, 48 + (i % 5) as u8, 70);
        arr.notes.push(n);
    }
    p.tracks[0].clips.push(arr);
    let scenes: Vec<SceneId> = (0..2).map(|_| SceneId(p.alloc_id())).collect();
    for (i, s) in scenes.iter().enumerate() {
        let mut sc = Scene::new(*s, format!("S{i}"));
        if i == 1 {
            sc.tempo = Some(132.0);
        }
        p.session.scenes.push(sc);
    }
    p.session.settings.seed = 33;
    let clip_of = |p: &mut Project, bars: i64, base: u8| {
        let mut c = p.make_clip("c", Ticks::ZERO, BAR * bars);
        c.content_length = Some(BAR * bars);
        for i in 0..bars * 8 {
            let n = p.make_note(Ticks(i * 480), Ticks(400), base + (i % 7) as u8, 100);
            c.notes.push(n);
        }
        SessionClip::new(c)
    };
    let mut first = clip_of(&mut p, 1, 60);
    first.follow = FollowActions {
        on: true,
        a: FollowAction::Next,
        b: FollowAction::Any,
        chance_a: 60,
        after: FollowAfter::Linked,
        count: 2,
    };
    first.envelopes.push(Automation::new(
        AutoTarget::TrackGain(k),
        vec![AutoPoint::new(Ticks::ZERO, 0.4), AutoPoint::new(BAR, 0.9)],
    ));
    let mut second = clip_of(&mut p, 1, 67);
    second.launch.legato = true;
    second.launch.quantize = Quantize::Quarter;
    let low = clip_of(&mut p, 2, 36);
    p.tracks[0].slots = vec![
        Slot {
            scene: scenes[0],
            clip: Some(first),
            stop: true,
        },
        Slot {
            scene: scenes[1],
            clip: Some(second),
            stop: true,
        },
    ];
    p.tracks[1].slots = vec![Slot {
        scene: scenes[0],
        clip: Some(low),
        stop: true,
    }];
    let _ = b;
    p
}

fn scenes(p: &Project) -> Vec<SceneId> {
    p.session.scenes.iter().map(|s| s.id).collect()
}

/// The performance: scene 0 (just after the start), the keys' legato clip in bar 4, the
/// bass stopped in bar 6, scene 1 (its tempo) in bar 7, everything back in bar 10.
fn presses(p: &Project) -> Vec<Timed> {
    let s = scenes(p);
    let (k, b) = (p.tracks[0].id, p.tracks[1].id);
    let at = |t: Ticks, input| Timed::at(t, input);
    vec![
        at(
            Ticks(10),
            Input::LaunchScene {
                scene: s[0],
                velocity: 1.0,
            },
        ),
        at(
            BAR * 3 + Ticks(700),
            Input::Launch {
                slot: SlotRef::new(k, s[1]),
                velocity: 1.0,
            },
        ),
        at(BAR * 5 + Ticks(3), Input::StopTrack { track: b }),
        at(
            BAR * 6 + Ticks(1),
            Input::LaunchScene {
                scene: s[1],
                velocity: 1.0,
            },
        ),
        at(BAR * 9 + Ticks(5), Input::Back { track: None }),
    ]
}

const END: Ticks = Ticks(BAR.0 * 12);

fn offline(p: &Project, block: usize) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let opts = RenderOptions {
        block_frames: block,
        tail_seconds: 0.0,
        end: Some(END),
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// The project a performance is committed into (arrangement record).
fn committed(p: &Project) -> Project {
    let perf = plan(p, &presses(p), END).perf;
    let mut q = p.clone();
    edit::commit_performance(p, &perf)
        .unwrap()
        .apply(&mut q)
        .unwrap();
    q
}

/// A processor playing `snap` from the start, as the application sets one up.
struct Rig {
    p: Processor,
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    tick: Ticks,
}

impl Rig {
    fn new(snap: Arc<Snapshot>, workers: Workers) -> Rig {
        let (tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
        let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
        let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
        p.set_workers(workers);
        let mut r = Rig {
            p,
            tx,
            rx,
            tick: Ticks::ZERO,
        };
        r.install(snap);
        r.p.settle();
        r.tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
        r.tx.push(EngineMessage::Play).ok();
        r
    }

    fn install(&mut self, snap: Arc<Snapshot>) {
        self.tx
            .push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
            .ok();
        let plan = cantus_engine::graph::compile(&snap);
        self.tx
            .push(EngineMessage::SetSnapshotPlanned(snap, plan))
            .ok();
    }

    fn block(&mut self, n: usize, out: &mut Vec<f32>) {
        let mut buf = vec![0f32; n * 2];
        self.p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while let Ok(ev) = self.rx.pop() {
            if let EngineEvent::Position { tick, .. } = ev {
                self.tick = tick;
            }
        }
    }
}

/// The performance played live: every UI frame (512 samples) the conductor plans what it
/// knows — the presses made by then, each a lead ahead of its tick — to a horizon eight bars
/// on, and installs the projection whenever it changed; blocks of `block` frames.
fn live(p: &Project, block: usize, workers: Workers, frames: usize) -> Vec<f32> {
    let all = presses(p);
    let lead = Ticks(480);
    let mut sent = plan(p, &[], BAR * 8).perf;
    let mut rig = Rig::new(
        Arc::new(Snapshot::compile(&project_play(p, &sent), 1)),
        workers,
    );
    let mut out = Vec::with_capacity(frames * 2);
    let mut since_frame = 0usize;
    while out.len() < frames * 2 {
        let n = block.min(frames - out.len() / 2);
        rig.block(n, &mut out);
        since_frame += n;
        if since_frame >= 512 {
            since_frame = 0;
            let now = rig.tick;
            let known: Vec<Timed> = all.iter().copied().filter(|t| t.at <= now + lead).collect();
            let perf = plan(p, &known, now + BAR * 8).perf;
            if perf != sent {
                rig.install(Arc::new(Snapshot::compile(&project_play(p, &perf), 2)));
                sent = perf;
            }
        }
    }
    out
}

#[test]
fn a_session_that_never_plays_sounds_as_the_song_without_it() {
    let p = song();
    let mut bare = p.clone();
    bare.session = Default::default();
    for t in &mut bare.tracks {
        t.slots.clear();
    }
    assert!(same(&offline(&p, 256), &offline(&bare, 256)));
    // Nothing pressed: the projection is the song itself.
    assert_eq!(project_play(&p, &plan(&p, &[], END).perf), p);
}

#[test]
fn a_launch_lands_on_its_boundary_as_an_arrangement_note_would() {
    let mut p = song();
    p.tracks[0].clips.clear();
    let s = scenes(&p);
    let slot = SlotRef::new(p.tracks[0].id, s[0]);
    let inputs = [Timed {
        at: Ticks(10),
        now: false,
        pressed: Ticks(10),
        input: Input::Launch {
            slot,
            velocity: 1.0,
        },
    }];
    let perf = plan(&p, &inputs, BAR * 2).perf;
    assert_eq!(perf.played[0].at, BAR);
    let heard = offline(&project_play(&p, &perf), 64);
    // The same notes laid by hand in the arrangement from bar 2.
    let mut by_hand = p.clone();
    let mut c = by_hand.slot_clip(slot).unwrap().clip.clone();
    c.start = BAR;
    c.length = BAR;
    c.content_length = Some(BAR);
    by_hand.tracks[0].clips.push(c);
    by_hand.tracks[0].slots.clear();
    let mut hand = offline(&by_hand, 64);
    hand.truncate(heard.len());
    let b = (2.0 * SR) as usize * 2; // bar 2 at 120 BPM, interleaved
    assert!(
        heard[..b].iter().all(|x| *x == 0.0),
        "nothing before the boundary"
    );
    assert!(heard[b..b + 4800].iter().any(|x| *x != 0.0));
    let head = 4 * SR as usize; // up to bar 3, where the hand's one bar ends
    assert!(same(&heard[..head], &hand[..head]), "to the sample");
}

#[test]
fn a_recorded_performance_renders_as_it_was_heard_live() {
    let p = song();
    let frames = (offline(&committed(&p), 512).len() / 2).min((24.0 * SR) as usize);
    let recorded = offline(&committed(&p), 512);
    let heard = live(&p, 128, Workers::SERIAL, frames);
    assert!(
        same(&heard, &recorded[..frames * 2]),
        "live and recorded differ at {:?}",
        heard
            .iter()
            .zip(&recorded)
            .position(|(a, b)| a.to_bits() != b.to_bits())
    );
    // At another block size, and with the worker pool.
    let other = live(&p, 441, Workers::SERIAL, frames);
    assert!(
        same(&other, &heard),
        "441 differs at {:?}",
        other
            .iter()
            .zip(&heard)
            .position(|(a, b)| a.to_bits() != b.to_bits())
            .map(|i| (i / 2, i as f64 / 2.0 / SR))
    );
    assert!(same(&live(&p, 333, Workers::parallel(3), frames), &heard));
    // Offline, at two block sizes.
    assert!(same(&offline(&committed(&p), 97), &recorded));
    // It did play: follow actions, the legato clip, the scene's tempo.
    let plan = plan(&p, &presses(&p), END);
    assert!(plan.perf.played.len() >= 5);
    assert_eq!(plan.perf.tempos.len(), 1);
}

/// D2: a snapshot that ends, behind the playhead, a note the playing one still had sounding
/// (a projection that came late, an edit) ends it at once — it does not ring on with its
/// note-off passed.
#[test]
fn a_note_cut_behind_the_playhead_is_ended() {
    let mut p = Project::new("cut");
    let t = p.make_track("Keys");
    p.tracks.push(t);
    let mut c = p.make_clip("long", Ticks::ZERO, BAR * 4);
    let n = p.make_note(Ticks::ZERO, BAR * 4, 60, 110);
    c.notes.push(n);
    p.tracks[0].clips.push(c);
    let old = Arc::new(Snapshot::compile(&p, 1));
    let mut rig = Rig::new(old.clone(), Workers::SERIAL);
    let mut out = Vec::new();
    for _ in 0..(SR as usize / 256) {
        rig.block(256, &mut out); // a second in: beat 2
    }
    // The note now ends at beat 1, a beat behind the playhead; the cut is worked out where
    // the snapshot is sent (`EngineHandle::compile` while playing), against the one playing.
    p.tracks[0].clips[0].length = Ticks::beats(1);
    let mut new = Snapshot::compile(&p, 2);
    new.cut_from(&old, rig.tick);
    assert_eq!(new.cuts.len(), 1);
    rig.install(Arc::new(new));
    out.clear();
    for _ in 0..(2 * SR as usize / 256) {
        rig.block(256, &mut out);
    }
    let tail = &out[out.len() - 2048..];
    assert!(
        tail.iter().all(|x| *x == 0.0),
        "the note rings on: {}",
        tail[0]
    );
}

/// Review finding 11: settling with a snapshot waiting for the grid (playing, off it) and a
/// message after it ends.
#[test]
fn settle_ends_with_a_snapshot_waiting_for_the_grid() {
    let p = song();
    let mut rig = Rig::new(Arc::new(Snapshot::compile(&p, 1)), Workers::SERIAL);
    let mut out = Vec::new();
    rig.block(100, &mut out); // off the grid
    rig.install(Arc::new(Snapshot::compile(&p, 2)));
    rig.tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let mut p_ = rig.p;
    let h = std::thread::spawn(move || {
        p_.settle();
        done_tx.send(()).ok();
        p_
    });
    assert!(
        done_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .is_ok(),
        "settle spun"
    );
    let _ = h.join();
}

/// A WAV of `seconds` of a 220 Hz tone, mono, at 48 kHz.
fn tone_file(dir: &std::path::Path, seconds: f64) -> std::path::PathBuf {
    let path = dir.join("loop.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    let n = (seconds * SR) as usize;
    for i in 0..n {
        let x = cantus_dsp::math::sin(i as f32 * 220.0 * std::f32::consts::TAU / 48_000.0) * 0.3;
        w.write_sample(x).unwrap();
    }
    w.finalize().unwrap();
    path
}

/// A third track whose session holds an unwarped audio loop of two bars (scene 0).
fn with_audio(mut p: Project, path: &std::path::Path) -> Project {
    let sid = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: sid,
        name: "loop".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: (12.0 * SR) as u64,
        sha256: None,
        recorded: false,
    });
    let t = p.make_track("Loop");
    p.tracks.push(t);
    let mut c = p.make_audio_clip("loop", sid, Ticks::ZERO, BAR * 2);
    c.content_length = Some(BAR * 2);
    let scene = p.session.scenes[0].id;
    let ti = p.tracks.len() - 1;
    p.tracks[ti].slots.push(Slot {
        scene,
        clip: Some(SessionClip::new(c)),
        stop: true,
    });
    p
}

/// A WAV of `seconds` of decaying noise bursts, mono, at 48 kHz (a sampled hit or a loop).
fn hits_file(path: &std::path::Path, seconds: f64, every: usize, seed: u32) -> String {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let mut z = seed | 1;
    for i in 0..(seconds * SR) as usize {
        z ^= z << 13;
        z ^= z >> 17;
        z ^= z << 5;
        let t = (i % every) as f32 / 48_000.0;
        let n = z as f32 / u32::MAX as f32 - 0.5;
        w.write_sample(n * 0.5 * cantus_dsp::math::exp(-t * 30.0))
            .unwrap();
    }
    w.finalize().unwrap();
    path.to_string_lossy().replace('\\', "/")
}

/// The second round's gaps: a Pulsus track whose session clip plays a snare of three
/// round-robin layers (in turn) and a shaker of four (at random), and a 100 BPM loop warped
/// to follow the song in both scenes (heard at 120 and, under the second scene, at 132).
fn with_kit_and_warp(mut p: Project, dir: &std::path::Path) -> Project {
    let scenes: Vec<SceneId> = p.session.scenes.iter().map(|s| s.id).collect();
    // The kit.
    let mut t = p.make_track("Kit");
    let mut kit = PulsusParams::studio();
    let snare = kit.index_of_pitch(38).unwrap();
    kit.pads[snare].layers = (0..3)
        .map(|i| {
            let f = dir.join(format!("snare{i}.wav"));
            PadLayer::new(hits_file(&f, 0.2, 9_600, 11 + i))
        })
        .collect();
    kit.pads[snare].round_robin = RoundRobin::NoRepeat;
    let mut shaker = DrumPad::new(70, "Shaker", PadVoice::Sample);
    shaker.layers = (0..4)
        .map(|i| {
            let f = dir.join(format!("shaker{i}.wav"));
            PadLayer::new(hits_file(&f, 0.1, 4_800, 31 + i))
        })
        .collect();
    shaker.round_robin = RoundRobin::Random;
    shaker.id = kit.next_id();
    kit.pads.push(shaker);
    t.instrument = InstrumentSpec::Pulsus(kit);
    let mut c = p.make_clip("Groove", Ticks::ZERO, BAR);
    c.content_length = Some(BAR);
    for s in 0..8 {
        let at = Ticks::QUARTER / 2 * s;
        c.notes
            .push(p.make_note(at, Ticks::SIXTEENTH, 70, 60 + 7 * s as u8));
        if s % 2 == 1 {
            c.notes.push(p.make_note(at, Ticks::SIXTEENTH, 38, 100));
        }
    }
    t.slots.push(Slot {
        scene: scenes[0],
        clip: Some(SessionClip::new(c)),
        stop: true,
    });
    p.tracks.push(t);
    // The warped loop: two bars at 100 BPM.
    let path = dir.join("loop 100.wav");
    let path = hits_file(&path, 4.8, 12_000, 77);
    let sid = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: sid,
        name: "loop 100".into(),
        path,
        sample_rate: 48_000,
        channels: 1,
        frames: (4.8 * SR) as u64,
        sha256: None,
        recorded: false,
    });
    let mut t = p.make_track("Warped");
    for scene in &scenes {
        let mut c = p.make_audio_clip("loop", sid, Ticks::ZERO, BAR * 2);
        c.audio.as_mut().unwrap().warp = Some(Box::new(cantus_core::warp::Warp::identity(
            48_000, 100.0, 0,
        )));
        c.content_length = Some(BAR * 2);
        t.slots.push(Slot {
            scene: *scene,
            clip: Some(SessionClip::new(c)),
            stop: true,
        });
    }
    p.tracks.push(t);
    p
}

/// Review finding 9: the session's audio is loaded before it is launched — with the
/// background loader, the first projection that plays it has its data.
#[test]
fn session_audio_is_loaded_before_it_is_launched() {
    let dir = tempfile::tempdir().unwrap();
    let p = with_audio(song(), &tone_file(dir.path(), 12.0));
    let mut h = EngineHandle::dummy();
    h.samples.set_background(true);
    h.compile(&p, 1);
    let t0 = std::time::Instant::now();
    while h.samples.loading() && t0.elapsed() < std::time::Duration::from_secs(10) {
        h.samples.poll_loaded();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let slot = SlotRef::new(p.tracks[2].id, scenes(&p)[0]);
    let perf = plan(
        &p,
        &[Timed::at(
            Ticks(10),
            Input::Launch {
                slot,
                velocity: 1.0,
            },
        )],
        BAR * 4,
    )
    .perf;
    let snap = h.compile(&project_play(&p, &perf), 2);
    let audio = &snap.tracks[2].audio;
    assert!(!audio.is_empty());
    assert!(
        audio.iter().all(|c| c.data.is_some()),
        "it plays from its first projection"
    );
}

/// The review's richer equality: an unwarped audio loop stopped mid-loop (its cut fade),
/// note effects (an arpeggiator), a modulator, a scene's tempo heard through the audio,
/// and the conductor's own lead at 128 frames — live at two block sizes and with the pool
/// equals its recorded arrangement, bit for bit.
#[test]
fn a_rich_performance_renders_as_heard() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = with_audio(song(), &tone_file(dir.path(), 12.0));
    let keys = p.tracks[0].id;
    p.tracks[0].note_fx = vec![cantus_core::notefx::NoteEffect::new(
        cantus_core::notefx::NoteFxId(p.alloc_id()),
        cantus_core::notefx::NoteFxType::Arp,
    )];
    let mut m = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
    m.routes = vec![ModRoute {
        target: AutoTarget::TrackGain(p.tracks[1].id),
        depth: 0.4,
    }];
    p.tracks[1].modulators.push(m);
    let loop_track = p.tracks[2].id;
    // A round-robin kit and a warped loop (the second round).
    let p = with_kit_and_warp(p, dir.path());
    let warped = p.tracks.last().unwrap().id;
    let s = scenes(&p);
    let presses = vec![
        Timed::at(
            Ticks(10),
            Input::LaunchScene {
                scene: s[0],
                velocity: 1.0,
            },
        ),
        Timed::at(BAR * 3 + Ticks(500), Input::StopTrack { track: loop_track }),
        Timed::at(
            BAR * 4 + Ticks(1),
            Input::LaunchScene {
                scene: s[1],
                velocity: 1.0,
            },
        ),
        Timed::at(BAR * 7 + Ticks(5), Input::Back { track: Some(keys) }),
        // The warped loop stops on a pass's end (bar 8), so its windows are the commit's.
        Timed::at(BAR * 7 + Ticks(9), Input::StopTrack { track: warped }),
    ];
    let end = BAR * 9;
    let mut h = EngineHandle::dummy();
    let committed = {
        let perf = plan(&p, &presses, end).perf;
        let mut q = p.clone();
        edit::commit_performance(&p, &perf)
            .unwrap()
            .apply(&mut q)
            .unwrap();
        q
    };
    // Every compile waits for its warp renders (`compile_for_render`): what a window plays
    // is then its render, live and recorded alike — what the cache has made by when is the
    // cache's business (N5's own test), not the projection's.
    let rec_snap = h.compile_for_render(&committed, 1);
    let opts = RenderOptions {
        block_frames: 512,
        tail_seconds: 0.0,
        end: Some(end),
        ..Default::default()
    };
    let recorded = render(rec_snap, &opts).unwrap().samples;
    let frames = recorded.len() / 2;
    // The conductor's lead at the run's block (132 BPM, the fastest heard: the most
    // ticks); a frame every block, as a press is planned and sent at once; the horizon
    // carried on as the conductor carries it, in jumps (a window cut at a horizon that crept
    // every block would be a new warp render every block).
    let run = |block: usize, workers: Workers, h: &mut EngineHandle| -> Vec<f32> {
        let lead = Ticks::from_seconds(cantus_ui_lead(block), 132.0);
        let mut horizon = BAR * 8;
        let mut sent = plan(&p, &[], horizon).perf;
        let mut rig = Rig::new(h.compile_for_render(&project_play(&p, &sent), 1), workers);
        let mut out = Vec::with_capacity(frames * 2);
        let mut since = 0usize;
        while out.len() < frames * 2 {
            let n = block.min(frames - out.len() / 2);
            rig.block(n, &mut out);
            since += n;
            if since >= block {
                since = 0;
                let now = rig.tick;
                let known: Vec<Timed> = presses
                    .iter()
                    .copied()
                    .filter(|t| t.at <= now + lead)
                    .collect();
                if now + BAR * 4 > horizon {
                    horizon = now + BAR * 8;
                }
                let perf = plan(&p, &known, horizon).perf;
                if perf != sent {
                    rig.install(h.compile_for_render(&project_play(&p, &perf), 2));
                    sent = perf;
                }
            }
        }
        out
    };
    let heard = run(128, Workers::SERIAL, &mut h);
    let first = heard
        .iter()
        .zip(&recorded)
        .position(|(a, b)| a.to_bits() != b.to_bits());
    assert!(
        first.is_none(),
        "live and recorded differ at frame {:?}",
        first.map(|i| i / 2)
    );
    assert!(heard.iter().any(|x| *x != 0.0));
    // The kit and the warped loop did play: their windows are in the commit.
    for name in ["Kit", "Warped"] {
        let t = committed.tracks.iter().find(|t| t.name == name).unwrap();
        assert!(!t.clips.is_empty(), "{name} played");
    }
    assert!(same(&run(441, Workers::SERIAL, &mut h), &heard));
    assert!(same(&run(333, Workers::parallel(3), &mut h), &heard));
}

/// The conductor's lead at `block` frames with a 3 ms compile (`cantus_ui::session_play::lead_s`).
fn cantus_ui_lead(block: usize) -> f64 {
    2.0 * block as f64 / SR + 0.003 + 0.002
}

/// N3: an install while playing with 32 dense drum-like tracks — chords of four on every
/// 32nd, each a beat long, every one shortened by the edit — ends the notes it cut behind
/// the playhead in a walk of a list worked out where it was sent: the install costs its
/// block well under a millisecond more than a block without one (the quadratic scan it
/// replaces cost 6.8 ms here, inside the callback). Robust to a loaded machine as the
/// other CPU tests are (round 23): nine measured pairs — the blocks just before an install
/// and the install's — and the median of their differences.
#[test]
fn an_install_cutting_32_dense_tracks_is_quick() {
    let make = |len: Ticks| {
        let mut p = Project::new("dense");
        for k in 0..32 {
            let t = p.make_track(format!("T{k}"));
            p.tracks.push(t);
            let mut c = p.make_clip("hits", Ticks::ZERO, BAR * 8);
            for i in 0..(8 * 4 * 8) {
                for j in 0..4u8 {
                    let n = p.make_note(Ticks(i * PPQ / 8), len, 36 + j * 3, 100);
                    c.notes.push(n);
                }
            }
            p.tracks[k].clips.push(c);
        }
        p
    };
    let p = make(Ticks::QUARTER);
    // The same notes (the same ids: the project is made the same way), half as long.
    let q = make(Ticks::QUARTER / 2);
    let mut diffs = Vec::new();
    let (mut plain_best, mut install_best) = (f64::MAX, f64::MAX);
    for _ in 0..9 {
        let old = Arc::new(Snapshot::compile(&p, 1));
        let mut rig = Rig::new(old.clone(), Workers::SERIAL);
        let mut out = Vec::new();
        // Well into the clip: sixteen beats of notes behind the playhead.
        rig.tx.push(EngineMessage::Seek(Ticks::beats(18))).ok();
        for _ in 0..40 {
            rig.block(256, &mut out);
        }
        let mut plain = f64::MAX;
        for _ in 0..4 {
            let t0 = std::time::Instant::now();
            rig.block(64, &mut out);
            plain = plain.min(t0.elapsed().as_secs_f64());
        }
        let mut new = Snapshot::compile(&q, 2);
        new.cut_from(&old, rig.tick);
        assert!(new.cuts.len() > 32 * 16, "{}", new.cuts.len());
        rig.install(Arc::new(new));
        let t0 = std::time::Instant::now();
        rig.block(64, &mut out);
        let install = t0.elapsed().as_secs_f64();
        diffs.push(install - plain);
        plain_best = plain_best.min(plain);
        install_best = install_best.min(install);
    }
    diffs.sort_by(f64::total_cmp);
    let median = diffs[diffs.len() / 2];
    eprintln!(
        "a 64-frame block: {:.3} ms; with an install cutting 32 dense tracks: {:.3} ms; \
         the median difference of nine pairs: {:.3} ms",
        plain_best * 1000.0,
        install_best * 1000.0,
        median * 1000.0
    );
    assert!(median < 0.001, "{:.3} ms", median * 1000.0);
}
