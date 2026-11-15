# Round 40 — `fix-d`: review 2's model, keys, mixer, inspector, words and picker items

Branch `r40/fix-d`, base `1357827a` (`r40/integration` with everything of the round but Chorda).
The items are review 2's section `fix-d` (`F:\projects\daw-r40\tmp\review-2.md`). This file
grows as the items land.

## R2-B11 — Slate Paper less blue: the owner kept today's

The owner: *"El Paper que mostraste, ¿puedes ponerlo menos azul?"* On 2026-11-12 he compared
three strengths side by side, whole window at 100 % — `docs/round40/look/slate-paper/sbs-{arrangement,mixer}.png`
(and the left 900 px of each, `-left`), single shots `{arrangement,mixer}-{today,less,neutral}-100.png`
— and **kept today's Slate Paper** (k = 1). No colour changes ship; the shots stay as the record.
How they were made: each grey of the room (surfaces, wells, strips, decks, grid, keys, caps,
edges, the veil and the grey text inks) moved toward the neutral grey of its own luminance in
linear light, keeping `k` of its chroma (1, 0.5, 0), so every contrast stayed where it was;
`shoot_look40 -- <dir> --paper` reproduces them (`slate_paper@<k>`).

## R2-B1 — the time keys reach the arrangement wherever it is

