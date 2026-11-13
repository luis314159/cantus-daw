# ADR 0130 — Cribrum, the auto filter

**Status:** proposed (fx side round, provisional number — renumbered at merge) · **Date:**
2026-11-13 · **Schema:** 62 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0031 §5.1 (at rest), ADR 0071 (effects that
sound on insert), ADR 0101 (presets by style and role)

## Context

Every market-leading DAW ships an auto filter (Ableton's Auto Filter, FL Studio's Fruity Filter and Autogun-era envelopes, Logic's AutoFilter, Bitwig's Filter+ with modulators). Cantus has filters inside its instruments and Aequalis's bands, but no insert that *moves* a filter: a sweep, a wobble, an envelope-follower wah.

## Decision

A new effect kind, `EffectKind::AutoFilter` (Latin *cribrum*, the sieve), `fx::cribrum`: analog filter models moved by an envelope, a tempo-synced LFO and an envelope follower. To be written by its builder: the parameter table (every default a null, ADR 0018), the DSP (`cantus_dsp::cribrum`), the editor, the presets and the starting preset or the reason for none (ADR 0071).

**The contract** (the fx side round's contract commit): `EffectKind::AutoFilter` declared and wired into
every list as an exact pass-through stub — one placeholder `out` parameter (0 dB, a null),
an engine unit that applies it, is always at rest and has no latency, a plain kit editor,
its locale pair, its exported CLAP/VST3 descriptor and ids, and schema 62's fixture.

## Consequences

To be written by the builder.

## What the tests hold

To be written by the builder.
