//! Delay compensation, proven with a unit that is nothing but a delay: whatever the
//! routing, every path reaches the master at the same time — so a track against its own
//! inverted copy through a latent strip is exact silence, a slot mix around a latent effect
//! does not comb, and a latent return lines up with the dry signal.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::{assert_null, peak};
use cantus_engine::*;
use std::sync::Arc;
use support::*;

const DELAY: usize = 100;

/// A pure delay of `n` samples that says so.
#[derive(Debug)]
struct PureDelay {
    l: Vec<f32>,
    r: Vec<f32>,
    pos: usize,
}

impl PureDelay {
    fn new(n: usize) -> Self {
        PureDelay {
            l: vec![0.0; n],
            r: vec![0.0; n],
            pos: 0,
        }
    }
}

impl FxUnit for PureDelay {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, l: &mut [f32], r: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        for i in 0..l.len() {
            let (ol, or) = (self.l[self.pos], self.r[self.pos]);
            self.l[self.pos] = l[i];
            self.r[self.pos] = r[i];
            self.pos = (self.pos + 1) % self.l.len();
            l[i] = ol;
            r[i] = or;
        }
    }
    fn reset(&mut self) {
        self.l.iter_mut().for_each(|x| *x = 0.0);
        self.r.iter_mut().for_each(|x| *x = 0.0);
    }
    fn latency(&self) -> usize {
        self.l.len()
    }
}

/// Play a project through a processor whose `latent` slots are pure delays, at irregular
/// block sizes. Returns the output and the latency the engine reported.
fn play(p: &Project, latent: &[EffectId], seconds: f64) -> (Vec<f32>, u64) {
    play_with(p, latent, DELAY, seconds, true)
}

