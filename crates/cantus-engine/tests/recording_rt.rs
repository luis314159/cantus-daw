//! The audio thread's rules hold while recording (ADR 0005, ADR 0025 §8): monitoring,
//! writing armed tracks into their rings (and overflowing one), loop wraps mid-pass and
//! audio clips playing — not one allocation or free in `Processor::process`.
//!
//! The file installs a counting allocator that counts per thread, like `rt_memory.rs`.

use cantus_core::*;
use cantus_engine::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

struct Counting;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static EVENTS: Cell<usize> = const { Cell::new(0) };
}

fn count() {
    let _ = ARMED.try_with(|a| {
        if a.get() {
            let _ = EVENTS.try_with(|n| n.set(n.get() + 1));
        }
    });
}

// SAFETY: every call is forwarded unchanged to the system allocator; the bookkeeping
// touches only const-initialised thread locals, which neither allocate nor panic.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        count();
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn armed(f: impl FnOnce()) -> usize {
    EVENTS.with(|n| n.set(0));
    ARMED.with(|a| a.set(true));
    f();
    ARMED.with(|a| a.set(false));
    EVENTS.with(|n| n.get())
}

#[test]
fn recording_monitoring_and_clips_never_allocate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..96_000 {
        w.write_sample((i as f32 * 0.01).sin() * 0.3).unwrap();
    }
    w.finalize().unwrap();

    let mut p = Project::new("rt");
    p.tempo_bpm = 120.0;
    p.metronome = true;
    p.loop_enabled = true;
    p.loop_region = Some(LoopRegion {
        start: Ticks::ZERO,
        end: Ticks::beats(1),
    });
    let src = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: src,
        name: "a".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 2,
        frames: 48_000,
        sha256: None,
        recorded: false,
    });
    let mut voice = p.make_track("Voice");
    voice.input = Some(TrackInput::stereo(0));
    voice.armed = true;
    let mut clip = p.make_audio_clip("a", src, Ticks(10), Ticks::beats(2));
    clip.audio.as_mut().unwrap().fade_in = Ticks(50);
    voice.clips.push(clip);
    p.tracks.push(voice);
    let mut guitar = p.make_track("Guitar");
    guitar.input = Some(TrackInput::mono(1));
    guitar.armed = true;
    p.tracks.push(guitar);
    let ids = [p.tracks[0].id, p.tracks[1].id];

    let mut snap = Snapshot::compile(&p, 1);
    assert!(SampleCache::new()
        .attach_audio(&mut snap, 48_000)
        .is_empty());
    let snap = Arc::new(snap);

    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    let (mut input, consumer) = rtrb::RingBuffer::<f32>::new(48_000 * 2);
    proc.attach_input(consumer, 2);
    // A roomy ring for one track, a tiny one (it will overflow) for the other.
    let (big, _big_out) = rtrb::RingBuffer::<f32>::new(48_000 * 8);
    let (small, _small_out) = rtrb::RingBuffer::<f32>::new(300);
    // The small one announces its gaps, as the handle's rings do.
    let (gaps, _gaps_out) = rtrb::RingBuffer::<(u64, u64)>::new(4);
    let mut small = cantus_engine::recorder::TrackRing::new(ids[1], 1, small);
    small.gaps = Some(gaps);
    let rings = RecordRings {
        tracks: vec![
            cantus_engine::recorder::TrackRing::new(ids[0], 2, big),
            small,
        ],
    };
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::AttachRecorder(Box::new(rings)))
        .unwrap();
    let mut out = vec![0f32; 512 * 2];
    // Settle: the snapshot, the rings, delay lines — everything that arrives once.
    for _ in 0..4 {
        for i in 0..512 * 2 {
            input.push((i as f32 * 0.001).sin()).unwrap();
        }
        proc.process(&mut out, 2);
        while rx.pop().is_ok() {}
    }
    tx.push(EngineMessage::SetRecording(true)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    // Three seconds: three loop wraps, the clip and its fade, the click, both rings.
    let mut events = 0;
    for _ in 0..280 {
        for i in 0..512 * 2 {
            input.push((i as f32 * 0.001).cos() * 0.5).unwrap();
        }
        events += armed(|| proc.process(&mut out, 2));
        while rx.pop().is_ok() {}
    }
    tx.push(EngineMessage::Stop).unwrap();
    for i in 0..512 * 2 {
        input.push(i as f32 * 0.0).unwrap();
    }
    events += armed(|| proc.process(&mut out, 2));
    assert_eq!(events, 0, "the audio thread allocated or freed");
    assert!(
        proc.stats().record_overflow > 0,
        "the small ring overflowed"
    );
}
