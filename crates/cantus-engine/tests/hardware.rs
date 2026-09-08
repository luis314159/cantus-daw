//! Real-device smoke test (idea.md §6 "real hardware test farm"). Ignored by default
//! because CI runners have no sound card; run with `cargo test -p cantus-engine -- --ignored`.

use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
#[ignore = "needs an audio output device"]
fn plays_two_seconds_on_the_default_device_without_overruns() {
    let mut p = Project::new("hw");
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(8));
    for i in 0..8 {
        let n = p.make_note(Ticks::beats(i), Ticks::EIGHTH, 60 + (i as u8 * 2) % 12, 90);
        c.notes.push(n);
    }
    t.clips.push(c);
    p.tracks.push(t);

    let mut engine =
        EngineHandle::start(&AudioConfig::default()).expect("open default output device");
    assert!(engine.is_live());
    engine.set_snapshot(Arc::new(Snapshot::compile(&p, 1)));
    engine.play();

    let start = Instant::now();
    let mut max_peak = 0f32;
    let mut positions = Vec::new();
    while start.elapsed() < Duration::from_secs(2) {
        engine.poll();
        max_peak = max_peak.max(engine.meters().master[0]);
        positions.push(engine.position());
        std::thread::sleep(Duration::from_millis(10));
    }
    engine.stop();
    engine.poll();
    let stats = *engine.stats();
    eprintln!(
        "device {} @ {} Hz, block {} frames, max {:.0} µs, load {:.1}%, overruns {}, blocks {}",
        engine.device_name,
        engine.sample_rate,
        stats.block_frames,
        stats.max_block_us,
        stats.load * 100.0,
        stats.overruns,
        stats.blocks_processed
    );
    assert!(stats.blocks_processed > 0, "callback never ran");
    assert_eq!(stats.overruns, 0, "audio callback overran its budget");
    assert!(max_peak > 0.01, "no signal reached the master bus");
    assert!(
        positions.last().copied().unwrap_or(Ticks::ZERO) > Ticks::beats(3),
        "playhead did not advance"
    );
    assert_eq!(engine.send_failures(), 0);
}