/// [`play`] with a delay of `delay` samples. Requests for longer lines are answered the way
/// the backend does, after the block that raised them — or, with `settle`, before the first
/// block, the way an offline render does.
fn play_with(
    p: &Project,
    latent: &[EffectId],
    delay: usize,
    seconds: f64,
    settle: bool,
) -> (Vec<f32>, u64) {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    let mut snap = Snapshot::compile(p, 1);
    snap.loop_region = None;
    for strip in &snap.strips {
        for slot in &strip.effects {
            let unit = if latent.contains(&slot.id) {
                EffectUnit::from_fx(slot, Box::new(PureDelay::new(delay)))
            } else {
                EffectUnit::new(slot, 48_000.0)
            };
            tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
        }
    }
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    let answer = |rx: &mut rtrb::Consumer<EngineEvent>, tx: &mut rtrb::Producer<EngineMessage>| {
        let mut answered = false;
        while let Ok(ev) = rx.pop() {
            if let EngineEvent::PdcNeed { count, delay } = ev {
                let lines = cantus_engine::pdc::lines_for(count, delay);
                tx.push(EngineMessage::PdcLines(lines)).unwrap();
                answered = true;
            }
        }
        answered
    };
    if settle {
        proc.settle();
        while answer(&mut rx, &mut tx) {
            proc.settle();
        }
    }
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let total = (seconds * 48_000.0) as usize;
    let mut out = Vec::new();
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut i = 0;
    while out.len() / 2 < total {
        let n = sizes[i % sizes.len()].min(total - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        answer(&mut rx, &mut tx);
    }
    (out, proc.stats().latency_samples)
}

/// The lead twice, each copy on a strip of its own; the bass muted.
fn twins() -> (Project, StripId, StripId) {
    let mut p = demo_project();
    p.tracks[1].muted = true;
    let mut copy = p.tracks[0].clone();
    copy.id = TrackId(p.alloc_id());
    let s2 = p.strips[5].id;
    copy.strip = Some(s2);
    p.tracks.push(copy);
    let s1 = p.tracks[0].strip.unwrap();
    // Same level and place for both, so only the latency tells them apart.
    let (g, pan) = {
        let s = p.strip(s1).unwrap();
        (s.gain_db, s.pan)
    };
    p.strip_mut(s2).unwrap().gain_db = g;
    p.strip_mut(s2).unwrap().pan = pan;
    (p, s1, s2)
}

#[test]
fn a_latent_strip_and_its_inverted_twin_cancel_exactly() {
    let (mut p, s1, s2) = twins();
    let slot = p.make_effect(EffectKind::Limiter);
    let id = slot.id;
    p.strip_mut(s1).unwrap().effects.push(slot);
    p.strip_mut(s2).unwrap().phase_invert = true;
    let (out, latency) = play(&p, &[id], 2.0);
    assert_eq!(latency, DELAY as u64, "the engine reports the slowest path");
    assert_eq!(peak(&out), 0.0, "the twins did not line up");
    // And the twin alone, not inverted, is the latent one alone: both arrive 100 late.
    let mut a = p.clone();
    a.strip_mut(s2).unwrap().phase_invert = false;
    a.strip_mut(s2).unwrap().muted = true;
    let mut b = p.clone();
    b.strip_mut(s2).unwrap().phase_invert = false;
    b.strip_mut(s1).unwrap().muted = true;
    let (only_latent, _) = play(&a, &[id], 2.0);
    let (only_twin, _) = play(&b, &[id], 2.0);
    assert!(peak(&only_latent) > 0.05);
    assert_null(
        &only_latent,
        &only_twin,
        0.0,
        "the compensated twin is not the latent one",
    );
}

#[test]
fn a_slot_mix_around_a_latent_effect_does_not_comb() {
    let (mut p, s1, _) = twins();
    p.tracks.pop();
    let slot = p.make_effect(EffectKind::Limiter);
    let id = slot.id;
    p.strip_mut(s1).unwrap().effects.push(slot);
    let (full, _) = play(&p, &[id], 2.0);
    p.strip_mut(s1).unwrap().effects[0].mix = 0.5;
    let (half, _) = play(&p, &[id], 2.0);
    // Wet and dry are the same signal, delayed alike: half and half is the whole.
    assert_null(
        &full,
        &half,
        0.0,
        "the dry path was not delayed with the wet one",
    );
}

#[test]
fn a_latent_return_lines_up_with_the_dry_signal() {
    let (mut p, s1, _) = twins();
    p.tracks.pop();
    p.pan_law = PanLaw::Minus6dB;
    let ret = p.strips[7].id;
    let slot = p.make_effect(EffectKind::Limiter);
    let id = slot.id;
    p.strip_mut(ret).unwrap().effects.push(slot);
    p.strip_mut(s1).unwrap().sends.push(Send::new(ret));
    // The return inverted: the dry path and the latent return cancel if and only if they
    // arrive together (the return's strip at centre through the −6 dB law is × 0.5, so
    // its fader makes up the difference exactly).
    p.strip_mut(ret).unwrap().phase_invert = true;
    p.strip_mut(ret).unwrap().gain_db = 20.0 * math::log10(2f32);
    let (out, latency) = play(&p, &[id], 2.0);
    assert_eq!(latency, DELAY as u64);
    let (dry, _) = {
        let mut q = p.clone();
        q.strip_mut(s1).unwrap().sends.clear();
        play(&q, &[id], 2.0)
    };
    assert!(peak(&dry) > 0.05);
    assert!(
        peak(&out) < peak(&dry) * 1e-3,
        "the latent return did not line up: {} left of {}",
        peak(&out),
        peak(&dry)
    );
}

#[test]
fn a_project_with_no_latency_is_untouched_by_compensation() {
    let p = demo_project();
    let (out, latency) = play(&p, &[], 2.0);
    assert_eq!(latency, 0);
    let offline = render_with(&p, 512);
    assert_null(
        &offline[..out.len()],
        &out,
        0.0,
        "compensation changed a project with none",
    );
}

/// A linear-phase EQ's latency is far longer than the lines the pool starts with; the
/// engine asks for longer ones and, once they arrive, the twins cancel exactly again.
#[test]
fn a_latency_longer_than_the_start_up_lines_grows_the_pool_and_lines_up() {
    let long = cantus_engine::pdc::START_LEN * 2 + 500;
    let (mut p, s1, s2) = twins();
    let slot = p.make_effect(EffectKind::Limiter);
    let id = slot.id;
    p.strip_mut(s1).unwrap().effects.push(slot);
    p.strip_mut(s2).unwrap().phase_invert = true;
    // Offline: settled before the first block, exact from the first sample.
    let (out, latency) = play_with(&p, &[id], long, 2.0, true);
    assert_eq!(latency, long as u64);
    assert_eq!(
        peak(&out),
        0.0,
        "the offline render was not compensated throughout"
    );
    // Live: the first block plays before the lines arrive; after one latency it is exact.
    let (out, _) = play_with(&p, &[id], long, 2.0, false);
    let settled = (long + 2048) * 2;
    assert!(
        peak(&out[..settled]) > 0.0,
        "nothing to settle: the test proves nothing"
    );
    assert_eq!(peak(&out[settled..]), 0.0, "the pool never grew");
}

/// Play `a`, switching to `b` and back at each sample in `at` (the switch lands exactly
/// there: the block is cut at it), with the `latent` slots as pure delays. Returns the
/// output and the latency the engine reported after each switch.
fn play_switching(
    a: &Project,
    b: &Project,
    latent: &[EffectId],
    at: &[usize],
    seconds: f64,
) -> (Vec<f32>, Vec<u64>) {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    let compile = |p: &Project| {
        let mut snap = Snapshot::compile(p, 1);
        snap.loop_region = None;
        Arc::new(snap)
    };
    let first = compile(a);
    for strip in &first.strips {
        for slot in &strip.effects {
            let unit = if latent.contains(&slot.id) {
                EffectUnit::from_fx(slot, Box::new(PureDelay::new(DELAY)))
            } else {
                EffectUnit::new(slot, 48_000.0)
            };
            tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
        }
    }
    tx.push(EngineMessage::SetSnapshot(first)).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let total = (seconds * 48_000.0) as usize;
    let mut out = Vec::new();
    let mut latencies = Vec::new();
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let (mut i, mut next, mut on_b) = (0, 0, false);
    while out.len() / 2 < total {
        let done = out.len() / 2;
        if next < at.len() && at[next] == done {
            on_b = !on_b;
            let snap = compile(if on_b { b } else { a });
            tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
            next += 1;
        }
        let limit = at.get(next).copied().unwrap_or(total).min(total);
        let n = sizes[i % sizes.len()].min(limit - done);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
        if next > 0 && latencies.len() < next {
            latencies.push(proc.stats().latency_samples);
        }
    }
    (out, latencies)
}

/// Soft bypass (ADR 0022): bypassing a latent insert keeps its latency — its slot passes
/// the input delayed by exactly that much — so compensation does not re-plan, and a track
/// against its inverted twin stays cancelled to the sample across every toggle, the chain
/// switch included.
#[test]
fn bypassing_a_latent_insert_keeps_its_latency_and_nothing_jumps() {
    let (mut p, s1, s2) = twins();
    let slot = p.make_effect(EffectKind::Limiter);
    let id = slot.id;
    p.strip_mut(s1).unwrap().effects.push(slot);
    p.strip_mut(s2).unwrap().phase_invert = true;
    let mut bypassed = p.clone();
    bypassed.strip_mut(s1).unwrap().effects[0].bypass = true;
    let toggles = [12_000, 30_011, 47_999, 70_100];
    let (out, latencies) = play_switching(&p, &bypassed, &[id], &toggles, 2.0);
    assert_eq!(
        latencies,
        vec![DELAY as u64; toggles.len()],
        "a bypass must not change the latency"
    );
    assert_eq!(
        peak(&out),
        0.0,
        "the twins came apart across a bypass toggle"
    );
    // The chain switch is a bypass of every insert: the same holds.
    let mut off = p.clone();
    off.strip_mut(s1).unwrap().fx_enabled = false;
    let (out, latencies) = play_switching(&p, &off, &[id], &toggles, 2.0);
    assert_eq!(latencies, vec![DELAY as u64; toggles.len()]);
    assert_eq!(
        peak(&out),
        0.0,
        "the twins came apart across the chain switch"
    );
    // And the twin alone is not silent: the test hears something.
    let mut alone = p.clone();
    alone.strip_mut(s1).unwrap().muted = true;
    let (out, _) = play_switching(&alone, &alone, &[id], &[], 1.0);
    assert!(peak(&out) > 0.05);
}

/// A unit with no latency that halves the signal: a bypass must take it out to the bit.
#[derive(Debug)]
struct Halve;

impl FxUnit for Halve {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, l: &mut [f32], r: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        l.iter_mut().chain(r.iter_mut()).for_each(|x| *x *= 0.5);
    }
    fn reset(&mut self) {}
}

