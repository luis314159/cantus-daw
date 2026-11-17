# ADR 0146 — Comparatio, the reference comparator

**Status:** proposed (fx side round wave C, provisional number — renumbered at merge; wave B
holds 0142–0144 in parallel) · **Date:** 2026-11-17 · **Schema:** 66 (provisional — 65 is
wave B's; renumbered when merged after round 40) · **Relates to:** ADR 0005 (the audio
thread), ADR 0022 (memory built off the audio thread), ADR 0052 (the master's loudness
meter, whose code Comparatio measures with), ADR 0112 (Aula's response by path), ADR 0137
(Arca's cabinet by path)

## Context

Mastering engineers compare a mix with a finished record at the same loudness, by ear and
by measure, and test their ears blind (ADPTR Metric AB, Mastering The Mix's REFERENCE,
Melda's MCompare — function only). The DSP was built first (`docs/round-fx/comparatio.md`:
Terminus's own loudness meter, EBU Tech 3341/3342 cases within 0.03 LU, the match within
0.06 LU, an eased equal-power switch at −94 dBFS, ABX and blind preference with an exact
binomial p).

## Decision

A new effect kind, `EffectKind::Reference`, model `cantus_core::fx::comparatio`, engine
unit `cantus_engine::fx::comparatio`, store `cantus_engine::fx::comparatio_store`, DSP
`cantus_dsp::comparatio`.

- **Parameters** (12 keys): `listen` (your mix, the reference, X hidden, the blind
  preference's One and Two), `match` (integrated, short term, by hand), `offset`, `refl`
  (the reference's loudness over what has played or its whole file), `fade`, `sync` (its
  own playhead, locked to the song with `soff`, a loop `lstart` … `lend`), `seed` and
  `trial` (the blind order), `res` (the display's). **The default is a null: the mix heard,
  to the bit** — and with no reference loaded, whatever is set.
- **The reference is a file by path** in `EffectSpec::ir`, set with `SetEffectIr` (the
  command Aula and Arca use; `takes_ir` is true for the kind), never copied into the
  project. It is read **off the audio thread**: decoded and resampled to the engine's rate
  with the audio clips' decoders and sinc (`samples::decode_audio`) on a thread of its own
  when live, at once when offline; split into two channels; its whole file analysed
  (`analyse_whole`: the integrated loudness for the whole-file match; the readings,
  histogram and tonal curve for the panel). Until it is in, the compiled snapshot names no
  reference (the mix plays); its arrival asks for a compile (`SampleCache::poll_loaded`).
  The unit asks for it through the memory path (`MemoryNeed::Comparatio`) and holds an
  `Arc` the store has a sibling of, so letting one go on the audio thread never frees.
  References longer than 20 minutes are cut there; at most three unheld are kept.
- **It is a monitoring unit, and a render never hears the reference.** Only the live
  engine's own answer (`EffectMemory::build_live`, called by `EngineHandle`'s poll) hands a
  unit its reference; every render, freeze, stem and range render answers its units' needs
  with `EffectMemory::build`, which gives none, and the compile for a render
  (`compile_as(render = true)`) also takes every reference away and sets `listen` to the
  mix. A test renders a Comparatio set to the reference, with its reference in the store,
  on a strip and on the master (blind X), and gets the dry project to the bit.
- **Its place**: an ordinary insert, offered on any strip and meant for the master's last
  slot (what the master sends out is what is compared). A fixed post-chain master slot (the
  DSP report's sketch) was not built: an insert keeps every existing path (the chain's
  order, bypass, the picker, the plug-in export) and the render guarantee holds wherever it
  sits. The master's meters read what is heard, the reference included — the point of
  monitoring.
- **A swap or a removal while the reference is heard** holds the DSP on the mix until it
  hears the mix (the crossfade), then swaps: no cut. A blind trial in progress is left by
  the same fade.
- **Transport**: the song's real samples (`project_samples`), so a locked reference follows
  the song in time through tempo changes.
- **Telemetry**: what is heard and the fade, the match gain and its target, the playhead and
  length, the whole-file loudness, both sources' 13 readings, both sources' stereo bands,
  and the two short-term histograms in ten pages. The tonal analyses are the interface's:
  the reference's from its whole file (the store), your mix's from the slot's signal
  (step 3: the slot's scope, as Speculum's).
- **No starting preset**: it starts at its defaults (it measures, `Neutral::Measures`).
- **Not a Tabula pedal**, ever: a reference is the master's monitoring, not a pedal's sound.

## Consequences

- The editor (design sheet v1, `compartir/comparatio/design/v1`): a two-deck console — your
  mix's deck and the reference's, the screen between (tonal balance and the difference,
  dynamics, stereo), the reference's strip, the big selector, the blind drawer.
- **The CLAP/VST3 plug-in (step 3)**: the reference's path in the plug-in's state (with the
  file's size and a hash; a missing file reported, never skipped silently; the audio never
  in the state), a *Load reference…* key in its editor (a native file dialog on the
  plug-in's UI thread) and a drop onto the editor; decoding and resampling on a plug-in
  background thread at the host's rate from `activate`; locked sync on the host's
  transport. The plug-in host answers the unit's need as the live engine does.
- Step 3 also: factory presets (ways of comparing), the reference list (several references
  in the project, one selected), the mix's tonal curve from the scope, the match-EQ
  suggestion sent to Aequalis, the AI's words, the manual, an e2e through
  `CantusApp::assemble` (a WAV the test writes loaded as the reference, heard on the master
  at the mix's integrated loudness within 0.1 LU, the mix back to the bit, an export
  without it).

## What the tests hold

`cantus-core` `fx::comparatio` (keys, the null) and an undo-fuzz arm for its reference;
`cantus-engine` `tests/fx_comparatio.rs` (the null with and without a reference asked for;
heard at the mix's loudness and back to the bit; a swap fades through the mix; no render
holds the reference, strip and master; blocks 1/37/64/512/4096 and a fork to the bit; a WAV
read and resampled through the store, its whole-file loudness, a missing file reported;
live = offline), `fx_contract`, `rt_memory::comparatio_never_allocates` (heard, matched,
blind, locked, looped, swapped while heard, taken away); `cantus-project` schema 66's
fixture (a reference by path); `cantus-ui` `fxc_whole`.
