# ADR 0116 — Insert and delete time, the section drag, the ripple mode

**Status:** Accepted (round 40, builder *ripple*) · **Date:** 2026-11-11 · **Round:** 40 ·
**Schema:** unchanged (62/63/64 not taken; the one setting is the app's, not the project's) · **Keymap file:** version 17 · **Extends:** ADR
0004 (commands), ADR 0010 (one path for a menu and a key), ADR 0025 (audio clips), ADR 0058
(comps), ADR 0060 (tempo, metre, markers), ADR 0064 (the freeze lock), ADR 0113 (sections),
ADR 0114 (the arrangement's tools) · **Design:** des-arrange, *Sections lane* ("Dragging it
moves its bars on every track; Ctrl copies, Shift inserts") and *Moving a clip*

## Context

Six of the eight leaders have insert/delete time and ripple editing (`docs/scorecard/features.md`,
E11); Cantus had neither. ADR 0113 left the section drag moving the section's name alone,
although the approved sheet says it moves its bars on every track. Research of the leaders'
manuals (round 40's report, `docs/round40/ripple.md` §1, with its sources):

- **Insert / delete time** — FL Studio (*Insert space* Ctrl+Ins, *Delete space* Ctrl+Del; a
  *Slice and insert space* variant cuts clips at the selection start), Ableton Live (*Insert
  Silence* Ctrl+I, *Delete Time*, *Cut/Paste/Duplicate Time*: "act upon all tracks", time
  signature markers included), Logic (*Insert Silence Between Locators*, *Snip* / *Splice*:
  regions across the locators are cut; global events move only when everything is selected),
  Cubase (*Insert Silence*, *Delete Time*: events across the range start are split; applied
  to the tracks the range covers — the tempo track only if it is selected, its users' most
  reported trap), Pro Tools (*Insert Time* / *Cut Time* in Time Operations, with options for
  the conductor rulers; *Insert Silence* splits clips in Shuffle), REAPER (*Insert empty space
  in time selection*: "all tracks are affected and happens regardless of the current ripple
  mode"; items split as needed), Studio One (*Insert Silence* and *Delete Time* always ripple,
  with or without the mode).
- **Ripple** — a **mode** wherever it exists: Pro Tools' *Shuffle* edit mode, Logic's
  *Shuffle* drag modes, Cubase's *Shuffle*, Studio One's *Ripple Edit* button (selected
  events), REAPER's three-way switch (off / per track / all tracks, Alt+P). FL and Live have
  no mode — only the time commands.
- **Section drags** — Logic's arrangement markers (a drag moves the section and the others
  close up and make room; Option copies), Studio One's arranger (drop between sections
  inserts, onto a section replaces, Alt copies, Shift replaces partly); Cubase's arranger is a
  play order, not an edit.

## Decision

### 1. Each edit one command (`cantus-core/src/ripple*.rs`)

`ripple::insert_time(project, at, len, ids)`, `ripple::delete_time(project, from, to, ids)`,
`ripple_bars::move_bars(project, from, to, dest, Landing { copy, insert }, ids)` and
`ripple_mode::rippled(project, gesture, label, mode, follows, ids)` each return one
`Command::Batch` of the model's own commands, worked out on a scratch copy of the song so
each step sees the one before (`Edit`). The batch's inverse is exact; the bus adds what any
tempo edit adds (`tempo::keep`) and the crossfades of new overlaps (ADR 0070). No new command
variant, so no schema bump, the agent's batch tools reach them as they reach any batch, and
the freeze lock guards them (a frozen track's clips refuse the edit whole; ADR 0064).

### 2. Insert and delete time are the whole song's

Every track and every lane (take lanes too, so a comp stays over its takes), the alternate
comps' kept windows (`with_comps`: each comp's windows edited by the same passes on a side
copy, one `SetTrackComps`), the markers, the sections, the mixer's cues (ADR 0093's snapshots
placed in the song — added by the mid-round review, see the amendment below), the loop and
punch ranges, the tempo track and the metre changes. A clip across the edit point is **split** there — the
arrangement's *Split* (`clipboard::split`); a looping clip cut where no repeat begins by its
loop pieces (the model has no loop phase) — and what follows moves. An unwarped audio window
that moves is laid through the tempo maps before and after (`audio_time::relocate`), so it
plays the same second of its file.

