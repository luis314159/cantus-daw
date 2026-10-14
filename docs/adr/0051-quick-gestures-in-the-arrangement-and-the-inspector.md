# ADR 0051 — Quick gestures in the arrangement and the inspector

**Status:** Accepted · **Date:** 2026-10-14 · **Round:** 27 (builder *gestures*) ·
**Extends:** ADR 0024 (the kit; §5 device windows, §9 the hint, §15 the one text field),
ADR 0027 (automation clips), ADR 0028 (hosted instruments), ADR 0044 (desk gestures) ·
**Schema:** none (24) · **Commands:** none new

> Numbering: written as 0051 on `r27/gestures`; the integrator renumbers it if another
> branch took it. The builder's account: `docs/round27/gestures.md`.

## Context

The owner, during round 26 (BACKLOG §6g): the arrangement and the inspector should open
things the way FL Studio does, with no hunting through menus — a double-click on a track
opens its instrument, a double-click on a clip opens its editor, *Open editor* brings a
window that is minimised or behind others to the front. §6g.2 first read "rename by
typing"; the owner's clarification of 2026-10-14 replaced it: **a track is renamed only by
clicking exactly on its name's letters**, never by typing with a track selected.

What stood in the way:

- A double-click on a header **renamed** the track (`views/arrangement.rs`), so the
  gesture FL users reach for to open a channel's plug-in was spent on something else.
- A double-click on a notes clip did nothing beyond selecting it; only an automation clip
  opened its editor (ADR 0027).
- Every *open editor* of an editor that was already open did one of three things: a native
  instrument's editor was **closed and opened again** (`open_inst_editor`), which kept a
  minimised one in the tray; `open_instrument_window` un-minimised it but left its tab
  behind another tab or its floating window under another; an effect's editor was reset
  and kept minimised. A hosted plug-in's window was restored with `ShowWindow(SW_RESTORE)`,
  which **un-maximises** a maximised window, and was never raised when the foreground was
  refused. The inspector's hosted row was a toggle whose second click *closed* the window.

## Decision

### 1. A double-click on a track header opens its instrument

`CantusApp::open_track_instrument` (`src/gestures.rs`), called on a double-click anywhere
on the header but the name's letters, the lamps, the icon chip and the record controls
(each keeps its own click): the track is selected and its instrument's window opens — or,
open already, comes back and to the front (§3) — a native instrument's editor, Pulsus's,
or a hosted CLAP/VST3 plug-in's own window. **A track whose instrument has no window**
selects the track and shows the **inspector**, where it is edited:

- **Firmus** (edited in the inspector, as `F8` already did for it);
- an **audio track** (ADR 0025 §3: an input and only audio clips) — its instrument is never
  played, and its inspector is what holds its input, monitoring and audio. Opening an
  instrument nobody hears would be a window about nothing.

There are no group tracks in the arrangement, and automation lanes are rows of their own
under the track, not its header.

### 2. A click on the name's letters renames it; nothing else does

- The header paints its name with the kit's text and now returns **where the letters were
  drawn** (`paint::header` → the galley's horizontal extent on the name line's height),
  published per frame as `ArrangementView::header_hits()` (`HeaderHit { track, row,
  letters }`, a buffer kept between frames: a steady frame allocates nothing).
- A **click** (press and release without travel) whose position is on the letters selects
  the track (an unselected one too) and opens the kit's field in place (ADR 0024 §15): the
  name selected, `Enter` commits one `SetTrackName`, `Esc` closes leaving the old name and
  no undo step, a click elsewhere commits, an untouched rename runs nothing, and no shortcut
  or the typing piano reads the keys while it is open. A **drag** that starts on the
  letters still reorders (egui reports no click once the pointer travelled).
