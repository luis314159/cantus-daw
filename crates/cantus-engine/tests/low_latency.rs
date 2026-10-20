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

// ---- review 1 ---------------------------------------------------------------------------

/// Play `input` (mono) into the voice, stopped, monitoring, and return the left output.
fn monitor(p: &Project, low: Option<u32>, input: &[f32]) -> (Vec<f32>, EngineStats) {
    let block = 256;
    let (mut h, mut proc, mut ring) = rig(p);
    h.send(EngineMessage::SetLowLatency(low));
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 4], block);
    let mut out = Vec::with_capacity(input.len());
    let mut buf = vec![0f32; block * 2];
    for chunk in input.chunks(block) {
        for x in chunk {
            ring.push(*x).unwrap();
        }
        proc.process(&mut buf[..chunk.len() * 2], 2);
        h.poll();
        out.extend(buf[..chunk.len() * 2].iter().step_by(2));
    }
    (out, *proc.stats())
}

/// The voice through Tonus (128 samples, under the limit) into a bus; the backing (a
/// linear-phase EQ, far over it) into the master, or into the same bus (`mixed`).
fn through_a_bus(mixed: bool) -> Project {
    let mut p = Project::new("bus");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Voice");
    t.input = Some(TrackInput::mono(0));
    t.armed = true;
    p.tracks.push(t);
    let backing = p.make_track("Backing");
    p.tracks.push(backing);
    let voice = p.tracks[0].strip.unwrap();
    let back = p.tracks[1].strip.unwrap();
    let bus = p.strips[6].id;
    let tonus = p.make_effect(EffectKind::AutoTune);
    p.strip_mut(voice).unwrap().effects.push(tonus);
    p.strip_mut(voice).unwrap().output = Some(bus);
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec.set("mode", 2.0).unwrap();
    p.strip_mut(back).unwrap().effects.push(eq);
    if mixed {
        p.strip_mut(back).unwrap().output = Some(bus);
    }
    p
}

/// Where an impulse at frame 2 000 of the input comes out, frames after it.
fn heard_after(p: &Project) -> (usize, u64) {
    let mut x = vec![0f32; 24_000];
    x[2_000] = 1.0;
    let (out, stats) = monitor(p, Some(144), &x);
    let at = out
        .iter()
        .position(|v| v.abs() > 1e-4)
        .expect("the voice is heard");
    (at - 2_000, stats.low_latency.monitored_samples)
}

#[test]
fn the_monitored_wait_is_measured_through_the_buses_it_goes_by() {
    // A bus of monitored input only: not held back, the voice heard after Tonus.
    let (heard, said) = heard_after(&through_a_bus(false));
    assert!((100..200).contains(&heard), "Tonus's latency: {heard}");
    assert_eq!(said as usize, heard, "the figure is what is heard");
    // A bus that mixes it with the backing: it waits there with the backing.
    let (heard, said) = heard_after(&through_a_bus(true));
    assert!(heard > 4_000, "with the backing's EQ: {heard}");
    assert_eq!(said as usize, heard, "the figure is what is heard");
}

#[test]
fn an_insert_back_from_a_hard_bypass_holds_nothing_from_before() {
    let p = project(false);
    let block = 256;
    let (mut h, mut proc, mut ring) = rig(&p);
    // Off: the voice's linear-phase EQ hears a noise.
    let mut rng = 1u32;
    let noise: Vec<f32> = (0..block * 40)
        .map(|_| {
            rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (rng >> 8) as f32 / (1u32 << 24) as f32 - 0.5
        })
        .collect();
    drive(&mut h, &mut proc, &mut ring, &noise, block);
    // On: bypassed hard while silence comes in; then off again.
    h.send(EngineMessage::SetLowLatency(Some(144)));
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 40], block);
    assert!(proc.stats().low_latency.off_count >= 1);
    h.send(EngineMessage::SetLowLatency(None));
    let mut out = vec![0f32; block * 2];
    for _ in 0..40 {
        for _ in 0..block {
            ring.push(0.0).unwrap();
        }
        proc.process(&mut out, 2);
        h.poll();
        assert!(
            out.iter().all(|v| *v == 0.0),
            "no noise from before the bypass"
        );
    }
}

#[test]
fn an_armed_instrument_track_is_monitored_with_low_latency() {
    let mut p = project(true);
    // The voice disarmed; a synth armed (no audio input) through a linear-phase EQ.
    p.tracks[0].armed = false;
    let mut synth = p.make_track("Synth");
    synth.armed = true;
    let s = synth.strip.unwrap();
    p.tracks.push(synth);
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec.set("mode", 2.0).unwrap();
    p.strip_mut(s).unwrap().effects.push(eq);
    let (_, stats) = monitor(&p, Some(144), &vec![0.0; 4096]);
    let ll = stats.low_latency;
    assert!(ll.monitoring, "its notes are monitored");
    assert!(ll.off_count >= 2, "its EQ and the master's limiter");
    assert_eq!(ll.off_total, ll.off_count as u16);
    assert_eq!(ll.monitored_samples, 0, "its instrument has no latency");
}

#[test]
fn an_insert_after_a_hard_bypassed_one_is_dated_without_it() {
    let mut p = project(false);
    let voice = p.tracks[0].strip.unwrap();
    let delay = p.make_effect(EffectKind::Delay);
    let id = delay.id;
    p.strip_mut(voice).unwrap().effects.push(delay);
    let (mut h, mut proc, mut ring) = rig(&p);
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; 1024], 256);
    assert!(proc.effect_late(id).unwrap() > 1000, "after the EQ");
    h.send(EngineMessage::SetLowLatency(Some(144)));
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; 1024], 256);
    assert_eq!(
        proc.effect_late(id),
        Some(0),
        "the EQ bypassed delays nothing"
    );
}

