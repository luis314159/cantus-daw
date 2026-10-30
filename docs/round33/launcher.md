# Round 33 — launcher: the session view, keyboard, MIDI learn, controller grids

ADR 0077 · keymap 14 · branch `r33/launcher` (from `r33/integration` 20505f04; merged
again at 7c810c9b for `SlotPhase::Stopping { since, from, at }`, and at 5eba37bd for the
session's conductor). The whole loop is checked: a press in the pane or on a grid →
`EngineMessage::Session` → the conductor (`session_play`) → `take_session_events()` → the
slot lit (`a_press_goes_through_the_conductor_and_comes_back_as_the_slots_light`).

## What was built

- **The Session pane** (`crates/cantus-ui/src/views/session*.rs`, `Pane::Session`): the
  arrangement's tracks as columns (colour, name, arm, mute, solo, *back to arrangement*),
  the scenes in a master column at the right (launchers, names, tempo/metre tags, a "+"
  row that adds one), the stop row and *stop all*, the header (global launch
  quantisation, arrangement record, back to arrangement, capture, new scene, the panel's
  key). Slots: the clip's colour, name, a miniature of its notes or waveform, the playing
  position along the foot; queued and stopping blink, recording is red; empty slots show
  the stop square or, armed, the record dot. Both axes scroll (wheel, Shift+wheel, kit
  scrollbars); column width and panel height are drags (right-click resets); the panel
  steps aside when the pane is too short (the grid keeps three rows) and says so.
- **Live state** folded from `SessionEvent`s once a frame (`session_live.rs`); overflow
  and a new project → `Resend`; a press shows queued at once (the conductor answers at
  least its lead ahead — derived from the device's blocks, about 5–48 ms by block size,
  ADR 0076 §7) and lets go unanswered after half a second. Repaints only while
  something blinks. Arrangement record is the conductor's (the view switches it and shows
  it).
- **Switching**: Tab, a Session key in the top bar's window strip, *View ▸ Session*, the
  palette. The Session tab comes in just before the arrangement's, in its leaf (a device
  tab closed after them leaves the arrangement in front). Bitwig's in-arrangement launcher
  is the dock: drag the tab beside the arrangement.
- **Editing in the grid**: double-click (one-bar MIDI clip / open in the piano roll), drag
  slot→slot (move; Alt/Ctrl copy), slot→arrangement, arrangement→slot (marked hook in the
  arrangement's drag release), sample from the browser → slot; F2, Ctrl+Shift+C, Ctrl+D,
  Ctrl+C/X/V, Del, Ctrl+Shift+L, Ctrl+I, Ctrl+Alt+I; right-click menus for clips, empty
  slots and scenes. All through session's commands and builders, one undo step each.
- **The panels**: a clip's — Launch, Loop (Live's bar.beat.sixteenth), Follow actions,
  Envelopes (drawn over the loop with `auto_curve`); a scene's — the tempo and metre it
  sets, its follow actions.
- **Keymap 14**: thirteen actions — the session's twelve (category *Session*) and *Toggle
  Ableton Link* reserved for sync (unbound, a marked stub until the integration) — ten bound (Tab, Enter, Shift+Enter,
  Ctrl+Enter, Ctrl+Shift+Enter, Alt+Enter, Ctrl+I, Ctrl+Alt+I, Alt+B, Ctrl+Alt+R), the
  scene cursor's two unbound for controllers; `NEW_IN_V14`. With the pane focused the
  arrows move the slot cursor and the editing keys act on the grid.
- **MIDI learn**: `MappingTarget::{SessionSlot, SessionScene, SessionStop,
  SessionQuantize}` in core (riding schema 46), launch on press / release on let-go with
  the pad's velocity, feedback by state; learn-mode outlines on slots, scenes, stops,
  stop all, the header's keys and the quantisation; *Add a mapping* lists the quantisation.
