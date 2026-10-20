//! Input, monitoring and recording in the engine (ADR 0025 §4, §7, §8): a processor driven
//! by hand stands in for the audio callback, and the test pushes into the input ring
//! itself, as the input stream would.

use cantus_core::*;
use cantus_dsp::math;
use cantus_engine::*;
use std::sync::Arc;

const SR: u32 = 48_000;

fn signal(frames: usize, channels: usize, seed: u32) -> Vec<f32> {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    (0..frames * channels)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x as f32 / u32::MAX as f32) * 1.6 - 0.8
        })
        .collect()
}

/// One empty track with an input; `direct` sends it straight to the master, otherwise it
/// feeds its strip.
fn project(input: TrackInput, armed: bool, direct: bool) -> Project {
    let mut p = Project::new("rec");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    t.input = Some(input);
    t.armed = armed;
    if direct {
        t.strip = None;
    }
    p.tracks.push(t);
    p
}

/// A processor with an input ring of `channels`, and the UI's ends.
struct Rig {
    p: Processor,
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    input: rtrb::Producer<f32>,
    channels: usize,
}

impl Rig {
    fn new(snap: Snapshot, channels: usize) -> Rig {
        let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        let mut p = Processor::without_midi(SR as f64, rx_engine, tx_engine);
        let (input, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * channels);
        p.attach_input(consumer, channels);
        tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
        Rig {
            p,
            tx,
            rx,
            input,
            channels,
        }
    }

    /// Push `frames` of interleaved input and render as many frames.
    fn run(&mut self, input: &[f32]) -> Vec<f32> {
        for x in input {
            self.input.push(*x).unwrap();
        }
        let n = input.len() / self.channels;
        let mut out = vec![0f32; n * 2];
        self.p.process(&mut out, 2);
        out
    }

    fn events(&mut self) -> Vec<EngineEvent> {
        let mut v = Vec::new();
        while let Ok(e) = self.rx.pop() {
            v.push(e);
        }
        v
    }
}

#[test]
fn a_monitored_input_at_unity_reaches_the_output_bit_exact() {
    let snap = Snapshot::compile(&project(TrackInput::mono(1), false, true), 1);
    let mut snap = snap;
    snap.tracks[0].input.as_mut().unwrap().monitor = MonitorMode::On;
    let mut rig = Rig::new(snap, 2);
    let input = signal(4096, 2, 1);
    for block in input.chunks(2 * 256) {
        let out = rig.run(block);
        for (o, i) in out.chunks_exact(2).zip(block.chunks_exact(2)) {
            // Channel 1, mono, into both sides, untouched.
            assert_eq!(o[0].to_bits(), i[1].to_bits());
            assert_eq!(o[1].to_bits(), i[1].to_bits());
        }
    }
}

#[test]
fn a_monitored_input_goes_through_its_strip() {
    // A stereo pair through a strip at the -6 dB pan law: exactly half, both sides —
    // the strip's controls apply, so its inserts do too.
    let mut p = project(TrackInput::stereo(0), false, false);
    p.pan_law = PanLaw::Minus6dB;
    p.tracks[0].input.as_mut().unwrap().monitor = MonitorMode::On;
    let mut rig = Rig::new(Snapshot::compile(&p, 1), 2);
    let input = signal(2048, 2, 2);
    for block in input.chunks(2 * 128) {
        let out = rig.run(block);
        for (o, i) in out.chunks_exact(2).zip(block.chunks_exact(2)) {
            assert_eq!(o[0], i[0] * 0.5);
            assert_eq!(o[1], i[1] * 0.5);
        }
    }
}

#[test]
fn trim_scales_the_monitored_input() {
    let mut input = TrackInput::mono(0);
    input.monitor = MonitorMode::On;
    input.trim_db = -6.0;
    let mut rig = Rig::new(Snapshot::compile(&project(input, false, true), 1), 1);
    let x = signal(512, 1, 3);
    let out = rig.run(&x);
    let g = math::powf(10f32, -6.0 / 20.0);
    for (o, i) in out.chunks_exact(2).zip(&x) {
        assert_eq!(o[0], i * g);
    }
}

