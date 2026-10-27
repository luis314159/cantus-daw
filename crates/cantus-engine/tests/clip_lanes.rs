//! A note clip's controller lanes and its notes' release velocities on the audio thread
//! (ADR 0068): absent is a null to the bit, present is heard, block-size independent,
//! bit-identical between real time and the offline render, sample-accurate, chased to
//! where the playhead lands, and put back to rest where its clip ends and at a stop.

use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::*;
use cantus_dsp::nulltest::{assert_null, compare};
use cantus_engine::inst::{self as einst, Controller, InstrumentUnit, NoteOn};
use cantus_engine::*;
use std::sync::{Arc, Mutex};

const SR: f64 = 48_000.0;

fn pt(beats: f64, value: f32) -> AutoPoint {
    AutoPoint::new(Ticks((beats * PPQ as f64) as i64), value)
}

/// Three instruments that read controllers and release velocities: the analog synth (bend,
/// mod wheel), the electric piano (bend, the damper's speed from the release velocity) and
/// the FM synth (its bend range) — each a clip of notes, every note with a release.
fn base_project() -> Project {
    let mut p = Project::new("lanes");
    for (i, kind) in [
        InstrumentKind::Analog,
        InstrumentKind::ElectricPiano,
        InstrumentKind::Fm,
    ]
    .into_iter()
    .enumerate()
    {
        let mut t = p.make_track(format!("T{i}"));
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        let mut clip = p.make_clip("phrase", Ticks::ZERO, Ticks::beats(6));
        for (k, pitch) in [57u8, 60, 64].iter().enumerate() {
            let mut n = p.make_note(Ticks::beats(k as i64 * 2), Ticks::beats(1), *pitch, 100);
            n.release = 0;
            clip.notes.push(n);
        }
        t.clips.push(clip);
        p.tracks.push(t);
    }
    p
}

/// The same with lanes on every clip: a bend up and back, the mod wheel rising, pressure
/// in steps — and the notes let go at different speeds.
fn laned_project() -> Project {
    let mut p = base_project();
    for t in &mut p.tracks {
        let c = &mut t.clips[0];
        let mut up = pt(1.0, 0.9);
        up.tension = 0.4;
        let mut steps = vec![pt(0.0, 0.2), pt(2.0, 0.7), pt(4.0, 0.4)];
        steps.iter_mut().for_each(|s| s.shape = Shape::Hold);
        c.lanes = vec![
            ClipLane::new(LaneKind::PitchBend, vec![pt(0.0, 0.5), up, pt(2.5, 0.5)]),
            ClipLane::new(LaneKind::Cc(1), vec![pt(0.0, 0.0), pt(6.0, 1.0)]),
            ClipLane::new(LaneKind::Pressure, steps),
        ];
        for (k, n) in c.notes.iter_mut().enumerate() {
            n.release = [20, 64, 127][k];
        }
    }
    p
}

fn snap(p: &Project) -> Arc<Snapshot> {
    Arc::new(Snapshot::compile(p, 1))
}

fn opts(block: usize) -> RenderOptions {
    RenderOptions {
        tail_seconds: 0.5,
        block_frames: block,
        ..Default::default()
    }
}

/// The null: lanes with no points, a muted clip with lanes, a take lane's clip with lanes,
/// and notes without a release render exactly as none.
#[test]
fn no_lanes_and_silent_lanes_render_exactly_as_none() {
    let plain = render(snap(&base_project()), &opts(512)).unwrap();
    assert!(plain.peak > 0.01);
    let mut p = base_project();
    for t in &mut p.tracks {
        t.clips[0].lanes = vec![ClipLane::new(LaneKind::Cc(74), vec![])];
    }
    let mut muted = p.make_clip("muted", Ticks::ZERO, Ticks::beats(4));
    muted.muted = true;
    muted.lanes = laned_project().tracks[0].clips[0].lanes.clone();
    let mut take = muted.clone();
    take.id = ClipId(p.alloc_id());
    take.muted = false;
    take.lane = 1;
    p.tracks[0].clips.push(muted);
    p.tracks[0].clips.push(take);
    let same = render(snap(&p), &opts(512)).unwrap();
    assert_null(&plain.samples, &same.samples, 0.0, "silent lanes");
}

/// Present, the lanes are heard; and they are block-size independent to the bit.
#[test]
fn lanes_are_heard_and_block_size_independent() {
    let s = snap(&laned_project());
    let reference = render(s.clone(), &opts(512)).unwrap();
    let plain = render(snap(&base_project()), &opts(512)).unwrap();
    assert!(
        !compare(&plain.samples, &reference.samples).passes(1e-3),
        "the lanes must be heard"
    );
    for block in [1usize, 7, 64, 333, 1024, 8192] {
        let r = render(s.clone(), &opts(block)).unwrap();
        assert_null(
            &reference.samples,
            &r.samples,
            0.0,
            &format!("block {block}"),
        );
    }
}

