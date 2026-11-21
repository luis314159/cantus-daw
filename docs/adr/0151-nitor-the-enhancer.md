# ADR 0151 — Nitor, the one-knob enhancer

**Status:** Accepted as a contract (round 41; the owner named it, 2026-11-20); the `nitor`
builder fills in the design and the measurements · **Date:** 2026-11-20 · **Builds on:**
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

## The builder's (to fill in)

The amount knob over curated multiband characters (Multiplex's crossovers), the low end
(sub-harmonics after Saltus's octave divider, tuned to `tonic`; tightening), the exciter by
band (oversampled through `fervor_os`), a level-matched output so its presets pass
`preset_render`'s no-level-change rule (or a documented exemption), the face, 12 presets by
ear, a page in `docs/plugins/`. See `docs/round41/briefs.md`.

## Consequences

- A song saved with a Nitor does not open in beta.16 or older (schema 78).
- The plug-in's ids and `out` are shipped with the next release whatever the unit becomes.

## What the tests hold

- `fx::nitor`'s defaults are nulls and `tonic` follows the song or the fixed key.
- `fx_contract`'s twins (the null, block sizes, rest, the board of one) cover it as a stub.
- The plug-in's bit identity through CLAP and VST3 (a wire), and its blessed ids.
- `schema_78_…` keeps its key and output.