/// What a track hears of its input, by monitor mode, armed state and transport.
fn hears(monitor: MonitorMode, armed: bool, playing: bool, recording: bool) -> bool {
    let mut input = TrackInput::mono(0);
    input.monitor = monitor;
    let mut rig = Rig::new(Snapshot::compile(&project(input, armed, true), 1), 1);
    if recording {
        rig.tx.push(EngineMessage::SetRecording(true)).unwrap();
    }
    if playing {
        rig.tx.push(EngineMessage::Play).unwrap();
    }
    let out = rig.run(&vec![0.25; 256]);
    out.iter().any(|x| *x != 0.0)
}

#[test]
fn monitor_modes_decide_what_is_heard() {
    use MonitorMode::*;
    // Off never, armed or not.
    assert!(!hears(Off, true, false, false));
    assert!(!hears(Off, true, true, true));
    // On always.
    assert!(hears(On, false, false, false));
    assert!(hears(On, false, true, false));
    // Auto: armed, and stopped or recording.
    assert!(hears(Auto, true, false, false));
    assert!(hears(Auto, true, true, true));
    assert!(
        !hears(Auto, true, true, false),
        "playing back: hear the takes"
    );
    assert!(!hears(Auto, false, false, false), "not armed");
}

#[test]
fn every_input_channel_is_metered() {
    let mut rig = Rig::new(Snapshot::compile(&Project::new("m"), 1), 3);
    let mut x = vec![0f32; 3 * 256];
    x[3 * 10] = 0.25;
    x[3 * 20 + 1] = -0.5;
    rig.run(&x);
    let meters = rig
        .events()
        .into_iter()
        .rev()
        .find_map(|e| match e {
            EngineEvent::Meters(m) => Some(m),
            _ => None,
        })
        .unwrap();
    assert_eq!(meters.input_count, 3);
    assert_eq!(&meters.inputs[..3], &[0.25, 0.5, 0.0]);
}

#[test]
fn the_input_ring_absorbs_two_clocks() {
    let mut input = TrackInput::mono(0);
    input.monitor = MonitorMode::On;
    let mut rig = Rig::new(Snapshot::compile(&project(input, false, true), 1), 1);
    let ramp: Vec<f32> = (0..100_000).map(|i| i as f32).collect();
    let mut pushed = 0;
    let mut push = |rig: &mut Rig, n: usize| {
        for x in &ramp[pushed..pushed + n] {
            rig.input.push(*x).unwrap();
        }
        pushed += n;
    };
    let mut out = vec![0f32; 128 * 2];
    // Not primed until a whole block waits: silence, nothing consumed.
    push(&mut rig, 100);
    rig.p.process(&mut out, 2);
    assert!(out.iter().all(|x| *x == 0.0));
    // A block and a bit: primed, the first block plays in order.
    push(&mut rig, 60);
    rig.p.process(&mut out, 2);
    assert_eq!(out[0], 0.0);
    assert_eq!(out[127 * 2], 127.0);
    // The input runs fast: 32 left over and 225 more is a frame over the target (a
    // block) plus a block — shed a frame at a time.
    push(&mut rig, 225);
    rig.p.process(&mut out, 2);
    assert_eq!(rig.p.stats().input_dropped, 1);
    assert_eq!(out[0], 129.0, "one frame skipped");
    assert_eq!(out[127 * 2], 256.0);
    // A backlog of more than a block goes at once.
    push(&mut rig, 128 * 4);
    rig.p.process(&mut out, 2);
    assert_eq!(rig.p.stats().input_dropped, 1 + 384);
    assert_eq!(out[0], 257.0 + 384.0);
    // The input runs slow: less than a block, and the last frame is held.
    let mut rig2 = Rig::new(Snapshot::compile(&project(input_on(), false, true), 1), 1);
    for x in &ramp[..128] {
        rig2.input.push(*x).unwrap();
    }
    rig2.p.process(&mut out, 2);
    for x in &ramp[128..128 + 100] {
        rig2.input.push(*x).unwrap();
    }
    rig2.p.process(&mut out, 2);
    assert_eq!(out[99 * 2], 227.0);
    assert_eq!(out[127 * 2], 227.0, "the last frame held");
    assert_eq!(rig2.p.stats().input_repeated, 28);
    // Dry: silence, and it waits for a whole block again.
    rig2.p.process(&mut out, 2);
    assert!(out.iter().all(|x| *x == 0.0));
}

