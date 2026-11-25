# ADR 0162 — Arcus's player: the bow's state, the stroke's start and the fingering

**Status:** Proposed (round 42, the `arcus-expr` builder; the number is provisional, the
integrator's to confirm) · **Date:** 2026-11-25 · **Builds on:** ADR 0157 (Arcus), ADR 0158
(the note's articulation and bow) · **Schema:** 83 · **`DSP_VERSION`:** 15

## Context

The owner, 2026-11-25: "more expressiveness: different attacks, different vibratos, volume
swells … where the bow comes from, how it attacks, which part of the bow it plays with …".
Stage 1 (`docs/round42/arcus-expr.md` §2) performed the quartet demo with what a user can enter
and measured what was still the instrument's: it did not know where the bow was along the
stick, a stroke from silence always started the same way (pass 6's open `t_steady`: 0.08–0.13
s against the players' 0.24–0.42), every note was played in first position on the highest
string that reaches it (an open string's pitch, held, could not vibrate as a player's fourth
finger does), and nothing called a slur a shift. The strings survey
(`research/instruments-next/strings-realism-survey.md`, items 1, 2, 3, 7, 10) asked for **one**
definition of the bow's state and of a shift, shared by every later layer.

## Decision

- **`cantus_dsp::arcus::bow`** is that definition:
  - **The bow along the stick**, `x` from the frog (0) to the tip (1), moved by the stroke's
    speed over the hair's length (violin 0.65 m … bass 0.57). A stroke from silence starts at the
    frog for a down-bow and the tip for an up-bow, or the middle in a fast passage (an
    inter-onset under 0.22 s), or where the note's **part** says (frog, middle, tip, whole).
  - **The arm's weight**: the force is × `1 + 0.35·bow.weight·(0.5 − x)` (±17 % at the ends).
  - **Running out**: with 14 cm left the player slows the bow (down to 0.4 × its speed) and
    moves nearer the bridge by the same factor (`v/β` — the level — and Schelleng's force kept),
    and with 1.2 cm left it **changes bow** (the soft-edged change of pass 5).
  - **Where the bow comes from**: *from the string*, the force placed before the motion, the
    acceleration to 0.6 of the stroke's speed, then the stroke's **contour** — force and speed
    settling over 0.12 / 0.07 s at pp to 0.05 / 0.03 at ff (Maestre 2010's per-note contours,
    Demoucron 2008); *from the air*, the bow landing at 0.45 of its speed and the force arriving
    in 5 ms at 1.12 × the stroke's. The note's **start** chooses; without one, the player chooses
    from the air for a velocity ≥ 0.8 or a staccato, from the string otherwise.
  - **The fingering** (`bow::finger`, a first engine): first position on the highest string
    that reaches the note, except that an expressive note at an open string's pitch is stopped
    on the string below, a slur keeps its string for steps of up to a major third, and a note may
    name its **string** (honoured where it reaches, up to an octave over the open string). A slur
    on one string by three semitones or more is a **shift**: the finger takes 7 ms more per
    semitone past two (at most 110), the slur's dip lasts as long, and the vibrato stops and grows
    back. **An open string has no vibrato**; a fast détaché has 0.3 of it, a fast slur 0.7.
- **Two parameters** (`cantus_core::inst::arcus`): **`player`** (0 … 1, default 1: the layer
  above; 0 is pass 6's instrument to the bit) and **`bow.weight`** (0 … 2, default 1), on the
  editor's *direction* card.
- **The note's bow** (schema 83): `NoteArt` gains `start: Option<BowStart>` (`string`, `air`),
  `part: Option<BowPart>` (`frog`, `middle`, `tip`, `whole`) and `string: Option<u8>` (counted
  as MusicXML's `<string>` counts it: 1 the highest), each skipped when `None`; set by the same
  `SetNoteArt` (its undo-fuzz arm draws them), shown in the piano roll's note menu
  (*Where the bow comes from*, *Part of the bow*, *String*). The engine hands them in with the
  articulation (`NoteHow`); Exemplar and hosted plug-ins ignore them.

## How these map to round 43's playing profile and to MIDI 2.0

Each value is an **open id** stored by name (append only, never reordered): `bow.start.string`,
`bow.start.air`, `bow.part.frog|middle|tip|whole`, `string.<n>`. Round 43's adaptive piano roll
(BACKLOG *An instrument's playing profile*) lists, per instrument, the techniques it offers; a
bowed instrument's profile offers these ids, and a profile that does not (a piano) never shows
them. For MIDI 2.0, a note-on's *attribute* (type *profile specific*) carries an articulation
index under a profile; when the MIDI Association's orchestral articulation profile is adopted
here, each id maps to its index there by a table in the profile — the stored names do not change.
Today no file format carries them: MusicXML's `<string>` is the one a later writer can use
(`<technical><string>n</string>`), DAWproject and MIDI have none.

## Consequences

- Every Arcus note sounds different at the new default (the player on): `DSP_VERSION` 15,
  `upper_tests::LOWER_HASHES` re-made, pass 6's hashes kept as the player's null
  (`the_player_off_keeps_pass_six_s_bits`). Arcus has never shipped (`READY` false), so model 2
  changes in place, as passes 2–6 did.
- Live playing has no look-ahead: the player reads only what has happened (the last note-on,
  the slur's interval). Look-ahead on playback (the survey's item 7) is the next layer, on top of
  this state; so are the transitions by interval (item 1), the portamento's kinds (item 10), the
  playing characters (item 16) and a seeded identity per player (item 6) —
  `docs/round42/arcus-expr.md`, *Next*.

## What the tests hold

`cantus-dsp` `arcus::tests_expr` (the bow from the frog to the tip and changed before it runs
out on all four instruments; the force heavier at the frog; from the air moving at once, from
the string from rest and settling over its contour; a part puts the stroke there; the fingering's
open-pitch, named-string and shift rules; no vibrato on an open string in a passage; the note's
marks inert with the player off); `upper_tests::the_player_off_keeps_pass_six_s_bits` (the
null); every earlier Arcus gate at the new defaults (the Guettler attack limits, the pitch, the
block sizes, offline = real time, the pool, rest); `cantus-project`'s schema-83 fixture;
`cantus-core`'s undo fuzz; `cantus-ui`'s note-menu test and the i18n keys.
