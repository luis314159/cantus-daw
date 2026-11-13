# ADR 0132 — Lapsus, the time shaper

**Status:** proposed (fx side round, provisional number — renumbered at merge) · **Date:**
2026-11-13 · **Schema:** 62 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0031 §5.1 (at rest), ADR 0071 (effects that
sound on insert), ADR 0101 (presets by style and role)

## Context

FL Studio's Gross Beat is one of its signature effects: time and volume envelopes drawn per beat — tape stops, half-time, stutters, scratches — on a buffer of the song's recent past. Fluctus (round 28) shapes volume only; nothing in Cantus bends time.

## Decision

A new effect kind, `EffectKind::TimeShaper` (Latin *lapsus*, a slip, a glide), `fx::lapsus`: time and volume envelopes synced to the song. To be written by its builder: the parameter table (every default a null), the DSP (`cantus_dsp::lapsus`; its buffer allocated off the audio thread, ADR 0005), the editor, the presets and the starting preset or the reason for none.

**The contract** (the fx side round's contract commit): `EffectKind::TimeShaper` declared and wired into
every list as an exact pass-through stub — one placeholder `out` parameter (0 dB, a null),
an engine unit that applies it, is always at rest and has no latency, a plain kit editor,
its locale pair, its exported CLAP/VST3 descriptor and ids, and schema 62's fixture.

## Consequences

To be written by the builder.

## What the tests hold

To be written by the builder.
