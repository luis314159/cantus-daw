# ADR 0144 — Cedo, the ducker

**Status:** proposed (fx side round wave B, provisional number — renumbered at merge) ·
**Date:** 2026-11-17 · **Schema:** 65 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0018 (null defaults), ADR 0022 (an effect's
sidechain and `midi_from`), ADR 0031 §5.1 (at rest), ADR 0055 (Fluctus's shapes), ADR 0071

## Context

Ducking in Cantus meant a compressor keyed by another strip. Producers want the pump as a
shape (FL's Fruity Limiter and Peak Controller, LFOTool, Kickstart, VolumeShaper — function
only), broadcast wants a voice-over ducker. The DSP was built first
(`docs/round-fx/cedo.md`): a sample-exact envelope fired by the key (hold from its last
sample over the threshold, an S attack), a drawn shape on the song's grid or on notes, a
centred edge kernel, band ducking split at minimum or linear phase, lookahead; CPU ≤ 1 %.

## Decision

A new effect kind, `EffectKind::Ducker`, model `cantus_core::fx::cedo`, engine unit
`cantus_engine::fx::cedo`, DSP `cantus_dsp::cedo`.

- **Parameters** (24 keys, every default a null — depth 0 ducks nothing, to the bit, at
  every trigger): the trigger, depth (80 dB is silence, shown ∞), threshold, attack, hold,
  release and its shape, the key's filter and detector, lookahead, the shape's cycle, swing
  and edge, the band, its split and crossover, link, mix, out, listen, velocity.
- **The shape is content** (`EffectSpec::cedo`, at most 64 points with the automation
  editor's segment shapes; `None` is the classic pump and is not written).
  `Command::SetCedoShape` replaces it, sanitized, its inverse the shape it replaced; a
  hand-edited shape is repaired on load (schema 65).
- **Its three triggers use the routes the engine has**: the sidechain is the slot's key
  (another strip's post-fader output); Tempo reads the song's position each pass; MIDI
  plays the notes of the track the slot listens to (`midi_from`, another track, ADR 0022).
  No new routing was needed.
- **The rest check sees the key**: a keyed insert whose key strip sounded this pass keeps
  its strip awake (`graph::strip::may_rest`), so a ducker whose own input is silent while
  its key plays never rests wrongly; tested through the engine with Listen at Key.
- **Latency** = lookahead + the linear-phase kernel's; the linear-phase split's floor is
  **60 Hz** (`cantus_dsp::cedo::band::LIN_MIN_HZ`, 36.8 ms at 48 kHz, fixed whatever the
  crossover) and **minimum phase is the default split** (no latency). The owner may later
  choose 50 Hz (44 ms) or 80 Hz (27.6 ms): one constant.
- **Starting preset** (ADR 0071): *Classic pump* — Tempo, quarters, 9 dB: it sounds on any
  track with no routing.

## Consequences

- The pump is a function of the song's sample: a render from a bar line hears what the
  whole render hears there (tested from bar 5).

## What the tests hold

`cantus-core`: the shape's sanitizing, the undo fuzz arm for `SetCedoShape`;
`cantus-project`: `schema_65_keeps_wave_b_s_kinds_and_cedo_s_shape`;
`cantus-engine/tests/fx_cedo.rs` (the null at every trigger, the starting preset at every
block size, live and from bar 5, the sidechain from another strip, a silent input with a
playing key, the notes of another track, the lookahead and the linear phase compensated,
forks), `fx_contract` (split and at rest), `rt_memory::cedo_never_allocates`.
