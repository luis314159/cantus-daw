# Round 23 — clipboard: Ctrl+C / Ctrl+X / Ctrl+V in the piano roll and the arrangement

Builder: **clipboard** (`r23/clipboard`, base 2ad043b). Decision record: ADR 0036.
BACKLOG-excellence §6d.1 (beta blocker).

## Why

Cut, copy and paste existed and had tests, but **in the real window the three keys did
nothing**: egui-winit 0.36.1 turns Ctrl+C / Ctrl+X / Ctrl+V (with Shift or Alt, and
Windows' Ctrl+Ins / Shift+Del / Shift+Ins) into `Event::Copy` / `Event::Cut` /
`Event::Paste(text)` and sends no key press for them, while `Keymap::pressed` only read
key presses — the tests injected key events or called `run_action`, so they passed. The
study (round-23 design note) also found: the piano roll never took the edit focus; cut
then paste in the arrangement, and paste onto an empty track, pasted nothing; clip paste
rounded to the nearest grid line and ignored the snap switch; no rules for automation
clips, take lanes, audio sources or overflowing rows; copy needed a selection; time
selections could not be copied; wrong undo labels. Every one of them is fixed and has a
test that failed first.

## Done

**Core — `cantus_core::clipboard`** (pure; no JSON, no I/O): the `Clipboard` type
(`format: "cantus-clip/1"`, origin = per-run nonce + project epoch, ppq, tempo, audio
sources with absolute paths, a `Notes` or `Clips` payload — rows from the topmost track,
starts from the earliest clip or the time range's start, `span` for a range), and the
builders `copy_notes`, `copy_clips`, `clips_payload`, `cut_clips`, `paste_notes`,
`notes_clip`, `paste_clips`, `copy_of` (moved from `cantus-ui/src/clips.rs`, re-exported
there; `clips::paste` is gone). Every cut and paste runs as **one `Command::Batch` with its
own label**: no new command, no schema change, no new crate.

**UI — `cantus-ui/src/clipboard.rs`:**

- The system clipboard is the clipboard: a copy writes compact JSON (`format` first) with
  `ctx.copy_text` — it survives closing Cantus and reaches another project or window —
  and keeps the copy in memory for when the system text is not ours. Text of a newer
  format version is refused with a status, never misread.
- Keys: `keymap::clipboard_binding` turns the three events back into the chord pressed;
  `Keymap::pressed` fires a binding on its key **or** its chord's event. Rebinding works;
  **Ctrl+Shift+C (clip colour) now works too** (it was dead for the same reason); the
  shortcuts dialog can capture the chords (`captured_binding`).
- Paste from Ctrl+V uses the event's text at once; a menu's Paste sends
  `ViewportCommand::RequestPaste` and pastes when the answer arrives (two frames later in
  eframe; the event is taken out of the input so Ctrl+V's binding cannot fire twice), or
  from memory after 3 frames and 0.1 s when none comes; a late answer after that is
  dropped. Exactly once. Headless: from memory at once.
- Focus: Cut / Copy / Paste go by `EditFocus` alone; the piano roll and the channel rack
  claim the focus on any press in their area (`piano_roll::claim_focus`, before the early
  returns). `CantusApp::last_selection` (clips or time range, whichever was touched last)
  decides what an arrangement copy takes.
- Copy: notes (selection, or every note the clip shows, relative to the clip's edge);
  clips (with their audio files); a **time range** — every lane-0 clip over it on every
  track, cropped with the arrangement's own split on a scratch copy (fades and reversed
  audio right), and looping clips cut where no repeat begins rebuilt so they play what
  was heard (`cantus_core::clipboard::time_crops`).
- Cut: `cmd.cut_notes`, `cmd.cut_clips`, `cmd.cut_time` (splits, removes the inside, no
  ripple). A paste afterwards needs no selection.
- Paste: anchor = the empty-timeline menu's point, else the time selection's start, else
  the playhead, **snapped down** when snap is on (raw when off); track = the menu's, the
  selected or the first; rows kept, a block past the last track moves up whole; copies
  with fresh ids on lane 0, selected; automation clips back to their own track from the
  same project and run, skipped (and counted in the status) from another; audio sources
  reused (same id / hash / file) or inserted fresh with **`recorded = false`**, resized
  for another tempo; notes into the piano roll grow the window (or a loop's period); notes
  into the arrangement make a whole-bar clip; clips into the piano roll say so.
- Menus: the Edit menu already went through `run_action` with the keys shown; the
  empty-timeline menu's *Paste* shows its key and lands where the menu was opened
  (`arrangement::paste_at_menu_point`, the point captured at the right-button press).
- Labels: `cmd.cut_notes`, `cmd.cut_clips`, `cmd.cut_time`, `cmd.paste_clips`,
  `cmd.duplicate_clips` (now also the clip duplicate and the Alt-drag copy) and nine
  status lines, in `locales/areas/clipboard.{en,es}.txt`.

**Deviations from the design note** (each in ADR 0036):

- Tempo scaling of a foreign audio clip scales its window, offset and fades but **not its
  start**: the start stays on its beat, so a copied block stays aligned to its bars.
- A whole-clip note copy is relative to the clip's left edge, not to the earliest note.
- Looping clips are not cropped with `split` (which sets an offset past the loop brace
  and silences the right half: an existing split defect for looping clips, untouched
  here): their pieces come from `loop_pieces` (notes written out; automation as the rest
  of the repeat plus the loop again).
- Pasting notes into a looping clip grows its loop by whole loops, repeating its notes in
  the new room, and grows the window to show the new loop (otherwise they are never
  heard).
- The new state is one field, `CantusApp::clipboard: ClipboardState` (board, paste text,
  pending menu paste, one-shot anchor, session nonce), plus `last_selection`.
- The coverage check in the undo fuzz is new: it asserts every clipboard case really ran.

## The adversarial review, and what it changed

Two reviewers read the branch; eight findings, all checked against the code and all real.
Each got a test that failed against the code as it was (seen red, by running it or by
putting the old behaviour back), then the fix.

1. **A time cut through a looping automation clip changed the rest of the clip** (major).
   Looping automation went through `split`, whose right piece reads past the brace.
   Fixed with a loop-aware crop for every looping clip: a piece that does not start on a
   repeat is, for automation, two clips (the rest of the repeat played once from the
   brace tick heard there, then the loop again); notes are still written out once. Test:
   `clipboard_crop::a_time_cut_through_a_looping_automation_clip_leaves_the_curve_outside_it_unchanged`
   (the compiled curve, sampled every 1/8 beat).
2. **Pasting notes past the end of a looping clip was silent** when the window was one
   loop long (the usual case: Loop on sets period = length), and a stretched period
   dragged later repeats off the bar (major). Now the loop grows by whole loops, the new
   room repeats the loop's notes, and the window grows to show the new loop. Test:
   `pasting_past_a_loop_as_long_as_its_clip_grows_both_by_whole_loops_and_keeps_what_played`.
3. **A DAWproject import kept the epoch** (minor), so a copy from before counted as
   "this project" and resolved ids in the wrong one. Now the import, and the undo or redo
   of any `ReplaceProject`, bump it. Test:
   `a_dawproject_import_and_its_undo_make_an_earlier_copy_foreign`.
4. **The time cut was never fuzzed** (minor). The crop and the `split` it uses moved to
   `cantus_core::clipboard` (`time_crops`, `split`, `split_audio`; `clips::split`
   re-exports), and the undo fuzz got arm 30: a time cut whose batch first lays a looping
   note clip, a looping automation clip or a reversed audio clip with fades under the
   range, with a coverage assertion for the loop-phase automation crop.
5. **The clipboard did not carry the project schema** (minor), so a newer build's clips
   could be misread. The text now carries `schema` (= `SCHEMA_VERSION`): newer is refused,
   older is read while it parses and refused with a new status line otherwise. Tests:
   `clipboard::tests::the_project_schema_decides_whether_a_clipboard_is_read`,
   `the_copied_text_carries_the_project_schema`.
6. **A menu paste pasted twice in the real window** (major). eframe delivers a
   `RequestPaste` answer two frames later, not one; the old code fell back to memory in
   the frame between and then pasted the late answer too. Now it waits for the answer or
   3 frames and 0.1 s, and drops a late answer that comes with no modifier held. Tests:
   `menu_paste_waits_for_the_answer_eframe_sends_two_frames_later_and_pastes_once`,
   `menu_paste_with_no_text_on_the_clipboard_falls_back_once_and_ignores_a_late_answer`
   (they replace `menu_paste_requests_the_system_clipboard_and_pastes_exactly_once`).
7. **A clip's right-click Cut cut a leftover time range across every track** (major).
   Opening the clip menu now makes the clips the last-touched selection
   (`arrangement::clip_menu_opened`). Test:
   `the_clip_menu_cuts_the_clicked_clip_not_a_leftover_time_range`.
8. **Shift+Del / Shift+Ins were recorded as Ctrl+X / Ctrl+V in the shortcuts dialog**
   (minor). Capture now takes a clipboard event only with Ctrl held
   (`keymap::captured_clipboard_binding`); Ctrl+Ins cannot be told from Ctrl+C and is
   documented as such. Test: `keymap::tests::capture_records_clipboard_events_only_with_ctrl`.
   The second half of the finding (a status when Ctrl+V gives no event) cannot be done:
   egui-winit sends neither an event nor a key press then, so the app never learns that
   Ctrl+V was pressed. Documented in ADR 0036's consequences and the module docs.

## Tests

| Where | Tests |
|---|---|
| `cantus-core/tests/clipboard.rs` (new, 14) | `copied_notes_start_at_zero_and_keep_pitch_velocity_length_mute`, `pasted_notes_land_at_the_anchor_with_fresh_ids_and_undo_in_one_step`, `pasting_past_the_clip_end_grows_the_window_and_undo_shrinks_it`, `pasting_into_a_looping_clip_grows_its_period_so_the_notes_sound`, `copied_clips_keep_rows_and_spacing`, `pasted_clips_keep_relative_track_offsets`, `a_block_below_the_last_track_moves_up_whole_instead_of_squashing`, `a_pasted_clip_shares_no_ids_with_its_original_and_lands_on_lane_0`, `automation_clips_go_back_to_their_track_in_the_same_project_and_are_skipped_from_another`, `audio_sources_are_reused_or_inserted_before_the_clip_and_undo_in_a_legal_order`, `an_audio_clip_pasted_at_another_tempo_plays_the_same_part_of_its_file`, `cut_clips_then_undo_restores_the_project_exactly`, `notes_pasted_into_the_arrangement_make_a_clip_of_whole_bars`, `clipboard_serde_roundtrip` |
| `cantus-core/tests/clipboard.rs` (review) | `pasting_past_a_loop_as_long_as_its_clip_grows_both_by_whole_loops_and_keeps_what_played` (15 in the file) |
| `cantus-core/tests/clipboard_crop.rs` (new, 3) | `a_time_cut_through_a_looping_automation_clip_leaves_the_curve_outside_it_unchanged`, `a_copy_of_a_range_inside_one_loop_is_one_clip_that_plays_what_was_heard`, `a_range_on_clip_edges_takes_whole_clips_and_leaves_neighbours_alone` |
| `cantus-core/tests/undo_invariants.rs` | arms 27 (paste of clips: automation, a foreign audio file, either origin), 28 (cut of clips or notes), 29 (note paste that may grow its clip), 30 (time cut over a freshly laid looping note clip, looping automation clip or reversed audio clip); `rng.below(31)`; a per-case "really applied" assertion over seven cases |
| `cantus-ui/tests/clipboard.rs` (new, 19; the whole `CantusApp::frame` with egui-winit's events) | `ctrl_c_arrives_as_an_event_and_copies_to_the_system_clipboard`, `copy_with_nothing_selected_copies_the_whole_clip`, `ctrl_x_cuts_in_one_undo_step_named_cut`, `event_paste_with_our_text_pastes_and_foreign_text_falls_back_to_the_app_clipboard`, `the_clipboard_survives_a_restart_and_crosses_projects`, `cut_then_paste_clips_in_the_arrangement_pastes_them_back`, `paste_onto_an_empty_track_works`, `the_empty_timeline_menu_pastes_where_it_was_opened`, `the_piano_roll_takes_the_edit_focus_when_clicked`, `paste_at_the_playhead_snaps_down_not_to_nearest`, `snap_off_pastes_at_the_exact_playhead`, `paste_lands_at_the_time_selection_start`, `notes_paste_into_the_piano_roll_at_the_snapped_playhead`, `copying_a_time_range_crops_clips_across_tracks`, `cutting_a_time_range_splits_removes_and_undoes_exactly`, `last_touched_selection_decides_between_time_range_and_clips`, `menu_paste_waits_for_the_answer_eframe_sends_two_frames_later_and_pastes_once`, `menu_paste_with_no_text_on_the_clipboard_falls_back_once_and_ignores_a_late_answer`, `a_paste_is_one_undo_step_named_for_what_it_pasted`, `every_clipboard_label_resolves_in_both_locales`; after the review also `the_clip_menu_cuts_the_clicked_clip_not_a_leftover_time_range`, `a_dawproject_import_and_its_undo_make_an_earlier_copy_foreign`, `the_copied_text_carries_the_project_schema` (23 in the file) |
| `cantus-ui` unit | `keymap::tests::{clipboard_events_fire_cut_copy_paste, capture_records_clipboard_events_only_with_ctrl}`; `clipboard::tests::{our_text_reads_back_as_what_was_written, a_newer_version_is_refused_and_foreign_text_is_ignored, the_project_schema_decides_whether_a_clipboard_is_read, a_session_number_is_drawn}` |
| `cantus-ui/tests/app_flows.rs` | `note_editing_operations` moved from the removed `copy_selection` / `paste_clipboard` / `cut_selection` to `run_action(Copy/Paste/Cut)` and `clipboard.board` |

Red first: the 14 core tests (stubs), then 8 of the 19 UI tests against the real app before
the key and focus fixes (every event-driven one and the focus one).

Final results on this branch after the review (`CARGO_INCREMENTAL=0`, see Notes):

- `cargo test -p cantus-core`: **132 passed**, 0 failed.
- `cargo test -p cantus-ui --no-fail-fast`: **660 passed**, 0 failed, 13 ignored (646 through
  cargo; the three `audio_setup*` binaries cannot be started by cargo on this machine —
  Windows' installer detection asks for elevation for an `.exe` named `*setup*`, os error
  740 — and were run under another file name: 4 + 4 + 6 passed).
- `cargo test -p cantus-project`: 47 passed, 1 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- rustfmt: the files this branch created, plus `app.rs`, `keymap.rs` and
  `undo_invariants.rs` (clean at the base), are formatted; the workspace as a whole was not
  fmt-clean at the base (e.g. `views/arrangement.rs`) and is left as it was.

## Looked at

`cargo run -p cantus-ui --example shoot_clipboard -- <dir> [es]` (new example, separate
from `shoot.rs` so the export builder's shot does not collide): `clipboard-edit-menu.png`
(the Edit menu open: Cut Ctrl+X, Copy Ctrl+C, Paste Ctrl+V, Duplicate Ctrl+B; the status
line "4 notes copied") and `clipboard-pasted.png` (a two-track block copied and pasted at
bar 5: spacing in time and tracks kept, both copies selected, the automation clip not
copied with it).

## Files

New: `crates/cantus-core/src/clipboard.rs`, `crates/cantus-core/tests/clipboard.rs`,
`crates/cantus-core/tests/clipboard_crop.rs`,
`crates/cantus-ui/src/clipboard.rs`, `crates/cantus-ui/tests/clipboard.rs`,
`crates/cantus-ui/examples/shoot_clipboard.rs`,
`crates/cantus-ui/locales/areas/clipboard.{en,es}.txt`, `docs/adr/0036-clipboard.md`, this
file.

Edited: `cantus-core/src/lib.rs` (module), `cantus-core/tests/undo_invariants.rs`,
`cantus-ui/src/lib.rs` (module), `cantus-ui/src/app.rs` (the two old clipboard fields →
`clipboard` + `last_selection`; the six old copy/cut/paste methods removed; Cut/Copy/Paste
dispatch; **`global_shortcuts`: two lines before the action loop and one after it**;
`captured_binding`; `last_selection` set in the five selection setters; the two
duplicate labels; `with_ids`, `exec_clip_batch`, `validate_selection` made `pub(crate)`;
`undo` / `redo` bump the epoch for a `ReplaceProject`),
`cantus-ui/src/keymap.rs`, `cantus-ui/src/clips.rs` (`split` re-exported from the core),
`cantus-ui/src/audio_clips.rs` (`split_audio` moved to the core), `cantus-ui/src/i18n.rs`,
`cantus-ui/src/views/{arrangement,piano_roll,channel_rack,interop}.rs`,
`cantus-ui/tests/app_flows.rs`.

## Left

- **A manual test in the real build on Windows**: Ctrl+C in Cantus, close it, open another
  project, Ctrl+V; Ctrl+V after copying text in Notepad (falls back to the in-memory copy;
  after a restart, nothing); two Cantus windows; the Edit menu's Paste. The headless tests
  drive egui with the events egui-winit sends, but not winit/arboard themselves.
- Ctrl+V does nothing when the system clipboard holds no text (an image copied elsewhere),
  even if Cantus copied before — egui-winit sends no event then. The menu path falls back.
- egui's selectable stock labels answer `Event::Copy` too and would overwrite our text
  while a label selection exists (a dialog); `selectable_labels = false` in
  `design::install_style` if it is ever seen.
- Automation clips are skipped across projects and across a re-open (new epoch); remapping
  track-relative targets is later. Linked clips (§6d.2) need `cantus-clip/2`.
- `clips::split` of a looping clip (the playhead split, not the clipboard) sets the right
  half's offset past the loop brace and silences it — found here, not fixed (outside this
  scope); the clipboard's time-range crop avoids it.
- Not done (optional in the brief): the soak's random actions do not yet include copy, cut
  and paste; `shortcuts.text` does not mention the keys; the clip menu bar entry has no
  Cut/Copy (the clip's context menu and the Edit menu do).
- `docs/STATUS.md` and BACKLOG §6d.1 are the integrator's to update.

## Notes for the integrator

- **`global_shortcuts`**: my change is two lines before the `for action in
  self.keymap.pressed(…)` loop (a comment and `self.take_paste_event(ctx);`) and
  `self.settle_paste(ctx);` right after the loop. The beta-hardening builder gates shortcuts while the export dialog is
  open in the same function; keep both (the paste stash can sit after their gate).
- `i18n.rs`: one line in each list (after `recovery`); additive.
- `app.rs` also has `undo` / `redo` bumping the epoch for a `ReplaceProject` (a const
  `REPLACE_PROJECT` above `CantusApp`), and `views/interop.rs` one `app.epoch += 1;` after
  a DAWproject import.
- `clips::split` and `audio_clips::split_audio` moved to `cantus_core::clipboard`
  (`clips::split` is a re-export, so callers are unchanged). If another branch edited
  either function, carry its change over to the core copy.
- ADR number: written as 0034, renumbered 0036 at the merge (the landing page took 0034).
- Builds on this machine hit an MSVC link error (`LNK2019` on `anon.*.llvm.*` symbols in
  `cantus_core`, `cantus_engine`, `cantus_host`) with incremental compilation after the
  first edits; `cargo clean -p` did not cure it, `CARGO_INCREMENTAL=0` did. If the merged
  tree shows the same, build non-incrementally.