fn input_on() -> TrackInput {
    let mut i = TrackInput::mono(0);
    i.monitor = MonitorMode::On;
    i
}

// ---- recording -------------------------------------------------------------------------

/// A handle and its processor with an input ring of one channel, the project sent.
fn handle_rig(p: &Project) -> (EngineHandle, Processor, rtrb::Producer<f32>) {
    let (mut h, mut proc) = EngineHandle::manual(SR as f64);
    let (input, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * 2);
    proc.attach_input(consumer, 1);
    let snap = h.compile(p, 1);
    h.set_snapshot(snap);
    (h, proc, input)
}

/// Push `input` and render it, `block` frames at a time, polling the handle as the UI does.
fn drive(
    h: &mut EngineHandle,
    proc: &mut Processor,
    ring: &mut rtrb::Producer<f32>,
    input: &[f32],
    block: usize,
) {
    let mut out = vec![0f32; block * 2];
    for chunk in input.chunks(block) {
        for x in chunk {
            ring.push(*x).unwrap();
        }
        let out = &mut out[..chunk.len() * 2];
        proc.process(out, 2);
        h.poll();
    }
}

fn read_wav(path: &std::path::Path) -> (hound::WavSpec, Vec<f32>) {
    let mut r = hound::WavReader::open(path).unwrap();
    let spec = r.spec();
    let v = r.samples::<f32>().map(|s| s.unwrap()).collect();
    (spec, v)
}

#[test]
fn a_pass_records_the_input_bit_exact_as_32_bit_float() {
    let dir = tempfile::tempdir().unwrap();
    let p = project(TrackInput::mono(0), true, true);
    let (mut h, mut proc, mut ring) = handle_rig(&p);
    assert_eq!(h.begin_recording(dir.path()), Ok(1));
    assert!(h.is_recording_audio());
    h.set_recording(true);
    h.play();
    let x = signal(30_000, 1, 9);
    drive(&mut h, &mut proc, &mut ring, &x, 480);
    h.stop();
    drive(&mut h, &mut proc, &mut ring, &[0.0; 480], 480);
    let takes = h.finish_recording();
    assert!(!h.is_recording_audio());
    assert_eq!(takes.len(), 1);
    let t = &takes[0];
    assert_eq!(t.track, p.tracks[0].id);
    assert_eq!(
        (t.frames, t.channels, t.sample_rate, t.dropped),
        (30_000, 1, SR, 0)
    );
    assert_eq!(t.start_tick, 0.0);
    assert!(t.wraps.is_empty());
    let (spec, samples) = read_wav(&t.path);
    assert_eq!(spec.bits_per_sample, 32);
    assert_eq!(spec.sample_format, hound::SampleFormat::Float);
    assert_eq!(samples, x, "what is recorded is the input, to the bit");
    let name = t.path.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("Voice ") && name.ends_with(".wav"),
        "{name}"
    );
}

