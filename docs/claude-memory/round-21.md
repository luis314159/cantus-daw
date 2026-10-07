---
name: round-21
description: "Round 21 (started 2026-10-06): performance wave 1 (rt, at-rest, graph, sched, math) per ADR 0031 + plan.md's revised section, plus the asio fix (one build with every driver, cpal ASIO duplex); base r20/integration be216de"
metadata:
  node_type: memory
  type: project
  originSessionId: a47a4c80-267b-403d-a74f-bdab0153945d
  modified: 2026-10-06T16:20:14.635Z
---

Round 20 finished on 2026-10-06: `r20/integration` @be216de, 1 616 tests green, clippy clean,
builds in C:\projects\daw-r20\builds\ (cantus.exe, cantus-asio.exe). The user asked to push:
on 2026-10-06 C:\projects\daw master was fast-forwarded to be216de and pushed to origin.

Round 21 wave 1, worktrees C:\projects\daw-r21\<name>, base 49f02f3 (`r21/contract`, brief
docs/round21/BUILDING.md): rt (new crate cantus-rt: MMCSS promotion R1, topology, padding,
memory lock, FMA3 guard, denormals), at-rest (exact at_rest() for every unit + heavy contract
tests), graph (Phase 0: explicit graph on one thread, canonical gather, limits 64→256,
FTZ in process; sole owner of processor*.rs), sched (both queue designs vs mock graphs, PCT
fuzz), math (deterministic transcendentals, DSP_VERSION bump — merge last). Deferred to wave
2: build (x86-64-v3; disk). Also `r21/asio` (from r20/integration): the owner found the
default build had no ASIO and the ASIO build lost ASIO4ALL — fix: one build with every
driver, cpal ASIO duplex. Baseline: 10-track demo = 103 % of one core at 128 samples.

**How to apply:** merge order asio, rt, at-rest, sched, graph, math; then wave 2 (graph+sched
→ live pool, build, kernels, voices, dither, replay). See [[round-20]],
[[feedback-one-build-all-drivers]], [[cantus-parallel-builders]].
