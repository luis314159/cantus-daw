# ADR 0141 — Caelum, the shimmer reverb

**Status:** proposed (fx side round wave A, provisional number — renumbered at merge) ·
**Date:** 2026-11-16 · **Schema:** 64 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0005 (the audio thread), ADR 0018 (null defaults), ADR 0022 (an effect's
sidechain), ADR 0031 §5.1 (at rest), ADR 0071 (effects that sound on insert),
`cantus_dsp::spatium` (its mix law)

## Context

Spatium's rooms, Aula's convolution and the legacy Reverb leave no pitch-shifted feedback,
infinite washes, bloom or freeze (Valhalla Shimmer and SuperMassive, Strymon BigSky,
Eventide Blackhole — function only). The DSP was built first (`docs/round-fx/caelum.md`:
an FDN of 8 or 16 lines with Jot's absorption, RT60 worst band 3.1 %, an energy-exchanging
shimmer that makes no energy, exact rest by a drain, CPU 1.6–4 % of a core).

## Decision

A new effect kind, `EffectKind::Shimmer`, model `cantus_core::fx::caelum`, engine unit
`cantus_engine::fx::caelum`, DSP `cantus_dsp::caelum`.

- **Parameters** (32 keys, every default a null — the mix at 0 over a musical space):
  size, decay, infinite, freeze, bloom, the pre-delay (free or a note value at the song's
  tempo, `psync`), diffusion, high and low damping, modulation, the shimmer (amount, two
  voices, the second's level, the loop's tone), ducking (keyed by the sidechain when one
  is routed), the wet's tone, width, the tank's lines, mix, `out` (applied after the DSP,
  gliding, not at all at 0 dB).
- **The owner's two open questions, each a parameter with today's measured behaviour the
  default:** `ishim` — infinite with the shimmer **fades** as it climbs out through the
  loop's tone (0, the default: −27 dB in ten seconds, −120 dB in a minute) or **sustains**
  (1: once nothing more comes in, the climb settles — the shimmer's amount glides to nothing
  over 1.5 s — and the hold keeps the wash it climbed to: within +0.2 dB for a minute);
  `ilevel` — an infinite tank
  **8 dB under** an 8 s tank (0, the default: it keeps all it is fed) or **as loud** (1).
  Both added to the DSP (`CaelumSettings::{infinite_sustain, infinite_full}`); every
  existing setting's bits unchanged. A preset can choose either.
- **Starting preset** (ADR 0071): *Octave shimmer* — the DSP's `presets::starting`.
- **Not yet a pedal** on Tabula (phase B).

## Consequences

To be completed in phase B.

## What the tests hold

`cantus-engine/tests/fx_caelum.rs`: the null to the bit and latency 0; the starting preset
sounds, live = offline, blocks of 1/37/64/512; the ducking hears the sidechain (live =
offline); a synced pre-delay is its note at the tempo (117.2 ms a sixteenth at 128 BPM);
a fork carries on; the two choices reach the DSP. `fx_contract`,
`rt_memory::caelum_never_allocates` (shimmer, infinite and sustained, frozen, a redesign,
keyed and synced). DSP: `infinite_with_the_shimmer_fades_or_sustains_and_its_level_is_a_choice`
and the DSP's own 28.
