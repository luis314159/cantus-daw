# ADR 0031 — The performance architecture: a compiled graph, a real-time pool, rendering ahead, and never doing work twice

**Status:** Proposed · **Date:** 2026-10-05 · **Extends:** ADR 0005 (real-time rules),
ADR 0014/0015 (strips, sends), ADR 0018/0022 (delay compensation, notes into effects),
ADR 0019 (hosting), ADR 0023 (instruments), ADR 0025 (recording), ADR 0026 (the master
chain) · **Plan:** `docs/perf/plan.md` · **Prototype:** `docs/perf/proto/`

## Context

The owner's goal (`docs/BACKLOG-excellence.md` §4b): *more tracks and plug-ins than any
DAW on the same machine, at a lower buffer, with zero dropouts — proved by a published
benchmark, not claimed.* Its principle: **never do work twice, never do work nobody hears,
and never let the real-time path wait.** §4 sets the first bar: a 100-track session with
plug-ins at 128 samples on an 8-core laptop.

What the engine is today (`crates/cantus-engine/src/processor.rs`, read in full for this
ADR):

- **One thread.** `Processor::process` runs on the driver's callback: the control phase
  (messages, MIDI, the input ring, recording marks), then `render`, which splits the block
  into **global segments** at every event of every track, every metronome beat and the
  loop end, and for each segment runs every track, then every strip in
  `Snapshot::strip_order`, then the master chain and the master strip. A session with a
  hundred tracks of sixteenth notes cuts a 128-frame block into dozens of segments, and
  every strip's whole chain is called once per segment.
- **Shared scratch.** `buf_l/r`, `dry_l/r`, `edge_l/r`, `mono` and `ana_pre` are one set for
  the whole processor; contributions are *scattered* into the destination strip's buffer
  (`d.buf_l[i] += …`) as each source finishes. Correct and cheap on one thread; a data race
  on two.
- **Ceilings.** `MAX_TRACKS = 64`, `MAX_STRIPS = 64` (`messages.rs`): the 100-track goal
  does not fit (the bench builder's `engine.s100` plays its first 64).
- **Strips with inserts never rest.** A strip is skipped only when nothing came in, its EQ
  is quiet and it has **no inserts** (`ringing = !st.effects.is_empty() || …`); every strip
  with a plug-in runs every segment of every block, silent or not.
- **Denormals** are flushed in software inside the DSP; the hardware FTZ/DAZ flags are set
  only in the exported plug-ins (`cantus-plugins/src/ftz.rs`), which made Sibilus 6 %
  faster there "without changing a sample" (STATUS, round 17).
- **What must not move:** bit-identical output for the same snapshot, rate and start
  (ADR 0005 §5), held by `tests/null_tests.rs` (block sizes, real time = offline, a SHA-256
  of `v5.cantus` recorded before strips existed), `tests/pdc.rs` (twins cancel to exact
  silence), `tests/midi_fx.rs`, `tests/rt_memory.rs` (zero allocations on the audio
  thread), every unit's block-size independence (`fx_contract.rs`, `inst_contract.rs`:
  blocks of 1, 7, 64, 1000, 4096) and the stems adding up to the master.

The bench builder (`r20/bench`, `docs/perf/benchmark.md`) has written the method — kernel
ns/frame, `engine.s{10,32,64,100}.b128` medians and p99s, a DAWbench-style DSP and VI test,
a CI gate, and an in-engine profiler that charges every nanosecond of a block to one node
and names the culprit of a dropout. Its first numbers (`baseline.md`) were not written yet
when this ADR was; every target below is stated against those metric names so the plan
can be checked by the gate, not by argument.

### How the others do it

| | Parallelism | Ahead of the device | Silence | Plug-in isolation |
|---|---|---|---|---|
| **REAPER** | "live FX multiprocessing on N CPUs" | **Anticipative FX**: renders ahead in background threads, default 200 ms; record-monitored tracks and everything depending on them stay real time; per-track *Prevent anticipative FX*; drawback: plug-in edits and on-screen-keyboard notes arrive 200 ms late [1][2][3] | auto-bypass on silence since 6.71, by reported tail or detection, off for render [4] | per plug-in: native, one shared bridge process, or a dedicated process [5] |
| **Cubase / Nuendo** | Multi Processing | **ASIO-Guard** (Low/Normal/High): channels and instruments that need no real-time path are pre-processed at a larger block; monitoring or record-enabling a channel moves it *and every dependent channel* to the real-time path with a short fade; off per plug-in; external devices never [6][7]; VST3's `kPrefetch` process mode exists for it [8]; the performance monitor shows real-time average, real-time peak and the ASIO-Guard path separately [9] | — | — |
| **Bitwig** | a graph across cores | — | — | *Within Bitwig / Together / By manufacturer / By plug-in / Individually*; a crash replaces the device with *Reload* [10] |
| **Ableton Live** | independent signal paths on separate threads; one track's chain **cannot** be split across cores; tracks tied by sidechains or returns are one path [11] | — (Freeze, by hand) | effects with no input are deactivated after their tail; lowers the average, *not the peak* [12] | — |
| **FL Studio** | multithreaded generator and mixer processing, per plug-in opt-out [13] | — | **Smart disable**, global and per plug-in; can break time- or envelope-based plug-ins [13][14] | *Make bridged*: a separate process, ~1–2 % CPU each [14] |
| **Logic Pro** | "Playback Tracks" vs "Playback & Live Tracks" threading [15] | non-live tracks at the *Process Buffer Range*; the **selected** instrument track is live — Apple's advice is to select an empty track for playback [16] | — | — (AU out-of-process on Apple silicon) |
| **Ardour** (open source) | a process graph with a trigger queue, worker threads woken by a semaphore every cycle (`libs/ardour/graph.cc`; from its source, not re-fetched for this ADR) | — | — | — |

What nobody ships, and §4b asks for: an automatic, invalidating **render cache** (all of
them freeze by hand), a dropout **with the node to blame** (Cubase 14 lists tracks with
high load; nobody names the plug-in at the dropout), and anticipation that is
**bit-identical** to the live path and to the export. Those are where Cantus can be first
rather than equal.

Sources (fetched 2026-10-05; the proxy blocked steinberg.help, reaper.blog, the Cockos
wiki and dlz.reaper.fm, so [1][3][4][5] rest on search excerpts of those pages):
[1] cockos.com/wiki Preferences_Audio_Buffering ·
[2] soundonsound.com/techniques/running-multiple-plug-ins ·
[3] kvraudio.com/forum/viewtopic.php?p=4950007 ·
[4] REAPER 6.68→6.72 change notes (dlz.reaper.fm) ·
[5] reaper.blog/2012/02/run-plugin-as-dedicated-process ·
[6] archive.steinberg.help/nuendo/v8 … asio_guard_c.html ·
[7] helpcenter.steinberg.de/hc/en-us/articles/206103564 ·
[8] github.com/steinbergmedia/vst3_pluginterfaces `vst/ivstaudioprocessor.h` ·
[9] helpcenter.steinberg.de … 4454943549330 (Audio Performance Monitor) ·
[10] bitwig.com/userguide/latest/vst_plug-in_handling_and_options ·
[11] help.ableton.com/hc/en-us/articles/209067649 ·
[12] ableton.com/en/live-manual/12/computer-audio-resources-and-strategies ·
[13] image-line.com … envsettings_audio.htm ·
[14] image-line.com … plugins/wrapper.htm ·
[15] support.apple.com/en-ie/101975 · [16] support.apple.com/en-us/108295.
Standards: CLAP `process.h`, `audio-buffer.h`, `ext/{tail,thread-pool,render,thread-check}.h`
(github.com/free-audio/clap); OS: learn.microsoft.com `avrt` (AvSetMmThreadCharacteristicsW,
AvSetMmThreadPriority) and *Multimedia Class Scheduler Service*; developer.apple.com *Adding
parallel real-time threads to audio workgroups*, WWDC20 10224; man7.org `sched(7)`; rtkit
README.