- **Controller grids**: Launchpad Mini MK3, X, Pro MK3 (programmer mode), APC mini mk2,
  APC40 mkII (host mode) — protocol in `cantus-core/src/controller_grid*.rs` (from the
  makers' references), driver in `cantus-ui/src/controllers_grid.rs`; found by port name,
  chosen or turned off in Settings ▸ MIDI and in the controllers pane; their ports taken
  from the engine; the session ring drawn on screen.
- ADR 0077; the manual's section and shortcuts (EN/ES); two beta-checklist items; shots in
  `docs/round33/launcher/`.

## Tests and numbers

- `cantus-core` (after the merge of the session engine): 561 passed, 0 failed (new: `controller::grid::tests` ×9, two session
  target tests, the undo fuzz's launcher stream `round33_launcher_command`).
- `cantus-ui`: 1 987 passed, 0 failed, 33 ignored (new: `views::session::tests` ×16,
  `workspace::session_tests`, `keymap_13_gains_the_session_keys`, `tests/session_view.rs`).
- **Frame budget** (`tests/session_view.rs`, a 16 × 32 session playing, queued, stopping,
  recording, at 1600 × 900): **18 draw calls, 68 allocations a frame, 49 160 triangles** (the session's whole frame);
  an idle grid asks no repaint (the test drives the session's whole frame, its blink's
  repaint included). Draw calls, allocations, triangles and the idle repaint are asserted;
  build + tessellate — **median 2.0 ms, p95 3.0 ms** — is *measured* (`session_timing`,
  ignored), not asserted: a machine-time bound would not hold on CI's runners.
- **A pad hit, press → `EngineMessage::Session`** (`pad_press_latency`, ignored; Windows,
  test profile, 50 hits each): a grid port's messages are taken from the engine by the
  tap's filter, and a taken message wakes the window at once (`controllers::waking_tap`:
  `request_repaint` from the MIDI callback), so an idle window does not wait for its next
  repaint. On the interface thread the pad's message becomes the press in **3 µs median
  (p95 5 µs) with a small session idle, 3 µs (p95 4 µs) with a 16 × 32 session playing**;
  the conductor plans it in the same frame (`sync_engine` runs after `controller_input`):
  **0.01 ms**. A hit landing during a frame waits for that frame's end — the Session pane's
  build + tessellate is 2.0 ms median, 3.0 ms p95 at 16 × 32 — so press → message is the
  wake plus at most one frame. The press shows queued on the pad and in the pane at once.
- A slot recording, launched (its dot, or a pad), ends the take at the boundary and the
  clip plays on — the conductor's rule; the view sends the launch and draws what it hears.
- Clippy `--workspace --all-targets` with `-D warnings`: clean on Windows and on Linux (WSL,
  `tools/wsl/check.sh`, the plug-ins without their editor too); Linux workspace tests at
  3f79cff3: **4 930 passed, 0 failed, 61 ignored**.

## The review's fix round (75213607 and after)

Each fix has a test, written against the reviewed fault (named *fails before the fix* in
`views/session_tests.rs`; their failure on the old code was not re-run, it follows from it):
a hidden pane keeps no slots to drop into (its layout forgotten when not drawn, and at
Tab); a scene launcher's release reaches the scene (the grid takes only a slot's press);
grids are given back on the close path (`release_all`) and when the controllers drop (a
panic's unwinding); an APC's faders and knobs go on to learn and mappings (the port stays
taken from the engine); the Launchpad Pro's buttons as its reference names them (track
control CC 1–8, track select CC 101–108; Stop Clip held + track select = stops, Shift +
Stop Clip = stop all; Session, Note, Chord and the rest nothing); Ctrl+D into a new scene
one step; a pasted clip's envelopes follow its new track (or are dropped); a key-held
launch is released with its key; the Enter family acts only with the pane focused or from a
controller; Ctrl+X of a scene copies its row; a held pad lets go of what it launched; a new
grid port is taken in its first frame (found before the filter, and on a rescan at once);
a learnt slot records into an armed track's empty slot as a pad does. Look: the panel's
cards keep their width and scroll sideways in a narrow pane; envelope end points show
whole; header keys, scene launchers, stop buttons and stop all have screen-reader names
(made only when asked: the frame allocates nothing for them).

After the fix round (016501de): Windows `cantus-core` 561 passed, `cantus-ui` 1 987 passed,
0 failed; workspace clippy clean on Windows and Linux; Linux workspace tests 4 940 passed,
1 failed — `cantus-sandbox` `a_hang_never_blocks_the_interfaces_frames_and_is_ended`, a
timing test the launcher does not touch, failing under the machine's load (three builders);
alone it passed 3 of 3.

## The second fix round

Each with a test in `views/session_tests.rs` (or `controller_grid_tests.rs`):

- **N1** — the arrangement's timeline is forgotten when it was not drawn last pass
  (`ArrangementView::forget_geometry`, the pass it was drawn in kept by the session view),
  so a slot's clip let go outside the grid with the arrangement's tab hidden lands nowhere
  (`a_hidden_arrangement_has_no_time_to_drop_into`).
- **N2** — the session's performance actions (`Action::held`: Tab, Enter and its family,
  arrangement record, back to arrangement, capture, insert scene) fire on a fresh press
  only, never on the OS's repeats of a held key (`a_held_enter_launches_once_not_with_every_repeat`:
  Enter on an armed empty slot, then repeats — one Record).
