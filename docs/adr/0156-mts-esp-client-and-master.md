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
- **Master:** *Be the MTS-ESP master* (Options ▸ Tuning…) registers Cantus and publishes the
  project's table whenever it changes, so hosted plug-ins that are clients (Surge XT, Vital,
  Pianoteq …) follow it; refused, with the reason, when another master is registered.
- **Our own instruments never need it** (ADR 0155 hands them the table); MTS-ESP is for
  hosted plug-ins and other programs.
- **A hosted plug-in is never tuned twice** (the mid-round review, correctness 1): an MTS-ESP
  client (Surge XT, Vital, Pianoteq …) retunes itself from the master, so Cantus sends it
  per-note tuning (ADR 0155 §4) only for what MTS-ESP does not give it — its track's table less
  the table MTS-ESP holds (the outside master's, or the project's while Cantus is the master),
  key by key, exactly `0.0` where they agree: nothing at all for a track that follows the master
  or the project Cantus publishes; the difference for a track with a tuning of its own. Without
  a master the whole table goes, as before. Per plug-in, **Tuning by** (the instrument's
  inspector row; `ExternalPlugin::tuning_by`, schema 80's, not written at its default,
  `SetInstrumentTuningBy` with its inverse and undo-fuzz arm): *MTS-ESP* (the default) or *Note
  expression* — the whole table per note, for a plug-in that does not read MTS-ESP. The live
  engine asks the link each block (`ExternalTuning::leads` and `read`, lock-free); an offline
  render takes what MTS-ESP held at its compile (`Snapshot::mts_heard`).
- **A master's change recompiles** (review correctness 2, 7, 8): each sync, a song that plays
  the master's tuning anywhere reads the master's table on the interface thread
  (`views::tuning::master_moved`); a new table, or the master coming or going, sends a fresh
  snapshot — so Chorda's design (made off the audio thread), the strips' and the master's
  pitched effects and every offline copy follow, and a track whose master went back plays the
  table it was compiled with (the live block refresh does the same at once). The boot hands the
  link to the engine and compiles again; an audio restart hands it to the new engine
  (`CantusApp::adopt_engine`).
- **MTS SysEx to hardware** (ADR 0155 §4) follows the same table: an `External` track sends the
  master's table (again when it changes); an instrument switched off MTS, disabled or removed is
  sent 12-TET at A440 once; a route whose port is not open yet keeps its messages (bounded) until
  it opens.
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

End to end on `CantusApp::assemble` with the fake library (`cantus-ui/tests/tuning_mts_e2e.rs`,
r42 fix-engine): a song in the master's tuning exports at its table, recompiles when the master
changes it (the live snapshot holds the new table), still follows after the engine is replaced,
and plays equal temperament when the master goes; as the master, Cantus publishes the project's
table and republishes an edit, the hosted CLAP test synth at *Tuning by: MTS-ESP* is sent no
per-note tuning and at *Note expression* the whole table. What only real MTS-ESP clients confirm
(Surge XT not tuned twice) is the checklist's.
