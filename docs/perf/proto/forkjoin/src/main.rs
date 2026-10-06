//! ADR 0031 prototype: a static task graph (tracks -> buses -> master) run by a pool of
//! spinning workers through a lock-free ready queue, in safe Rust, std only.
//!
//! Measures: (1) dispatch overhead per block, (2) speed-up on a heavy graph, (3) that the
//! parallel result equals the serial one to the bit (canonical gather order), (4) how long
//! a spinning or parked worker takes to see a new block, (5) the cost of copying audio
//! through relaxed `AtomicU32` cells (a safe single-writer ring) against a plain copy.
//!
//! `cargo run --release` (pin with `start /affinity` to compare; see docs/perf/proto.md).

mod queue;

use queue::Queue;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::*};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

const FRAMES: usize = 128;

#[derive(Clone)]
struct NodeDef {
    /// Predecessors in canonical order, each with the gain its edge applies.
    inputs: Vec<(u32, f32)>,
    succ: Vec<u32>,
    /// Biquad passes over the block: the node's "DSP".
    work: u32,
    source: bool,
    /// Do nothing at all: what is left is the scheduling itself.
    bare: bool,
}

struct NodeState {
    out: Vec<f32>,
    z1: f32,
    z2: f32,
}

#[repr(align(64))]
struct Padded(AtomicU32);

#[derive(Copy, Clone, PartialEq)]
enum Idle {
    Spin,
    Park,
}

struct Shared {
    defs: Vec<NodeDef>,
    npred: Vec<u32>,
    state: Vec<RwLock<NodeState>>,
    pending: Vec<Padded>,
    remaining: AtomicUsize,
    queue: Queue,
    epoch: AtomicU64,
    block: AtomicU64,
    stop: AtomicBool,
    idle: Idle,
    seen_at: Vec<AtomicU64>,
    base: Instant,
}

fn graph(tracks: usize, buses: usize, track_work: u32, bus_work: u32, master_work: u32) -> Vec<NodeDef> {
    let n = tracks + buses + 1;
    let master = (n - 1) as u32;
    let mut defs: Vec<NodeDef> = (0..n)
        .map(|i| NodeDef {
            inputs: vec![],
            succ: vec![],
            work: if i < tracks { track_work } else if i < n - 1 { bus_work } else { master_work },
            source: i < tracks,
            bare: false,
        })
        .collect();
    for t in 0..tracks {
        // Every fourth track goes straight to the master, the rest to a bus.
        let dest = if t % 4 == 3 { master } else { (tracks + t % buses) as u32 };
        defs[dest as usize].inputs.push((t as u32, 0.5 + (t % 7) as f32 * 0.07));
        defs[t].succ.push(dest);
    }
    for b in 0..buses {
        let i = tracks + b;
        defs[n - 1].inputs.push((i as u32, 0.8));
        defs[i].succ.push(master);
    }
    // Canonical order: by producer index (tracks first, then strips in play order).
    for d in &mut defs {
        d.inputs.sort_by_key(|x| x.0);
    }
    defs
}

impl Shared {
    fn new(defs: Vec<NodeDef>, idle: Idle, workers: usize) -> Arc<Self> {
        let n = defs.len();
        let mut npred = vec![0u32; n];
        for d in &defs {
            for s in &d.succ {
                npred[*s as usize] += 1;
            }
        }
        Arc::new(Shared {
            npred,
            state: (0..n)
                .map(|_| RwLock::new(NodeState { out: vec![0.0; FRAMES], z1: 0.0, z2: 0.0 }))
                .collect(),
            pending: (0..n).map(|_| Padded(AtomicU32::new(0))).collect(),
            remaining: AtomicUsize::new(0),
            queue: Queue::new(n * 2),
            epoch: AtomicU64::new(0),
            block: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            idle,
            seen_at: (0..workers).map(|_| AtomicU64::new(0)).collect(),
            base: Instant::now(),
            defs,
        })
    }