#[test]
fn a_stereo_input_records_its_pair_with_its_trim() {
    let dir = tempfile::tempdir().unwrap();
    let mut input = TrackInput::stereo(1);
    input.trim_db = 6.0;
    let p = project(input, true, true);
    let (mut h, mut proc) = EngineHandle::manual(SR as f64);
    let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * 3);
    proc.attach_input(consumer, 3);
    let snap = h.compile(&p, 1);
    h.set_snapshot(snap);
    h.begin_recording(dir.path()).unwrap();
    h.set_recording(true);
    h.play();
    let x = signal(4_800, 3, 10);
    let mut out = vec![0f32; 480 * 2];
    for chunk in x.chunks(480 * 3) {
        for s in chunk {
            ring.push(*s).unwrap();
        }
        proc.process(&mut out, 2);
        h.poll();
    }
    h.stop();
    proc.process(&mut out, 2);
    h.poll();
    let takes = h.finish_recording();
    let (spec, got) = read_wav(&takes[0].path);
    assert_eq!(spec.channels, 2);
    let g = math::powf(10f32, 6.0 / 20.0);
    let want: Vec<f32> = x
        .chunks_exact(3)
        .flat_map(|f| [f[1] * g, f[2] * g])
        .collect();
    assert_eq!(got, want);
}

#[test]
fn nothing_armed_records_nothing_and_an_idle_pass_leaves_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let p = project(TrackInput::mono(0), false, true);
    let (mut h, _proc, _ring) = handle_rig(&p);
    assert_eq!(h.begin_recording(dir.path()), Ok(0));
    assert!(!h.is_recording_audio());
    let p = project(TrackInput::mono(0), true, true);
    let (mut h, mut proc, mut ring) = handle_rig(&p);
    assert_eq!(h.begin_recording(dir.path()), Ok(1));
    // Prepared, but the transport never ran with recording on.
    drive(&mut h, &mut proc, &mut ring, &[0.1; 960], 480);
    assert!(h.finish_recording().is_empty());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn loop_passes_are_marked_where_the_loop_wraps() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = project(TrackInput::mono(0), true, true);
    // One beat: 24 000 frames at 120 BPM.
    p.loop_enabled = true;
    p.loop_region = Some(LoopRegion {
        start: Ticks::ZERO,
        end: Ticks::beats(1),
    });
    let (mut h, mut proc, mut ring) = handle_rig(&p);
    h.begin_recording(dir.path()).unwrap();
    h.set_recording(true);
    h.play();
    drive(&mut h, &mut proc, &mut ring, &signal(60_000, 1, 11), 1000);
    h.stop();
    drive(&mut h, &mut proc, &mut ring, &[0.0; 100], 100);
    let takes = h.finish_recording();
    assert_eq!(takes[0].frames, 60_000);
    assert_eq!(takes[0].wraps, vec![24_000, 48_000]);
}

#[test]
fn a_take_lines_up_with_what_was_heard() {
    // The player hears beat 2 and plays a click on it; the input arrives `latency`
    // frames later. Played back, the take's click must land on beat 2.
    for latency in [250i64, 1_000, 137] {
        let dir = tempfile::tempdir().unwrap();
        let p = project(TrackInput::mono(0), true, true);
        let (mut h, mut proc, mut ring) = handle_rig(&p);
        h.set_record_offset(latency);
        h.begin_recording(dir.path()).unwrap();
        h.set_recording(true);
        h.play();
        let beat2 = 2 * 24_000usize;
        let mut x = vec![0f32; 100_000];
        x[beat2 + latency as usize] = 1.0;
        drive(&mut h, &mut proc, &mut ring, &x, 512);
        h.stop();
        drive(&mut h, &mut proc, &mut ring, &[0.0; 512], 512);
        let take = h.finish_recording().remove(0);
        // Place it as the UI does: the clip at the take's start, rounded to a tick.
        let mut q = Project::new("play");
        q.tempo_bpm = 120.0;
        let mut t = q.make_track("t");
        t.strip = None;
        q.tracks.push(t);
        let id = AudioSourceId(q.alloc_id());
        q.audio_pool.push(AudioSource {
            id,
            name: "take".into(),
            path: take.path.to_string_lossy().into_owned(),
            sample_rate: SR,
            channels: 1,
            frames: take.frames,
            sha256: None,
            recorded: true,
        });
        let start = take.start_tick.round() as i64;
        // A take that begins before zero is windowed from zero, as the UI would.
        let (clip_start, offset) = if start < 0 { (0, -start) } else { (start, 0) };
        let mut c = q.make_audio_clip("take", id, Ticks(clip_start), Ticks::beats(8));
        c.offset = Ticks(offset);
        q.tracks[0].clips.push(c);
        let mut s = Snapshot::compile(&q, 1);
        assert!(SampleCache::new().attach_audio(&mut s, SR).is_empty());
        let out = render(Arc::new(s), &RenderOptions::default()).unwrap();
        let at = out
            .samples
            .chunks_exact(2)
            .position(|f| f[0] == 1.0)
            .unwrap();
        // A tick is 25 frames here, and the model places a clip to the tick.
        let err = at as i64 - beat2 as i64;
        let exact = latency % 25 == 0;
        assert!(
            if exact {
                err.abs() <= 1
            } else {
                err.abs() <= 13
            },
            "latency {latency}: the click came back {err} frames off"
        );
    }
}

