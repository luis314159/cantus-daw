# Testing and verifying a real-time engine

*Round 20, engine-research, 2026-10-06. For ADR 0031's review ("What the tests hold") and
the bench method (`docs/perf/benchmark.md` on `r20/bench`). Sources are **[fetched]** (read)
or **[excerpt]** (search excerpt only; some PDFs downloaded but could not be parsed). The
proxy returned 403 for the Linux Foundation cyclictest wiki and a vi-control thread, 500
for timur.audio; nothing was bypassed.*

## 1. Systematic concurrency testing: what each tool can and cannot prove

| Tool | What it does | Limits that matter for Cantus |
|---|---|---|
| **loom** (tokio-rs; https://docs.rs/loom, https://github.com/tokio-rs/loom **[fetched]**) | Explores all interleavings of a small model under the C11 memory model, pruning equivalent executions (CDSChecker's technique); `LOOM_MAX_PREEMPTIONS` bounds forced switches — "2 or 3 is enough to catch most bugs" | Treats `SeqCst` as `AcqRel` (false alarms), does not explore load buffering (can miss real bugs), sees only loom's own types, thread count capped. A pass is not a proof for `Relaxed` paths |
| **CDSChecker** (B. Norris, B. Demsky, "CDSChecker: checking concurrent data structures written with C/C++ atomics", OOPSLA 2013; https://plrg.ics.uci.edu/publications/470.bib **[excerpt]**) | loom's basis: stateless model checking of C/C++11 atomics | — |
| **CHESS** (M. Musuvathi, S. Qadeer, T. Ball, G. Basler, P. A. Nainar, I. Neamtiu, "Finding and reproducing Heisenbugs in concurrent programs", OSDI 2008; https://static.usenix.org/events/osdi08/tech/full_papers/musuvathi/musuvathi_html/index.html **[fetched]**) | Preemption bounding: *k* steps, *c* preemptions → ~k^c schedules; most bugs need 1–2 preemptions; every schedule logged and replayable; 27 bugs in PLINQ, Singularity… | Small thread counts (2–8) |
| **PCT** (S. Burckhardt, P. Kothari, M. Musuvathi, S. Nagarakatte, "A randomized scheduler with probabilistic guarantees of finding bugs", ASPLOS 2010; abstract https://www.microsoft.com/en-us/research/?p=158821 **[fetched]**, PDF **[excerpt]**) | Random thread priorities, *d − 1* random priority-change points; finds a bug of depth *d* with probability ≥ 1/(n·k^(d−1)) per run | Bound degrades with *k* (steps): keep episodes short |
| **Shuttle** (awslabs; https://github.com/awslabs/shuttle **[fetched]**; from J. Bornholt et al., "Using lightweight formal methods to validate a key-value storage node in Amazon S3", SOSP 2021, best paper **[excerpt]**) | Random, **PCT** (`check_pct(depth, iterations)`), DFS and replay schedulers; a failure prints a schedule string `replay()` reruns | "Not sound" by design: scale over exhaustiveness |
| **GenMC** (M. Kokologiannakis, V. Vafeiadis, "GenMC: a model checker for weak memory models", CAV 2021; https://plv.mpi-sws.org/genmc/ **[excerpt]**) and **Miri-GenMC** (`-Zmiri-genmc`, https://rust.googlesource.com/miri/+/HEAD/doc/genmc.md **[fetched]**) | Sound, complete, optimal exploration under RC11 — every execution the Rust model allows | Miri-GenMC is work in progress, Linux only, bounded programs |
| **Miri** (https://github.com/rust-lang/miri **[fetched]**) | UB and data-race detection; "some" weak-memory effects; `-Zmiri-many-seeds`, `-Zmiri-preemption-rate` | "Weak memory emulation is not complete"; tests one execution per seed; no FFI |

**Against ADR 0031.** The ADR's test plan — "a *schedule fuzz* that randomises the ready
queue's order, the continuation slot and chunk sizes (seeded, 1 000 seeds)" plus loom and
Miri on `cantus-rt`'s unsafe types — is the right shape, and comparing the **output hash**
with the serial render is a stronger oracle than CHESS's or Shuttle's (it catches
summation-order drift as well as races). Four improvements:

1. **PCT, not uniform randomness.** A uniform random choice at every pop makes any one
   specific bad ordering exponentially unlikely in the number of dispatches; PCT spends
   its randomness on *d − 1* change points and gives a stated bound. Per seed: random node
   priorities, two or three change points over the block's dispatches, and a worker that
   is "preempted" (made to stall) at a change point. 1 000 seeds then become a number ("a
   depth-2 bug in a 400-dispatch block is missed with probability ≤ …").
2. **Short episodes for the fuzz, long ones for the null corpus.** The PCT bound degrades
   as k^(d−1): fuzz small random graphs over a few blocks; run the whole corpus with fewer
   seeds as the end-to-end check.
3. **A replay token.** A failing seed prints a schedule string (Shuttle's idea, CHESS's
   logged schedule) that a `#[test]` reruns under a debugger.
4. **Know loom's blind spots.** Write the ring and the sandbox protocol with
   Acquire/Release only (no `SeqCst`, which loom weakens; no `Relaxed` data paths, which
   it cannot fully reorder); the shared-memory protocol crosses processes, which neither
   Miri nor loom sees, so model it in-process behind a trait with loom types and test the
   model. Miri-GenMC as an optional Linux job once it matures.

## 2. Deterministic simulation and replay

FoundationDB runs a whole cluster deterministically in one thread on a simulated clock,
with injected faults, reproducible from `--seed`
(https://apple.github.io/foundationdb/testing.html **[fetched]**; J. Zhou et al.,
"FoundationDB: a distributed unbundled transactional key value store", SIGMOD 2021,
https://www.foundationdb.org/files/fdb-paper.pdf **[excerpt]**). Two of its practices fit
Cantus exactly: **the "unseed" self-check** — run each seed twice and assert the same final
random state, which catches nondeterminism leaking in from outside the seed ("Debugging in
FDB", https://github.com/apple/foundationdb/wiki/Debugging-in-FDB **[fetched]**) — and
**BUGGIFY**, hostile code paths enabled at random only in simulation **[excerpt]**.
TigerBeetle's VOPR runs the real code with mocked I/O at ~1000× speed, 24/7 on 1 024 cores,
with static allocation only (https://docs.tigerbeetle.com/concepts/safety/ **[fetched]**).
rr records every nondeterministic input crossing a boundary and replays it (R. O'Callahan
et al., "Engineering record and replay for deployability", USENIX ATC 2017,
https://ar5iv.labs.arxiv.org/html/1705.05937 **[fetched]**); it is Linux/x86 only and cannot
see other processes' shared-memory writes.

**For Cantus.** The offline render *is* the simulator — Cantus already has TigerBeetle's
time acceleration for free, because the engine's only clock is the sample clock. What is
missing is the **recorded boundary**: a session recorder that logs, at the engine's edge,
every `EngineMessage` with the block index and frame it was applied at, every MIDI event,
the sequence of device block sizes, the input audio (or its hash plus a capture), the
initial snapshot, and for sandboxed or non-deterministic hosted plug-ins their outputs. A
bug report then replays offline, bit for bit, through the same engine. And a
`buggify!(site)` macro under a test feature: one-sample chunks, forced continuation-slot
misses, a late ahead worker, a fake preemption, a delayed ring reader — each firing from the
seeded generator.

## 3. Measuring latency, jitter and glitches

- **Round trip**: Oblique Audio's RTL Utility (a pulse through a loopback cable, compared
  with what the driver reports; https://www.gearnews.com/rtl-utility-for-measuring-audio-latency-gets-an-official-release/
  **[fetched]**), `jack_iodelay` **[excerpt]**, Ableton's Driver Error Compensation
  **[excerpt]**. Cantus already has a loopback probe (ADR 0025 §7). Android's method
  correlates a coded burst and reports a confidence
  (https://source.android.com/docs/core/audio/latency/measure **[fetched]**; OboeTester
  sends "a random series of bits encoded using smoothed Manchester encoding",
  https://github.com/google/oboe/blob/main/apps/OboeTester/docs/Usage.md **[fetched]**) — a
  coded burst is more robust than a click against noise and room sound.
- **Glitches, objectively**: OboeTester's glitch test locks onto a sine's magnitude and
  phase and flags any sample that departs from the prediction; if the xrun count rises
  with each glitch, the cause is preemption. Fuchsia's `SlopeChecker` flags any step larger
  than the steepest step a clean sine of that frequency can take
  (https://fuchsia.googlesource.com/fuchsia/+/main/src/media/audio/lib/analysis/glitch.h
  **[fetched]**). DAWbench Universal itself carried **a plain sine track with no effects
  "used for detecting glitching"** (M. Walker, "Solve latency problems", *Sound On Sound*,
  May 2008, https://www.soundonsound.com/node/4910245 **[fetched]**).
- **Distributions**: HdrHistogram (G. Tene; "no allocation operations involved in
  recording", 3–6 ns per record; https://hdrhistogram.github.io/HdrHistogram/ **[fetched]**)
  and **coordinated omission** — when a stall stops the measuring loop, the samples that
  should have been taken during it are never recorded and percentiles are wrong by orders
  of magnitude (G. Tene, "How NOT to measure latency", QCon 2012 **[excerpt]**).
- **The OS**: LatencyMon (ISR/DPC times and hard page faults per driver,
  https://www.resplendence.com/latencymon **[fetched]**); `cyclictest` (rt-tests,
  https://man.archlinux.org/man/cyclictest.8.en **[fetched]**); Android's list of latency
  contributors names CFS scheduling, priority inversion, long IRQs and **DVFS (frequency
  scaling)** (https://source.android.com/docs/core/audio/latency/contrib **[fetched]**).

**For Cantus.** (a) The bench's own note — "blocks run back to back … a live callback has
idle gaps between blocks (caches cool, the core may clock down), so these are lower bounds
on a live block" — is exactly the coordinated-omission and DVFS problem. Add a **paced
mode** that waits for a timer at the device period between blocks (spinning, not sleeping,
on a worker-class thread) so caches cool and clocks drop as they do live; report both.
(b) A **glitch-detector node**: a sine source at the head of the graph and a phase-locked
predictor on the master, so the live DAWbench figure is decided by a detector, not by ear
or by a driver's xrun counter (ASIO often reports none). (c) Count **missed callbacks**
from jumps in the device's sample position and record them as late blocks (HdrHistogram's
`record_correct`), or a dropout vanishes from the percentiles exactly when it matters.

## 4. How DAWs are benchmarked

- **Origins.** M. Walker, "PC benchmark tests", *Sound On Sound*, July 2006
  (https://www.soundonsound.com/techniques/pc-benchmark-tests **[fetched]**): plug-ins added
  one at a time until "obvious audio breakup", CPU meters deliberately ignored as
  inaccurate at low latency. P. Gardner, "CPU performance tested", *SOS*, July 2015
  (https://www.soundonsound.com/reviews/cpu-performance-tested **[fetched]**): DAWbench DSP
  Universal stacks ReaXcomp instances in REAPER until the audio corrupts — and "won't give
  the full picture in regards to ASIO performance".
- **Today.** V. Curigliano, "DAWbench 2026 suite", Jan 2026
  (https://dawbench.substack.com/p/dawbench-suite-2026 **[fetched]**): DSP/BUS on Analog
  Obsession TUBA at 4× oversampling (the old SGA1566's footprint), 640 and 960 tracks, **in
  serial and parallel bus variants**; DSP MIX, a realistic mix with buses; VI SMP on Kontakt
  8 Player, VI SYN on Surge XT, OB-Xf and Vital; sessions for Bitwig, Cubase, REAPER, Sonar,
  Studio One. Interface ratings normalise to an RME reference and multiply by an RTL factor
  (https://dawbench.substack.com/p/audio-interface-low-latency-performance-d25 **[fetched]**).
  PCAudioLabs: ReaXcomp on 48 tracks at 44.1/96 kHz, buffers 16–1024; Kontakt at 20 voices a
  track (https://pcaudiolabs.com/pcaudiolabs-pro-audio-cpu-benchmarks-and-comparisons/
  **[fetched]**).
- **Meters.** Steinberg's Audio Performance Monitor separates **Real-Time (Average)**, the
  **ASIO-Guard (Average)** path and **Peak** — "if this meter peaks/clips, it will cause an
  audio dropout" — and lists dropouts on a timeline with the tracks involved
  (https://www.steinberg.help/r/new-audio-performance-monitor-in-cubase-14/nuendo-14
  **[fetched]**). No peer-reviewed benchmark of audio engines was found.

**For Cantus.** The bench's method already follows DAWbench closely and fixes its worst
flaw (the by-ear call) for the offline estimate. Add: the **serial-chain variant** beside
the wide one (DAWbench 2026 added both because a parallel engine gains nothing on chains —
precisely ADR 0031's Amdahl caveat and the case §7's pipelining is for); report the
**real-time path and the ahead path separately with their peaks**, like Steinberg; decide
the live figure with the glitch detector of §3.

## 5. Proving real-time safety in code

RealtimeSanitizer (RTSan), Clang 20: in `[[clang::nonblocking]]` functions it intercepts
`malloc`/`free`, mutexes, blocking system calls and functions marked `[[clang::blocking]]`
(https://clang.llvm.org/docs/RealtimeSanitizer.html **[fetched]**); by D. Trevelyan and
A. Barker, upstreamed with C. Apple; **Linux and macOS only, not Windows**
(https://github.com/realtime-sanitizer/rtsan **[fetched]**; C. Apple, D. Trevelyan, "LLVM's
realtime safety revolution", CppCon 2024 **[excerpt]**). Clang's function-effect analysis
checks `nonblocking`/`nonallocating` at compile time
(https://clang.llvm.org/docs/FunctionEffectAnalysis.html **[fetched]**). **Rust:** nightly
`-Zsanitizer=realtime` with `#[sanitize(realtime = "nonblocking")]` (rust-lang/rust PR
#147935, merged 2025-11-08, https://github.com/rust-lang/rust/pull/147935 **[fetched]**) and
the `rtsan-standalone` crate on stable (`#[nonblocking]`, Linux/macOS,
https://docs.rs/rtsan-standalone **[fetched]**). `assert_no_alloc` sees only the global
allocator (https://docs.rs/assert_no_alloc **[fetched]**). pluginval's `--rtcheck` (D.
Rowland, JUCE forum, Oct 2025, https://forum.juce.com/t/pluginval-real-time-safety-checking/67439
**[fetched]**) interposes allocation, syscalls and mutexes during `process`.

**For Cantus.** `tests/rt_memory.rs` is the `assert_no_alloc` approach and only covers
allocation; ADR 0031 amends ADR 0005's "no syscalls" rule and adds threads, so it needs a
tool that sees syscalls and locks. Add an **RTSan job on Linux CI** over `Processor::process`
and every pool thread's loop, with the one permitted wake (§2.4 of the ADR) as an explicit
scoped exception — which turns the amended rule into something a machine checks. Keep the
counting allocator as the gate that runs on Windows. Run pluginval `--rtcheck` on the
exported CLAP/VST3 builds (ADR 0020) on macOS.

## 6. Performance regression in CI

T. Kalibera, R. Jones, "Rigorous benchmarking in reasonable time", ISMM 2013
(https://kar.kent.ac.uk/33611/ **[fetched]**): variation lives at several levels (iteration,
process, build); spend repetitions where the variance is, and **repeat across fresh
processes**, because thread placement and memory layout change only between processes.
Gungraun (formerly iai-callgrind) counts instructions under Callgrind — deterministic even
on shared CI, **not on Windows** (https://github.com/iai-callgrind/iai-callgrind
**[fetched]**). LLVM's benchmarking tips: turbo off, SMT siblings off, cores reserved
(https://llvm.org/docs/Benchmarking.html **[fetched]**). Bencher's thresholds (t-test, IQR;
https://bencher.dev/docs/explanation/thresholds/ **[fetched]**).

**For Cantus.** The bench's gate (25 % and non-overlapping deciles, within one machine
class) is sound. Add a **deterministic tier** — instruction counts of each kernel and of
one serial engine block on a Linux runner — that can fail every pull request without a
quiet machine, and run the wall-clock suite across several fresh processes rather than
more iterations of one.

## 7. Bit-exact practice

B. Dawson, "Floating-point determinism", 2013
(https://randomascii.wordpress.com/2013/07/16/floating-point-determinism/ **[fetched]**):
FPU state is per thread; transcendental functions are not pinned by IEEE; "same binary,
same processor" is easy and anything wider takes discipline. Rust's guarantees and their
limits: RFC 3514 (https://rust-lang.github.io/rfcs/3514-float-semantics.html **[fetched]**) —
no implicit FMA, NaN payloads nondeterministic. Reproducible summation independent of
order: J. Demmel, H. D. Nguyen, "Parallel reproducible summation", *IEEE Trans. Computers*
2015 **[excerpt]**; P. Ahrens, J. Demmel, H. D. Nguyen, "Efficient reproducible floating
point summation and BLAS", UCB/EECS-2015-229
(https://www2.eecs.berkeley.edu/Pubs/TechRpts/2015/EECS-2015-229.html **[fetched]**) — a
binned accumulator, ~7n additions, ~4× slower than MKL on a dot product.

**For Cantus.** ADR 0031 §1.3's canonical gather order is the cheap and correct answer —
reproducible summation would cost 4× and is needed only where the order *cannot* be fixed,
which the plan avoids. Two additions from this literature: assert at the start of every
node that the thread's MXCSR is the expected one (a hosted plug-in may have changed it,
and FPU state is per thread — the ADR re-asserts it, which is cheaper still, so count the
times it differed); and make NaN in any golden comparison a failure, never a bit compare.
Golden hashes are valid per (target, toolchain, machine class) until the transcendental
functions are Cantus's own (`dsp-quality.md` §9).
