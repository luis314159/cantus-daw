# Scheduling: the open-source engines' code and the papers behind them

*Round 20, engine-research, 2026-10-06. For ADR 0031 §2 (the scheduler), §3 (anticipation)
and §6 (the sandbox). Source code was read from the projects' repositories (master/develop
as of today); papers were downloaded and read. **[fetched]** = read; **[excerpt]** = search
excerpt only; **not researched** = not reached in this round.*

## 1. Ardour — `libs/ardour/graph.cc` (the reference the ADR could not fetch)

Read: `libs/ardour/graph.cc`, `ardour/graph.h`, `graphnode.cc`, `libs/pbd/pbd/mpmc_queue.h`,
`session.cc`, `utils.cc`, `libs/backends/coreaudio/*` (github.com/Ardour/ardour, master
**[fetched]**; copyright Carl Hetherington, Paul Davis, John Emmas, Tim Mayberry, Robin Gareus).

- **Ready queue: the same algorithm as Cantus's prototype.** `PBD::MPMCQueue<ProcessNode*>
  _trigger_queue` — "Inspired by http://www.1024cores.net/…/bounded-mpmc-queue — Kudos to
  Dmitry Vyukov"; one queue, no priority classes; reserved to 1 024, grown in `prep()` off the
  hot path.
- **Workers sleep on semaphores, never spin.** After a pop a thread wakes at most
  `min(idle_cnt + 1, work_avail) − 1` others ("Wake up idle threads, but at most as many as
  there's work in the trigger queue"); with no work it blocks on `_execution_sem.wait()`.
- **The callback does not help.** The backend's callback signals `_callback_start_sem` and
  waits on `_callback_done_sem` — a semaphore round trip every cycle; a dedicated graph "main
  thread … just becomes a normal worker". The thread that runs the last terminal node
  signals completion, then yields until all workers are idle.
- **Activation**: refcounts set in `prep()`, `atomic_dec_and_test` in `trigger()`, every
  ready successor goes through the queue — **no continuation**.
- **Topology change**: a new `GraphChain` behind a `shared_ptr` whose deleter hands deletion
  to the butler thread ("It was never rt-safe to re-chain the graph … the last reference
  [may] be held by the process-callback"); per-node activation sets are RCU maps keyed by
  chain, so two chains coexist during the swap.
- **Threads**: `processor-usage` setting, default `max(num_cpu − 1, 2)`.
- **macOS audio workgroups: yes.** The CoreAudio backend reads
  `kAudioDevicePropertyIOThreadOSWorkgroup`; every process thread (graph workers included)
  sets a Mach real-time policy with the device period, then `os_workgroup_join`, handling
  `EALREADY`/`EINVAL`, and leaves on exit.

## 2. Tracktion Graph (Dave Rowland)

Read: `tracktion_LockFreeMultiThreadedNodePlayer.{h,cpp}`, `tracktion_NodePlayerThreadPools.{h,cpp}`,
`tracktion_Node.h`, `nodes/tracktion_LatencyNode.h`, `nodes/tracktion_SummingNode.h`,
`tracktion_EditPlaybackContext.cpp` (github.com/Tracktion/tracktion_engine, develop
**[fetched]**).

- **Six wake strategies**: `conditionVariable`; `realTime` ("uses pause, yield and sleeps …
  so the main audio thread never interacts with the thread scheduler" — 50 pauses, 100
  yields, 1 ms sleeps, 5 ms sleeps); `hybrid` (25 pauses, 25 yields, a CV); `semaphore`;
  `lightweightSemaphore` (moodycamel, spins up to 10 000 before a kernel wait);
  `lightweightSemHybrid` — **Tracktion Engine's default**. Threads default to
  `hardware_concurrency − 1`, or `max_parallel_threads − 1` with workgroups (supported, **off
  by default**).
- **Ready queue**: `rigtorp::MPMCQueue<Node*>` (a ticket-based bounded MPMC, the same
  progress class as Vyukov's) sized to the node count; the audio thread resets counters, then
  enqueues ("only incremented after all the nodes have been queued"), then signals; **the
  audio thread helps** and `pause()`s waiting for the root.
- **Continuation** (`RETURN_MID_NODES_OPTIMISATION`, on): the first successor that becomes
  ready runs on the same thread — "Attempt to process serial Node chains on this thread to
  reduce context switches and overhead". Cantus's continuation slot.
- **Serial fallback** only at 0 threads or a single node — no load threshold.
- **Buffers**: each node owns its buffer; a pooled mode exists as "a temporary method used for
  benchmarking". Rowland, *Optimising a real-time audio processing library*, ADC 2022 (slides,
  github.com/drowaudio/presentations **[fetched]**): "The overhead of using less memory
  outweighed the cache benefits; complexity in run-time assignment of memory".
- **PDC** by inserting `LatencyNode`s at summing points, to a fixpoint; node UIDs let a new
  graph inherit an old node's resources.
- **Swap**: built on the message thread, published lock-free; a retired graph is destroyed
  only after workers quiesce.
- Talk: D. Rowland, "Introducing Tracktion Graph: a topological processing library for
  audio", ADC 2020 (slides https://data.audio.dev/talks/2020/introducing-tracktion-graph/slides.pdf
  **[fetched]**): "Fully real-time implementation means no system calls … requires worker
  threads spinning on the FIFO … or brief yields/sleeps — very difficult to get them to wake
  up at the right time"; on the forum: "the difficult thing … is to ensure the CPU use is
  actually lower whilst still keeping jitter consistent" (JUCE forum 43052 **[fetched]**).
  Rowland, "Lock-free queues in the multiverse of madness", ADC 2025 (slides **[fetched,
  skimmed]**): SPSC/SPMC/MPMC surveyed; "cooperative consuming — producer thread consumes".

## 3. JACK2 (Letz, Fober, Orlarey)

S. Letz, D. Fober, Y. Orlarey, "Jackdmp: Jack server for multi-processor machines", LAC 2005,
pp. 29–36 (https://archive2010.grame.fr/ressources/publications/Jackdmp-lac2005.pdf
**[fetched]**); S. Letz, Y. Orlarey, D. Fober, "Jack audio server for multi-processor
machines", ICMC 2005 (…/Jackdmp-ICMC2005.pdf **[fetched]**); precursor with P. Davis, SMC
2004 (https://zenodo.org/records/849189 **[fetched]**).

- **Data-flow activation**: "Each client uses an activation counter to count the number of
  input client which it depends on … The last activated input client will resume the
  following client in the graph." `JackActivationCount { fValue; fCount; }` lives **in shared
  memory**; `Signal()` posts the client's semaphore when the decrement reaches zero.
- **Lock-free graph change, two states**: "A state change consists in atomically switching
  from the current state to the next state. This is done by the RT audio server thread at the
  beginning of a cycle" (`JackAtomicState`).
- **Synchronous vs asynchronous**: "The server no longer waits for the client execution end.
  It uses the buffers computed at the previous cycle … this new model adds a one buffer more
  latency". In the source, `ProcessAsync` writes the previous cycle's output; **async is the
  default** (`"sync"` = false in `JackControlAPI.cpp`).
- Measured: "a worst case additional latency of 150 to 200 microseconds" for the parallel
  activation on a dual G5.

## 4. PipeWire (Wim Taymans)

*Graph scheduling* (https://docs.pipewire.org/page_scheduling.html, 1.6.x **[fetched]**):
each node has "an eventfd to signal the node that it can start processing" and "an
activation record that lives in shared memory with memfd" (`status`, `pending`, `required`);
the driver resets `pending` from `required`, decrements targets, and at zero "the eventfd is
signaled" — and "writing to the remote client eventfd will wake the client directly without
going to the server first". The driver first checks the previous cycle and marks unfinished
nodes as xruns. **Async nodes** (PipeWire 1.2.0, 2024-06-27, NEWS **[fetched]**): "it will use
the output of the previous cycle (or silence) instead. This adds one cycle of latency";
double-buffered (`(cycle+1)&1` written, `cycle&1` read), the added latency reported through
`SPA_PARAM_Latency`. Real-time via `RLIMIT_RTPRIO`, else the portal or rtkit (`module-rt`
**[fetched]**).

## 5. SuperCollider's Supernova (Tim Blechmann)

T. Blechmann, "supernova, a multiprocessor-aware synthesis server for SuperCollider", LAC
2010 (https://tim.klingt.org/publications/lac2010_supernova.pdf **[fetched]**); "Supernova —
a scalable parallel audio synthesis server for SuperCollider", ICMC 2011
(…/icmc2011_supernova.pdf **[fetched]**); diploma thesis, TU Wien 2011
(https://repositum.tuwien.at/handle/20.500.12708/9407, abstract **[fetched]**, full text
bot-blocked).

- Activation counts "similar to the implementation of Jack2"; "sequential synths are combined
  into a single queue node to avoid the overhead of scheduling each synth" — clustering.
- "It is not feasible to use blocking primitives for synchronization of the main audio
  thread and the audio helper threads. Instead Supernova wakes all helper threads in the
  beginning of the main audio callback and all threads poll a lock-free stack … This greedy use
  of CPU resources is not friendly to other processes … Unless one uses highly tuned systems
  running the RT preemption patches … it seems to be the only way to dispatch threads quickly
  enough."
- Worst-case dispatch latency 100 µs on a Core2 laptop, 12 µs on an i7 with power
  management, SMT and frequency scaling off. "The speedup of the worst case is more
  interesting than the speedup of the average case."
- Source (`server/supernova/dsp_thread_queue/` **[fetched]**): a bounded
  `boost::lockfree::stack` (LIFO); after running, the **first successor that reaches zero runs
  next on the same thread** (continuation); helpers woken each tick by a semaphore post, then
  poll with exponential `pause` backoff (8 → 16 384); **the main audio thread participates**
  and busy-waits for the end; helpers have **mlocked, pre-touched stacks** and are pinned; a
  TLSF-based pool allocator, mlocked.

## 6. Faust (Orlarey, Fober, Letz)

Y. Orlarey, D. Fober, S. Letz, "Adding automatic parallelization to Faust", LAC 2009
(https://archive2010.grame.fr/ressources/publications/faustLAC09.pdf **[fetched]**): OpenMP over
topological levels with a barrier per level — "it is interesting to note how catastrophic are
the performances of the parallel versions" on some machines; memory bandwidth the limit.
S. Letz, Y. Orlarey, D. Fober, "Work stealing scheduler for automatic parallelization in
Faust", LAC 2010 (…/FAUST_LAC2010.pdf **[fetched]**; hal-02158924):

- **One work-stealing queue per thread**, private LIFO pop and public FIFO steal — "the local
  LIFO Pop operation allows better cache locality"; activation counters for joins.
- **Continuation**: with a direct link "the tasknum of the next task is the tasknum of the
  output task"; others are pushed.
- The calling thread participates; *n − 1* threads resumed each cycle.
- Results: Sonik Cube on 8 threads ×5.29 with work stealing against ×4.17 with OpenMP;
  "simple applications … tend to perform usually better in scalar mode"; open questions: which
  successor to continue with, thread affinity, and "performance can severally degrade as soon
  as too much threads are competing for the limited number of physical cores".
- Today's source (`architecture/scheduler.cpp` **[fetched]**): each thread's work-stealing
  queue is a **fixed-size `int` array** with head and tail packed in one CAS word — **no
  growth, no garbage collection**; stealers yield after 50 µs.

## 7. Others, briefly

Csound: J. ffitch, "Parallel execution of Csound", ICMC 2009; "Running Csound in parallel",
LAC 2011 (abstracts **[fetched]**; full texts not researched). Pd and FTS: subpatches
assigned to CPUs at the cost of a signal vector of delay; `pd~` runs a subpatch in another
process (via Blechmann 2011 **[fetched]**). Max `poly~ @parallel` (docs.cycling74.com
**[fetched]**). Radium (K. Matheussen, `audio/MultiCore.cpp` **[fetched]**): the main audio
thread is a runner and gets newly-ready work first when it is waiting; a continuation. A
DAFx 2026 paper — S. Balasubramaniam, R. Ramachandran, J. Timoney, "Real-time neural audio
on Apple silicon: benchmarking inference frameworks under realistic DAW contention"
(https://dafx.de/paper-archive/2026/papers/DAFx26_paper_16.pdf **[fetched]**) — shows an
isolated real-time factor of 0.211 becoming a p99 callback utilisation of 158.5 % and 355
xruns inside a Tracktion Engine pool: **isolated averages lie; measure p99 under
contention**. T. Cucinotta, D. Faggioli, G. Bagnoli, "Low-latency audio on Linux by means of
real-time scheduling", LAC 2011 (https://lac.linuxaudio.org/2011/papers/46.pdf **[fetched]**):
JACK inside a CBS reservation sized by a percentile estimator. GPU Audio's scheduling patent
(US 12026518) and ADC talks: **[excerpt]**, not verified.

## 8. Standards

**CLAP** (github.com/free-audio/clap, `include/clap/` **[fetched]**): `thread-pool.h` — "using a
thread pool may break hard real-time rules … If the host knows that it is running under hard
real-time pressure it may decide to not provide this interface"; `request_exec` "will block
until all the tasks are processed … The host should check that the plugin is within the
process call". `thread-check.h` — "The audio-thread is symbolic … the host must guarantee
that single plugin instance will not be two audio-threads at the same time". `render.h` —
`has_hard_realtime_requirement` "especially useful for plugin acting as a proxy to an
hardware device". `process.h` — `CLAP_PROCESS_SLEEP`: "no more processing is required, until
the next event or variation in audio input"; `steady_time` advances by at least the frame
count. `audio-buffer.h` — `constant_mask` "is a hint … the buffer must be filled with the
constant value". **VST3** (`ivstaudioprocessor.h` **[fetched]**): `kPrefetch` "should process
at the same quality level than realtime, plug-in must not slow down to realtime (e.g. disk
streaming)! The host should avoid to process in kPrefetch mode such sampler based plug-in";
"switching between kRealtime and kPrefetch process modes are done in realtime thread without
need of calling IAudioProcessor::setupProcessing".

## 9. Scheduling theory

- H. Topcuoglu, S. Hariri, M.-Y. Wu, "Performance-effective and low-complexity task
  scheduling for heterogeneous computing" (HEFT), *IEEE TPDS* 13(3), 2002,
  doi:10.1109/71.993206 (abstract **[fetched]**): select by highest **upward rank**, assign by
  earliest finish time on heterogeneous processors. ADR 0031 §2.3's rank *is* HEFT's upward
  rank; HEFT's assignment half is the precedent for P/E-aware placement.
- R. L. Graham, "Bounds for certain multiprocessing anomalies", *Bell System Technical
  Journal* 45(9), 1966 (**[fetched, OCR]**): "an increase in the number of processing units
  can cause an increase in the total length of time" — list-scheduling anomalies; the
  2 − 1/m bound (*SIAM J. Appl. Math.* 17(2), 1969 **[excerpt]**).
- R. D. Blumofe, C. E. Leiserson, "Scheduling multithreaded computations by work stealing",
  *JACM* 46(5), 1999 **[excerpt]**: T₁/P + O(T∞).
- U. A. Acar, G. E. Blelloch, R. D. Blumofe, "The data locality of work stealing", SPAA 2000
  (https://www.cs.cmu.edu/~guyb/papers/locality2000.pdf **[fetched]**): in *iterative*
  data-parallel programs — an audio block loop is one — "random steals may cause poor data
  locality"; **locality-guided stealing**: each task has an affinity for the processor that
  ran it on the previous step, delivered through a per-processor mailbox checked before
  stealing; matches static partitioning when the machine is quiet and beats it "under
  multiprogramming workloads".
- E. A. Lee, D. G. Messerschmitt, "Static scheduling of synchronous data flow programs for
  digital signal processing", *IEEE Trans. Computers* C-36(1), 1987
  (https://ptolemy.berkeley.edu/publications/papers/87/staticscheduling/ **[fetched]**): "the
  scheduling of SDF nodes need not be done at runtime, but can be done at compile time
  (statically), so the runtime overhead evaporates."
- D. R. Jefferson, "Virtual time", *ACM TOPLAS* 7(3), 1985, pp. 404–425 **[excerpt]**:
  optimistic execution with **rollback** (Time Warp) — a process runs ahead of its inputs and,
  when a message arrives in its past, rolls back to a checkpoint and re-executes.
- A. Mokhov, N. Mitchell, S. Peyton Jones, "Build systems à la carte", ICFP 2018; extended
  "… theory and practice", *JFP* 30, 2020
  (https://www.microsoft.com/en-us/research/uploads/prod/2018/03/build-systems-a-la-carte.pdf
  **[excerpt]**): static vs dynamic dependencies, **constructive traces** (keys that
  determine the stored result, the basis of shared caches) and **early cutoff** (stop
  rebuilding downstream when a rebuilt value equals the old one).

## 10. What this says about ADR 0031

| ADR decision | What the engines and papers say |
|---|---|
| Static graph compiled off the audio thread, swapped by message (§1.2) | Universal: Ardour (chain + butler deletion), Tracktion (prepared node, quiesce), JACK2 (two states), JUCE. **Supported.** |
| Canonical gather, parallel = serial to the bit (§1.3) | No surveyed engine claims bit-identity; Supernova adds implicit dependencies to keep sequential *semantics*. **Cantus is ahead**; keep. |
| Every output owns its buffer; reuse only for large blocks (§1.4) | Rowland measured reuse losing to per-node buffers on desktop. **Supported; and doubt reuse for the ahead path too until measured.** |
| Callback helps (§2.2) | Tracktion, Supernova, Faust, Radium help; Ardour does not (semaphore round trip per cycle). **Supported by the majority and by the ADR's own measurement.** |
| Bounded Vyukov MPMC (§2.3) | Ardour uses exactly this; Tracktion a ticket MPMC; Supernova a lock-free stack; **Faust a fixed-size per-thread work-stealing array**. All are bounded and allocation-free, but the MPMC ones are not lock-free (`realtime-systems.md` §2). **Better exists: Faust's fixed per-worker deques + Acar's affinity.** |
| Critical-path priority queues (§2.3) | No audio engine does it; Faust lists "which successor to run" as open; HEFT is the theory. **Novel and justified; measure that it pays.** |
| Continuation slot (§2.3) | Tracktion, Supernova, Faust, Radium. **Supported.** |
| Spin, park after silence, wake only at idle → busy (§2.4) | Supernova spins every block (and says it is unfriendly); Tracktion's default spins then waits on a lightweight semaphore; Ardour always waits. Cantus's "spin while playing, park after silence" sits between them and is the only one that never wakes on a busy block. **Supported.** |
| Clustering ≥ 20 µs (§2.5) | Supernova merges sequential synths; Faust found simple graphs faster scalar. **Supported.** |
| Load-based serial fallback (§2.5) | No precedent; Faust's dynamic thread adapter is the nearest. **Novel; keep behind measurement.** |
| Anticipation (§3) | JACK2's async mode and PipeWire's async nodes are one-cycle versions of the same trade (latency for robustness); REAPER and ASIO-Guard the many-block versions. **Supported**; Time Warp offers rollback for edits. |
| Sandbox: shared-memory job/completion (§6) | JACK2 and PipeWire are exactly this, with activation counters in shared memory and eventfd/semaphore wakes — PipeWire wakes the next client directly. And both offer an **async** mode with a cycle of latency. **Supported, with a better option for live sandboxed nodes.** |
| Render cache keys (§4.2) | Build systems' constructive traces and early cutoff; the settle point is early cutoff in time. **Supported by theory.** |
