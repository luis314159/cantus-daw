# Round 22 — *pool*: the multicore live engine

*Builder **pool**, branch `r22/pool` (base `c8bc9f2`). ADR 0031 §2 with the review's R2, R3,
R5, R13; `docs/perf/plan.md` Phase 1 as amended; round 21's `graph`, `sched`, `rt` reports.*

## What was built

The live engine now runs on a pool of real-time workers. Every engine test passes unchanged
at `W = 0` and at `W ∈ {1, 3, 7}`, the null-test hashes (`v5.cantus`, the goldens) did not
move, and the parallel engine gives the serial engine's output **to the bit** on random
sessions, under shuffled priorities, clustering on and off, random block sizes and the
PCT-shaped schedule fuzz inside the engine's own pool.

### The graph on cells (`graph/exec.rs`, `cantus-rt/src/cell.rs`)

- **`cantus_rt::cell`** — `NodeCell<T>` (exclusive borrows only: `Sync` for `T: Send`, so a
  hosted plug-in's `Send`-but-not-`Sync` instance can live in one) and `ShareCell<T>` (many
  readers or one writer: an `AtomicRefCell`). **Try-only**: there is no panicking
  `borrow`/`borrow_mut`, so R16's rule holds by construction. One CAS to borrow, one store to
  release. The `unsafe` is in `cantus-rt` (every block has its `SAFETY:` line); the three unit
  tests, including a four-thread stress, **pass under Miri** (nightly). `cantus-engine` stays
  `#![forbid(unsafe_code)]` (the stale `deny` comment in `lib.rs` is gone).
- **`Nodes`** (the engine's `sched::Graph`, shared by the callback and the workers through an
  `Arc`): per track slot its state (`NodeCell<TrackSlot>`), its output and its note outbox
  (`ShareCell`); per strip slot its state, its output (post-chain pair, output edge, gains,
  and now each send's level) and its key; every delay line (`pdc::Lines`, shared with the
  planner); each delayed send's line output; the listeners; per pool slot its scratch
  (buffers, the outgoing gestures, its profiler), 128-byte padded; the analyser, the
  instrument probe, the mix recorder and the stem tap each in a cell of its own; per-slot
  meters as atomics; per-node start/end times. Everything is allocated and pre-faulted in
  `Nodes::new` (R11).
- **What a pass reads** is one `Frame` (`graph/ctx.rs`): the pass context (snapshot,
  transport at the pass, clock, slice, song position, pan law, switches, automation's last
  control point, the pass and block stamps) and the **task map** — the plan resolved to
  slots and lines (each gather's inputs already know whether a send is delayed) and each
  task's nodes. Borrowed shared once per task.
- **Nodes are free functions** over the cells: `run_track` (`graph/track.rs`), `run_strip`
  + `strip_chain` + `strip_outputs` (`graph/strip.rs`, `strip_run.rs`) — the serial engine's
  arithmetic expression for expression. The master runs on the callback after the pool's
  part of each pass (`graph/master_run.rs`): it is the sink, and the serial master chain is
  Amdahl's share anyway.
- **What used to be shared, and what replaced it.** Peaks reset "for every slot at block
  start" → a block stamp in the node; `key_valid` per pass → a pass stamp in the key buffer;
  a consumer reading its producer's send level from the producer's *state* → the level in
  the producer's *output*; a key from a strip **later** in the order (silence in the serial
  engine, and no dependency) → `key_before`, so it is not even looked at; hosted plug-ins'
  gestures pushed into the event ring by whichever node → the slot's scratch, forwarded by
  the callback after the pass; `dropped_events`, the meters, the analysed unit's readings,
  the probe → atomics and cells read after the block. The **stem tap** sums its *direct*
  bus in track order, so a stem render runs its nodes on the callback in plan order.
- **The control phase** (`processor_msg.rs`, `processor_snap.rs`, `processor_auto.rs`,
  `processor_rec.rs`) reaches the nodes between passes through the same cells; slots' ids
  are mirrored on the callback so a live note finds its slot without borrowing 256 cells.
  Automation's per-slot ramps moved into the nodes' state.
- `processor.rs` (2 129 lines) is now `processor.rs` (struct, construction, `process`) +
  `processor_msg.rs` + `processor_snap.rs` + `processor_pass.rs` + `processor_pool.rs`.

### The scheduler connected (`sched/`)

- **`Graph` is just `run(task, slot)`**; the shape is a `Topology` (predecessor counts,
  successors, ranks, serial hint) the callback hands the pool between blocks
  (`Pool::set_topology`): the pool keeps the successor lists as atomics, so the engine can
  change its tasks without the workers ever reading a half-written shape.
