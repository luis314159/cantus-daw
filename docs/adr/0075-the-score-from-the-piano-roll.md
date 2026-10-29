# ADR 0075 — The score from the piano roll

**Status:** accepted (round 32) · **Date:** 2026-10-28 · **Extends:** ADR 0004 (commands and
undo), ADR 0024 (the kit), ADR 0060 (the tempo map), ADR 0069 (the song's key) · schema
45, keymap file version 13

## Context

BACKLOG §0.2 *Notation* and §0.4 item 10: Logic, Cubase and Studio One show a part as a
score; Cantus had only the piano roll. The owner's bar: a score **well engraved, not only
correct** — Dorico, MuseScore 4, Sibelius and Gould's *Behind Bars* as the reference —,
MusicXML and PDF out, printing, editing on the staff if it fits with quality.

## Decision

1. **A crate of its own, `cantus-score`** (GPL-3.0-or-later; no egui). Notes in, a
   **display list in staff spaces** out (`display::Item`: glyphs, square-ended lines,
   beams, tapered curves, words, the brace) — what the screen, the PDF and the golden tests
   share, so what is tested is what is shown and printed. The pipeline:
   - **Reading** (`read`): notes rounded for reading only. Each quarter (a dotted beat in
     compound metres) weighs straight 16ths and 32nds, triplets, sextuplets and quintuplets
     by how far its onsets move plus a price per onset for the rarer reading; ends snap to
     the grid of the slot they end in; two triplet quarters a note crosses merge into a
     triplet over the half note (quarter triplets). The track's **Read as** overrides it.
   - **Staves and voices**: a grand staff on *Auto* when the part spans both hands; the
     hands' split (`hands`) follows them — each hand's centre and last onset, a hand
     holding a chord less likely to move on, a span past a tenth expensive — or a fixed
     note. Voices (`voices`): one start and end make a chord; a line against a held note is
     a second voice (stems down), the higher line keeping the first; a legato overlap of a
     32nd or less is the same line.
   - **Rhythm** (`rhythm`, Gould's rules): a value is written whole when it exists and
     hides no point stronger than where it starts, or as the centred syncopation
     (quarter–half–quarter, eighth–quarter–eighth in a half bar, sixteenth–eighth–sixteenth
     in a beat), or a dotted value inside a beat ending on it (the Scotch snap); otherwise it
     is cut at the strongest point it hides and tied. Rests are stricter (no syncopation, no
     dotted rest in simple time, a rest over a beat only on the half bar; a whole-bar rest
     whatever the metre).
   - **Spelling** (`spell`): the key signature from the song's key (a mode's parent major,
     a minor scale's relative major); diatonic notes as the key spells them, chromatic ones
     by their degree (♯1 ♭3 ♯4 ♭6 ♭7 in major; ♭2 ♯3 ♯4 ♯6 ♯7 in minor) and by the line
     they move in; the smaller alteration first (a mode's own letters with a natural:
     F♮ in C Lydian, B♭ in D Dorian); never a double accidental a single one spells. Accidentals per bar and
     line, none on a tie's continuation, courtesy ones in parentheses after a bar that
     altered the note and for another octave altered in the bar.
   - **Marks** (`staff_marks`): beams by the metre's units (each beat; four plain eighths in
     each half of 4/4; dotted beats; 2 + 2 + 3), hooks toward the note they belong to; stems
     by the farthest note from the middle line; tuplet brackets only when the tuplet is not
     one beam; octave lines for runs past four ledger lines; dynamics from velocity with
     hysteresis (a step that the next attack confirms, or two at once).
   - **Clefs by range**: *Auto* takes the clef with the fewest ledger lines, octave clefs
     only when they save a good many.
   - **Layout** (`layout`): columns shared by every staff; space by duration, √2 more per
     doubling (Dorico's default ratio) from 3.8 spaces a quarter, never less than what keeps
     neighbours apart (accidentals, flipped seconds, dots, flags, ledger lines); systems
     broken by dynamic programming over the whole piece (a stretched system costs, a
     compressed one more, a last system under two thirds full costs too; a last system at least
     half full is justified like the others — Dorico's default — and a shorter one keeps its
     natural spacing); staves stacked
     by skylines, not boxes. Beams after *Behind Bars*: slope by the outer interval (¼ space
     for a second up to one space), capped by length, flat over a concave line; the shortest
     stem three spaces plus ¾ per extra beam; notes off the staff reach the middle line;
     inside the staff every beam end sits, straddles or hangs (no quarter-space wedge). Ties
     away from the stem (a chord's upper half up, lower down), Bravura's 0.1 / 0.22 space
     thickness. Accidentals stacked in columns, highest then lowest inward, a seventh apart
     sharing one. Three formats: **Continuous** (the staff under the piano roll), **Wrap**
     (the Score pane) and **Pages** (A4 / Letter, 1.75 mm staff space, title and composer,
     page numbers, full pages spread to the bottom margin).
   - **Bravura 1.482** (SIL OFL 1.1, `assets/`, in `THIRD-PARTY-NOTICES.md`): its
     `engravingDefaults`, glyph boxes and stem anchors read from its metadata into a
     generated table (`tools/score/gen_bravura.py`). The brace is drawn from its outline,
     stretched far taller than wider (a font renderer scales evenly).
2. **MusicXML 4.0, partwise**, written from the logical score — values, ties, tuplets,
   beams, stems, accidentals, clefs, key, metres, tempo, octave lines, dynamics,
   transposition — and each note's velocity in its `dynamics` attribute (a percentage of
   MIDI forte, two decimals, so it reads back exactly). Read back (`musicxml_in`) as a part's
   notes on the timeline (ties joined, transposition undone, the concert key). The files
   validate against the MusicXML 4.0 XSD (`tools/score/validate_musicxml.ps1`, .NET's
   validator); MuseScore 4 itself was not run on this machine (see the round's report).
3. **PDF** (`pdf`, pdf-writer and subsetter, MIT/Apache): vector pages from the same
   display list; Bravura's CFF and the text faces' TrueType embedded as tagged subsets, CID
   fonts with ToUnicode maps (the words can be searched and copied); the project's name and
   the export's *Artist* as title and author. Every filled shape winds the same way (one
   nonzero path: overlaps never cancel). **Print** writes that PDF to the temporary folder
   under a name of safe characters only (`cantus-score-<pid>-<name>.pdf`) and hands it to
   the system on a thread of its own: on Windows PowerShell's `Start-Process -Verb Print`
   (the shell's print verb, no console window; `cantus-ui` forbids `unsafe`, so no
   `ShellExecuteW`), else `explorer.exe` opens it; `lp` (CUPS) on Linux and macOS, else
   the PDF viewer. Never `cmd`. The paper is the region's (Windows' *Region* setting,
   `LC_PAPER`/`LANG` elsewhere): Letter in the US, Canada and the Americas that follow
   them, A4 elsewhere.
4. **The model** (schema 45): `Track::score: ScoreSettings` — clef (or *Auto*), staves,
   the grand staff's split (or the hands), transposition (written = sounding + n, ±36),
   dynamics from velocity, the reading grid, the part's name. The default is every older
   track's and is not saved. One command, `SetTrackScore`, swaps the whole settings (its
   inverse the old ones; a name typed merges into one step); it never changes what plays,
   so the freeze key leaves it out. A hand-edited file's invalid settings are repaired on
   load.
