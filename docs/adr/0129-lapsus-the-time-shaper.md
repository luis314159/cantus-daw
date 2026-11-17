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
Fluctus (round 28) shapes volume only; nothing in Cantus bent time. The owner approved the
design sheet with changes (2026-11-13): separate banks as Gross Beat's, finer strokes, the
palette kept, the display dark in every theme, and the sound heard on a real guitar.

## Decision

1. **`EffectKind::TimeShaper`, `fx::lapsus`.** Eleven parameters, every default a null:
   `tslot` and `vslot` (0 … 35; slot 1 of each factory bank is *Through*), `mix` (100 %),
   `xfade` (2 ms), `smooth` (1 ms), `notes` (off / switch / hold — hold), `retrig` (off),
   `tkey` (60, C4) and `vkey` (24, C1), `ahead` (0 ms), `out` (0 dB, the contract's key).
2. **Two banks, as Gross Beat's** (the owner's verdict): thirty-six **time** patterns and
   thirty-six **volume** patterns, chosen apart and played together — each by its knob
   (automatable), its pads, or a note of its own range. Each pattern is a length (¼, ½, 1, 2
   or 4 bars of 4/4) and a curve of up to 64 points in Fluctus's model (place, value,
   tension, the automation editor's six shapes; the curve wraps), on **a cycle of its own**.
