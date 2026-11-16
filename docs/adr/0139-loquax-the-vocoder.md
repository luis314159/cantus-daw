# ADR 0139 — Loquax, the vocoder

**Status:** proposed (fx side round wave A, provisional number — renumbered at merge) ·
**Date:** 2026-11-16 · **Schema:** 64 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0022 (an effect's MIDI source and sidechain),
ADR 0031 §5.1 (at rest), ADR 0071 (effects that sound on insert), ADR 0134 (Tabula)

## Context

Cantus had no vocoder; FL Studio's Vocodex is the bar the owner holds us to, with Arturia's
Vocoder V, Waves' OVox and Morphoder and the Roland VP-330, SVC-350 and EMS 2000 as the
character references (read about, nothing copied). The DSP was built first in the DSP lane
(`docs/round-fx/loquax.md`: Butterworth banks of order 4/6/8 within 0.003 dB of the
analytic response, NCM 0.74 at 16 bands, latency 0, CPU 1.6–4.7 % of a core).

## Decision

A new effect kind, `EffectKind::Vocoder`, model `cantus_core::fx::loquax`, engine unit
`cantus_engine::fx::loquax`, DSP `cantus_dsp::loquax`.

- **Parameters** (100 keys, every default a null — the mix at 0): mix, carrier source,
  swap, bands (8–64), slope order, range, width, followers, formant and band shift, tilt,
  flatten, sibilance and consonant noise, hold, stereo spread, the raw carrier through, the
  synthesiser (saw/pulse/noise blend, unison, octave, envelope, level), the robot's drone,
  and one level per band, `b0` … `b63` (the display's drawn bars), `out`.
- **Routing — the carrier and the voice.** The strip's own sound is the voice. The carrier
  is the built-in polyphonic synthesiser played by the notes of the track the slot listens
  to (`EffectSlot::midi_from`, `TAKES_NOTES`), and/or the slot's **sidechain**
  (`EffectSlot::sidechain`, `TAKES_SIDECHAIN`): the engine's ordinary key path — the strip
  runner reads the keyed strip's buffer and hands it to `FxUnit::process` as `key`, exactly
  as for a compressor's detector. Nothing new was built for it; `src` chooses the
  sidechain, the synthesiser or both, and `swap` makes the key the voice and the strip the
  carrier. A strip with a note listener never rests, so held keys always reach the unit.
- **Notes** wait in a fixed list of 64 per block (an allocation-free insertion sort keeps
  them in time order) and reach the DSP with their frames.
- **Starting preset** (ADR 0071): *Robot* — the drone on C3, 16 bands, all wet: a voice
  through it is vocoded at once with no notes and no sidechain.
- **Telemetry:** the DSP's own 32 numbers, then the bands' output levels in 32 groups.
- **Not yet a pedal** on Tabula (`pedal_allowed` false until its face joins the board in
  phase B).

## Consequences

To be completed in phase B (the editor, presets, export, the AI's words, docs, e2e).

## What the tests hold

`cantus-engine/tests/fx_loquax.rs`: the null to the bit and latency 0; the starting preset
sounds, live = offline, blocks of 1/37/64/512 at tolerance 0; another track's notes play
the synthesiser (live = offline, blocks of 1 and 64); the sidechain carries and `swap`
trades the roles (live = offline, three block sizes); a fork carries on as the unit does.
`fx_contract` (`split::loquax`, `at_rest::loquax`), `rt_memory::loquax_never_allocates`,
the undo fuzz (its parameters through `SetEffectParam`). The DSP's 22 tests
(`docs/round-fx/loquax.md`).
