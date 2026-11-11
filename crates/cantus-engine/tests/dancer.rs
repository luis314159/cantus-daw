//! The dancer's unit (round 35, ADR 0087): an instrument that makes no sound.
//!
//! **Null tests first.** A project with a dancer — notes in its move map, automation of
//! `move`, an LFO on its `hue`, a follower on its `level` — renders bit-identical to the
//! same project without it, at every block size, offline and in real time, serial and on
//! the pool. Then what it tells the screen: a frame at every segment's end, the same at
//! any block size (the frames that end on the same sample agree), automation landing on
//! the bars it was drawn, notes and held keys, the offline timeline a video samples.

mod support;
use cantus_core::inst::dancer::{self, Move};
use cantus_core::modulator::*;
use cantus_core::*;
use cantus_engine::dance::DanceFrame;
use cantus_engine::*;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::*;

const SR: f64 = 48_000.0;

/// The demo (lead and bass, 128 BPM) with a dancer: notes Clap (beat 1), Jump (beat 3)
/// and Spin (beat 5, held two beats) and a key outside the map; `move` automated to Wave
/// over bar 2 (beats 4…8) and back to the routine; an LFO on `hue`, a follower listening
/// to the lead's strip on `level`, `react` at half.
fn with_dancer(base: &Project) -> (Project, TrackId) {
    let mut p = base.clone();
    let lead_strip = p.tracks[0].strip.unwrap();
    let mut t = p.make_track("Dancer");
    let id = t.id;
    let mut spec = NativeSpec::new(InstrumentKind::Dancer);
    spec.set("react", 0.5).unwrap();
    t.instrument = InstrumentSpec::Native(spec);
    let mut c = p.make_clip("Moves", Ticks::ZERO, Ticks::beats(8));
    let key = |m: Move| dancer::BASE_KEY + m.index() as u8;
    for (beat, k, len) in [
        (1, key(Move::Clap), Ticks::QUARTER),
        (3, key(Move::Jump), Ticks::QUARTER),
        (5, key(Move::Spin), Ticks::beats(2)),
        (6, 100, Ticks::QUARTER),
    ] {
        c.notes.push(p.make_note(Ticks::beats(beat), len, k, 90));
    }
    t.clips.push(c);
    let wave = dancer::move_value(Move::Wave) / 16.0;
    let auto = p.make_automation_clip(
        "Move",
        AutoTarget::Instrument {
            track: id,
            key: "move".into(),
        },
        Ticks::beats(4),
        Ticks::beats(4),
        vec![AutoPoint {
            tick: Ticks::ZERO,
            value: wave,
            tension: 0.0,
            shape: Shape::Hold,
        }],
    );
    t.clips.push(auto);
    p.tracks.push(t);
    let target = |key: &str| AutoTarget::Instrument {
        track: id,
        key: key.into(),
    };
    let mut lfo = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
    lfo.routes = vec![ModRoute {
        target: target("hue"),
        depth: 0.5,
    }];
    let mut follow = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Follower);
    if let ModSource::Follower(f) = &mut follow.source {
        f.listen = Listen::Strip(lead_strip);
    }
    follow.routes = vec![ModRoute {
        target: target("level"),
        depth: 1.0,
    }];
    p.modulators_mut(ModHost::Track(id))
        .unwrap()
        .extend([lfo, follow]);
    (p, id)
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i} differs: {} vs {}", a[i], b[i]);
    }
}

/// A render with its dancers' timelines.
fn render_frames(p: &Project, block: usize) -> (Vec<f32>, Vec<(TrackId, Vec<DanceFrame>)>) {
    let opts = RenderOptions {
        tail_seconds: 0.25,
        block_frames: block,
        ..Default::default()
    };
    let progress = RenderProgress::default();
    progress.dance.want_timelines();
    let r = render_with_progress(
        Arc::new(Snapshot::compile(p, 1)),
        &opts,
        Vec::new(),
        Vec::new(),
        &progress,
    )
    .unwrap();
    (r.samples, progress.dance.timelines())
}

#[test]
fn a_dancer_renders_bit_identical_to_the_project_without_it() {
    let base = demo_project();
    let (with, _) = with_dancer(&base);
    for block in [1usize, 64, 256, 1000] {
        let a = render_with(&base, block);
        let b = render_with(&with, block);
        assert!(a.iter().any(|x| *x != 0.0), "the demo sounds");
        same_bits(&a, &b, &format!("block {block}"));
    }
    // In real time too (irregular blocks, the units sent as the backend sends them).
    let len = (8.5 * 60.0 / 128.0 * SR) as usize * 2;
    same_bits(
        &modfx_realtime(&base, len),
        &modfx_realtime(&with, len),
        "real time",
    );
}

/// `support::modfx::realtime`, with the instruments' units sent first.
fn modfx_realtime(project: &Project, len: usize) -> Vec<f32> {
    let mut r = Rig::new(project, Workers::SERIAL);
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    while out.len() < len {
        let n = sizes[i % sizes.len()].min((len - out.len()) / 2);
        i += 1;
        out.extend(r.play(&[n]));
    }
    out
}

