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
  - **The arm's weight**: `1 + 0.15·(0.5 − x)` (±7.5 % at the ends) on the attack's force of
    the cello and the bass, from where the stroke starts (measured: without it the bass's soft
    E2 took 288 ms to settle against Guettler's 214; the violin's and the viola's soft attacks
    settle without it). Along a sustained stroke the player **makes up 85 %** of the fall
    (pronation and the index finger keep a held note even; the coordinator's call after the
    held-out third regressed with none made up), so the stroke's force is
    × `1 + 0.15·0.15·bow.weight·(0.5 − x)` (±1.1 %): `bow.weight` scales the uncompensated part.
  - **Running out** (built, measured, **off**): slowing the bow in its last 12 cm, nearer the
    bridge by the same factor, and changing bow with 1.2 cm left. Without a note's length the
    player changed bow in the middle of the gates' single 3–5 s notes, and the slow-down moved
    the bowed pitch (its tables measured at the dynamic's speed): both wait for look-ahead.
  - **Where the bow comes from**: *from the string*, Guettler's measured acceleration with the
    force placed before the motion, then a heavy attack's force **easing** into the stroke's over
    0.12 s (pp) … 0.05 s (ff) (Maestre 2010's per-note contours; a longer contour breaks the
    pitch gate until the pitch tables are measured with it); *from the air*, the bow landing at
    0.45 of its speed and the force arriving in 5 ms at 1.12 × the stroke's. The note's
    **start** chooses; without one, from the air for a staccato, from the string otherwise.
  - **The fingering** (`bow::finger`, a first engine): first position on the highest string that
    reaches the note, or the string the note **names** (honoured where it reaches, up to an
    octave over the open string). A slur on one string by three semitones or more is a
    **shift**: the finger takes 7 ms more per semitone past two (at most 110), the slur's dip
    lasts as long, and the vibrato stops and grows back. **An open string in a passage has no
    vibrato**; a fast détaché has 0.3 of it, a fast slur 0.7; a tremolo keeps pass 6's. (An open
    string's pitch held stopped on the string below, and a slur kept on its string, moved the
    bowed pitch 1.4 cents: they wait for pitch tables per string.)
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
