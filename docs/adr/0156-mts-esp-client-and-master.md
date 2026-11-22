# ADR 0156 — MTS-ESP: Cantus follows a master, and is one

**Status:** Accepted (round 42; the integrator, with the owner's mandate of 2026-11-22) ·
**Date:** 2026-11-22 · **Builds on:** ADR 0005 (real time; unsafe code in `cantus-rt`), ADR
0155 (the project's tuning), ADR 0047 (licences)

## Context

MTS-ESP (ODDSound) is the tuning link between hosts and plug-ins: a master publishes a
128-note table through a shared library, `LIBMTS`, and every client reads it. Verified on
2026-11-22 (`research/instruments/microtuning-verified.md` §3): the client and master
wrappers are 0BSD; `LIBMTS` itself is published as binaries only (Windows
`Program Files\Common Files\MTS-ESP\LIBMTS.dll`, macOS `/Library/Application
Support/MTS-ESP/libMTS.dylib`, Linux `/usr/local/lib/libMTS.so`), installed by ODDSound's free
MTS-ESP Mini, Surge XT or Pianoteq; nothing reserves the master's role for ODDSound's paid
Suite (Surge XT is a master); a client reads the table without locking or allocating; no DAW
is a built-in master today.

## Decision

- **No library is shipped.** `cantus-rt::mts` loads `LIBMTS` at run time from its standard
  place, if present, on the main thread; without it the *External* tuning and *Be the
  master* are shown greyed with the reason ("MTS-ESP is not installed").
- **Client:** the project's `Tuning::system = External` reads the master's table; the engine
  refreshes each instrument's `TuningTable` from it once per block on the audio thread (the
  read is lock-free and allocates nothing — `rt_memory` holds it), and an offline render
  takes one table at its start (the master can change live; a render is a snapshot).
- **Master:** *Be the MTS-ESP master* (Project ▸ Tuning) registers Cantus and publishes the
  project's table whenever it changes, so hosted plug-ins that are clients (Surge XT, Vital,
  Pianoteq …) follow it; refused, with the reason, when another master is registered.
- **Our own instruments never need it** (ADR 0155 hands them the table); MTS-ESP is for
  hosted plug-ins and other programs.
- **Name:** the interface says "MTS-ESP" descriptively, with no logo (no trademark terms
  found; `research/…` §7.2).

## Consequences

- One unsafe module in `cantus-rt` (the loader and the C ABI), behind a safe API the engine
  uses (`ExternalTuning`); `cantus-engine` stays `forbid(unsafe_code)`.
- macOS: the app needs the *disable-library-validation* entitlement to load a library it did
  not sign (the release's signing step, when it exists, keeps it).
- What only a machine with MTS-ESP installed confirms goes in `docs/beta-checklist.md`
  (Ronda 42): Cantus as client of Surge XT's master, and Surge XT as Cantus's client.

## What the tests hold

The safe API against a fake library (a test `cdylib` built by the tests that exports the
same C symbols): register, publish, read, unregister, and a second master refused; the block
refresh allocates nothing; without the library every call is a no-op and the interface says
why.