/// A processor playing a project as the application drives it (`tests/pool.rs`'s rig),
/// keeping the dancers' feeds.
struct Rig {
    p: Processor,
    rx: rtrb::Consumer<EngineEvent>,
    feeds: Vec<(TrackId, Arc<cantus_engine::dance::DanceFeed>)>,
}

impl Rig {
    fn new(project: &Project, workers: Workers) -> Rig {
        let snap = Arc::new(Snapshot::compile(project, 1));
        let effects = snap.effect_slots().count();
        let instruments = cantus_engine::inst::units_for(&snap, SR as f32, |_, _| false);
        let feeds = cantus_engine::dance::feeds_of(&instruments);
        let (mut tx, rx_engine) =
            rtrb::RingBuffer::<EngineMessage>::new(64 + effects + instruments.len());
        let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
        let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
        p.set_workers(workers);
        for slot in snap.effect_slots() {
            tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
                slot, SR as f32,
            ))))
            .ok();
        }
        for unit in instruments {
            tx.push(EngineMessage::AddInstrument(unit)).ok();
        }
        tx.push(EngineMessage::Automation(AutomationRuntime::build(&snap)))
            .ok();
        let plan = cantus_engine::graph::compile(&snap);
        tx.push(EngineMessage::SetSnapshotPlanned(snap, plan)).ok();
        p.settle();
        while let Ok(ev) = rx.pop() {
            if let EngineEvent::EffectNeed { effect, need } = ev {
                let memory = EffectMemory::build(need.need(), SR as f32);
                tx.push(EngineMessage::EffectMemory { effect, memory }).ok();
                p.settle();
            }
        }
        tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
        tx.push(EngineMessage::Play).ok();
        Rig { p, rx, feeds }
    }

    fn play(&mut self, blocks: &[usize]) -> Vec<f32> {
        let mut out = Vec::new();
        let mut buf = vec![0f32; MAX_BLOCK_FRAMES * 2];
        for &n in blocks {
            let b = &mut buf[..n * 2];
            self.p.process(b, 2);
            out.extend_from_slice(b);
            while self.rx.pop().is_ok() {}
        }
        out
    }
}

#[test]
fn the_pool_gives_the_serial_bits_with_a_dancer_and_the_same_frames() {
    let (p, _) = with_dancer(&demo_project());
    let blocks: Vec<usize> = (0..400).map(|i| [128, 64, 256, 17][i % 4]).collect();
    let mut serial = Rig::new(&p, Workers::SERIAL);
    let reference = serial.play(&blocks);
    let last = serial.feeds[0].1.latest().unwrap();
    for w in [1usize, 3] {
        let mut workers = Workers::parallel(w);
        workers.shuffle = Some(7 + w as u64);
        let mut r = Rig::new(&p, workers);
        same_bits(&reference, &r.play(&blocks), &format!("W = {w}"));
        assert_eq!(r.feeds[0].1.latest().unwrap(), last, "W = {w}: the frame");
    }
}

/// The frames by the sample their segment ended on (playing).
fn by_sample(frames: &[DanceFrame]) -> BTreeMap<i64, DanceFrame> {
    frames
        .iter()
        .filter(|f| f.playing)
        .map(|f| ((f.seconds * SR).round() as i64, *f))
        .collect()
}

#[test]
fn the_frames_are_block_size_independent() {
    let (p, track) = with_dancer(&demo_project());
    let (_, a) = render_frames(&p, 64);
    let (_, b) = render_frames(&p, 1000);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].0, track);
    let (fa, fb) = (by_sample(&a[0].1), by_sample(&b[0].1));
    assert!(fa.len() > 1_000, "a frame at every segment: {}", fa.len());
    let mut common = 0;
    for (s, x) in &fb {
        let Some(y) = fa.get(s) else { continue };
        common += 1;
        assert_eq!(x.values, y.values, "sample {s}: the numbers");
        assert_eq!((x.note, x.held), (y.note, y.held), "sample {s}: the notes");
        assert_eq!(
            (x.bar, x.bar_start_beats, x.tempo_bpm),
            (y.bar, y.bar_start_beats, y.tempo_bpm),
            "sample {s}: the bar"
        );
        assert!((x.beats - y.beats).abs() < 1e-9, "sample {s}: the beats");
    }
    assert!(common > 100, "{common} frames end on the same sample");
}