5. **The view**: the piano roll's **Score** switch (`Alt+S`, *Show the score under the
   piano roll*, keymap 13) opens the clip's part as one long system under the roll, its
   height dragged; the **Score pane** (*View ▸ Score*, `Alt+Shift+S`) shows the selected track's part in
   wrapped systems. Both are cached per edit (`bus.revision`), size and composer — a
   200-bar part engraves once and then only paints what is in view. They follow the
   playhead, show the piano roll's selection, select on a click (in the pane, the note's
   clip with it), scroll and zoom (Ctrl+wheel, − / +). Drawn by `views::score_paint` with
   egui's text for glyphs (Bravura as a music font role in `design.rs`), rectangles for
   lines, polygons for beams and feathered meshes for ties; in the kit's tokens in both
   themes.
6. **Editing on the staff**, MuseScore's way, every edit a command on the notes (`Batch`
   labelled `cmd.score.*`): a click selects; ↑ ↓ a degree of the song's key (a semitone
   without one), Ctrl an octave; Delete; **Input** (N): 3–7 the value, `.` a dot, A–G a note
   in the key at the cursor in the octave nearest the last one (the written letter in a
   transposed part), the cursor moving on, a click moving it; only inside the clip's
   window (input stops at its end). The keys are the score's only while it was clicked last
   and is on screen, taken before the keymap's with exactly their modifiers (Shift+↑ still
   transposes an octave, Alt+C still toggles the count-in, Esc outside note input still
   clears the selection). Each settings change is its own undo step.
