# ADR 0143 — Lenis, the resonance suppressor

**Status:** proposed (fx side round wave B, provisional number — renumbered at merge) ·
**Date:** 2026-11-17 · **Schema:** 65 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0005 (the audio thread), ADR 0018 (null defaults), ADR 0031 §5.1 (at
rest), ADR 0071 (effects that sound on insert), delay compensation (the strips' latency)

## Context

Nothing in Cantus found and tamed resonances moment by moment (the function of oeksound's
soothe2 and the dynamic-EQ resonance modes — function only, never their algorithm). The DSP
was built first (`docs/round-fx/lenis.md`): a low-delay asymmetric STFT (Mauler & Martin),
a per-bin detector against the spectrum's own envelope (a greatest-of CFAR envelope under
the prominent peaks, harmonic awareness under 800 Hz, an onset lift), the cut a share of
each bump; 10.6 ms of latency at Standard; CPU 3.3 % of a core linked.

## Decision

A new effect kind, `EffectKind::ResonanceSuppressor`, model `cantus_core::fx::lenis`,
engine unit `cantus_engine::fx::lenis`, DSP `cantus_dsp::lenis`.

- **Parameters** (37 keys): depth, sharpness, selectivity, attack, release, the focus's
  limits, six focus bands (on, frequency, amount, width), channels (Linked, Independent,
  Mid and side), curve (Soft, Hard), quality (Fast, Standard, Precise), delta, mix, out.
  **The default is a null**: depth 0 — the output is the input delayed by the latency, to
  the bit (the DSP's dry ring), and the engine's compensation holds every other path back
  by as much (tested through the engine at each quality).
- **A quality change resets the unit and reports a new latency.** The three frames have
  different latencies (255 / 511 / 1 023 samples at 48 kHz); two outputs delayed
  differently cannot crossfade without a comb, and the engine must re-align anyway. The DSP
  takes the change at its next hop, starting again from silence (a gap of one latency,
  5–21 ms); the engine reads `latency()` each pass, so its compensation follows.
- **The engine's reset is a new unit**: `Lenis::restart` clears the audio state and puts the
  frame grid's clock at 0 (the engine skips a reset unit to where the stream is), so a reset
  twin and a new one agree to the bit. A quiet detector now holds the state silence leaves
  (calm, no lift, no onset), so a unit at rest is as good as new.
- **Delta** hears what is taken away (`dry − output`, to the bit at 0 dB out).
- **Telemetry**: the meter's 128 display points travel in five interleaved pages of 26 (the
  page is the frame count's); the editor keeps the whole picture.
- **Starting preset** (ADR 0071): *Gentle* — depth 50 %, Soft, the focus from 150 Hz.

## Consequences

- A latent insert by default (10.6 ms), like any linear-phase unit; a live monitoring path
  bypasses it hard where low-latency monitoring asks (ADR 0064 §4).

## What the tests hold

`cantus-dsp`: 22 + `a_clone_carries_on_exactly_as_the_original`, `a_restarted_unit_is_a_new_one`;
`cantus-engine/tests/fx_lenis.rs` (the dry mix delayed by exactly the latency at each
quality, a quality change reporting its latency, the starting preset at every block size
and live, the delta, forks), `fx_contract` (split and at rest), `rt_memory::lenis_never_allocates`.
