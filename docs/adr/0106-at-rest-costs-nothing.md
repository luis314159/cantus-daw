# ADR 0106 — At rest, Cantus costs nothing: the desk, the dancer and the idle threads

**Status:** accepted (round 38, the scorecard builder) · **Date:** 2026-11-08 · **Schema:**
none · **Amends:** ADR 0031 §5.1 (rest generalised from pad chains to every strip, the
master chain and Spatium), ADR 0059 §6 (Chorda's reference keyboard no longer designed at
start), ADR 0091 (the stage's idle breath stops once nothing happens), ADR 0102 (how the
start-up rows are measured) · **Relates to:** ADR 0024 §13 (the repaint ticker), ADR 0100
§6 (a pad's chain at rest)

## Context

Round 37's first measured scorecard (`docs/scorecard/README.md`) left Cantus losing three
start-up rows to FL Studio and REAPER. Traced this round from outside the process — a
sampling profiler that suspends each thread and walks its stack with the program's PDB
(`tools/scorecard/profile.ps1`), and the per-thread CPU times — on the beta.12 build:

- **At rest, 32.8 % of a core** on the demo Cantus opens with: the audio callback 13.9
  points (every strip with an insert, the master chain and the master's loudness meter
  ran over silence: Spatium, Sibilus, Multiplex, Culmen, Terminus, the K-weighting and the
  4× true peak), the graphics driver's thread 11.3 and the main thread 6.1 (the dancer's
  breath: twelve full-window frames a second).
- **The quiet tail** (6–42 s after the window): Chorda's reference keyboard designed on ten
  threads in the 3 s after the window (≈ 7.5 s of CPU for a piano the demo does not have),
  then the at-rest load itself: 0.29–0.33 of a core against the 0.35 threshold, so "1.5 s
  in a row under 0.35" fell at random — the long tail was the at-rest row seen through a
  threshold, not start-up work.
