# ADR 0051 — Quick gestures in the arrangement and the inspector

**Status:** Accepted, §4's audio row amended by ADR 0054 · **Date:** 2026-10-14 · **Round:** 27 (builder *gestures*) ·
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
  letters still reorders (egui reports no click once the pointer travelled), never resizes.
- **The letters win over their neighbours.** A row's interactive area starts below the
  divider grip above it (a press on the line belongs to the row whose height it changes),
  so on a short row (0.34 × 40, or 0.6 × the smallest zoom of 22) the name line reaches
  into the row above's band and into this row's own resize grip. The letters are therefore
  **a widget of their own** (`arr-name`, click and drag), sensed after the row and the rows
  above it: a click anywhere on them is this track's — select and open its name — the
  I-beam shows there, a drag from them reorders, and a right-click opens the track menu.
- **On a short row the words fit the row.** The name and its field are scaled to the
  name line (the body face's row height against the line's, at most 1), and the icon is
  no taller than the line: at full size they overflowed into the rows round it (seen in
  the pictures, `examples/shoot_gestures.rs`, `short-*`).
  A **clip's label** on such a row (no title bar) is scaled the same way to the clip's
  height, never drawn outside it, and **hidden below six tenths of its size**
  (`paint::label_fit`); the clip's hover hint now leads with its **name** (then its
  essentials), so a hidden label is still read there.
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

**A hosted effect's own window** comes back the same way: the mixer menu's *Open the
editor* and the inspector's insert rows (`open_effect_editor`) bring an open plug-in window
back from the taskbar and to the front (`open_window` → `focus`) as well as the effect's
card; the card's key reads *Open plug-in window*, *Show plug-in window* while it is open
(it used to close it — it now brings it forward; the window closes with its own close
box), and *Close plug-in window* for a plug-in that floats its own (`plugins::window_key`,
`show_plugin_window`).

**F8 and the rack's channel button toggle what the user sees.** Closed → opened; open but
**not in front** → brought back and to the front (`present` / `focus`); **in front →
closed** (`instrument_window_in_front`, `toggle_instrument_window_of`). The rack's channel
button now does it for a hosted instrument too (it toggled native editors only). *Not in
front* counts **only what bringing it forward can change**, or the toggle would bring it
"forward" for ever and never close it:

- a device (`workspace_device::in_front`; it lives in the context, so the rack's button
  asks it from inside a tab): minimised, or not drawn this pass or the last (a tab behind
  another tab), or — for a window of its own — another window **of its own layer order**
  over its middle. A **docked tab** showing in its leaf is in front whatever floats over
  it: the background it shares is never raised, so a floating window over it cannot be
  put under it. A dialog, a menu or a tooltip of a higher order stays above whatever is
  done, so it does not count either;
