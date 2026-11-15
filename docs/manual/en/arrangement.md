# The arrangement: time, sections and ripple

*Cantus 0.1.0-beta.15 and later · [Leer en español](../es/arrangement.md)*

The arrangement is where the song is laid out in time: a row per track, the ruler and the
sections lane above them, the tempo track when you show it. This page is about editing
**time itself** — making room in a song, taking bars out of it, moving a whole section — and
about the **ripple mode**, which makes a clip's edit move what follows it.

## Insert time

Drag in the ruler to select the time you want to add (say, bars 17–18), then either:

- right-click inside the selection (on a clip or between them) and choose **Insert time**, or
- press **Insert**, or
- open the command palette (**Ctrl+Shift+P**) and run *Insert time*.

That much empty time goes in at the start of the selection, **on every track**:

- a clip that runs across that point is split there (both halves keep the whole clip's
  content, as *Split* does), and everything after it moves later;
- the automation clips move with the music, and so do the take lanes and the alternate comps;
- the tempo track and the metre changes move too, and the new bars keep the tempo the song
  had there — inside a tempo ramp, the ramp is cut so the bars before and after play exactly
  as they did;
- markers and the mixer's cues (its snapshots placed in the song) after the point move; a
  section across it grows by the new bars; the loop and the punch range move or grow the same
  way.

Afterwards the new, empty bars are selected. With no time selected, **Insert** puts one bar in
at the bar the playhead is in.

## Delete time

Select the time and choose **Delete time** in the same menu, or press **Ctrl+Delete**
(**Ctrl+Backspace** does the same; ⌘⌫ on a Mac). What lies inside goes on every track — clips
across the edges are cut there — and everything after moves earlier to close the gap, with its
automation, tempo and metre changes, markers, mixer cues and sections. A marker or a cue
inside the deleted time goes with it; a loop or punch range wholly inside it is cleared.
A cue holds until the next one, so what follows keeps the scene it played under: when the last
cue inside the deleted time was still in effect at its end, a cue of the same snapshot is left
where the gap closed. Moving or copying a section works the same way: its bars arrive with
their scene, and the music after them keeps its own.

Deleting time you have just inserted gives the song back exactly: the clips that insert split
are joined again, the tempo track is as it was — nothing is left behind.

Each of these is **one undo step**.

## Dragging a section moves its bars

The sections lane, above the tracks, names the parts of the song ("Verse · 16 bars"). Drag a
section by its body and **its bars move on every track**:

| Hold | When you let go (Section drag: **Insert**, the default) |
|---|---|
| nothing | the bars are taken out and put in where they land; what was there moves on — the song rearranged, nothing lost |
| **Shift** | the bars move and replace what was where they land; where they were is left silent |
| **Ctrl** | a copy of the bars is put in where it lands; the song after it moves on (it gets longer) |
| **Ctrl+Shift** | a copy of the bars replaces what was where it lands |
| **Alt** | (with any of the above) leaves the bar lines: the drop lands where the pointer is |

If you would rather a plain drag replace (as FL Studio moves content), choose **Replace** in
*Settings ▸ Editing ▸ Section drag*: then a plain drag replaces and **Shift** inserts. Insert
is the default because it never overwrites anything, and it is what Logic and Studio One do
when you drag a section.

While you drag nothing changes yet: the section rides as a ghost, the bars it will replace are
tinted in the accent across every track (or, when it inserts, an accent line shows where it
will go in), the slot it leaves is outlined in dashes, and the help line at the bottom says exactly what
the drop will do. Let go, and it is one undo step.

The inserting drags carry the section's tempo and metre with its bars; the replacing
drags keep the song's clock, as moving a clip does. The markers and the mixer's cues inside
the section go with its bars (a copy gets cues of its own).

To move a section's **name and span alone**, without its music, select the time it should
cover and choose **Move the section to the time selection** in the section's menu. Its end can
still be dragged to resize it.

## The ripple mode

The key with a clip and two chevrons, at the right of the arrangement's tool row beside
*Follow*, switches the ripple mode — or press **Alt+Shift+P**. It cycles:

- **Off** (the default, every time Cantus starts): an edit moves only what it touches.
- **Ripple · this track**: deleting a clip closes its gap on its track; moving a clip along its
  track moves the clips after it on that track by as much; drawing a clip's right edge out or
  in moves what follows it; a paste goes in, and what was after the paste point moves on.
- **Ripple · every track**: the same, on every track, with the markers, mixer cues, sections and
  tempo changes after the edit. Clips on other tracks that started inside a closed stretch are
  left where they are — the ripple mode never cuts a clip; *Delete time* does.

While the mode is on, its key is tinted in the accent and says which one it is. Each edit,
with what it moved, is one undo step.

## Keys

| Action | Key |
|---|---|
| Insert time | Insert |
| Delete time | Ctrl+Delete, Ctrl+Backspace |
| Ripple mode (off, this track, every track) | Alt+Shift+P |

The time keys are the arrangement's: they act while it is what you are editing (you clicked
in it last) and the pointer is not over another area or a window — over an effect's editor,
Ctrl+Delete removes its band and leaves the song's time alone, and Insert in the mixer
inserts nothing. Anywhere on the arrangement counts — its ruler, its sections lane, its tool
row — floating in a window of its own or maximised too. If you reached it from the keyboard
(**Ctrl+F6**, *Next area*), the keys work wherever the mouse happens to rest, until you press
it. When a time key does nothing, the status line says why and how to get there.

FL Studio inserts space with Ctrl+Insert; the window toolkit Cantus is built on reads that
chord as *Copy* before Cantus sees it, so Cantus uses Insert alone. Every key can be changed in
*Help ▸ Keyboard shortcuts*.