- **Positions:** a start at or after the insert point moves; an end past it grows (a section
  across the point grows by the new bars, as Logic's and Studio One's sections do). Deleting,
  a mark inside the range goes, a span across an edge loses what was inside, a loop or punch
  wholly inside is cleared and switched off.
- **Tempo:** the tick-0 point stays; the new time holds the tempo at the point; a ramp cut by
  the point gets a point at the cut holding its value and one after the new time ramping on,
  so the music before and after plays as it did (`ripple_tempo`). Deleting, the song goes on
  at the cut as it was at the range's end (a point is added there unless the step before
  already plays it). A point the music does not need at the cut is left out.
- **Metre:** changes at or after the point move; deleting, the metre at the range's end goes
  on from the cut. A change that lands off a bar line starts a bar anyway (ADR 0060 §3); the
  UI's ranges are on bars unless Alt.
- **Delete heals.** Two pieces that meet where the gap closed and continue each other (one
  clip split there, its content going on: `ripple::continues`) are glued back — so inserting
  time and deleting the same range is the identity, to the tick. **We differ from the
  leaders** here (none heals) because an invisible cut left by an undone-by-hand insert is a
  defect a user cannot see.
- **We differ from Cubase** in scope: the time edits are always the song's, never the
  selected tracks', so the tempo track can never be left behind; a single track's ripple is
  the ripple mode's.

### 3. The section drag moves its bars

A drag of a section's body (the end grip still resizes it) moves **its bars on every
track**, as the approved sheet says. A drop lands one of four ways (`Landing { copy, insert }`):

| Landing | Bars | The song |
|---|---|---|
| insert | taken out and put in where they land; what was there moves on | same length, rearranged |
| replace | moved; what was where they land is replaced, where they were is left silent | same length |
| insert a copy | a copy put in where it lands | longer |
| replace with a copy | a copy replaces what was where it lands | same length |

**Which one a plain drag is, is a setting** — *Settings ▸ Editing ▸ Section drag*: **Insert**
(the default) or **Replace** (`Settings::section_drag`, a preference of the machine, not of
the project; a settings file from before it opens with Insert). **Shift gives the other
one**, **Ctrl copies** (Ctrl+Shift: a copy, the other way), **Alt** leaves the bar lines (Ctrl
was the lane's "free" key until now). `ripple_ui::landing_for` is the one place keys and
setting become a landing.

*Why Insert by default* (the owner left the choice to the integrator, 2026-11-12, asking for
excellence or configurable modes): it never overwrites material — the worst a drop can do
is put bars somewhere unexpected, one undo away — and it is what the two leaders with a
section drag do: Logic's arrangement markers (a drag moves the section, the others close up
and make room; Option copies) and Studio One's arranger (dropped between sections it inserts
and the song moves on; dropped onto a section it replaces; Alt copies). The des-arrange sheet's
"Ctrl copies, Shift inserts" read the plain drag as replacing (FL's playlist moves content, it
does not rearrange); that reading is the *Replace* setting, kept whole for those who want it.