/// Real time, in uneven blocks, equals the offline render to the bit.
#[test]
fn realtime_processing_matches_the_offline_render() {
    let s = snap(&laned_project());
    let offline = render(s.clone(), &opts(256)).unwrap();
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
    let mut live = (*s).clone();
    live.loop_region = None;
    live.metronome = false;
    let live = Arc::new(live);
    for unit in cantus_engine::inst::units_for(&live, SR as f32, |_, _| false) {
        tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(&live)))
        .unwrap();
    tx.push(EngineMessage::SetSnapshot(live)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let total = offline.frames();
    let mut out = Vec::with_capacity(total * 2);
    let sizes = [128usize, 333, 512, 96, 1024, 17, 5];
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    assert_null(&offline.samples, &out, 0.0, "realtime vs offline");
}

/// What an instrument heard, sample by sample.
#[derive(Debug, Default)]
struct Log {
    /// (sample, controller, value)
    controls: Vec<(u64, Controller, f32)>,
    /// (sample, key, release velocity)
    offs: Vec<(u64, u8, f32)>,
}

#[derive(Debug)]
struct Ear {
    log: Arc<Mutex<Log>>,
    at: u64,
}

impl InstrumentUnit for Ear {
    fn configure(&mut self, _spec: &NativeSpec, _assets: &einst::Assets) {}
    fn note_on(&mut self, _note: NoteOn) {}
    fn note_off(&mut self, key: u8, _id: u64, velocity: f32) {
        self.log.lock().unwrap().offs.push((self.at, key, velocity));
    }
    fn controller(&mut self, c: Controller, value: f32) {
        self.log.lock().unwrap().controls.push((self.at, c, value));
    }
    fn all_notes_off(&mut self) {}
    fn reset(&mut self) {}
    fn process(&mut self, l: &mut [f32], _r: &mut [f32]) -> bool {
        self.at += l.len() as u64;
        false
    }
    fn active_voices(&self) -> usize {
        0
    }
    fn telemetry(&self) -> einst::Telemetry {
        [0.0; einst::TELEMETRY_LEN]
    }
}

/// A one-track project at 120 BPM (a beat is 24 000 samples) whose instrument is an ear,
/// played from `start` in blocks of `block` for seven beats.
fn ear_render(p: &Project, start: Ticks, block: usize) -> Log {
    let log = Arc::new(Mutex::new(Log::default()));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(SR, rx_engine, tx_engine);
    let mut s = Snapshot::compile(p, 1);
    s.metronome = false;
    let track = p.tracks[0].id;
    tx.push(EngineMessage::AddInstrument(Box::new(NativeUnit {
        track,
        kind: UnitKind::Native(InstrumentKind::Analog),
        serial: 1,
        unit: Box::new(Ear {
            log: log.clone(),
            at: 0,
        }),
        automation: Default::default(),
    })))
    .unwrap();
    tx.push(EngineMessage::SetSnapshot(Arc::new(s))).unwrap();
    tx.push(EngineMessage::Seek(start)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let total = 7 * 24_000usize;
    let mut done = 0;
    while done < total {
        let n = block.min(total - done);
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        done += n;
        while rx.pop().is_ok() {}
    }
    let out = std::mem::take(&mut *log.lock().unwrap());
    out
}

fn ear_project() -> Project {
    let mut p = base_project();
    p.tracks.truncate(1);
    p.tempo_bpm = 120.0;
    let c = &mut p.tracks[0].clips[0];
    c.lanes = vec![ClipLane::new(
        LaneKind::PitchBend,
        vec![pt(0.0, 0.5), pt(1.0, 1.0)],
    )];
    for (k, n) in c.notes.iter_mut().enumerate() {
        n.release = [20, 64, 127][k];
    }
    p
}

/// Every bend value lands on the sample its tick is at, whatever the block size; the
/// release velocities reach the note-offs; the lane rests where the clip ends.
#[test]
fn lanes_and_releases_land_on_their_samples() {
    let p = ear_project();
    let a = ear_render(&p, Ticks::ZERO, 512);
    let b = ear_render(&p, Ticks::ZERO, 37);
    assert_eq!(a.controls, b.controls, "the same at any block size");
    let bends: Vec<&(u64, Controller, f32)> = a
        .controls
        .iter()
        .filter(|c| c.1 == Controller::PitchBend)
        .collect();
    // At the start: the chase (at rest) and the curve's first value, the centre.
    assert!(bends.iter().filter(|c| c.0 == 0).all(|c| c.2 == 0.0));
    // Tick t is sample t × 25 at 120 BPM and 48 kHz: the ramp on the 1/96-beat grid.
    let step = cantus_core::clip_lanes::STEP as u64 * 25;
    let ramp: Vec<&&(u64, Controller, f32)> =
        bends.iter().filter(|c| c.0 > 0 && c.0 <= 24_000).collect();
    assert_eq!(ramp.len(), 96, "{ramp:?}");
    assert!(ramp.iter().all(|c| c.0 % step == 0));
    assert!(ramp.windows(2).all(|w| w[1].2 > w[0].2));
    let top = ramp.last().unwrap();
    assert_eq!(top.0, 24_000);
    assert!((top.2 - 2.0).abs() < 1e-6, "the top: +2 semitones");
    let last = bends.last().unwrap();
    assert_eq!(last.0, 6 * 24_000, "the rest where the clip ends");
    assert_eq!(last.2, 0.0);
    let offs: Vec<(u64, u8, f32)> = a.offs.iter().copied().take(3).collect();
    assert_eq!(
        offs,
        vec![
            (24_000, 57, 20.0 / 127.0),
            (72_000, 60, 64.0 / 127.0),
            (120_000, 64, 1.0),
        ]
    );
}

/// A render that starts inside the bend hears the bend there first (chase).
#[test]
fn a_start_inside_a_lane_chases_its_value() {
    let p = ear_project();
    // Off the lane's grid, so what it holds there is the chase alone.
    let from = Ticks(PPQ / 2 + 5);
    let log = ear_render(&p, from, 512);
    // What it holds at the first sample: the last of what was sent there (a rest may come
    // first, from the transport's seek).
    let first = log
        .controls
        .iter()
        .rfind(|c| c.1 == Controller::PitchBend && c.0 == 0)
        .copied()
        .unwrap();
    // The last event before it (at half a beat), as ±2 semitones.
    let lane = &p.tracks[0].clips[0].lanes[0];
    let last_tick = (from.0 - 1) / cantus_core::clip_lanes::STEP * cantus_core::clip_lanes::STEP;
    let want = (lane.value_at(last_tick as f64).unwrap() - 0.5) * 4.0;
    assert!((first.2 - want).abs() < 1e-6, "{} vs {want}", first.2);
}

/// The ear's rig: a processor with the ear on `p`'s first track, the snapshot sent.
fn ear_rig(p: &Project) -> (rtrb::Producer<EngineMessage>, Processor, Arc<Mutex<Log>>) {
    let log = Arc::new(Mutex::new(Log::default()));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, _rx) = rtrb::RingBuffer::<EngineEvent>::new(1 << 16);
    let proc = Processor::without_midi(SR, rx_engine, tx_engine);
    tx.push(EngineMessage::AddInstrument(Box::new(NativeUnit {
        track: p.tracks[0].id,
        kind: UnitKind::Native(InstrumentKind::Analog),
        serial: 1,
        unit: Box::new(Ear {
            log: log.clone(),
            at: 0,
        }),
        automation: Default::default(),
    })))
    .unwrap();
    tx.push(EngineMessage::SetSnapshot(snap(p))).unwrap();
    (tx, proc, log)
}