#[test]
fn a_full_ring_drops_and_reports_and_never_blocks() {
    let p = project(TrackInput::mono(0), true, true);
    let mut rig = Rig::new(Snapshot::compile(&p, 1), 1);
    // A ring with room for 1 000 frames, and nobody draining it.
    let (producer, _consumer) = rtrb::RingBuffer::<f32>::new(1_000);
    let rings = RecordRings {
        tracks: vec![cantus_engine::recorder::TrackRing::new(
            p.tracks[0].id,
            1,
            producer,
        )],
    };
    rig.tx
        .push(EngineMessage::AttachRecorder(Box::new(rings)))
        .unwrap();
    rig.tx.push(EngineMessage::SetRecording(true)).unwrap();
    rig.tx.push(EngineMessage::Play).unwrap();
    let started = std::time::Instant::now();
    for _ in 0..10 {
        rig.run(&[0.5; 256]);
    }
    rig.tx.push(EngineMessage::Stop).unwrap();
    rig.run(&[0.0; 256]);
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    // Three blocks fit (768 frames); the other seven were lost and said so.
    assert_eq!(rig.p.stats().record_overflow, 7 * 256);
    let marks: Vec<RecordMark> = rig
        .events()
        .into_iter()
        .filter_map(|e| match e {
            EngineEvent::Record(m) => Some(m),
            _ => None,
        })
        .collect();
    let lost: u64 = marks
        .iter()
        .map(|m| match m {
            RecordMark::Overflow { frames, .. } => *frames,
            _ => 0,
        })
        .sum();
    assert_eq!(lost, 7 * 256);
    assert!(matches!(marks.first(), Some(RecordMark::Started { tick, .. }) if *tick == 0.0));
    assert_eq!(marks.last(), Some(&RecordMark::Stopped { frames: 2_560 }));
}

