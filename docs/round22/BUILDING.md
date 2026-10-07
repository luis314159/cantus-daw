# Round 22 — the multicore live pool, and the first beta-hardening work

**The owner's goal:** the first public beta (`docs/BETA.md`) — *"the best DAW in the world,
made with excellence"* (Colossians 3:23). This round closes the performance blocker (the
demos must play without dropouts at a low buffer) and starts the hardening items that do
not touch the engine.

Read, in order: this file; `docs/STATUS.md` (top: "what round 21 left", Round 21);
`docs/BETA.md`; your section below and the documents it names. Rules as in rounds 20–21:
your worktree and branch only (`C:\projects\daw-r22\<name>`, `r22/<name>`), never
`C:\projects\daw`, never push or merge; WIP commits early and often; files in pieces under
~300 lines per tool call; Bash heredocs not delimited by `EOF` (use `XEOF` or the Write
tool); the Zscaler proxy is never bypassed; tests first; `cargo clippy --tests` clean; kit
only in views; strings in EN and ES; finish with `docs/round22/<name>.md` and a report
(done / measurements / tests / files outside your area / left / notes for the integrator).
**Machine:** several builders share it — measure in paired A/B runs on a pinned P-core and
say so; the ASIO SDK is cached at
`%LOCALAPPDATA%\cantus-build-tools\asiosdk-2.3.4\ASIOSDK` (`cargo run -p xtask -- asio-sdk`
prints the variables), and without it the app still builds as a developer build.

| Builder | Branch | Owns |
|---|---|---|
| **pool** | `r22/pool` | The multicore live engine (`docs/perf/plan.md` Phase 1 as amended; ADR 0031 §2 and its review R2, R3, R5, R13): the graph implements `sched::Graph` on its plan; per-node state behind safe cells, per-worker scratch; Chase–Lev deques (recommended by sched) with critical-path ranks and clustering of light nodes; the serial fallback with hysteresis; workers promoted with `cantus_rt::promote_worker(.., Waiting::Bounded)` after a bounded spin (≈ half a period) then park — **never spinning inside MMCSS**; count a worker only once it has promoted itself; the pool started by the backend with a worker count from settings (default Auto = P-cores − 2); `EngineStats::{wakes, schedule_conflicts}` and the profiler per worker with critical-path blame; every engine test at W = 0 and W ∈ {1, 3, 7}; the schedule fuzz over the null corpus; `rt_memory` on every pool thread. **Targets:** `engine.s64.b128` ≤ 0.45× median and ≤ 0.6× p99 against the baseline on the P-cores; the startup song and every demo with zero overruns at 256 frames. Sole owner of `processor*.rs`, `graph/` and `sched/`. |
| **build** | `r22/build` | The x86-64-v3 release with an x86-64-v2 fallback (`--v4` behind a flag), a tiny `launch` stub that picks one at start, packaged by `xtask package` (keep its single Windows edition and file names), a CI job comparing the null-test hashes of the v2 and v3 builds; measure v3 vs v2. |
| **math2** | `r22/math2` | The f32 `powf` fast path in `cantus_dsp::math` (`exp2(y·log2 x)` in f64 for x > 0, or better — measure ulp and ns) for the per-sample callers (Porta, Sibilus, the Terminus/Culmen meters); R10 counter-based TPDF dither in Terminus's `Ditherer` (used by Culmen) so they can rest with dither on; Aequalis's dynamic residual fix (at-rest's finding 3); **one** `DSP_VERSION` 3 bump re-blessing the goldens in one commit with the measured size of the change. Never touches `processor*.rs`. |
| **export** | `r22/export` | Export formats beyond WAV — **MP3** (choose a licence-compatible encoder: LAME is LGPL, fine for a GPL app; or a pure-Rust one if one is good enough — measure quality), **FLAC**, **Ogg Vorbis**: bit depth, sample rate, quality/bitrate, tags (title, artist, album, year, the Cantus credit), loudness-normalised export and presets (Streaming −14 LUFS, CD, stems for a mastering engineer); the export dialog in the kit; stems in every format. The render itself (`render.rs`, `stems*.rs`) belongs to *pool* this round — encode what the renderer produces, through a new module/crate, and ask pool for any hook you need. |
| **beta-docs** | `r22/beta-docs` | `THIRD-PARTY-NOTICES` (every crate's licence via `cargo about` or equivalent, the patched cpal/asio-sys, the Steinberg ASIO SDK and its trademark notice, libm, egui and fonts, the CC0 packs, the verse translations); a privacy statement (no telemetry; exactly what the update check sends); both linked from About (redesign the stock About window in the kit); a quick-start manual in English and Spanish (`docs/manual/`): install, audio setup, record, edit, mix, master, export, shortcuts, known issues — accurate to the current app (read the code, render the screens you describe); GitHub issue templates (bug, hardware report, feature); release notes draft `docs/releases/v0.1.0-beta.1.md`; the version bump to `0.1.0-beta.1` (no tag). |
| **soak** | `r22/soak` | Stability: an automated soak harness (a long session — the demos looped, edits, recordings, plug-in windows — for N minutes, watching memory, handles, frame time, overruns); crash-recovery tests that kill the process mid-edit and mid-recording and prove autosave restores the project and a recording's WAV survives (header flushed); a corrupt or newer project refused cleanly; fix what they find in the app/UI/project crates (not `processor*.rs` — report engine findings to pool). |