- **Workers count only once they have started**: a worker runs its start hook
  (`Config::on_start(slot, period, serve)`: placement, promotion, the denormal guard, then
  `serve()`), and only from inside `serve` is it in the availability mask. The callback
  seeds mailboxes only to available slots (an unavailable home's sources go to the
  callback's), wakes only available parked workers, and runs a pass alone when none is
  available. A pool built before its stream's first block waits for the period: the first
  `Pool::set_period` releases the workers.
- **Idle**: `Idle::Spin` (round 21's 1.5 periods, yield, park after 100 ms) is kept for
  spinning workers; **`Idle::BOUNDED`** (the engine's default) spins until **half a period
  after the block was armed** — and at least 50 µs after the worker's last task, so blocks
  that come back to back never park anyone — then parks. A worker is then never busy past
  half a period unless the block's own work keeps it so: safe in MMCSS "Pro Audio", which
  is what `promote_worker(Live, period, Waiting::Bounded { busy_share: 0.5 })` gives it.
- **Passes and the adaptive rule**: `run_pass()` runs one pass without judging it;
  `adapt(work, period)` applies §2.5's hysteresis once per *audio block* (an automated
  block has many short passes; judging each would read every one as "light").
- `DynChaos`: the schedule fuzz's hooks inside the engine's own pool.

### Tasks: clustering, ranks, the serial hint (`graph/cluster.rs`)

Built on the callback, allocation-free, when the plan or the slots change and every half
second of audio (eight blocks after a new plan), from each slot's measured cost (the
profiler's per-node time, smoothed). A node under 10 µs a block is fused into its only
successor that is not the master; light nodes that feed only the master are binned to
≥ 20 µs; a Kahn sort checks the task graph (a cycle falls back to one task per node). Ranks
are HEFT's upward rank; the serial hint is "critical path > 80 % of the work". Changing the
clustering changes no sample (tested: the fuzz switches it on and off at random).

### The processor drives the pool (`processor_pool.rs`, `processor_pass.rs`)

- `Workers { count, mode, promote, spin, chaos, shuffle }`, `Processor::set_workers`:
  `Workers::auto()` (P-cores − 2 from `cantus_rt::Topology`, cached), `live(n)`,
  `parallel(n)` (tests and bench: always parallel, unpromoted), `SERIAL`. `Processor::new`
  reads **`CANTUS_WORKERS`** (`0`, `N`, `N:auto`, `auto`; `CANTUS_WORKERS_PROMOTE=1`,
  `CANTUS_WORKERS_SPIN=1`), which is how every engine test runs at any `W` unchanged.
- A worker's life: the denormal guard for the whole thread, its stack pre-faulted, placed on
  a live P-core (`set_current_thread_cpus` from `Topology::placement`), promoted as above,
  then `serve()`.
- **`EngineStats`**: `wakes` (the pool's, added to the callback's promotion), new
  **`schedule_conflicts`** (every failed borrow, anywhere), **`workers`** (joined) and
  **`parallel`** (the last block used the pool).
- **The profiler per worker** (`profile::SlotProfile` in each slot's scratch, folded after
  the block) and **blame on the critical path**: when a parallel block overruns, the chain
  of nodes that finished last is walked back from the master and only its nodes (with the
  master's inserts and the engine) may be blamed; the callback's own wait for that chain is
  nobody's time. Serial blocks blame as before.
- `backend.rs`: the stream's processor gets `Workers::auto()` or the settings' count, and
  the process leaves EcoQoS at the first stream start (rt's R1 item for the integrator,
  done here because the paced bench showed what it costs — see below).
- **Settings ▸ Audio**: a row *Audio cores* — Automatic (n) / One (the audio thread alone) /
  1…n, applied with the device; a note under it says what the pool is doing; the engine
  readout's tooltip says it too. EN and ES.

## Measurements

**Conditions.** The round's laptop (i7-12700H, 6 P + 8 E, Windows 11), shared with the
other round-22 builders (cargo builds, another builder's bench and soak harness ran during
most of the session). Paired A/B runs alternate the round-21 tree (`c8bc9f2`, built from a
`git archive` into `target/base-src`) and this branch, the same driver
(`crates/cantus-bench/examples/pool_ab.rs`, copied into the base tree) built twice, each
run pinned to one thread of every P-core (`ProcessorAffinity = 0x555`) at High priority.

TBD

## Tests

TBD

## Files outside my area

TBD

## Left

TBD

## Notes for the integrator

TBD
