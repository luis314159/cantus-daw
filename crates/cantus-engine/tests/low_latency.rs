//! Low-latency monitoring (round 29, ADR 0064 §4): what it bypasses and names, what it
//! does not hold back, and that a take recorded with it lands where it lands without it —
//! placed by the latency captured when the pass started.

use cantus_core::*;
use cantus_engine::*;

const SR: u32 = 48_000;

/// A singer (armed, monitored, a linear-phase EQ on their strip) and a backing track whose
/// strip has a linear-phase EQ as well; the master with a look-ahead limiter.
fn project(master_limiter: bool) -> Project {
    let mut p = Project::new("rec");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    t.input = Some(TrackInput::mono(0));
    t.armed = true;
    p.tracks.push(t);
    let backing = p.make_track("Backing");
    p.tracks.push(backing);
    for i in 0..2 {
        let mut eq = p.make_effect(EffectKind::Eq);
        eq.spec.set("mode", 2.0).unwrap();
        let s = p.tracks[i].strip.unwrap();
        p.strip_mut(s).unwrap().effects.push(eq);
    }
    if master_limiter {
        let mut lim = p.make_effect(EffectKind::Limiter);
        lim.spec.set("look", 5.0).unwrap();
        p.master.effects.push(lim);
    }
    p
}

fn rig(p: &Project) -> (EngineHandle, Processor, rtrb::Producer<f32>) {
    let (mut h, mut proc) = EngineHandle::manual(SR as f64);
    let (input, consumer) = rtrb::RingBuffer::<f32>::new(SR as usize * 4);
    proc.attach_input(consumer, 1);
    let snap = h.compile(p, 1);
    h.set_snapshot(snap);
    (h, proc, input)
}

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

/// Record a click played on beat 2; `low` before the pass, `then` switched in mid-pass.
/// Returns where the click lands on the timeline (frames) and the stats at the end.
fn record(p: &Project, low: Option<u32>, then: Option<Option<u32>>) -> (f64, EngineStats) {
    let block = 256;
    let dir = tempfile::tempdir().unwrap();
    let (mut h, mut proc, mut ring) = rig(p);
    h.send(EngineMessage::SetLowLatency(low));
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 20], block);
    h.begin_recording(dir.path()).unwrap();
    h.set_recording(true);
    h.play();
    let beat2 = 48_000usize;
    let mut x = vec![0f32; 100_000];
    x[beat2] = 1.0;
    let (a, b) = x.split_at(60_000);
    drive(&mut h, &mut proc, &mut ring, a, block);
    if let Some(l) = then {
        h.send(EngineMessage::SetLowLatency(l));
    }
    drive(&mut h, &mut proc, &mut ring, b, block);
    h.stop();
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 4], block);
    let stats = *proc.stats();
    let take = h.finish_recording().remove(0);
    let file = read_wav(&take.path);
    let at = file.iter().position(|v| *v == 1.0).unwrap();
    let tpf = 120.0 * PPQ as f64 / (60.0 * SR as f64);
    (take.start_tick / tpf + at as f64, stats)
}

#[test]
fn a_take_recorded_with_low_latency_lands_where_it_does_without() {
    let p = project(false);
    let (off, s_off) = record(&p, None, None);
    let (on, s_on) = record(&p, Some(144), None);
    assert!(!s_off.low_latency.on && s_on.low_latency.on);
    assert_eq!(s_on.low_latency.off_count, 1, "the voice's EQ is bypassed");
    assert_eq!(off, on, "the take lands to the sample");
    // Played against a backing heard the project's latency late: moved back by it.
    assert_eq!(
        off,
        48_000.0 - s_off.latency_samples as f64,
        "where it was played, to the frame"
    );
}

#[test]
fn a_take_is_placed_by_the_latency_when_its_pass_started() {
    // The master's look-ahead limiter is over the limit: with the mode on, the backing is
    // heard without it, and the take is placed by that. Switching the mode off mid-pass
    // brings the limiter back (the project's latency grows) — the take must not move.
    let p = project(true);
    let (steady, s) = record(&p, Some(144), None);
    assert!(
        s.low_latency.off_count >= 2,
        "the voice's EQ and the master's limiter"
    );
    let (switched, s2) = record(&p, Some(144), Some(None));
    assert!(
        s2.latency_samples > s.latency_samples,
        "the limiter is back"
    );
    assert_eq!(switched, steady, "placed by the latency at the start");
    assert_eq!(steady, 48_000.0 - s.latency_samples as f64);
}

#[test]
fn the_monitored_path_waits_only_for_its_own_chain() {
    let p = project(false);
    let (mut h, mut proc, mut ring) = rig(&p);
    let block = 256;
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 4], block);
    let lp = proc.stats().latency_samples;
    assert!(lp > 1000, "a linear-phase EQ: {lp}");
    h.send(EngineMessage::SetLowLatency(Some(144)));
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 4], block);
    let ll = proc.stats().low_latency;
    assert!(ll.on && ll.monitoring);
    assert_eq!(
        ll.monitored_samples, 0,
        "its EQ bypassed, nothing else on its way"
    );
    assert_eq!(
        proc.stats().latency_samples,
        lp,
        "the backing keeps its alignment"
    );
    // Disarmed and not monitored *On*: nothing is monitored, nothing bypassed.
    let mut q = p.clone();
    q.tracks[0].armed = false;
    let snap = h.compile(&q, 2);
    h.set_snapshot(snap);
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 4], block);
    let ll = proc.stats().low_latency;
    assert!(ll.on && !ll.monitoring && ll.off_count == 0);
    // Off: as before.
    h.send(EngineMessage::SetLowLatency(None));
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 4], block);
    assert!(!proc.stats().low_latency.on);
}
