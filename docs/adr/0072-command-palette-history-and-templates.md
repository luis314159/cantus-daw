# ADR 0072 — The command palette, the undo history and templates

**Status:** accepted (round 31) · **Date:** 2026-10-25 · **Extends:** ADR 0004 (commands and
undo), ADR 0024 (the kit and the workspace), ADR 0029 (the device picker), keymap file
version 11

## Context

BACKLOG §3 *Workflow* and the round's ask (item 4): the daily tools an FL, Ableton, Bitwig
or Studio One user reaches for first and Cantus lacked — a command palette (VS Code's
Ctrl+Shift+P, Bitwig's *Commander*, Ableton 12's search), an undo history panel
(Photoshop's, Studio One's), project templates with a start page, and track templates.
Cantus had no list of recent projects either, which the palette and the start page need.

## Decision

1. **The command palette** (`palette_index`, `views/palette*.rs`). `Ctrl+Shift+P` on every
   preset (⌘⇧P on a Mac: not a chord macOS keeps; keymap version 11 fills it into older
   files where it is free), *View ▸ Command palette…*. One field over a ranked list of
   **every `Action`** (its shortcut at the right), the areas no action toggles (the devices
   panel), the windows (shortcuts, appearance, the top bar's customising, the start page,
   About), **every Settings page**, **every effect and instrument of the device
   catalogue** (an effect is inserted on the selected strip — else the selected track's,
   else the master; an instrument makes a new track after the selected one, through
   `apply_pick`, one undo step), the **recent projects**, the **project templates** and the
   **track templates**. Each entry has its name in the interface's language **and** in
   English; the match folds case and accents and finds every typed word as a subsequence,
   scoring word starts, adjacent letters and a leading match (an English name a little
   below the interface's). The order adds a bonus for uses — `12·ln(1+count)` capped at 36,
   plus up to 12 for the last 20 choices — kept in `palette.toml` beside the keymap; with
   nothing typed, the most recently used come first. Entries are made when it opens; a
   keystroke ranks them again; only the rows in view are painted. Drawn with the kit: a
   floating frame, the shared search field (`link_picker::search_field`), rows with a
   glyph, the name, the detail and the kind's word in its own column, the kit scrollbar.
   A test fails if an `Action` has no entry or no name in either language.
2. **The undo history** (`Pane::History`, `views/history.rs`, `history_text`). A workspace
   area (`Alt+H`, *View ▸ Show the undo history*), placed as a tab beside the inspector and
   docked, stacked or floated like any other. Rows: the project as opened, then every step
   oldest first, then the steps to redo (dimmed). **A click goes to the row by undoing or
   redoing each step between, one at a time, exactly as `Ctrl+Z`/`Ctrl+Y` do** — the
   history is never rewritten, a click on a later row comes back, and a step whose undo is
   refused (the tempo hold during an audio take) stops the walk there. Names are the
   step's `cmd.*` label, capitalised, or a sentence that says what it touched where that
   fits (*Move 3 notes*, *Insert Concentus on Lead*, *Delete track Bass*, *Rename track to
   …*, *Insert track template …*), made only for the rows in view.
   **The saved mark** needed the bus to know states, not revisions: every history entry
   carries a **step number** (`HistoryEntry::step`) that names the state it leaves the
   project in — never reused, kept through undo and redo, renewed when a gesture merges
   more into the step; the base below the oldest step has one too. `CommandBus` keeps the
   saved state (`saved_step`), set by `mark_saved` and by `mark_saved_at(revision)` (a save
   written off the thread names its revision: the bus remembers the last 1 024 revisions'
   states); a saved state no step holds any more (replaced after an undo, merged into, or
   fallen off the history's end) is `None`. The panel tags that row *Saved* and dots the
   steps between it and the current position.
3. **Project templates and the start page** (`templates`, `views/start_page.rs`,
   `views/template_name.rs`). A template is a project file in
   `<data>/templates/projects` — written and read through `cantus-project`, so an old one
   migrates as any project — saved by *File ▸ Save as template…* (its audio paths made
   absolute, so they are found from the templates' folder; a template of the same name is
   replaced, and the panel says so). Four **starters** are built in code from our own
   instruments and effects — *Empty song*, *Beat*, *Band recording*, *Mastering* — never
   files in the repository; their effects are set by hand (`templates::BEAT_REVERB`,
   `BAND_REVERB`, `MASTERING_CHAIN`): a return is full wet as every return Cantus makes,
   with *Dark tail* (Beat) or *Vocal plate* (Band), and Mastering's master chain carries
   the equaliser's *Mastering tilt*, the multiband's *Mastering glue*, the stereo tool's
   *Master: tighten the low end* and the maximiser's *Transparent master*, Speculum as it
   comes. A template carries no automation mode: a saved project's tracks and master, and
   a saved track, come back reading (`templates::read_only`). *File ▸ New from template…* opens the **start page**: a card per
   template (starters first; a user's has a × that deletes it on a second click) and the
   recent projects; a project made from a template is untitled and unsaved, and when the
   open one has unsaved changes the *Save changes?* question comes first
   (`unsaved::Then::Template`). **The start page opens when Cantus starts** (the owner's
   decision, 2026-10-26): `CantusApp::assemble` calls `workflow::open_at_start`, unless a
   project was given to open (`Files::project_path`: the command line, a file from
   Explorer) or the setting *Show at start* (`Settings::start_page_at_start`, on by
   default; the page's own switch and *Settings ▸ General*) is off. A file opened later
   (Finder, the Dock, a drop: `open`) and work restored from a crash (`restore_recovery`)
   close it. It waits — open, not drawn — behind the crash-recovery offer and the audio
   first-run setup (`workflow::start_page_waits`), and keeps clear of the start verse's
   card (at its left when there is room, else above it); a click on the card does not
   close it. The start-up plug-in scan is quiet (its result goes to the status line), so
   nothing else overlaps. Closing it leaves the song Cantus opened on. The update's
   relaunch opens no project, so the page shows then as at any start.
4. **Track templates.** *File ▸ Save track as template…* writes, to `<data>/templates/tracks`,
   a project holding one track — instrument, colour, icon, level, balance, input,
   modulators; not its clips, takes, comps, freeze or delay, never muted, soloed or armed —
   its strip (inserts, EQ, fader, pan, width, sends, output; not frozen, in no group) and,
   as named stand-ins, the strips its sends, output and side-chain keys reach. Inserting
   one (the device picker's **Track templates** shelf — `Source::Template`,
   `PickChoice::TrackTemplate`, also the devices panel's drag — and the palette) is **one
   undo step** (`cmd.insert_track_template`: a batch of `RemoveStrip` + `InsertStrip` into
   the first blank strip's place, else after the desk's middle, then `InsertTrack`): every
   id is new (track, strip, inserts, modulators, their routes), and a stand-in is found **by
   name** in the project (`routing_actions::strip_name`, case and accents aside) — a send
   to *Reverb* reaches that project's *Reverb* return; what is not there is left out (an
   output to the master) and the status line names it. Every automation target a
   modulator can ride is mapped by name in an exhaustive match, so a new kind of target
   must say how a template carries it. It is never swapped in for a track's instrument.
   The start page lists the user's track templates under the recent projects, where a ×
   (clicked twice) deletes one.
5. **Recent projects** (`recent.rs`): the last 12 opened or saved, newest first, in
   `recent.toml` beside the settings; a file that is gone leaves the list when opening it
   fails.
6. **Wiring.** The templates' folder, the recent list and the palette's uses are set in
   `CantusApp::assemble` (`workflow::wire`), the boot's path; the shared files carry one
   line each (`app.rs`'s field, frame call, `run_action` hook; the menus; `workspace.rs`'s
   pane; `save_job.rs`'s recent note).

### Amended after the round's review

- A template asks its hosted plug-ins for their state and waits as a save does, then is
  written from `project_for_save`; a plug-in kept as last said is named.
- A track template's stand-ins are named by the strip's own name or its first track's,
  never by its number; the blank strip the new one replaces is chosen first and never a
  target. A track template drops on a track's header (it lands after it), and lands
  after the selected track from the picker and the devices panel. Its note-effect chain
  (ADR 0069) comes with new ids, and `AutoTarget::NoteFx` is mapped with it.
- The history marks no step *Saved* while the project has no file; a save names the
  history state it took (`mark_saved_state`). The start page reads the disk when it opens
  and when a template changes. `recent.toml` and `palette.toml` are written through a
  temporary file. The palette's own key closes it, its field focused or not.

### The audio cores as engines (the owner's, 2026-10-27)

Settings ▸ Audio's *Audio cores* shows each count with its engine tier and a model known
for it, by the model's name only, with a silhouette drawn with the kit's vectors — the
body, cabin and wings as convex shapes, the windows and wheel arches cut in the ground's
colour, wheels with hubs, one detail in the theme's selection colour — at the right of
each row (as tall as the row) and large under the field for the chosen count
(`engines::paint`, `Look`; `shoot_ship engines` draws all ten; `engines.rs`: one table,
`TABLE`, the counts each tier starts at; a count between two takes the one below). The
field and the line under it count alike — **cores at work, the audio thread and its
helpers** (a setting of *n* helpers is *n* + 1 cores; `settings_audio::workers_count`,
`now_line`, `perf_status::pool_words`) — so one screen never shows two engines for one
pool. **Owner item:** model names can be registered marks too — a legal
review may swap them in that one table.

## Consequences

- The keymap file is version 11 (`Ctrl+Shift+P`, `Alt+H`); five new actions —
  `command-palette`, `view-history`, `new-from-template`, `save-as-template`,
  `save-track-template` — in the shortcuts dialog, the menus (and the macOS menu bar) and
  the palette.
- No project schema change: templates are projects, and `HistoryEntry::step` lives only in
  memory.
- Track templates survive model changes the way projects do (they are projects); a hosted
  plug-in in one loads as it does in any project.
- The device picker gains a shelf only when the user has track templates.

## What the tests hold

- `cantus-core` `bus.rs`: the saved state is known through undo past it and redo back, a
  save landing at an older revision, a merge into the saved step (gone), new steps over
  undone ones (gone), a reset; the base follows the history's end.
- `tests/workflow.rs`: every action has a palette entry, its shortcut and a name in both
  languages, as do every Settings page, window and area; Spanish words, English words and
  abbreviations find the same entries, and uses reorder close matches; the palette's key
  opens it, a row runs and is counted (and kept on disk), an effect lands on the selected
  track's strip in one step; the history's rows, names and saved mark, a click undoing and
  redoing several steps and back to the start, `Alt+H`; every starter saves, loads equal
  and has consistent routing; *New from template* (and the unsaved question); a user's
  template kept, listed, opened and deleted (and nothing outside the store deleted); a
  track template inserted as one step with new ids, its send finding *Reverb* by name, its
  modulator riding the new strip, undo restoring the project exactly, a missing return
  said; the picker's shelf and `apply_pick`; recent projects on save and open, a gone one
  dropped.
- `tests/workflow_boot.rs`: the boot's `assemble` wires the templates' folder, the recent
  list and the palette's uses.
- `keymap.rs`: a version-10 file gets the two keys where free; every preset has the
  palette's chord; ⌘⇧P is not a chord macOS keeps.
- `history_text.rs`, `recent.rs`: names with counts and devices; the list's order, cap and
  file.
