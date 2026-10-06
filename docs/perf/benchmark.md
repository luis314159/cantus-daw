# How Cantus is measured

*Round 20 (bench). The goal the project states — more tracks and plug-ins than any DAW on
the same machine, at a lower buffer, with no dropouts, and exports faster than all of them
(`docs/BACKLOG-excellence.md` §4b) — is a claim until it is measured the same way every
time. This file is the method. `docs/perf/baseline.md` is the first set of numbers.*

## 1. Rules

1. **Release builds only.** `cantus-bench` warns when it is a debug build; its numbers
   then mean nothing. The `release` profile is the one users run (thin LTO, 4 codegen
   units, opt-level 3).
2. **Medians of repeated runs, with their spread.** Every metric is run *n* times (3 in
   the quick suite, 5 in the full one; `--runs` changes it) and reported as the median
   with the 10th–90th percentile range and min–max. Never a single run, never a mean.
3. **Say what the machine was doing — and measure CPU time beside wall time.** A shared
   machine (other builds running) widens the spread; the report records the CPU, the core
   count and the machine class, and the baseline document says what else was running.
   Every session run also records the thread's own CPU time (`cpu.rs`: `GetThreadTimes`,
   `CLOCK_THREAD_CPUTIME_ID`): `….cpu` is the work per block without the waits for a
   core, and `….share` the share of a core the run got (100 % on an idle machine). On a
   busy machine the `.cpu` figures are the ones to read; the wall figures are what a
   callback would have lived through.
4. **Pin on hybrid CPUs.** On a CPU with performance and efficiency cores (Intel 12th gen
   and later, Apple silicon) a thread that migrates measures the scheduler. On Windows,
   `cargo run -p xtask -- perf --pin 2` starts the suite on one logical CPU
   (`start /AFFINITY`); pick a performance core.
5. **Same sessions everywhere.** Every session is built in code
   (`crates/cantus-bench/src/sessions.rs`), so every machine and every future version
   plays exactly the same notes through exactly the same settings.
6. **Nothing is optimised to the benchmark.** A change that makes a number better must
   make the program better; a kernel that special-cases the test signal is a bug.

## 2. What is measured

Run everything: `cargo run --release -p cantus-bench -- --full --json out.json`
(start-up also needs `cargo build --release -p cantus`). Groups: `--only fx,inst,…`.

### 2.1 DSP kernels — `fx.*`, `inst.*`

- **Effects.** Each of the twelve native effects, as the engine runs it
  (`EffectUnit::process_keyed`, 128-frame stereo blocks, 48 kHz), at its **defaults** and
  at **heavy** settings (`kernels::heavy`: linear phase at its finest with eight bands for
  Aequalis, 4× oversampling for Comprimo and Fervor, true peak and the longest lookahead
  for Terminus and Culmen, linear-phase crossovers for Multiplex, an 8-second Spatium…).
  The signal is noise and two tones swelling and falling at 2 Hz, so detectors work.
  One second of warm-up, then each run's median block. Unit: **ns per stereo frame**
  (`fx.<name>.<default|heavy>`), and `…fit128`: how many instances one core could run in a
  128-sample buffer (the budget over one instance's block — a ceiling, not a promise).
- **Instruments.** Firmus, Pulsus and the seven native instruments with **1, 8 and 32
  voices** held (Pulsus: that many hits every sixteenth), through the processor (one
  track) because voices are fed by the sequencer. Unit: ns per stereo frame for the whole
  voice set (`inst.<name>.v<n>`); the note says how many voices actually sounded — an
  instrument caps its own polyphony.

### 2.2 The processor at scale — `engine.*`

The **demo songs scaled up** (`sessions::scaled(n)`): the house song's whole desk (drum
bus, returns, the mastering chain on the master), with tracks from all six demos added
round-robin until there are *n*, each with a copy of its own strip and inserts; every
song's full-band section is lined up on bars 16–24 and looped. Sizes 10, 32, 64 and 100
tracks at 128 frames, and 64 tracks at 64 frames (the quick suite: 10 and 32 tracks at
128). Each run: one second of warm-up, then `session_seconds` of blocks timed one by one
around `Processor::process`. Reported: the median block (`engine.s<n>.b<frames>`, µs),
its 99th percentile (`.p99`), the thread's CPU time per block (`.cpu`), the share of a
core it got (`.share`) and the median's share of the budget (`.load`, %).

The engine holds **at most 64 tracks and 64 strips** (`MAX_TRACKS`, `MAX_STRIPS`): the
100-track session is built and played, the engine plays its first 64, and the metric says
so. That ceiling is the first thing between Cantus and the 100-track goal.

Blocks run back to back, as fast as the CPU allows; a live callback has idle gaps between
blocks (caches cool, the core may clock down), so these are lower bounds on a live block.

