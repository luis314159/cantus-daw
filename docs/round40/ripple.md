# Round 40 — `ripple`: insert and delete time, the section drag, the ripple mode

Builder: *ripple* · branch `r40/ripple` (base `f00ed550`) · ADR 0116 · keymap 16 → 17 · no
schema bump · locale area `ripple` · manual: `docs/manual/{en,es}/arrangement.md`.

## 1. What the leaders do (research, with the pages)

| DAW | Insert / delete time | Clips across the edge | What moves | Ripple |
|---|---|---|---|---|
| FL Studio | Playlist *Insert space* (Ctrl+Ins) and *Delete space* (Ctrl+Del) over the time selection; *Slice and insert space* (Ctrl+Alt+Ins) | sliced at the selection start (the slicing variant) | pattern clips; markers have their own "Move content" entries | none (a time selection, Shift+drag copies it) |
| Ableton Live | *Insert Silence* (Ctrl+I), *Delete Time*, *Cut / Paste / Duplicate Time* | not documented; clips either side close up | "act upon all tracks"; time-signature markers too | no mode |
| Logic Pro | *Insert Silence Between Locators*, *Snip: Cut Section*, *Splice: Insert Snipped Section* | cut at the locators | regions; global events (tempo, signature, markers) only when everything is selected | *Shuffle L / R* drag modes; arrangement markers: drag moves the section, the rest close up and make room, Option copies |
| Cubase | *Edit ▸ Range ▸ Insert Silence*, *Delete Time*, *Cut Time* | events across the range start split | the tracks the range covers; the tempo track only if selected (the most reported trap: "Insert Time is a mess") | *Shuffle* mode (per track) |
| Pro Tools | *Event ▸ Time Operations ▸ Insert Time / Cut Time* | *Insert Silence* splits in Shuffle | options for the conductor rulers (tempo, meter, key, chords); markers don't follow clips moved by hand | *Shuffle* edit mode (F1), per track |
| REAPER | *Insert empty space in time selection*; *Remove contents of time selection (moving later items)* | items split as needed | "all tracks are affected … regardless of the current ripple mode" | **off / per track / all tracks** (Alt+P) |
| Studio One | *Insert Silence*, *Delete Time* — "always ripple, whether or not ripple mode is on" | — | the arranger moves "Events, Parts, Markers, tempo changes, and automation" | *Ripple Edit* button (selected events); arranger: drop between = insert, onto = replace, Alt copies, Shift partial replace |

Sources:
- FL Studio Playlist: <https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm>; shortcuts: <https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_shortcuts.htm>; FL Studio Mobile playlist ("Clips straddling the selection will be sliced at the start of the selection"): <https://www.image-line.com/fl-studio-learning/fl-studio-beta-online-manual/html/plugins/FL%20Studio%20Mobile_Playlist.htm>
- Ableton Live 12 manual, Arrangement View: <https://www.ableton.com/en/live-manual/12/arrangement-view>; keyboard shortcuts: <https://www.ableton.com/en/manual/live-keyboard-shortcuts>
- Logic Pro, add or remove gaps: <https://help.apple.com/logicpro/mac/10.1/en.lproj/lgcp21e2cb21.html>; arrangement markers: <https://help.apple.com/logicpro/mac/10.1/en.lproj/lgcpf7c0a3d7.html>
- Cubase / Nuendo, inserting silence: <https://archive.steinberg.help/nuendo/v13/en/cubase_nuendo/topics/range_editing/range_editing_inserting_silence_t.html>; users on the scope trap: <https://forums.steinberg.net/t/insert-time-is-a-mess/1017471>, <https://forums.steinberg.net/t/insert-silence-upsets-time-and-temp/966569>
- Pro Tools, Insert Time options: <https://apps.avid.com/proToolsFirstHelp/version12.0/enu/Pro%20Tools%20First%20Help/Arr1.conductors.31.68.html>; Time Operations: <https://pcaudiolabs.com/time-operations-window-in-pro-tools/>; Sound On Sound on markers not following clips: <https://www.soundonsound.com/techniques/pro-tools-losing-time>
- REAPER: ripple modes <https://ask.audio/articles/ripple-editing-in-reaper>; insert empty space (all tracks, regardless of ripple): <https://reaperaccessibility.com/wiki/Editing>; podcast editing in all-tracks ripple: <https://www.soundonsound.com/techniques/reaper-speedy-podcast-editing>
- Studio One / Fender Studio Pro arranger track: <https://fenderstudiopromanual.fender.com/en/Content/Arranging_Topics/ArrangerTrack.htm>; Ripple Edit: <https://www.soundonsound.com/techniques/ripple-edit-studio-one-4>

