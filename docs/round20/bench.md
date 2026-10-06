# Round 20 — bench: measure before optimising

*Builder: bench (`r20/bench`). Brief: `docs/round20/briefs-backlog.md` § bench;
`docs/BACKLOG-excellence.md` §4b. Method: `docs/perf/benchmark.md`. Numbers:
`docs/perf/baseline.md`.*

## What was built

**In the engine — `cantus_engine::profile`** (new file; small hooks in `processor.rs`,
`processor_master.rs`, `render.rs`, two fields in `EngineStats`):

- `BlockHistogram`: every callback's wall time in 64 log-spaced buckets (4 per octave,
  1 µs – 55 ms), in `EngineStats::histogram` — it travels by value over the existing
  ring, so nothing new crosses threads. Quantiles, windows (`since`), counts above a time.
- `Profiler`: charges every nanosecond of a block to exactly one node — the engine's own
  work, a track's instrument, a strip's mixing, an insert, a master insert — by stamping
  the clock at each boundary (one `Instant::now` per node per segment; a node list of 779
  preallocated counters; only the touched ones are visited at block end).
- **Dropout detector with blame**: when a block overruns, the longest node is resolved to
  project ids (`Culprit`) and kept in `EngineStats::last_dropout` (`Dropout`: culprit,
  its µs, the block's µs, the budget, which block). `Processor::{set_profiling, profiler,
  profiler_mut, culprit}`. The offline renderer turns the per-node clock off.
- ADR 0005 holds: no allocation (the `rt_memory` test passes with it on), no lock, no
  syscall beyond the clock the callback already reads; null tests unchanged.

**In the UI** — `perf_status.rs` (new): names a culprit as the user would ("Comprimo
compressor on Bass", "Calor on Chords", "mixing on Drum bus", "Terminus limiter on
Master"); the status bar shows **"Dropout: Spatium on Vocal took 3.1 ms of 2.7 ms"** in
the warning colour beside the engine readout, with the whole block in its tooltip; the
engine readout's tooltip gives the callback histogram (median, 99 %, 99.9 %, slowest,
budget, overruns). New locale area `perf.{en,es}`. `startup_probe.rs` (new): with
`CANTUS_STARTUP_REPORT=<file>` the app writes its start-up stages and closes — off
otherwise.

**`crates/cantus-bench`** (new crate, a workspace member, not a default member):
kernels (12 effects × default/heavy, 9 instruments × 1/8/32 voices), the processor on the
demo songs scaled to 10/32/64/100 tracks (wall *and* thread CPU time per block, p99, load,
the share of a core the run got), the profiler's own cost, offline render × real time,
project open by phase, UI frame times (whole window, 64 tracks, mixer and piano roll),
start-up of the real executable, DAWbench-style DSP/VI capacity by bisection; JSON reports
per machine class; the compare/gate logic. Its own harness, not criterion (why: `lib.rs`).

**The gate** — `cargo run -p xtask -- perf` (`xtask/src/perf.rs`): builds in release,
runs the quick suite, compares with `perf/baselines/<class>.json`; `--save`, `--full`,
`--only`, `--pin <cpu>` (Windows `start /AFFINITY`), `--threshold`, `--class`. A regression
needs both a median past the threshold (25 %) and non-overlapping runs. CI: a `perf` job
on `windows-latest` that reports and keeps the JSON as an artifact, `continue-on-error`
until that runner class has a baseline.

**Docs** — `docs/perf/benchmark.md` (the method, the DAWbench-style DSP and VI tests
defined for REAPER/Ableton/FL/Bitwig/Cubase, the live procedure, the gate, the engine
instrumentation) and `docs/perf/baseline.md` (today's numbers and hot spots).
