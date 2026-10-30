//! The session while Link is followed (ADR 0076 §7, the integrated seam 7): a scene's tempo,
//! carried by the projection it plays in (`Snapshot::scene_tempos`), is proposed to the Link
//! session once, timed to when its tick is heard — Live's scene tempo at its launch — and a
//! projection installed again, or the song the performance ends with, proposes nothing.

use cantus_core::*;
use cantus_engine::transport::sync::{self, Proposal, SourceKind, Timeline};
use cantus_engine::*;
use std::cell::Cell;
use std::sync::Arc;

const SR: f64 = 48_000.0;
const BLOCK: usize = 256;
const T0: f64 = 5_000_000.0;

thread_local! {
    static NOW: Cell<i64> = const { Cell::new(0) };
}
fn now() -> i64 {
    NOW.with(|c| c.get())
}

struct Rig {
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    p: Processor,
    h: sync::SyncHandle,
    blocks: u64,
}

impl Rig {
    fn new() -> Rig {
        let mut song = Project::new("song");
        song.tempo_bpm = 120.0;
        let (mut tx, rxe) = rtrb::RingBuffer::new(64);
        let (txe, rx) = rtrb::RingBuffer::new(8192);
        let mut p = Processor::without_midi(SR, rxe, txe);
        let (port, h) = sync::pair();
        p.attach_sync_clocked(port, None, now);
        let _ = tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
            &song, 1,
        ))));
        p.settle();
        Rig {
            tx,
            rx,
            p,
            h,
            blocks: 0,
        }
    }

    fn block(&mut self) {
        let s = (self.blocks * BLOCK as u64) as f64;
        NOW.with(|c| c.set((T0 + s * 1e6 / SR) as i64));
        let mut buf = vec![0f32; BLOCK * 2];
        self.p.process(&mut buf, 2);
        while self.rx.pop().is_ok() {}
        self.blocks += 1;
    }

    fn tempos(&mut self) -> Vec<(f64, i64)> {
        let mut out = Vec::new();
        while let Ok(p) = self.h.proposals.pop() {
            if let Proposal::Tempo { bpm, at } = p {
                out.push((bpm, at));
            }
        }
        out
    }
}

/// A projection whose scene sets 140 BPM at `at`.
fn projection(at: Ticks, rev: u64) -> Arc<Snapshot> {
    let mut p = Project::new("projection");
    p.tempo_bpm = 120.0;
    p.tempo_points = vec![
        cantus_core::tempo::TempoPoint {
            tick: Ticks::ZERO,
            bpm: 120.0,
            ramp: false,
        },
        cantus_core::tempo::TempoPoint {
            tick: at,
            bpm: 140.0,
            ramp: false,
        },
    ];
    let mut s = Snapshot::compile(&p, rev);
    s.scene_tempos = vec![(at, 140.0)];
    Arc::new(s)
}

#[test]
fn a_scenes_tempo_is_proposed_to_link_as_it_is_heard() {
    let mut r = Rig::new();
    r.h.shared.timeline.publish(&Timeline {
        kind: SourceKind::Link,
        tempo: 120.0,
        host: 0,
        position: 0.0,
        rate: 1.0,
        playing: true,
        quantum: 4.0,
        start_stop: true,
        epoch: 1,
    });
    for _ in 0..(4.0 * SR / BLOCK as f64) as usize {
        r.block();
    }
    assert!(r.p.transport().following(), "following Link");
    let _ = r.tempos();
    // The scene's bar four beats on.
    let at = Ticks((r.p.transport().position_f64() / PPQ as f64).ceil() as i64 * PPQ + 4 * PPQ);
    let _ = r.tx.push(EngineMessage::SetSnapshot(projection(at, 2)));
    r.block();
    assert!(r.tempos().is_empty(), "not before its bar");
    let mut proposed = Vec::new();
    let mut heard_at = None;
    for _ in 0..(3.0 * SR / BLOCK as f64) as usize {
        let before = r.p.transport().position_f64();
        let s = (r.blocks * BLOCK as u64) as f64;
        r.block();
        let after = r.p.transport().position_f64();
        if heard_at.is_none() && before <= at.0 as f64 && (at.0 as f64) < after {
            heard_at = Some(s);
        }
        proposed.extend(r.tempos());
        if r.blocks.is_multiple_of(50) {
            // The same projection again (a replan that changed nothing here).
            let _ = r.tx.push(EngineMessage::SetSnapshot(projection(at, 3)));
        }
    }
    assert_eq!(proposed.len(), 1, "once: {proposed:?}");
    assert_eq!(proposed[0].0, 140.0);
    assert!(heard_at.is_some(), "the bar was played");
    // The song the performance ends with: nothing proposed.
    let mut song = Project::new("song");
    song.tempo_bpm = 120.0;
    let _ =
        r.tx.push(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
            &song, 4,
        ))));
    for _ in 0..40 {
        r.block();
    }
    assert!(r.tempos().is_empty(), "the end proposes nothing");
}