(Some vendor pages — Avid's, Steinberg's archive — answered 403 to the fetch; their content is
cited from the search engine's extracts and the third-party pages above.)

**What we copied and where we differ** (ADR 0116 has the reasons): the time edits are the
whole song's, always (Live, REAPER, Studio One; not Cubase's selected tracks); clips across the
edge are split (all); tempo, metre, markers, sections, loop and punch move (Studio One's
arranger list); the ripple mode is REAPER's three-way switch. **Different:** delete time heals
the cut an insert left (none of them does); the plain section drag replaces, Shift inserts (the
approved sheet; Logic and Studio One insert by default).

## 2. What changed

- `crates/cantus-core/src/ripple.rs` — `insert_time`, `delete_time`, the `Shift` mapping,
  the scratch `Edit`, the clip passes (cut, clear, move through the tempo maps, heal), the
  alternate comps (`with_comps`), `Scope` for the ripple mode.
- `ripple_tempo.rs` — the tempo track and the metre changes through an edit (ramps cut, seams,
  needless points), and a section's clock carried by an inserting drag.
- `ripple_marks.rs` — markers, sections, loop, punch.
- `ripple_bars.rs` — `move_bars` with the four `Landing`s; a section put in inside another
  cuts it around itself.
- `ripple_mode.rs` — `RippleMode` and `rippled` (a gesture plus what follows it).
- `crates/cantus-ui/src/ripple_ui.rs` — the app's *Insert time*, *Delete time*, the section
  drop, `SectionDrag` and `landing_for` (the one place keys and the setting become a
  landing), the mode's switch and its follow on clip delete / move / right-edge trim / paste; words.
- `views/ripple_settings.rs` — *Settings ▸ Editing ▸ Section drag*: Insert (default) | Replace,
  with its note; `Settings::section_drag` (an old settings file opens with Insert).
- The sections lane (`arrangement_sections.rs`): the drag rides as a preview (ghost, accent
  band or insert line through every lane, dashed slot, help line with the keys for the
  setting) and drops one command; Alt for free; the menu's *Move the section to the time
  selection*.
- The arrangement: the selection's *Time* band (a right-click inside the selection, over a
  clip or not), the panel menu's *Insert time*, *Delete time* and the ripple mode's three
  entries, the overlay call; the tool row's ripple key (`clips_chrome::ripple_right`);
  `ArrangementView::ripple`.
- Keymap 17: `InsertTime` (Insert), `DeleteTime` (Ctrl+Del, Ctrl+Backspace), `CycleRipple`
  (Alt+Shift+P).
- Locales: `areas/ripple.{en,es}.txt`. Manual page in both languages, linked from the README.
  `docs/beta-checklist.md` has a *Ronda 40* item.

**Files outside the brief's list** (all additive, marked `// r40:ripple`): `app.rs` (dispatch;
the ripple hooks in `delete_clip_selection` and `commit_clip_drag`), `settings.rs` (one field),
`keymap.rs`, `views/arrangement.rs`, `views/arrangement_tools.rs` (one call),
`views/clips_chrome.rs` (`ripple_right`), `views/clipfx_settings.rs` (one call),
`views/mod.rs` (one module).

