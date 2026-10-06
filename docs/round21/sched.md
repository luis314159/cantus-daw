# Round 21 — *sched*: the real-time multicore scheduler (wave 1, mock graphs)

*Builder: sched · branch `r21/sched` (base `49f02f3`) · ADR 0031 §2 with amendments R2, R5,
R13 · `docs/perf/plan.md` Phase 0/1 "sched".*

## What was built

`crates/cantus-engine/src/sched/` — safe Rust (`#![forbid(unsafe_code)]` kept: every shared
cell is an atomic), std only, no new dependency.

| File | Lines | What |
|---|---:|---|
| `mod.rs` | ~155 | `trait Graph` (len, `preds` = dependency counts, `succs`, `ranks` = every task once, critical path first, `run(task, slot)`, `serial_hint`), `trait Chaos` + `Calm`, `Site`, the `buggify!` macro, `Pad<T>` (128 bytes, R11) |
| `queue.rs` | ~450 | `trait ReadyQueue` (`new`, `seed`, `push`, `pop`) and both designs: **`Vyukov`** (two bounded MPMC queues, critical = top quarter of the ranks, popped first) and **`ChaseLev`** (one fixed-capacity Chase–Lev deque per slot, the callback's included, plus one *mailbox* per slot that the sources are seeded into by **home** = the slot that ran them last block); `Meta` (priorities, homes) |
| `pool.rs` | ~570 | `Pool<G, Q, C = Calm>`: `Config` (W, max tasks, device period, park grace, `Mode`, the `on_start` hook where *rt*'s promotion plugs in), `run_block()` (arm → help → spin only on running tasks), the continuation slot, spin → yield → park, wakes, the serial fallback with hysteresis, `BlockReport`, `PoolStats` |
| `fuzz.rs` | ~500 | SplitMix64 `Rng`, `Token` (`sched1-<hex>`), `Episode` (a whole episode from one seed), the PCT chaos `Pct`, the deterministic `Stall`, the checking `FuzzGraph`, `run_episode`, `check` (twice per seed, under a watchdog) |

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

PLACEHOLDER-TESTS

## Measurements

PLACEHOLDER-MEASURE

## Recommendation

PLACEHOLDER-RECO

## Left for the next wave / notes for the integrator

PLACEHOLDER-LEFT
