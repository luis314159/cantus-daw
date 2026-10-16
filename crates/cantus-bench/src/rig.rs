//! A processor driven the way the audio callback drives it, timed block by block.
//!
//! The setup is the offline renderer's (`cantus_engine::render`): every effect's unit and
//! every native instrument's unit sent before the snapshot, delay-compensation lines and
//! effect memory answered before the first block. Then the transport plays and each
//! block's wall time is taken around `Processor::process` — the same span the engine's
//! own histogram measures, from outside, so the two can be checked against each other.
//!
//! Blocks run back to back, as fast as the CPU allows. A real callback has idle gaps
//! between blocks (caches cool, the core may clock down), so these numbers are a lower
//! bound on a live block's time; `docs/perf/benchmark.md` §5 says how the live figures
//! are taken.

use cantus_core::{Project, Snapshot, Ticks};
use cantus_engine::profile::Node;
use cantus_engine::{inst, EffectMemory, EffectUnit, EngineEvent, EngineMessage, Processor};
use std::sync::Arc;
use std::time::Instant;

pub const SAMPLE_RATE: f64 = 48_000.0;

pub struct Rig {
    pub processor: Processor,
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    buf: Vec<f32>,
    pub frames: usize,
    /// The project playing, to name the profiler's nodes.
    pub project: Project,
}

/// What a timed run measured.
#[derive(Clone, Debug)]
pub struct Run {
    /// Each block's wall time, µs.
    pub block_us: Vec<f64>,
    pub frames: usize,
    /// Voices sounding at the end (from the engine's stats).
    pub voices: usize,
    /// The thread's CPU seconds over the timed blocks (`crate::cpu`), when the OS says.
    pub cpu_seconds: Option<f64>,
}

impl Run {
    /// The block's budget at the rig's rate, µs.
    pub fn budget_us(&self) -> f64 {
        self.frames as f64 / SAMPLE_RATE * 1e6
    }

    /// Total audio time this run rendered, seconds.
    pub fn audio_seconds(&self) -> f64 {
        (self.block_us.len() * self.frames) as f64 / SAMPLE_RATE
    }

    pub fn wall_seconds(&self) -> f64 {
        self.block_us.iter().sum::<f64>() / 1e6
    }

    /// CPU µs per block: the work, without the time spent waiting for a core.
    pub fn cpu_us_per_block(&self) -> Option<f64> {
        self.cpu_seconds
            .map(|c| c * 1e6 / self.block_us.len().max(1) as f64)
    }

    /// The share of a core the run got, percent (100 on an idle machine).
    pub fn cpu_share_pct(&self) -> Option<f64> {
        self.cpu_seconds
            .map(|c| c / self.wall_seconds().max(1e-9) * 100.0)
    }
}

impl Rig {
    /// A processor loaded with `project`, positioned at `start` and playing, with blocks of
    /// `frames`. Loop the project yourself (`Project::loop_region`) if the run is longer
    /// than the music.
    pub fn new(project: &Project, start: Ticks, frames: usize) -> Rig {
        Self::with_workers(project, start, frames, cantus_engine::Workers::from_env())
    }