*Reproduced*: `ripple_e2e::the_time_keys_reach_the_arrangement_wherever_it_is_and_say_why_when_not`
fails with the old gate at its first floating case (the floating window is an `Order::Middle`
layer, which the C-3 gate took for an editor's window). *Fix*: `Workspace::pane_at` resolves what
is under the pointer (main surface, maximised or not; a floating window by egui_dock's own layer
id, the leaf under the pointer, its frame its area's; a layer above; bare chrome), and
`CantusApp::time_keys` accepts the arrangement wherever it is. After **Ctrl+F6** the keys act
wherever the mouse rests until it is pressed (`focus_by_keys`). Refused, the status line says why
and the way there (`ripple.keys.*`, both locales); over a layer with keys of its own, nothing.
The test covers docked, floating and maximised; ruler, section, tool row and a track; the mixer
under the pointer and the mixer focused (refused and said); keyboard focus, then a mouse press.
The start page the app opens with covers the middle of the window: over it the keys are its own
(the test closes it first).

## R2-B2 — cues keep the scene in effect

*Reproduced*: `ripple_cues_tests::every_time_edit_keeps_the_scene_each_piece_of_music_plays_under`
(3 000 random songs with scoped and whole-desk cues, snapshots of a few channels, a recall-safe
channel; a random insert, delete or section drag; where the music went derived independently)
failed at once — a copied section's music after its landing played the section's last cue — and
`deleting_time_keeps_the_scene_of_what_follows` (the reviewer's case) failed. *Fix*:
`cantus_core::ripple_cues` — each edit names its pieces (new start ← old start) and
`keep_scenes` adds, at each piece's start, cues of its own where the scene differs (an unscoped
one when the master differs, then one scoped cue per snapshot). The every-track ripple keeps a cue
inside the closed time when another track still plays over it (`still_played`). ADR 0116 amended
(the rule, the scene's definition, the one thing it cannot do: unset a channel no cue had set).
Undo fuzz: the time-edit arm now runs among snapshots and cues (`over_cues`). End to end:
`ripple_e2e::deleting_time_from_the_keys_keeps_the_scene_of_what_follows`.

## R2-B3 — Merge back keeps the user's routing

Decided as the review proposed: *Merge back* undoes *Split to strips* and only that.
`padout::split_onto` (heard on a strip made for pads) drives `can_merge` and `merge`; a pad on a
strip the user chose stays there. A render's copy (`merge_in_place`) still takes every pad back.
Refused, the status line says so (`padout.merge_users`). ADR 0117 amended; manual (en, es).

## R2-B9 — `fixtures/v65.cantus` regenerated

On this tree with `write_fixture`: it now holds the record tap (schema 62's block); every old
fixture still loads (`cantus-project` tests).

## R2-B10 — the real strip menu in `pad_user_strip_e2e`

The test boots through `CantusApp::assemble`, shows the mixer, right-clicks the strip's head and
reads and clicks the menu as drawn: no *Split / Merge* on the user's strip; on the kit's strip
*Merge back* changes nothing (and, asked, says why); *Split to strips* from that menu, then
*Merge back* from a strip it made, the kick staying on the user's strip, the song to the bit.

## R2-B4 — the inspector's send bar and a value under its figures

The send row's value column is Level's (`row.left + margin + sh.col + gap`): its bar starts where
Volume's does, the name filling up to it (both *Inspector values* styles). Where a value's edge
falls under its number, a small notch at the bar's top and bottom edges points at it, a caret's
height clear of the digits (`kit::slider::value_bar`) — readable in Night where the fill's edge
alone was not. Shots: `docs/round40/fix-d/inspector-slate_{night,paper}-100.png` (Volume −3.5 dB).

## R2-B5 — words

*Top bar extras* in Spanish: the segments keep short words (*Ninguno · Armado · Teclado · Ambos*)
and a note under the row says what each is (*Armado: la tecla que arma la escritura de
automatización. Teclado: el teclado del ordenador…*) — the long words elided in a four-way
choice ("Armar automatizac…", shot before the change). English gets the same note. The record
tap's row is *Toma* (*Sin FX · Con FX*), so *Grabar [Armar]* two rows down no longer repeats the
verb; the English *Take from* stays. Shots: `settings-slate_{paper,night}-100-es.png`,
`insp-record-tap-{night,paper}-100-es.png`.

## R2-B6 — Delay's note list at small heights

`fx::frame::popup_list` opens under its field when it fits, above it when that fits; a list the
window can hold whole is raised until it does; a longer one scrolls (under its field while eight
rows find room there, else above). `fx_impetus`'s key test looks for the list above its field too
(its 16-strip list now flips above a low field instead of running past the window's foot). Test:
`delay_window_e2e::the_note_list_stays_inside_a_short_window` (720 pt: every value in view inside
the window, one chosen with a click). Shot: `delay-notes-720.png`.

## R2-B7 — the mixer's next strips before the right dock

True numbers kept (the C-1 decision). With the dock at the edge the run ends 18 pt before the
dock's divider, and a ghost's number is ink 4 at 60 % — fainter than an empty strip's. The "16"
after the returns is not a ghost: it is the default desk's third return strip, empty (a real
strip). Shots: `mixer-slate_{night,paper}-100.png`.

## R2-B8 — the dancer picker

The kit's scrollbar (`kit::widgets::scrollbar`), draggable, inside the card at its right, the
tiles clear of it; the wheel and the bar move it by points (a tile partly in view is clipped);
`MIN_SIDE` is scaled with the interface. **"Metrónomo" whole** (the integrated suite's one
failure, `ux_whole_words`): a name runs across its cell (tile and gap) and takes the smaller role
when the label's is too long. **Compás's own Shy**: *Cueing* / *Preescucha*, a DJ's cup lifted off
one ear (`cast/compas.rs`) — a posture added to an approved character, for the owner's eyes
(`cast-compas-moods.png`, last column). Tests: `dancer_window::every_character_fits_the_picker`
(1× and 1.5×), `the_picker_scrolls_with_the_kits_scrollbar_and_the_wheel`, `ux_whole_words`.
Shots: `dancer-edit-slate-100-es.png`, `dancer-edit-paper-100-en.png`, `dancer-play-slate-150-en.png`.

## What the owner should look at

- `docs/round40/fix-d/inspector-slate_night-100.png` — the send bar aligned; the notch on Volume.
- `settings-slate_paper-100-es.png` — *Extras de la barra* with its note.
- `mixer-slate_night-100.png` — the next strips apart from the dock.
- `dancer-edit-slate-100-es.png` — the picker's scrollbar, *Metrónomo* whole.
- `cast-compas-moods.png` — Compás's *Cueing* (last column): his call whether it stays. The
  controllers pane keeps naming the slot *Shy* (one name per kind for every dancer, as the
  choice table has it); the editor shows the character's word.
- `delay-notes-720.png` — Delay's notes in a 720-pt window.

## Verified only in emulation

Everything above is tested in the CPU-rendered window and the model; nothing here needs a
device.
