# ADR 0013 — A step is a note, a pattern is a clip, and the instrument names the lanes

**Status:** Accepted · **Date:** 2026-09-10 · **Extends:** ADR 0004 (command bus), ADR 0011 (dense views)

## Context

The ask was a drum kit with a good kick, snare and hi-hat, a piano roll whose keys are
renamed for percussion, and an FL Studio-style channel rack. FL's channel rack is the
reference most people arrive with, and it makes three structural choices that are worth
examining before copying, because each one is a second copy of something Cantus already
has:

1. **The step sequencer and the piano roll are two representations of a pattern**, and
   FL converts between them. A pattern that has been "opened in the piano roll" is no
   longer a step pattern.
2. **Patterns are a global bank.** Every channel shares one current pattern; patterns are
   then placed in the playlist as pattern clips. That is a second timeline model beside
   the playlist's own.
3. **Every drum is its own channel**, with its own mixer track. That buys per-drum
   inserts, and it means a kit is not an instrument but a folder.

Also on the table: how the piano roll should know that a track plays drums.

## Decision

### A step *is* a note

The channel rack is a second **editor** for the clip the piano roll is editing, not a
second data model. A step is a note — pad pitch, position `i × step`, a short fixed
length, a velocity — and switching a step on or off is `InsertNotes` / `RemoveNotes`,
exactly as drawing in the piano roll is. There is nothing to convert, so switching views
in the middle of an edit converts nothing and loses nothing, and the rack inherits undo,
save, the clipboard and every note operation for free.

`switching_between_the_rack_and_the_piano_roll_converts_nothing` asserts that a project
is byte-identical after any number of round trips between the two views, and
`a_step_is_a_note_and_switching_it_off_puts_the_clip_back` asserts that a step, on then
off, is the identity on what the engine plays.

### A pattern is a window of time across every track, and clips stay the unit of reuse

The rack lists **every track** and edits a **pattern region**: the window of the selected
clip — its start, and one period if it repeats or its whole length if not — on each track.
A row shows what its track's clip plays inside that window; switching a step on in a row
whose track has no clip there creates one, the pattern's size and repeat, in the same undo
step. FL's rack edits one global pattern; ours edits a bar of the song across the band.
Clips still do what patterns are for — duplicate, repeat, colour, name — so there is one
timeline model. This is the place Cantus deliberately does not copy FL, recorded so it is
a decision and not a discovery.

The consequence a user notices: the rack's step count follows the selected clip. A clip
that repeats shows one period; one that does not shows its whole window. Anything else
would mean the rack and the arrangement disagreed about how long the thing being edited
is.

### A kit is an instrument, pads are pitches — and a channel is a track

**Pulsus** is one `InstrumentSpec` variant, with a pad per pitch on the General MIDI drum
map. Everything already built applies unchanged: one clip, one note list, the same
commands, the same snapshot. Pads carry their own **level, pan and choke group** inside
the kit, because placing a kit's parts relative to each other is part of the instrument.

**Revised the same day it was decided.** The first cut said *one kit on one track, always*,
and deferred per-drum routing to buses. That threw away the thing FL's channel rack is
actually for: a channel with its own sound, level and route into the mixer. The revision
is that a channel **is a track** — Cantus already has the object: an instrument, a mixer
strip, inserts, clips — so a drum that needs its own strip is a track with a **one-pad
kit** (`PulsusParams::single`), and a whole kit on one strip is a track with seven. The
same type, the same rows in the rack, the same edits; the musician chooses the layout.
The welcome project opens with kick, snare and hi-hat as three tracks for exactly this
reason. Routing *several* tracks into one mixer channel is a bus, and buses are the next
block; each track already lands on its own strip, and the rack shows the strip number so
the route is visible before it is editable.

A pad's sound is a **voice**: three synthesized shapes, or `Sample` — a WAV the pad plays
back, retuned by the pad's tune, choked and muted like any other. Sampled and synthesized
pads live in one kit, so "use samples or generate sounds" is a per-pad choice and not two
instruments. The file's path is stored as given; a file that has moved makes the pad
silent and the inspector say so, never a project that will not open. Decoded audio rides
in the snapshot (`TrackSnapshot::samples`, filled by the engine's `SampleCache` before the
snapshot is sent), so real-time playback and offline render see the same buffers, and the
cache never evicts, so an `Arc` dropped on the audio thread is never the last one.

A muted pad is resolved when the snapshot is compiled, like a muted track: the audio
thread is handed events with the pad already gone, and never consults a kit per note.

### The instrument names the lanes

`InstrumentSpec::lanes()` returns a [`Lanes`]: `Chromatic` for a keyboard, `Pads(&[DrumPad])`
for a kit. The piano roll and the channel rack build their vertical axis from it — lane →
pitch, pitch → lane, lane → name — plus a row for any pitch the clip holds that the
instrument has no lane for, so changing a track's instrument can never make music
invisible. Drum mode is therefore a property of the *kit*, not a switch the user has to
find, and a sampler with named zones later adds a variant and changes no editor.

The plan had asked for `lanes() -> Option<&[Pitch]>`. It is `Lanes` instead because an
editor needs all three mappings and the keyboard fallback for each; handing back a slice
of pitches would have pushed the other two, and the "is this a drum or a piano" branch,
into every call site.

Two things follow that were not in the plan and are decisions:

- **Vertical moves are in rows.** Dragging a note up, or pressing the transpose key, moves
  it to the next *lane*. On a keyboard that is a semitone; on a kit it is the next pad,
  because "a semitone" is a pitch the kit has no drum for. The octave keys move four pads
  — a row of a 4×4 pad grid — since neither an octave nor a semitone names anything on a
  drum kit, and a key that silently did nothing would be worse.
- **A drum's note has a fixed short length** (a 1/16, or the grid if finer). A drum has no
  note-off worth aiming at; a note as long as the grid would tile the row into a solid bar
  at a coarse grid and stop reading as hits.

## Consequences

- `SetInstrumentParam` addresses an `InstrumentParam` — `Firmus(FirmusParam)` or
  `Pulsus { pad, field }` — rather than a `FirmusParam`. A second instrument did not need
  a second command, and neither will a third. `SetPadName` renames a pad **in the kit**,
  so the piano roll's lane and every clip on the track pick it up at once.
- Schema 6 is the arrival of the `Pulsus` variant. Nothing in a schema-5 file changes
  shape; the version number is what makes an older build say *saved by a newer Cantus*
  instead of failing on an unknown enum variant. Schema 8 is `PadVoice::Sample` and
  `DrumPad::sample`.
- `SetInstrument`, `InsertPad`, `RemovePad` and `SetPadSample` are commands like the rest,
  and the undo fuzz runs them. It caught the first `PadField::Voice`, which also reset the
  pad's decay: the inverse restored the voice and not the decay, so undo was not exact. A
  parameter set changes exactly one field.
- The audio thread holds **both** engines per track slot, preallocated, and switches with
  a flag. A boxed trait object would have meant an allocation on the audio thread the
  first time a track changed instrument (ADR 0005). It costs a few kilobytes per slot.
- Pulsus renders **stereo**, because its pads are panned. Firmus stays mono and is copied
  into both channels; from the polarity switch onwards the two share a signal path, and
  the null tests cover a kit at every block size, in real time against offline, and with
  every pad centred producing bit-identical channels.
- The rack's pads × steps grid goes into one `Mesh` with a level-of-detail rule from its
  first line (ADR 0011), and a test asserts the shape count rather than a frame time, so
  it stays honest on a loaded machine.