#[test]
fn live_peaks_arrive_while_recording_and_headers_are_flushed() {
    let dir = tempfile::tempdir().unwrap();
    let p = project(TrackInput::mono(0), true, true);
    let (mut h, mut proc, mut ring) = handle_rig(&p);
    h.begin_recording(dir.path()).unwrap();
    h.set_recording(true);
    h.play();
    let mut x = vec![0.0f32; 4_800];
    x[300] = 0.75;
    x[700] = -0.5;
    drive(&mut h, &mut proc, &mut ring, &x, 480);
    let mut peaks = Vec::new();
    let since = std::time::Instant::now();
    while peaks.len() < 4_800 / PEAK_FRAMES && since.elapsed().as_secs() < 5 {
        for lp in h.poll_recording() {
            assert_eq!(lp.track, p.tracks[0].id);
            peaks.extend(lp.peaks);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(peaks.len(), 4_800 / PEAK_FRAMES);
    assert_eq!(peaks[1], (0.0, 0.75), "frames 256..512");
    assert_eq!(peaks[2], (-0.5, 0.0), "frames 512..768");
    // The writer updates the header while the pass runs: the file reads back now.
    std::thread::sleep(std::time::Duration::from_millis(800));
    let path = std::fs::read_dir(dir.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let r = hound::WavReader::open(&path).unwrap();
    assert_eq!(r.duration(), 4_800);
    h.stop();
    drive(&mut h, &mut proc, &mut ring, &[0.0; 480], 480);
    h.finish_recording();
}

// ---- the click -------------------------------------------------------------------------

/// The click as the engine has always played it: a burst on every beat (24 000 frames at
/// 120 BPM), the downbeat accented, at the built-in level.
fn reference_click(frames: usize) -> Vec<f32> {
    let mut c = cantus_dsp::Click::new(SR as f32);
    (0..frames)
        .map(|i| {
            if i % 24_000 == 0 {
                c.trigger(i % 96_000 == 0);
            }
            c.next()
        })
        .collect()
}

/// The left output of an empty project with the metronome on.
fn click_run(
    mode: ClickMode,
    db: f32,
    recording: bool,
    record_start: Option<Ticks>,
    frames: usize,
) -> Vec<f32> {
    let mut p = Project::new("click");
    p.tempo_bpm = 120.0;
    p.metronome = true;
    p.recording.click = mode;
    p.recording.click_db = db;
    let mut rig = Rig::new(Snapshot::compile(&p, 1), 1);
    rig.tx
        .push(EngineMessage::SetRecordStart(record_start))
        .unwrap();
    rig.tx.push(EngineMessage::SetRecording(recording)).unwrap();
    rig.tx.push(EngineMessage::Play).unwrap();
    let mut left = Vec::with_capacity(frames);
    let mut out = vec![0f32; 1000 * 2];
    while left.len() < frames {
        rig.p.process(&mut out, 2);
        left.extend(out.chunks_exact(2).map(|f| f[0]));
        rig.events();
    }
    left.truncate(frames);
    left
}

#[test]
fn the_always_click_is_bit_identical_to_before() {
    let want = reference_click(200_000);
    let got = click_run(ClickMode::Always, 0.0, false, None, 200_000);
    assert!(want.iter().any(|x| *x != 0.0));
    for (i, (a, b)) in want.iter().zip(&got).enumerate() {
        assert_eq!(a.to_bits(), b.to_bits(), "frame {i}");
    }
    // Recording or not makes no difference to it.
    assert_eq!(click_run(ClickMode::Always, 0.0, true, None, 200_000), got);
}

#[test]
fn the_click_level_is_relative_to_the_built_in_one() {
    let want = reference_click(100_000);
    let got = click_run(ClickMode::Always, -6.0, false, None, 100_000);
    let g = math::powf(10f32, -6.0 / 20.0);
    for (a, b) in want.iter().zip(&got) {
        assert!((a * g - b).abs() <= 1e-6, "{a} × {g} against {b}");
    }
}

#[test]
fn the_recording_click_sounds_only_while_recording() {
    let silent = click_run(ClickMode::Recording, 0.0, false, None, 100_000);
    assert!(silent.iter().all(|x| *x == 0.0));
    let heard = click_run(ClickMode::Recording, 0.0, true, None, 100_000);
    assert_eq!(heard, reference_click(100_000));
}

#[test]
fn the_count_in_click_stops_at_the_record_start() {
    // A bar of count-in: the take begins at beat 4 (frame 96 000).
    let got = click_run(
        ClickMode::CountInOnly,
        0.0,
        true,
        Some(Ticks::beats(4)),
        200_000,
    );
    let want = reference_click(200_000);
    assert_eq!(&got[..96_000], &want[..96_000]);
    assert!(got[96_000..].iter().all(|x| *x == 0.0));
    // No count-in, no click.
    let none = click_run(ClickMode::CountInOnly, 0.0, true, None, 100_000);
    assert!(none.iter().all(|x| *x == 0.0));
}
