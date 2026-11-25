# ADR 0161 — Editor windows that behave: the view comes back, a pin, several at once

> **Provisional number** (branch `r42/editor-windows`): renumber at the merge if 0161 is taken.

**Status:** Accepted (round 42, the `editor-windows` builder; the owner's request of
2026-11-25) · **Date:** 2026-11-25 · **Amends:** ADR 0024 §5 (device windows dock by default
and float by choice), ADR 0051 §3 (every *open editor* brings it to the front) · **Builds on:**
ADR 0029 (the device windows' buttons, the tray), ADR 0073 (a hosted plug-in's Cantus view),
ADR 0005 (the audio thread) · **Schema:** none (window state stays outside the project)

## Context

The owner, 2026-11-25 (translated): *"When you open an instrument (also a plug-in) to
configure it and then close it, closing it should bring back the screen you had before
opening it — the mixer, the arrangement, or whatever was showing. I'd also like to pin a
plug-in or instrument so I keep seeing it if I want, and/or bring several onto the screen, so
that clicking something else doesn't make them disappear."*

What happened before this ADR (reproduced in `tests/editor_windows_e2e.rs` and the unit tests
of `views::workspace`, which fail on the old code):

1. **Closing showed the wrong view.** An instrument's editor is a device tab of the dock
   (ADR 0024 §5), appended to the arrangement's leaf, or to the lower zone's when the
   arrangement is not on the main surface — the mixer maximised (a double-click on its tab),
   or the arrangement hidden (`F5`). egui_dock, removing a tab at or before the active one,
   makes the tab *before it* active: the editor opened over the mixer (the leaf's tabs Mixer,
   Piano roll, Channel rack, the editor) closed onto the **channel rack**. The same at
   *opening*: an editor too tall for its leaf floats at once, and the frame it spent docked
   moved the leaf's front — opened over the session view, it left the **arrangement** in
   front.
