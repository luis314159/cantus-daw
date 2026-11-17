# ADR 0132 — Saltus, pitch

**Status:** accepted (fx side round wave 3) ·
**Date:** 2026-11-15 · **Schema:** 73 ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0031 §5.1 (at rest), ADR 0053 (Tonus's PSOLA),
ADR 0071 (effects that sound on insert), ADR 0101 (presets), ADR 0131 (Tabula: every unit is
also a pedal), ADR 0135 (the user's own starting presets)

## Context

The owner's pedalboard (ADR 0131) needs an octaver, a pitch shifter and a whammy, and BACKLOG
§3b *Pitch and creative* asks for a harmoniser; he asked (2026-11-13) that every new engine
wave 3 makes be a full unit of the family as well as a pedal, and chose the name Saltus
(Latin, *the leap*).

## Decision

`EffectKind::PitchShift`, `fx::saltus`: one unit, four modes, three engines.

- **Modes** (`mode`): *Shift* (two voices at fixed intervals, ±24 st, ±100 ct), *Harmony*
  (two voices a number of scale degrees from the note heard — in a key and scale, in the
  chord held on MIDI, or singing the MIDI melody; the singer's intonation carried, the
  interval chosen by the scale and glided over 12 ms when it changes), *Octave* (one and
  two down, one up), *Whammy* (one voice between a heel and a toe interval, the pedal glided
  — or, `wsrc`, pressed to the toe while a MIDI note is held).
- **Engines**: *mono* — the pitch base's streaming tracker and pitch-synchronous PSOLA
  shifters (ADR 0053's), formants kept or moved, a *throat*; 128 samples at 48 kHz. *Poly* —
  a streaming phase-locked vocoder (4 096 points, hops of 512, the spectral envelope kept by
  the stretcher's true envelope) for chords and mixes; 4 095 samples (85 ms). *The divider*
  — a comparator on the fundamental behind a tracker-steered low-pass, sub-octave sines
  locked to its edges carrying the note's level, the octave up the fundamental rectified;
  no latency, as the analog octavers. Harmony runs mono; Octave runs the divider (*Analog*)
  or mono (*Clean*); Shift and Whammy choose.
- **The latency is fixed by the settings** (`latency_for`: mode, engine, octave style),
  never by the audio, reported through `FxUnit::latency`; the dry is delayed to line up.
- **The null**: at the defaults (Shift, mono, voice 1 at unison and full level, voice 2
  silent, no dry) the output is the input delayed by the latency, bit for bit: a level of
  exactly 1 is not multiplied, a pan of 0 not applied, a level of 0 not added, sums start at
  −0.0.
- **The table**: 29 keys (`cantus-core/src/fx/saltus.rs`), `out` kept from the contract;
  `TAKES_NOTES = true` (the harmony's chord and melody, the whammy's notes). Their order is
  what a host lists and what a pedal's row on Tabula shows after its face's knobs.
- **Starting preset**: *Octave up, twelve-string* (poly, +12 st at 60 % over the dry) — the
  owner's choice to keep; the factory presets come in both engines (*Fifth up, live* and
  *Octave down, live* are mono), and the user can choose Saltus's start (ADR 0135).
- **The window**: the leap (a keyboard, the note heard lit and named, an arc to each voice
  labelled with its interval and landing, draggable), a grand staff above it writing the
  last seconds and the notes now, each note falling to its key; the whammy's pedal drawn in
  depth; the modes as lit legends; the engine switch with each engine's latency. The face:
  graphite anodised, chartreuse and lilac on a VFD (`side.rs` `SALTUS`). On Tabula: the leap
  motif, the knobs Interval, Level, Dry and Pedal.

## Consequences

- Poly's 85 ms is felt when playing through it live; the engine switch puts the mono engine
  (2.7 ms) and the analog octave (none) one click away, each with its latency beside it.
- The harmony on a note outside the chord moves by the interval the snapped note would take
  (the singer's intonation kept): over C major, an A sung two chord steps up gives F♯; the
  display draws exactly what the engine sings.
- The tracker is the pitch base's (`pitch_stream`, shared with Tonus); a report that it can
  read an octave high was not seen on Saltus's material (a sung A3 reads A3) — Saltus does
  not work around it, and Tonus's files are untouched.
- The poly engine has no input level in its telemetry: the display does not dim it at rest.

## What the tests hold

- `cantus-dsp` (`saltus_tests*.rs`, 21): the null bit for bit; every other mode's controls
  unheard in Shift; each mode's latency at three rates; the reported latency is where the dry
  comes out; block-size independence and notes at their sample in every mode; exact rest
  (four twins) in every mode; extremes finite; a new engine starts as a new unit's; a copy is
  the unit; the whammy by notes. Measured: the shift lands within 0.16 cents across four
  octaves on sines; a voice's formants kept within 1.1 dB from −12 to +12 st; a shifted-down
  voice keeps its level within a dB; the octaver locks on a bass line; harmony in key and on
  a held chord; the whammy's glide without zipper; poly chord partials within 0.01 cent and a
  real strum's spectrum moved within 0.6 dB.
- `cantus-core`: the table (keys unique, defaults in range, stepped choices),
  `tests/saltus_commands.rs` (every key on an insert and on a Tabula pedal, at and past its
  ends, undoes, redoes and undoes exactly; the starting preset in range).
- `cantus-engine`: `fx::saltus` (the table's defaults are the DSP's, every key reaches the
  DSP, the defaults are the input delayed, the latency per setting, telemetry);
  `tests/fx_saltus.rs` (the dry mix delayed by its latency at the defaults and for every
  engine, block sizes 1–4096, live = offline, another track's notes heard, a board of one
  Saltus is the insert); `fx_contract` (split invariance, exact rest, a board of one);
  `rt_memory::saltus_never_allocates`.
- `cantus-ui`: `tests/saltus_e2e.rs` on the boot path (inserted as the picker inserts it,
  the twelve-string heard and late by the poly latency, the engine switched to mono, a fifth
  heard and undone to the bit, the null to the bit); the staff's places and gap; the face's
  distinctness (`side.rs`); the presets' tests; `start_menu_whole`.
- `cantus-plugins`: the CLAP/VST3 export equals the DSP (`common` `Direct::Saltus`); notes
  through both formats (`notes.rs`, the harmony singing the MIDI melody).
