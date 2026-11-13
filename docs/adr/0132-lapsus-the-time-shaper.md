# ADR 0132 — Lapsus, the time shaper

**Status:** proposed (fx side round, provisional number — renumbered at merge) · **Date:**
2026-11-13 · **Schema:** 62 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0022 (memory off the audio thread), ADR 0031
§5.1 (at rest), ADR 0055 (the family's clock, Fluctus's curve), ADR 0071 (effects that
sound on insert), ADR 0074 (faces), ADR 0101 (presets by style and role)

## Context

FL Studio's Gross Beat is one of its signature effects: time and volume envelopes drawn per
beat — tape stops, half-time, stutters, scratches — on a buffer of the song's recent past.
Fluctus (round 28) shapes volume only; nothing in Cantus bent time.

## Decision (phase A: the model, the DSP, the engine unit; the editor's design sheet)

1. **`EffectKind::TimeShaper`, `fx::lapsus`.** Nine parameters, every default a null:
   `slot` (0 … 35; slot 1 of the factory bank is *Through*), `mix` (100 %), `xfade` (2 ms),
   `smooth` (1 ms), `notes` (off / switch / hold — hold), `retrig` (off), `key` (48, C3),
   `ahead` (0 ms), `out` (0 dB, the contract's key kept).
2. **The bank is content, not parameters.** Thirty-six patterns, each a length (¼, ½, 1, 2
   or 4 bars of 4/4) and two curves of up to 64 points in Fluctus's model (place, value,
   tension, the automation editor's six shapes; the curve wraps). They live in
   `EffectSpec::lapsus: Option<Box<Bank>>` beside Aula's `ir`, holding only the slots that
   differ from the factory bank (`fx::lapsus::factory()`, forever once shipped); `None` —
   every spec until a slot is edited — is the factory bank and is not written. As
   parameters the bank would be ≈ 4 600 host parameters and a configure that reads them
   all on every automation step; as content, a host sees nine and the engine compares
   only the slots that are not the factory's. *Phase B*: the editing commands (with their
   exact inverses and undo-fuzz arms), the exported plug-in's state carrying the bank.
3. **The time curve's value is the delay**, in pattern lengths, behind the live signal:
   flat 0 is the signal as it arrives — and read as a whole sample, the same bits (the
   null); a slope is a speed (`1 − slope`); a held step is a repeat. The cycle is the
   pattern's length in quarter notes on the song sample, its bars on the samples the
   engine puts them on (the engine's own expression, `beats·60/tempo·rate` rounded), so a
   tape stop ends on the bar to the sample at any tempo; `retrig` starts it at a note.
4. **Nothing before the cycle's start is read** (it reads silence), and everything that
   carries state across samples starts again on each cycle's first sample from the
   pattern alone: the level's glide, a join's crossfade, the jump detector. So a render
   from bar 5 is the whole render's bars from 5 *to the bit, whatever the music before*
   (held for all 36 factory patterns through the engine, and on continuous music in the
   DSP's tests).
5. **Joins.** Inside a cycle, where the delay jumps (found from the pattern itself — the
   step there differs from both neighbours' — so a skip finds the same jumps a run does),
   the old read carries on at its speed and fades out over `xfade` against the new one (an
   equal-gain raised cosine). At the cycle's end the fade is placed *before* the line, onto
   the next cycle's first read, so the next bar starts on its line at full weight. A
   pattern switched by a note or the slot fades the same way.
6. **The history** is a stereo ring the unit writes every sample into, sized for the
   bank's longest pattern at 60 BPM plus the lookahead (one bar: 4 s), built off the audio
   thread through the memory path (`MemoryNeed::Lapsus`, ADR 0022) and swapped without
   allocating. Below 60 BPM a read further back than that is silence.
7. **The interpolator depends on how far behind *now* it reads**: a whole sample is that
   sample; under one sample a line; from two a four-point Hermite; from 16 a 32-tap
   Kaiser sinc — crossfaded across 1 … 2 and 16 … 24 samples, so a tape stop leaving now
   changes kernel without a step; read faster than real time, the cut-off follows the
   speed (kernels at 1, ½, ¼ of Nyquist, 32 / 64 / 128 taps). The fraction is taken from
   the delay alone, never from `now − delay`, so the same delay reads with the same
   coefficients anywhere in the song.
8. **Notes** (`TAKES_NOTES`): note `key + k` plays slot `k` — until another (switch) or
   while held (hold) — at its sample. **Lookahead** (`ahead`) delays the whole output (the
   reported latency) so the curve may read that far into the future.
9. **Exact rest**: the history holds only silence (all of what a read could see), no note
   chooses the pattern, a cycle has begun since anything that was not the pattern's own,
   the glides landed; a skip runs the level's glide and the pattern's own joins as a run
   would. At its defaults it rests after the history's length of silence (4 s).
10. **The face** (`kit/skin/side.rs`, `fx.lapsus`): a DJ's deck at midnight — petrol matte
    (192°), an amber VFD (36°) with the level in ice blue (190°), soft knobs, lamp keys, a
    painted stripe, vents and rules.

## Consequences

- `EffectSpec` has a field more (written only when a slot is edited); every struct literal
  of it names it. `MemoryNeed`, `NeedTicket` and `EffectMemory` have a Lapsus arm.
- Phase B: the editor's gestures as commands, the presets (≥ 12) and the starting preset,
  the export's state and note input, the docs, the site, the AI's words.

## What the tests hold (phase A)

- `cantus-dsp` `lapsus::tests` (21): the defaults and a flat curve through the full path
  touch no sample; half time reads at half speed (to 1e-4); a tape stop ends on the bar's
  sample at 60, 120 and 174 BPM; seven pattern families from bar 5 equal bar 5 of the whole
  render on continuous music; every block size the same bits with notes; joins: the second
  difference of a stuttered 220 Hz sine 0.0019 with 2 ms (the sine's own 0.0008) against
  0.058 hard; the sinc's error −95 dB at 1 kHz, −87 dB at 18 kHz; read at 2× the 15 kHz
  tone that would fold is −93 dB; the kernels hand over without a step; held notes, the
  release, retrigger; rest and a skip equal to silence processed; the history's size;
  lookahead a pure delay.
- `cantus-engine` `fx_lapsus` (9): null; mix 0 a wire; all 36 heard (Through not); blocks
  1 … 4 096; live = offline; all 36 from bar 5 to the bit; tape stop ¼ at 60/120/174 BPM;
  a track's notes; lookahead compensated. `fx_contract` (split invariance, exact rest at
  default and heavy), `rt_memory::lapsus_never_allocates`.
