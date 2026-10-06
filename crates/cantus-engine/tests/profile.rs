//! The callback histogram and the dropout detector with blame (round 20, bench;
//! `crate::profile`): the profiler changes no sample, every block is counted, and when a
//! block overruns, the node that ran long is the one named — an insert, a master insert,
//! an instrument — by its ids in the project.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::*;

/// An effect that does nothing to the sound and takes `spin` of wall time per call: the
/// node a dropout must be blamed on.
#[derive(Debug)]
struct Slow {
    spin: Duration,
}

impl FxUnit for Slow {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, l: &mut [f32], _r: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        if l.is_empty() {
            return;
        }
        let t = Instant::now();
        while t.elapsed() < self.spin {
            std::hint::spin_loop();
        }
    }
    fn reset(&mut self) {}
}

/// A processor playing `p`, with `slow` effects spinning for `spin` each, and its rings.
fn rig(p: &Project, slow: &[EffectId], spin: Duration) -> Processor {
    let snap = Snapshot::compile(p, 1);
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(256);
    let (tx_engine, _rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
    for slot in snap.effect_slots() {
        let unit = if slow.contains(&slot.id) {
            EffectUnit::from_fx(slot, Box::new(Slow { spin }))
        } else {
            EffectUnit::new(slot, 48_000.0)
        };
        tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
    }
    for unit in cantus_engine::inst::units_for(&snap, 48_000.0, |_, _| false) {
        tx.push(EngineMessage::AddInstrument(unit)).unwrap();
    }
    tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    // The producer must outlive the first block, which drains it.
    proc.process(&mut [0.0; 2], 2);
    drop(tx);
    proc
}

fn play(proc: &mut Processor, blocks: usize, frames: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(blocks * frames * 2);
    let mut buf = vec![0f32; frames * 2];
    for _ in 0..blocks {
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
    }
    out
}

/// A project with work on every kind of node: two instruments, inserts on a strip, and a
/// master chain.
fn busy() -> Project {
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Compressor);
    add_effect(&mut p, EffectKind::Space);
    let lim = p.make_effect(EffectKind::Limiter);
    p.master.effects.push(lim);
    p
}

#[test]
fn profiling_changes_no_sample() {
    let p = busy();
    let mut on = rig(&p, &[], Duration::ZERO);
    let mut off = rig(&p, &[], Duration::ZERO);
    off.set_profiling(false);
    let a = play(&mut on, 400, 128);
    let b = play(&mut off, 400, 128);
    assert_null(&a, &b, 0.0, "profiler on against off");
    assert!(a.iter().any(|x| *x != 0.0), "the project is heard");
    // And the offline render, which runs without the profiler, is the same music (the
    // rig's first block was one frame long, so `a` starts at frame 1).
    let rendered = render_with(&p, 128);
    let m = (rendered.len() - 2).min(a.len());
    assert_null(&a[..m], &rendered[2..m + 2], 0.0, "live against offline");
}

#[test]
fn every_block_is_counted_in_the_histogram() {
    let p = busy();
    let mut proc = rig(&p, &[], Duration::ZERO);
    play(&mut proc, 300, 64);
    let s = *proc.stats();
    assert_eq!(s.histogram.count(), s.blocks_processed);
    assert!(s.histogram.quantile(0.5) > 0.0);
    assert!(s.histogram.quantile(1.0) >= s.histogram.quantile(0.5));
    // Every nanosecond of the measured blocks is charged to some node, and the nodes the
    // project has are among them.
    let prof = proc.profiler();
    assert_eq!(prof.total_blocks(), s.blocks_processed);
    let nodes: Vec<_> = prof.totals().map(|(n, _)| n).collect();
    use cantus_engine::profile::Node;
    assert!(nodes.contains(&Node::Engine));
    assert!(nodes.iter().any(|n| matches!(n, Node::Track(_))));
    assert!(nodes.iter().any(|n| matches!(n, Node::Insert(_, 1))));
    assert!(nodes.iter().any(|n| matches!(n, Node::MasterInsert(0))));
}

#[test]
fn an_overrun_is_blamed_on_the_insert_that_ran_long() {
    let mut p = busy();
    let slow = add_effect(&mut p, EffectKind::Gate);
    let strip = p.tracks[0].strip.unwrap();
    // 128 frames at 48 kHz is 2.67 ms: a 4 ms insert overruns every block.
    let mut proc = rig(&p, &[slow], Duration::from_millis(4));
    play(&mut proc, 3, 128);
    let s = *proc.stats();
    assert!(s.overruns >= 3, "{} overruns", s.overruns);
    let d = s.last_dropout.expect("a dropout is reported");
    assert_eq!(
        d.culprit,
        Culprit::Insert {
            strip,
            effect: slow,
            index: 2
        }
    );
    assert!(d.culprit_us >= 3_900.0, "{} µs", d.culprit_us);
    assert!(d.block_us >= d.culprit_us && d.block_us > d.budget_us);
    assert!((d.budget_us - 2_666.7).abs() < 1.0, "{}", d.budget_us);
    assert_eq!(d.block, s.blocks_processed - 1);
}

#[test]
fn an_overrun_on_the_master_is_blamed_on_the_master() {
    let mut p = busy();
    let slot = p.make_effect(EffectKind::Stereo);
    let slow = slot.id;
    p.master.effects.push(slot);
    let mut proc = rig(&p, &[slow], Duration::from_millis(4));
    play(&mut proc, 2, 128);
    let d = proc.stats().last_dropout.expect("a dropout is reported");
    assert_eq!(
        d.culprit,
        Culprit::MasterInsert {
            effect: slow,
            index: 1
        }
    );
}

#[test]
fn with_the_profiler_off_overruns_are_counted_but_nobody_is_blamed() {
    let mut p = busy();
    let slow = add_effect(&mut p, EffectKind::Gate);
    let mut proc = rig(&p, &[slow], Duration::from_millis(4));
    // The rig's first block ran with the profiler on; what it blamed stays as it was.
    let before = proc.stats().last_dropout;
    proc.set_profiling(false);
    play(&mut proc, 2, 128);
    let s = proc.stats();
    assert!(s.overruns >= 2);
    assert_eq!(s.last_dropout, before, "nobody new is blamed");
    assert_eq!(s.histogram.count(), s.blocks_processed);
}
