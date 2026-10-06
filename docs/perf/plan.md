# The performance plan — ADR 0031 in phases, for parallel builders

*Round 20 (perf-design). How `docs/adr/0031-performance-architecture.md` gets built: seven
phases, each one useful alone, each with its builders, the files each one owns, the
tests that must hold before it merges and the number the bench must show. Targets use
`cantus-bench`'s metric names (`docs/perf/benchmark.md`); "×" means against the baseline of
the same machine class (`perf/baselines/<class>.json`), so every target is checkable the
day the bench's baseline lands.*

## Rules for every phase

1. **One owner per hot file.** `processor.rs` and the `graph/` node files have exactly one
   owner at a time (the *graph* builder below). Anyone else who needs a change there writes
   it as a request in the round's coordination notes, not as an edit.
2. **Bits first.** Before a phase merges, every test in `crates/cantus-engine/tests` passes
   with the pool at `W = 0` **and** `W ∈ {1, 3, 7}` (`CANTUS_WORKERS`), and the `v5.cantus`
   SHA-256 in `null_tests.rs` is unchanged. A phase that changes a sample on purpose does it
   in a commit of its own with a `DSP_VERSION` bump and the reason in STATUS.
3. **Nothing allocates on a rendering thread.** `rt_memory.rs` counts the callback and every
   pool thread.
4. **Measure before and after** with `cargo run -p xtask -- perf` on the same machine class;
   the phase's target goes in its report with the JSON.
5. **Worktree per builder** (`C:\projects\daw-r20\<builder>`), snapshot commits, file-by-file
   merge by the integrator (as in rounds 17–20).

## Dependency picture

```
P0a rt ─────────────┐
P0b at-rest ────────┤
P0c graph (serial) ─┴─► P1 pool ──► P2 export + suspension ──► P4 anticipation ──► P5 cache
P1s sched (mock) ───────┘                                         │
P3 SIMD + v3 build ── in parallel with P1–P2, its own files ──────┘           P6 sandbox
                                                                (after P1; prototype any time)
```

## Phase 0 — the graph in the serial engine (3 builders + 1 starting P1 early)

The riskiest change, made first and without threads: per-node segments, canonical gather,
note outboxes, FTZ inside `process`, the limits raised. ADR §1, §5.2.