7. **MusicXML in** as clips: each part a new track holding one clip, one undo step; into
   a song with no notes yet the file's tempo, metres and key come in the same step (each
   only where the song accepts it), a song with notes keeps its own. A modal key is
   written and read as its mode (`dorian`, `lydian`…).

## Consequences

- A track's part is readable and printable without leaving Cantus, and travels to every
  notation program through MusicXML.
- Slurs, articulations, lyrics, chord symbols and grace notes have no source in a
  track's notes; the display list has the curve a slur needs, but nothing writes one yet.
  Key changes mid-song do not exist in the model (one song key, ADR 0069).
- More than two voices on a staff are folded into two (the earlier note of a third line
  shortened).
- The interface's Score pane borrows the piano roll's glyph until the kit has a score
  glyph of its own (the kit is the skin builder's this round).

## The review (fix round)

Nine findings, each with a test written first and seen failing: the PDF's mixed windings
(`every_filled_shape_winds_the_same_way_and_overlaps_stay_black`); the print path's shell
(`the_print_file_name_is_safe`); a second-voice note dropped
(`three_ends_at_one_onset_keep_every_note`, the `voices` golden passage); the staff's keys
taking more than theirs (`the_staff_takes_only_its_own_keys`); settings merged into one undo
step (`each_settings_change_is_one_undo_step`); modal spelling and modes
(`modes_spell_their_chromatic_notes_by_the_signature`, `a_modal_key_keeps_its_mode`); note
input outside the window and transposed letters
(`note_input_stays_in_the_window_and_reads_written_letters`); an onset at the very end of the
last bar (`a_note_at_the_very_end_of_the_last_bar_is_drawn`); the import's tempo, metres and
key and the region's paper (`an_import_into_an_empty_song_brings_its_tempo_metre_and_key`,
`the_default_paper_follows_the_region`).

## What the tests hold

- `cantus-score/tests/engraving.rs`: the seven golden passages (a melody, a two-hand
  piano piece, triplets, syncopation, long tied notes, a metre change with a tempo change,
  a high line under an 8va) as display-list snapshots, wrapped and on A4
  (`CANTUS_SCORE_BLESS=1` writes them anew); nothing collides; every note drawn and every
  stem between 2.5 and 9 spaces; the page's maps agree with its ticks; nothing off the paper.
- `cantus-score/tests/musicxml_roundtrip.rs`: every passage written and read back gives its
  notes, velocities, metres, tempo and key; a played (jittered) part reads back as it was
  read and lands on its grid; measures fill exactly.
- `cantus-score/tests/pdf.rs`: A4 and Letter read back by lopdf — page sizes, four fonts
  embedded as tagged subsets far smaller than their faces, ToUnicode, title and composer
  found in the text; a long part makes several pages.
- Unit tests of reading, metre, rhythm, spelling, hands, voices, chord shapes, beams,
  skylines and the brace outline.
- `cantus-project`: `schema_45_keeps_score_settings` (and a hand edit repaired).
- `cantus-core/tests/undo_invariants.rs`: `SetTrackScore` from a stream of its own between
  the other draws (none of which reads it), applied and refused, undone and redone.
- `cantus-ui/tests/score.rs`: the split and the pane draw their engraving; the settings are
  commands with an undo; the staff's keys edit (step, octave, input, delete); MusicXML and
  PDF out and MusicXML in through the app; keymap 13 gives an older keymap `Alt+S`; the
  boot path's fonts hold Bravura; a 200-bar part's frames within the budget.