fn blocks(proc: &mut Processor, n: usize) {
    for _ in 0..n {
        let mut buf = vec![0f32; 512 * 2];
        proc.process(&mut buf, 2);
    }
}

/// Review #3: stopped, a snapshot, a seek and a stop send the instrument nothing — a held
/// pedal or a bend would sound under the keyboard; playing chases.
#[test]
fn stopped_the_lanes_are_not_chased() {
    let p = ear_project();
    let (mut tx, mut proc, log) = ear_rig(&p);
    blocks(&mut proc, 2);
    tx.push(EngineMessage::Seek(Ticks(PPQ / 2 + 5))).unwrap();
    blocks(&mut proc, 2);
    tx.push(EngineMessage::SetSnapshot(snap(&p))).unwrap();
    tx.push(EngineMessage::Stop).unwrap();
    blocks(&mut proc, 2);
    assert!(
        log.lock().unwrap().controls.is_empty(),
        "{:?}",
        log.lock().unwrap().controls
    );
    tx.push(EngineMessage::Play).unwrap();
    blocks(&mut proc, 1);
    assert!(
        !log.lock().unwrap().controls.is_empty(),
        "chased at the start"
    );
}

/// Review #4: a lane removed while it plays puts its controller back to rest.
#[test]
fn a_lane_removed_mid_play_rests_its_controller() {
    let p = ear_project();
    let (mut tx, mut proc, log) = ear_rig(&p);
    tx.push(EngineMessage::Play).unwrap();
    // A beat and a half in: the bend is up (+2 semitones).
    blocks(&mut proc, 70);
    let last_bend = |log: &Arc<Mutex<Log>>| {
        log.lock()
            .unwrap()
            .controls
            .iter()
            .rfind(|c| c.1 == Controller::PitchBend)
            .map(|c| c.2)
    };
    assert!((last_bend(&log).unwrap() - 2.0).abs() < 1e-6);
    let mut q = p.clone();
    q.tracks[0].clips[0].lanes.clear();
    tx.push(EngineMessage::SetSnapshot(snap(&q))).unwrap();
    blocks(&mut proc, 1);
    assert_eq!(last_bend(&log), Some(0.0), "back to the centre");
}
