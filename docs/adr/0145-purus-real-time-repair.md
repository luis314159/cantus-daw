# ADR 0145 — Purus, the real-time repair unit

**Status:** proposed (fx side round wave C, provisional number — renumbered at merge; wave B
holds 0142–0144 in parallel, so the integrator renumbers this one after them) ·
**Date:** 2026-11-17 · **Schema:** 66 (provisional — 65 is wave B's; renumbered when merged
after round 40) · **Relates to:** ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0031 §5.1 (at rest), ADR 0071 (effects that sound on insert), ADR 0096 (the audio
editor's denoise and declick, whose cores Purus shares)

## Context

The audio editor repairs a take offline (ADR 0096). Nothing cleaned a track as it played:
mains hum and its harmonics, clicks and crackle, a room's tail on a voice (iZotope RX's
De-hum and Dialogue De-reverb, Waves X-Hum and NS1, Accusonus ERA, Acon DeVerberate —
function only). The DSP was built first (`docs/round-fx/purus.md`: notches −37 … −43 dB, a
gentle canceller with a phase-locked reference and a 50 ms look-ahead, de-click on the
editor's own detector, de-crackle by subtraction, a Habets-model de-reverb with a Direct
sound, every module's delta, exact rest, CPU ≤ 4.5 % of a core with all three).

## Decision

A new effect kind, `EffectKind::Repair`, model `cantus_core::fx::purus`, engine unit
`cantus_engine::fx::purus`, DSP `cantus_dsp::purus`.

- **Parameters** (27 keys): three modules, each with its switch, mix, output and delta —
  de-hum (`hmode` fixed / followed / found by itself, `hhz`, `hharm`, `hdepth`, `hwidth`,
  `hgentle`), de-click (`csens`, `crackle`), de-reverb (`vamount`, `vdecay` set / estimated,
  `vt60`, `vdirect`, `vsmooth`, `vkeep`) — and the unit's `out`. **Every default is a null:
  every module off**, the output the input to the bit and undelayed.
- **The de-click's sensitivity stops at 0.7** (`SENSITIVITY_MAX`): past it the repair harms
  the music more than the clicks do (the DSP report's finding, the editor's declick alike).
  The engine clamps a hand-edited file to it too.
- **The Direct sound's default is one constant**, `DIRECT_DB_DEFAULT` = +6 dB (dialogue
  first: a dry voice keeps its level within 1 dB); far-microphone presets set 0 dB. The
  owner may move it; the DSP's default and the table's are held equal by a test.
- **Latency** is the sum of the modules that are on — the gentle de-hum's 50 ms, the
  de-click's 767 samples, the de-reverb's `N − 1` (1 023 samples: 21.3 ms at 48 kHz,
  **23.2 ms at 44.1 kHz**) — read live each pass, so the engine's compensation follows a
  module switched on or off.
- **The de-click shares the audio editor's declick cores bit-identically** (ADR 0096): the
  model's fit, the robust scale, the threshold, the rebuilding, made allocation-free cores
  the editor's functions now wrap (hash-guarded in `purus_tests_shared.rs`).
- **Rest**: the DSP's drain, except while the de-reverb estimates and has learned a decay —
  what it learned of the room outlives silence (on purpose: each phrase does not start from
  the decay set), so a resting unit would not be as good as new; it does not rest then.
- **Starting preset** (ADR 0071): *Mains hum 50 Hz* — the de-hum on, followed, eight
  harmonics, the notches. Not a Tabula pedal until its face joins the board in phase B.
- **Telemetry**: the hum's fundamental and nominal, each harmonic's level, depth and hold,
  the decay in force and estimated, the clicks rebuilt, the peaks and what was taken out,
  and the de-reverb's 48 reduction bands in three pages.

## Consequences

- The editor (design sheet v1, `compartir/purus/design/v1`): a restoration bench — the
  spectrogram on dark glass with what is taken out in the residue's colour, the hum's
  needles, the clicks' lane, the room's tail before and after; three module cartridges
  (the de-hum's tuning fork engraved with the fundamental it follows). Its spectrogram's
  columns come from the slot's signal: in the sheet from the real unit over a demo take;
  live, the slot's scope (step 3, as Speculum's).
- Step 3 (after the owner's verdict): factory presets (ten or more), the editor's pointer
  on the screen (the fundamental, the comb's width, the reduction as a level), CLAP/VST3
  export reporting the latency as it changes, the AI's words, the manual, an e2e through
  `CantusApp::assemble` (a hummed track, the de-hum switched on, the hum gone from the
  master, the de-reverb's latency compensated).

## What the tests hold

`cantus-core` `fx::purus` (keys, the null, the cap, the constant); `cantus-engine`
`tests/fx_purus.rs` (the null at defaults; each module's latency at 48 and 44.1 kHz and the
telemetry's; the compensation; the starting preset takes a 50.3 Hz hum down > 25 dB; live =
offline; blocks 1/37/64/512; a fork; the cap and the constant), `fx_contract` (block-size
independence, split invariance, exact rest twins — heavy, estimating, never rests),
`rt_memory::purus_never_allocates`; `cantus-project` schema 66's fixture; `cantus-ui`
`fxc_whole` (every word whole, the bench's controls).