**Hot spots.** The engine's own profiler (`cantus_engine::profile`, §7) charges every
nanosecond of a block to one node — a track's instrument, a strip's mixing, an insert, a
master insert, or the engine's own work. The report lists the fifteen heaviest nodes of
the 64-track session, named as the user would name them ("Spatium on Pad bus").

`engine.profiler_overhead.s32` is the profiler's own cost: the 32-track session with it on
and off, as a percentage of the block.

### 2.3 Offline render — `render.*`

The export path itself (`cantus_engine::render`, 512-frame blocks): each demo song whole,
and the scaled sessions' looped section once through (the quick suite: the 10-track
section only). Unit: **multiples of real time** — audio seconds over wall seconds
(`.xrt`), and over the rendering thread's CPU seconds (`.xrt_cpu`); higher is better.

### 2.4 Opening a project — `open.*`

The 10- and 64-track sessions saved to a temporary `.cantus` file, then: read and parse
with the integrity check (`cantus_project::load`), compile the snapshot, build every
effect's and instrument's unit (`units_for` decodes samples and prepares wavetables). Each
phase in ms, and the total — what stands between a double-click and the first note, less
the window.

### 2.5 UI frames — `ui.*`

The real application (`CantusApp::from_parts` around a manual engine), driven through
`eframe::App::ui` at 1920×1080 with a block of audio before every frame so the transport
plays and meters move (the harness of `crates/cantus-ui/tests/frame_budget.rs`). Build +
tessellate, CPU, ms per frame — median of each run's median (`ui.<case>`) and its 95th
percentile (`.p95`). Cases: the 64-track session in the mixer, in the piano roll (with the
arrangement), and the project the program opens on. The GPU's share is
`research/ui/rendering-performance.md` §2.3's method and is not in this suite.

### 2.6 Start-up — `startup.*`

The release executable is started with a fresh, empty data folder (`LOCALAPPDATA`,
`XDG_DATA_HOME` and `HOME` pointed at a temporary directory: nothing of the user's is read
or written, every start is a first start) and `CANTUS_STARTUP_REPORT=<file>`, which wraps
the application in a probe (`cantus_ui::startup_probe`) that writes, once the main window
has drawn its second frame, how long each stage took since `run` began — then closes it.
`startup.ready` is spawn → report (what a person waits through), `startup.splash` the
splash's first frame, `startup.app` the assembled application's first frame. One
unmeasured start first, so the file cache is warm; a cold start (after a reboot) is
measured by hand and noted. It opens a window and the sound card: full suite only.

## 3. The DSP test (DAWbench-style) — `dawbench.dsp.*`

DAWbench (Vin Curigliano's long-running DAW test suite) asks one question per test: *how
much of the same load does a host sustain at a given buffer before it drops out?* Its DSP
tests stack many instances of one plug-in on many tracks of one sustained source and add
tracks until the audio breaks. Cantus's version is defined so the same session can be
rebuilt in REAPER, Ableton Live, FL Studio, Bitwig and Cubase:

| | |
|---|---|
| Rate, format | 48 kHz, stereo, 32-bit float |
| Buffer sizes | 64 and 128 samples (256 for hosts that cannot run lower) |
| Track | one sustained source → 8 compressor instances in series → master |
| Source | a sawtooth held for the whole loop, one per track (Cantus: Firmus, one voice; in other hosts: track 1 of the Cantus session rendered to WAV, on an audio track) |
| Compressor | threshold −30 dB, ratio 4:1, everything else at its default, so every instance is reducing gain |
| Variant A | **Comprimo**, Cantus's compressor — natively in Cantus, and as the CLAP/VST3 build (`cargo run -p xtask -- bundle comprimo`, bit-identical to the native one, ADR 0020) in every other host: identical DSP, so the difference is the host |
| Variant B | one free third-party compressor installed as CLAP or VST3 in every host, Cantus included (through its plug-in hosting, ADR 0019), its name and version recorded with the result: the whole package |
| Count | tracks × 8 = instances |

**Procedure in each host.** Start from 8 tracks; play the loop for 60 s; if no dropout,
add 8 tracks (then 1 at a time near the limit); the result is the most tracks that played
60 s clean, × 8. Three passes, the median. A *dropout* is the host's own xrun/underrun
counter moving (REAPER's Performance Meter, FL Studio's underrun count, Cantus's status
bar) or, where a host has none, a click heard on headphones.

**Cantus's offline estimate** (`cantus-bench --only dawbench`): the track count is found by
bisection; a count *passes* when the 99th percentile of 128- (or 64-) frame blocks stays
under 80 % of the budget over the run. It is computed in seconds without a device and is
what the trend is tracked with; the live figure above is what is published.

## 4. The VI test — `dawbench.vi.*`

| | |
|---|---|
| Track | one instrument instance, no inserts → master |
| MIDI | an eight-note chord every half note, four chords over four bars at 120 BPM, notes at 90 % of a half note so releases overlap the next chord (`sessions::vi`; export it with *File ▸ Export MIDI* to load the same notes elsewhere) |
| Instrument, Cantus-only | **Calor** (the analog polysynth) at its default patch — `dawbench.vi.b<frames>` |
| Instrument, cross-DAW | a free synth available as CLAP/VST3 in every host, at its init patch (e.g. Surge XT), hosted in Cantus as an instrument (round 20's hosted instruments); its version recorded |
| Count | tracks × 8 = voices |

Same procedure and same dropout rule as §3. Cantus's native instruments are not yet built
as plug-ins (ADR 0020 exports the effects), so the cross-DAW VI comparison runs on a
third-party synth until they are.

## 5. Comparing with other DAWs, live

On the same machine, the same day: the same audio interface and driver (ASIO on Windows),
the same buffer, the power plan at maximum performance, on mains power, nothing else
running, Wi-Fi off. Install the plug-ins from §3–§4 in every host. Record each host's
version, the interface and driver, and the CPU's temperature at the start. Results go in a
table per buffer size with the method's version (this file's date) beside them — and are
published with this method, or not at all.

The 100-track mix (`engine.s100`) is for Cantus's own trend: the native effects in it are
Cantus's, and the route to rebuild it elsewhere (DAWproject export with the CLAP/VST3 builds
of the effects installed) is not verified yet.

