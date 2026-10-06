# Round 21 — performance, wave 1: the brief every builder reads first

**The ask (the owner, 2026-10-06):** *after round 20, launch the new round with the
optimisation backlog — the multicore engine first.* And: *"I want the audio engine to be the
very best; take the best ideas from today's engines and even from studies and papers; let's
make something excellent"* (Colossians 3:23).

**Why it matters now:** the round-20 bench measured the 10-track demo at **103 % of one core
at 128 samples** — the single-threaded engine cannot play our own demos at a low buffer. The
engine also caps tracks and strips at 64 without saying so, and on Windows (WASAPI) the
audio callback runs at **normal** priority.

Read, in order: this file; `docs/adr/0031-performance-architecture.md` **including its last
section** (the review against the state of the art, amendments R1–R16);
`docs/perf/plan.md` — **its last section supersedes the order and calendar above it**;
`docs/perf/baseline.md` and `docs/perf/benchmark.md` (the numbers and how to measure);
`research/engine/README.md`; ADR 0005 (real-time rules); and your section below.

## Rules (from the plan, plus the round's)

1. **One owner per hot file.** `processor.rs`, `processor_*.rs` and the new `graph/` files
   belong to the *graph* builder alone. Anyone else needing a change there writes it in
   `docs/round21/requests.md` on their own branch and tells the integrator in the report.
2. **Bits first.** Every test in `crates/cantus-engine/tests` passes; the `v5.cantus` hash in
   `null_tests.rs` is unchanged. A change of output bits is on purpose, in its own commit,
   with the reason — and only the *math* builder's planned `DSP_VERSION` bump is expected
   this wave.
3. **Nothing allocates on a rendering thread** (`rt_memory.rs`).
4. **Measure before and after** with `cargo run -p xtask -- perf --pin <core>` on this
   machine (quiet now); put the JSON numbers in your report.
5. Your worktree and branch only (`C:\projects\daw-r21\<name>`, `r21/<name>`); never touch
   `C:\projects\daw`; never push or merge. WIP commits early and often. Files in pieces under
   ~300 lines per tool call. Bash: a heredoc delimited by `EOF` fails — use `XEOF` or the
   Write tool. The corporate proxy (Zscaler) may quarantine downloads ~10 min — never bypass
   it. `.cargo/config.toml` (untracked) keeps `jobs = 4`.
6. **Finish with a report** (done / measurements / tests / files outside your area / left /
   notes for the integrator) and `docs/round21/<name>.md`.

## Wave 1 — who owns what

| Builder | Branch | Owns (from `docs/perf/plan.md` Phase 0 + the review's additions) |
|---|---|---|
| **rt** | `r21/rt` | new `crates/cantus-rt/` — `thread.rs` (`promote_callback()`: MMCSS "Pro Audio" + high priority on WASAPI, nothing on ASIO; EcoQoS opt-out; macOS time constraint before workgroup join; Linux rtkit/FIFO), `topology.rs` (P/E classes and last-level-cache domains), `pad.rs` (128-byte), `memory.rs` (pre-fault, best-effort lock), `fpenv.rs` (`_set_FMA3_enable(0)` interim; the denormal guard and its re-assert), `denormal.rs` (moved from `cantus-plugins/src/ftz.rs`, which becomes a re-export), `examples/rt_probe.rs`; Miri where it applies. **Also the R1 quick win, by request to graph:** the callback's first block after a stream start calls `promote_callback()` — write the request; graph wires it. |
| **at-rest** | `r21/at-rest` | `FxUnit::at_rest`, `InstrumentUnit::at_rest`, `EffectUnit::at_rest` (exact: "state equals a fresh reset", default `false`), every native implementation and the DSP state it reads; extended contract tests (heavy settings, random split points, blocks 1–4096) in `fx_contract.rs`, `inst_contract.rs` + `tests/support/heavy.rs`. **Merges before graph.** |
| **graph** | `r21/graph` | Phase 0: `processor.rs` → control phase + glue; `graph/{mod,plan,gather,track,strip,master,notes}.rs`; per-node segments (tracks split at their own events, strips and master whole blocks); the canonical gather order (parallel will equal serial bit for bit); note outboxes; FTZ inside `process`; limits 64 → 256 (`tests/limits.rs`); the plan compiled on the UI thread only when the routing changes, sent with `SetSnapshot`; `render.rs`/`stems*.rs` send the plan; the bench's `profile.rs` keeps charging nodes; R1's `promote_callback()` call once rt lands (until then, a call behind a small shim you own). Tests: everything unchanged + `graph_order.rs` (proptest topologies, gather = old scatter to the bit), `limits.rs`, `ftz.rs`. **Target:** `engine.s64.b128` ≤ 0.9× and `engine.s100.*` plays all 100 tracks. |
| **sched** | `r21/sched` | `crates/cantus-engine/src/sched/{mod,queue,pool,fuzz}.rs` against a `trait Graph` and mock graphs only: **both** the bounded Vyukov queues and per-worker fixed-capacity Chase–Lev deques with home-worker seeding behind one trait (R2); spin/yield/park with a time budget, park after silence (amended ADR 0005 rule 3), the continuation slot, two priorities (critical path first); `tests/sched.rs` with a stall injected between claim and publish; the PCT-shaped fuzz with replay tokens, each seed run twice comparing the final RNG state; `buggify!` under a `fuzz` feature; measure both queues and recommend one. Never touches `processor*.rs`. |
| **math** | `r21/math` | R9: one deterministic transcendental module (`cantus-dsp/src/math.rs` — pinned `libm` or own polynomials, justified by accuracy and speed) used by every `cantus-dsp` unit; `clippy.toml` `disallowed-methods` for std's `sin, cos, tan, exp, ln, log*, powf, tanh` in `cantus-dsp` (and the engine's DSP paths, listing any it cannot reach without touching `processor*.rs`); `DSP_VERSION` in `cantus-dsp/src/lib.rs`, bumped once, goldens and the `v5.cantus` hash updated **in one commit** with the reason; no approximate reciprocals. Also R10 if time allows: counter-based TPDF dither in Terminus's `Ditherer` (used by Culmen) so `at_rest` can hold. |
| **build** *(wave 2 — two extra release builds; deferred for disk space)* | `r21/build` | Phase 3: `xtask` release builds at `x86-64-v3` with an `x86-64-v2` fallback (`--v4` behind a flag), a tiny `launch` binary beside `crates/cantus-app` that picks one at start (coordinate with packaging's `xtask/src/package.rs` — keep its file names), a CI job comparing null-test hashes of v2 and v3; measure v3 vs v2 on `engine.s64.b128`. |

Also running: **asio** (`r21/asio`) — one build with every driver and the cpal ASIO duplex fix; it owns `devices.rs` and `EngineHandle::start`'s stream setup, so graph keeps its `backend.rs` edits to the plan-sending code.

Merge order planned by the integrator: asio, rt, at-rest, sched, graph (rebased on at-rest
's contract tests), then math last (its planned bit bump re-blesses the hashes once). The next wave (graph + sched → the live
pool) starts from that merge.
