# Round 42 — `strings-rec`: the recorded strings and the note's articulation (ADR 0158)

*The `strings-rec` builder's report. Branch `r42/strings-rec` from `r42/contract`.*

## What was built

### The note's articulation, end to end

- **Delivery** (`NoteEvent::art`, `cantus-core/src/snapshot.rs`): a note-on carries its
  `Note::art` (`None` on every note-off and on a note without one). The engine's `fire`
  (`processor.rs`, one marked line, the integrator's and `tuning`'s OK) hands it to the
  instrument just before `note_on_seq`, through `TrackSlot::note_art` (native units only: a
  hosted plug-in, Firmus and a kit hear nothing). `fire` is the one place an arrangement clip,
  a session clip's projection, an offline render, a freeze or a stem turns an event into a
  note-on. The note effects carry it on the notes they move (`notefx::Msg::art`; an arpeggio's
  own notes carry none; `processor_msg.rs` got one marked `art: None` for live input). The
  art is not in the events' sort key; equality includes it (a changed art is a changed
  snapshot).
- **The clip's look-ahead** (`note_art::tie_legato`, at compile, off the audio thread): a
  note marked *legato* that starts while no earlier note sounds, at most an eighth after the
  last one ended, ties that note to itself (its note-off one tick after the legato note).
  Lindemann's expired US 7,718,885 is the idea; nothing moves for a note without art.
- **Seek and chase**: Cantus does not chase notes (a note begun before the playhead is not
  started); every note a render or a seek starts carries its art (tested).
- **The piano roll's articulation lane** (`views/piano_roll_art.rs`): from the velocity row's
  **+** ▸ *Articulation*. Each note's marks at its start, drawn by hand as a score prints them
  (tenuto, slur, dot, wedge, a plucked ring, three tremolo strokes; ⊓ down-bow, V up-bow; a
  hollow ring for "the instrument decides"). A click on a note's mark — or a double-click on
  the note in the grid (FL's note properties gesture) — opens **the note's menu**:
  articulations and bows, the current checked; one `SetNoteArt` for the note or the selection.
  Shots: `docs/round42/strings-rec/`.

### Exemplar (schema 82)

- **Articulation and bow per zone** (`z.art`, `z.bow`; `exemplar_art.rs`): a note that names
  an articulation plays its zones (or the nearest the key has: staccato ↔ spiccato, legato →
  sustain, tremolo → sustain) whatever keyswitch is in force; a note with none follows the
  keyswitch, so a key-switched instrument is played both ways. The bow turns while the
  instrument sounds and starts down after a silence; a slurred note keeps its bow; a bow the
  key lacks plays the other, never both.
- **A continuous controller crossfade** (`z.xfcc`, `z.xfinlo` … `z.xfcurve`: SFZ's
  `xfin_loccN` … `xf_cccurve`): followed while the note sounds, folded into the layer's gain
  ramp (sample by sample) — never a switch at thresholds. Equal power by default. The tone
  that follows the dynamic is the existing modulation matrix (mod wheel → cutoff).
- **The scripted legato** (`legato`, `legato.xf`, `legato.glide`, `legato.skip`;
  `exemplar_legato.rs`): a sustained note started while another sustained note is held (≥ 20 ms
  before: a chord never slurs) crossfades out of it (`x(2 − x)` curves) and glides from its
  pitch, both lengthened by the interval (a twelfth per semitone, at most ×2), starting past its
  attack; the old note plays no release samples. A note marked *legato* slurs even with the
  switch off. Two ordinary samples, nothing stored or built after the note-on.
- **Release envelopes** per articulation group (the packs' SFZ: long for sustains, short for
  short notes).
- Schema 82: a migration that writes nothing, `fixtures/v82.cantus` (a *Strings* Exemplar),
  `schema_82_keeps_exemplar_s_articulations_and_legato`; every older fixture loads.
- The SFZ importer reads `xfin_loccN`…, `xf_cccurve`, `cantus_articulation`, `cantus_bow`,
  `cantus_legato*`; the zone card's title shows a zone's articulation and bow.

### The packs (on demand, credited, never in git)

FILLED BELOW

## How it is tested

FILLED BELOW

## Verified only in emulation

FILLED BELOW

## Left (the owner's)

FILLED BELOW
