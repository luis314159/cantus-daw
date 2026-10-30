# ADR 0077 — The session view: the pane, keyboard, MIDI learn and controller grids

**Status:** accepted (round 33) · **Date:** 2026-10-29 · **Extends:** ADR 0024 (the kit),
0060 (markers and tempo; the keymap's actions as controller targets), 0062 (MIDI learn),
0072 (the palette), 0076 (the session's model, commands and engine) · keymap 14

## Context

ADR 0076 gave the session its model, its commands and the engine's message and event API.
BACKLOG §0.2 asks for the rest the user touches: Ableton Live's and Bitwig's clip launcher
as a pane, switching between it and the arrangement at once, editing in the grid, a
session clip's panel (launch, loop, follow actions, its own envelopes), keys, MIDI learn of
slots and scenes, and the grid controllers people own (Novation's Launchpads, Akai's APCs).

## Decision

1. **The Session pane** (`views/session*.rs`, `Pane::Session`): the arrangement's tracks
   as columns (their colours, names, arm, mute, solo and *back to arrangement*), the scenes
   as rows with their launchers, names and tempo/metre tags in a **master column at the
   right** (Live's layout: it stays put while the columns scroll), the stop row and *stop
   all* under the grid, the global launch quantisation, arrangement record, *back to
   arrangement*, capture and a new scene in the pane's header. A slot with a clip is FL's
   clip face made a cell — a title line in the clip's colour with the launcher (▶, lit
   while it plays, blinking while queued or stopping, a red dot recording), the name, a
   miniature of its notes or its waveform, and the playing position as a bar along its
   foot; an empty slot is a well with its track's stop square or, armed, its record dot.
   The layout is pure arithmetic (`session_layout.rs`, tested): only the cells in view are
   painted and sensed, both axes scroll (wheel, Shift+wheel, the kit's scrollbars), every
   column's width is one drag on a head's edge, the clip panel's height a drag on its top
   edge (right-click resets either). Both themes, 75–200 %.
2. **What plays is folded from the engine's events once a frame** (`session_live.rs`):
   per slot its phase, per scene its phase, which tracks the session holds, arrangement
   record; an overflow forgets and asks `Resend`. The playing position is
   `SlotPhase::content_at` and the transport's tick — no traffic per block. Nothing the
   view shows is polled from the engine.
3. **A press shows at once**: the session's conductor (`crate::session_play`, ADR 0076
   §7) takes a press at least its lead ahead of the playhead — derived from the device's
   blocks and the compiles' times, about 5–48 ms by block size — and answers when it has
   planned it; the view draws the pressed slot or scene as queued from the press
   (`Live::press`) until that word comes — or, unanswered (a frozen track's clip), for half
   a second. Arrangement record is the conductor's too: the view sends the switch and
   shows its state; what was heard is committed there, one undo step.
4. **Switching**: `Pane::Session` is a tab beside the arrangement;
   **Tab** (Live's) brings one or the other forward (the Session tab comes in just before
   the arrangement's, so a tab closed after them leaves the arrangement in front), the top bar's window strip has its key,
   the View menu and the palette list it. The selection is kept (a session clip chosen is
   the piano roll's clip). Back to arrangement per track (the head's key) and for all.
   Bitwig's launcher *inside* the arrangement was weighed: the dock already gives it — drag
   the Session tab to the arrangement's side and both show at once, each at full size — so
   no second, narrower launcher was built. A scene chosen (its launcher clicked) shows the
   scene's panel instead of a clip's: the tempo and the metre launching it sets, and its
   follow actions.
5. **Editing in the grid**: a double-click on an empty slot makes a one-bar looping MIDI
   clip; a double-click on a clip opens it in the piano roll; a drag moves a clip to another
   slot (Alt or Ctrl copies) or, let go over the arrangement, puts it there at the time
   under the pointer; arrangement clips dragged onto a slot go into their tracks' columns
   from that row down (a marked line in the arrangement's drag release); a sample from the
   browser lands in an empty slot as a looping audio clip; F2 renames a clip or a scene in
   place (the kit's field), Ctrl+Shift+C recolours, Ctrl+D duplicates (a clip into the next
   free slot below, a scene under itself), Ctrl+C/X/V copy, cut and paste clips, Del
   deletes; the right-click menus hold the rest (loop, put in the arrangement, consolidate
   the column, the stop button, a scene's tempo and metre, insert, capture, move). Every
   edit is one of ADR 0076's commands or builders — one undo step.
6. **The clip panel** (`session_clip.rs`, `session_env.rs`): four cards of label | value
   rows on the kit — *Launch* (mode, quantisation, legato, velocity amount), *Loop* (loop,
   start marker, loop start, loop length or end, in Live's bar.beat.sixteenth), *Follow
   actions* (on, A, the chance A : B, B — a jump names its scene —, after linked or a time,
   how many times), *Envelopes* (a parameter of the clip's track: volume, pan, its strip's,
   its inserts' and its instrument's first parameters, the last tweaked; its curve drawn
   over the clip's loop with the automation editor's tracer and painters; click adds a
   point snapped to a sixteenth, drag moves it, double- or right-click deletes it). A drag
   merges into one undo step.
7. **Keyboard (keymap 14)**: thirteen actions — the session's twelve and *Toggle Ableton
   Link*, reserved here for sync (ADR 0078) — are `Action`s, rebindable, in the palette,
   learnable, with defaults that are never a bare letter (the typing piano keeps
   them): Tab switches, Enter launches the cursor's slot, Shift+Enter its scene, Ctrl+Enter
   stops its track, Ctrl+Shift+Enter stops all, Alt+Enter records into the slot, Ctrl+I
   inserts a scene, Ctrl+Alt+I captures one (Live's Ctrl+Shift+I is our element picker),
   Alt+B goes back to the arrangement, Ctrl+Alt+R is arrangement record; the scene cursor's
   up and down are unbound actions for controllers. The grid's actions (Enter and its
   family, insert, capture, the scene cursor) act only while the pane has the focus (the
   last press of the pointer was in it) or from a controller — a bare Enter elsewhere is a
   dialog's or the arrangement's; a launch by a key is let go with the key (a gate clip
   stops). With the focus the arrows move the slot cursor (Shift: eight) and the editing
   keys act on the grid — as the mixer's arrows walk its strips. A file of keymap 13
   gains the new bindings where their keys are free (`NEW_IN_V14`).
8. **MIDI learn**: four targets join `MappingTarget` — `SessionSlot { track, scene }`,
   `SessionScene`, `SessionStop` (launchers: a press launches at the pad's velocity, a
   release releases a gate clip; their feedback lights full playing, half queued, a quarter
   holding a clip) and `SessionQuantize` (portable; a knob steps through the fourteen
   grids, `SESSION_QUANTIZE`). *Stop all*, arrangement record, *back to arrangement* and the
   scene cursor are keymap actions, which ADR 0062 already maps. In the learn mode every
   slot, scene launcher, stop button, the quantisation field and the header's keys are
   outlined and learnt with a click. The targets ride schema 46: they name what schema 46
   brought, so no file older can hold one, and none was released between.
9. **Controller grids** (`cantus-core/src/controller_grid*.rs`, pure and tested with byte
   sequences; `cantus-ui/src/controllers_grid.rs` drives them): the Launchpad Mini MK3,
   X and Pro MK3 in **programmer mode** and the APC mini mk2 and APC40 mkII in their
   host-controlled modes. A port is a grid when its name says so — the interface each mode
   talks on: a Mini's or an X's *second* (`MIDIIN2 (LPMiniMK3 MIDI)` on Windows,
   `… LPMiniMK3 MIDI …` on macOS, `… LPMiniMK3 MI …` on Linux), the Pro's *first* (its
   second is DIN, its third the DAW's), an APC's own — or when the user chooses (Settings ▸
   MIDI, the controllers pane: off, or a device). Its messages are taken from the engine
   (the tap's filter, ADR 0062), its pads launch the ring's slots at their velocity (an
   armed track's empty slot records), its scene buttons launch rows, its stop buttons stop
   tracks, its arrows move the **session ring** (Shift: a page), which the pane draws as a
   box. Lights: a clip's colour steady (RGB where the device takes it), green pulsing while
   it plays, green flashing while queued, its colour flashing while it stops, red pulsing
   recording, a dim red on an armed track's empty pad; scene buttons green; stops red; the
   arrows lit where the ring can move. Only what changed is sent, once a frame, through the
   feedback outputs. Let go (turned off), a device is sent its own mode again.
   - **Launchpads** (*Launchpad Mini [MK3] Programmer's Reference Guide*, *Launchpad X
     Programmer's Reference Guide*, *Launchpad Pro [MK3] Programmer's Reference Guide*,
     Focusrite/Novation 2019–2020): SysEx header `F0 00 20 29 02 <0D|0C|0E>`;
     programmer mode `0Eh 01h` (Mini, X) and layout `00h 11h` (Pro); pads 11–88 from the
     bottom left, the right column CC 19–89, the top row CC 91–98; the LED SysEx `03h` with
     colour specs of type 0 (static), 1 (flashing, colour B then A), 2 (pulsing) and 3 (RGB,
     0–127). The Mini and the X have no stop row: their lowest scene button lends the
     bottom row to the stop buttons (Live's way) and their *Session* button stops all. The
     Pro: the guide's text gives its *track select* row as CC 101–108 (under the grid),
     its *track control* row as CC 1–8, Shift as CC 90 and the left column's up and down;
     Cantus follows Ableton's scheme — *Stop Clip* (the track control row's last, CC 8,
     as Novation's user guide labels it) held makes the track select row the stops, Shift
     with it stops all, and its mode buttons (Session, Note, Chord, Custom, Sequencer,
     Projects, CC 93–98) are no part of the grid. The arrows (CC 80, 70 up/down; 91, 92
     left/right) and which labelled button is which are read off the layout figure and the
     user guide: the owner confirms them on a device (`docs/beta-checklist.md`).
   - **APCs** (*APC mini mk2 Communication Protocol* and *APC40 mkII Communications
     Protocol*, Akai Professional): the introduction SysEx `F0 47 7F <4F|29> 60 00 04
     <mode> 00 00 00 F7` (the APC40 in mode 41h, Ableton Live's, so the host lights every
     LED; 40h gives it back); the APC mini's pads on notes 0–63 from the bottom left, track
     buttons 100–107, scene buttons 112–119, Shift 122, its RGB SysEx `24h` (each colour
     0–255 as two 7-bit bytes) for steady pads, a note-on whose channel sets the behaviour
     (7–10 pulse, 11–15 blink, a quarter note each here) for the others, velocity 0/1/2 for
     the single-colour buttons; Shift with track buttons 1–4 moves the ring and with the last
     scene button stops all (Ableton's script). The APC40's clip pads on notes 0–39 (0x20–0x27
     the top row), scene launch 0x52–0x56, clip stop 0x34 on the track's channel, stop all
     0x51, bank select 0x5E–0x61, Shift 0x62.
   - Flashing and pulsing take a palette entry, not a colour: `palette_index` picks the
     nearest hue of the 128-entry palette both makers share (Ableton's).

## Consequences

- The pane never polls: an idle session costs nothing a frame but its painting (the
  frame budget test holds it), and a blinking one repaints at 60 ms only while something
  blinks.
- A grid port's notes never reach an instrument; a port the user turns off plays notes
  again (the filter is published again).
- What only hardware can confirm — the Pro MK3's side buttons, the APCs' blink rates, the
  colours as lit — is on the owner's list (`docs/beta-checklist.md`).

## What the tests hold

- `cantus-core` `controller::grid::tests`: every device's port names found (and the DAW and
  DIN ports refused), its mode messages, presses decoded (the guides' examples: 11 the
  lower left, 81 the upper left, 18 the lower right), lights encoded (the Launchpads' LED
  SysEx specs, the APC mini's RGB SysEx and behaviour channels, the APC40's notes), the
  palette, the ring's moves; `controller::tests`: the session targets' existence, learning
  as buttons, the quantisation's steps and its command undone.
- `cantus-ui` `views::session::tests`: events folded (hand-over, held tracks, overflow and
  Resend), the playing position (a stopping clip's too), a press shown queued at once and
  let go unanswered, the whole loop through the conductor (press → `perform_frame` → the
  slot lit); what launching sends; recording refused unarmed; the keys (cursor only with the
  focus, Enter and Shift+Enter); every grid edit one undo step; capture; drags slot to
  slot (move, copy, onto nothing) and arrangement to slot; the layout's hits, scroll
  clamping, nothing overlapping; a Launchpad found, greeted, lit, launching, its ring moved,
  a playing pad pulsing green; a grid turned off given back; a learnt pad launching and
  releasing with its light; the pane drawn headless in both themes, with and without a
  session. `tests/session_view.rs`: the kit lint over the session's files, the frame
  budget. `keymap` tests: version 14 and its migration.
