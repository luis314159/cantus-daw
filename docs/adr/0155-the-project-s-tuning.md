# ADR 0155 — The project's tuning: one tuning every pitched instrument follows

**Status:** Accepted (round 42; the integrator, with the owner's mandate of 2026-11-22) ·
**Date:** 2026-11-22 · **Builds on:** ADR 0004 (commands), ADR 0005 (real time), ADR 0059 and
0098 (Chorda's keys and their tuning), ADR 0069 and 0153 (the song's key and how it reaches
units), ADR 0080 (per-note expression to hosted plug-ins) · **Schema:** 80

## Context

The owner asked for microtuning for professionals on 2026-11-13 (BACKLOG §0.2): Scala scales,
MTS-ESP, historical temperaments rotated to any key, just intonation per key, with an override
per instrument, as an advanced option that never crowds an ordinary user. The design note is
`research/instruments/microtuning.md`; its verification (formats, licences, byte layouts, the
temperaments' tables, what Live 12, Bitwig, Logic, Pianoteq and Surge XT do) is
`research/instruments/microtuning-verified.md`; where each instrument turns a key into a
frequency is `docs/round42/pitch-map.md`.

Every pitched instrument computes its pitch its own way (the map: Calor's polynomial exp2,
Firmus's `powf`, Exemplar's f64 ratio, Chorda's 88-key design, Organum's wheel increments).
A tuning must reach all of them without changing one sample of a song that uses none.

## Decision

1. **The model** (`cantus-core::tuning`):
   - `Tuning { a4_hz, system, map }`. `system` is one of: `Equal` (12 steps to the octave);
     `Temperament { id, key }` (a named historical or just table rotated so its best key sits
     on `key`, 0 = C); `Scale(ScalaScale)` (a `.scl`'s degrees, kept as written: cents or a
     ratio); `External` (MTS-ESP: the master's table, ADR 0156). `map` is an optional
     `KeyboardMap` (a `.kbm`: size, first/last key, middle key, reference key and frequency,
     formal octave degree, the mapping with unmapped keys).
   - `Project::tuning: Tuning` — default `Equal` at 440 Hz, not written (every older project
     is that).
   - `Track::tuning: TrackTuning` — `Follow` (the default, not written), `Equal` (this track
     ignores the project's: a drum bus, a sampled loop) or `Own(Tuning)`.
   - **The one function every unit reads:** `Tuning::offsets() -> TuningTable`, a `[f32; 128]`
     of semitones from 12-TET at A440 per MIDI key (a fractional key is the key's offset plus
     the plain fraction — a bend moves along 12-TET from the tuned key, as in Live and
     Pianoteq). At the default every entry is `0.0`, and `TuningTable::is_equal()` says so.
     An unmapped key of a `.kbm` sounds nothing (`f32::NAN` in the table; units skip the
     note).
2. **Formats tested with outside bytes:** Scala `.scl` and `.kbm` read and written (the
   grammar in the verified note, every quirk counted in the archive's 5 401 files), the MIDI
   Tuning Standard's bulk dump, single-note change and scale/octave messages read and written
   (three checksum conventions accepted on read, the spec's written). Tests use the MIT
   `tuning-library` (Surge) test files (committed with their notice) and the Scala archive
   version 94, downloaded by a test tool from its fixed Wayback copy and checked by
   SHA-256 `d2218f2f…b97b9a` — the archive has no licence, so none of its files is committed.
3. **The commands:** `SetProjectTuning { tuning }` and `SetTrackTuning { track, tuning }`,
   each the old value as its inverse, with undo-fuzz arms. A Scala import is a
   `SetProjectTuning` (or `SetTrackTuning`) holding the parsed scale: the song keeps the
   scale, never a path.
4. **The engine:** the snapshot carries each instrument track's resolved `TuningTable`
   (`Arc`, built off the audio thread); `InstrumentUnit::set_tuning(&TuningTable)`, a no-op
   by default, is called after `configure` (`apply_instrument`), in every offline copy
   (freeze, range renders, stems, the listen fork). Each instrument adds `table[key]` where
   its key first becomes a pitch (the map's insertion points), so at the default it adds
   `0.0` and keeps the bits; a multiplicative step (Organum's wheels, Lamina's ratio) is taken
   only when the offset is not zero. Chorda's tuning is part of its design key (a redesign
   off the audio thread). Pitched effects (Tonus's targets, Saltus's harmony, Nitor's tonic,
   Loquax's carrier) follow the strip's track's table. Hosted instruments receive a
   per-note tuning expression at note-on only when the offset is not zero (CLAP
   `NOTE_EXPRESSION_TUNING`, VST3 `kTuningTypeID`, the MPE route's member bend); a plain MIDI
   route gets MTS SysEx when the user turns it on. The MPE route's bend is a 14-bit member
   bend over ±48 semitones (the zone's range): steps of 0.586 cents, so a key lands within
   0.29 cents of its table (the 0.1-cent target is the native units'), and an offset beyond
   ±48 semitones (a far non-octave map) is clipped there — a plug-in that takes CLAP or VST3
   note expressions has neither limit. A hosted MTS-ESP client is sent only what MTS-ESP does
   not give it (ADR 0156).
5. **The surface:** Project ▸ Tuning (the system, the diapason with its presets 415–443, the
   temperament and its key, *follow the song's key*, import/export Scala, a circle of fifths
   coloured by each fifth's and third's beat rate, an audition), the track's override in its
   inspector, Chorda's Edit ▸ Tuning (its stretch and unisons stay its own); a one-line summary
   only when the tuning is not the default ("Werckmeister III in E flat · A415", the key named as the song's key is named — the fonts
have no ♯ or ♭).

## Consequences

- A song with no tuning is unchanged to the bit (null tests per instrument and an
  end-to-end one); `DSP_VERSION` stays 11.
- Old versions refuse a schema-80 song (as every bump).
- Unpitched instruments (Pulsus, Tympanum, a drum sampler at keytrack 0) are unaffected by
  construction: a keyboard map's unmapped key (`NAN`) silences a key only where it is pitched —
  Exemplar decides per zone (a zone at keytrack 0 plays every key; review correctness 6). A
  hosted drum plug-in cannot be told apart from a hosted synth: on a song with a map that leaves
  keys unmapped, set its track to *Equal temperament* (the track's own `Equal`), which plays
  every key as it is.
- A frozen render keeps the tuning it was made in: the region's tracks' resolved tunings (and
  the project's, where a pitched effect of the region follows it) are in its content key,
  written only where they are not the default (review correctness 4).
- Organum (one wheel serves several keys) and Lamina (keys folded by octaves) follow only the
  pitch class of a non-octave scale; they say so in their tuning summary.

## What the tests hold

Per instrument: the default table touches no sample; a tuned key's measured pitch equals its
table within 0.1 cent (Chorda's keys, every model instrument's, Exemplar's zones), 1 cent for
the bowed string; block-size independence and offline = real time with a tuning set; the
Scala parser over the whole archive (every file parses or is refused for a stated reason,
round-trip to the same cents) — a hand-run tool (`tools/test/scala-archive.sh`, the `#[ignore]`
test `tuning_scala_archive`), not part of CI, since the archive has no licence and is never
committed; in CI, Surge's MIT `tuning-library` files; MTS bytes from the MMA's published example
and Scale Workshop's exporter test (its dump's SHA-256 matched by ours); the end-to-end
test: a project in Werckmeister III at A415 plays Chorda, Exemplar and a hosted CLAP test
synth in tune with each other through `CantusApp::assemble`.
