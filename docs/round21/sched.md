# Round 21 — *sched*: the real-time multicore scheduler (wave 1, mock graphs)

*Builder: sched · branch `r21/sched` (base `49f02f3`) · ADR 0031 §2 with amendments R2, R5,
R13 · `docs/perf/plan.md` Phase 0/1 "sched".*

## What was built

`crates/cantus-engine/src/sched/` — safe Rust (`#![forbid(unsafe_code)]` kept: every shared
cell is an atomic), std only, no new dependency.

| File | Lines | What |
|---|---:|---|
| `mod.rs` | 154 | `trait Graph` (len, `preds` = dependency counts, `succs`, `ranks` = every task once, critical path first, `run(task, slot)`, `serial_hint`), `trait Chaos` + `Calm`, `Site`, the `buggify!` macro, `Pad<T>` (128 bytes, R11) |
| `queue.rs` | 458 | `trait ReadyQueue` (`new`, `seed`, `push`, `pop`) and both designs: **`Vyukov`** (two bounded MPMC queues, critical = top quarter of the ranks, popped first) and **`ChaseLev`** (one fixed-capacity Chase–Lev deque per slot, the callback's included, plus one *mailbox* per slot that the sources are seeded into by **home** = the slot that ran them last block); `Meta` (priorities, homes) |
| `pool.rs` | 573 | `Pool<G, Q, C = Calm>`: `Config` (W, max tasks, device period, park grace, `Mode`, the `on_start` hook where *rt*'s promotion plugs in), `run_block()` (arm → help → spin only on running tasks), the continuation slot, spin → yield → park, wakes, the serial fallback with hysteresis, `BlockReport`, `PoolStats` |
| `fuzz.rs` | 511 | SplitMix64 `Rng`, `Token` (`sched1-<hex>`), `Episode` (a whole episode from one seed), the PCT chaos `Pct`, the deterministic `Stall`, the checking `FuzzGraph`, `run_episode`, `check` (twice per seed, under a watchdog) |

Outside my files: one line `pub mod sched;` in `cantus-engine/src/lib.rs` and the `fuzz = []`
feature (three lines) in `cantus-engine/Cargo.toml`.

### The block protocol (ADR 0031 §2.2)

1. The callback **arms**: priorities from `ranks()` (first of *n* gets *n*; the top quarter is
   *critical*), every pending count from `preds()`, `remaining = n`, every task without a
   predecessor made ready **in rank order** — all of them, not just the tracks (the
   prototype's hung-bus rule) — then the epoch is published (`SeqCst`, which also orders it
   before the parked-flag loads of the wake protocol).
2. It wakes **parked** workers only (one non-blocking `unpark` each, counted), never waits.
3. It **helps** (slot 0): pop → run → `finish`. `finish` decrements every successor; of those
   that reach zero the most critical is kept in the **continuation slot** and run next, the
   others are pushed least-critical first (so a deque owner's LIFO end pops the more
   critical first); `remaining` drops last, so it cannot reach zero while work is ready.
4. With nothing ready and `remaining > 0` it spins (R5: bare polls, then `spin_loop`, no
   clock): it waits only on tasks already running.

If `graph.len()` exceeds the pool's `max_tasks` the block is **refused** (`too_big`) rather
than overrun — the graph builder answers with a bigger pool from the UI thread, like
`PdcNeed`. If `ranks()` is not a permutation the sources fall back to index order rather
than hang.

### The two queues behind one trait (R2)

- **Vyukov** — exactly the architect's §2.3: two shared bounded MPMC arrays. The seeds and
  every push go through two shared counters.
- **Chase–Lev** — per slot a fixed deque (capacity = the plan's task count, so it never
  grows, shrinks or is replaced: no reclamation, the crossbeam objection does not arise) with
  Lê et al.'s C11 orderings, and a **mailbox**: the callback writes the block's sources whose
  home is that slot, critical first, and publishes them with one `Release` store of a
  `(len << 32) | next` word; anyone claims with one `fetch_add`, so a claim has **no window**.
  `pop(slot)`: own deque (LIFO) → own mailbox → the others' mailboxes (the sources nobody may
  be doing — a late or parked owner's) → steal the others' deques (FIFO). A thief stalled
  mid-steal blocks no one; an owner stalled mid-push hides only that one task.
- **Homes** (Acar–Blelloch–Blumofe affinity): `Meta::set_home(task, slot)` on every run;
  the next block seeds the task into that slot's mailbox. Serial blocks leave homes alone.

Both rely on "a task is made ready at most once per block": capacities never fill (a full
push would be retried and counted in `PoolStats::full_pushes` — zero in every test).

### Idle, spin, park (§2.4, amended ADR 0005 rule 3, R5)

A worker spins for **1.5 device periods** after its last block (bare polls, then `PAUSE`, the
clock read every 256 polls), then **yields**, and **parks** after `park_after` (100 ms) with no
block. Parking is a Dekker handshake with the arm: the worker stores `parked = true`
(`SeqCst`) and re-reads the epoch before `thread::park()`; the callback bumps the epoch
(`SeqCst`) and then reads each flag. One `unpark` per parked worker at the first block after
the silence, `BlockReport::woke` and `PoolStats::wakes` count them; a block whose workers are
spinning makes no system call. `Drop` stops and joins promptly, parked or not.

### Serial when parallel does not pay (§2.5)

`Mode::Auto`: 32 blocks in a row whose *work* (task time summed over slots, measured with one
`Instant::now()` per task in a run of tasks) is under 25 % of the period → serial; one block
over 35 % → parallel at the next block; `Graph::serial_hint()` forces serial. Serial runs a
topological walk on callback-owned counters (no atomics shared, no queue); the workers spin
out their budget and park. `Mode::Serial`/`Mode::Parallel` force; `W = 0` is the serial
engine with no threads.

### Zero allocation after construction

Everything is sized in `Pool::with_chaos`: pending counts (padded), metas, both queues'
buffers, mailboxes, the callback's sources and serial stack (`Vec::with_capacity`, never
pushed past). Worker threads are spawned there with their names. Tested (below).

## Tests — `crates/cantus-engine/tests/sched.rs`

17 tests (+1 ignored measurement); 15 run without the feature, all 17 with
`--features fuzz`. Both green on the final code:

```
cargo test -p cantus-engine --release --test sched                    # 15 passed, 1 ignored
cargo test -p cantus-engine --release --features fuzz --test sched    # 17 passed, 1 ignored
```

| Test | Holds |
|---|---|
| `a_stalled_pusher_hides_later_work_in_vyukov_only_its_own_in_chase_lev` | Through the `ReadyQueue` trait: slot 1 stopped between claim and publish of task 5, slot 2 pushes task 6. **Vyukov: slot 0 pops `None`** (6 is published but hidden behind the claimed cell — R2's hazard, reproduced); **Chase–Lev: slot 0 gets 6**. After the release both deliver everything, nothing lost |
| `a_deque_owner_stalled_mid_push_hides_only_that_task` | One deque: a thief takes the published task, sees `Empty` for the stalled one, gets it after the publish |
| `the_mpmc_queue_delivers_every_value_exactly_once` | 4 producers × 20 000, 4 consumers, capacity 64 |
| `the_deque_gives_every_task_to_exactly_one_thread` | 200 000 tasks, an owner pushing in bursts and popping, 3 thieves |
| `every_task_runs_once_per_block_after_its_predecessors_{vyukov,chase_lev}` | 40 random DAGs (1–40 tasks, random ranks unrelated to the topology) × W ∈ {1, 2, 3, 7}, 2–5 blocks each: no task twice, none before a predecessor *finished*, none missed, output hash = the serial run's |
| `workers_park_after_silence_and_wake_once_{vyukov,chase_lev}` | 50 back-to-back blocks: 0 wakes; silence → both workers parked; the next block: `woke == 2`; 50 more: 0; blocks 5 ms apart (inside the grace): 0 wakes; `Drop` of a parked pool < 1 s |
| `the_serial_fallback_has_hysteresis_{vyukov,chase_lev}` | Parallel at 45 % load; exactly 32 quiet blocks then serial; one loaded serial block → parallel the next; counters (9 serial, 34 parallel); forced modes |
| `no_workers_is_the_serial_engine_and_a_graph_too_big_is_refused` | W = 0 runs serially with no thread; 12 tasks in a pool built for 8 → `too_big`, nothing overrun; the empty graph |
| `the_pool_never_allocates_after_construction` | A counting global allocator counts every heap event on the pool's workers (flagged by the `on_start` hook) and the callback: **0** over 370 blocks per queue that include parallel, park, wake, serial and auto, after one warm-up of each path |
| `pct_schedule_fuzz_{vyukov,chase_lev}` | 150 seeds per queue, **each run twice** (`fuzz::check`): oracle as above, plus the generator's final state and the output hash equal across the two runs (the unseed check), under a 20 s watchdog (deadlock / lost task → the token). With the feature, last run: Vyukov 240 change points fired (18 between claim and publish), Chase–Lev 188 (19) |
| `a_replay_token_rebuilds_the_same_episode` | `sched1-<hex>` round-trips; the same token gives the same graph, ranks, change points and generator state |
| `a_worker_stalled_between_claim_and_publish_loses_nothing` *(fuzz)* | Six sources fanning out to six tasks each: a worker stalls 2 ms mid-push in every block, 50 blocks — every task once, in order, serial output |
| `the_callback_never_waits_on_a_worker_waking` *(fuzz)* | Both workers held 400 ms before their first wait: 10 blocks finish in < 150 ms — the callback ran them alone |

One test failed once as first written, on the loaded machine: blocks "5 ms apart" against a
30 ms grace saw 27 wakes because the test thread's sleeps stretched past the grace. The grace
in that test is now 200 ms and only gaps measured under 100 ms are asserted wake-free.

**Replay.** A failure prints e.g. `sched1-9f1c… (chase-lev): 1 ran twice, …`; rerun exactly that
episode with `CANTUS_SCHED_REPLAY=sched1-9f1c… cargo test -p cantus-engine --release
--features fuzz --test sched pct`. `CANTUS_SCHED_SEEDS=2000` for a longer soak.

**What the fuzz is and is not.** PCT proper controls a single-stepping scheduler; this one
perturbs real threads: random task priorities (the ranks), 2–3 change points per episode at
which whichever thread reaches the scheduler's *k*-th site stalls 50–600 µs, short episodes
(≤ 40 tasks, ≤ 5 blocks, so *k* ≈ 4 n stays small). It finds ordering and progress bugs
(a stall that loses a task hangs the watchdog), not weak-memory reorderings — those want
loom on the deque and the mailbox (left, below).

## Measurements

**Conditions — read first.** All runs on the i7-12700H, `start /high /affinity 555` (one
thread of each P-core, the prototype's placement), while the other wave-1 builders compiled:
Windows reported **100 % CPU** before most runs, and the serial s64 block took 13–30 ms
for 12.9 ms of nominal work, i.e. the cores ran 1–2.3× slower from run to run. Absolute
times are the neighbours' as much as ours. What is robust is what repeats across runs and
across rounds of one run, where both queues run back to back for each W. The command:

```
cargo test -p cantus-engine --release --features fuzz --test sched --no-run
set CANTUS_SCHED_PER_NS=0.282 & set CANTUS_SCHED_ROUNDS=5 & set CANTUS_SCHED_MMCSS=0
start "" /high /affinity 555 /wait /b target\release\deps\sched-….exe --ignored --nocapture --test-threads 1 bench_both_queues
```

Raw outputs (tables on stdout, one JSON object per row on stderr) are in
`docs/round21/sched-bench/`: run 1 (High class, calibrated, 300 blocks), run 2 (MMCSS,
calibrated under load — the work size differs, so not comparable), run 3 (5 rounds × 300
blocks, work fixed at 0.282 iterations/ns, **MMCSS off** and **on**). The tables below are run 3
with MMCSS off (the cleaner one), **median across 5 rounds** of each round's p50 and p99,
µs; the budget at 128 frames is 2 667 µs.

**The mock sessions** (`tests/sched.rs`, `scaled(n)`): the bench's `sessions::scaled` shape
— the house desk (four drum tracks into a drum bus, sends to a reverb and a delay return,
the mastering chain on the master) plus demo-track copies each with its own strip to the
master; tracks and strips separate tasks, no clustering. Costs from `docs/perf/baseline.md`
(quiet P-core, ns/frame × 128): the fifteen heaviest instruments of the 64-track profile
(Unda on Pad 1.6 ms … Unda on Bells 0.19 ms), lighter instruments 20–180 µs, strips
5–35 µs, Spatium 159 µs, the master chain 250 µs. **s64**: 132 tasks, 12.9 ms of work,
critical path 1.89 ms. **s100**: 204 tasks, 17.6 ms, same critical path. **fan**: a
synthetic fan-out (8 sources each readying 12 tasks of 30 µs at once, a bus per group, a
master): 113 tasks, 3.25 ms. **s64.bare**: s64 with every cost zero (the scheduler alone).
Each task is a dependent floating-point chain (real work, not a clock wait, so a preempted
task really takes longer).

### Normal blocks (run 3, MMCSS off)

| Session | W | Vyukov p50 | Chase–Lev p50 | Vyukov p99 | Chase–Lev p99 |
|---|---:|---:|---:|---:|---:|
| s64 (serial: 29 913 / 36 725) | 1 | 15 743 | 15 691 | 17 718 | 18 331 |
| | 2 | 11 448 | 6 616 | 15 496 | 16 711 |
| | 3 | 4 519 | 3 503 | 11 979 | 4 786 |
| | 4 | 3 460 | 3 466 | 9 787 | 33 938 |
| s100 (serial: 17 438 / 19 214) | 1 | 9 559 | 9 355 | 12 236 | 12 168 |
| | 2 | 13 799 | 14 537 | 16 311 | 22 050 |
| | 3 | 10 634 | 10 878 | 38 242 | 16 161 |
| | 4 | 11 818 | 11 846 | 42 786 | 58 367 |
| fan (serial: 6 840 / 7 420) | 1 | 4 055 | 4 080 | 5 024 | 5 391 |
| | 2 | 2 644 | 3 026 | 4 295 | 15 211 |
| | 3 | 2 025 | 1 997 | 32 564 | 2 314 |
| | 4 | 2 024 | 1 684 | 32 480 | 32 508 |

The serial row ran in round 1 only (it is slow), under a different moment's load than most
parallel rows — s100's serial block came out *faster* than s64's — so speed-ups over serial
cannot be read from this run; the prototype's (1.98× at 2 threads … 4.73× at 5) stand. Between
the queues, on real work, **no difference survives the noise**: p50s agree within a few
percent at most points and swap sign; p99s are the machine's (several rounds ran a
neighbour's link step on our cores; the "best p99" column of the raw tables is 2–4× lower).

### The scheduler alone (s64.bare: 132 tasks of nothing)

| W | Vyukov p50 / p99 | Chase–Lev p50 / p99 | per task, p50 (V / C) |
|---:|---:|---:|---:|
| serial | 4 / 6 | — | 0.03 µs |
| 1 | 51 / 78 | 52 / 71 | 0.39 / 0.39 µs |
| 2 | 60 / 93 | 47 / 68 | 0.45 / 0.36 µs |
| 3 | 58 / 88 | 43 / 66 | 0.44 / 0.33 µs |
| 4 | 58 / 95 | 41 / 65 | 0.44 / 0.31 µs |

The same ordering in all three runs and with MMCSS on (Vyukov 55–61 µs, Chase–Lev 41–47 µs
p50 at W = 1–4): **the deques dispatch 15–30 % cheaper from two workers up, with lower p99**,
because nothing is shared — the Vyukov seeds and pops all hit two shared counters, the
mailboxes and deques spread over W + 1 cache-line pairs. At W = 1 they tie. 0.3–0.45 µs per
task is the prototype's 310–530 ns again: a 132-task block costs ~45 µs of dispatch,
1.7 % of a 128-frame budget, which is what §2.5's clustering is for.

### The stalled pusher

**Mixing sessions do not push.** `pushes/block` is **0.0** for s64 and s100 at every W, both
queues: a gather is readied by its *last* input, which keeps it in its continuation slot,
and the sources are seeded by the callback before the epoch (nobody looks at a queue
during the seeding). So the R2 hazard — a thread stopped between claim and publish hiding
*other* threads' later pushes — has nothing to act on in the shape the engine runs today;
the 500 µs stall never fired there (stalled blocks: 0). Fan-outs push: 88 per block in
`fan` (a note source feeding many listeners, a send split, sub-tasks of a CLAP
`thread-pool` request).

On `fan`, with a worker stopped 500 µs at one of its first eight pushes in every block
(every block stalled): p50 Vyukov / Chase–Lev **4 502 / 4 337** (W 1), **2 972 / 2 968** (2),
**2 104 / 2 136** (3), **1 801 / 1 809** (4) — equal. Both designs pay the stall the same way
here, because the stalled thread is also holding *its own* unpushed successors (the push
loop is sequential) and the block cannot end without them; what Vyukov adds — hiding a
second thread's push behind the stalled cell — needs a second push inside the 500 µs
window to land behind it *and* matter for the block's end. The deterministic test shows the
mechanism exactly (`a_stalled_pusher_hides_later_work_in_vyukov…`: Vyukov's consumer sees
an empty queue holding a published task); the bench shows it is rare in effect. Its
cost is a *tail* event on fan-out graphs under preemption, which this machine's noise
cannot resolve.

### A finding for *rt*: MMCSS and spinning

Run 3 with every pool thread (and the bench's callback) in MMCSS "Pro Audio" at HIGH,
through the `on_start` hook — the promotion *rt* will own: the worst block in a row was
**9.2 s** (s64, Vyukov, W 2), **17.6 s** (s64, Chase–Lev, W 2, stalled), 3.6 s, 2.6 s, 2.5 s,
and many 0.3–1.1 s. With MMCSS off, the same build in the next 15 minutes: worst **155 ms**
anywhere. Run 2 (MMCSS) also had 3.5 s and 1.7 s blocks; run 1 (High class only) at most
0.6 s. The likely mechanism is MMCSS's own: a "Pro Audio" thread that uses more than its
share of a period is demoted to a low priority until the next period (the 20 %
`SystemResponsiveness` reserve), and a thread that *spins* uses all of it — and once
demoted, on a machine at 100 %, it waits behind every normal-priority compiler. **A spinner
in MMCSS may be worse than a spinner at High class.** This needs *rt*'s probe on a quiet
machine before wave 2 relies on it (options: MMCSS only for the callback with the workers at
`THREAD_PRIORITY_TIME_CRITICAL`; `AvSetMmThreadPriority(CRITICAL)`; shorter spin budgets that
yield inside each period; RTWQ as R3 suggests).

### Engine bench (`xtask perf`)

Not run: nothing in the processor path changed (the scheduler is not connected until wave
2; `processor*.rs` untouched), so `engine.*` cannot move, and with the machine at 100 % a
`--pin` run would measure the neighbours. The decision metric the plan names —
`engine.s64.b128` p99 per queue — needs the graph builder's nodes and is wave 2's first
measurement (both queues stay behind the trait for it).

## Recommendation

**Chase–Lev: per-slot fixed deques with home-seeded mailboxes.** Make it the default in
wave 2 and keep Vyukov behind `ReadyQueue` until `engine.s64.b128` p99 has been measured
with both on the real graph (a type parameter, so it costs nothing to keep).

The evidence, strongest first:

1. **Progress (a guarantee, not a measurement).** With Vyukov, a thread stopped between
   claim and publish makes every consumer see an empty queue while a published task sits
   behind it (reproduced deterministically through the trait). With the deques it hides only
   its own task, a thief stopped mid-steal blocks nobody, and a mailbox claim is a single
   `fetch_add` with no window at all. That is what makes ADR 0031 §2.2's sentence ("the
   callback waits only on tasks already running") true without R2's caveat.
2. **Dispatch.** Equal at one worker, **15–30 % cheaper from two workers up, with a lower
   p99**, in every run (0.31–0.36 µs per task against 0.44–0.45 at W = 2–4) — the gap grows
   with W, the direction the pool scales in (14 workers on a big part).
3. **Locality and the home worker for free.** Sources go back to the slot that ran them, so
   a track's instrument state tends to stay in one core's L2 — Acar–Blelloch–Blumofe's
   result; it also gives §8 risk 3's "home worker" for single-thread-minded plug-ins.
4. **Equal elsewhere.** On real work the two are within noise at every W on both
   bench-shaped sessions, and equal under a stalled pusher on the fan-out graph.

What Vyukov has over it: half the code (90 lines against 200) and a *global* priority
order (the critical lane is shared), where the deques order priority per slot (seeds in
rank order per mailbox, successors pushed least critical first, thieves FIFO). With the
continuation slot taking the most critical successor and the seeds in rank order, that
difference did not show — and mixing graphs never push anyway.

What would change the choice: wave 2's `engine.s64.b128` p99 with the real nodes and
clustering showing Vyukov ahead beyond run-to-run noise on a quiet machine (the criterion:
the median of ≥ 5 interleaved rounds, as here).

## Left for the next wave / notes for the integrator

**For the graph builder (wave 2, connecting the pool):**

- Implement `sched::Graph` on the compiled plan: `len` = tasks (clusters), `preds`/`succs`
  from the plan's arrays, `ranks` = the clusters by upward rank (a permutation; it need not
  be topological), `run(task, slot)` = run the cluster's nodes in order with `slot`'s
  scratch, `serial_hint` = critical path > 80 % of the total.
- Topology changes only between blocks, on the callback (`install_snapshot`), through the
  graph's own interior mutability; the pool reads it only inside `run_block`. Size the pool
  for the plan (`Config::max_tasks`); a bigger plan returns `BlockReport::too_big` — answer
  it like `PdcNeed` (build a new pool on the UI thread, send it, the old one back as
  garbage).
- `Config::period` = the device period (spin budget 1.5×); `on_start` = rt's
  `promote_worker(slot)` — **see the MMCSS finding above before choosing MMCSS for spinning
  workers.**
- `EngineStats::wakes` ← `PoolStats::wakes` (or `BlockReport::woke`); `schedule_conflicts` is
  the graph's (its `try_borrow` failures) — the pool cannot see it.
- The callback's own `run_block` is allocation-free and lock-free; it returns
  `work_ns` (task time over all slots) for the readout's load and the serial fallback.

**Left (mine, wave 2):**

- `loom` model tests of `Deque` and the mailbox (and `Mpmc` if kept) — not done: no new
  dependency this wave (proxy), and the types are safe Rust, so Miri adds little. The C11
  orderings are Lê et al.'s; the stress tests and the fuzz exercise them on x86 only (TSO —
  an aarch64 run is the one that could catch a weak ordering).
- The CLAP `thread-pool` sub-queue (plan Phase 1) and the external (sandbox) tasks
  (Phase 6) — the `ReadyQueue` push/pop is the place for both.
- Ahead workers (Phase 2'), and per-worker cycle counters for blame (§2.9).
- `clippy::disallowed_methods` for `AtomicRefCell::borrow*` in `sched/` (R16) — nothing in
  `sched/` uses `atomic_refcell`; the rule belongs with the graph's nodes.
- If *rt*'s `pad.rs` lands, `sched::Pad` becomes a re-export of it.
- A quiet-machine re-run of `bench_both_queues` (≈ 13 min per MMCSS setting).

**Notes for the integrator:**

- Files outside `sched/`: `cantus-engine/src/lib.rs` (+1 line `pub mod sched;`),
  `cantus-engine/Cargo.toml` (+3 lines: the `fuzz = []` feature). No `processor*.rs`
  touched; no request to graph needed this wave.
- `tests/sched.rs` has the crate's second `#[global_allocator]` (each test binary has its
  own, as `rt_memory.rs` does) and, for the ignored bench only, a 10-line `extern
  "system"` block calling `avrt.dll` (MMCSS) — test code, outside the engine crate's
  `forbid(unsafe_code)`; it goes away when the bench uses `cantus-rt`.
- CI: add `cargo test -p cantus-engine --features fuzz --test sched` beside the default
  run (the stall tests and the firing fuzz need the feature).
- Release-profile test builds took ~18 minutes per change on the loaded machine (thin LTO
  links every test binary); `--test sched` alone is what I ran.