- **N3** — one rule for a clip copied to another track: core's `session::envelopes_for`
  (now public) keeps a level and balance envelope always, an instrument's only when the
  track plays the same instrument (same native kind, Firmus, the same plug-in), a note
  effect's only when the track has a note effect of that kind at that id; the paste calls
  it (the view's own retargeting gone). `a_clip_copied_to_another_instrument_keeps_the_same_envelopes_either_way`:
  an Unda clip with a level and a cutoff envelope, copied to Calor's track by a Ctrl-drag
  and by Ctrl+C/V — both keep the level (retargeted) and drop the cutoff.
- **N4** — a grid's port keeps every note-shaped message (note on/off, poly and channel
  pressure) it does not decode; only CC, program change and pitch bend go on to learn and
  the mappings (`GridDevice::passes_on`; `a_pads_pressure_learns_nothing`).
- **N5** — the shots install the workspace style; the narrow shot re-taken.
- **N6** — the Pro MK3's track select row lights as track select (the tracks' colours, the
  selected one bright) and as the stops only while Stop Clip is held; Stop Clip (CC 8) lit
  while held (`a_launchpad_pros_bottom_row_selects_tracks_or_stops_them`).
- **Paste of a scene** — a scene copied or cut with the scene chosen comes back whole when
  pasted with a scene chosen: a new scene under the cursor's with the name, colour, tempo,
  metre and follow actions, its clips on the tracks they came from, one undo step
  (`a_cut_scene_pasted_comes_back_whole_in_one_step`). With a slot chosen, a paste fills
  slots as before.

Marked lines (`// r33:launcher`) outside the launcher's files: `views/arrangement.rs`
(`forget_geometry`, the drawn pass), `cantus-core/src/session.rs` (`envelopes_for` public,
`envelope_follows` between two markers).

Each of the six view tests was run against the code before the fixes (the fixes reverted,
the tests kept): all six failed; with the fixes, all pass. Windows: `cantus-core` 561
passed, `cantus-ui` 1 993 passed, 0 failed; workspace clippy `-D warnings` clean; the
session frame at 16 × 32 still 68 allocations. Linux (WSL, 34f85de3): workspace clippy clean (the plug-ins
without their editor too); workspace tests 4 939 passed, 0 failed, 61 ignored.

## After integration: Settings ▸ MIDI's inputs table

Merged (ce23e294), the inputs table — sync's columns, MPE's and the grid's — was about
764 pt on a page of about 588: the Grid column clipped whole, the MPE cell a third shown,
unreachable at 200 %. Now (`views/settings_midi.rs`, `inputs_band` between two markers):

- **Every toggle an LED key, as wide as its heading reads** (an LED key at the least, the
  word measured in the locale shown): Notes, Controls, Clock, MTC and **MPE** — the MPE
  mark is an LED key like its neighbours, not an Off / MPE pair.
- **The Grid cell as wide as its longest device name** (`grid::cell_width`, measured):
  its field shows the device's name alone (*Launchpad Mini MK3*); its menu still says which
  one was found.
- **The port's column takes what is left** (150–260 pt), its name elided with the whole
  name in the hover; headings are laid over their cells (the port's at the left).
- **Past that, the table scrolls sideways** with the app's solid scroll bar, shown only
  when it overflows: in a window too narrow for it every column is still reachable.

`tests/settings_midi_layout.rs` reads where the table was laid out (`midi_inputs_layout`):
at the window's default size on a 1080p screen at 100 % and 200 %, every column's cell
lies inside the page, each heading is over its cell, nothing overlaps and *Launchpad Mini
MK3* is drawn whole; on a 1366 × 768 screen at 200 % the table scrolls and every column
lies within what the scroll reaches. Shots: `launcher/settings-midi-{slate,paper}-{100,200}.png`
(four inputs, MPE marked on the Seaboard, a Launchpad Mini on the second).

Windows: `cantus-core` 587 passed; `cantus-ui` 2 020 passed, 1 failed —
`crash_recovery`'s `killed_mid_edit_the_autosave_brings_back_all_but_the_last_interval`, a
timing test (the autosave 2 008 ms behind under the machine's load), 3 of 3 alone;
workspace clippy `-D warnings` clean. The layout test needs the table's record, new
here, so it was not run on the code before; its *Launchpad Mini MK3* check fails there
(the cell drew *Launchpad Mini MK3 (found)* elided). Linux (WSL, f800fa0a): workspace clippy clean (the
plug-ins without their editor too); workspace tests 5 104 passed, 0 failed, 61 ignored.

## Only the owner

- A real Launchpad or APC (`docs/beta-checklist.md`): the Pro MK3's buttons — the guide's
  text gives the track select row (CC 101–108) and the track control row (CC 1–8); which
  labelled button is *Stop Clip* (taken as CC 8, Novation's user guide) and the arrows
  (CC 80/70/91/92) are read off its figure and the user guide, unconfirmed on a device, the APCs' blink and pulse rates, the colours as lit.