Nothing changes while the section is dragged: the section rides as a ghost in the lane (the
names it rides over hidden: text draws over every shape), and the preview always shows what
the drop will do — the bars it would replace tinted in the accent with accent edges through
every lane, or, inserting, a 2-px accent line with a cap where it goes in; the slot a move
leaves dashed in ink 3 — and the help line says it in words ("Intro · 4 bars → 13.1 · goes in
here, the song after it moves on") with the keys that change it. The drop is one undo step.
The inserting landings carry the section's tempo and metre (`tempo_span` / `paste_tempo`);
the replacing ones keep the song's clock, as a clip moved does. A linked clip is copied as
another placement of the same clip (FL's pattern clips); others as clips of their own. A
section the inserted bars go into is cut around them. To move a section's name alone: *Move
the section to the time selection* in its menu (Studio One's Ctrl+Alt drag, as an entry).

### 4. The ripple mode

`ArrangementView::ripple: RippleMode` — **Off** (the default; view state, never saved, never
an undo step, off at every start), **This track**, **Every track** (REAPER's switch). The
tool row shows its key beside Follow — a clip and two chevrons — accent-tinted with its word
("Ripple · this track") while on: the one clear sign. Alt+Shift+P cycles it (REAPER's Alt+P
is our punch). With it on:

- deleting clips closes their gaps (on every track, the deleted spans merged, each closed once);
- moving clips along their track (not a copy, not to another track) moves what follows them
  on their track by as much;
- drawing a clip's right edge out or in moves what follows its end;
- a paste goes in: what started at or after the paste point moves on by the pasted block
  (REAPER's and Studio One's ripple paste).

*Every track* moves the song's marks and clock with the clips (the time edits' passes with
`Scope { cut: false, marks: true }`); it **never cuts** a clip: one on another track that
began inside a closed stretch stays where it is (REAPER's behaviour; *Delete time* is the
cutting edit).

### 5. Keys and menus (keymap 16 → 17)

`InsertTime` (Insert), `DeleteTime` (Ctrl+Delete, Ctrl+Backspace — ⌘⌫ on a Mac),
`CycleRipple` (Alt+Shift+P), in every preset, filled into saved maps by version 17. FL's
Ctrl+Ins cannot be had on Windows: egui-winit reads it as Copy before the application sees
the key (`keymap::clipboard_binding`). The empty timeline's menu leads with a *Time* band
(Insert time, Delete time) while time is selected; the panel menu lists both; the palette
reaches all three. Insert with no selection inserts a bar at the playhead's bar; Delete time
with none says to select first.

## Consequences

- The scorecard's E11 is met by all three: the time edits, ripple per track and all tracks,
  and the section drag the leaders with an arranger have.
- The plain section drag changed meaning (ADR 0113 moved the name alone); the old behaviour
  is the section menu's entry. Its landing is a setting (Insert by default, Replace).
- Clips of other tracks may overlap after an every-track ripple; overlaps are allowed and
  crossfaded as any (ADR 0070).
- Limits, said: a tempo ramp running across a deleted range's start is drawn to the value at
  the range's end (one point per tick cannot end one ramp and start another); a looping clip
  cut between repeats becomes its written-out pieces (sounds the same, heals only on its
  repeats); an unwarped audio window under a tempo track that changes over its hidden part is
  re-laid in seconds, so its halves heal only where one tempo holds.

## What the tests hold

- `cantus-core/src/ripple_tests.rs` — the parts of the song through each edit (clips split and
  moved, sections, markers, loop, punch), the tempo track with a cut ramp (bpm at every bar,
  seconds after the cut = old + the new bars), metre changes, a looping clip, an audio clip's
  fades, takes and alternate comps, nothing-to-do refused, undo through the bus.
- `cantus-core/src/ripple_bars_tests.rs` — the four landings; the ripple mode off, this track,
  every track.
- `cantus-core/tests/ripple_props.rs` — **insert then delete the same range is the identity
  to the tick on 400 random songs** (note, looping, linked, take-lane, automation and audio
  clips forward, reversed and faded; sections, markers, metre changes, tempo tracks with
  ramps, loop, punch); every edit and drag undoes and redoes whole.
- `cantus-core/tests/undo_invariants.rs` — `time_edits_and_section_drags_undo_and_redo_among_the_others`
  (`// r40:ripple`): every kind applied among the others, a stale edit refused whole.
- `cantus-engine/tests/ripple_render.rs` — an inserted bar of silence moves the audio by
  exactly its length, **sample-exact** (bit-identical before, silent bar, bit-identical after),
  with one tempo and under a tempo track; deleting it renders the song as it was.
- `cantus-ui/tests/ripple_e2e.rs` — through `CantusApp::assemble`, the pointer and keys where
  a user puts them: a right-click inside the time selection opens its menu and inserts time,
  Ctrl+Del deletes it back to the original song, Insert puts a bar at the playhead; the
  section drag under both *Section drag* settings — Insert, the one a settings file without
  the field opens with (a plain drag rearranges, Ctrl+Shift copies over, Shift replaces), and
  Replace, stored in the settings file (Shift rearranges, Ctrl copies over, a plain drag
  replaces); the ripple mode's key (Alt+Shift+P), its sign in the tool row, a delete, a drag
  and a paste on this track only, the cycle back to off; each one undo step; saved and loaded.
- `cantus-ui/src/keymap.rs` — version 17; no preset conflicts.

## Amendment — the mid-round review (round 40, builder `fix-b`)

- **The mixer's cues move with the music** (review B-1). A `MixCue { at, .. }` is a point of the
  song as a marker is, and was left behind by every time edit. Now `ripple_marks::cues` moves a
  cue at or after the insert point, and deleting takes one inside the range (`RemoveCue`) and
  moves a later one back (`SetCue`, which keeps the cue's place in the list, so insert-then-
  delete is still the identity). The section drags carry the cues in their bars (`Carried::cues`):
  a replacing landing clears the cues where it lands, a copy gets cues of its own (fresh ids),
  appended to the list. *Every track* ripple moves them with the marks (`Scope::marks`); *This
  track* does not. Tested: `ripple_tests::the_mixers_cues_move_with_the_time_and_go_with_deleted_time`,
  `ripple_bars_tests::the_bars_carry_their_cues_and_replace_the_ones_where_they_land` and
  `every_tracks_ripple_moves_the_cues_after_the_edit`; the 400 random songs of
  `ripple_props.rs` hold up to three cues each.
- **The time keys are the arrangement's** (review C-3). Ctrl+Del and Ctrl+Backspace fired
  alongside views that delete on Delete or Backspace whatever the modifiers (an EQ's band, a
  multiband's, an automation point, a cable), and Insert inserted time wherever the pointer
  was. The key dispatch now runs `InsertTime` and `DeleteTime` only while the arrangement is
  the keys' focus and on screen, and the pointer is over neither a window above the workspace
  (an editor, a menu, a dialog: any layer above the background) nor another area of the dock
  (`CantusApp::time_keys_here`) — FL's insert and delete space are the playlist's. The menus
  and the palette run them as before. Tested end to end (`ripple_e2e::the_time_keys_act_only_in_the_arrangement`):
  Insert over the mixer, or with the mixer focused, inserts nothing; Ctrl+Del over an EQ's
  selected band takes the band and leaves the song's time and the time selection.
- **Ctrl+Ins** (§5): it is egui-winit that turns the chord into Copy, not Windows; the manual
  said otherwise and is corrected (en, es).
- **The engine plays it** (review B-4): `ripple_e2e::an_inserted_bar_moves_the_sound_by_its_length_to_the_sample`
  boots through `CantusApp::assemble`, imports a take at the song's start, inserts a bar with
  the key, plays the real processor in blocks of 256–1024 frames and finds the bar silent and
  the song after it bit-identical to the export's render of the song before. (A take, not an
  instrument: the first try, a Pulsus groove, differed by ~1e-8 from the first samples after
  the bar — the kit after a silent bar is not the kit at the start; not chased here, noted
  for the engine's owner.) The undo fuzz has two more kinds:
  a clip deleted in *This track* and a clip moved in *Every track* (`rippled`, kinds 6 and 7).
- **Words** (review C-4, C-5): the Spanish status lines agree for any length ("Tiempo insertado:
  1 compás en 5.1, en todas las pistas"; "Estribillo: se movió a 9.1 (4 compases)"); a length
  of bars that is not whole reads "2.5 bars" / "2,5 compases" (`ripple_ui::decimal_words`: two
  decimals at most, no trailing zeros, the language's decimal mark); the ripple key's tip says
  its keys first and what it does when on.

## Amendment — review 2 (round 40, builder `fix-d`)

- **A cue keeps its sound, not only its place** (review 2, R2-B2). A mixer cue holds until the
  next one, so moving cues like markers was not enough: deleting time whose last cue was inside
  the range (and none at its end) took away the scene of everything after; bars laid elsewhere
  played under whatever cue came before their new place, and the music after them under the
  bars' last cue. The rule now is the sound's: **every piece of music plays under the scene it
  played under before the edit.** Each edit says where its music went (`ripple_cues::Piece`: this
  stretch of the new song plays what began at that tick of the old) and `ripple_cues::keep_scenes`
  looks at each piece's start: where the scene in effect differs from the one the music had, cues
  of their own put it back (fresh ids, at the list's end, so they play after a cue already at that
  tick) — an unscoped cue when the master differs, then one scoped cue per snapshot for the
  channels that still differ; nothing where it is right. Delete time: what followed the range at
  the seam. Insert time: nothing to add (the cues move with the music). The section drags: the
  bars at their landing (the scene at the section's start), the music after them (where they
  replaced: the song's own scene there; where they went in: what was at that place), and, a move
  that leaves its place, the music after the place it left.
  - *The scene of a channel* is the snapshot of the latest cue that sets it, in playing order
    (tick, then id, as `mix_snap::cue_lanes` lays them): a cue sets the channels its snapshot holds
    that its scope names — every one but the recall-safe when unscoped — and the master only when
    unscoped (`ripple_cues::scene_at`).
  - *What it cannot do*: a channel no cue had set for some music played the desk's own values; if
    that music now lies after a cue that sets the channel, it hears that cue — a cue cannot unset a
    channel, and capturing the desk into a new snapshot would freeze values the user may still
    change. Rare (bars from before the song's first cue copied after it), and said here.
  - *Every track* ripple: deleting a clip closes its time on every track while the other tracks'
    clips that start before its end stay where they are. A cue inside that time that another
    track still plays over stays where it is, with that music (`ripple_mode::still_played`,
    `Scope::keep_cues`); one nothing plays over goes, and what moved up starts under the scene it
    had. Where moved music and music that stayed now sound together, one scene plays for both: the
    cues that stayed are the music's that stayed.
  - Tested: `ripple_cues_tests::every_time_edit_keeps_the_scene_each_piece_of_music_plays_under`
    — 3 000 random songs (four channels, one recall-safe now and then, three snapshots of a few
    channels each, the master in some, up to seven cues, scoped and not), each through a random
    insert, delete or one of the four section drags; where the music went derived on its own
    (stretches of the new song and where each began), and at each stretch's start and every cue
    inside it the scene equals the music's before; one undo step gives the cues back. Failed before
    the fix (a copied section's music after its landing). Also `deleting_time_keeps_the_scene_of_what_follows`
    (the reviewer's case), `every_track_ripple_keeps_a_cue_another_track_still_plays_and_the_moved_musics_scene`,
    the undo fuzz's time edits now among snapshots and cues (`over_cues`), and end to end
    `ripple_e2e::deleting_time_from_the_keys_keeps_the_scene_of_what_follows`.
- **The time keys reach the arrangement wherever it is** (review 2, R2-B1). The C-3 gate refused
  any layer above the background, and a floating area is a window of its own (`Order::Middle`):
  the keys were dead in a floating arrangement. `Workspace::pane_at` resolves what is under the
  pointer — an area of the main surface (maximised or not), an area in a floating window (egui_dock's
  window layer, the leaf under the pointer, the window's frame its area's), a layer above (an
  editor, a menu, a dialog: their keys) or bare chrome — and `CantusApp::time_keys` accepts the
  arrangement wherever it is: its ruler, its sections lane, its tool row. A keyboard user who moved
  the focus there with Ctrl+F6 (*Next area*) has the keys wherever the mouse rests, until the mouse
  is pressed (`focus_by_keys`). Refused, the status line says why and the way there
  (`ripple.keys.*`: the arrangement hidden; another view focused; the pointer on another area —
  "move there with Ctrl+F6", the key as the keymap has it); over a layer with keys of its own,
  nothing is said (the key was that layer's). Tested end to end:
  `ripple_e2e::the_time_keys_reach_the_arrangement_wherever_it_is_and_say_why_when_not` (docked,
  floating and maximised, on the ruler, a section, the tool row and a track; over the mixer and
  with the mixer focused, refused and said; Ctrl+F6 with the mouse resting on the mixer, then a
  press of the mouse) — it failed with the old gate (floating, on the ruler).
