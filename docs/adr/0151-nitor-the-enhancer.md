# ADR 0151 — Nitor, the one-knob enhancer

**Status:** Accepted (round 41; the owner named it, 2026-11-20); the design below by the `nitor`
builder, its face's sheet and its presets' sound with the owner · **Date:** 2026-11-20 · **Builds on:**
ADR 0018 (the effect family's nulls), ADR 0071 (a starting preset on insert), ADR 0101
(presets as data), ADR 0153 (the song's key reaches effects) · **Schema:** 78

## Context

FL Studio's Soundgoodizer (a one-knob "make it sound better" built on Maximus), Low Lifter
(sub-harmonics and low-end tightening) and the Maximus/Emphasis family's exciters have no
answer in Cantus (BACKLOG §6l B9, B10, B12). The owner chose one unit for all three for round
41 and named it **Nitor**: the Latin for brightness, splendour.

## Decision (what the contract fixed)

- **`EffectKind::Enhancer`**, appended to `EffectKind::ALL` (its index never moves: board
  presets store a kind by its index), offered as an insert and allowed as a pedal.
- Its core module `fx::nitor`: `out` (dB, default 0) and **`key`** — 0 follows the song's
  key (ADR 0153: the engine hands every effect the project's key), 1 … 12 fix C … B (the
  exported plug-in has no song). `tonic(key, song)` resolves it. **Every default a null to the
  bit.**
- A stub engine unit (`cantus-engine/src/fx/nitor.rs`: a wire times `out`, keeping the key it
  is handed), an editor stub in the device frame (`views/fx/nitor.rs`), a placeholder face
  (`kit/skin/side_r41.rs`, a key of its own), its pedal face row, locale files, an empty
  preset file (`presets/effects/enhancer.toml`, floor 0 until its presets land), the picker's
  Mastering shelf, the AI's word `nitor` (aliases `enhancer`, `exciter`).
- **Exported** as a CLAP/VST3 plug-in from the contract on: `com.github.lalmazan.cantus.nitor`,
  VST3 class id `46395C6F-19D2E9B4-7D15652E-590ED7C6`, blessed in
  `cantus-plugins/tests/golden/ids.txt` — **forever**, the class id by its rule (SHA-256 of `cantus/vst3/nitor`). The parameters `out` and `key` stay forever.

## The builder's decisions (round 41, `nitor`)

1. **One knob over curated recipes, the parts open on top.** `amount` (0 … 100 %) turns up a
   **character** (`char`: Clear, Punch, Warm, Air, Loud). A character is a recipe at amount 1
   (`cantus_dsp::nitor::RECIPES`): four Multiplex bands (20–150 Hz, 150 Hz–1.5 kHz, 1.5–7 kHz,
   over 7 kHz) with downward and upward compression and a static tilt, plus what it adds to the
   exciter's bands and the low end. The amount takes every ratio from 1:1 to the recipe's and
   every gain from 0 dB to its own. The low end's and the exciter's own knobs add to the
   character's (each part 0 … 1, the sum held at 1).
2. **The dynamics are Multiplex's dynamic phase** (`cantus_dsp::multiplex`, unchanged): no
   latency; while no band moves the path is the input to the bit; equal gains are a pure
   gain. Reused, not copied: Multiplex's goldens do not move.
3. **Level independence by a pre-gain**: a slow K-weighted loudness of the input brings any
   programme to −18 LUFS before the dynamics (the recipes' thresholds are set against band
   peaks of programme there, measured) and takes it back after. Measured: the band peaks and
   gains of the test programme identical at 0 and −20 dB of input.
4. **The exciter adds only harmonics**: each band normalised by its own envelope, `v/√(1+v²)`
   less `v` (odd) and its square (even), blended by `color`, 4× oversampled minimum phase
   (`fervor::os`), high-passed at the band's lower edge, times the envelope. No transcendental
   function on the audio path; the same share at −40 dBFS as at −6 (measured within 0.5 dB).
5. **The low end**: the mid under `xover` (Linkwitz–Riley) feeds the sub (`submode`: Saltus's
   divider an octave under the bass, or a sine on the **tonic** in octave 1, gated by the low
   band); the bass's harmonics (2× oversampled, 1.5–6 × the crossover); **tight** (a hit — the
   mid's one-pole at 3 × the crossover, a held peak 30 % over a slow follower — opens the low
   band 30 ms, then a 120 ms fall toward −12 dB × tightness through a Butterworth low shelf
   designed each sample); **lift** (bells, Q 2.5, on the tonic, +6 dB, and its octave, +3 dB).
   The tonic is `fx::nitor::tonic(key, song)`; the engine's unit keeps the `key` parameter and
   re-reads only the tonic when the song's key arrives (no allocation on the audio thread).
6. **Level-matched output** (`match`, default on — honest A/B): the input's and the wet path's
   BS.1770 K-weighted loudness, each a running mean becoming a 1.5 s exponential one, their
   ratio glided (150 ms) on a 16-sample grid within ±12 dB; nothing moves under −70 LUFS; 8 s of
   exact digital silence forgets the measures. Every preset keeps it on, so the presets'
   render check (±2 LU of the dry source) holds without an exemption.
7. **Grid cells for block-size independence**: what a 16-sample cell runs is decided at its
   first sample and what is cleared at its last, so a block that ends inside a cell changes
   nothing (tolerance 0 at blocks of 1, 37, 64, 512 and mixed, with changes mid-render).
8. **The dynamics wait for sound**: upward compression acts on silence, so Multiplex never
   rests by itself; after 0.5 s of exact silence (its output exactly zero) it is reset and
   *parked*, and blended back in over 20 ms from the end of the first cell that carries sound.
   A new unit starts parked, so a resting unit is a new one (the rest twins hold it).
9. **No latency** in any setting: the oversampling is minimum phase (the harmonics, which had
   no counterpart in the dry, come about a sample late), the dynamics have no look-ahead.
10. **Parameters** (forever): `amount`, `char`, `sub`, `submode`, `harm`, `tight`, `lift`,
    `xover`, `body`, `pres`, `air`, `color`, `match`, `mix`, with the contract's `out` and
    `key`. The starting preset is *Gentle shine* (`amount` 0.35, Clear, matched).

## Consequences

- A song saved with a Nitor does not open in beta.16 or older (schema 78).
- The plug-in's ids and `out` are shipped with the next release whatever the unit becomes.

## What the tests hold

- `fx::nitor`'s defaults are nulls and `tonic` follows the song or the fixed key.
- `fx_contract`'s twins (the null, block sizes, rest, the board of one) cover it as a stub.
- The plug-in's bit identity through CLAP and VST3 (a wire), and its blessed ids.
- `schema_78_…` keeps its key and output.