- Opened on the *Beat* starter, **44.5 %**: the mixer asked egui for a repaint in 16 ms
  directly, which egui shortens by its predicted frame (round 37's finding), so the window
  drew at the screen's 144 Hz while a meter moved; the effect editors, the pinned inspector
  meter and the dancer editor did the same.
- On an empty song, 2.0 %: a MIDI-out thread that slept 1 ms a thousand times a second with
  no port to serve (1.1 points) and a device watcher that listed every audio endpoint every
  1.5 s (0.5 points).

## Decision

1. **Every strip with inserts rests in silence** (ADR 0031 §5.1, generalised; pad chains
   were the first, ADR 0100 §6). A strip rests when nothing came in, its EQ and lines are
   quiet, **no latent insert's dry line holds signal**, nobody watches it (analyser, scope,
   the recorder on it), no insert hears notes, every insert that runs hears a silent key —
   its key strip left none this pass, which is what it would read — and every insert that
   runs is at rest; its inserts are told `skip` — and an insert that follows the song is
   told the position first, as a running one is (Fluctus and Translatio otherwise read the
   next pass as a seek: `listen.rs`'s block-of-1 test found it). `EngineStats::strip_rests` counts it
   (`chain_rests` still counts the pad chains among them).
2. **The master chain rests** on the same rule when the summed mix is exact silence
   (`EngineStats::master_rests`), and **the master's loudness meter** takes exact silence
   through clear filters as a count of columns (`LoudnessMeter::silence`): the readings are
   the same to the bit.
3. **Spatium rests** (`FxUnit::at_rest` for `EffectKind::Space`): every late line, all-pass,
   absorption and pre-delay state +0.0 (counted with `rest::ZeroRun` after each block, never
   scanned when asked), every glide landed, no freeze, ducking at zero, the wet EQ at rest;
   the spring style keeps running. `skip` moves the rings and the late field's modulation
   exactly as ticks would (one `lfo_step` for both). The network's writes add `+ 0.0`, which
   turns the −0.0 its signs and Hadamard make of silence into +0.0 and leaves every other
   value as it is — that −0.0 read back as +0.0 already, so no output changes. Default
   settings rest after ≈ 10 s of silence.
4. **Exact, held by twins**: `resting_changes_no_sample_of_the_demo` plays the demo through
   silence, play, stop, the tails dying away and play again on a processor that rests and on
   one that never does (`Processor::set_never_rest`): the same bits; `fx_contract`'s twin
   test now requires Space to rest at its defaults; `rest_blockers()` names what keeps an
   insert running.
5. **The dancer holds still once nothing happens** (ADR 0091 amended). Stopped, a dancer
   breathes for [`dance_live::DOZE_AFTER`] (10 s) after the last thing that happened — the
   pointer over the window, a key, Play, a drag, a hover on a dancer, or a note on a
   dancer's track (a key held or a new move-map note, played live from a MIDI keyboard:
   round 38's review) — then holds its pose,
   and the window asks for no frame for it; the next thing that happens wakes it from the
   pose it held (the breath's clock stops while it holds, `dance_live::Doze`). Not while the
   companion window is open: what it streams keeps breathing. A dancer the user adds keeps this rule; at start
   there is none (item 9).
6. **Nothing asks egui for a frame directly at the screen's rate**: the mixer's meters, the
   effect editors, the pinned inspector meter and the dancer editor go through the ticker
   (≤ 60 Hz, none when idle).
7. **Chorda's reference keyboard is designed on intent** (ADR 0059 §6 amended): not at
   start, not after a device change; a project with Chorda asks for its tunings with its
   first snapshot (as before), and opening the instrument picker asks for the reference
   keyboard, so it is ready by the time the piano is chosen (its design takes ≈ 1.6 s on
   ten threads).
8. **Idle threads sleep**: the MIDI-out thread looks every 20 ms when it has no port and no
   external instrument to serve (1 ms when it has); the device watcher waits for Windows to
   say its audio endpoints changed (`cantus_rt::endpoints`: `RegNotifyChangeKeyValue` on
   `MMDevices\Audio`), lists them again 150 ms after a change and at least every 10 s,
   and polls every 1.5 s as before where it cannot watch.
9. **Nothing dances at start** (the owner's decision, 2026-11-08: *el dancer por default
   cada vez me gusta menos, quítalo*). The song Cantus opens on when it is given nothing —
   the house demo's second drop — is built without its dancer (`demos::startup_project`);
   the whole song (*File ▸ Demo songs*) keeps Trino, and `startup_project_with_dancer` gives
   the excerpt with it (the stage's shots). The welcome song never had one, the companion
   window was already closed by default, and the feature is whole: the instrument picker,
   the palette and a saved song with a dancer bring it back as before. No setting or
   schema changes. Item 5's hold still applies to a dancer the user adds.
10. **What keeps the window drawing is written down when asked**: `CANTUS_REPAINT_LOG=<file>`
   logs every frame's events, the ticker's reasons and egui's repaint causes.

## Consequences

- Measured on the same machine as round 37 (`docs/round38/scorecard.md`, the README's
  table): see there for the numbers before and after, Cantus on its demo and on an empty
  song, FL Studio and REAPER again by the same script.
- A strip's meter, gain-reduction readout and telemetry stop moving while it rests (they
  read silence); an analyser or scope on an insert keeps its strip running.
- The first time the piano is chosen in a session without the picker (the palette, an
  agent, a template), it is silent for the design's ≈ 1.6 s, with the editor's *designing
  the strings* caption — the behaviour every non-reference tuning already had.
- A device plugged in shows within ≈ 150 ms where the registry is watched, and within
  10 s on a system whose endpoints change without touching it (to be confirmed on real
  hardware: `docs/beta-checklist.md`).

## What the tests hold

- `crates/cantus-ui/tests/at_rest_e2e.rs` (through `CantusApp::assemble` on a real
  processor): every chain of the demo and its master rest every pass in silence, and no
  insert is left running (`rest_blockers`); resting changes no sample of the demo through
  silence, play, stop and play again; the dancer holds still and the window then asks for
  no frame, and the pointer wakes it from the pose it held.
- `crates/cantus-ui/tests/inst_chorda_boot.rs`: the boot path designs no keyboard; an
  effect's picker asks for none; the instrument picker does.
- `crates/cantus-engine/tests/fx_contract.rs`: Space must rest at its defaults; its twins
  (processing zeros, skipped, reset, new) agree to the bit when signal returns.
- `cantus-dsp`: `silence_through_a_clear_meter_is_counted_exactly` (the meter's fast path
  against every sample through the filters, at three block sizes); `dance_live`'s
  `it_holds_still_after_a_while_and_wakes_where_it_held`; `cantus-rt`'s endpoint watch
  times out on a quiet system.
- `crates/cantus-ui/tests/memory_e2e.rs` prints what a start holds, step by step, and what
  the per-slot parts hold (the memory row's map, `docs/round38/scorecard.md`).

## Found by round 38's review, and fixed at integration

- A dancer played live from a MIDI keyboard while stopped froze once it held still: notes
  wake the window but were not *something happening*. A note on a dancer's feed now wakes
  it (`a_note_played_live_wakes_a_dancer_that_holds_still`, through `assemble`).
- The import re-check was asked on two clocks (the breath's, standing still, and the wall's),
  so an imported character was re-asked of the worker every frame while its card showed; it
  runs on the wall clock alone.
- The dancer editor's stage card asked for 60 frames a second while its dancer held still;
  it asks for none then.
- The endpoint watch is armed again as soon as it fires (a value written while the watcher
  settles and lists the devices signals its next wait, not the 10 s rescan), and a watch the
  registry gives up falls back to the 1.5 s poll instead of a rescan every 0.4 s.
- Two tests could not fail on what they backed: `memory_e2e::what_a_start_holds` now bounds
  each step of a start about a tenth above round 38's measure (its two tests run one at a
  time, as the count is the binary's), and
  `at_rest_e2e::a_knob_turned_while_the_desk_rests_changes_nothing_heard` turns a knob on
  every chain while the desk rests and holds the result to a twin that never rests, to the
  bit.