| Builder | Owns (creates / edits) | Must not touch |
|---|---|---|
| **rt** | new `crates/cantus-rt/` (`lib.rs`, `denormal.rs` — moved from `cantus-plugins/src/ftz.rs`, `thread.rs` — MMCSS / Mach time-constraint + workgroups / rtkit via `audio_thread_priority`, `topology.rs` — physical P/E cores via CPU sets, `pad.rs` — cache-line padding); `cantus-plugins/src/ftz.rs` becomes a re-export; root `Cargo.toml` member | the engine |
| **at-rest** | `FxUnit::at_rest` in `fx/mod.rs`, `InstrumentUnit::at_rest` in `inst/mod.rs`, `EffectUnit::at_rest` in `effects.rs` (the four built-in arms), the implementations in `fx/*.rs`, `inst/*.rs` and the DSP state they read in `cantus-dsp`; tests `fx_contract.rs`, `inst_contract.rs` (+ `tests/support/heavy.rs`: the bench's *heavy* settings, copied until the bench crate merges) | `processor*.rs`, `graph/` |
| **graph** | `processor.rs` (becomes control phase + glue), `processor_master.rs` and `processor_rec.rs` (move into nodes), new `graph/{mod.rs, plan.rs, gather.rs, track.rs, strip.rs, master.rs, notes.rs}`, `messages.rs` (`SetSnapshot { snapshot, plan }`, `GraphNeed`, limits 256), `backend.rs` (plan only when `topology_key` changes), `render.rs`, `stems.rs`, `stems_master.rs` (send the plan; tap points move into nodes), `profile.rs` (the bench's profiler keeps charging nodes — coordinate with the bench builder) | `fx/`, `inst/`, `cantus-dsp` |
| **sched** (Phase 1's, starts now) | new `crates/cantus-engine/src/sched/{mod.rs, queue.rs, pool.rs, fuzz.rs}` against a `trait Graph` and mock graphs only; `tests/sched.rs` | everything else |

**Order inside the phase:** *at-rest*'s extended contract tests (heavy settings, random
split points, blocks 1–4096) merge **before** *graph* — they are what makes per-node
segmentation safe (ADR risk 1). *rt* merges any time; *graph* then calls
`DenormalGuard` in `process`.

**Tests that must hold:**
- Everything in `crates/cantus-engine/tests` unchanged, the `v5.cantus` hash, `pdc.rs`
  twins at exact silence, `midi_fx.rs` (notes now travel by outbox), `heard.rs`,
  `stems.rs` (stems still add up to the master), `recording*.rs`, `rt_memory.rs`.
- New `tests/graph_order.rs`: on random topologies (proptest: tracks, strips, outputs,
  sends, keys, `midi_from`, mutes, a mono device), the gather of `graph/gather.rs` equals a
  reference implementation of the old scatter order, bit for bit.
- New `tests/limits.rs`: 256 tracks and 256 strips play; a 257th is refused as today.
- New `tests/ftz.rs`: `process` runs with FTZ/DAZ set and restores the caller's MXCSR.
- `at-rest`: the twin test for every unit kind (ADR §5.1).
- `cantus-rt`: Miri on the padding and denormal types; `examples/rt_probe.rs` prints the
  topology and whether MMCSS/FIFO was granted (run by hand on Windows, macOS, Linux).

**Target:** `engine.s64.b128` median ≤ 0.9× (each strip chain called once per block
instead of once per segment); `engine.s100.*` plays all 100 tracks.

## Phase 1 — the live pool (2 builders)

ADR §2. The *graph* builder integrates; *sched* finishes the pool.

| Builder | Owns |
|---|---|
| **sched** | `sched/` (pool lifecycle, spin/yield/park with the time budget, `wakes`, the continuation slot, two priority queues, the CLAP `thread-pool` sub-queue API), `tests/sched.rs`, the schedule fuzz harness used by every engine test (`sched/fuzz.rs`, seeded) |
| **graph** | nodes behind `AtomicRefCell`, per-worker scratch, the processor driving the pool, clustering and ranks (sent as arrays, §2.5), the serial fallback with hysteresis, `devices.rs`/`backend.rs` starting the pool with `cantus-rt` priority and placement, `EngineStats::{schedule_conflicts, wakes}`, `profile.rs` per worker + critical-path blame (with the bench builder) |
| *(host, small)* | `cantus-host/src/clap/host.rs`: the `thread-pool` host extension wired to `sched`'s sub-queue; thread-check unchanged |
| *(ui, small)* | `cantus-ui/src/views/settings_audio.rs` and `settings.rs`: worker count (default *Auto*), the readout's tooltip shows the pool; coordinate with the audio-setup builder who owns the setup chip |

**Tests that must hold:**
- Every engine test at `W = 0` and `W ∈ {1, 3, 7}`; the schedule fuzz: the null corpus and
  the bench's scaled sessions, 1 000 seeds in CI (randomised pop order, continuation slot
  on/off, yields injected), parallel = serial to the bit.
- `schedule_conflicts == 0` in every test; `rt_memory` on every pool thread;
  `wakes` only at idle → busy (a test plays, stops, waits 200 ms, plays).
- The CLAP test plug-in requests `thread-pool` tasks and gets them, on our workers.
- `tests/hardware.rs` (by hand, real device): 64 and 128 frames, the scaled 64-track demo,
  zero overruns over 10 minutes with the UI open.

**Targets:** `engine.s64.b128` median ≤ 0.45×, p99 ≤ 0.6× on 6 P-cores (the master chain
is serial: Amdahl's share); `engine.s100.b128` p99 under 80 % of the budget (§4's bar:
100 tracks at 128 on an 8-core laptop); `dawbench.dsp.b64` ≥ 3×; the profiler's overhead
(`engine.profiler_overhead.s64`) not above its baseline.

## Phase 2 — export in parallel, and suspension (2 builders)

ADR §7.1–7.2, §5.1.

| Builder | Owns |
|---|---|
| **export** | `render.rs`, `stems.rs`, `stems_master.rs`, new `graph/pipeline.rs` (per-edge rings of a few blocks; each insert as its own node in export), the offline pool (all cores, normal priority, `MAX_BLOCK_FRAMES` blocks); `cantus-host`: CLAP `render` OFFLINE in `clap/instance.rs`, VST3 `kOffline` in `vst3/audio_thread.rs` |
| **graph** | suspension in `graph/{track,strip,master}.rs` (a node at rest with all-zero input and empty lines is not called; whole silent subtrees skipped); hosted units: CLAP process status (`SLEEP`, `CONTINUE_IF_NOT_QUIET`, `TAIL`) and `constant_mask`, VST3 `silenceFlags`/`getTailSamples` — the host half in `cantus-host/src/clap/audio.rs` and `vst3/process_objects.rs`, by agreement with the host owner |

**Tests that must hold:** export = real time to the bit at every worker count and block
size (`null_tests.rs`, `stems.rs`, `master_chain.rs`); a pipelined five-insert strip equals
the unpipelined one; the offline render with hosted test plug-ins in OFFLINE mode equals the
real-time render where the plug-in is mode-independent (the gain test plug-in); suspension
changes no sample (the null corpus with suspension forced on and off) and a silent
section's nodes are not called (a counting test unit).

**Targets:** `render.*` ≥ 4× on 14 cores; a five-heavy-insert chain exports ≥ 3× faster
than serially; the scaled session's silent half costs ≤ 10 % of its playing half.

## Phase 3 — SIMD and the v3 build (in parallel with Phases 1–2; 1 + N builders)

ADR §5.3. Files no other phase touches.

| Builder | Owns |
|---|---|
| **build** | `xtask` (the release build at `x86-64-v3` and the `x86-64-v2` fallback; `--v4` behind a flag), the start-up stub that picks one (a small `launch` binary beside `crates/cantus-app`, with the packaging builder), the CI job that compares the null-test hashes of the v2 and v3 builds |
| **kernel-*k*** (one per hot spot, from the bench's list) | one DSP file each in `cantus-dsp` (e.g. the oversampling halfbands, the linear-phase convolver, the biquad cascade) and that kind's tests; `DSP_VERSION` (new, in `cantus-dsp/src/lib.rs`) is bumped only by the integrator, once per merged kernel that changes bits |
| **voices** | structure-of-arrays voice banks for the heaviest native instruments (by `inst.*.v32`), one instrument per commit: `inst/<kind>.rs` and its DSP file |

**Tests that must hold:** each kernel bit-identical to the scalar path it replaces, or a
recorded `DSP_VERSION` bump with goldens updated in the same commit; v2 = v3 hashes in CI;
each unit's own tests and `fx_contract`/`inst_contract`.

**Targets:** heavy kernels ≥ 1.3× (`fx.*.heavy`); `inst.*.v32` ≥ 2× where SoA lands; the
v3 build ≥ 1.15× on `engine.s64.b128` over v2.

## Phase 4 — anticipative processing (3 builders)

ADR §3. Starts when Phase 1 has merged.

| Builder | Owns |
|---|---|
| **ring** | `cantus-rt/src/ring.rs`: the single-writer timestamped ring (cursor `Release`/`Acquire`, slice copies), with `loom` models of write-while-read and wrap, and Miri |
| **ahead** | new `crates/cantus-engine/src/ahead/{mod.rs, cursor.rs, eligibility.rs, switch.rs, declick.rs}`; the ahead workers in `sched/` (by agreement with sched); timestamped meters and telemetry; `EngineStats::ahead_lead`; `EngineMessage::SetPromoted`; the graph builder hands it the node files for this phase |
| **ui-host** | promotion from the UI (an effect or instrument editor open, a gesture begun: `views/fx/frame.rs`, the instrument editors' frame, the kit's parameter gestures); the live track counted live only with a live source (ADR §3.2, `app.rs`); the anticipation level in `views/settings_audio.rs`; *never ahead* per plug-in in `views/plugins.rs`; VST3 `kPrefetch` ↔ `kRealtime` in `cantus-host/src/vst3/audio_thread.rs`; CLAP `render.has_hard_realtime_requirement` read in `clap/instance.rs` |

**Tests that must hold:** ahead = live = offline to the bit at Low/Normal/High, with chunk
sizes randomised; the edit-at-`S` oracle (a serial engine given the same structural edit at
the reported `S` matches to the bit); a parameter edit on an ahead node is heard at its
cursor, on a promoted one at once; the stop/seek crossfade is bounded (no sample step above
the signal's own largest step over the crossfade); a monitored armed track is never ahead
and neither is anything downstream of it; `rt_memory` on ahead workers; meters arrive when
their audio is heard (a test with a known latency).

**Targets:** `dawbench.dsp.b64` ≥ 1.5× Phase 1 with nothing live; a live armed track at 64
samples with the rest of the 64-track session ahead: zero overruns over the bench's run on
the reference laptop; `ahead_lead` ≥ `H/2` at Normal for the scaled 64-track session.

## Phase 5 — the render cache (4 builders)

ADR §4. Starts when Phase 4's cursors and rings have merged.

| Builder | Owns |
|---|---|
| **key** | `crates/cantus-engine/src/cache/key.rs`: the page key from a snapshot (exhaustive destructuring, no `..`), settle points from the at-rest predicate; `tests/cache_key.rs` |
| **store** | new crate `crates/cantus-cache` (safe Rust, std I/O): the page format, the content-addressed store under `%LOCALAPPDATA%\Cantus\cache\render`, LRU by cost per byte, the reader thread with a preallocated page pool, the disk side of the write-behind ring; its own tests (format round trip, eviction, a torn file rejected by its CRC) |
| **play** | `cache/{mod.rs, play.rs, warm.rs, freeze.rs, audit.rs}` in the engine: cached nodes as ahead renderers, warming, the write-behind tap, background freeze on the offline executor, the audit; the determinism probe for hosted plug-ins (with `cantus-host`) |
| **ui** | the cache switch and budget in settings, "not cacheable: …" in the plug-in manager, a quiet indicator of cached nodes in the mixer (with the design owner) |

**Tests that must hold:** cached = computed to the bit (the null corpus played from a cold
cache, then from a full one); an edit at bar 30 invalidates exactly its pages up to the
node's next settle point (counted); undo finds its pages; a `DSP_VERSION` change
invalidates all; a hosted test plug-in with a random seed fails the probe and is never
cached; a missing or corrupt page falls back to computing and is reported; no I/O on any
real-time thread (a file-system shim that counts calls per thread).

**Targets:** an unchanged re-export ≥ 50× real time; playing a fully cached 64-track session
costs ≤ 15 % of computing it; the audit runs clean through a round of real use before the
cache is on by default.

## Phase 6 — plug-in sandboxing (prototype any time; build after Phase 1; 3 builders)

ADR §6.

| Builder | Owns |
|---|---|
| **ipc-proto** | `docs/perf/proto/ipc/`: two processes, a shared mapping, spin-then-event; round-trip median and p99 at 64 and 128 frames with 1, 4 and 16 instances per group. Decides the default hosting mode before anything is built |
| **shm** | `cantus-rt/src/shm.rs` (Windows `CreateFileMapping`, POSIX `shm_open`/`mmap`), job/completion sequence cells, named events for parking; `loom` where it applies |
| **sandbox** | new binary crate `crates/cantus-sandbox`; `cantus-host`'s remote unit (an effect/instrument adapter that is an *external task*); hosting modes in the plug-in database and `views/plugins.rs`; crash notices with *Reload* |
| *(sched, small)* | external tasks in `sched/`: in-flight jobs polled by the scheduler loop |

**Tests that must hold:** a crashing test plug-in in *Together* leaves the engine playing
and the plug-ins of other groups running; a hanging one is late, counted, and its node
passes its input; sandboxed output = in-process output to the bit for the gain test
plug-in; no host rendering thread ever blocks on the sandbox (a sandbox that sleeps 50 ms).

**Targets:** round trip ≤ 5 % of a 64-sample budget per group with the sandbox spinning,
or the default stays *Within Cantus*; *Together* with 32 instances of the test plug-in
costs ≤ 1.2× in-process on `dawbench.dsp.b128`.

## Phase 7 — time-parallel export (research-gated, 1 builder)

ADR §7.3: a node renders segments that start at its settle points on separate cores, with
separate instances. Only after Phase 2 and the at-rest tests have held for a round.
**Test:** time-split export = serial export to the bit on the null corpus. **Target:**
songs with pauses export ≥ 2× faster than Phase 2.

## Calendar sketch (waves of builders, not dates)

| Wave | Builders in parallel |
|---|---|
| 1 | rt · at-rest · graph (P0) · sched (mock) · build · ipc-proto |
| 2 | graph + sched (P1) · kernel-1…3 · voices |
| 3 | export · graph (suspension) · kernel-4… |
| 4 | ring · ahead · ui-host |
| 5 | key · store · play · ui · shm · sandbox |
| 6 | time-parallel export; the published benchmark against the other DAWs (`benchmark.md` §5) |

Each wave ends with the integrator's merge, the full test suite at `W = 0` and `W > 0`,
`xtask perf` against the baseline, and STATUS updated with the numbers.

## Open questions for the owner

1. **Default anticipation level.** *Normal* (85 ms edit latency on nodes that are not
   promoted) is proposed; REAPER defaults to 200 ms. Lower is safer for feel, higher for
   capacity.
2. **The render cache's disk budget** (10 GiB proposed) and whether it is per user (shared
   across projects, proposed) or beside each project.
3. **Hosting mode default** for third-party plug-ins: *Together* if the IPC prototype meets
   its target, else *Within Cantus* (today's behaviour).
4. **Shipping two x86 binaries** (v3 and a v2 fallback) — the packaging builder's call on
   how the stub and the installer present it.