2. **Only one editor per family.** `app.inst.open: Option<TrackId>`, `app.fx.open`,
   `app.eq_editor.open`, `app.comp_editor.open` (the three effects' slots closed each other),
   `app.pulsus.open` and `app.fx.hosted.inst_open` each held one editor, and opening another
   closed it — the "disappears" the owner saw when the next instrument or effect opened. A
   docked tab also went behind any tab of its leaf the user clicked (the arrangement's).
   Per-editor state hung off the same singletons: the held keys, the view, the popups, the
   analyser's pairs, the frame's preset and lists, each kind's own state; and the engine has
   one instrument probe (`set_instrument_probe`) and one analyser (`set_analyser`).

The references: FL Studio floats plug-in windows, many at once, each with its own controls;
Ableton has *Multiple Plug-in Windows* (open several, or one at a time) and *Auto-Hide
Plug-in Windows*; Bitwig pins its device panel to a device; REAPER's floating FX windows have a
pin that keeps them on top.

## Decision

### 1. The view comes back — and only when it is the user's

`Workspace::device_leaves` is the one road a device tab takes out of its leaf — closed by any
means (its shell's ✕, its tab's ✕ — now taken out by the workspace, not egui_dock — `Esc`,
`F8`, the tray's ×, `F12`, the track or effect deleted, minimised) or floated (a too-tall
editor's first frame, *Float*, a pin, a maximise's restore). **When the tab was the one in
front of its leaf**, the tab it covered when it came there comes back in front — if it is
still in that leaf — and the keys that acted in the leaf act there. **When it was behind
another tab** (the user brought the mixer or the piano roll forward while the editor was
open) nothing changes: closing never pulls the user back to a view they left. What a device
covered is recorded when it is placed and, for every other road to the front (its tab
clicked, dragged into a leaf, brought forward by *Open editor*), by `watch_fronts` at the end
of each frame; a device stacked over another that has gone gives the front to what that one
covered. `app.central` follows the lower zone's front as before, so the central view is the
mixer again too. The layout is not touched (a device tab never splits a leaf), and the views
keep their own scroll.

### 2. A pin on every device window

The kit's window controls (`kit::window_ctl`) gain a **pin key** left of minimise: a push pin
leaning when not pinned (`Glyph::Pin`), upright on the selection's tint when pinned
(`Glyph::Pinned`), its tooltip saying which (`editors.tip.pin` / `.unpin`), the status line
saying what changed. Every device window in Cantus has it (instruments, Pulsus, effects,
a hosted plug-in's Cantus view: `device_chrome::ctl`); the controllers' windows and the
exported plug-ins' windows do not. The pin lives in the device registry
(`workspace_device::Entry::pinned`). A pinned editor:

- **is not replaced** when another of its family opens (§3);
- **is not hidden**: pinned while it shares a leaf with other tabs it floats where it is
  (`Pending::Float`), so no click on a tab or on the arrangement can put it behind one; alone
  in a leaf (split by the user) or floating, it stays where it is;
- **stays above the editors that are not pinned** (`keep_pinned_above`, after the editors
  are drawn: when a device window that is not pinned lies above a pinned one — it opened, it
  was clicked — the pinned ones go back on top in their own order; dialogs are not compared,
  so a dialog opened over a pinned editor stays over it);
- **is not closed by an Esc aimed elsewhere**: Esc closes the device clicked last, as before;
  its fallback — the floating device under the pointer — skips pinned ones; `F12` skips them.
  Its own ✕, Esc after a click in it, `F8` on it in front, the tray and its track or effect
  deleted still close it.

### 3. Several at once: one slot per family, the others parked

Each family keeps **one editor in its slot** — the singleton the existing code reads — and
the others **parked** in `app.editors` (`editor_windows`), each with its own copy of the
per-editor state (`editor_windows_park`: for an instrument its view, held keys, popups and
its kind's state; for an effect its frame, its pairs, its spec cache, its kind's state through
`views::fx::Editor`, a hosted plug-in's view; the EQ's and the compressor's whole state).
Every frame each parked editor is swapped into the slot, drawn by its family's own `window`,
and swapped out. Forty editors keep their code; none can see another's state. What is
already per track or per effect (A/B, presets, scenes) and the user's choices for every
editor (collapsed bands, the keyboard card) stay shared.

The rules (`editor_windows`, unit-tested):

1. **Opening an editor that is not open makes room in its family.** Settings ▸ Editing ▸
   *Editor windows ▸ Editors open*: **One at a time** (the default — today's behaviour) closes
   the family's unpinned editors and keeps the pinned ones; **Several at once** (Ableton's
   *Multiple Plug-in Windows*) closes none. The slot's editor is parked when it stays.
2. **Families** are today's: the effects (Aequalis, Comprimo and the plug-in family's slot)
   are one family across their three slots, as they closed each other before; the native
   instruments, Pulsus and a hosted instrument's Cantus view are three, as before. A hosted
   plug-in's own window (ADR 0028/0073) is the operating system's and was already one per
   instance; it has no pin.
3. **Opening an editor already open, parked included, brings it to the front** (ADR 0051 §3).
4. **The engine reports on the editor touched last.** A press on a parked editor moves it
   into its slot (`touch`). The engine keeps **one** instrument probe and **one** analyser
   (nothing more is asked of the audio thread, nothing allocated there — ADR 0005): the probe
   follows the native instrument in the slot, the analyser the effect editor opened or touched
   last (`live_of`), sent only on a change (`sync_editor_engine`). Every other editor on
   screen keeps its controls live and shows its meters, histories and analyser **at rest**
   until it is touched: `DawHost` and the compressor give it no engine telemetry, no EQ deltas
   and no analyser pairs (`effect_reads_engine`), `inst_telemetry` none — never another
   editor's numbers. Supporting several probes and analysers would change the engine's
   reports and its graph's hot path; it is left for when an editor needs it (BACKLOG).
5. **An editor whose track or effect is gone closes, parked or not**, after every command,
   undo and redo and once a frame (`prune_editors`) — its held notes off on the way. Undoing
   the deletion brings the track back and no editor.
6. **While a parked editor draws, an open it asks for waits** until the parked editors are
   drawn (`Deferred`: Pulsus's chain opening an effect, a link in an instrument's frame), so
   no open ever runs with another editor's state in the slot.

A new floating editor **opens where it covers the others least** (`free_spot`,
`place_beside`): its own place when that covers them by less than a quarter, else the centre,
a corner or a side of the workspace — side by side on a wide screen, cascaded on a small one.

### 4. Each instrument's own notes

The held keys are per editor (parked with it): two keyboards on screen each send their own
notes to their own track, and an editor closing — however — sends its held keys' notes off
(`close_inst_editor`, unchanged, now run for a parked editor by swapping it in first).

### 5. Window state stays outside the project

Which editors are open and which are pinned live for the session, in the egui context's
registry and `app.editors`, as the dock's device tabs always did (`Pane::Device` is never
saved). A project carries music, not where windows were; a project opened on another machine
or by another user does not bring windows with it. **No schema change.** The setting is
`Settings::editors_open` (`#[serde(default)]`: a settings file from before reads *One at a
time*; Settings is not versioned, so there is no migration to write).

## Consequences

- Closing an instrument opened over the mixer, the arrangement or the session view gives it
  back; the user's own view changes are never undone.
- Several instruments and effects can be on screen at once, pinned or by the setting, each
  with its own state; one at a time stays the default, so nothing changes for a user who does
  not pin.
- A parked editor's meters rest until it is touched — said in the manual and the setting's
  note. The cost of a parked editor is a swap of a few structs per frame (measured nowhere
  near the frame budget; nothing allocated after the first swap of a kind).
- Public additions: `editor_windows::{EditorsOpen, Editors, opening_closes, live_of}`,
  `CantusApp::{toggle_editor_pin, set_editors_open, effect_editor_open, close_effect_editor,
  sync_editor_engine}`, `workspace_device::{is_pinned, set_pinned, toggle_pin, note_layer,
  keep_pinned_above, free_spot, place_beside, Pending::Float}`, `Entry::{pinned, layer}`,
  `WindowCtl::pin`, `WindowRects::pin`, `WinHot::Pin`, `shell::Event::Pin`,
  `Glyph::{Pin, Pinned}`, `device_chrome::Spot::InstKeys`, `Workspace::covered`.

## What the tests hold

- `tests/editor_windows_e2e.rs` (its own binary; `CantusApp::assemble` on a real processor,
  frames fed as egui-winit feeds them, every gesture through published geometry): an
  instrument opened by `F8` over the maximised mixer and closed by its ✕ or by `Esc` gives the
  mixer back with the same layout and central view; opened over the session view (floating)
  or the arrangement, it leaves that view in front at opening and at closing; a view brought
  forward while it was open stays; a pinned instrument stays on screen and above an effect
  opened after it, through a click in the arrangement and an Esc aimed at nothing, and Esc
  after a click in the effect closes the effect only; another instrument opening does not
  replace it, and unpinned the next one does; *Several at once* keeps both, side by side;
  two instruments each sound only their own held key on their own track's meter and go
  silent when closed with the key held (Esc, *Close instrument*); deleting the track of a
  pinned parked editor (and of a pinned editor in its slot) closes it with no tab, no
  registry entry and nothing parked, and Ctrl+Z brings the track back with no editor.
- `views::workspace` (`device_leaves_tests`): the front comes back on close and on float,
  not when the device was behind, in turn for stacked devices, over the session.
- `views::workspace_device`: `place_beside` beside, inside, below; the pin kept and a shared
  tab asked to float. `editor_windows`: rules 1 and 4.
- The existing device tests (`device_ux`, `quick_gestures`, `hosted_*`, the editors' own) pass
  unchanged.
