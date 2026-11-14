# ADR 0136 — Rota, the rotary speaker

**Status:** proposed (fx side round wave 3, provisional number — renumbered at merge) ·
**Date:** 2026-11-14 · **Schema:** 63 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0031 §5.1 (at rest), ADR 0071 (effects that
sound on insert), ADR 0101 (presets), ADR 0134 (Tabula: every unit is also a pedal)

## Context

The rotary speaker lives inside Cantus's organ (`organum_rotary`) but not as an effect (BACKLOG §3b *Still open*); the owner's pedalboard (ADR 0134) wants it, and a Uni-Vibe voicing. He asked (2026-11-13) that every new engine of wave 3 be a full unit as well as a pedal.

## Decision

A new effect kind, `EffectKind::Rotary`, `fx::rota`: the rotary speaker for any stereo signal. To be written by its builder, following the design (`tabula-design.md` §6): it wraps `organum_rotary::Rotary` — speed stop/slow/fast with ramps, horn and drum rpm, microphone spread, balance, drive; notes toggle the speed (`TAKES_NOTES = true`, the contract left it false); latency 0; at rest only when its lines are silent and the rotors' speed constant (`skip` advances the angle in closed form). **The Uni-Vibe** is a new Phasis style appended to its choices, by the same builder (Phasis keeps every existing key and default). The parameter table (every default a null), the DSP (`cantus_dsp::rota`), the editor, the presets and the starting preset or the reason for none.

**The contract** (wave 3's contract commit): `EffectKind::Rotary` declared and wired into
every list as an exact pass-through stub — one placeholder `out` parameter (0 dB, a null),
an engine unit that applies it, is always at rest and has no latency, a plain kit editor,
a placeholder face (a tone-family sibling's look under its own key), its row of Tabula's
faces (`views/fx/tabula_faces.rs`), its locale pair, an empty preset file, its exported
CLAP/VST3 descriptor and ids, and schema 63's fixture.

## Consequences

To be written by the builder.

## What the tests hold

To be written by the builder.
