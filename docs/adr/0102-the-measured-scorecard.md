# ADR 0102 — The measured scorecard

**Status:** accepted; first measurements taken (round 37, the integrator) · **Date:** 2026-11-06 ·
**Schema:** none · **Relates to:** BACKLOG-excellence §0 (the scorecard) and §0.1, ADR 0031
(performance), `docs/perf/benchmark.md` (how Cantus is measured), ADR 0020 (our effects as
CLAP/VST3)

## Context

The project's final goal is to be *undeniably and measurably* the best DAW (BACKLOG §0). Until
round 37 the claim had no instrument: the performance suite (`cantus-bench`) measured Cantus
against its own past, and nothing measured Cantus against another DAW. Round 37's brief asked
for the scorecard's first measured form: the feature matrix, and repeatable measurements of
start-up and DAWbench-style capacity against FL Studio and REAPER where they are installed —
and, where they are not, the harness left ready and the owner told what to install.

Two traps shaped the decision:

- **A comparison made only with our own tools proves little.** A probe inside Cantus reports
  what Cantus thinks; it cannot be put inside FL Studio. A cross-DAW figure must be taken from
  *outside* each program, by the same rule.
- **Vendor claims are not measurements.** A feature cell must rest on the vendor's current
  documentation (cited, versioned) and "better" must never be claimed without a number taken
  on one machine, the same day, by the same method.

## Decision

1. **The feature dimension** is a matrix of 97 rows (`docs/scorecard/rows.md`) over Cantus and
   twelve DAWs (eight commercial leaders and four free ones), every non-Cantus cell sourced
   from the vendor's current pages with its version and URL (`docs/scorecard/matrix/`), a cell
   no page confirmed marked `unverified`. Cantus's cells cite the repository. It is re-run each
   release; `docs/scorecard/features.md` lists the gaps.
2. **Start-up is measured from outside** by `tools/scorecard/startup.ps1`, the same script for
   every program: spawn → the first visible top-level window whose title matches answers a
   message (*window*); spawn → the process has used under 10 % of one core for 1.5 s after that
   (*settled*); private bytes and working set at *settled*. One unmeasured warm-up start, then
   five measured ones, each closed politely; the median and the best are reported.
3. **Capacity is DAWbench's method** (`docs/perf/benchmark.md` §3–§5): the same session — one
   held source and eight compressor instances per track — rebuilt in each host, with our
   compressor as the bit-identical CLAP/VST3 build in the others (variant A: the DSP is the
   same, so the difference is the host) and one free third-party compressor everywhere
   (variant B). The **published** figure is the live one (the host's own dropout counter, 60 s
   clean, three passes, the median); Cantus's offline estimate (`cantus-bench --only dawbench`)
   tracks the trend between releases and is labelled as an estimate wherever it is shown.
4. **Results are published with their method and their conditions** (machine, versions,
   driver, buffer, date) in `docs/scorecard/README.md` and on the website's scorecard page, or
   not at all. A row where Cantus loses is published as plainly as one where it wins.
5. **What only the owner can run** — a live DAWbench in FL Studio and REAPER with a real
   interface, a program that is not installed — goes in `docs/beta-checklist.md` with the exact
   steps, and the published table says which cells are still empty and why.

## Consequences

- The first measured rows exist: start-up for Cantus and FL Studio on the owner's machine, and
  Cantus's capacity estimate (`docs/scorecard/README.md`). REAPER is not installed on the
  machine, and no live DAWbench has been run in another host yet: those cells are empty and
  say so.
- The scorecard is a living document: every release re-runs the start-up script and the
  estimate; the matrix is re-checked when a leader ships a new version.
- The harness never loads anything inside the other programs and never changes their files;
  it only starts, watches and closes them.

## What the tests hold

- `cantus-bench`'s `dawbench` and `startup` suites run in the full performance run
  (`cargo run --release -p cantus-bench -- --only dawbench,startup`); the gate (`xtask perf`)
  compares them with the stored baseline.
- `tools/scorecard/startup.ps1` is a script, not a test: its rule is written in its header,
  and every result it writes carries the program's version and the raw per-start rows, so a
  published median can be checked against the starts it came from.