3. **The banks are content, not parameters.** They live in `EffectSpec::lapsus:
   Option<Box<Bank>>` beside Aula's `ir`, holding only the slots that differ from the
   factory banks (`fx::lapsus::factory(lane)`, forever once shipped); `None` — every spec
   until a slot is edited — is the factory's and is not written. As parameters the banks
   would be thousands of host parameters re-read on every automation step; as content a
   host sees eleven, and the engine compares only the slots that are not the factory's.
   **`Command::SetLapsusPattern`** sets one slot of one bank (`None`: the factory's), its
   inverse the slot as it was (an emptied bank goes back to `None`); the undo fuzz, freeze
   and record-mode arms include it. Schema 62 (the side round's) carries a bank in its
   fixture.
4. **The time curve's value is the delay**, in pattern lengths, behind the live signal:
   flat 0 is the signal as it arrives — and read as a whole sample, the same bits (the
   null); a slope is a speed (`1 − slope`); a held step is a repeat. A cycle is the
   pattern's length in quarter notes on the song sample, its bars on the samples the engine
   puts them on (the engine's own expression, `beats·60/tempo·rate` rounded), so a tape
   stop ends on the bar to the sample at any tempo; `retrig` starts a lane's cycle at a note.
5. **Nothing before the time cycle's start is read** (it reads silence), and everything
   that carries state across samples starts again on its own lane's cycle start from the
   pattern alone: the joins and the jump detector on the time cycle's, the level's glide on
   the volume cycle's. A render from bar 5 is the whole render's bars from 5 *to the bit,
   whatever the music before* — held for every pattern of both banks through the engine.
6. **Joins.** Inside a cycle, where the delay jumps (found from the pattern itself — the
   step there differs from both neighbours' — so a skip finds the same jumps a run does),
   the old read carries on at its speed and fades out over `xfade` (an equal-gain raised
   cosine). At the cycle's end the fade is placed *before* the line, onto the next cycle's
   first read, so the next bar starts on its line at full weight; the level's own fade to
   its next cycle is no longer than a time join, so a level rising on a line rises with the
   read that joins there. A pattern switched by a note or a knob fades the same way.
7. **The history** is a stereo ring the unit writes every sample into, sized for the time
   bank's longest pattern at 60 BPM plus the lookahead (one bar: 4 s), built off the audio
   thread through the memory path (`MemoryNeed::Lapsus`, ADR 0022) and swapped without
   allocating. Below 60 BPM a read further back than that is silence.
8. **The interpolator depends on how far behind *now* it reads**: a whole sample is that
   sample; under one sample a line; from two a four-point Hermite; from 16 a 32-tap Kaiser
   sinc — crossfaded across 1 … 2 and 16 … 24 samples, so a tape stop leaving now changes
   kernel without a step; read faster than real time, the cut-off follows the speed
   (kernels at 1, ½, ¼ of Nyquist, 32 / 64 / 128 taps). The fraction comes from the delay
   alone, never from `now − delay`, so a delay reads with the same coefficients anywhere.
9. **Notes** (`TAKES_NOTES`): note `tkey + k` plays time pattern `k`, `vkey + k` volume
   pattern `k` — until another (switch) or while held (hold) — at its sample. **Lookahead**
   (`ahead`) delays the whole output (the reported latency) so the time curve may read
   that far into the future.
10. **Exact rest**: the history holds only silence (all a read could see), no note chooses
    a pattern, each lane's cycle has begun since anything that was not its pattern's own,
    the glides landed; a skip runs the level's glide and the time lane's own joins as a run
    would. At its defaults it rests after the history's length of silence (4 s).
11. **The window** (the design sheet approved with the owner's changes): the timeline
    draws both patterns over the longer cycle (the shorter repeated) — the time curve above
    over the cycle's input, the level below — with the playhead and **the slip** (the line
    from where the song is to where it is read, on the ruler); points dragged, added and
    removed, segments bent, the pencil's steps, each one undo step. The banks card shows one
    bank at a time behind a Time / Volume switch naming what each plays, the pads drawn as
    their curves (a right-click restores the factory's), the keyboard with both note ranges.
    Strokes are fine (hairline grids, a 1.4-point curve, hollow handles, a pixel playhead);
    the display is a dark VFD in every theme.
12. **The face** (`kit/skin/side.rs`, `fx.lapsus`): a DJ mixer at midnight — a dark
    teal-petrol body (174°) in matte rubberised paint, the time curve amber (36°) on a VFD,
    the level and the controls ice blue (208°), soft knobs, lamp keys, a painted stripe and
    printed rules. *Amended after the integration review:* the first face was petrol under
    smoked glass with an amber accent — Fragor's material and accent — and its ice blue
    (190°) and body (192°) were Cribrum's cyan and its Petrol colourway's body. Amber is
    taken in every finish but paper, so the ice blue became the accent and the amber the
    second voice (the views draw the time curve in the second voice), on matte, which no
    unit wears near 208°. `side.rs`'s tests hold it: no face of the round shares a material
    and an accent within 20° with any unit or colourway, and no two of the round's sheets
    share both voices or a tinted body under a shared voice (15° bands).
13. **Presets and start** (ADR 0071, 0101): seventeen factory presets pairing a time
    pattern and a volume one (*Tape stop* with *Fade the last beat*: a tape head's level
    falls with its speed); an insert starts on **Half time**.
14. **The exported plug-in** carries the banks beside its values: kept on the main thread
    (the editor's edits, a state loaded), posted to the processor through a lock-free ring
    and swapped in at its next block, the old one handed back to be freed off the audio
    thread (`cantus_plugins::lapsus_bank`); saved in the state as the project saves it.

## Consequences

- `EffectSpec` has a field more (written only when a slot is edited); every struct literal
  of it names it. `MemoryNeed`, `NeedTicket` and `EffectMemory` have a Lapsus arm; `FxHost`
  has `set_lapsus_pattern` (a no-op for every other editor).
- Patterns' lengths count quarter notes from the song's start (4/4 bars): in 3/4 a one-bar
  pattern does not follow the bars. A later decision.

## What the tests hold

- `cantus-core`: the defaults a null, the keys, the two note ranges apart, both factory banks
  within the model (each pattern but *Through* moves something), a bank holding only what
  differs; the undo fuzz with `SetLapsusPattern`. `cantus-project`: schema 62 carries a bank.
- `cantus-dsp` `lapsus::tests` (22): the defaults and a flat curve through the full path
  touch no sample; half time reads at half speed (to 1e-4); a tape stop ends on the bar's
  sample at 60, 120 and 174 BPM; seven pattern families from bar 5 equal bar 5 of the whole
  render on continuous music; every block size the same bits with notes; joins: the second
  difference of a stuttered 220 Hz sine 0.0019 with 2 ms (the sine's own 0.0008) against
  0.058 hard; the sinc's error −95 dB at 1 kHz, −87 dB at 18 kHz; read at 2× the 15 kHz tone
  that would fold is −93 dB; the kernels hand over without a step; held notes on each bank,
  the release, retrigger; rest and a skip equal to silence processed; the history's size;
  lookahead a pure delay; the two lanes on cycles of their own.
- `cantus-engine` `fx_lapsus` (9): null; mix 0 a wire; every pattern of both banks heard
  (Through with Through not); blocks 1 … 4 096; live = offline; every time pattern with
  Through volume, every volume pattern with Through time and four combinations from bar 5 to
  the bit; *Tape stop ¼* with its fade at 60/120/174 BPM; a track's notes; lookahead
  compensated. `fx_contract` (split invariance, exact rest at default and heavy),
  `rt_memory::lapsus_never_allocates` (a history swapped in included).
- `cantus-ui`: `fx_lapsus` (pads of each bank, a pad's right-click, a point dragged / added
  / removed and the pencil — each one undo step —, the presets in both languages and the
  starting preset); `lapsus_e2e` (through `CantusApp::assemble`: inserted on its starting
  preset it is heard in the master; a flat time curve is the dry song to the bit; undone,
  half time again to the bit; the trance gate shuts the master in sixteenths).
- `cantus-plugins`: the CLAP/VST3 suites bit-identical to the engine's unit; `notes.rs`
  (notes switching both banks through CLAP, both dialects, and VST3, at their samples);
  `lapsus_bank` (a bank posted reaches the processor and the old one comes back).
