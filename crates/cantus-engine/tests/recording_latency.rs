//! Round 19 fixes on the recording path (ADR 0025 §7, §8): what a take's latency is made
//! of, the input ring's wait, and a full recording ring that keeps the take's length.

use cantus_core::*;
use cantus_engine::*;

const SR: u32 = 48_000;

fn signal(frames: usize, seed: u32) -> Vec<f32> {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    (0..frames)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x as f32 / u32::MAX as f32) * 1.6 - 0.8
        })
        .collect()
}

fn project() -> Project {
    let mut p = Project::new("rec");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    t.input = Some(TrackInput::mono(0));
    t.armed = true;
    t.strip = None;
    p.tracks.push(t);
    p
}

fn rig(p: &Project) -> (EngineHandle, Processor, rtrb::Producer<f32>) {
    let (mut h, mut proc) = EngineHandle::manual(SR as f64);
    let (input, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * 2);
    proc.attach_input(consumer, 1);
    let snap = h.compile(p, 1);
    h.set_snapshot(snap);
    (h, proc, input)
}

/// Render `blocks` blocks of `block` frames, pushing `input` a block at a time first
/// (silence once it runs out), polling the handle as the UI does.
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
        proc.process(&mut out[..chunk.len() * 2], 2);
        h.poll();
    }
}

fn read_wav(path: &std::path::Path) -> Vec<f32> {
    let mut r = hound::WavReader::open(path).unwrap();
    r.samples::<f32>().map(|s| s.unwrap()).collect()
}

#[test]
fn a_takes_latency_is_every_part_of_the_trip() {
    let l = RecordLatency {
        input: 100,
        output: 200,
        plugin: 30,
        queue: 40,
        offset: -5,
    };
    assert_eq!(l.total(), 365);
    assert_eq!(l.measured(), 370);
    // A fresh handle: nothing is known yet but the user's offset.
    let (mut h, _proc) = EngineHandle::manual(SR as f64);
    h.set_record_offset(7);
    assert_eq!(h.record_latency().total(), 7);
}

#[test]
fn the_input_rings_wait_is_part_of_the_latency() {
    // A block of input waits in the ring before it is popped (it was pushed a block
    // ahead): every frame is placed a block after it came in.
    let block = 500; // 20 ticks at 120 BPM and 48 kHz.
    let p = project();
    let (mut h, mut proc, mut ring) = rig(&p);
    for _ in 0..block {
        ring.push(0.0).unwrap();
    }
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 40], block);
    assert_eq!(proc.stats().input_queue, block as f64);
    assert_eq!(h.record_latency().queue, block as i64);
    assert_eq!(h.record_latency().total(), block as i64);

    // Without the extra block the input is placed where it came in: no wait.
    let (mut h, mut proc, mut ring) = rig(&p);
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 40], block);
    assert_eq!(h.record_latency().queue, 0);
}

#[test]
fn a_take_with_the_ring_s_wait_lines_up() {
    // The click is played on beat 2; it is popped a block later than it came in. With the
    // wait counted, the take's start puts it back on beat 2, to the frame.
    let block = 500;
    let dir = tempfile::tempdir().unwrap();
    let p = project();
    let (mut h, mut proc, mut ring) = rig(&p);
    for _ in 0..block {
        ring.push(0.0).unwrap();
    }
    // Let the wait settle before the pass (the stats are smoothed).
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 10], block);
    h.begin_recording(dir.path()).unwrap();
    h.set_recording(true);
    h.play();
    let beat2 = 48_000usize;
    let mut x = vec![0f32; 100_000];
    x[beat2] = 1.0;
    drive(&mut h, &mut proc, &mut ring, &x, block);
    h.stop();
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block], block);
    let take = h.finish_recording().remove(0);
    let file = read_wav(&take.path);
    let at = file.iter().position(|v| *v == 1.0).unwrap();
    // Where the file's click lands on the timeline, in frames.
    let tpf = 120.0 * PPQ as f64 / (60.0 * SR as f64);
    let placed = take.start_tick / tpf + at as f64;
    assert!(
        (placed - beat2 as f64).abs() < 1e-6,
        "the click lands at {placed}"
    );
}

#[test]
fn a_full_ring_keeps_the_take_s_length_and_timing() {
    // The disk stalls for a while: the ring (8 s) fills and two seconds are lost. The
    // file holds silence there, so what follows is still where it was played, and the
    // loss is known while the pass runs.
    let dir = tempfile::tempdir().unwrap();
    let p = project();
    let (mut h, mut proc, mut ring) = rig(&p);
    h.begin_recording(dir.path()).unwrap();
    h.set_recording(true);
    h.play();
    let block = 480;
    let x = signal(SR as usize * 12, 3);
    let ring_frames = SR as usize * cantus_engine::recorder::RING_SECONDS;
    let stall = SR as usize * 10;
    assert_eq!(h.record_dropped(), 0);
    h.hold_record_writer(true);
    drive(&mut h, &mut proc, &mut ring, &x[..stall], block);
    // A block's loss is marked at the next block's start.
    drive(
        &mut h,
        &mut proc,
        &mut ring,
        &x[stall..stall + block],
        block,
    );
    let lost = stall + block - ring_frames;
    assert_eq!(
        h.record_dropped(),
        (stall - ring_frames) as u64,
        "known while recording"
    );
    h.hold_record_writer(false);
    // Let the writer catch up before the next blocks (a disk that came back).
    let since = std::time::Instant::now();
    let mut written = 0;
    while written < ring_frames as u64 && since.elapsed().as_secs() < 10 {
        for lp in h.poll_recording() {
            written = written.max(lp.frames);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    drive(&mut h, &mut proc, &mut ring, &x[stall + block..], block);
    h.stop();
    drive(&mut h, &mut proc, &mut ring, &[0.0; 480], block);
    let take = h.finish_recording().remove(0);
    assert_eq!(take.dropped, lost as u64);
    assert_eq!(
        take.frames,
        x.len() as u64,
        "the take keeps the pass's length"
    );
    let file = read_wav(&take.path);
    assert_eq!(file.len(), x.len());
    assert_eq!(file[..ring_frames], x[..ring_frames]);
    assert!(file[ring_frames..ring_frames + lost]
        .iter()
        .all(|v| *v == 0.0));
    assert_eq!(
        file[ring_frames + lost..],
        x[ring_frames + lost..],
        "what follows the gap is where it was played"
    );
}