#[test]
fn what_it_publishes_follows_the_song() {
    let (p, _) = with_dancer(&demo_project());
    let (_, t) = render_frames(&p, 256);
    let frames = &t[0].1;
    let at = |beats: f64| {
        *frames
            .iter()
            .rev()
            .find(|f| f.playing && f.beats <= beats)
            .unwrap()
    };
    let wave = dancer::move_value(Move::Wave);
    // The automation holds Wave over bar 2 (beats 4…8), and only there.
    assert_eq!(at(3.99).value("move"), Some(0.0));
    assert_eq!(at(4.01).value("move"), Some(wave));
    assert_eq!(at(7.99).value("move"), Some(wave));
    assert_eq!(at(4.01).bar, 1);
    assert_eq!(at(4.01).bar_start_beats, 4.0);
    // Notes: Clap from beat 1 (held a beat), Spin held from beat 5 for two.
    let clap = dancer::BASE_KEY + Move::Clap.index() as u8;
    let f = at(1.5);
    let n = f.note.unwrap();
    assert_eq!((n.key, n.beats), (clap, 1.0));
    assert!((n.velocity - 90.0 / 127.0).abs() < 1e-6);
    assert!(f.is_held(clap));
    assert!(!at(2.5).is_held(clap));
    let spin = dancer::BASE_KEY + Move::Spin.index() as u8;
    assert!(at(6.5).is_held(spin) && at(6.5).is_held(100));
    let n = at(6.5).note.unwrap();
    assert_eq!(
        (n.key, n.beats, n.released),
        (spin, 5.0, None),
        "a key outside the map is no note"
    );
    assert_eq!(
        at(6.5).prev_note.unwrap().key,
        dancer::BASE_KEY + Move::Jump.index() as u8
    );
    assert_eq!(at(7.5).note.unwrap().released, Some(7.0));
    // The LFO moves the hue, the follower the level (the lead plays from the start).
    let hues: Vec<f32> = frames.iter().map(|f| f.value("hue").unwrap()).collect();
    assert!(hues.iter().any(|h| *h > 10.0) && hues.iter().any(|h| *h < -10.0));
    assert!(frames.iter().any(|f| f.value("level").unwrap() > 0.05));
    assert_eq!(at(2.0).value("react"), Some(0.5));
    // The render plays on through its tail.
    assert!(frames.last().unwrap().beats > 8.0);
    assert!((at(8.0).tempo_bpm - 128.0).abs() < 1e-6);
}

#[test]
fn stopped_it_still_publishes() {
    let (p, _) = with_dancer(&demo_project());
    let snap = Arc::new(Snapshot::compile(&p, 1));
    let units = cantus_engine::inst::units_for(&snap, SR as f32, |_, _| false);
    let feed = cantus_engine::dance::feeds_of(&units)[0].1.clone();
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64 + units.len());
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(SR, rx_engine, tx_engine);
    for u in units {
        tx.push(EngineMessage::AddInstrument(u)).ok();
    }
    tx.push(EngineMessage::SetSnapshot(snap)).ok();
    tx.push(EngineMessage::Seek(Ticks::beats(6))).ok();
    let mut buf = vec![0f32; 512];
    for _ in 0..4 {
        proc.process(&mut buf, 2);
        while rx.pop().is_ok() {}
    }
    let f = feed.latest().unwrap();
    assert!(feed.published() >= 4);
    assert!(!f.playing);
    assert!(
        (f.beats - 6.0).abs() < 1e-9,
        "the stopped playhead: {}",
        f.beats
    );
    assert_eq!(f.bar, 1);
}

#[test]
fn a_step_sequencer_on_move_changes_the_move_step_by_step() {
    let mut p = demo_project();
    let mut t = p.make_track("Dancer");
    let id = t.id;
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Dancer));
    p.tracks.push(t);
    let mut steps = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Step);
    if let ModSource::Step(s) = &mut steps.source {
        s.steps = vec![0.0, 0.25, 0.5, 1.0];
        s.unipolar = true;
    }
    steps.routes = vec![ModRoute {
        target: AutoTarget::Instrument {
            track: id,
            key: "move".into(),
        },
        depth: 1.0,
    }];
    p.modulators_mut(ModHost::Track(id)).unwrap().push(steps);
    let (_, t) = render_frames(&p, 256);
    let moves: std::collections::BTreeSet<i32> = t[0]
        .1
        .iter()
        .filter(|f| f.playing)
        .map(|f| f.value("move").unwrap().round() as i32)
        .collect();
    assert!(moves.len() >= 3, "the steps choose moves: {moves:?}");
    assert!(moves.iter().all(|m| (0..=16).contains(m)));
}

/// A WAV export (any render that did not ask) keeps no dancer's timeline: a dancer
/// publishes at every segment, and a song's frames would be hundreds of MB.
#[test]
fn a_render_keeps_no_timeline_unless_a_video_asks_for_it() {
    let (p, _) = with_dancer(&demo_project());
    let progress = RenderProgress::default();
    render_with_progress(
        Arc::new(Snapshot::compile(&p, 1)),
        &RenderOptions {
            tail_seconds: 0.25,
            ..Default::default()
        },
        Vec::new(),
        Vec::new(),
        &progress,
    )
    .unwrap();
    let t = progress.dance.timelines();
    assert_eq!(t.len(), 1, "the feed is there");
    assert!(t[0].1.is_empty(), "{} frames kept", t[0].1.len());
}