#[test]
fn every_bypass_is_counted_past_the_names_kept() {
    let mut p = project(false);
    let voice = p.tracks[0].strip.unwrap();
    for _ in 0..10 {
        let mut eq = p.make_effect(EffectKind::Eq);
        eq.spec.set("mode", 2.0).unwrap();
        p.strip_mut(voice).unwrap().effects.push(eq);
    }
    let (_, stats) = monitor(&p, Some(144), &vec![0.0; 1024]);
    let ll = stats.low_latency;
    assert_eq!(ll.off_count as usize, ll.off.len());
    assert!(ll.off_total as usize > ll.off.len(), "{}", ll.off_total);
}

// ---- review 2 ---------------------------------------------------------------------------

#[test]
fn an_insert_after_a_hard_bypassed_one_keeps_its_tail_when_the_chain_is_edited() {
    // The voice's strip: [compressor, linear-phase EQ (bypassed hard), delay].
    let mut p = project(false);
    let voice = p.tracks[0].strip.unwrap();
    let comp = p.make_effect(EffectKind::Compressor);
    let mut delay = p.make_effect(EffectKind::Delay);
    delay.spec.set("time_ms", 50.0).unwrap();
    delay.spec.set("feedback", 0.9).unwrap();
    delay.spec.set("mix", 1.0).unwrap();
    let s = p.strip_mut(voice).unwrap();
    s.effects.insert(0, comp);
    s.effects.push(delay);
    let block = 256;
    let (mut h, mut proc, mut ring) = rig(&p);
    h.send(EngineMessage::SetLowLatency(Some(144)));
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 4], block);
    let mut x = vec![0f32; block * 8];
    x[10] = 1.0;
    drive(&mut h, &mut proc, &mut ring, &x, block);
    // The compressor taken out while the delay rings.
    let mut q = p.clone();
    q.strip_mut(voice).unwrap().effects.remove(0);
    let snap = h.compile(&q, 2);
    h.set_snapshot(snap);
    let mut out = vec![0f32; block * 2];
    let mut heard = 0.0f32;
    for _ in 0..16 {
        for _ in 0..block {
            ring.push(0.0).unwrap();
        }
        proc.process(&mut out, 2);
        h.poll();
        heard = out.iter().fold(heard, |m, v| m.max(v.abs()));
    }
    assert!(heard > 0.01, "the echoes go on: {heard}");
}

/// The voice through Tonus into a bus of its own, into a group that mixes it with the
/// backing (a linear-phase EQ): two hops of monitored input only, then a mixed bus.
fn two_hops() -> Project {
    let mut p = through_a_bus(true);
    let vox = p.strips[6].id;
    let group = p.strips[7].id;
    let back = p.tracks[1].strip.unwrap();
    p.strip_mut(vox).unwrap().output = Some(group);
    p.strip_mut(back).unwrap().output = Some(group);
    p
}

#[test]
fn a_voice_two_buses_down_waits_with_the_backing_it_is_mixed_with() {
    let p = two_hops();
    let mut x = vec![0f32; 24_000];
    x[2_000] = 1.0;
    let (out, stats) = monitor(&p, Some(144), &x);
    let at = out.iter().position(|v| v.abs() > 1e-4).expect("heard") - 2_000;
    assert_eq!(
        at as u64, stats.latency_samples,
        "it reaches the master with the backing"
    );
    assert_eq!(
        stats.low_latency.monitored_samples as usize, at,
        "the figure"
    );
}

#[test]
fn a_take_follows_the_backing_when_its_heard_latency_moves_mid_pass() {
    // The backing through a linear-phase EQ, the voice armed with nothing on its way.
    let mut p = project(false);
    let voice = p.tracks[0].strip.unwrap();
    p.strip_mut(voice).unwrap().effects.clear();
    let back = p.tracks[1].strip.unwrap();
    let block = 256;
    let dir = tempfile::tempdir().unwrap();
    let (mut h, mut proc, mut ring) = rig(&p);
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 20], block);
    let h0 = proc.stats().latency_samples as usize;
    assert!(h0 > 1000);
    h.begin_recording(dir.path()).unwrap();
    h.set_recording(true);
    h.play();
    // Played on beat 2 as heard: h0 late; the EQ then goes, and beat 4 is played as heard
    // without it.
    let (beat2, beat4) = (48_000usize, 96_000usize);
    let mut x = vec![0f32; 70_000];
    x[beat2 + h0] = 1.0;
    drive(&mut h, &mut proc, &mut ring, &x, block);
    let mut q = p.clone();
    q.strip_mut(back).unwrap().effects.clear();
    let snap = h.compile(&q, 2);
    h.set_snapshot(snap);
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block], block);
    let h1 = proc.stats().latency_samples as usize;
    let played = 70_000 + block;
    let mut y = vec![0f32; 60_000];
    y[beat4 + h1 - played] = 1.0;
    drive(&mut h, &mut proc, &mut ring, &y, block);
    h.stop();
    drive(&mut h, &mut proc, &mut ring, &vec![0.0; block * 4], block);
    let take = h.finish_recording().remove(0);
    let file = read_wav(&take.path);
    let tpf = 120.0 * PPQ as f64 / (60.0 * SR as f64);
    let at: Vec<f64> = file
        .iter()
        .enumerate()
        .filter(|(_, v)| **v == 1.0)
        .map(|(i, _)| take.start_tick / tpf + i as f64)
        .collect();
    assert_eq!(
        at,
        vec![beat2 as f64, beat4 as f64],
        "each where it was played"
    );
}
