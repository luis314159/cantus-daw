//! The block's report — meters, stats and the modulators' values — in a mailbox of its own
//! (ADR 0107), out of the event queue.
//!
//! The event queue's slots are as large as its largest kind: with the meters in it (256
//! track and 256 strip peaks, 256 gain reductions, two telemetry blocks) an event was
//! 3 880 B and the queue's 4 096 of them 15.9 MB, for the few bytes most events carry. The
//! report now travels in a few boxes that go round between the two threads, over two
//! `rtrb` rings of pointers: the audio thread fills an empty box and hands it over full;
//! the interface reads it and hands it back empty. Nothing is allocated after
//! construction, nothing locks, and the audio thread never waits.
//!
//! **Nothing is lost when the interface is late.** A block whose report finds no empty box
//! is *folded* into the report being held — the latest block's readings, with the
//! readings the interface takes the highest of (an analysed compressor's peaks and
//! reduction, the inputs' peaks) kept at their highest — and goes with the next block
//! that finds one. The master's true peak starts again only once a report got through,
//! as before (`MasterLoud::sent`). So a reader that drains every report and folds them,
//! as the backend does, sees what it saw when every block had its own event.

use crate::messages::{EngineStats, Meters};
use crate::modulator::ModTelemetry;
use rtrb::{Consumer, Producer, RingBuffer};

/// Boxes going round: one held by the audio thread while it folds, the rest in flight. A
/// reader draining at 60 Hz gets every block on its own down to 128-frame blocks at
/// 48 kHz (6.25 a frame, seven boxes in flight); smaller blocks come folded, which loses
/// nothing a reader folds. About 42 KB in all.
pub const BOXES: usize = 8;

/// One block's report, or several folded.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub meters: Meters,
    pub stats: EngineStats,
    /// The modulators' live values, when there are any (ADR 0057).
    pub modulation: Option<ModTelemetry>,
    /// The blocks this report stands for (1 unless the reader was late).
    pub blocks: u32,
}

impl Report {
    /// Fold a later block in: its readings, and the highest of those a reader folds.
    fn fold(&mut self, meters: &Meters, stats: &EngineStats, modulation: Option<ModTelemetry>) {
        let comp = self.meters.comp;
        let n = self.meters.input_count.min(crate::recorder::MAX_INPUTS);
        let inputs = self.meters.inputs;
        self.meters.clone_from(meters);
        for (a, b) in self.meters.comp.iter_mut().zip(comp) {
            *a = a.max(b);
        }
        let m = self.meters.input_count.min(crate::recorder::MAX_INPUTS);
        for (a, b) in self.meters.inputs[..m.min(n)].iter_mut().zip(inputs) {
            *a = a.max(b);
        }
        self.stats = *stats;
        if modulation.is_some() {
            self.modulation = modulation;
        }
        self.blocks = self.blocks.saturating_add(1);
    }
}

/// The audio thread's end.
pub struct Outbox {
    full: Producer<Box<Report>>,
    empty: Consumer<Box<Report>>,
    /// The report being written: the blocks since the last one got through.
    held: Box<Report>,
}

/// The interface's end.
pub struct Reports {
    full: Consumer<Box<Report>>,
    empty: Producer<Box<Report>>,
}

/// A mailbox: every box built here, off the audio thread.
pub fn mailbox() -> (Outbox, Reports) {
    let (full_tx, full_rx) = RingBuffer::new(BOXES);
    let (mut empty_tx, empty_rx) = RingBuffer::new(BOXES);
    for _ in 1..BOXES {
        let _ = empty_tx.push(Box::<Report>::default());
    }
    (
        Outbox {
            full: full_tx,
            empty: empty_rx,
            held: Box::default(),
        },
        Reports {
            full: full_rx,
            empty: empty_tx,
        },
    )
}

impl Outbox {
    /// The block's report: folded into the one held, which goes out if an empty box is
    /// back. Returns whether it went (the true peak may start again). No allocation.
    pub fn post(
        &mut self,
        meters: &Meters,
        stats: &EngineStats,
        modulation: Option<ModTelemetry>,
    ) -> bool {
        if self.held.blocks == 0 {
            self.held.meters.clone_from(meters);
            self.held.stats = *stats;
            self.held.modulation = modulation;
            self.held.blocks = 1;
        } else {
            self.held.fold(meters, stats, modulation);
        }
        let Ok(mut next) = self.empty.pop() else {
            return false;
        };
        std::mem::swap(&mut next, &mut self.held);
        self.held.blocks = 0;
        // Room for every box there is: never refused.
        match self.full.push(next) {
            Ok(()) => true,
            Err(rtrb::PushError::Full(back)) => {
                self.held = back;
                false
            }
        }
    }
}

impl Reports {
    /// Every report waiting, oldest first, each handed back once read. No allocation.
    pub fn drain(&mut self, mut f: impl FnMut(&Report)) -> usize {
        let mut n = 0;
        while let Ok(r) = self.full.pop() {
            f(&r);
            n += 1;
            let _ = self.empty.push(r);
        }
        n
    }

    /// The latest report waiting, the others read past (a test's look at the meters).
    pub fn latest(&mut self) -> Option<Report> {
        let mut last = None;
        self.drain(|r| last = Some(r.clone()));
        last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(peak: f32, gr: f32) -> Meters {
        let mut m = Meters {
            input_count: 1,
            ..Default::default()
        };
        m.master = [peak; 2];
        m.comp = [peak, peak, gr];
        m.inputs[0] = peak;
        m
    }

    #[test]
    fn every_block_read_in_time_comes_as_it_was() {
        let (mut out, mut reports) = mailbox();
        let s = EngineStats::default();
        assert!(out.post(&block(0.5, 3.0), &s, None));
        assert!(out.post(&block(0.25, 1.0), &s, None));
        let mut got = Vec::new();
        reports.drain(|r| got.push((r.meters.master[0], r.meters.comp[2], r.blocks)));
        assert_eq!(got, vec![(0.5, 3.0, 1), (0.25, 1.0, 1)]);
    }

    /// A reader that is late loses nothing it folds: the blocks that found no box come
    /// folded into the next, the latest readings with the highest peaks.
    #[test]
    fn blocks_a_late_reader_missed_are_folded_into_the_next() {
        let (mut out, mut reports) = mailbox();
        let s = EngineStats::default();
        for _ in 1..BOXES {
            assert!(out.post(&block(0.1, 0.0), &s, None));
        }
        assert!(!out.post(&block(0.9, 6.0), &s, None), "no box free");
        assert!(!out.post(&block(0.2, 1.0), &s, None));
        assert_eq!(reports.drain(|_| {}), BOXES - 1);
        assert!(out.post(&block(0.3, 2.0), &s, None));
        let r = reports.latest().expect("the folded report");
        assert_eq!(r.blocks, 3);
        assert_eq!(r.meters.master[0], 0.3, "the latest block's meters");
        assert_eq!(r.meters.comp, [0.9, 0.9, 6.0], "the highest of the three");
        assert_eq!(r.meters.inputs[0], 0.9);
    }
}
