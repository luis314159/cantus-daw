# ADR 0142 — Vinculum, the bus compressor

**Status:** proposed (fx side round wave B, provisional number — renumbered at merge) ·
**Date:** 2026-11-17 · **Schema:** 65 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0005 (the audio thread), ADR 0018 (null defaults), ADR 0022 (an effect's
sidechain), ADR 0031 §5.1 (at rest), ADR 0071 (effects that sound on insert)

## Context

Comprimo is a general compressor with styles; a mix bus wants the three characters
engineers reach for by name — the console's VCA bus compressor with its Auto release, the
photocell leveller, the variable-mu valve limiter — each with the controls its panel has.
The DSP was built first (`docs/round-fx/vinculum.md`): the VCA after Giannoulis, Massberg
and Reiss (JAES 2012) with a two-capacitor Auto release; the opto as a T4-style cell in
feedback; the vari-mu as a remote-cutoff valve whose loop is solved at zero delay through
its tabulated equilibrium, with an odd-harmonic colour at 2× on the residual only. Latency
0; CPU ≤ 0.37 % of a core.

## Decision

A new effect kind, `EffectKind::BusCompressor`, model `cantus_core::fx::vinculum`, engine
unit `cantus_engine::fx::vinculum`, DSP `cantus_dsp::vinculum`.

- **Parameters** (12 keys): `model`, `thr`, the VCA's panel as stepped positions (`ratio`
  2/4/10, `att` 0.1–30 ms, `rel` 0.1–1.2 s and Auto), the opto's and the vari-mu's
  Compress/Limit (`char`), the vari-mu's six time positions (`time`), the side chain's
  high-pass (`hpf`, 20 Hz is off), `makeup`, `colour`, `mix`, `out`. **Every default is a
  null**: the threshold at its top (+24 dBFS) over any signal, no make-up, no colour, `out`
  at 0 dB not applied — tested to the bit through the engine for each model.
- **The external key** is the slot's sidechain (another strip's post-fader output, the
  engine's ordinary key path); with none routed the input is its own key. In the feedback
  models the key takes the input's place in the loop. `Vinculum::process_keyed(…, None)` is
  `process` to the bit.
- **Starting preset** (ADR 0071): *Glue* — the VCA at 4 : 1, 30 ms, Auto, −18 dB, +2 dB.

## The owner's open choice

- **The opto Limit's attack** measures 0.9 ms to 63 % at 16 dB over the threshold against
  the published ~10 ms average (the panel's eighth power lights the cell far past where it
  settles). The DSP is kept as measured; closing it (a bounded conductance, or a
  drive-dependent rate) changes how the Limit sounds on hits, so it waits for the owner's
  ear (`compartir\vinculum\sound`).

## Consequences

- One more dynamics unit with a face of its own (a rack bus compressor, three faceplates).
- The side chain's high-pass now lets go of states under 1e-15 when fed exact zeros: on
  silence a low corner's states cycled around 1e-19 and the unit never rested (found by the
  engine's rest test); no output sample changes while there is signal.

## What the tests hold

`cantus-dsp`: the models against their laws (27 tests), `the_external_key_drives_the_side_chain`,
`the_side_chain_high_pass_comes_to_rest`; `cantus-engine/tests/fx_vinculum.rs` (the null for
each model, the starting preset at every block size and live, every model block-size
independent, the external key and its high-pass, forks), `fx_contract` (split and at rest),
`rt_memory::vinculum_never_allocates`.
