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

**Conditions — read first.** The round's laptop (i7-12700H, 6 P + 8 E, Windows 11), shared
with the other round-22 builders: cargo builds, another builder's bench and the soak harness
ran through most of the session, and the same binary's serial 64-track block took anywhere
from 7 to 43 ms. Every comparison below is therefore **paired**: the round-21 tree
(`c8bc9f2`, exported with `git archive` into `target/base-src`, built into its own target)
and this branch run the same driver (`crates/cantus-bench/examples/pool_ab.rs`, copied into
the base tree) alternately, back to back, each pinned to one thread of every P-core
(`ProcessorAffinity = 0x555`) at High priority (`target/ab.ps1`, not committed). The
metric is the bench's `engine.s64.b128`: `sessions::scaled(64)` at 128 frames, one second
of warm-up, four seconds of blocks back to back, the median and the 99th percentile of the
block's wall time. The pool here is `CANTUS_WORKERS=4` (always parallel, unpromoted), the
count `Workers::auto()` gives this machine.

### `engine.s64.b128` — paired, base vs pool (µs per block)

| Pair | Base p50 | Pool p50 | **p50 ratio** | Base p99 | Pool p99 | **p99 ratio** |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 7 008 | 1 813 | **0.26** | 14 882 | 3 079 | **0.21** |
| 2 | 7 216 | 1 997 | **0.28** | 15 097 | 3 092 | **0.20** |
| 3 | 16 995 | 4 094 | **0.24** | 68 228 | 12 923 | **0.19** |
| 4 | 28 625 | 4 769 | **0.17** | 72 924 | 15 124 | **0.21** |
| 5 | 19 581 | 2 166 | **0.11** | 64 309 | 3 620 | **0.06** |
| 6 | 8 898 | 2 254 | **0.25** | 29 117 | 3 567 | **0.12** |

Median of the pairs' ratios: **0.25× at the median and 0.20× at the 99th percentile**,
against the targets 0.45× and 0.6×; the two quietest pairs (1, 2) give 0.26–0.28× and
0.20–0.21×. The ratio of the medians of the runs is 0.17×. Every pair meets both targets.

After the last changes (critical-path head to the callback, spin share 0.1), two more pairs,
on a quieter moment of the machine: base 7 131 / 6 972 µs p50 and 10 147 / 10 643 µs p99,
pool 1 890 / 2 397 and 2 839 / 4 234 — **0.27× / 0.34× at the median, 0.28× / 0.40× at the
99th percentile**. The target holds on every one of the eight pairs.