    /// One node: gather its inputs in canonical order into an accumulator that starts at
    /// +0.0, then its DSP. Exclusive access to its own state, shared to its inputs'.
    fn exec(&self, i: usize, scratch: &mut [f32; FRAMES]) {
        let d = &self.defs[i];
        if d.bare {
            return;
        }
        let blk = self.block.load(Relaxed);
        scratch.iter_mut().for_each(|x| *x = 0.0);
        if d.source {
            for (k, x) in scratch.iter_mut().enumerate() {
                let p = (blk as usize * FRAMES + k) as f32 * (0.001 + i as f32 * 1e-5);
                *x = (p - p.floor()) * 2.0 - 1.0;
            }
        } else {
            for (p, g) in &d.inputs {
                let src = self.state[*p as usize].try_read().expect("schedule: input is done");
                for k in 0..FRAMES {
                    scratch[k] += src.out[k] * g;
                }
            }
        }
        let mut st = self.state[i].try_write().expect("schedule: one runner per node");
        let (b0, b1, b2, a1, a2) = (0.2f32, 0.4, 0.2, -0.5, 0.3);
        let (mut z1, mut z2) = (st.z1, st.z2);
        for _ in 0..d.work {
            for x in scratch.iter_mut() {
                let y = b0 * *x + z1;
                z1 = b1 * *x - a1 * y + z2;
                z2 = b2 * *x - a2 * y;
                *x = y;
            }
        }
        st.z1 = z1;
        st.z2 = z2;
        st.out.copy_from_slice(scratch);
    }

    fn finish(&self, i: usize) {
        for s in &self.defs[i].succ {
            if self.pending[*s as usize].0.fetch_sub(1, AcqRel) == 1 {
                let _ = self.queue.push(*s);
            }
        }
        self.remaining.fetch_sub(1, AcqRel);
    }

    fn drain(&self, scratch: &mut [f32; FRAMES]) {
        loop {
            if let Some(i) = self.queue.pop() {
                self.exec(i as usize, scratch);
                self.finish(i as usize);
            } else if self.remaining.load(Acquire) == 0 {
                return;
            } else {
                std::hint::spin_loop();
            }
        }
    }

    /// The callback's side: arm the block, wake (park mode), help until done.
    fn run_block(&self, workers: &[thread::Thread], scratch: &mut [f32; FRAMES]) {
        self.block.fetch_add(1, Relaxed);
        for (p, n) in self.pending.iter().zip(&self.npred) {
            p.0.store(*n, Relaxed);
        }
        self.remaining.store(self.defs.len(), Release);
        // Seed every node with no predecessor — not only the tracks: a bus nothing feeds
        // has none either, and left out it would never run and the block never end.
        for (i, n) in self.npred.iter().enumerate() {
            if *n == 0 {
                let _ = self.queue.push(i as u32);
            }
        }
        self.epoch.fetch_add(1, Release);
        if self.idle == Idle::Park {
            for w in workers {
                w.unpark();
            }
        }
        self.drain(scratch);
    }

    fn master_out(&self) -> Vec<f32> {
        self.state[self.defs.len() - 1].read().unwrap().out.clone()
    }
}

fn worker(sh: Arc<Shared>, me: usize) {
    let mut scratch = [0f32; FRAMES];
    let mut seen = 0u64;
    loop {
        let mut spins = 0u32;
        // Spin for a *time*, not a count: just over one block period (3 ms covers 128
        // frames at 48 kHz), so a worker is spinning when the next block comes.
        let spin_until = Instant::now() + Duration::from_micros(3000);
        loop {
            let e = sh.epoch.load(Acquire);
            if e != seen {
                seen = e;
                sh.seen_at[me].store(sh.base.elapsed().as_nanos() as u64, Relaxed);
                break;
            }
            if sh.stop.load(Relaxed) {
                return;
            }
            spins += 1;
            match sh.idle {
                Idle::Spin if spins % 64 != 0 || Instant::now() < spin_until => {
                    std::hint::spin_loop()
                }
                Idle::Spin => thread::yield_now(),
                Idle::Park => thread::park_timeout(Duration::from_millis(5)),
            }
        }
        sh.drain(&mut scratch);
    }
}

struct Pool {
    sh: Arc<Shared>,
    handles: Vec<thread::JoinHandle<()>>,
    threads: Vec<thread::Thread>,
}

impl Pool {
    fn new(defs: Vec<NodeDef>, workers: usize, idle: Idle) -> Pool {
        let sh = Shared::new(defs, idle, workers);
        let handles: Vec<_> = (0..workers)
            .map(|me| {
                let s = sh.clone();
                thread::spawn(move || worker(s, me))
            })
            .collect();
        let threads = handles.iter().map(|h| h.thread().clone()).collect();
        Pool { sh, handles, threads }
    }
    fn stop(self) {
        self.sh.stop.store(true, Relaxed);
        for t in &self.threads {
            t.unpark();
        }
        for h in self.handles {
            h.join().unwrap();
        }
    }
}

mod report;

fn main() {
    report::run();
}
