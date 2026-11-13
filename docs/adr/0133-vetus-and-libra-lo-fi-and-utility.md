# ADR 0133 — Vetus and Libra, lo-fi and utility

**Status:** proposed (fx side round, provisional number — renumbered at merge) · **Date:**
2026-11-13 · **Schema:** 62 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0031 §5.1 (at rest), ADR 0071 (effects that
sound on insert), ADR 0101 (presets by style and role)

## Context

Two everyday tools Cantus lacks as inserts: a lo-fi unit (bit crushing, sample-rate reduction, vinyl and tape wear — Ableton's Redux and Vinyl Distortion, FL's Fruity Squeeze, RC-20-class colour) and a utility (gain, polarity, channel operations, a delay to align, a tuner — Ableton's Utility, FL's Fruity Balance and Stereo Shaper, Logic's Gain).

## Decision

Two new effect kinds: `EffectKind::LoFi` (Latin *vetus*, old), `fx::vetus`, and `EffectKind::Utility` (Latin *libra*, the balance), `fx::libra`. To be written by their builders: each parameter table (every default a null), the DSP (`cantus_dsp::vetus`, `cantus_dsp::libra`), the editors, the presets and the starting preset or the reason for none.

**The contract** (the fx side round's contract commit): `EffectKind::LoFi` and `EffectKind::Utility` declared and wired into
every list as an exact pass-through stub — one placeholder `out` parameter (0 dB, a null),
an engine unit that applies it, is always at rest and has no latency, a plain kit editor,
its locale pair, its exported CLAP/VST3 descriptor and ids, and schema 62's fixture.

## Consequences

To be written by the builder.

## What the tests hold

To be written by the builder.
