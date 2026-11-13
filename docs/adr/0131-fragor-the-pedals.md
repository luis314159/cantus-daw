# ADR 0131 — Fragor, the pedals

**Status:** proposed (fx side round, provisional number — renumbered at merge) · **Date:**
2026-11-13 · **Schema:** 62 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0031 §5.1 (at rest), ADR 0071 (effects that
sound on insert), ADR 0101 (presets by style and role)

## Context

Fervor (ADR 0026) is saturation and colour; it is not a guitarist's pedalboard. The reference pedals — the Tube Screamer, the RAT, the DS-1, the Fuzz Face, the Big Muff — each have a sound that comes from their circuit (clipping diodes in the feedback loop or to ground, a transistor fuzz's bias, a tone stack), and the best plug-ins model those circuits rather than a generic curve.

## Decision

A new effect kind, `EffectKind::Pedal` (Latin *fragor*, a crash, a roar), `fx::fragor`: distortion and fuzz pedals, circuit-modelled. To be written by its builder: the models and their parameter table (every default a null), the DSP (`cantus_dsp::fragor`, oversampled, deterministic maths), the editor, the presets and the starting preset or the reason for none.

**The contract** (the fx side round's contract commit): `EffectKind::Pedal` declared and wired into
every list as an exact pass-through stub — one placeholder `out` parameter (0 dB, a null),
an engine unit that applies it, is always at rest and has no latency, a plain kit editor,
its locale pair, its exported CLAP/VST3 descriptor and ids, and schema 62's fixture.

## Consequences

To be written by the builder.

## What the tests hold

To be written by the builder.
