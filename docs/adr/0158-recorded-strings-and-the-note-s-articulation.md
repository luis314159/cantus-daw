# ADR 0158 — Recorded strings, and the note's articulation and bow

**Status:** Accepted (round 42; the integrator, with the owner's mandate of 2026-11-22) ·
**Date:** 2026-11-22 · **Builds on:** ADR 0030 (packs on demand), ADR 0042 (CC0 and CC BY with
credit), ADR 0157 (Arcus) · **Schema:** 81 (the note's articulation, with Arcus's kind); 82
reserved for this builder

## Context

The owner's strings are "modelled and recorded" (BACKLOG §3e). The research of 2026-11-22
(`research/instruments-next/strings-verified.md` §2) verified the CC0 content file by file:
VSCO 2 CE's strings (violin, viola and cello sections, a solo violin and a solo bass; sustain,
spiccato, pizzicato, tremolo; two dynamic layers, one or two round robins, sustains unlooped
10–14 s, file names an octave low), Karoryfer's Bigcat cello (four dynamics, up and down bows,
staccato with four round robins, pizzicato) and the Smolken double bass (five dynamics, up and
down bows, pizzicato). VCSL has no bowed strings. There is no CC0 solo viola. Honestly, two
layers and no recorded legato reach "a finished sketch", not BBC SO; the card says so.

Both voices need a note to carry how it is played: an articulation, and for bowed strings the
bow's direction. Nothing in the note does today (`Note` holds velocity, release, mute and the
MPE curves).

## Decision

- **The note's articulation** (schema 81): `Note::art: Option<NoteArt>`, `NoteArt {
  articulation: Articulation, bow: Option<Bow> }` — `Articulation` an open list (`Sustain`,
  `Legato`, `Staccato`, `Spiccato`, `Pizzicato`, `Tremolo`, appended only, never reordered),
  `Bow` `Up`/`Down`. `None` — every older note — is "the instrument decides". Set by
  `SetNoteArt { track, clip, arts }` — `arts` a list of `(NoteId, Option<NoteArt>)`, one per
  note, refused whole for a note not in the clip or named twice (the old values its inverse), shown and edited in the piano
  roll as a note property (a lane of articulation marks, and the note's menu); the engine hands
  it to the instrument before the note starts (`InstrumentUnit::note_art`, a no-op by default).
  A hosted instrument gets nothing (no standard carries it; a key-switch map is later work).
- **The recorded strings** are Exemplar presets over packs on demand (ADR 0030), credited on
  their cards: VSCO's sections and solos, the Bigcat cello, the Smolken bass (Meatbass and
  Sneakybass as extras). The work Exemplar gains for them: loops found offline and stored as
  the pack's metadata (never computed on the audio thread); articulation by key-switch **and**
  by the note's `art`; a **continuous** CC1 crossfade between dynamic layers with a tone filter
  that follows it (never a stepped switch at thresholds — Fable Sounds' US 7,915,514 is avoided
  this way); release envelopes; a scripted legato by overlap — the next note's attack crossfaded
  and glided by its interval, with look-ahead from the clip's notes when they are known (no
  stored transition "joint" built after a note-on — Yamaha's US 7,557,288 is avoided).

## Consequences

- Old versions refuse a schema-81 song. Notes without `art` play as before, to the bit.
- The packs never enter the repository; their licences and credits are shown on install.
- Both patents above are listed for a lawyer before release (`docs/beta-checklist.md`, Ronda 42).

## What the tests hold

`SetNoteArt`'s undo-fuzz arm; a note without art changes no sample; each pack's preset
measured against its source (pitch per zone ±1 cent after the files' octave fix, loops
click-free by an offline spectral check, CC1 crossfade monotone in level); an end-to-end test
that installs a pack from a local file through `CantusApp::assemble`, writes a four-bar line
with a legato pair and a CC1 swell, and hears it.

## Amendment (round 42, `strings-rec`): how it was built

- **Delivery.** The note's art rides in the snapshot's `NoteEvent` (`art`, `None` on every
  note-off and on a note without one; not part of the events' order). The engine's `fire` —
  the one place an arrangement clip, a session clip's projection, an offline render, a freeze
  or a stem turns an event into a note-on — hands it to the instrument just before the
  note-on; the note effects carry it on the notes they move (`notefx::Msg::art`), not on an
  arpeggio's own. Live input carries none. A hosted plug-in, Firmus and a kit hear nothing.
- **The clip's look-ahead** (`note_art::tie_legato`, when the snapshot compiles): a note
  marked *legato* that starts while no earlier note sounds, at most an eighth after the last
  one ended, ties that note to itself (its note-off one tick after the legato note's start).
- **Exemplar (schema 82).** A zone's articulation and bow (`z{n}.art`, `.bow`), its continuous
  controller crossfade (`.xfcc`, `.xfinlo` … `.xfouthi`, `.xfcurve`: SFZ's `xfin_loccN` …,
  `xf_cccurve`), and the scripted legato (`legato`, `legato.xf`, `legato.glide`,
  `legato.skip`), all defaulted so every older instrument plays as it did. A note's
  articulation chooses its zones (the nearest the key has when it lacks it); with none, the
  keyswitch does. The bow turns while the instrument sounds and starts down after a silence; a
  slurred note keeps the bow it slurs from.
- **The packs** carry their zones measured offline (`stringsbuild`, manifest format 6): the
  root each sample sounds (the octave fix: VSCO 2 CE's and the Bigcat cello's names are an
  octave low), the cents to its key, the loop kept only when Exemplar plays it click-free;
  the install writes them as plain SFZ with Cantus's three opcodes (`cantus_articulation`,
  `cantus_bow`, `cantus_legato*`).