## Decisions

### 1. The graph: a static task graph compiled per topology, run whole-block per node

**1.1 Nodes.** The processor becomes a set of nodes, each owning its state and its
outputs:

| Node | Owns | Runs |
|---|---|---|
| **Track** *t* | the slot's instrument (Firmus, Pulsus or a native unit; a hosted instrument, ADR 0028), its event cursor, the track's PDC line (`Edge::Track`), its note *outbox* | fires its own events at their samples, renders the instrument, adds audio clips and the monitored input (`Live::add_track_audio`), applies level and balance, runs its line; writes its **output** (stereo) and its peak |
| **Strip** *s* | inserts, slot switches, EQ, the strip's PDC lines (`Out`, `Send(k)`, `Dry(i)`, `Capture` when it is the take), its key buffer, each listening insert's note queue | **gathers** its inputs (§1.3), polarity → swap → inserts → EQ → width → pan law; writes its **post-chain** pair plus the gains `(gl, gr)`, its key buffer (`l·gl`), each *delayed* edge's buffer, the capture frames |
| **Master** | the master chain (ADR 0026) and its dry lines, the click and its line | gathers what feeds the master, runs the chain, the master strip, the master take, the click; writes the device's interleaved buffer |

The **control phase** stays serial on the callback, before the graph: draining messages
and MIDI, popping the input ring, recording marks, `record_segment` (the armed tracks'
input into their rings), transport changes, live notes. It is short and it is where
everything that is *now* enters.

**Edges.** *Audio*: track → strip or master; strip → its output; send `k`. *Key*: the key
strip → the keyed strip or master (already ordered: `Project::strip_order` treats a
sidechain as a route). *Notes*: a track → every node with an insert that listens to it
(ADR 0022) — new, because the notes now travel by outbox (§1.5) instead of a shared
queue. Every edge is a dependency; that is the whole scheduling contract.

**1.2 Who compiles what.** `cantus_engine::graph::compile(&Snapshot) -> GraphPlan` runs on
the **UI thread** (it allocates and sorts): node list in snapshot indices, predecessor
counts, successor lists, each node's *canonical input list* (§1.3), critical-path ranks
(§2.3), the static half of anticipation eligibility (§3.2), and the buffer colouring
(§1.4). The plan carries a `topology_key` (a hash of exactly what it was computed from:
routing, sends, keys, `midi_from`, node counts — never a parameter value), so a knob turn
compiles **no** plan; `EngineHandle::set_snapshot` sends a plan only when the key
changes, in the same message as the snapshot (`SetSnapshot { snapshot, plan:
Option<Box<GraphPlan>> }`). On the audio thread, `install_snapshot` keeps doing what only
it can — slot reuse so voices and tails ring across snapshots — and then maps the plan's
snapshot indices to slots into preallocated arrays (no allocation); the old plan goes back
as garbage. **Delay compensation stays on the audio thread** (`compensate`), because it
reads the live units' latencies; the plan does not depend on latency.

**1.3 Canonical summation order — why parallel equals serial to the bit.** Floating-point
addition is not associative, so "parallel gives the same result" is a statement about the
*order of additions*, and the order is defined here once:

> A node's input is an accumulator that starts at **+0.0** and receives its contributions
> **in canonical order**: first the tracks that feed it, in snapshot track order; then,
> for each strip in `strip_order`, that strip's output edge if it leads here, then its
> sends `k = 0, 1, …` that lead here. On a mono device the master adds each contribution's
> left, then its right, into the one channel, as `render` does today.

