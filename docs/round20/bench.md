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

## Baseline in one paragraph

Numbers are from a quiet machine, pinned to a P-core. The ones marked *loaded* were taken
while ten other builds were running, and are 2–3× worse even in CPU time. All are in
`docs/perf/baseline.md`.

- **Kernels:** Comprimo 143 ns/frame (145 instances per core at 128 samples). Spatium
  1.24 µs, Culmen 1.0 µs, Terminus 0.73 µs, Calor 3.5 µs at 8 voices, Exemplar 11.9 µs at
  32 voices.
- **Engine:** one core cannot play the scaled demo sessions in real time. The 10-track
  session takes 2.75 ms of CPU per 2.67 ms block (103 %), and the 32-track one takes 448 %.
- **Profiler overhead:** +0.1 % median, which is within noise.
- **Export:** the demos render at about 0.5–1.6× real time per CPU second.
- **Open:** a 64-track project opens in 182 ms.
- **UI:** the 64-track mixer frame is 6.4 ms p50.
- **Start-up:** 0.7 s to the splash, 5.7 s to the application.

## Hot spots (ranked in baseline.md §8)

1. The processor runs on one thread, and export does too.
2. `MAX_TRACKS` and `MAX_STRIPS` are both 64. A 100-track session plays only 64 tracks and
   says nothing.
3. Calor voices.
4. Unda pads.
5. Exemplar costs about 10× what a sampler should.
6. Pulsus per hit.
7. Spatium, Culmen and Terminus. Most of Terminus's cost is true-peak oversampling, which
   is on by default.
8. Inserts on silent strips always run.
9. Start-up is 5–6 s.
10. The mixer frame is over its 4 ms budget on real sessions.

## Tests

- `cantus-engine`:
  - `profile` unit tests: 3.
  - `tests/profile.rs`, 5 tests: profiling changes no sample, comparing on, off and the
    offline render, which null exactly. Every block is counted. A slow insert, or a slow
    master insert, is the one blamed. With the profiler off, nobody is blamed.
  - These still pass: `null_tests`, `rt_memory` (no allocation with the profiler on),
    `pdc`, `master_chain`.
- `cantus-bench`:
  - `stats` unit test.
  - `tests/bench.rs`, 5 tests: the sessions have the tracks asked for, with fresh ids. The
    DAWbench sessions are built as the method says. The gate fails exactly on a clear
    regression; noisy, improved, new and missing results are reported and do not fail it.
    A report round-trips and its machine-class slug is right. The rig plays a session and
    names its hot spots.
- `cantus-ui`:
  - `tests/perf_status.rs`, 2 tests. Every culprit is named as the user names it. A real
    overrun (from 1-frame blocks) is painted in the status bar; I rendered it and looked
    at it.
  - These still pass: the lib tests (131, including the locale parity test),
    `frame_budget`, `workspace_budget`, `views_budget`, `kit_lint`.
- `cargo clippy` is clean on `cantus-engine`, `cantus-bench`, `cantus-ui` and `xtask`
  (the two warnings in `cantus-plugins` were already there). `cargo fmt` has been run.

## Files touched outside the new ones

- `Cargo.toml`: one workspace member.
- `Cargo.lock`.
- `.github/workflows/ci.yml`: the `perf` job.
- `crates/cantus-engine/src/{lib.rs, messages.rs, processor.rs, processor_master.rs,
  render.rs}`: these are small hooks.
- `crates/cantus-ui/src/{lib.rs, i18n.rs, views/workspace_status.rs}`:
  - `lib.rs`: the start-up probe wrapper in `run`.
  - `i18n.rs`: one line per locale for the `perf` area.
  - `workspace_status.rs`: the dropout readout and the histogram tooltip.
- `xtask/src/main.rs`: one match arm.

## Left

- **The quiet re-run of the full suite was stopped unfinished** at the integrator's
  request. Still missing on a quiet machine:
  - the 64- and 100-track sessions, and the 64-frame session;
  - the whole-song renders;
  - the DAWbench capacities with the new median criterion.
  Rerun with `cargo run -p xtask -- perf --full --save --pin 2`.
- The live DAWbench comparison against REAPER, Ableton, FL, Bitwig and Cubase (method §3–§5)
  needs those hosts and an ASIO interface.
- `perf/baselines/gha-windows-latest.json` does not exist yet. Once the CI job has run a
  few times, store it and drop `continue-on-error`.
- The callback histogram is cumulative since start. A "reset" or a windowed view in the
  UI would help.
- Device-level dropouts (a late callback whose own work was quick) are not detected yet.
  Only overruns of our own work are.