- a hosted plug-in's window (`PluginWindow::is_frontmost`): minimised or hidden, or a
  window over its middle that `focus` can go above. The window at the middle
  (`WindowFromPoint` → `GetAncestor(GA_ROOT)`) is it, **or a window it owns, directly or
  not** (`GW_OWNER` chain: the plug-in's own popups, preset browsers and dialogs, which
  always stay above their owner); **an always-on-top window** (`WS_EX_TOPMOST`, any
  program's) does not count — `HWND_TOP` cannot go above it; a middle on **no screen**
  (`MonitorFromPoint`) counts as in front. A plug-in that floats its own window counts as
  in front, so F8 closes it as before.

The inspector's hosted row reads **Open** when the window is closed, **Show** when it is
open (it was *Close*): a click opens it or brings it back and to the front; it closes with
its own close box, `F8` or the rack's channel button. A plug-in that **floats a window of
its own** keeps **Close** there: Cantus cannot bring that window forward, only close it
(`inspector_hosted::window_row`).

### 4. A double-click on a clip opens its editor, on it

`CantusApp::open_clip_editor` (`src/gestures.rs`), from the arrangement's double-click on a
clip (not on an audio clip's handle — that still resets the handle — and not on an empty
lane, which still makes a four-bar clip). Which editor is `clips::editor_for(clip, lanes,
rack.grid)` — the clip and its track's instrument's lanes — **derived from the content**,
nothing stored:

| The clip | Opens |
|---|---|
| automation | the automation editor (ADR 0027) |
| audio | the inspector, on the clip (there is no audio clip editor; the inspector holds its gain, fades and reverse) — *(amended by ADR 0054, 2026-10-15: an audio clip opens the pitch editor, `ClipEditor::Pitch`; its gain, fades and file stay in the inspector)* |
| notes the rack shows **with nothing lost** | the **channel rack** on its pattern, at the clip's own grid |
| notes otherwise, or empty | the **piano roll** on the clip |

**The rule: the rack only when it can show and edit the clip without losing anything**
(`clips::rack_grid`), at some step grid:

- **every note is a step**: it starts on a step of the clip's own grid (content tick minus
  the clip's offset, a multiple of the step) and lasts no longer than one step — a hit,
  which is what a step the rack switches on is;
- **every note has a row of its own**: a channel's row plays one pitch — in FL's channel
  rack too, where a melodic pattern is edited in the piano roll. Cantus's rack gives a
  melodic (chromatic) instrument **one row**, which adds any pitch but shows only on and
  off, so on such a track every note must be **the same pitch** (a hi-hat on a synth
  qualifies; a sixteenth-note bass line does not — it opens the piano roll, where its
  pitches are); a **kit** gets a row per pad and per stray pitch (`Axis::build`), so a
  pattern across its pads qualifies;
- **no two notes are one step**: the same pitch twice on the same step would show as one.

**The grid is the clip's own**, not whatever the rack was last set to: the rack's current
grid is tried first, then sixteenths, eighths, thirty-seconds, sixteenth and eighth
triplets, and quarters; the first that fits is set as the rack's grid when the rack opens
(a pattern made at 1/16T opens at 1/16T with the rack at 1/16). A melody, a held note, a
note off every grid, two pitches on a melodic track or an empty clip opens the piano roll,
which shows everything as it is.

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
rack, at the clip's grid, fits the whole pattern (`rack.fit`) from its first step and
scrolls the clip's track — its channel and a kit's pad rows — into view at its next layout
(`ChannelRackView::reveal_track`); the track is the selected one.

## Consequences

- A click on a track's name to select it now also opens the name; a click away commits
  nothing when nothing was typed (ADR 0024 §15), so selecting by the name costs one `Esc`
  or one click elsewhere at most. Selecting by the rest of the header is as before.
- A double-click on a Firmus or an audio track shows the inspector rather than nothing.
- The mixer and the rack rename on a double-click, the arrangement on a click on the
  letters — deliberately (§2).
- The inspector's hosted row and a hosted effect's card no longer close the plug-in's
  window (but for a plug-in that floats its own); F8 and the rack's
  channel button close an editor only when it is in front.
- No schema change, no new `Command` (the rename is `SetTrackName`, already in the undo
  fuzz), no new keymap action. New strings in `locales/areas/gestures.{en,es}.txt`
  (`track.header.tip`, `track.header.name_tip`), `hosted.show_window` and
  `host.editor.show_window`. A Windows-only dev-dependency for cantus-ui's tests
  (`windows-sys`, already in the tree through cantus-host) minimises a plug-in window.
- Public additions: `ArrangementView::header_hits`, `HeaderHit`, `clips::{editor_for,
  ClipEditor, rack_grid}`, `CantusApp::{open_track_instrument, open_clip_editor,
  show_plugin_window, instrument_window_in_front}`, `ChannelRackView::reveal_track`,
  `plugins::window_key`,
  `piano_roll::fit_clip`, `PianoRollView::grid_size`, `workspace_device::{present,
  take_reveals, in_front}`, `AreaState::raise`,
  `PluginWindow::{is_minimised, is_frontmost, is_floating}`, `HostedInst::{window_minimised,
  window_frontmost, window_floating}`, `Hosted::{window_frontmost, window_floating}`,
  `PluginHost::{instrument_window_minimised, instrument_window_frontmost}`.

## What the tests hold

`crates/cantus-ui/tests/quick_gestures.rs` (21), whole-window, the pointer driving each
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
inspector *(since ADR 0054: the pitch editor)*, an empty lane still makes a clip; the rack-or-roll rule (steps, long notes,
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
After the adversarial review (11 findings), each seen failing first: **the rack-or-roll
rule** — a sixteenth-note bass line on a synth opens the piano roll (it opened the rack),
a one-pitch hi-hat on a synth and a pattern across a kit's pads open the rack, two notes of
one pitch on one step open the roll, and the clip's own grid decides (sixteenths with the
rack at eighths; triplets at 1/16T with the rack at 1/16, the rack set to 1/16T when it
opens) — in the rule's test and whole-window; **the rack scrolls the clip's track into
view** (a kit below forty tracks); **F8 closes a docked editor under a floating effect
editor** (it brought it "forward" for ever); **the letters win** on rows of 0.34 × 40 and
0.6 × 22, at their top and bottom (a click there selected the track above). The
inspector's hosted row keeps *Close* for a floating plug-in (`inspector_hosted`'s unit
test), and `hosted_plugins.rs` (Windows): the mixer's *Open the editor* brings a hosted
effect's minimised window back (it stayed minimised), the card's *Show* keeps it open and
brings it back, and the card's key is *Close* for a floating plug-in.

`crates/cantus-host/tests/clap_window.rs` (Windows): a second *open* restores a minimised
plug-in window, leaves a maximised one maximised (it failed with the old `SW_RESTORE`),
and puts it back above another window of the process; it is not *frontmost* while
minimised or while another window of the process covers its middle, and is when nothing
does. And it is in front with its own popup over its middle, and with an always-on-top
window over it (both made it "not in front" for ever).