## 3. How it is tested

See ADR 0116 §*What the tests hold*. In short: unit tests of every part; the property
**insert then delete = identity to the tick on 400 random songs**; undo/redo whole for every
edit and drag; the undo-fuzz arm (`// r40:ripple`); a **sample-exact render** with one tempo
and under a tempo track; an **end-to-end test through `CantusApp::assemble`** — the menu and
keys, the section drag's landings under both *Section drag* settings (the default an old
settings file opens with, and Replace, stored), the ripple mode's key, its sign in the tool
row and its follow on a delete and a drag — each one undo step, saved and loaded.

**Suites (Windows, through the slots, 2026-11-12):** cantus-core and cantus-engine 1 923
passed, 0 failed; cantus-ui 2 654 passed after two fixes the full run found — Slate's window
goldens re-blessed for the ripple key (the only pixels that moved: its 24 × 20 pt in the tool
row, diffed) and `arrange_e2e`'s two-bar section drag run under *Replace* (inside its own bars
the default *Insert* leaves a section where it is). Clippy `--all-targets -D warnings` clean on
the three crates; `cargo fmt -p` for each. Linux not run (the integrator's).

## 4. Verified only in emulation

Everything here is model and interface work tested headless; nothing needs a device. The
pointer gestures and keys are synthetic egui events (as every e2e test here): the owner's own
hands on a mouse have not tried the section drag's modifiers nor the ripple mode. The sound is
verified by the offline render (bit-exact), not by ear.

## 5. What the owner should look at

Pictures in `docs/round40/ripple/` (Slate Night and Paper, 1920 × 1080 at 125 %):

- `ripple-menu-*.png` — a time selection (bars 5–6) and its menu with the *Time* band:
  *Insert time · Insert*, *Delete time · Ctrl+Delete*.
- `ripple-inserted-*.png` — after *Insert time*: the new bars selected and empty on every
  track, the Drop and Outro sections and the marker moved on.
- `ripple-deleted-*.png` — after *Delete time* over bars 5–6.
- `ripple-drag-insert-*.png` — the Intro dragged to bar 13 (a plain drag, by default): the
  ghost, the accent insert line with its cap, the dashed slot, the help line.
- `ripple-drag-replace-*.png` — the same replacing (Shift by default): the accent band over
  the Outro's bars on every track.
- `ripple-drag-copy-*.png` — a copy replacing (Ctrl+Shift by default; no slot left).
- `ripple-drag-insert-copy-*.png` — the Drop copied in at bar 17 (Ctrl by default).
- `ripple-mode-track-*.png` — the ripple key lit, "Ripple · this track", after the Kick's
  first clip was deleted: its second clip closed the gap; the other tracks unmoved.
- `ripple-mode-all-*.png` — the key in its every-track mode.
- `ripple-settings-*.png` — *Settings ▸ Editing* with *Section drag: Insert | Replace*.

To try: select bars in the ruler, right-click inside them (on a clip or not), *Insert time*;
Ctrl+Del takes them out again (the song comes back whole). Drag a section with nothing, Shift,
Ctrl and Ctrl+Shift held; switch *Section drag* to Replace and drag again. Alt+Shift+P (or the
key beside *Follow*) and delete or drag a clip.

## 6. What is left

- **The owner's hands**: the drag's modifiers and the ripple mode with a real mouse
  (`docs/beta-checklist.md`, *Ronda 40*).
- Said limits (ADR 0116 *Consequences*): a ramp across a deleted range's start is drawn to the
  value at its end; a looping clip cut between repeats is written out (sounds the same); an
  unwarped audio window under a tempo track that changes over its hidden part heals only where
  one tempo holds. The ripple mode ripples a delete, a move along the track, a right-edge trim
  and a paste; not a left-edge trim or a move to another track (as REAPER's per-track mode,
  which moves what follows an item's end).