    /// [`Rig::new`] with the pool's live workers given (`CANTUS_WORKERS` otherwise).
    pub fn with_workers(
        project: &Project,
        start: Ticks,
        frames: usize,
        workers: cantus_engine::Workers,
    ) -> Rig {
        let snap = Arc::new(Snapshot::compile(project, 1));
        let effects: usize = snap.effect_slots().count();
        // A Chorda's keyboard designed before its unit is made: a unit never waits for
        // it, so notes held from tick 0 would find none and the run would time a silent
        // piano (sound review 2).
        for t in &snap.tracks {
            if let Some(n) = t.instrument.native() {
                if n.kind == cantus_core::InstrumentKind::Chorda {
                    let c = &inst::chorda::values_of(n);
                    inst::chorda::ensure(inst::chorda::tuning_of(c), SAMPLE_RATE as f32);
                }
            }
        }
        let instruments = inst::units_for(&snap, SAMPLE_RATE as f32, |_, _| false);
        let (mut tx, rx_engine) =
            rtrb::RingBuffer::<EngineMessage>::new(64 + effects + instruments.len());
        let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        let mut p = Processor::without_midi(SAMPLE_RATE, rx_engine, tx_engine);
        p.set_workers(workers);
        for slot in snap.effect_slots() {
            let unit = EffectUnit::new(slot, SAMPLE_RATE as f32);
            tx.push(EngineMessage::AddEffect(Box::new(unit))).ok();
        }
        for unit in instruments {
            tx.push(EngineMessage::AddInstrument(unit)).ok();
        }
        tx.push(EngineMessage::SetSnapshot(snap)).ok();
        p.settle();
        while let Ok(ev) = rx.pop() {
            match ev {
                EngineEvent::PdcNeed { count, delay } => {
                    let lines = cantus_engine::pdc::lines_for(count, delay);
                    tx.push(EngineMessage::PdcLines(lines)).ok();
                    p.settle();
                }
                EngineEvent::EffectNeed { effect, need } => {
                    let memory = EffectMemory::build(need, SAMPLE_RATE as f32);
                    tx.push(EngineMessage::EffectMemory { effect, memory }).ok();
                    p.settle();
                }
                _ => {}
            }
        }
        tx.push(EngineMessage::Seek(start)).ok();
        tx.push(EngineMessage::Play).ok();
        Rig {
            processor: p,
            tx,
            rx,
            buf: vec![0.0; frames * 2],
            frames,
            project: project.clone(),
        }
    }

    /// Send the engine a message (a live note, a stop).
    pub fn send(&mut self, msg: EngineMessage) {
        let _ = self.tx.push(msg);
    }

    /// Process one block and return its wall time, µs.
    #[inline]
    pub fn block(&mut self) -> f64 {
        let t = Instant::now();
        self.processor.process(&mut self.buf, 2);
        let us = t.elapsed().as_secs_f64() * 1e6;
        // Keep the event ring from filling; what is in it is of no interest here.
        while self.rx.pop().is_ok() {}
        us
    }

    /// The last block's output, interleaved stereo.
    pub fn output(&self) -> &[f32] {
        &self.buf
    }

    /// Run `warm` blocks untimed, then `blocks` timed; the profiler's totals cover the
    /// timed ones only.
    pub fn run(&mut self, warm: usize, blocks: usize) -> Run {
        for _ in 0..warm {
            self.block();
        }
        self.processor.profiler_mut().reset_totals();
        let (block_us, _, cpu_seconds) =
            crate::cpu::timed(|| (0..blocks).map(|_| self.block()).collect());
        Run {
            block_us,
            frames: self.frames,
            voices: self.processor.stats().active_voices,
            cpu_seconds,
        }
    }

    /// Where the timed blocks' time went: each node in the project's words, its share of
    /// the time and µs per block, the heaviest first.
    pub fn hot_spots(&self, session: &str, top: usize) -> Vec<crate::report::HotSpot> {
        let prof = self.processor.profiler();
        let total = prof.total_ns().max(1) as f64;
        let blocks = prof.total_blocks().max(1) as f64;
        let mut v: Vec<(Node, u64)> = prof.totals().collect();
        v.sort_by_key(|n| std::cmp::Reverse(n.1));
        v.into_iter()
            .take(top)
            .map(|(node, ns)| crate::report::HotSpot {
                session: session.to_string(),
                node: self.name(node),
                share_pct: ns as f64 / total * 100.0,
                us_per_block: ns as f64 / 1e3 / blocks,
            })
            .collect()
    }

    /// A node in the project's words.
    pub fn name(&self, node: Node) -> String {
        cantus_ui::perf_status::culprit_text(&self.project, self.processor.culprit(node))
    }
}