- **From one name to another in one click**: with a track's name open, a click on another
  track's letters commits the first (one `SetTrackName`, as a click away does) and opens
  the second, its name selected, in the same click. The second name is opened once every
  row has run (`open_name`), and a first name still open then — its row came after the
  clicked one, so its field had not yet seen the click — is committed explicitly
  (`commit_track_name`, the field's own commit), whichever row is above.
- **Hovering the letters** shows the I-beam and the hint *Click: rename* (its caps say it is
  typed and has a menu); hovering the rest of the header says *Double-click: open the
  instrument · Drag: reorder · Right-click: menu*. The lamps, the icon and the arm lamp
  write their own hints after it.
- **Typing never renames.** No view starts a rename from the keyboard with something
  selected (`F2` stays the selected *clip*'s rename, a keymap action).

**The mixer's strips and the channel rack's pads keep their double-click rename.** The
owner's rule is about the arrangement header, where the double-click now has a better job;
a strip has no instrument to open, and its name zone is also its number cell — the strip's
selection and its drag handle along the desk (ADR 0044) — so a single click on its letters
renaming would turn every *select this strip* into an edit; a pad row's button already
selects the pad on a click and has no window of its own (the rack's *channel* button opens
the instrument on a click, as FL's does). Both open the same kit field with the same rules,
and neither renames on typing. The inspector's *Name* rows are fields already: a click
there edits.

### 3. Every *open editor* brings the editor to the front

`workspace_device::present(ctx, key)` — for an editor **already open**: back from the tray
if it was minimised, made the one `Esc` closes, its **tab** made the showing one in its leaf
(the workspace does it before laying the dock out: `Entry::reveal`, `take_reveals`), and
its **window layer** raised above the other windows the next time it is drawn
(`Entry::raise`: a floating tab's body layer, or an effect editor's area, via
`Context::move_to_top`; the background the docked tabs share is never raised). A device not
registered yet needs none of it: a new tab is placed in front, a new window on top.
`restore` (the tray's chips) is `present` too.

It is called by every road to an editor that is open: `open_instrument_window` (`F8`, the
rack's channel button, the header's double-click, a swap), `open_inst_editor` (the
inspector's and the instrument frame's *Open editor* — no longer a close and reopen),
`pulsus_editor::open`, and `open_effect_editor` (the mixer menu's *Open the editor*, the
inspector's insert rows — no longer a reset of the editor's state).

**Hosted plug-in windows** (`cantus-host/src/window.rs`, the crate's existing Win32 code):
`PluginWindow::focus` restores with `SW_RESTORE` **only when the window is minimised**
(`IsIconic`; on a maximised window it would un-maximise it), shows it if hidden, puts it on
top of the z-order (`SetWindowPos(HWND_TOP)`) and asks for the foreground
(`SetForegroundWindow`). The foreground lock: Windows lets a process take the foreground
only while it has it; the click that asked for the editor was on Cantus's own window, so it
does. When it is refused anyway, the window is still on top of its siblings and Windows
flashes its taskbar button — its own answer, kept rather than worked around
(`AttachThreadInput` tricks steal focus from other programs). A plug-in that floats a window
of its own leaves ours hidden: `focus` does not show it. **Elsewhere** (Linux, macOS) there
is no plug-in window yet (opening fails with `Unsupported`): `focus` is a documented no-op,
`is_minimised` and `is_frontmost` are `false`, all `pub` on a `pub` type — no `cfg` dead
code under `-D warnings`.

**F8 and the rack's channel button toggle what the user sees.** Closed → opened; open but
**not in front** — minimised, its tab behind another tab, its window under another window
— → brought back and to the front (`present` / `focus`); **in front → closed**
(`instrument_window_in_front`, `toggle_instrument_window_of`). *In front*, for a device:
not minimised, drawn this pass or the last (a tab behind another is not drawn), and the top
layer at its body's middle is its own, or the background a docked tab shares — no window
or dialog of the middle or foreground orders over it (`workspace_device::in_front`; it
lives in the context, so the rack's button asks it from inside a tab). For a hosted
plug-in's window: shown, not minimised, and the window at its middle on screen is it or
its child (`PluginWindow::is_frontmost`: `WindowFromPoint` → `GetAncestor(GA_ROOT)`); a
plug-in that floats its own window counts as in front, so F8 closes it as before.

The inspector's hosted row reads **Open** when the window is closed and **Show** when it is
open (it was *Close*): a click opens it or brings it back and to the front. The window
closes with its own close box, `F8` or the rack's channel button.

### 4. A double-click on a clip opens its editor, on it

`CantusApp::open_clip_editor` (`src/gestures.rs`), from the arrangement's double-click on a
clip (not on an audio clip's handle — that still resets the handle — and not on an empty
lane, which still makes a four-bar clip). Which editor is `clips::editor_for(clip,
rack.grid)`, **derived from the clip** with nothing stored:

| The clip | Opens |
|---|---|
| automation | the automation editor (ADR 0027) |
| audio | the inspector, on the clip (there is no audio clip editor; the inspector holds its gain, fades and reverse) |
| notes, **step-shaped** at the rack's grid | the **channel rack** on its pattern |
| notes otherwise, or empty | the **piano roll** on the clip |

*Step-shaped*: at least one note, and every note starts on a step of the clip's own grid
(content tick minus the clip's offset, a multiple of the rack's step — where the rack's
steps fall) and lasts no longer than a step. That is exactly what a step the rack switches
on is (a hit of `min(1/16, step)`), and what a drum pattern drawn as hits in the piano roll
is too — the rack shows it as steps without loss. A melody, a held note, a note off the
grid or an empty clip opens in the piano roll, which shows everything as it is.

Why not a stored hint ("the editor it was last edited in", the backlog's first wording):
it would be a schema bump (25) and a migration for a field the musician never sees, linked
copies would have to agree about it, and it can disagree with the content (a pattern made
in the rack and then given a melody in the roll). The rack and the roll share one data
model — a step *is* a note (`views/channel_rack.rs`) — so the content already says which
view shows it whole.

The editor opened is **in front** (its pane revealed after the dock is drawn — the
arrangement is a tab, and the dock is checked out while tabs draw), **is the central view**
(the keys' F6/F7 state), **has the focus** (`EditFocus::PianoRoll`: `Delete`, `Ctrl+C` and
the arrows act on its notes) and **shows the clip**: the piano roll fits its window (one
period of a looping clip, else its length) across the grid from the content's start and
centres its notes' rows, making the keys smaller only when they would not fit — at its next
layout, where the grid's size is known (`piano_roll::fit_clip`, `fit_pending`); the channel
rack fits the whole pattern (`rack.fit`) from its first step.

## Consequences

- A click on a track's name to select it now also opens the name; a click away commits
  nothing when nothing was typed (ADR 0024 §15), so selecting by the name costs one `Esc`
  or one click elsewhere at most. Selecting by the rest of the header is as before.
- A double-click on a Firmus or an audio track shows the inspector rather than nothing.
- The mixer and the rack rename on a double-click, the arrangement on a click on the
  letters — deliberately (§2).
- The inspector's hosted row no longer closes the plug-in's window; F8 and the rack's
  channel button close an editor only when it is in front.
- No schema change, no new `Command` (the rename is `SetTrackName`, already in the undo
  fuzz), no new keymap action. New strings in `locales/areas/gestures.{en,es}.txt`
  (`track.header.tip`, `track.header.name_tip`) and `hosted.show_window` (replacing
  `hosted.close_window`).
- Public additions: `ArrangementView::header_hits`, `HeaderHit`, `clips::{editor_for,
  ClipEditor}`, `CantusApp::{open_track_instrument, open_clip_editor}`,
  `piano_roll::fit_clip`, `PianoRollView::grid_size`, `workspace_device::{present,
  take_reveals, in_front}`, `AreaState::raise`, `CantusApp::instrument_window_in_front`,
  `PluginWindow::{is_minimised, is_frontmost}`, `HostedInst::{window_minimised,
  window_frontmost}`, `Hosted::window_frontmost`,
  `PluginHost::{instrument_window_minimised, instrument_window_frontmost}`.

## What the tests hold

`crates/cantus-ui/tests/quick_gestures.rs` (16), whole-window, the pointer driving each
gesture through `CantusApp::frame`: a double-click on a header opens Unda's editor (in
front, the track selected, no rename), Pulsus's, and shows the inspector for Firmus and an
audio track; a click on the letters of an unselected track selects it and opens the field
with the name selected (typing replaces it), `Enter` is exactly one undo step
(`cmd.set_track_name`) and `Ctrl+Z` brings the old name back; a click on the header off the
letters (on the name line and under it) never renames; typing letters with a track
selected never renames; `Esc` leaves the old name and no undo step; with the field open,
`Ctrl+Z`, `M`, `Delete`, `F8` and `Space` do nothing; hovering the letters shows the I-beam
and *Click: rename*, the rest of the header the double-click hint; a double-click on a
melody opens the piano roll from behind the mixer — central, focused, on the clip, its
eight beats across the grid and every note's row on screen; on a step pattern the channel
rack (fitted); an automation clip still opens its editor, an audio clip shows the
inspector, an empty lane still makes a clip; the rack-or-roll rule (steps, long notes,
empty, automation, audio, a note off the grid, a held note, a coarser grid); *Open editor*
on a minimised native instrument restores it, focused, its tab in front, and brings a
docked tab from behind the arrangement (and the kit's editor likewise); on two overlapping
floating editors it raises the one asked for; on a minimised effect editor it restores and
draws it. With the behaviours reverted to master's (the new accessors kept), ten of those
fourteen fail; the other four are the two guards that held before (typing, a click off the
letters), the hover (not reverted) and the pure rule. Added after the first review, both
seen failing first: **F8** opens a closed editor, brings back a minimised one, brings a
docked tab forward from behind the arrangement (it used to close it) and raises a
floating one from under another window (it used to close it), and closes it only when
it is in front; **a click on another track's letters** with a name open commits the
first (one undo step) and opens the second with its name selected, in the same click,
with the first above the second and below it (below, the second used not to open).

`crates/cantus-host/tests/clap_window.rs` (Windows): a second *open* restores a minimised
plug-in window, leaves a maximised one maximised (it failed with the old `SW_RESTORE`),
and puts it back above another window of the process; it is not *frontmost* while
minimised or while another window of the process covers its middle, and is when nothing
does.
