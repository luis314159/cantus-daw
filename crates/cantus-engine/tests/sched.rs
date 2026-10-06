//! The real-time scheduler (ADR 0031 §2, R2, R13) against mock graphs.
//!
//! - The queues alone: exactly-once under contention, and the stall R2 is about — a pusher
//!   stopped between claim and publish hides *later* pushes from every consumer of a
//!   Vyukov queue, and only its own task in a Chase–Lev deque. No task is ever lost.
//! - The pool: every task exactly once per block, after its predecessors, the output equal
//!   to the serial run, on random DAGs at 1–7 workers with both queues; park after
//!   silence, one wake per parked worker at the next block and none while spinning;
//!   the serial fallback; zero allocations after construction (a counting allocator).
//! - The PCT-shaped schedule fuzz with replay tokens, each seed twice (stalls at the
//!   scheduler's sites need `--features fuzz`).
//! - `#[ignore]`d: the measurement of both queues on sessions shaped like the bench's 64-
//!   and 100-track ones (`docs/round21/sched.md`):
//!   `cargo test -p cantus-engine --release --features fuzz --test sched -- --ignored --nocapture`

use cantus_engine::sched::fuzz::{self, Episode, FuzzGraph, Token};
use cantus_engine::sched::queue::{Deque, Mpmc, Steal};
use cantus_engine::sched::{Chaos, ChaseLev, Config, Graph, Meta, Mode, Pool, ReadyQueue, Vyukov};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::*};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------------------
// A counting allocator: counts heap events on the threads that opted in (the pool's
// workers through the start hook, the callback around its blocks) while armed.
// ---------------------------------------------------------------------------------------

struct Counting;

static ARMED: AtomicBool = AtomicBool::new(false);
static EVENTS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static COUNTED: Cell<bool> = const { Cell::new(false) };
}

fn count() {
    if ARMED.load(Relaxed) && COUNTED.try_with(|c| c.get()).unwrap_or(false) {
        EVENTS.fetch_add(1, Relaxed);
    }
}

// SAFETY: every call is forwarded unchanged to the system allocator; the bookkeeping is
// an atomic and a const-initialised thread local, neither of which allocates or panics.
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

// ---------------------------------------------------------------------------------------
// The queues alone
// ---------------------------------------------------------------------------------------

/// Slot 1 stops between claiming and publishing task 5; meanwhile slot 2 pushes task 6
/// and slot 0 pops. Returns what slot 0 saw during the stall, and everything popped after.
fn stalled_pusher<Q: ReadyQueue>() -> (Option<u32>, Vec<u32>) {
    let q = Arc::new(Q::new(8, 3));
    let meta = Arc::new(Meta::new(8, 3));
    meta.set_ranks(&[0, 1, 2, 3, 4, 5, 6, 7]);
    let release = Arc::new(AtomicBool::new(false));
    let (claimed_tx, claimed_rx) = mpsc::channel();
    let pusher = {
        let (q, meta, release) = (q.clone(), meta.clone(), release.clone());
        thread::spawn(move || {
            assert!(q.push(1, 5, &meta, || {
                claimed_tx.send(()).unwrap();
                while !release.load(Acquire) {
                    std::hint::spin_loop();
                }
            }));
        })
    };
    claimed_rx.recv().unwrap();
    let other = {
        let (q, meta) = (q.clone(), meta.clone());
        thread::spawn(move || assert!(q.push(2, 6, &meta, || ())))
    };
    other.join().unwrap();
    let during = q.pop(0, &meta, || ());
    release.store(true, Release);
    pusher.join().unwrap();
    let mut after = Vec::new();
    while let Some(t) = q.pop(0, &meta, || ()) {
        after.push(t);
    }
    (during, after)
}

#[test]
fn a_stalled_pusher_hides_later_work_in_vyukov_only_its_own_in_chase_lev() {
    // R2's hazard, shown: the published task 6 is invisible behind the claimed cell.
    let (during, after) = stalled_pusher::<Vyukov>();
    assert_eq!(
        during, None,
        "Vyukov: a consumer sees empty behind a stalled pusher"
    );
    assert_eq!(after, vec![5, 6], "and nothing is lost once it publishes");

    // The deques: slot 2's task is in slot 2's deque, stealable at once.
    let (during, after) = stalled_pusher::<ChaseLev>();
    assert_eq!(
        during,
        Some(6),
        "Chase–Lev: only the stalled task is hidden"
    );
    assert_eq!(after, vec![5], "and it is not lost");
}

#[test]
fn a_deque_owner_stalled_mid_push_hides_only_that_task() {
    let d = Arc::new(Deque::new(8));
    let release = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let owner = {
        let (d, release) = (d.clone(), release.clone());
        thread::spawn(move || {
            assert!(d.push_with(1, || ()));
            assert!(d.push_with(2, || {
                tx.send(()).unwrap();
                while !release.load(Acquire) {
                    std::hint::spin_loop();
                }
            }));
        })
    };
    rx.recv().unwrap();
    assert_eq!(d.steal_with(|| ()), Steal::Took(1));
    assert_eq!(d.steal_with(|| ()), Steal::Empty);
    release.store(true, Release);
    owner.join().unwrap();
    assert_eq!(d.steal_with(|| ()), Steal::Took(2));
    assert_eq!(d.steal_with(|| ()), Steal::Empty);
}

