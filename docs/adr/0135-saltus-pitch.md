# ADR 0135 — Saltus, pitch

**Status:** proposed (fx side round wave 3, provisional number — renumbered at merge) ·
**Date:** 2026-11-14 · **Schema:** 63 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0031 §5.1 (at rest), ADR 0071 (effects that
sound on insert), ADR 0101 (presets), ADR 0134 (Tabula: every unit is also a pedal)

## Context

The owner's pedalboard (ADR 0134) needs an octaver, a pitch shifter and a whammy, and BACKLOG §3b *Pitch and creative* asks for a harmoniser; he asked (2026-11-13) that every new engine wave 3 makes be a full unit of the family as well as a pedal, and chose the name Saltus (Latin, *the leap*).

## Decision

A new effect kind, `EffectKind::PitchShift`, `fx::saltus`: the pitch unit. To be written by its builder, following the design (`tabula-design.md` §6): the shift by `psola::Shifter`, formant-keeping (128 samples at 48 kHz); the harmony as a second voice in scale degrees (`pitch_stream`, `tonus_scale`); the octaver (−1/−2) from a tracker-driven divider, latency-free, and a clean PSOLA mode; the whammy as a `pedal` 0…1 over a chosen interval, glided; a poly mode by phase vocoder (`stretch_pv`, ≈ 2048). Latency fixed per mode and reported; `TAKES_NOTES = true` (the contract left it false). The parameter table (every default a null), the DSP (`cantus_dsp::saltus`), the editor, the presets and the starting preset or the reason for none.

**The contract** (wave 3's contract commit): `EffectKind::PitchShift` declared and wired into
every list as an exact pass-through stub — one placeholder `out` parameter (0 dB, a null),
an engine unit that applies it, is always at rest and has no latency, a plain kit editor,
a placeholder face (a tone-family sibling's look under its own key), its row of Tabula's
faces (`views/fx/tabula_faces.rs`), its locale pair, an empty preset file, its exported
CLAP/VST3 descriptor and ids, and schema 63's fixture.

## Consequences

To be written by the builder.

## What the tests hold

To be written by the builder.