## 6. The gate — `cargo run -p xtask -- perf`

`xtask perf` builds `cantus-bench` in release, runs the **quick** suite (every group but
start-up and DAWbench, three runs each) into `target/perf/latest.json`,
and compares it with **`perf/baselines/<machine class>.json`**. The quick suite is the
kernels, the 10- and 32-track sessions, the 10-track render, project open and the UI
frames — a few minutes on a quiet laptop:

- The **machine class** is `CANTUS_PERF_CLASS` if set, else the OS and the CPU model as a
  slug (`windows-12th-gen-intel-core-i7-12700h`). Baselines are only ever compared within
  a class: a number from one laptop says nothing about another.
- A gated metric **regresses** when its median is worse than the baseline's by more than
  the threshold (25 %; `--threshold`) **and** the runs do not overlap (the current best
  run is worse than the baseline's worst run). Both, so that one noisy run on a busy
  machine does not fail the build, and a small repeatable shift does not either.
  Ungated metrics (99th percentiles, capacities, start-up) are printed, never failed.
- No baseline for the class: the run passes and says how to make one —
  `cargo run -p xtask -- perf --save` on a quiet machine; review the JSON; commit it.
- `--full` runs and compares the full suite against `<class>-full.json`.

CI (`.github/workflows/ci.yml`, job `perf`) runs the quick suite on `windows-latest`
with the class `gha-windows-latest` and keeps the report as an artifact. Hosted runners
are shared and vary by tens of percent, so the job **reports and does not fail** until a
baseline for that class has been gathered over enough runs to know its spread; then remove
`continue-on-error`. A self-hosted runner with a fixed class is the real gate.

## 7. Inside the engine: histograms and blame

`cantus_engine::profile` (ADR 0005 rules hold: no allocation, no lock, no syscall beyond
the clock the callback already reads):

- **Block histogram.** Every block's wall time goes into 64 log-spaced buckets (four per
  octave, 1 µs to ~55 ms) in `EngineStats::histogram`, which travels to the UI by value
  like every other stat. The engine readout's tooltip in the status bar gives its median,
  99th and 99.9th percentiles, the slowest block and the budget.
- **Dropout detector with blame.** A profiler charges every nanosecond of a block to
  exactly one node by stamping the clock at each boundary: the engine's own work, each
  track's instrument (with its audio clips and input), each strip's mixing, each insert,
  each master insert. When a block overruns its budget, the node that took longest is
  resolved to the project's ids (`Culprit`) and kept in `EngineStats::last_dropout`; the
  status bar shows it in words — *"Dropout: Spatium on Vocal took 3.1 ms of 2.7 ms"* — with
  the whole block in its tooltip. The same totals are the benchmark's hot spots.
- **Cost.** One clock read per node per segment. `engine.profiler_overhead.s32` measures
  it; the offline renderer switches the profiler off (`Processor::set_profiling(false)`).
- **Proof.** `crates/cantus-engine/tests/profile.rs`: the profiler changes no sample (on,
  off and the offline render null exactly), every block is counted, and a deliberately slow
  insert, master insert, or nothing at all is what gets blamed.
