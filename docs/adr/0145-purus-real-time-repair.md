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
  harmonics, the notches.
- **Telemetry**: the hum's fundamental and nominal, each harmonic's level, depth and hold,
  the decay in force and estimated, the clicks rebuilt, the peaks and what was taken out,
  and the de-reverb's 48 reduction bands in three pages.

## Decision, phase B (step 3)

- **The live spectrogram reads the slot's analyser tap**, the (input, output) pairs every
  open editor already gets — not a scope ring of its own: the scope (ADR 0056) carries the
  slot's output only, and what was taken out needs the input too. The editor reads the input
  **as late as the unit's latency** (from the telemetry) against the output, cuts a column
  every 40 ms on its own thread (an FFT of 2 048 with the audio editor's window and scale,
  folded into 84 logarithmic rows), and draws what fell out of a row where it is 6 dB or
  more. The audio thread does nothing it did not do before. The design sheet's demo runs the
  real unit through the same path, so the sheet draws what the app draws; the plug-in's
  window reads its own tap the same way.
- **The screen answers the pointer**: the comb's needles (the fundamental follows the needle
  held, to a tenth of a hertz, 40–70 Hz; none while the hum module is off) and the room's
  tail (the reduction to the depth under the pointer, by half decibels; the mark where the
  early reflections end, 5–200 ms; none while the room module is off). Each drag one undo
  step. The comb's *width* stays on its card: at 0.5–20 Hz it is a few pixels on a rail
  three decades tall, too fine to drag honestly. The bench's keys and toggles offer
  automation and MIDI learn (`stepped_offer`; the 50/60 Hz and Set/Estimated keys learn the
  pair they choose between).
- **The room's tail "before"** (the owner's note on sheet v1) is drawn as a solid line in the
  screen's bright cream, as heavy as the line after, so the gap between them reads.
- **Sixteen factory presets** (`repair.toml`), fitted with `preset_tool fit`; the far-
  microphone ones (*Far-microphone interview*, *Lecture hall*) set Direct sound 0 dB, as the
  DSP report advised; the rest keep the +6 dB default (the owner's ears still to judge).
- **Exported** as `Cantus Purus` (CLAP and VST3, the class id by its rule). Its latency
  changes with its modules; the processor's configure stores it and flags the change: CLAP
  asks for a restart and reports the new latency on reactivation, VST3's UI-thread tick calls
  `restartComponent(kLatencyChanged)` and `getLatencySamples` reads it — both tested.
- **A Tabula pedal**: a de-hum pedal is natural on a guitar's board. Its face: the
  fundamental, the harmonics, the depth, the mix. Its latency is the board's lane latency as
  any pedal's, and follows a module switched on the pedal.

## Consequences

- The editor (design sheet v1, `compartir/purus/design/v1`): a restoration bench — the
  spectrogram on dark glass with what is taken out in the residue's colour, the hum's
  needles, the clicks' lane, the room's tail before and after; three module cartridges
  (the de-hum's tuning fork engraved with the fundamental it follows). Its spectrogram's
  columns come from the slot's signal: in the sheet from the real unit over a demo take;
  live, the slot's scope (step 3, as Speculum's).
  live, the slot's analyser tap (phase B above).
- The editor now costs an FFT pair every 40 ms on the UI thread while it is open (two
  2 048-point transforms: negligible) and two 64 k-sample rings (512 KB) made at its first
  frame.
- A host sees a latency that moves. Hosts that do not honour a restart request keep the old
  compensation until they next activate the plug-in; Cantus itself reads the latency every
  pass.
- Open for the owner: the Direct sound's default (+6 dB) after listening; the presets'
  voicing on real recordings (every signal measured is synthetic but the CC0 guitar's DI).

## What the tests hold

`cantus-core` `fx::purus` (keys, the null, the cap, the constant); `cantus-engine`
`tests/fx_purus.rs` (the null at defaults; each module's latency at 48 and 44.1 kHz and the
telemetry's; the compensation; the starting preset takes a 50.3 Hz hum down > 25 dB; live =
offline; blocks 1/37/64/512; a fork; the cap and the constant), `fx_contract` (block-size
independence, split invariance, exact rest twins — heavy, estimating, never rests),
`rt_memory::purus_never_allocates`; `cantus-project` schema 66's fixture; `cantus-ui`
`fxc_whole` (every word whole, the bench's controls).

Phase B: `cantus-ui` `tests/purus_e2e.rs` through `CantusApp::assemble`, five takes imported
as a user imports them — the eight harmonics of a 50 Hz hum down 40.0 dB each on the master
(the music at 440 Hz +0.00 dB); the hum module's delta is the hum (−0.09 dB) with the music
22.7 dB under it; clicks down 34.1 dB against the clean music, aligned to the sample; a
reverberant voice's tail down 11.7 dB in the pauses (the syllables −1.6 dB); every module on
(87 ms) at mix 0, the strip's master is a parallel dry strip's to the bit; an edit heard and
undone to the bit; live in odd blocks equal to the render; every module off on three strips,
the master without Purus to the bit. `tests/purus_drag.rs`: every bench toggle and key one
undo step; the comb dragged by its first and second needles; the tail's reduction and early
mark dragged; neither while its module is off. `learn_everywhere` green for Purus.
`cantus-engine` `fx_purus::as_a_pedal_it_is_the_insert_and_its_latency_is_the_board_s`.
`cantus-plugins` `clap::purus_s_latency_follows_its_modules_and_is_re_reported`,
`vst3::purus_s_latency_follows_its_modules_through_vst3`, the bit-identical suites with every
module on, the ids golden.
