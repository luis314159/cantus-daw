# Round 27 — gestures: FL's quick gestures in the arrangement and the inspector (BACKLOG §6g)

Branch **`r27/gestures`** (worktree `daw-r27/gestures`, base master c76e85f). Decision
record: **ADR 0051** (*Quick gestures in the arrangement and the inspector*). No schema
change, no new command, no golden hash moved, no keymap change. The owner's clarification
of 2026-10-14 (rename only by clicking on the name's letters, never by typing) overrides
§6g.2's first wording; §6g is marked done with it.

## Done

| Gesture | What |
|---|---|
| 1. double-click a track header | `CantusApp::open_track_instrument` (`src/gestures.rs`): selects the track and opens its instrument's window — Unda & co. (a device tab), Pulsus, a hosted plug-in's own window — or brings it back and to the front; **Firmus and audio tracks show the inspector** (no window; an audio track's instrument never plays). Off the letters, lamps, icon chip and record controls |
| 2. click the name's letters | `paint::header` returns where the letters were drawn; `ArrangementView::header_hits()` publishes it per frame (`HeaderHit`, a reused buffer). A click there selects and opens the kit field (name selected, Enter = one `SetTrackName`, Esc = old name, no undo step, click away commits); a drag still reorders. Hover: I-beam and the hint *Click: rename*; the rest of the header: *Double-click: open the instrument · Drag: reorder · Right-click: menu*. Typing never renames (nothing listens for it) |
| 3. *Open editor* brings it to the front | `workspace_device::present` (un-minimise, focus for Esc, reveal the tab — `take_reveals` in the workspace's `before` — and raise the window's layer when drawn); called by `open_instrument_window`, `open_inst_editor` (no longer close + reopen), `pulsus_editor::open`, `open_effect_editor` (no longer a reset), and the tray's `restore`. Hosted: `PluginWindow::focus` = `SW_RESTORE` only if iconic (a maximised window stays maximised), `SW_SHOW` if hidden, `SetWindowPos(HWND_TOP)`, `SetForegroundWindow` (foreground lock: our click owns the foreground; refused → Windows flashes the taskbar button); a floating plug-in's hidden window is left alone; `is_minimised` lets F8 and the rack's button bring a minimised plug-in window back instead of closing it. The inspector's hosted row reads *Show* while open (it used to *Close*) |
| 4. double-click a clip | `CantusApp::open_clip_editor` with the rule `clips::editor_for(clip, rack.grid)`: automation → its editor; audio → the inspector (no audio editor exists); notes **step-shaped** (every note a hit, ≤ one step, on the clip's own step grid) → the channel rack (fitted, from its first step); else, and empty → the piano roll, **fitted to the clip** at its next layout (`piano_roll::fit_clip`: the window across the grid, the notes' rows centred, keys smaller only if needed). The editor is revealed, central and focused (`EditFocus::PianoRoll`). A handle's double-click still resets it; an empty lane still makes a clip |

Decided on the way (reasons in the ADR): the **mixer's strips and the rack's pads keep
their double-click rename** (a strip has no instrument to open and its name zone is its
selection and drag handle; a pad has no window of its own; both use the same kit field and
never rename on typing); the **rack-or-roll rule is derived from the content**, not a
stored "last edited in" hint (no schema 25, no invisible state for linked copies to agree
on); the **audio track** double-click shows the inspector rather than an instrument
nobody hears.

## Tests, and what each holds

- `crates/cantus-ui/tests/quick_gestures.rs` (14, new) — whole-window, the pointer driving
  each gesture through `CantusApp::frame` (listed in ADR 0051 *What the tests hold*). With
  the behaviours reverted to master's (accessors kept) ten fail; the other four are guards
  that held before (typing, a click off the letters), the hover (not reverted) and the pure
  rule. Seen failing on the way, for real: a coarser rack grid wrongly accepted (the test's
  pattern was on eighths — fixed the data), double-clicks less than 0.3 s after an earlier
  click (egui counts them; the rig now waits as a hand would), Ctrl+Z needing
  `ModifiersChanged`.
- `crates/cantus-host/tests/clap_window.rs` (+1, Windows) — a second *open* restores a
  minimised plug-in window, keeps a maximised one maximised (**failed** with the old
  `SW_RESTORE`), and puts it above another window of the process.
- Pictures: `examples/shoot_gestures.rs` (`name-hover`, `name-open`, `header-hover`,
  `header-open`, `clip-roll`, `clip-rack`; Night and Paper, EN and ES, 75/100/200 %), in
  `F:\projects\daw-r27\shots\gestures\`. Looked at: the field with the name selected, both
  hints whole in the hint bar at 200 % in Spanish (two lines of prose), Unda opened from
  the header, the piano roll showing the two-bar motif whole and centred, the rack in front
  on the kick.

## Files outside the arrangement and the inspector

`src/gestures.rs` (new), `src/clips.rs` (the rule), `views/piano_roll.rs` (`fit_clip`,
`grid_size`), `views/workspace_device.rs` (`present`, `take_reveals`, `Entry::{reveal,
raise}`, `AreaState::raise`), `views/workspace_frame.rs` (the reveals), `views/fx/mod.rs`
(an effect editor's raise), `views/device_chrome.rs`, `views/inst/mod.rs`,
`views/pulsus_editor.rs`, `views/comp_editor.rs`; `cantus-host`: `window.rs` (`focus`,
`is_minimised`, `floating`), `session_inst.rs`, `session_inst_idle.rs`. Strings:
`locales/areas/gestures.{en,es}.txt` (new), `hosted.show_window`. Docs: ADR 0051, BACKLOG
§6g, the manual EN/ES (*Quick gestures* in chapter 5), the beta's release notes.

## Linux

The new Win32 calls are in `cantus-host`'s existing `#[cfg(windows)] mod win32`; the
non-Windows `PluginWindow` gains `is_minimised` (`false`) beside its no-op `focus`, both
`pub` on a `pub` type (no dead code under `-D warnings`). The new host test is in the
Windows-only `clap_window.rs`. Nothing else is platform-specific.

## Left

- F8 (`toggle_instrument_window_of`) still *closes* a native editor whose tab is open but
  behind another tab (it counts as shown); *Open editor* and the header's double-click
  bring it forward. Whether F8 should do the same is the owner's call (it is a toggle).
- The owner, by hand with a real CLAP/VST3 instrument: minimise its window, *Show* in the
  inspector or double-click the track — it comes back, maximised if it was.
