# ADR 0114 — The arrangement's tools: a radio set, the grid menu, Follow, the panel menu

**Status:** Accepted (round 39 close-out, builder *lo-arrange*) · **Date:** 2026-11-10 ·
**Round:** 39 · **Schema:** unchanged · **Keymap file:** version 16 (two actions added to
round 39's own version) · **Extends:** ADR 0004 (commands), ADR 0010 (one path for a menu
and a key), ADR 0024 (the kit's toolbar), ADR 0110 (panels, maximise), ADR 0111 (the design
system), ADR 0113 (the arrangement's structure) · **Design:** dir-a's `b/b-toolbar-*`,
des-arrange's *Panel*, *Clip states*, *Moving a clip*, *Resize* and `max-*`, des-views'
*Piano roll — Head* and *Clip content* (approved by the owner 2026-11-09)

## Context

The approved sheets draw the arrangement's tool row as FL's playlist has it: the panel's
name, **six tools as one radio set** (Select, Draw, Slice, Erase, Mute, Zoom), the grid's well
with a menu in musical order (Triplets on Shift+3, "Follow the zoom"), and at the right
**Follow** and the panel's **menu**. The row built through round 39 was something else: a
row of clip *actions* (split, glue, duplicate, mute, repeat, make unique, paint, colour,
delete), the snap lamp and two readouts. The arrangement had no tools at all — its mouse did
one thing (select, move, resize) and FL's tool letters always reached the piano roll's
tools, whichever view was being worked in.

The sheets also draw clip states the painter did not have (`lit`, `dim`, the moving clip's
dashed slot and shadow, the resize edge's light, the loop notch), automation values at the
close zoom, the piano roll's head (a clip menu, a selection readout), and a maximised
arrangement whose docks fold to a row of tabs.

## Decision

1. **A tool is the view's state, never the song's.** `ArrangementView::tool`
   (`views/arrangement_tools.rs`) is chosen in the row, by FL's letters or by the number row;
   choosing one is not an undo step. What a tool does is a `Command` like any edit, one undo
   step a gesture:
   - **Select** — what the view always did (the default, so nothing changes for anyone who
     never picks another).
   - **Draw** — a click on an empty lane adds a one-bar clip in the grid cell clicked
     (`InsertClip`); a drag draws it to the length dragged (one `InsertClip` on release); on
     a clip it moves and resizes as Select does.
   - **Slice** — a click splits the clip at the pointer on the grid (`clips::split`, the
     same batch as *Split at the playhead*), the cut drawn under the pointer first.
   - **Erase** / **Mute** — a click deletes or (un)mutes the clip; a drag across clips does
     it to each it touches, merged into one step (`MergeKey` + `end_gesture`).
   - **Zoom** — a click zooms in at the pointer, Alt-click out; a drag fits the bars dragged.
2. **The tool keys reach the view being worked in.** `ToolSelect`/`ToolDraw`/`ToolSlice`/
   `ToolDelete`/`ToolMute` and the new **`ToolZoom`** (8 on the number row; Z stays *Zoom to
   selection*) pick the arrangement's tools while it has the focus, the piano roll's
   otherwise — as before for the piano roll. B stays the arrangement's paint tool. The new
   **`GridTriplets`** (Shift+3) switches the triplets of the focused view's grid. Both are in
   keymap version 16 (round 39's, not yet released), so a saved map gains them.
3. **The grid** (`tools::grid_menu`, shared with the piano roll): Bar, 1/2, Beat, 1/8, 1/16,
   1/32, then Triplets (two thirds of the value; a value picked keeps them) and "Follow the
   zoom" (`grid_auto`: the finest value whose lines stand 12 points apart, brought up to the
   zoom before the view is drawn — view state, no repaint). The snap is the grid's: "No snap"
   in its well, *Snap to the grid* in the panel menu; a value picked turns it back on.
4. **The clip actions move to the panel menu** ("…"): split, glue, duplicate, mute, repeat,
   make unique, the paint tool, delete, loop and zoom to the selection, the snap, then the
   timeline's own entries — one menu, every entry the same action as its key (ADR 0010).
   They are all in the clip's own menu and on their keys too.
5. **Follow** (on at start, FL's default): while the song plays, when the playhead crosses
   the view's edge — or starts outside it, or a loop wraps — the view turns the page with the
   playhead 5 % in. A view the user scrolled away is left alone until the playhead enters it
   and leaves again. It runs only while playing: nothing moves at rest (ADR 0106).
6. **Clip states** (`arrangement_paint.rs`): `lit` (the selected track's clips, a step toward
   their colour, no edge), `dim` (a track another track's solo silences: its tint 62 % toward
   the lane, its notes and attacks at 32 %, its name at 45 % — silenced at a glance), a moving clip's slot dashed in ink 3 (4/3) with the clip
   riding over a soft shadow (35 % Night, 12 % Paper, +3 px), the resize edge's 4-px light
   (the clip's colour + 20 % white, lit while dragged), and a **loop notch** — a 9-px corner
   at a note clip's top right, shown when the pointer finds the clip; dragged, it switches the
   clip's repeat on at its length and draws its end out (`SetClipLoop` + the trim, one step).
7. **Automation values at the close zoom** (≥ 60 px a beat): each point's value in its units
   ("688 Hz", "−6.0 dB"), Medium 9, ink 2, above the point (under it at the lane's top), left
   out where it would touch the one before.
8. **The piano roll's head**: the panel's name, the **clip menu** (the track's stripe, its
   name › the clip's; the track's note clips with a check, then the other tracks — a pick
   opens it here), and the **selection readout** at the right ("8 notes selected", then
   "F3 – F4 · velocity 89 – 102" in ink 3; the clip's count when nothing is selected), built
   only when the song, the selection or the language changes. Its menu moves to the right
   ("…"), as the arrangement's.
9. **The tools wear the icon family's drawings** (set 2: `select`, `draw`, `slice`, `erase`,
   `muteTool`, `zoom`, `follow`) through `kit::icons::paint_name`, by the family's name —
   additive, no `Glyph` added. The piano roll's corner over the keys is des-views' "Rows"
   well (Compact 8, Default 12, Tall 18) in place of FL's note-colour tab.
10. **A maximised area folds the others to a row** (`workspace_fold::docks_row`): one 28-px
   row at the workspace's foot with the hidden areas' tabs and their keys ("Mixer F9 · Piano
   roll F7") and "Show the docks ⌃" at its right. A tab gives the areas back with that one in
   front; "Show the docks" gives them back as they were. What maximise hides is ux-b's (ADR
   0110): every other area, the inspector included — the sheet keeps the inspector beside
   the maximised arrangement, which is left to the owner.

## Consequences

- The row no longer shows the selection's count or the time selection's length; the help
  line says both while they are drawn (`arrange.range.tip`, `arrange.move.tip`).
- A user who pressed P or C with the arrangement focused used to change the piano roll's
  tool without seeing it; now it changes the arrangement's, in the row.
- **Not built, for the owner to place:** the section drag that ripples the whole song
  (`BACKLOG-excellence.md` §3).

## What the tests hold

- `tests/lo_arrange_e2e.rs` (through `CantusApp::assemble`, the whole window at 1920 × 1080,
  the pointer pressing where a user would): the row paints the sheet's words; Follow's key
  toggles; the tool keys reach the focused view; Draw adds a bar-long clip on the bar and a
  drag stretches it, one undo step each; Slice cuts a clip in two; Mute toggles; the eraser's
  sweep is one undo step; Zoom clicks in and out and fits a dragged range; the loop notch
  switches the repeat on and draws the clip out, one undo step; the grid menu lists the
  values in order with Triplets and Follow the zoom, Shift+3's action switches triplets, and
  the grid follows the zoom; automation values show at the close zoom only; the piano roll's
  head names track › clip, counts the notes, reads "2 notes selected" with the range, and its
  menu lists the other tracks; a maximised arrangement shows the docks' row at the foot, and
  both its tab and "Show the docks" give the areas back.
- Unit tests (`arrangement_tools.rs`): Follow's page turns only when the playhead leaves the
  view (not while the user is scrolled away; again on a loop's wrap); every grid value has its
  triplet, the menu is in musical order, a value keeps the triplets, the zoom's grid keeps
  its lines 12 points apart; every tool's action finds it again.
- `app_flows::the_arrangement_toolbar_is_a_row_of_icons_with_a_grid_picker` holds the new row.