That is exactly the order in which today's serial `render` *scatters* into a strip's
buffer, so a gather in that order reproduces today's sums bit for bit. Each contribution is
computed by the same expression as today — `l[i] * gl` for an undelayed edge (now evaluated
by the consumer from the producer's post-chain pair and gains), the line's output at unity
for a delayed one (`x * 1.0 == x`). Two consequences, both exact:

- An all-zero contribution may be skipped. The accumulator starts at +0.0, and under
  round-to-nearest a sum is −0.0 only when both addends are −0.0, so the accumulator is
  never −0.0, and `x + (±0.0) == x` for every other `x`. Hence an idle source (today's
  `continue`) and a muted one (today's `l · 0`) need no special case.
- The schedule does not matter. Which worker runs a node, and when, cannot change a sample:
  a node's output is a function of its state and its gathered input, and the gather order
  is fixed by the plan.

**1.4 Buffer ownership and liveness.** Every buffer has exactly one writer, and readers
only read after the writer's node has finished (the edge is a dependency):

- a node's **output** pair (a track's leveled signal, a strip's post-chain pair, the gains
  beside it), its **delayed-edge** buffers (one per edge with a line), its **key** buffer,
  a track's **note outbox**: written by the node, read by its successors;
- **scratch** (`dry`, `edge`, `mono`, the analyser's input): per *worker*, not per
  processor, so no two nodes running at the same time share one;
- **meters and telemetry**: per node, cache-line padded (§5.4), folded into `Meters` by the
  callback after the graph.

*Phase 1 gives every output its own buffer* — no reuse. At 128 frames a stereo pair is
1 KiB; 256 tracks and 256 strips are half a megabyte, less than one P-core's L2 (1.25 MiB
on the 12700H this was written on). Sharing buffers would save memory nobody is short of
and buy hazards. *Reuse by liveness* starts with the large blocks of anticipation and
export (§3, §7), where a pair is 8–64 KiB: buffer *b* of node *u* may be reused by node *v*
only if **every** consumer of *u* happens-before *v* in the graph (reachability, not a
serial order — a parallel schedule may run any two unordered nodes at once). The plan
computes reachability as bitsets (512 nodes → 32 KiB) and colours buffers greedily in
critical-path order; the slab is sized by the colour count.

**Memory comes like delay lines.** The node buffers live in one slab owned by the engine,
sized for the plan's needs at `MAX_BLOCK_FRAMES`; a plan that needs more than the slab
holds is answered the way `PdcNeed` is (ADR 0018): the audio thread asks
(`EngineEvent::GraphNeed`), the UI thread allocates, the old slab returns as garbage, and
until then the plan that fits keeps running. `MAX_TRACKS` and `MAX_STRIPS` rise to **256**
each; the per-slot fixed arrays become slab entries, so the ceiling costs nothing until it
is used.

**1.5 Each node at its own segments.** The global segment loop goes. A track splits its
block only at **its own** events (and at a loop wrap, which stays global: every track
releases and re-seeks there, so a wrap ends a *pass* of the graph and the next pass starts
at the loop start). A strip, the master and every insert run **whole passes** — usually the
whole block. The click splits only itself. Why this keeps the bits:

- every native unit is already required to be split-invariant: `fx_contract.rs` and
  `inst_contract.rs` render every kind at blocks of 1, 64 and 1000 to the same bits, and
  the units' own tests go to 7 and 4096. An instrument still receives its notes
  *between* `process` calls at exactly the same samples; what changes is only that its
  calls are no longer *also* cut at other tracks' events, which is a split the contract
  already makes irrelevant;
- notes into effects carry an *offset* "always inside" the next `process` call — ADR 0022
  wrote that delivery is "independent of the segment structure — a whole-block strip stage
  (planned after this round) keeps it sample-accurate with no change to the contract".
  This is that stage. The shared `fx_notes` queue becomes a per-track **outbox** (written by
  the track node: the note, its absolute due sample) and a per-insert queue owned by the
  listening node, which drains the outboxes of the tracks it listens to (they are its
  predecessors) and delivers each note at `due − pass start`. Live notes and
  all-notes-off are written into the outbox by the control phase.

Hosted plug-ins get fewer, longer `process` calls, which CLAP and VST3 both prefer
(ADR 0019 left "whole-block processing for hosted units" open: this closes it); their
song position is the pass's start (a pass never crosses a loop wrap).

What it buys before a single thread is added: a 128-frame block of a busy session today
calls every strip chain once per global segment (tens of times); after this it calls each
once. Phase 0 lands this in the **serial** engine, alone, under every existing null test
and the `v5.cantus` hash — the riskiest change of the plan done first and with no
concurrency in the way.

### 2. The multicore scheduler: a real-time pool that never makes the callback wait for a wake-up

**2.1 Threads.** The driver's callback thread plus **W live workers**: by default the
number of *physical performance cores* **minus two** — the callback takes one core and one
is left to the OS, the UI and the driver's own threads — at least 1, at most 14. The
prototype showed why the second one matters: six spinning threads on six cores let any
other thread preempt one mid-node, and the block's 99th percentile went from 2.05 ms to
97 ms (§2.10). Hyper-threads
are not counted: DSP is floating-point bound, a sibling shares the core's FP units, and
what matters at 64 samples is the worst block, not throughput. On a hybrid CPU (the
12th-gen laptop this ADR was measured on: 6 P-cores with two threads each, 8 E-cores) the
live workers go on P-cores and the **ahead/background workers** (§3, §4) on E-cores and
whatever is left. The count is a setting with this default; `W = 0` is the serial engine.

**2.2 The block protocol.**

1. The callback runs the control phase (§1.1).
2. It *arms* the block: each node's pending count is reset from the plan, `remaining` is
   set to the node count, every node with no predecessor is pushed — critical path first
   (§2.3) — and a block **epoch** is published (`Release`).
3. It **helps**: the callback is a worker like the others for this block. It pops a node,
   runs it, decrements each successor's pending count, pushes those that reach zero.
4. When the queue is empty and `remaining` is not zero, the callback spins (`spin_loop`)
   until it is: what it waits for is nodes **already running** on other workers, never a
   worker waking up. A worker that wakes late finds nothing to do, or less.
5. When `remaining` is zero, the callback reads the master's output, folds the meters,
   reports, and returns.

The callback never blocks, never sleeps, never calls the OS to wait. The worst it can do is
spin on a node that a worker is running, which is bounded by that node's own cost — the
same cost it would have paid running the node itself. The one way a worker can hurt it is
by being **preempted mid-node**; that is what priority (§2.6) and the dropout blame (§2.9)
are for, and why the queue gives the critical path to whoever is first.

**2.3 The ready queue.** A bounded multi-producer multi-consumer array queue of `u32` node
indices (Vyukov's algorithm), fixed capacity, allocated with the plan. Each cell is a pair
of atomics, so it is **safe Rust**: no `unsafe` (the prototype's `queue.rs` is 80 lines).
Two queues, *critical* and *normal*, popped in that order; a node's rank is the longest
path from it to the master, weighted by its measured cost (the profiler's running mean,
§2.9), recomputed off the audio thread every second, and the top quarter is critical. A
worker that makes exactly one successor ready keeps it in a **continuation slot** and runs
it next (its input is hot in that core's cache) instead of queueing it — the useful half of
work stealing without its machinery.

Why not the usual crates (sources in the research notes of this round):

- **rayon**: idle workers sleep on a `Mutex` + `Condvar` and waking one takes the lock;
  `spawn` boxes its job; its injector allocates blocks as it fills. Not real-time safe.
- **crossbeam-deque** (Chase–Lev): `push` doubles the buffer when full (allocates), `pop`
  halves it when a quarter full, and old buffers are freed later through
  `crossbeam-epoch`, whose collection can run on any pinning thread — the audio thread
  included. Not real-time safe either, and not needed: a block holds a few hundred
  coarse tasks (microseconds each), not the millions of nanosecond tasks work stealing
  was designed for. If the profiler ever shows queue contention above 2 % of a block, a
  per-worker fixed-capacity deque is the next step, written in `cantus-rt` (§2.7).

**2.4 Idle, spin, park.** A worker that finishes a block spins on the epoch for a *time*
budget — 1.5 device periods, so it is spinning when the next block comes — then yields,
and **parks** only after an idle grace (transport stopped and the graph silent for 100 ms,
or the adaptive rule below running the graph serially for 50 blocks). Parking is what
keeps a laptop cool and its battery alive when nothing plays; spinning is what makes a
block start on every core at once. The prototype measured both on this laptop, pinned to
the six P-cores at High priority (§2.10): a
spinning worker sees a new block **2.1 µs** (median) after the callback publishes it; a
parked one **55 µs** (median, 315 µs p99) — 4 % of a 64-sample budget, acceptable once
after silence, not every block. And spinning has one hard rule: six spinning threads on
six cores made the median **17.7 ms**, because nothing else could run without preempting
a spinner. Spinners never outnumber the cores they are given (§2.1).

Waking a parked worker takes a system call (`WakeByAddressSingle`, a futex wake,
`semaphore_signal`). The callback makes it **only** at an idle → busy transition, once per
parked worker, never waits for its effect, and never on a block that already has its
workers spinning. **This amends ADR 0005 rule 3** ("no syscalls"): *no system call that can
block, and none in steady state; the scheduler's non-blocking wake at an idle → busy
transition is the one exception, counted in `EngineStats::wakes`.* Ardour wakes its graph
workers through a semaphore on every cycle; we do it only after silence.

**2.5 Serial when parallel does not pay.** Dispatching costs (prototype, pinned):
the queue and counters alone cost **51 ns per node** run serially,
and **310–530 ns per node** with one to four workers contending on one queue over nodes that
do nothing (385 nodes: 120–200 µs per block, 15 % of a 64-sample budget). That is the worst
case — real nodes keep the queue mostly idle — but it decides two things. First,
**clustering**: the plan fuses nodes the profiler measures under ~10 µs into *tasks* — a
light track with the strip it feeds, a run of light strips in one chain, siblings that
feed the same bus — executed in topological order inside the task, which keeps §1.3's
gather order; a task aims at ≥ 20 µs, so dispatch stays under 3 % of its cost. A
100-track session of light tracks is then tens of tasks, not hundreds. Clusters are
recomputed with the ranks (§2.3), off the audio thread, and sent as an array; changing them
changes no sample.

Second, **the serial fallback**: when the previous 32 blocks' load was under 25 % of the budget, or the plan's
critical path is more than 80 % of its total cost (one long chain), the callback runs the
graph alone, in rank order, with the workers left spinning (or parked after the grace).
Both ways give the same bits (§1.3), so the switch can happen at any block; it has
hysteresis (enter parallel above 35 %).

**2.6 Priority and placement.**

- **Windows**: every live worker calls `AvSetMmThreadCharacteristicsW(L"Pro Audio")` and
  `AvSetMmThreadPriority(AVRT_PRIORITY_HIGH)`; ahead workers take "Pro Audio" at
  `AVRT_PRIORITY_LOW`, background freeze threads "Audio" at `NORMAL`. Placement by **CPU
  sets** (`SetThreadSelectedCpuSets`, which the scheduler treats as a preference and
  which does not fight the power manager the way a hard affinity mask does), choosing
  P-cores from `GetSystemCpuSetInformation` (`EfficiencyClass`) and one logical processor
  per core. The callback thread belongs to the driver (ASIO/WASAPI) and is never moved.
  `SystemResponsiveness` still reserves 20 % for normal work by default; the setup chip
  (§6b of the backlog) may say so, Cantus does not change it.
- **macOS**: `thread_policy_set(THREAD_TIME_CONSTRAINT_POLICY)` with period = device
  period, then **join the device's audio workgroup** (`kAudioDevicePropertyIOThreadOSWorkgroup`,
  `os_workgroup_join`) — Apple's documented way for "parallel real-time threads" to be
  scheduled against the device deadline on Apple silicon; ahead workers create their own
  `AudioWorkIntervalCreate` interval. `audio_thread_priority` (Mozilla) does the Mach
  policy but **not** workgroups, so this is ours.
- **Linux**: `SCHED_FIFO` via rtkit (which requires `RLIMIT_RTTIME` and
  `SCHED_RESET_ON_FORK`, caps the priority and watches with a canary), or directly when
  `RLIMIT_RTPRIO` allows; otherwise the pool runs at normal priority and the setup chip
  says so. `audio_thread_priority` does exactly this and is used for it.

**2.7 Unsafe code, and where it is allowed.** `cantus-engine` keeps
`#![forbid(unsafe_code)]`. The scheduler is safe: the queue is atomics; a node's state is
behind an **`AtomicRefCell`** (the `atomic_refcell` crate, MIT/Apache, one atomic per
borrow): a worker takes `try_borrow_mut` of the node it runs and `try_borrow` of its
inputs' outputs. Neither can block. The schedule guarantees they succeed; a failure is a
scheduler bug, counted (`EngineStats::schedule_conflicts`, asserted zero in every test),
and the node is skipped as silence rather than panicking (ADR 0005 rule 3).

What genuinely needs `unsafe` goes into one new crate, **`cantus-rt`**, the way
`cantus-plugins` and `cantus-host` already hold the FFI they need: MMCSS, CPU sets,
Mach policy and workgroups (FFI); the **denormal guard** (moved from
`cantus-plugins/src/ftz.rs`, re-exported there); the anticipation rings (§3.4); the
sandbox's shared memory (§6). Rules for it: every `unsafe` block has a `// SAFETY:` line;
every type with interior unsafety has a Miri test and a `loom` model test; the crate stays
under 2 000 lines; nothing in it knows about audio.

**2.8 Hosted plug-ins on workers.** CLAP's `thread-check` says "audio thread" is a role,
and the host may use a pool as long as one instance runs on one thread at a time; ADR
0019's thread check is already a role raised for each audio-half call, so it holds on any
worker. One instance per node and one node per worker at a time is exactly CLAP's rule.
VST3 asks the same. **CLAP `thread-pool`**: a heavy plug-in's `request_exec(n)` is served
by our pool — the calling worker pushes `n` sub-tasks to a per-node sub-queue the other
workers drain, helps with them itself, and returns when they are done (the extension
already blocks the caller "until all tasks are done"; here it waits only on tasks that are
running, as the callback does). A plug-in that parallelises itself through us instead of
its own threads does not oversubscribe the machine. Nested requests are refused; the
plug-in then runs its own loop, as the extension allows.

**2.9 Blame, per node and per worker.** The bench builder's profiler charges each
nanosecond to one node; with the pool it charges each node on the worker that ran it,
and a dropout's culprit becomes the **critical path** of the late block — the chain of
nodes that finished last — named as today ("Spatium on Vocal took 3.1 ms of 2.7 ms").
A node whose wall time is several times its running mean on that block only (the next
blocks are normal again) is reported as *possibly preempted*, with the plug-in's name
beside it rather than as its fault: a dropout caused by the OS must not be pinned on a
plug-in. Each worker's thread cycle counter (`QueryThreadCycleTime`, read once per block
by the worker, off the callback) separates the two cases when it matters.

**2.10 What the prototype measured.** `docs/perf/proto/forkjoin` is this section in 300
lines of safe, std-only Rust: the Vyukov queue, pending counters, an epoch, spinning or
parked workers, a callback that helps, and nodes that gather their inputs in canonical
order and run biquads. Run on the i7-12700H this was written on, pinned to one thread of
each of the six P-cores at High priority (`start /high /affinity 555`) — **while other
builders compiled on the same machine**, so absolute times are about three times a quiet
machine's (a biquad sample cost ~14 ns, against ~4 ns quiet) and the tails are the
neighbours' as much as ours; the ratios are the result (`docs/perf/proto/README.md`):

| Measure | Result |
|---|---|
| 100 tracks + 16 buses + master, 2 832 biquad passes per block: speed-up over serial (median) | 2 threads 1.98×, 3: 3.09×, **5: 4.73×**; 6 threads on 6 cores: 3.90× and a p99 of 97 ms |
| Same, p99 per block | serial 6.7 ms → 5 threads 2.05 ms |
| Parallel = serial, bit for bit, 500 blocks | identical at 1, 3, 7 and 13 workers |
| A worker sees a new block | spinning 2.1 µs, parked 55 µs (medians) |
| Scheduling cost per node, nodes doing nothing | serial 51 ns; 1–4 workers 310–530 ns |
| Relaxed `AtomicU32` copy vs plain copy | 2.5–2.8 ns vs 0.6–0.9 ns per sample |

One bug the prototype found is a rule for the real one: **seed every node without a
predecessor**, not just the tracks — a bus nothing feeds has none, and the first version
hung waiting for it.

### 3. Anticipative processing: the work nobody is playing is done before it is needed

REAPER's anticipative FX and Cubase's ASIO-Guard are the single biggest lever the market
has: what is not being played live is computed ahead, in large blocks, on whatever core is
free, so the device's deadline only governs the little that is live. Cantus does it with
one idea that also makes it exact: **every node has one render cursor**.

**3.1 Cursors and rings.** Each node renders its timeline strictly in order — its state
admits nothing else — and records `cursor`, the absolute engine sample up to which its
output exists. Its outputs (the post-chain pair and gains, delayed-edge buffers, key
buffer, note outbox, meter values) go into a **timestamped ring** of `H + C` frames, where
`H` is the horizon and `C` the ahead chunk. Whoever renders a node next — the live graph
or an ahead worker — renders it from `cursor`; consumers read any range below `cursor`.

- **The live graph** (§2) needs every node's output for `[now, now + b)`. A node whose
  cursor is already past `now + b` is *read from its ring*, not run. A node whose cursor is
  at `now` is run by the live graph as in §2.
- **Ahead workers** (low-priority threads, §2.6) look for nodes that are *eligible* (§3.2)
  and whose inputs all have cursors past the chunk they would render; they claim one,
  render `[cursor, cursor + C)` in one call per pass (chunks of `C` = 512–2048 frames), and
  publish the new cursor (`Release`). A chunk ends at or before `now + H`.

**The claim rule** keeps the two from waiting on each other: an ahead worker may claim a
node only while `cursor ≥ now + G`, where the guard `G` is two device blocks **plus four
times the node's measured chunk time** (the profiler knows it). The live graph needs
`[now, now + b)`, so a node an ahead worker holds has, unless something extraordinary
happens, already rendered what the live graph needs from it — the live side reads the ring
and does not wait for the chunk to finish. The extraordinary case is the OS preempting the
ahead worker mid-chunk for longer than the guard: then the live graph finds the node held
and its cursor short of `now + b`. It spins until the block's *late* threshold (half the
remaining budget), and if the cursor still has not arrived it mixes that node's
contribution as silence for this block — a local, counted glitch (`EngineStats::ahead_late`,
named by the blame like a dropout) instead of a whole-output dropout — and doubles that
node's guard. Ahead workers live on cores the live workers do not use (§2.1), so our own
threads never cause this; only the rest of the system can. A node that falls behind the guard (the machine is overloaded, the
node just became eligible, the transport just started) is simply run by the live graph
until the ahead workers get in front again: **anticipation degrades into the engine of §2,
never into a dropout of its own making.** Eligibility does not even need to be propagated
by hand: a node downstream of a live node can never get ahead, because its input's cursor
never does.

**3.2 Who may render ahead.** A node is *live* — rendered only by the live graph — when
any of these holds; everything else is eligible:

| Live because | Today's field |
|---|---|
| a track whose input is monitored now (On, or Auto while armed and stopped or recording) | `Live::tracks[slot]` |
| the track receiving live notes, and its echoes | `live_track`, `live_echo` |
| a node keyed from, listening to the notes of, or fed by a live node | the plan's edges |
| a node whose editor is open, or whose parameter is being dragged (a gesture is open) | UI → `EngineMessage::SetPromoted` |
| a hosted plug-in that says it needs real time (CLAP `render.has_hard_realtime_requirement`), one the user marked *never ahead* (REAPER's and Cubase's per-plug-in switch), and external hardware inserts | plug-in flags |
| the master, when anything is live | — |

When nothing is live — playback of a finished song, the commonest case — **the whole graph
including the master renders ahead**, and the callback copies the master's ring to the
device. That is how REAPER plays sessions others choke on.

**The selected track.** The UI makes the selected track the keyboard's target
(`cantus-ui/src/app.rs`: `set_live_track(self.selected_track)`), which is Logic's model —
the selected instrument track is live, and Apple's advice is to select an empty track for
playback. Cantus keeps the convenience and drops the trap: the live track counts as live
only while something can play it live — a MIDI input port is open, or the computer
keyboard is playing notes — so with no controller attached, selecting a track changes
nothing. When it is live, only it and its path to the master leave the ahead path.
Changing the selection during playback promotes the new track within `H`, which is shorter
than the time it takes to reach for the keyboard.

**3.3 How far ahead.** A setting with three levels, like ASIO-Guard's:

| Level | Chunk `C` | Horizon `H` | Edit latency (≤ `H`) | Ring memory per node (stereo f32) | 256 nodes |
|---|---|---|---|---|---|
| Low | 256 | 1 024 (21 ms) | ≤ 21 ms | 10 KiB | 2.5 MiB |
| **Normal** (default) | 1 024 | 4 096 (85 ms) | ≤ 85 ms, ≈ 0 with promotion | 40 KiB | 10 MiB |
| High | 2 048 | 16 384 (341 ms) | ≤ 341 ms | 144 KiB | 36 MiB |

(48 kHz; at 96 kHz frames double and the times stay.) Ring memory is `(H + C) × 2 × 4 B`
per node plus its delayed-edge and key buffers when it has them; it is allocated with the
plan's slab (§1.4) on the UI thread. `H` is not output latency — **anticipation adds no
latency to what is heard**: delay compensation, the heard position and the click are
exactly as without it. It is only how late a change to an ahead node can be.

**3.4 Meeting in time.** Everything a consumer reads is addressed by absolute engine sample,
so the ahead and live paths meet without any arithmetic beyond today's: an edge's delay
line belongs to its source and is rendered ahead with it; a consumer reading
`[now, now + b)` of a producer's ring gets exactly what the producer would have handed it in
a serial engine. Notes travel in the outbox with their absolute due sample (§1.5). Meters
and telemetry are stored per chunk with their sample and published by the callback when
`now` reaches them — the meter moves with what is heard, not with what was computed.
The rings are single-writer, many-reader, written below a cursor published with `Release`
and read with `Acquire`: a slice copy, not per-sample atomics (the prototype measured a
per-sample `AtomicU32` copy at 2.5–2.8 ns per sample, 3–4× a plain copy, which is too much for
large chunks), so the ring is one of `cantus-rt`'s few unsafe types, with Miri and `loom`
tests of exactly that protocol.

**3.5 Edits, seeks and the transport.** Three classes:

1. **A node's own parameters** (an effect's spec, a strip's gain/pan/width/mute, a send
   level, an instrument's parameters, automation values): the node applies a new
   snapshot's values at its **own cursor**. A live node hears it at once; an ahead node up
   to `H` later — unless it was promoted when its editor opened or the gesture started
   (§3.2), which takes at most `H` and happens before the first value arrives in practice.
   Automation (ADR 0027) is a function of song time in the snapshot, evaluated at the
   chunk's time like everything else: nothing special.
2. **Structure** (routing, a node added or removed, a latency change and its new
   compensation, tempo, the loop region, an instrument swap): applied at one **switch
   point** `S` — the furthest cursor of any ahead node, at most `now + H` — for every node,
   live ones included (the live graph keeps the old plan until `S`). Every sample is then
   produced by one consistent graph, and a run with a structural edit equals, to the bit, a
   serial engine given the same edit at sample `S` (the engine reports `S`; a test holds
   it).
3. **The transport** (stop, play, seek, panic, a loop wrap the user forces): these cannot
   wait `H`. The rings beyond `now` are **dropped**, every cursor is reset to `now`, and
   the transport change applies as today (release all, re-seek). The nodes' states are
   ahead of `now` by up to `H`, so what follows is not the continuation of what was in the
   ring: for the first `X = 128` samples the output crossfades from the dropped ring's
   content (which still holds the "would-have-been" audio) to the new rendering, so a stop
   never clicks. Play from stopped starts every node at its cursor with nothing ahead; the
   live graph renders them until the ahead workers have caught up (§3.1's degradation),
   which takes `H × f / (1 − f)` of real time when keeping pace needs a fraction `f` of
   the ahead workers — 85 ms at Normal for `f = ½`.

**3.6 What it keeps exact.** With no edit during playback, *ahead == live == offline* to the
bit: a node's output depends only on its state and inputs, which are the same; only the
`process` call lengths differ, which the block-size contracts make irrelevant (§1.5). Tests:
the null corpus rendered at each level with a scheduler that randomises chunk sizes; a
structural edit equals a serial edit at the reported `S`; a stop's crossfade is bounded;
`rt_memory` holds for the callback and every worker.

**3.7 Hosted plug-ins ahead.** VST3 gets `processMode = kPrefetch` while it runs ahead and
`kRealtime` when live (switching needs no `setupProcessing`, per the SDK); CLAP has no
prefetch mode and runs in its realtime render mode, since rendering ahead is still
real-time quality. A plug-in whose output changes with *wall time* rather than its input
(a network or hardware device) declares it (`has_hard_realtime_requirement`) or is marked
by the user, and stays live.

### 4. The render cache: automatic background freeze

Every DAW freezes by hand (Ableton's *Freeze Track* even locks the device's parameters).
Cantus freezes by itself, invisibly, and exactly: a node whose inputs have not changed
since it was last rendered is **played from its earlier output** instead of being computed.
Heavy synths and long reverbs cost nothing once they stop changing; any edit invalidates
exactly what it touches.

**4.1 What is cached, and where.** The output of an *expensive* node at its **pre-fader**
point, so that the cheapest and most frequent edits — a fader, a pan, a send level, a
mute — never invalidate it:

- a **track**: the instrument with its clips, *before* the channel's level and balance;
- a **strip**: the gathered input through the insert chain, *before* the EQ, width, pan
  and fader (the EQ is the strip's cheapest stage and the most often touched);
- never a live node (§3.2): its input is not reproducible.

"Expensive" is measured, not guessed: the profiler's running cost of the node (§2.9); the
nodes with the highest cost per cached byte are cached first, within the disk budget
(§4.6).

**4.2 The fingerprint: a chain over time pages.** The song is cut into **pages** of
2¹⁵ frames (0.68 s at 48 kHz), counted from engine sample 0 of the timeline. A page's key
is a 128-bit hash of everything its output can depend on:

```
key(node, k) = H( DSP_VERSION, sample rate, tempo map,
                  node's static key   — kinds, every parameter value, bypass/mix/switches,
                                         the instrument spec, sample and asset content
                                         hashes, a hosted plug-in's id, version and state
                                         hash, the delays of its own lines,
                  page inputs (k)     — a track: its events, automation and audio-clip
                                         windows (with source content hashes) that touch
                                         page k; a strip: for every input in canonical
                                         order, key(pred, k) *and* what the predecessor
                                         does after its own cache point (a track's level
                                         and balance, a strip's EQ, width, pan, fader,
                                         send level — and their automation on page k);
                                         the same for its key source and note source,
                  state (k)           — SETTLED if the node was at rest (§5.1) at the
                                         page's first sample, else key(node, k − 1) )
```

The `state` term is what makes it exact: a page depends on everything before it *unless*
the node was at rest when the page began, and every render records, per page, whether it
ended at rest. So the chain restarts at every **settle point** — the silence between
phrases, the end of a reverb. After an edit on page *j* nobody knows yet where the new
render will come to rest (the edit may lengthen a tail), so the pages from *j* on are
re-rendered until one ends at rest; from there the old keys are recomputed equal and the
old pages are used again. An edit at bar 30 costs the pages from bar 30 to the node's next
settle point, not the rest of the song. The key function destructures every snapshot
field it reads without `..`, so a field added to the model fails to compile until someone
decides whether it belongs in the key.

**4.3 Playing from the cache.** A cached node is an *ahead* node (§3) whose "render" is a
copy: an ahead worker copies the page from memory into the node's ring. Switching between
cached and computed respects the node's state:

- **computed → cached** at any page boundary: the output is known, the state is simply no
  longer advanced (the node becomes *cold*);
- **cached → computed** only at a page whose start is a settle point (the node is reset,
  which is its state there by definition) — or after **warming**: an ahead worker runs the
  cold node from the last settle point before the transition, output discarded, so its
  state is right when playback arrives. If it cannot finish in time, the node computes from
  where play started instead: slower, never wrong.

Pages are read from disk by the streaming reader (the backlog's disk item) into a pool of
preallocated page buffers, at least two seconds ahead of the playhead; the audio side only
ever sees resident pages. *Play* waits for the first page of each cached node, as disk
streaming waits for its first buffers (on NVMe, 100 nodes × 256 KiB is a few tens of
milliseconds). A page that is not resident in time is a **disk overload**, reported like
one ("the disk is too slow for the render cache: Pad"), and the node falls back to
computing from its last settle point.

**4.4 Writing it: for free while playing, in the background when idle.** (a) Whatever an
eligible node renders during playback is copied, as a by-product, into a write-behind ring
that a disk thread drains (no I/O on any real-time thread; a full ring drops the page from
the cache, never audio). (b) When the transport is stopped, **background freeze** threads
(lowest priority, E-cores first; §2.6) render invalid pages of the most expensive nodes with
the offline executor (§7), in large blocks, starting from each node's settle points and
nearest the playhead first. It stops the moment the user plays and resumes after.

**4.5 Hosted plug-ins.** Native units are deterministic by ADR 0005. A hosted plug-in is
cached only when both hold:

1. **Its state is fingerprinted.** Parameter values are in the snapshot; for state that is
   not a parameter, the host saves the plug-in's state on the main thread when it says it
   changed (CLAP `state.mark_dirty`, VST3 `setDirty` / `restartComponent`) and hashes the
   blob. A plug-in that never says so is hashed on each save of the project, and is cached
   only between saves.
2. **It passed the determinism probe.** The first page cached for a plug-in is rendered a
   second time by a fresh instance loaded with the same state (the offline render already
   builds such instances, ADR 0019) and the two compared bit for bit. Different — a
   random seed from the clock, analog drift, internal threads racing — and the plug-in is
   **never cached**, which the plug-in manager says ("its output differs between runs").
   The user can override in both directions.

**4.6 On disk.** Content-addressed, per user, shared across projects:
`%LOCALAPPDATA%\Cantus\cache\render\ab\abcdef….page`, one file per page, named by its key.
Undo restores old keys, so undoing an edit finds its pages still there — the cache does not
just survive undo, undo is free. A page file: a 64-byte header (magic `CNTSRPG1`, format
version, rate, frames, channels, flags `silent` / `settled_at_start` / `settled_at_end`,
the key, a CRC-32 of the payload), then planar little-endian `f32` (left, then right) —
256 KiB a page, **nothing** for a silent one. Raw, not compressed: a general-purpose
lossless codec saves little on float audio and costs CPU on the read path; revisited if
the budget bites. Budget: a setting, default 10 GiB, least-recently-used eviction weighted
by the node's cost per byte. A three-minute stereo node is 69 MB; a 100-node song fully
cached is about 7 GB, so the budget is what makes "cache the expensive nodes first" a rule.

**4.7 Trusting it.** A stale page is the worst bug a DAW can have — the user hears
yesterday's mix. So: the key covers the snapshot by construction (4.2); `DSP_VERSION` is
bumped whenever any kernel's output changes (§5.3), which invalidates every page at once;
and an **audit** re-renders a random cached page per minute of background time and compares
bits — any difference disables the cache for that node kind and is reported as a bug. The
cache ships **off by default** until the audit has run clean through a round of real use.

### 5. Silence, denormals, SIMD and memory

**5.1 At rest: suspension that is exact.** Today a strip with an insert runs every
segment of every block, silent or not. FL's *Smart disable* and REAPER's auto-bypass
suspend plug-ins by *detecting* silence, and FL's manual warns that it breaks time- and
envelope-based plug-ins — because detection guesses. Cantus asks the unit instead. The
contracts gain one method each, default `false` (so nothing changes until a unit opts in):

```rust
/// True when the unit is **at rest**: its state is bit-identical to a freshly reset and
/// configured unit's, so that with silent input (and silent key, and no note) `process`
/// would output exact zeros and leave the state unchanged. Skipping such calls, and
/// resetting the unit before the next non-silent one, changes no sample.
fn at_rest(&self) -> bool { false }        // FxUnit; InstrumentUnit likewise
```

A node whose units are all at rest, whose gathered input is all-zero this pass (§1.3
already knows: every contribution was skipped) and whose delay lines are empty is
**suspended** — not called at all, its output marked silent. A strip whose inputs are all
suspended is suspended in turn, so a whole silent bus subtree costs a few comparisons. It
wakes on the next non-zero input, note or parameter change, by `reset` + `configure`,
which by the definition above is the state it would have had. A unit whose state moves
without input — a free-running LFO, a smoother still gliding, a meter's integration
window — is not at rest until it truly is; an LFO that follows song position (a function
of the sample clock, not an accumulator) can be at rest. The *same* predicate marks the
render cache's settle points (§4.2) and the time-parallel export's split points (§7.3):
three features from one honest method.

Hosted plug-ins: CLAP's process status says it for them — `CLAP_PROCESS_SLEEP` ("no more
processing is required until the next event or variation in audio input"),
`CONTINUE_IF_NOT_QUIET`, `TAIL` with the `tail` extension — and `constant_mask` lets the
host pass a silent buffer without filling it; VST3 has `getTailSamples` and per-channel
`silenceFlags`. A hosted node is suspended when the plug-in says so; its bits are then the
plug-in's own business, as the standards intend.

*What it buys, honestly:* Ableton's manual says it of its own device deactivation — it
lowers the **average** load, not the **peak**. A silent section is cheap; the busiest bar is
not made cheaper. Suspension pays in the ahead path, the export and the battery; the peak
is the scheduler's job (§2) and anticipation's (§3).

**Exactness test** (`fx_contract.rs`, `inst_contract.rs`, every kind, defaults and the
bench's *heavy* settings): feed signal, then silence until `at_rest()`; a twin that is
reset there and a twin that keeps processing zeros must produce the same bits when signal
returns at a random sample, at random block sizes. A unit that fails keeps the default.

**5.2 FTZ/DAZ.** `Processor::process` sets the flags at entry and restores them at exit
through `cantus_rt::DenormalGuard` (the guard from `cantus-plugins/src/ftz.rs`, moved), and
every pool thread sets them when it starts and re-asserts them before each node — a
hosted plug-in may change MXCSR and not put it back, and `ldmxcsr` is a handful of cycles.
The point of doing it inside `process` rather than in the callback is **determinism**: the
callback, the offline render, the stems, the bench and every test then run in the same
floating-point mode, so the null tests keep comparing like with like. The DSP already
flushes its own state in software (round 17 measured FTZ changing no sample of Sibilus
while making it 6 % faster); Phase 0 proves it for the whole corpus — every null test and
the `v5.cantus` hash with the flags on. A hash that moved would mean a kernel relied on
subnormal arithmetic, and its change would be a reviewed `DSP_VERSION` bump, not an
accident.

**5.3 SIMD: whole-program first, hand kernels where the profile says.**

- **The build targets x86-64-v3** (AVX2, FMA, BMI2 — every Intel since Haswell, every AMD
  since Excavator) with an **x86-64-v2 fallback binary** chosen at start-up by a stub that
  checks `is_x86_feature_detected!`; ARM64 has NEON as its baseline. STATUS measured the v3
  build of the exported plug-ins 15–25 % faster on the heavy effects, for nothing. It is
  also bit-identical: Rust never contracts `a * b + c` into an FMA by itself and LLVM does
  not reorder float reductions without fast-math, so the same source gives the same bits
  at v2 and v3 — which a CI job proves by comparing the null-test hashes of both builds.
- **Hand kernels** with portable SIMD (`wide`, safe, compiled for the binary's target), in
  the order the bench's hot-spot list gives — likely the oversampling FIR halfbands
  (Comprimo, Fervor at 4×), the linear-phase EQ's FFT convolution (`realfft` / `rustfft`,
  which already dispatch AVX at run time inside their own audited `unsafe`), biquad
  cascades across the two channels, and the synths' oscillators and filters.
- **Synth voices in structure-of-arrays**: a voice bank of `[f32; 8]` fields processed
  eight voices to an AVX2 register (four to NEON), each voice keeping its lane for its life.
  Bit-identical by construction: each lane does exactly the scalar voice's operations,
  and the voices are summed **in voice order**, lane by lane, as the scalar loop does
  (a horizontal SIMD add would reorder the sum and change bits). Transcendentals use one
  polynomial for scalar and SIMD. Where a kernel *cannot* keep the bits (a new polynomial,
  a fused multiply-add on purpose), it is a reviewed `DSP_VERSION` bump: goldens updated in
  the same commit, render-cache pages invalidated (§4.7), the change written in STATUS.
- **AVX-512** only as a third binary (x86-64-v4) if the bench shows it pays: Intel's client
  CPUs (this 12th-gen laptop included) do not have it; Zen 4/5 do.

**5.4 Memory.**

- **Nothing allocates on the callback or on any pool thread** — ADR 0005 extended to every
  thread that renders. `tests/rt_memory.rs`'s per-thread counting allocator counts every
  thread the pool marks real-time.
- **One slab per plan** (§1.4) instead of hundreds of `Vec`s: node buffers 64-byte aligned
  (a cache line, an AVX-512 register) and padded to whole lines, so two nodes written by
  two workers never share a line.
- **Per-node hot scalars** — pending count, cursor, flags, peaks — each in its own
  cache-line-padded cell (128 bytes on Apple silicon, whose lines are 128), separate from
  the cold configuration the plan holds read-only.
- **Per-worker scratch** (`dry`, `edge`, `mono`, the analyser's input) allocated by the
  worker on its own thread at pool start, so it lives in that core's cache.
- Unit state stays where ADR 0005 put it: built on the UI thread, delivered in a `Box`,
  returned as garbage.

### 6. Plug-in sandboxing at scale (designed now, built after the scheduler)

ADR 0019 isolates the *scan*; a plug-in that crashes in `process` still takes Cantus
down. The model is Bitwig's: a hosting mode per plug-in — **Within Cantus**, **Together**
(one sandbox for every third-party plug-in), **By vendor**, **By plug-in**, **Individually**
— with **Together** the default for third-party plug-ins (one crash cannot reach the engine,
and one process boundary per wave of the graph, not per plug-in) and our own units always
within. REAPER's "separate" and "dedicated" process modes are the same idea in two
settings; FL's bridge costs it ~1–2 % CPU per plug-in.

- **The sandbox** is a `cantus-sandbox` executable running `cantus-host` as today (the
  same `AudioThread`/`MainThread` halves, its own main-thread pump for windows and state),
  started per group, MMCSS "Pro Audio" on its processing threads.
- **Shared memory** (one mapping per group, from `cantus-rt`): per instance, its input,
  key and output buffers, its event lists (notes, parameters, transport) and a
  **job/completion** pair of sequence numbers; per group, a job ring. All fixed-size,
  built when the instance is loaded.
- **No host thread waits on another process.** A sandboxed node is an *external task*: the
  worker that would run it writes its inputs and events, bumps the job number, and goes on
  with other ready nodes; the scheduler loop polls the completion numbers it is waiting
  for (a load per in-flight job) and makes the successors ready when one lands. The
  sandbox's threads spin on the job ring while the transport plays and park on an event
  when idle — the same spin-then-park policy as §2.4, and Windows' `WaitOnAddress` does not
  cross processes, so the park is a named event. A round trip costs two context switches
  when the sandbox has parked, a cache-line handoff when it spins; Phase 6 measures both
  before it is built, against a 64-sample budget.
- **Ahead nodes hide it entirely**: a sandboxed plug-in that is not live (§3.2) runs in
  the ahead path, where a few tens of microseconds of round trip are nothing.
- **A crash or a hang** (a job not completed by the block's deadline): the node's output
  is its dry input for that block, counted (`EngineStats::sandbox_late`), never waited
  for; a dead process turns every instance in it into a wire and the device into a
  *Reload / Reload all* notice, as Bitwig does. The engine never stalls.
- **Per-plug-in CPU**: the sandbox writes each instance's nanoseconds per job beside its
  completion number, so the profiler's blame (§2.9) names a sandboxed plug-in as it names
  a native one.
- **Latency changes without a dropout**: the restart a latency change needs (ADR 0019)
  happens in the sandbox beside the running instance; the new one is swapped in at a
  structural switch point (§3.5) with compensation re-planned there.

### 7. Export in parallel, at many times real time

The export (`render.rs`, stems) drives the *same* processor and keeps every bit; it gets the
same graph and executor with the brakes off:

1. **All cores, no deadline**: every P- and E-core at normal priority, blocks of
   `MAX_BLOCK_FRAMES` (8 192) — longer calls amortise every unit's per-call overhead, and
   block-size independence keeps the bits. Hosted plug-ins in their offline mode (CLAP
   `render` OFFLINE, VST3 `kOffline`), which ADR 0019 left unused.
2. **Pipelined across time**: every edge carries a ring of a few blocks, so a node can
   render block *k + 1* while its consumer renders block *k*. A chain of *L* nodes then
   keeps *L* cores busy. In export and in the ahead path (never live, where it would add
   latency) **each insert is its own node**, so one strip with five heavy plug-ins — the
   case Ableton says cannot be spread across cores — is a five-stage pipeline. The master
   chain of a mastering session is exactly that case.
3. **Parallel across time**, at settle points (§5.1): a node that is at rest at bar 33 can
   render bars 1–32 and 33–64 on two cores with two instances (a native unit is rebuilt off
   the audio thread; a hosted one loaded with the same state), because at a settle point
   the state *is* a fresh unit's. Bit-identical by the same argument as the cache, and the
   way past the critical path on a song with pauses. Built last of the export work, behind
   the at-rest tests.
4. **The render cache** (§4): pages already valid are copied; a re-export after a mix tweak
   recomputes only what the tweak touched.

What "many times real time" means is arithmetic, not a promise. A session the serial
engine plays at load *ℓ* (fraction of one core at real time) exports serially at `1/ℓ` ×
real time; with *c* effective cores and pipelining, at most `c/ℓ`. A session at 50 % of a
core exports at ≈ 2× serial today and ≈ 20× on ten effective cores. §4b's 50–200× is
reachable for lighter sessions and with the cache, and the bench's `render.*` metrics
are where it is measured, per session, against the other DAWs' export times.

### 8. Risks, and the order of work

The order is in `docs/perf/plan.md` with each phase's builders, files and tests; here is
why it is that order.

| # | Risk | What contains it |
|---|---|---|
| 1 | Per-node segmentation moves bits: a unit that is split-invariant at its defaults and not at some setting | Phase 0 runs first, serial, alone; before it, `fx_contract`/`inst_contract` are extended to the bench's heavy settings and random split points; the `v5.cantus` hash and every null test gate it |
| 2 | A preempted worker makes the parallel engine drop out where the serial one would not | MMCSS / workgroups / FIFO; a core left free (§2.1); the serial fallback (§2.5); blame that says *preempted* (§2.9); anticipation (§3) takes most nodes off the deadline |
| 3 | Hosted plug-ins that assume one thread (thread-local caches, a VST3 that dislikes a new thread each block) | a per-plug-in *home worker* (its node only ever runs on one worker; costs scheduling freedom, keeps the plug-in happy), a compatibility list in the plug-in database |
| 4 | Anticipation's switch points and re-anchoring are where subtle bugs live | the edit-at-`S` oracle (a serial engine given the same edit at `S` must match to the bit), a seeded scheduler that randomises interleavings and chunk sizes in tests, the stop crossfade bounded by a test |
| 5 | A stale render-cache page — the user hears an old mix | the key's exhaustive destructuring, `DSP_VERSION`, the bit audit, off by default for a round (§4.7) |
| 6 | The C runtime's `sin`/`exp` may pick code paths by CPU at run time: renders could differ *between machines* (true today, unrelated to threads) | the null tests compare within a machine; the render cache key includes the CPU's feature class; DSP that the corpus covers uses its own approximations |
| 7 | `unsafe` in `cantus-rt` is unsound | small crate, `SAFETY` comments, Miri and `loom` in CI, nothing audio-specific in it |
| 8 | Spinning burns battery and heat on a laptop | park after 100 ms of silence (§2.4); an *Eco* setting that parks between blocks and accepts the 55 µs wake |
| 9 | Builders collide in `processor.rs` | Phase 0 splits it into node files with one owner each (`plan.md`) before anyone else touches it |
| 10 | There are no baseline numbers yet | every target below is relative to `cantus-bench`'s own baseline on the same machine class, so it is checkable the day `baseline.md` lands |

**Order:** (0) the graph in the serial engine — per-node segments, canonical gather,
outboxes, FTZ in `process`, the `cantus-rt` crate, the at-rest contracts, limits to 256;
(1) the live pool; (2) the parallel export and suspension; (3) SIMD and the v3 build, in
parallel with 1–2; (4) anticipation; (5) the render cache; (6) sandboxing. Each one is
useful alone and measurable by the bench, and each later one stands on the earlier ones
exactly where the dependency is real: anticipation needs the node cursors and the pool;
the cache is an ahead renderer; the sandbox is an external task of the pool.

**Targets** (`cantus-bench` metric names; "×" is against that machine class's baseline):

| Phase | Target |
|---|---|
| 0 | every null test, the `v5.cantus` hash, `rt_memory` unchanged; `engine.s64.b128` median ≤ 0.9× (fewer calls per strip); `engine.s100` plays all 100 tracks |
| 1 | `engine.s64.b128` median ≤ 0.45× and p99 ≤ 0.6× on 6 P-cores (the master chain stays serial: Amdahl's share); `engine.s100.b128` p99 under 80 % of budget; `dawbench.dsp.b64` ≥ 3×; zero `schedule_conflicts`; parallel = serial on the whole corpus under 1 000 seeded schedules |
| 2 | `render.*` ≥ 4× its baseline on 14 cores; a silent half of the scaled session costs ≤ 10 % of its playing half |
| 3 | heavy kernels ≥ 1.3× (`fx.*.heavy`), `inst.*.v32` ≥ 2× where SoA lands, hashes equal at v2 and v3 |
| 4 | `dawbench.dsp.b64` ≥ 1.5× Phase 1 with nothing live (larger calls, E-cores working); a live armed track at 64 samples with the rest ahead: zero dropouts over the bench's run; ahead = live = offline to the bit |
| 5 | an unchanged re-export ≥ 50× real time; an edit invalidates only its pages to the next settle point (a test counts them); audit clean |
| 6 | a crashing plug-in in *Together* leaves audio running; round trip measured ≤ 5 % of a 64-sample budget per group, or the default stays *Within Cantus* |

## What the tests hold (summary; per phase in `plan.md`)

- **Bits:** every existing null test, block-size test and the `v5.cantus` SHA-256, run with
  `W = 0` and with `W ∈ {1, 3, 7}`; a *schedule fuzz* that randomises the ready queue's
  order, the continuation slot and chunk sizes (seeded, 1 000 seeds in CI) and asserts
  parallel = serial; ahead = live = offline at each anticipation level; cached = computed.
- **Real time:** `rt_memory` on the callback and every pool thread; `schedule_conflicts`
  zero; `EngineStats::wakes` only at idle → busy transitions.
- **Semantics:** the edit-at-`S` oracle; the stop crossfade's bound; the at-rest twin test
  for every unit; the cache's invalidation counts; a crashing test plug-in in a sandbox.
- **Speed:** `xtask perf` gates the targets above per machine class.

## Consequences

- New crates: `cantus-rt` (the only `unsafe`: OS threads, denormals, rings, shared
  memory); later `cantus-sandbox` (a binary). New engine modules: `graph/` (plan, nodes),
  `sched/` (pool, queues), `ahead/`, `cache/`. `processor.rs` becomes the control phase
  and the glue.
- `EngineMessage::SetSnapshot` carries an optional plan; new `SetPromoted`, `GraphNeed`
  (event), `EngineStats::{schedule_conflicts, wakes, sandbox_late, ahead_lead, ahead_late}`.
- Contracts: `FxUnit::at_rest`, `InstrumentUnit::at_rest` (default `false`). The
  `fx_notes` queue becomes outboxes; the contract of `FxUnit::note` is unchanged.
- ADR 0005 rule 3 is amended (§2.4). ADR 0019's "whole-block processing for hosted units"
  and CLAP `render`/`tail`/`thread-pool` are taken up; ADR 0018's pool of lines stays.
- `MAX_TRACKS`/`MAX_STRIPS` 256. Settings: worker count, anticipation level, render-cache
  budget and switch, per-plug-in *never ahead*, *home worker*, hosting mode.
- What the user sees: the engine readout gains *ahead lead* (how far ahead the anticipative
  path is) beside the load; the dropout notice names the node, or says *preempted*; the
  plug-in manager says which plug-ins are not cacheable and why.