#[test]
fn the_mpmc_queue_delivers_every_value_exactly_once() {
    const PER: u32 = 20_000;
    let q = Arc::new(Mpmc::new(64));
    let seen: Arc<Vec<AtomicU64>> = Arc::new((0..4 * PER).map(|_| AtomicU64::new(0)).collect());
    let taken = Arc::new(AtomicUsize::new(0));
    let mut hs = Vec::new();
    for p in 0..4u32 {
        let q = q.clone();
        hs.push(thread::spawn(move || {
            for i in 0..PER {
                while !q.push_with(p * PER + i, || ()) {
                    std::hint::spin_loop();
                }
            }
        }));
    }
    for _ in 0..4 {
        let (q, seen, taken) = (q.clone(), seen.clone(), taken.clone());
        hs.push(thread::spawn(move || {
            while taken.load(Relaxed) < (4 * PER) as usize {
                if let Some(v) = q.pop_with(|| ()) {
                    seen[v as usize].fetch_add(1, Relaxed);
                    taken.fetch_add(1, Relaxed);
                }
            }
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
    assert!(seen.iter().all(|s| s.load(Relaxed) == 1));
}

#[test]
fn the_deque_gives_every_task_to_exactly_one_thread() {
    const N: u32 = 200_000;
    let d = Arc::new(Deque::new(256));
    let seen: Arc<Vec<AtomicU64>> = Arc::new((0..N).map(|_| AtomicU64::new(0)).collect());
    let done = Arc::new(AtomicBool::new(false));
    let thieves: Vec<_> = (0..3)
        .map(|_| {
            let (d, seen, done) = (d.clone(), seen.clone(), done.clone());
            thread::spawn(move || {
                while !done.load(Acquire) {
                    if let Steal::Took(v) = d.steal_with(|| ()) {
                        seen[v as usize].fetch_add(1, Relaxed);
                    }
                }
            })
        })
        .collect();
    // The owner pushes in bursts and pops some back, as a worker does.
    let mut next = 0u32;
    while next < N {
        let burst = (next % 7 + 1).min(N - next);
        for _ in 0..burst {
            while !d.push_with(next, || ()) {
                std::hint::spin_loop();
            }
            next += 1;
        }
        if next.is_multiple_of(3) {
            if let Some(v) = d.pop() {
                seen[v as usize].fetch_add(1, Relaxed);
            }
        }
    }
    while let Some(v) = d.pop() {
        seen[v as usize].fetch_add(1, Relaxed);
    }
    // Thieves may still hold one in flight: wait for the counts to settle.
    let until = Instant::now() + Duration::from_secs(5);
    while seen.iter().any(|s| s.load(Relaxed) == 0) && Instant::now() < until {
        thread::yield_now();
    }
    done.store(true, Release);
    for t in thieves {
        t.join().unwrap();
    }
    let bad: Vec<_> = (0..N)
        .filter(|&i| seen[i as usize].load(Relaxed) != 1)
        .take(5)
        .collect();
    assert!(bad.is_empty(), "values not taken exactly once: {bad:?}");
}

// ---------------------------------------------------------------------------------------
// The pool on random DAGs
// ---------------------------------------------------------------------------------------

/// An episode's graph without its change points, at `workers` workers.
fn calm_episode(seed: u64, workers: usize) -> Episode {
    let mut ep = Episode::from_token(Token(seed));
    ep.workers = workers;
    ep.changes.clear();
    ep
}

fn every_task_once<Q: ReadyQueue>() {
    for workers in [1, 2, 3, 7] {
        for seed in 0..40 {
            let ep = calm_episode(0x5EED_0000 + seed, workers);
            let o = fuzz::run_episode::<Q>(&ep);
            if let Err(e) = o.verdict() {
                panic!("{workers} workers: {e}");
            }
        }
    }
}

#[test]
fn every_task_runs_once_per_block_after_its_predecessors_vyukov() {
    every_task_once::<Vyukov>();
}

#[test]
fn every_task_runs_once_per_block_after_its_predecessors_chase_lev() {
    every_task_once::<ChaseLev>();
}

/// A fan: one source, `n - 2` middle tasks, one sink; each middle task costs `cost`.
fn fan(n: usize, cost: Duration) -> FuzzGraph {
    let mut preds = vec![Vec::new()];
    for _ in 1..n - 1 {
        preds.push(vec![0]);
    }
    preds.push((1..n as u32 - 1).collect());
    let mut costs = vec![cost; n];
    costs[0] = Duration::ZERO;
    costs[n - 1] = Duration::ZERO;
    FuzzGraph::new(preds, costs, (0..n as u32).collect())
}

fn block_ok(g: &FuzzGraph, b: u64) {
    let (missed, _) = g.end(b);
    assert_eq!(missed, 0);
    assert_eq!(g.double.load(Relaxed) + g.early.load(Relaxed), 0);
}

fn wait_parked<G: Graph, Q: ReadyQueue, C: Chaos>(pool: &Pool<G, Q, C>, within: Duration) -> bool {
    let until = Instant::now() + within;
    while Instant::now() < until {
        if (1..=pool.workers()).all(|w| pool.is_parked(w)) {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    false
}

fn park_and_wake<Q: ReadyQueue>() {
    let g = Arc::new(fan(10, Duration::from_micros(30)));
    let mut cfg = Config::new(2, 10);
    cfg.period = Duration::from_micros(200);
    cfg.park_after = Duration::from_millis(200);
    cfg.mode = Mode::Parallel;
    let mut pool = Pool::<FuzzGraph, Q>::new(g.clone(), cfg).unwrap();
    let mut b = 0;
    let mut block = |pool: &mut Pool<FuzzGraph, Q>| {
        b += 1;
        g.begin(b, 1);
        let r = pool.run_block();
        block_ok(&g, b);
        r
    };
    // Busy: back-to-back blocks, workers spinning, no wakes.
    for _ in 0..50 {
        assert_eq!(block(&mut pool).woke, 0);
    }
    assert_eq!(pool.stats().wakes, 0, "no wake while the workers spin");
    // Silence: both workers park.
    assert!(
        wait_parked(&pool, Duration::from_secs(2)),
        "workers park after silence"
    );
    // The first block after it wakes each parked worker once, and completes.
    let r = block(&mut pool);
    assert_eq!(r.woke, 2, "one wake per parked worker");
    // Then busy again: no more wakes.
    for _ in 0..50 {
        assert_eq!(block(&mut pool).woke, 0);
    }
    assert_eq!(pool.stats().wakes, 2);
    // Blocks 5 ms apart, well inside the 200 ms grace, keep them awake (spinning, then
    // yielding), never parked. (A loaded machine can stretch a sleep: only gaps that
    // stayed under half the grace count.)
    let mut last = Instant::now();
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(5));
        let gap = last.elapsed();
        let r = block(&mut pool);
        if gap < Duration::from_millis(100) {
            assert_eq!(
                r.woke, 0,
                "no wake while blocks keep coming ({gap:?} apart)"
            );
        }
        last = Instant::now();
    }
    // Parked workers stop promptly when the pool goes.
    assert!(wait_parked(&pool, Duration::from_secs(2)));
    let t = Instant::now();
    drop(pool);
    assert!(t.elapsed() < Duration::from_secs(1));
}

#[test]
fn workers_park_after_silence_and_wake_once_vyukov() {
    park_and_wake::<Vyukov>();
}

#[test]
fn workers_park_after_silence_and_wake_once_chase_lev() {
    park_and_wake::<ChaseLev>();
}

fn serial_fallback<Q: ReadyQueue>() {
    // 8 tasks of 150 µs: 1.2 ms of work, 45 % of a 2.67 ms period.
    let g = Arc::new(fan(10, Duration::from_micros(150)));
    let mut pool = Pool::<FuzzGraph, Q>::new(g.clone(), Config::new(2, 10)).unwrap();
    let mut b = 0;
    let mut block = |pool: &mut Pool<FuzzGraph, Q>, jitter: u32| {
        b += 1;
        g.begin(b, jitter);
        let r = pool.run_block();
        block_ok(&g, b);
        r
    };
    assert!(
        !block(&mut pool, 1).serial,
        "a loaded graph starts parallel"
    );
    // Quiet (no cost): 32 quiet blocks, then serial.
    let quiet: Vec<bool> = (0..40).map(|_| block(&mut pool, 0).serial).collect();
    assert!(
        !quiet[..32].iter().any(|&s| s),
        "parallel for the first 32 quiet blocks"
    );
    assert!(quiet[32..].iter().all(|&s| s), "then serial");
    // One loaded block (still serial) puts the next one back in parallel.
    assert!(block(&mut pool, 1).serial);
    assert!(!block(&mut pool, 1).serial);
    let s = pool.stats();
    assert_eq!((s.serial_blocks, s.parallel_blocks), (9, 34));
    // Forced modes.
    pool.set_mode(Mode::Serial);
    assert!(block(&mut pool, 1).serial);
    pool.set_mode(Mode::Parallel);
    assert!(!block(&mut pool, 0).serial);
}

#[test]
fn the_serial_fallback_has_hysteresis_vyukov() {
    serial_fallback::<Vyukov>();
}

#[test]
fn the_serial_fallback_has_hysteresis_chase_lev() {
    serial_fallback::<ChaseLev>();
}

#[test]
fn no_workers_is_the_serial_engine_and_a_graph_too_big_is_refused() {
    let g = Arc::new(fan(12, Duration::ZERO));
    let mut pool = Pool::<FuzzGraph, ChaseLev>::new(g.clone(), Config::new(0, 12)).unwrap();
    assert_eq!(pool.workers(), 0);
    g.begin(1, 1);
    assert!(pool.run_block().serial);
    block_ok(&g, 1);
    let mut small = Pool::<FuzzGraph, Vyukov>::new(g.clone(), Config::new(2, 8)).unwrap();
    assert!(
        small.run_block().too_big,
        "12 tasks in a pool built for 8: refused, not overrun"
    );
    // The empty graph.
    let e = Arc::new(FuzzGraph::new(Vec::new(), Vec::new(), Vec::new()));
    let mut pool = Pool::<FuzzGraph, Vyukov>::new(e, Config::new(2, 4)).unwrap();
    pool.set_mode(Mode::Parallel);
    assert_eq!(pool.run_block().work_ns, 0);
}

fn no_allocation<Q: ReadyQueue>() {
    let ep = calm_episode(0xA110C, 3);
    let n = ep.preds.len();
    let g = Arc::new(FuzzGraph::new(
        ep.preds.clone(),
        ep.cost.clone(),
        ep.ranks.clone(),
    ));
    let mut cfg = Config::new(3, n);
    cfg.period = Duration::from_micros(100);
    cfg.park_after = Duration::from_millis(20);
    cfg.on_start = Some(Arc::new(|_slot| COUNTED.with(|c| c.set(true))));
    let mut pool = Pool::<FuzzGraph, Q>::new(g.clone(), cfg).unwrap();
    COUNTED.with(|c| c.set(true));
    let mut b = 0;
    let mut run = |pool: &mut Pool<FuzzGraph, Q>, blocks: usize, mode: Mode| {
        pool.set_mode(mode);
        for _ in 0..blocks {
            b += 1;
            g.begin(b, 1);
            pool.run_block();
        }
    };
    // Warm-up: every path once (parallel, park, wake, serial, auto).
    run(&mut pool, 20, Mode::Parallel);
    assert!(wait_parked(&pool, Duration::from_secs(2)));
    run(&mut pool, 20, Mode::Parallel);
    run(&mut pool, 5, Mode::Serial);
    run(&mut pool, 40, Mode::Auto);
    EVENTS.store(0, SeqCst);
    ARMED.store(true, SeqCst);
    run(&mut pool, 200, Mode::Parallel);
    assert!(wait_parked(&pool, Duration::from_secs(2)));
    run(&mut pool, 50, Mode::Parallel);
    run(&mut pool, 20, Mode::Serial);
    run(&mut pool, 100, Mode::Auto);
    ARMED.store(false, SeqCst);
    COUNTED.with(|c| c.set(false));
    let events = EVENTS.load(SeqCst);
    assert!(pool.stats().wakes >= 6, "the run included wakes");
    assert_eq!(
        events,
        0,
        "{}: heap events on the pool's threads after construction",
        Q::NAME
    );
}

#[test]
fn the_pool_never_allocates_after_construction() {
    // One test for both queues: the allocator's arming is global.
    no_allocation::<Vyukov>();
    no_allocation::<ChaseLev>();
}

// ---------------------------------------------------------------------------------------
// The PCT-shaped schedule fuzz (R13)
// ---------------------------------------------------------------------------------------

/// `CANTUS_SCHED_REPLAY=sched1-…` reruns one token; `CANTUS_SCHED_SEEDS` sets how many
/// seeds per queue (default 150).
fn fuzz_queue<Q: ReadyQueue>() -> (u32, u32) {
    let tokens: Vec<Token> = match std::env::var("CANTUS_SCHED_REPLAY") {
        Ok(t) => vec![Token::parse(&t).expect("a replay token: sched1-<hex>")],
        Err(_) => {
            let seeds: u64 = std::env::var("CANTUS_SCHED_SEEDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(150);
            (0..seeds).map(|s| Token(fuzz::mix(0xC0FFEE ^ s))).collect()
        }
    };
    let (mut fired, mut claimed) = (0, 0);
    let mut failures = Vec::new();
    for token in tokens {
        match fuzz::check::<Q>(token, Duration::from_secs(20)) {
            Ok(o) => {
                fired += o.fired;
                claimed += o.fired_claimed;
            }
            Err(e) => failures.push(e),
        }
    }
    assert!(
        failures.is_empty(),
        "{} schedule fuzz failures; replay with CANTUS_SCHED_REPLAY=<token>:\n{}",
        Q::NAME,
        failures.join("\n")
    );
    (fired, claimed)
}

fn fuzz_and_report<Q: ReadyQueue>() {
    let (fired, claimed) = fuzz_queue::<Q>();
    println!(
        "{}: {fired} change points fired, {claimed} between claim and publish",
        Q::NAME
    );
    if cfg!(feature = "fuzz") && std::env::var("CANTUS_SCHED_REPLAY").is_err() {
        assert!(
            fired > 0,
            "with the fuzz feature the change points must fire"
        );
    }
}

#[test]
fn pct_schedule_fuzz_vyukov() {
    fuzz_and_report::<Vyukov>();
}

#[test]
fn pct_schedule_fuzz_chase_lev() {
    fuzz_and_report::<ChaseLev>();
}

#[test]
fn a_replay_token_rebuilds_the_same_episode() {
    let t = Token(0x0123_4567_89AB_CDEF);
    assert_eq!(t.to_string(), "sched1-0123456789abcdef");
    assert_eq!(Token::parse(&t.to_string()), Some(t));
    let (a, b) = (Episode::from_token(t), Episode::from_token(t));
    assert_eq!((a.preds, a.ranks, a.rng), (b.preds, b.ranks, b.rng));
    assert_eq!(a.changes, b.changes);
    assert!((2..=3).contains(&a.changes.len()));
}

/// With the `fuzz` feature: a worker stalled between claim and publish on every block —
/// the graph still completes every task exactly once, in order, with the serial output.
#[cfg(feature = "fuzz")]
fn stalled_pusher_pool<Q: ReadyQueue>() -> Duration {
    use cantus_engine::sched::fuzz::Stall;
    use cantus_engine::sched::Site;
    // Six sources, each fanning out to six tasks, all into one sink: a finished source
    // readies six tasks at once, so its thread keeps one and *pushes* five. (Gathers alone
    // never push: the last input readies one task, which the continuation slot takes.)
    let mut preds: Vec<Vec<u32>> = (0..6).map(|_| Vec::new()).collect();
    for src in 0..6u32 {
        for _ in 0..6 {
            preds.push(vec![src]);
        }
    }
    preds.push((6..42).collect());
    let n = preds.len();
    let g = Arc::new(FuzzGraph::new(
        preds,
        vec![Duration::from_micros(20); n],
        (0..n as u32).collect(),
    ));
    let mut cfg = Config::new(3, n);
    cfg.mode = Mode::Parallel;
    let chaos = Stall::new(Site::Claimed, 0, Duration::from_millis(2));
    let mut pool = Pool::<FuzzGraph, Q, Stall>::with_chaos(g.clone(), cfg, chaos).unwrap();
    let t = Instant::now();
    for b in 1..=50 {
        g.begin(b, 1);
        pool.run_block();
        block_ok(&g, b);
    }
    assert!(pool.chaos().fired.load(Relaxed) > 0, "the stall fired");
    t.elapsed() / 50
}

#[cfg(feature = "fuzz")]
#[test]
fn a_worker_stalled_between_claim_and_publish_loses_nothing() {
    let v = stalled_pusher_pool::<Vyukov>();
    let c = stalled_pusher_pool::<ChaseLev>();
    println!("2 ms stall mid-push every block: vyukov {v:?}/block, chase-lev {c:?}/block");
}

/// With the `fuzz` feature: workers that never wake (stalled before they wait) do not
/// hold the callback up — it runs the block itself.
#[cfg(feature = "fuzz")]
#[test]
fn the_callback_never_waits_on_a_worker_waking() {
    use cantus_engine::sched::Site;
    /// Each worker's first wait stalls 400 ms: "asleep" for the first blocks.
    struct Sleepy([AtomicBool; 3]);
    impl Chaos for Sleepy {
        fn at(&self, site: Site, slot: usize) {
            if site == Site::Idle && !self.0[slot].swap(true, Relaxed) {
                fuzz::spin_for(Duration::from_millis(400));
            }
        }
    }
    let g = Arc::new(fan(10, Duration::from_micros(20)));
    let mut cfg = Config::new(2, 10);
    cfg.mode = Mode::Parallel;
    let chaos = Sleepy(Default::default());
    let mut pool = Pool::<FuzzGraph, ChaseLev, Sleepy>::with_chaos(g.clone(), cfg, chaos).unwrap();
    thread::sleep(Duration::from_millis(20));
    let t = Instant::now();
    for b in 1..=10 {
        g.begin(b, 1);
        pool.run_block();
        block_ok(&g, b);
    }
    assert!(
        t.elapsed() < Duration::from_millis(150),
        "the callback ran them alone"
    );
}

// ---------------------------------------------------------------------------------------
// The measurement (ignored by default; docs/round21/sched.md)
// ---------------------------------------------------------------------------------------

/// A dependent floating-point chain: `iters` steps, a few cycles each, not vectorisable.
#[inline(never)]
fn burn(iters: u64, seed: f64) -> f64 {
    let (mut x, mut y) = (seed, 0.25f64);
    for _ in 0..std::hint::black_box(iters) {
        x = x * 0.999_999_9 + y;
        y = y * 0.999 + 1e-7;
    }
    std::hint::black_box(x + y)
}

/// Iterations of [`burn`] per nanosecond on this thread's core (the fastest of 7 tries).
fn calibrate() -> f64 {
    let iters = 4_000_000;
    let best = (0..7)
        .map(|_| {
            let t = Instant::now();
            burn(iters, 1.0);
            t.elapsed().as_nanos() as f64
        })
        .fold(f64::MAX, f64::min);
    iters as f64 / best
}

/// A mock session: tasks with real floating-point work, critical-path ranks.
struct Session {
    name: String,
    preds: Vec<Vec<u32>>,
    succs: Vec<Vec<u32>>,
    cost_us: Vec<f64>,
    iters: Vec<u64>,
    ranks: Vec<u32>,
    sink: Vec<AtomicU64>,
}

impl Session {
    /// `preds` in topological index order; costs in µs at 128 frames.
    fn new(name: &str, preds: Vec<Vec<u32>>, cost_us: Vec<f64>, per_ns: f64) -> Session {
        let n = preds.len();
        let mut succs = vec![Vec::new(); n];
        for (t, ps) in preds.iter().enumerate() {
            for &p in ps {
                succs[p as usize].push(t as u32);
            }
        }
        // HEFT's upward rank: a task's cost plus its most expensive path to the sink.
        let mut up = vec![0f64; n];
        for t in (0..n).rev() {
            let tail = succs[t].iter().map(|&s| up[s as usize]).fold(0.0, f64::max);
            up[t] = cost_us[t] + tail;
        }
        let mut ranks: Vec<u32> = (0..n as u32).collect();
        ranks.sort_by(|a, b| up[*b as usize].total_cmp(&up[*a as usize]).then(a.cmp(b)));
        Session {
            name: name.to_string(),
            iters: cost_us
                .iter()
                .map(|c| (c * 1000.0 * per_ns) as u64)
                .collect(),
            sink: (0..n).map(|_| AtomicU64::new(0)).collect(),
            preds,
            succs,
            cost_us,
            ranks,
        }
    }

    fn total_us(&self) -> f64 {
        self.cost_us.iter().sum()
    }

    /// The longest path's cost (the floor of any schedule).
    fn critical_us(&self) -> f64 {
        let n = self.preds.len();
        let mut up = vec![0f64; n];
        for t in (0..n).rev() {
            let tail = self.succs[t]
                .iter()
                .map(|&s| up[s as usize])
                .fold(0.0, f64::max);
            up[t] = self.cost_us[t] + tail;
        }
        up.into_iter().fold(0.0, f64::max)
    }
}

impl Graph for Session {
    fn len(&self) -> usize {
        self.preds.len()
    }
    fn preds(&self, task: u32) -> u32 {
        self.preds[task as usize].len() as u32
    }
    fn succs(&self, task: u32) -> &[u32] {
        &self.succs[task as usize]
    }
    fn ranks(&self) -> &[u32] {
        &self.ranks
    }
    fn run(&self, task: u32, _slot: usize) {
        let v = burn(self.iters[task as usize], 1.0 + task as f64);
        self.sink[task as usize].store(v.to_bits(), Relaxed);
    }
}

/// The bench's scaled sessions (`cantus-bench` `sessions::scaled(n)`): the house desk —
/// four drum tracks into a drum bus, sends to a reverb and a delay return, the mastering
/// chain on the master — and copies of the demos' tracks, each with its own strip to the
/// master. Costs from `docs/perf/baseline.md` (quiet P-core, ns/frame × 128): the fifteen
/// heaviest instruments of the 64-track profile (Unda on Pad 1.6 ms … Unda on Bells
/// 0.19 ms), the lighter instruments (Gravis 24 µs … Pulsus 179 µs), strips 5–35 µs
/// (Porta, Aequalis, Comprimo, Sibilus), Spatium 159 µs, the master chain 250 µs
/// (Comprimo + Culmen + Terminus). Tracks and strips are separate tasks: no clustering.
fn scaled(tracks: usize, per_ns: f64, zero: bool) -> Session {
    const HEAVY: [f64; 15] = [
        1600., 1080., 1080., 700., 530., 530., 300., 290., 280., 280., 260., 260., 260., 230., 190.,
    ];
    const LIGHT: [f64; 10] = [24., 41., 83., 60., 30., 45., 100., 20., 145., 179.];
    const MORE: [f64; 10] = [530., 24., 41., 83., 60., 260., 30., 45., 100., 20.];
    const STRIP: [f64; 6] = [18., 9., 35., 5., 25., 12.];
    let t = tracks;
    let (drum, reverb, delay, master) = (2 * t, 2 * t + 1, 2 * t + 2, 2 * t + 3);
    let mut preds = vec![Vec::new(); 2 * t + 4];
    let mut cost = vec![0f64; 2 * t + 4];
    let (mut h, mut l) = (0, 0);
    for i in 0..t {
        cost[i] = if i >= 64 {
            MORE[i % MORE.len()]
        } else if i % 4 == 1 && h < HEAVY.len() {
            h += 1;
            HEAVY[h - 1]
        } else {
            l += 1;
            LIGHT[(l - 1) % LIGHT.len()]
        };
        let s = t + i;
        cost[s] = STRIP[i % STRIP.len()];
        preds[s].push(i as u32);
        if i < 4 {
            preds[drum].push(s as u32);
        } else {
            preds[master].push(s as u32);
        }
        if matches!(i, 4 | 5 | 6 | 8) {
            preds[reverb].push(s as u32);
        }
        if matches!(i, 7 | 9) {
            preds[delay].push(s as u32);
        }
    }
    cost[drum] = 30.;
    cost[reverb] = 159.;
    cost[delay] = 4.;
    cost[master] = 250.;
    preds[master].extend([drum as u32, reverb as u32, delay as u32]);
    for p in &mut preds {
        p.sort_unstable();
    }
    if zero {
        cost.iter_mut().for_each(|c| *c = 0.);
    }
    let name = format!("s{tracks}{}", if zero { ".bare" } else { "" });
    Session::new(&name, preds, cost, per_ns)
}

/// A synthetic fan-out graph that makes workers *push*: 8 sources (a track feeding
/// many listeners) each readying 12 tasks of 30 µs at once, each group gathered by a bus,
/// the buses by a master. Mixing sessions are gathers and almost never push.
fn fanout(per_ns: f64) -> Session {
    let mut preds: Vec<Vec<u32>> = (0..8).map(|_| Vec::new()).collect();
    let mut cost = vec![30.; 8];
    for src in 0..8u32 {
        for _ in 0..12 {
            preds.push(vec![src]);
            cost.push(30.);
        }
    }
    for src in 0..8u32 {
        preds.push((8 + src * 12..8 + (src + 1) * 12).collect());
        cost.push(10.);
    }
    preds.push((104..112).collect());
    cost.push(50.);
    Session::new("fan", preds, cost, per_ns)
}

/// The promotion *rt* will own (ADR 0031 §2.6, `cantus-rt` `thread.rs`): MMCSS "Pro
/// Audio" at HIGH. Bench only, through the pool's `on_start` hook (and once on the bench's
/// callback thread); `CANTUS_SCHED_MMCSS=0` turns it off.
#[cfg(windows)]
mod mmcss {
    #[link(name = "avrt")]
    extern "system" {
        fn AvSetMmThreadCharacteristicsW(task: *const u16, index: *mut u32) -> isize;
        fn AvSetMmThreadPriority(handle: isize, priority: i32) -> i32;
    }

    pub fn promote() -> bool {
        if std::env::var("CANTUS_SCHED_MMCSS").is_ok_and(|v| v == "0") {
            return false;
        }
        let name: Vec<u16> = "Pro Audio\0".encode_utf16().collect();
        let mut index = 0u32;
        // SAFETY: a NUL-terminated UTF-16 name and a valid out pointer. The handle is kept
        // for the thread's life (bench threads end with their pool).
        unsafe {
            let h = AvSetMmThreadCharacteristicsW(name.as_ptr(), &mut index);
            h != 0 && AvSetMmThreadPriority(h, 1) != 0
        }
    }
}

#[cfg(not(windows))]
mod mmcss {
    pub fn promote() -> bool {
        false
    }
}

/// One configuration's block wall times (sorted, ns): all blocks, and those in which a
/// stall fired; and the pool's counters.
struct Measured {
    all: Vec<u64>,
    stalled: Vec<u64>,
    stats: cantus_engine::sched::PoolStats,
}

fn measure<Q: ReadyQueue, C: Chaos>(
    g: &Arc<Session>,
    workers: usize,
    chaos: C,
    blocks: usize,
) -> Measured {
    let mut cfg = Config::new(workers, g.len());
    cfg.mode = if workers == 0 {
        Mode::Serial
    } else {
        Mode::Parallel
    };
    cfg.on_start = Some(Arc::new(|_slot| {
        mmcss::promote();
    }));
    let mut pool = Pool::<Session, Q, C>::with_chaos(g.clone(), cfg, chaos).unwrap();
    for _ in 0..blocks / 8 + 10 {
        pool.run_block();
    }
    let before = pool.stats();
    let mut all = Vec::with_capacity(blocks);
    let mut stalled = Vec::with_capacity(blocks);
    for _ in 0..blocks {
        let f = stall_count(pool.chaos());
        let t = pool.run_block().wall_ns;
        all.push(t);
        if stall_count(pool.chaos()) > f {
            stalled.push(t);
        }
    }
    all.sort_unstable();
    stalled.sort_unstable();
    let mut stats = pool.stats();
    stats.pushes -= before.pushes;
    stats.wakes -= before.wakes;
    Measured {
        all,
        stalled,
        stats,
    }
}

/// The stalls a chaos made (only [`fuzz::Stall`] counts them).
fn stall_count<C: Chaos>(c: &C) -> u64 {
    let any: &dyn std::any::Any = c;
    any.downcast_ref::<fuzz::Stall>()
        .map_or(0, |s| s.fired.load(Relaxed))
}

fn pct(sorted: &[u64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    sorted[((sorted.len() - 1) as f64 * q).round() as usize] as f64 / 1000.0
}

/// One row's numbers in one round (µs).
#[derive(Clone, Copy)]
struct Round {
    p50: f64,
    p99: f64,
    max: f64,
    stalled_p50: f64,
    stalled_p99: f64,
    pushes: f64,
    stalled: usize,
}

fn round<Q: ReadyQueue, C: Chaos>(g: &Arc<Session>, w: usize, chaos: C, blocks: usize) -> Round {
    let m = measure::<Q, C>(g, w, chaos, blocks);
    Round {
        p50: pct(&m.all, 0.5),
        p99: pct(&m.all, 0.99),
        max: m.all[m.all.len() - 1] as f64 / 1000.0,
        stalled_p50: pct(&m.stalled, 0.5),
        stalled_p99: pct(&m.stalled, 0.99),
        pushes: m.stats.pushes as f64 / m.all.len() as f64,
        stalled: m.stalled.len(),
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.retain(|x| !x.is_nan());
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Both queues on the bench-shaped sessions, W = 1 … `CANTUS_SCHED_MAXW` (default 4: the
/// 12700H's six P-cores minus two), plus the serial run; with the `fuzz` feature also a
/// worker stalled 500 µs between claim and publish at one of its first pushes of every
/// block. `CANTUS_SCHED_ROUNDS` rounds (default 5) interleave every configuration, and the
/// table gives the **median across rounds** of each round's p50 and p99 — on a shared
/// machine one round's tail is mostly its neighbours'. `CANTUS_SCHED_PER_NS` fixes the work
/// per µs (default: calibrated); `CANTUS_SCHED_MMCSS=0` leaves the threads unpromoted.
#[test]
#[ignore = "a measurement: run with --release --features fuzz -- --ignored --nocapture"]
fn bench_both_queues() {
    let env = |k: &str, d: usize| {
        std::env::var(k)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(d)
    };
    let max_w = env("CANTUS_SCHED_MAXW", 4);
    let blocks = env("CANTUS_SCHED_BLOCKS", 300);
    let rounds = env("CANTUS_SCHED_ROUNDS", 5);
    let promoted = mmcss::promote();
    let per_ns = std::env::var("CANTUS_SCHED_PER_NS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(calibrate);
    println!(
        "work: {per_ns:.3} burn iterations per ns; {blocks} blocks per row; {rounds} rounds; MMCSS Pro Audio: {promoted}"
    );
    let graphs = [
        Arc::new(scaled(64, per_ns, false)),
        Arc::new(scaled(100, per_ns, false)),
        Arc::new(fanout(per_ns)),
        Arc::new(scaled(64, per_ns, true)),
    ];
    let stall = || {
        let mut s = fuzz::Stall::new(
            cantus_engine::sched::Site::Claimed,
            0,
            Duration::from_micros(500),
        );
        s.spread = 8;
        s
    };
    // (graph, queue, W, case) → one Round per round.
    type Key = (usize, &'static str, usize, &'static str);
    let mut table: Vec<(Key, Vec<Round>)> = Vec::new();
    let mut put = |key: Key, r: Round| match table.iter_mut().find(|(k, _)| *k == key) {
        Some((_, v)) => v.push(r),
        None => table.push((key, vec![r])),
    };
    for r in 0..rounds {
        eprintln!("round {}/{rounds}", r + 1);
        for (gi, g) in graphs.iter().enumerate() {
            let bare = g.name.ends_with("bare");
            let blocks = if bare { blocks * 10 } else { blocks };
            if r == 0 || bare {
                put(
                    (gi, "serial", 0, "normal"),
                    round::<Vyukov, _>(g, 0, cantus_engine::sched::Calm, blocks),
                );
            }
            for w in 1..=max_w {
                put(
                    (gi, Vyukov::NAME, w, "normal"),
                    round::<Vyukov, _>(g, w, cantus_engine::sched::Calm, blocks),
                );
                put(
                    (gi, ChaseLev::NAME, w, "normal"),
                    round::<ChaseLev, _>(g, w, cantus_engine::sched::Calm, blocks),
                );
                if cfg!(feature = "fuzz") && !bare {
                    put(
                        (gi, Vyukov::NAME, w, "stalled"),
                        round::<Vyukov, _>(g, w, stall(), blocks),
                    );
                    put(
                        (gi, ChaseLev::NAME, w, "stalled"),
                        round::<ChaseLev, _>(g, w, stall(), blocks),
                    );
                }
            }
        }
    }
    for (gi, g) in graphs.iter().enumerate() {
        println!(
            "\n{}: {} tasks, {:.0} µs of work, critical path {:.0} µs (budget 2 667 µs at 128 frames)\n",
            g.name,
            g.len(),
            g.total_us(),
            g.critical_us()
        );
        println!("| queue | W | case | p50 µs | p99 µs | best p99 | worst max | pushes/block | stalled p50 | stalled p99 |");
        println!("|---|---|---|---:|---:|---:|---:|---:|---:|---:|");
        for ((_, queue, w, case), rs) in table.iter().filter(|((i, ..), _)| *i == gi) {
            let col = |f: fn(&Round) -> f64| median(rs.iter().map(f).collect());
            let best_p99 = rs.iter().map(|r| r.p99).fold(f64::MAX, f64::min);
            let worst = rs.iter().map(|r| r.max).fold(0.0, f64::max);
            println!(
                "| {queue} | {w} | {case} | {:.0} | {:.0} | {best_p99:.0} | {worst:.0} | {:.1} | {:.0} | {:.0} |",
                col(|r| r.p50),
                col(|r| r.p99),
                col(|r| r.pushes),
                col(|r| r.stalled_p50),
                col(|r| r.stalled_p99),
            );
            eprintln!(
                "{{\"name\":\"sched.{}.{queue}.w{w}.{case}\",\"p50_us\":{:.1},\"p99_us\":{:.1},\"best_p99_us\":{best_p99:.1},\"worst_max_us\":{worst:.1},\"rounds\":{},\"stalled_blocks\":{}}}",
                g.name,
                col(|r| r.p50),
                col(|r| r.p99),
                rs.len(),
                rs.iter().map(|r| r.stalled).sum::<usize>(),
            );
        }
    }
}
