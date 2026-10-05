# ADR 0026 — The master's insert chain, and three mastering effects

**Status:** accepted (round 19, contract) · **Date:** 2026-10-04

## Context

The user: *"being able to put effects on the master channel is very, very important — and
effects for mastering, like a Maximus, saturation or colour (like Blood Overdrive or
Fruity WaveShaper), or stereo control (Patcher with a mid/side preset)."* Until now the
master had gain, balance, width and polarity, and no inserts: a limiter on the mix needed
a bus strip that everything routed through.

## Decision

**1. The master has an insert chain**, as FL's master track does: `Master::effects` (the
same `EffectSlot`s as a strip, up to the same limit) and `Master::fx_enabled`. It runs on
the summed mix **before** the master's gain, balance and width — the order a mastering
chain expects, and the one FL uses. Every effect command addresses it as
`StripId::MASTER` (`u64::MAX`, never allocated), so the device windows, presets, A/B,
undo, the analyser and the CLAP/VST3 editors need no second path.

**2. Latency on the master** delays every path equally, so it compensates nothing inside
the mix; it is added to the output latency ADR 0022 reports, so the heard position and
recording (ADR 0025) stay right. A master take in the mix recorder is post-chain.

**3. Sidechains into the master chain** may key from any strip (stem-aware limiting); the
master cannot be keyed from (it would be a cycle for every strip).

**4. Three new effects**, to the family's bar (ADR 0017/0018, FabFilter-class, one shell):

- **Fervor** (`EffectKind::Saturator`) — saturation and colour: tape, tube, transistor,
  overdrive/fuzz (Blood Overdrive's pre-amp/tone/post-gain idea) and a **drawn
  waveshaper** (Fruity WaveShaper's curve editor); oversampled, with tone, mix, bias for
  even harmonics, and an output that can be level-matched.
- **Latitudo** (`EffectKind::Stereo`) — stereo control: width, mid and side gain, mono
  below a frequency, balance, rotation, polarity per side, and a mid/side tone section
  (Patcher's mid/side preset, built in); the editor shows a correlation meter and a
  vectorscope (goniometer).
- **Culmen** (`EffectKind::Maximizer`) — the mastering maximizer, FL's Maximus to our bar:
  up to three bands plus the full band, each with a compressor/expander curve, a
  look-ahead brickwall ceiling, linked or split stereo, presets for the usual jobs, and
  the loudness readouts Terminus has.

Each keeps the null rule: at its defaults it touches no sample (or is bit-exact through a
latency-free path), is block-size independent, and ships in the CLAP/VST3 build.

## Consequences

- Schema 18 (shared with ADR 0025). The fixture puts a limiter on the master.
- What the tests hold: an empty or bypassed master chain is bit-exact against the
  previous master; the chain runs before the master fader; master latency moves the heard
  position; each new effect's null test, block-size independence and determinism.
