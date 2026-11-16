# ADR 0140 — Fractus, the glitch sequencer and grain cloud

**Status:** proposed (fx side round wave A, provisional number — renumbered at merge) ·
**Date:** 2026-11-16 · **Schema:** 64 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0022 (the memory path; an effect's MIDI source), ADR 0031 §5.1 (at rest), ADR 0071
(effects that sound on insert), ADR 0083 (a range render's checkpoint), ADR 0132 (Lapsus)

## Context

Lapsus is a time curve; the owner's list (Effectrix, Turnado, Stutter Edit, Beat Repeat,
Portal, Granulator) asks for a *sequence of events*. The DSP was built first
(`docs/round-fx/fractus.md`: ten modules on ten lanes, a grain cloud, every choice a
function of the song sample and the seed, joins −62…−71 dB of splatter, latency 0).

## Decision

A new effect kind, `EffectKind::Glitch`, model `cantus_core::fx::fractus`, engine unit
`cantus_engine::fx::fractus`, DSP `cantus_dsp::fractus`.

- **Parameters** (52 keys, every default a null — nothing painted, the cloud at 0): the
  grid, the trigger, every module's settings, the cloud's, `out`. Note values are indices
  into the DSP's fourteen; `vslice`, `ttime` and `grate` keep 0 for their own meaning.
- **The lanes are content, not parameters:** `EffectSpec::fractus:
  Option<Box<Pattern>>` — per lane a sorted list of blocks `{at, len, amount, prob}`,
  only the lanes that hold one; `None` when nothing is painted (and not written). Schema 64,
  `migrate_63_to_64` writes nothing; a hand-edited pattern is repaired on load
  (`fx::fractus::repair_project`). **`Command::SetFractusLane {strip, effect, lane,
  blocks}`** replaces one lane (sanitized: sorted, apart, inside the 256 steps); its exact
  inverse is the lane it replaced; `cmd.fractus_lane`; its undo-fuzz arm.
- **The memory, off the audio thread.** The DSP holds 8.6 MB at 48 kHz (34 MB at
  192 kHz). A unit is made without it — a wire, exactly what the DSP plays at the defaults —
  and asks through the memory path (`MemoryNeed::Fractus`, a value small enough for the
  event) once its settings make it more than a wire. The UI thread builds the whole DSP
  (`EffectMemory::Fractus(Box<Fractus>)`) and the unit takes the box (a pointer moved; it
  hands back one it cannot use). `EffectUnit::new` supplies it at once, so a project's
  render and a live play are the same from the first sample; an empty Fractus costs
  nothing. Once taken it stays (freed with the unit, off the audio thread). This follows
  Lapsus's and Arca's path rather than building 8.6 MB at every insert of an empty unit.
- **The pitch jump's style** (the owner's open choice): `pstyle` — the classic four-head
  delay-line warble (0, the default: the DSP report calls it deliberate, the glitch pitch)
  or **clean** (1): two heads, one sounding at a time with a splice of a fifth of the
  window (sin²/cos², summing to one), no comb between taps. Added to the DSP
  (`PitchSettings::clean`); the classic's bits unchanged.
- **Forks** clone the whole DSP (history, capture, cloud, the sequence's place).
- **Starting spec** (ADR 0071, `Start::Spec`): *Last beat stutter* — a repeat on the last
  beat of each bar of sixteenths.
- **Not yet a pedal** on Tabula (its pattern is not routed through a board; phase B).

## Consequences

To be completed in phase B (the editor with its painted lanes, presets, export with the
pattern in the plug-in's state, the AI's words, docs, e2e).

## What the tests hold

`cantus-engine/tests/fx_fractus.rs`: the null to the bit, latency 0, no memory asked when
empty; the starting spec sounds, live = offline, blocks of 1/37/64/512; every lane and the
cloud live = offline; painted while playing it asks once and takes its DSP; another
track's notes play the lanes (live = offline); a fork carries on. `fx_contract`,
`rt_memory::fractus_never_allocates` (empty, painted while playing — the DSP taken —
cloud, notes, cleared and painted again). Core: the pattern's sanitizing and its JSON;
`SetFractusLane` in the undo fuzz. Project: `schema_64_keeps_wave_a_s_kinds_and_fractus_s_lanes`
(and the repair). DSP: `the_clean_pitch_jump_holds_its_level_and_the_classic_warbles`,
`a_pitch_jump_lands_on_its_interval` (both styles), and the DSP's own 26.