/// A bypassed insert with no latency is still a wire, bit for bit, and still costs
/// nothing (it does not run) — in a project that has latency elsewhere, too.
#[test]
fn a_bypassed_insert_with_no_latency_is_still_a_wire() {
    let (mut p, s1, s2) = twins();
    let latent = p.make_effect(EffectKind::Limiter);
    let latent_id = latent.id;
    p.strip_mut(s2).unwrap().effects.push(latent);
    let dry = play_switching(&p, &p, &[latent_id], &[], 1.0).0;
    let mut halved = p.clone();
    let mut slot = halved.make_effect(EffectKind::Gate);
    slot.bypass = true;
    let halve_id = slot.id;
    halved.strip_mut(s1).unwrap().effects.push(slot);
    // `play_switching` builds every slot not listed as latent with `EffectUnit::new`;
    // build this one by hand as `Halve` through a local copy of the driver.
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    let snap = Snapshot::compile(&halved, 1);
    for strip in &snap.strips {
        for slot in &strip.effects {
            let unit = if slot.id == latent_id {
                EffectUnit::from_fx(slot, Box::new(PureDelay::new(DELAY)))
            } else if slot.id == halve_id {
                EffectUnit::from_fx(slot, Box::new(Halve))
            } else {
                EffectUnit::new(slot, 48_000.0)
            };
            tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
        }
    }
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::new();
    while out.len() < dry.len() {
        let n = 333.min((dry.len() - out.len()) / 2);
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    assert!(peak(&dry) > 0.05);
    assert_null(
        &out,
        &dry,
        0.0,
        "a bypassed insert with no latency is not a wire",
    );
}