Scaling, one pair each (same conditions; the base row is the pair's own):

| W | Base p50 | Pool p50 | Ratio | Base p99 | Pool p99 | Ratio |
|---|---:|---:|---:|---:|---:|---:|
| 1 | 31 143 | 13 366 | 0.43 | 76 157 | 32 823 | 0.43 |
| 2 | 29 859 | 3 284 | 0.11 | 67 618 | 4 788 | 0.07 |
| 3 | 9 928 | 2 716 | 0.27 | 29 910 | 4 096 | 0.14 |
| 4, adaptive | 11 479 | 2 523 | 0.22 | 33 588 | 7 399 | 0.22 |
| 5 | 32 330 | 4 099 | 0.13 | 69 743 | 14 024 | 0.20 |

(W = 1 is two threads: the per-node `Instant` stamps, the cells and the dispatch cost
nothing visible; the base row's spread is the machine's.)

### The songs at 256 frames, paced (`cantus-bench paced`)

`cantus-bench paced` (new) plays the startup project and the six demos through the bench's
`Rig` at the device period: each block starts at its period's start (a block that ends
after its period ended is a **miss**: a dropout on a device, which then drops the periods it
missed rather than queueing them), the thread promoted as the WASAPI callback is (MMCSS "Pro
Audio" HIGH), the process out of EcoQoS, the pool `Workers::auto()` = 4 promoted, placed
workers (`--workers`, `--share`, `--spin`, `--promote`, `--callback` change that); it prints
misses, the engine's overruns, lateness, block percentiles, wakes, the share of parallel
blocks, the conflicts, the last overrun's culprit and the three heaviest nodes.

**The target — every song with zero misses over two minutes — is not met on this machine.**

The final run (this branch's last code, the defaults: 4 promoted bounded workers, spin share
0.1, two minutes per song, 22 500 blocks each):

| Song | Misses | Overruns | Block p50 µs | Block p99 µs | Parallel | Conflicts | Heaviest node (µs/block) |
|---|---:|---:|---:|---:|---:|---:|---|
| startup | **170** | 76 | 2 098 | 4 797 | 99 % | 0 | Calor on Chords 1 560 |
| house | **995** | 584 | 2 382 | 6 147 | 87 % | 0 | Calor on Chords 1 615 |
| ballad | **3 427** | 2 216 | 3 021 | 7 176 | 34 % | 0 | Unda on Pad 2 323 |
| funk | **0** | 0 | 978 | 1 910 | 4 % | 0 | Culmen on Master 180 |
| lofi | **80** | 38 | 1 289 | 4 394 | 77 % | 0 | Calor on Pad 360 |
| ambient | **553** | 312 | 1 516 | 5 555 | 97 % | 0 | Unda on Pad 1 002 |
| trap | **2 036** | 1 521 | 1 591 | 8 342 | 96 % | 0 | Exemplar on Pad 1 048 |

One song (funk) meets the target; the startup song and lofi miss under 1 % of periods; the
ballad, trap, house and ambient miss 2–15 %. The same two-minute run of the serial engine
earlier in the session (other load): startup 6 883, house 1 741, ballad 6 539, funk 2,
lofi 1, ambient 1 625, trap 272 misses — the machine's state, not the engine, dominates
any single run, which is why the paired 20–30 s triplets below are the evidence for the
pool against the serial engine.

What the paced runs show, in order of weight:

1. **Each song is gated by one instrument.** The heaviest node of each song, alone, takes
   a large share of a 5.33 ms period (`examples/song_profile.rs`, back to back, serial):
   *Calor on Chords* 2.8 ms of the startup song's 5.8 ms; *Unda on Pad* 3.8 ms of the
   ballad's 5.2 ms (70 %); *Exemplar on Pad* 0.9–4.8 ms in trap; *Calor on Pad* in lofi;
   *Unda on Pad* in ambient. A block can never end before its heaviest node plus its strip
   plus the master chain (0.4–0.8 ms, serial by nature): the pool cannot split a node, and
   on a loaded laptop these nodes alone reach the period. The lever is **inside those
   instruments** — their voices rendered as sub-tasks on the pool (see *Left*), or cheaper
   voices (the plan's *kernel*/*voices* builders).
2. **The machine's state swings the result by 10–100×** between two runs of the same
   configuration minutes apart (the heaviest node's own average moved from 1.4 to 3.1 ms
   between rounds of the same song). Only paired, interleaved runs say anything; see the
   pairs below.
3. **Spinning costs clock.** With every worker spinning until half a period after the arm,
   the paced runs missed more periods than with workers that park after a tenth — in every
   one of six interleaved triplets (startup 2 / 0 and 1 226 / 674 misses, house 236 / 70
   and 1 / 0, ambient 5 / 2 and 2 / 0 at share 0.5 / 0.1), with the heaviest node itself
   faster: a laptop's power budget spent on spinning is turbo clock taken from the thread
   doing the work. The default is now **0.1** (`Workers::spin_share`), and the pool beat
   the serial engine in all six of those triplets (startup 0 vs 413 and 674 vs 1 896 misses;
   house 70 vs 248, 0 vs 0; ambient 2 vs 4, 0 vs 6).
4. **Spinning workers outside MMCSS** (`--spin 1`: TIME_CRITICAL, 1.5 periods) were the
   worst configuration measured — every song missed more, block p50 rose 1.5–2× (same
   reason, and a TIME_CRITICAL spinner starves its sibling hyper-thread's work). **Bounded
   workers in MMCSS** (rt's recommendation) are the default.
5. **Before the EcoQoS opt-out** the paced runs had the callback seconds late (MMCSS
   demotion cascades): `EngineHandle::start` now opts the process out at the first stream
   start (rt's R1 item, which the app had not wired).

Wakes: with bounded workers every block whose workers parked wakes them (≈ 4 per block at
4 workers, ≈ 22 000 in a 30 s run); never on a block whose workers are still spinning
(`tests/pool.rs` checks it block by block). The ADR 0005 rule 3 amendment's "only at idle →
busy" holds in that sense — a parked worker is idle — but in steady live play at 256 frames
that is every block: one non-blocking `unpark` per worker on the callback (a few µs each)
and a 50–300 µs late start for the woken worker, which is why the critical path's head is
seeded to the callback.

## Tests

`cargo test -p cantus-engine --features fuzz`, every test binary, **at `W = 0`, 1, 3 and 7**
(`CANTUS_WORKERS`): all pass (49 binaries at W = 1 and 3; at W = 7 the profiler's blame test
failed once, fixed — the callback's wait on a parallel pass was charged to "the engine" —
and passes at 0, 1, 3, 7). That includes `null_tests.rs` with the **`v5.cantus` SHA-256
unchanged** and the goldens, `ftz.rs`, `graph_order.rs`, `pdc.rs`, `midi_fx.rs`,
`automation.rs`, `recording*.rs`, `stems.rs`, the `fx_*`/`inst_*` contracts, `limits.rs`
(256 tracks on 256 strips, on the pool), `rt_memory.rs`, `profile.rs`.
Final run on the last commit, `--features fuzz`, `--no-fail-fast`: **51 test binaries, all
green at `W = 0` and at `W = 3`** (W = 1 and 7 ran green on the code before the last two
commits, which changed only the spin share's default and the seeding of the critical
path's head; the W = 7 blame fix was re-run at 0, 1, 3, 7).

New and changed:

- **`tests/pool.rs`** — *parallel = serial, to the bit*: 12 random sessions (1–12 tracks,
  native instruments with random parameters, buses and returns, sends, keys from earlier
  strips, inserts hearing a track's notes, latent inserts, mutes, polarity, swap, a master
  limiter, a fader ride, loops), random block sizes 1–1 024, each at W ∈ {1, 3, 7} with the
  priorities shuffled and clustering switched on or off by seed, against the serial engine;
  the demo and a busy desk at W 1 / 3 (adaptive) / 7; **with `--features fuzz`, the
  PCT-shaped change points of `sched::fuzz` stalling the engine's own pool** at the
  scheduler's sites, 12 more sessions × W ∈ {1, 3, 7}; `schedule_conflicts == 0`
  everywhere; and the wakes: no wake on any block whose workers had no idle moment, exactly
  one per worker after a 200 ms silence. `CANTUS_POOL_SEEDS=n` runs more; a failure names
  its seed and W.
- **`tests/rt_memory.rs`** — the counting allocator also counts on every pool worker
  (`processor::on_worker_start` marks the thread while it serves); new
  `the_pool_threads_never_allocate` (the full scenario — linear-phase EQ swaps, soft bypass,
  the chain switch, a gate played by notes, the click, a strip take — on three workers, zero
  heap events on the callback and the workers); at `CANTUS_WORKERS=3` every test of the file
  counts its workers too.
- **`tests/sched.rs`** — the pool's new API (`for_graph`, the `(slot, period, serve)` start
  hook, `Idle::Spin`), and waiting for workers to join where a test needs them working.
  17 pass (+1 ignored measurement).
- **`cantus-rt` cell tests** (3) — pass, and **pass under Miri** (`cargo +nightly miri test
  -p cantus-rt --no-default-features --lib cell`).
- `tests/render_progress.rs` (export's request), `tests/soak_fixes.rs` (E1, E6).
- `cargo clippy -p cantus-engine -p cantus-rt -p cantus-bench -p cantus-ui --tests
  --features cantus-engine/fuzz`: clean. `cargo check --workspace --all-targets`: clean.

## Also done (requests that came during the round)

- **Export's request** — `render_with_progress(snapshot, opts, units, hosted, &RenderProgress)`
  in `render.rs`: `RenderProgress { done, total, cancel }` (atomics, shared through an
  `Arc`), `fraction()`, `cancel()`; the frames are stored after every block and the cancel
  flag read before each, ending in the new `RenderError::Cancelled`.
  `render_with_instruments` is it with a progress nobody reads (same bits:
  `tests/render_progress.rs`).
- **Soak's E1** (`backend.rs`) — the handle forgets an effect's id as soon as it sends a
  snapshot without it, and an old unit coming back no longer makes it forget an id the last
  snapshot sent has (a fresh unit went with it): a remove and its undo in quick succession
  now deliver a unit, nothing dropped (`tests/soak_fixes.rs`, which fails on the base).
- **Soak's E6** (processor) — a count-in clicks whatever the metronome switch says, and
  only until the recording's start; its click line is planned when a count-in starts
  (`Processor::click_on`, `click_allowed_at`). The app's workaround stays harmless.

## Files outside my area

- `crates/cantus-rt/src/{cell.rs (new), lib.rs}` — the cells (all the new `unsafe`).
- `crates/cantus-engine/src/pdc.rs` — the line pool behind cells (`Lines`, shared by the
  planner and the nodes); `Compensation` keeps its API (`want`, `finish`, `supply`,
  `take_request`) and mirrors each line's edge, length and delay.
- `crates/cantus-engine/src/{backend.rs, messages.rs, lib.rs, profile.rs, render.rs}` —
  the pool started with the stream and the EcoQoS opt-out, E1; `EngineStats::{
  schedule_conflicts, workers, parallel}`, `AudioConfig::workers`; exports; the slot
  profiler and the critical-path blame; `render_with_progress`.
- `crates/cantus-engine/tests/{sched.rs, rt_memory.rs, devices.rs}` (updated), new
  `pool.rs`, `render_progress.rs`, `soak_fixes.rs`.
- `crates/cantus-bench/` — `paced.rs` and the `paced` subcommand, `Rig::with_workers`
  (`Rig::new` follows `CANTUS_WORKERS`), `cantus-rt` moved from dev- to normal dependency,
  `examples/{pool_ab.rs, song_profile.rs}`.
- `crates/cantus-ui/` — `settings.rs` (`AudioSettings::workers`), `views/settings_audio.rs`
  (the *Audio cores* row and its note), `perf_status.rs` (the pool in the readout's
  tooltip), `locales/{en,es}.txt` (6 strings each), and the `workers` field in two struct
  literals of `audio_setup/`.

## Left

- **The demos at 256 frames with zero misses.** Gated by single instruments (above). The
  next step is **voice-level sub-tasks**: Calor, Unda and Exemplar render their voices in
  groups on the pool, each group into its own buffer, summed in voice order (so the bits do
  not move) — the same mechanism as the CLAP `thread-pool` extension, which is not wired
  either. That is `cantus-dsp`'s and the instruments' code (not this builder's) plus a
  sub-queue in `sched/` (`ReadyQueue::push` is the place). A quiet-machine paced run should
  come first: on a quiet core these nodes take roughly half of what was measured here.
- **The CLAP `thread-pool` host extension** (plan Phase 1, *host*): not done.
- **`loom` models** of the deque, the mailbox and the cells (no new dependency this round);
  the cells have Miri, the queues their stress tests and the schedule fuzz (x86 only).
- **"Possibly preempted"** in the blame (§2.9: a node several times its running mean on one
  block) and per-worker thread cycle counters (`QueryThreadCycleTime`): not done; the blame
  names the critical path's heaviest node.
- **Ahead workers, RTWQ vs MMCSS, macOS workgroups (`promote_worker_in`)**: not touched.
- **`xtask perf --full --save` on a quiet machine**: not run (the machine never was); the
  gate's stored baselines are round 20's serial numbers, and `engine.*` will now read the
  pool only when the bench runs with `CANTUS_WORKERS` (the rig follows it; unset, it is
  the serial engine, as the stored baselines are).
- **Clustering's thresholds** (10 µs light, 20 µs bins) are the ADR's; not tuned on
  measured dispatch cost.

## Notes for the integrator

- **Merge order**: anywhere. Everything that touches samples is covered by the null tests
  at every W. `processor*.rs`, `graph/`, `sched/` are wholly replaced/extended here; any
  other branch that edited them this round must be merged by hand (none should have).
- **CI**: add `cargo test -p cantus-engine --features fuzz` with `CANTUS_WORKERS=3` beside
  the plain run (`--test pool` and `--test sched` are the ones that need the feature for
  their stalls); the pool tests take ~2 min on this laptop.
- **The app**: the stream's processor gets `Workers::auto()` (or *Settings ▸ Audio ▸ Audio
  cores*). `CANTUS_WORKERS` set in a user's environment would also give a pool to
  `cantus-plugins`' processors (plug-in builds use `Processor` too); nobody sets it, but a
  plug-in build could call `set_workers(Workers::SERIAL)` to be explicit.
- **ADR 0005 rule 3, as amended by ADR 0031 §2.4**: with bounded workers the idle → busy
  wake happens at every block whose workers parked — at 256 frames, every block. That is
  rt's predicted trade ("the idle → busy wake on every block"), now measured; the ADR text
  should say so.
- `docs/STATUS.md`: round 22's line for *pool* can take the first two paragraphs of
  *What was built* and the honest summary of the paced target.
