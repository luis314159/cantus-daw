# ADR 0036 — Clipboard: cut, copy and paste of notes, clips and time ranges

**Status:** Accepted · **Date:** 2026-10-09 · **Round:** 23 (clipboard) · **Extends:** ADR 0004 (commands), ADR 0010 (keymap), ADR 0025 (audio clips), ADR 0027 (automation clips)

> Numbering: written as 0034 on `r23/clipboard`; renumbered 0036 at the merge, as the
> landing-page session took 0034 (`0034-website.md`) and the export took 0035.

## Context

Cut, copy and paste existed in an early form (two in-memory clipboards on `CantusApp`, one
for notes, one for clips) and passed their tests, but a static study of the code and of
egui-winit 0.36.1 found that **in the real window Ctrl+C, Ctrl+X and Ctrl+V did nothing**:

1. egui-winit turns every Ctrl+C / Ctrl+X / Ctrl+V — with Shift or Alt too, and on Windows
   Ctrl+Ins / Shift+Del / Shift+Ins — into `Event::Copy`, `Event::Cut` and
   `Event::Paste(text)` and returns **before** it pushes an `Event::Key`. `Keymap::pressed`
   only matched key presses; the tests injected key events, or called `run_action`. The
   same hid Ctrl+Shift+C (the clip colour) and made the shortcuts dialog unable to capture
   the three chords.
2. The piano roll never took the edit focus, so after any click in the arrangement the
   keys acted on clips while the user edited notes.
3. Cut then paste in the arrangement pasted nothing; paste onto an empty track did nothing
   (both fell through to the note paste because the clip selection was empty).
4. Clip paste rounded the playhead to the *nearest* grid line and ignored the snap switch;
   note paste ignored the snap switch.
5. No rules for automation clips, take lanes, audio sources, or a block that runs past the
   last track (it was squashed onto it).
6. Copy needed a selection, time selections could not be copied, and the undo labels were
   wrong ("paste notes" for clips, "delete clip" for a cut, "duplicate notes" for clips).

BACKLOG-excellence §6d.1 asks for notes, clips and time selections; a paste at the
playhead or the selected time/track with grid snap; across projects, with a format that
survives a restart; cut/copy/paste in the menus with their keys.

## Decisions

### 1. The data and the builders live in `cantus-core`

`cantus_core::clipboard` (pure, no I/O, no JSON):

- `Clipboard { format: "cantus-clip/1", schema, origin: ClipOrigin { session, epoch },
  ppq, tempo_bpm, sources: Vec<AudioSource>, payload }`, `Payload::Notes { notes }` (content
  ticks from the copy's origin) or `Payload::Clips { rows: Vec<ClipRow { row, track, clip }>,
  span }` (rows from the topmost track, starts from the earliest clip or from the time
  range's start; `span = Some(to − from)` for a range).
- Builders returning plain commands: `copy_notes`, `copy_clips`, `clips_payload`,
  `cut_clips`, `paste_notes`, `notes_clip`, `paste_clips`, and `copy_of` (moved here from
  `cantus-ui/src/clips.rs`, which re-exports it).
- The time-range crop, `time_crops(project, from, to, ids) -> Vec<Crop { index, track,
  pieces, cut }>`, with the arrangement's `split` and `split_audio` it is built on (moved
  here from `cantus-ui/src/clips.rs` and `audio_clips.rs`; `clips::split` re-exports
  it). Being in the core puts the time cut under the undo fuzz.

Every mutation is a **`Command::Batch` with its own label** (`cmd.cut_notes`,
`cmd.cut_clips`, `cmd.cut_time`, `cmd.paste`, `cmd.paste_clips`): one undo step for a
whole cut or paste, an exact inverse by construction (ADR 0004). No new `Command` variant,
no schema bump, no new crate.

### 2. The system clipboard is the clipboard

The UI (`cantus-ui/src/clipboard.rs`) writes the clipboard as compact JSON with `format`
first (`ctx.copy_text`). The system clipboard outlives the process (Windows keeps
`CF_UNICODETEXT`), so a copy survives a restart and reaches another project or another
Cantus window. The app also keeps the last copy in memory: a paste falls back to it when
the system text is not ours (text from another program, a failed write, a headless test).
Reading probes `{format, schema}` first; another version of the family is refused with a
status ("from a newer version of Cantus"), never misread; a different `ppq` counts as such.

The payload is the live model (`Clip`, `Note`, `AudioSource`), so the text also carries
the **project schema** it was written in (`schema` = `cantus_project::SCHEMA_VERSION`, set
by the UI when it publishes: the core cannot see that crate). A newer schema is refused
like a newer format: serde would drop the fields this build does not know and paste a clip
that plays differently. An older schema is read while it still parses (new fields take
their defaults) and refused with its own status ("from an older version ... copy it
again") when it does not, instead of falling back silently to the in-memory copy. So a
schema bump needs no clipboard change, and `FORMAT` changes only when the clipboard's own
shape does.

### 3. The keys are read as the events they arrive as

`keymap::clipboard_binding(event, modifiers)` turns `Copy` / `Cut` / `Paste(_)` back into
the chord that was pressed (`Ctrl[+Shift][+Alt]+C/X/V`; the chords without Ctrl are the
platform's spellings of the plain one). `Keymap::pressed` fires a binding when its key was
pressed **or** its chord arrived as an event, so rebinding still works and Ctrl+Shift+C
reaches the clip colour. The shortcuts dialog records a clipboard event only when Ctrl is
held (`keymap::captured_clipboard_binding`): Shift+Del and Shift+Ins arrive as the same
events with no key press, and recording them as Ctrl+X / Ctrl+V would bind a chord the user
never pressed. Ctrl+Ins cannot be told from Ctrl+C (both arrive as `Event::Copy` with Ctrl
held), so it records as Ctrl+C, and a binding of Ctrl+Ins never fires.

### 4. Paste: from the key at once, from a menu after asking

`global_shortcuts` takes the frame's `Event::Paste` text before the actions run
(`take_paste_event`); Paste with text in hand pastes now. A menu's Paste has no event: it
sends `ViewportCommand::RequestPaste` and records the frame number and input time.

eframe 0.36 (`glow_integration.rs`, `run_ui_and_paint`) runs a frame as: take the input,
run the app, paint and carry out the queued `actions_requested`, then
`handle_viewport_output`, which only now queues the new `RequestPaste`. So a request sent
in frame N is carried out in frame N+1's paint, after the app ran, and its answer reaches
the app in frame **N+2**. The paste therefore waits until the answer comes, or until at
least `ANSWER_FRAMES` (3) frames **and** `ANSWER_SECS` (0.1 s) went by with none (the
clipboard holds no text: egui-winit then sends nothing), and only then pastes the
in-memory copy. The answer is taken out of the input so it fires no binding and reaches no
text field. An answer that comes after the fallback, with no modifier held (the hand is on
the mouse) within `LATE_SECS` (1 s), is that request's late answer and is dropped; Ctrl+V,
Ctrl held, is always a new paste. Exactly once either way. Without a window (headless) the
menu pastes from memory at once.

### 5. Focus decides; the last-touched selection decides within the arrangement

Cut, Copy and Paste dispatch on `EditFocus` alone (a paste needs no selection; the
`editing_clips()` rule stays for Duplicate, Delete and the arrows). The piano roll and the
channel rack claim `EditFocus::PianoRoll` on a press anywhere in their area
(`piano_roll::claim_focus`), before their early returns. A new `CantusApp::last_selection`
(`SelKind::{Clips, Time}`) is set by every place that makes a clip or time selection; an
arrangement copy takes the time range when it was touched last (or when no clip is
selected), the clips otherwise. Opening a clip's own right-click menu makes the clips the
last-touched selection (`arrangement::clip_menu_opened`), so its Cut and Copy act on the
clip clicked, never on a time range drawn earlier across every track.

### 6. What is copied

- **Notes (piano roll):** the selected notes, relative to the earliest; with none
  selected, every note the clip shows (its window, or one period of a loop), relative to
  the clip's left edge — so a pattern starting on its second beat still does after a paste.
- **Clips:** the clip selection; audio sources go into `sources` with **absolute** paths.
- **A time range:** every playing-lane clip on every track that overlaps `[from, to)`,
  cropped. Note, audio and automation clips are cut with the arrangement's own `split`
  run on a scratch copy (fades stay at the outer edges, a reversed audio clip's halves
  swap their windows). A **looping** clip (a brace shorter than its window) cannot be
  split by moving its offset, because content past the brace is not heard, and the model
  has no loop phase (a looping clip's window always begins at its brace's start). So its
  pieces are new clips (`loop_pieces`): the same loop when a piece starts on a repeat;
  otherwise, for **notes**, the notes heard there written out once; for **automation**,
  two clips (the rest of the repeat played once from the brace tick heard there, then the
  loop again from its next repeat), because a shaped curve segment cut at a repeat's seam
  has no exact shorter twin. Each reads only inside the brace, so both play exactly what
  the loop played. A time cut leaves a gap and moves nothing later (no ripple); take lanes
  are left alone.

### 7. Where a paste lands

- **Arrangement, time:** the empty-timeline menu's point (captured with the right-button
  press, used once), else the time selection's start, else the playhead; snapped **down**
  to the grid when snap is on, raw when it is off; never before zero.
- **Arrangement, track:** the menu's track, else the selected track, else the first.
  Rows keep their offsets; a block that would run past the last track **moves up whole**;
  only a block taller than the project folds its extra rows onto the last track.
- Every pasted clip is a copy (fresh clip and note ids, so glue stays safe — linked clips,
  §6d.2, are a later round) on **lane 0**. The pasted clips are selected.
- **Automation clips** go back to their own track, target unchanged, when the copy came
  from this project in this run (`ClipOrigin`: a per-run nonce and the project epoch) and
  the track still exists; otherwise they are skipped and the status says how many. Ids are
  a per-project counter, so a target from another project would point at a stranger. The
  epoch is bumped by everything that swaps the whole project: New, Open, a recovery, a
  **DAWproject import**, and the **undo or redo of a `ReplaceProject`** (its label,
  `cmd.replace_project`, is checked before the bus steps).
- **Audio clips:** same origin and the source still in the pool → reuse it; a pool entry
  with the same SHA-256 or the same canonical file → reuse it; otherwise insert a new
  source (fresh id, path stored relative to the project when inside its folder,
  **`recorded = false`** so a save never moves another project's file), its
  `InsertAudioSource` before the first clip that plays it. From another project at another
  tempo, the window, offset and fades are scaled by `dst_bpm / src_bpm` so the clip plays
  the same seconds of its file; its start stays on the beat it had (the design had scaled
  the start too; keeping it musical keeps a copied block aligned to its bars).
- **Notes into the piano roll:** into the selected note clip, at the time selection's start
  or the playhead when either is inside the clip, else at its left edge; snapped down when
  snap is on. A clip that plays once grows its window to hold them. A looping clip grows
  its loop by **whole loops** (4 beats to 8, never 5, so the repeats stay on the bar), the
  new room filled with copies of the loop's notes so every repeat heard before is heard
  where it was, and its window grows to show the new loop at least once (with Loop on, the
  window is usually exactly one loop long; growing only the period would leave the paste
  outside the window). Notes stored past the old brace (hidden by it) become audible, as
  when a loop is lengthened by hand.
- **Notes into the arrangement:** a new note clip on the anchor track, as long as the notes
  rounded up to whole bars (at least one).
- **Clips into the piano roll:** a status, nothing pasted.

### 8. Labels and menus

New undo labels in `locales/areas/clipboard.{en,es}.txt`; `cmd.duplicate_clips` fixes the
clip duplicate and the Alt-drag copy. The Edit menu already listed Cut, Copy, Paste and
Duplicate with their keys through `run_action`; the empty-timeline menu's *Paste* now shows
its key and pastes at the point it was opened on (`arrangement::paste_at_menu_point`).

## What the tests hold

- `cantus-core/tests/clipboard.rs` (15): copied notes keep pitch, velocity, length and mute
  from zero; a note paste lands at the anchor with fresh ids and undoes in one step;
  pasting past the end grows the window (and a loop's period, heard in the snapshot); a
  paste past a loop as long as its clip grows both by whole loops, every pasted note and
  every earlier repeat heard;
  rows and spacing are kept; a block below the last track moves up whole; a pasted clip
  shares no ids and lands on lane 0; automation goes home in the same project and is
  skipped from another; audio sources are reused or inserted first (never recorded) and
  undo in a legal order; tempo scaling; a cut undoes exactly; notes make a whole-bar
  clip; the JSON round-trips with `format` first.
- `cantus-core/tests/clipboard_crop.rs` (3): a time cut through a looping automation clip
  (neither edge on a repeat, a curve point past the brace) leaves the compiled curve
  outside the range unchanged, pasting the pieces back restores the whole curve, and undo
  is exact; a range inside one repeat is one piece playing the loop's curve; a range on
  clip edges takes whole clips only.
- `undo_invariants.rs`: four new arms (paste of clips with automation clips, a foreign
  audio file and either origin; a cut of clips or notes; a note paste that may grow its
  clip; a **time cut** whose batch first lays a looping note clip, a looping automation
  clip or a reversed audio clip with fades under the range). The test **asserts each
  clipboard case was really applied** across the seeds, the loop-phase automation crop
  included.
- `cantus-ui/tests/clipboard.rs` (19), driving the whole `CantusApp::frame` with the events
  egui-winit sends: Ctrl+C writes our JSON to the system clipboard; Ctrl+X is one undo step
  named *cut*; our text pastes, foreign text falls back, a newer version is refused;
  across a restart and projects (audio source inserted not-recorded with an absolute path,
  automation skipped and said); cut then paste in the arrangement; paste onto an empty
  track; the empty-timeline menu's point; the piano roll takes the focus on a click; snap
  down, snap off, the time selection's start; time-range copy crops across four kinds of
  clip and the cut splits, removes, pastes back to the same notes heard and undoes
  exactly; the last-touched selection; the clip menu cuts the clicked clip, not a
  leftover range; the menu paste waits for the answer eframe sends two frames later and
  pastes once (one undo step); with no answer it falls back once and drops a late answer,
  while Ctrl+V still pastes; a DAWproject import and its undo/redo make an earlier copy
  foreign; the copied text carries the project schema; every label in both locales.
- `keymap::tests::{clipboard_events_fire_cut_copy_paste,
  capture_records_clipboard_events_only_with_ctrl}`; codec tests in `clipboard::tests`
  (`the_project_schema_decides_whether_a_clipboard_is_read` among them); `app_flows`
  updated to the new field.

## Consequences

- The three chords work in the real window, Ctrl+Shift+C reaches the clip colour, and the
  shortcuts dialog can capture them.
- Copy overwrites what the user had on the system clipboard (the standard behaviour).
- `Event::Paste` comes only when the system clipboard holds text: after copying an image in
  another program, Ctrl+V does nothing even if Cantus copied before (the menu path falls
  back to the in-memory copy). egui-winit then sends neither an event nor a key press, so
  the app cannot even tell that Ctrl+V was pressed in order to say why; Ctrl+V cannot be
  captured in the shortcuts dialog in that state either.
- A menu paste takes about a tenth of a second when the system clipboard holds no text
  (the wait for an answer that never comes), and two frames when it does. On X11 without a clipboard manager the text dies with the
  process; Windows and macOS keep it.
- egui's selectable stock labels (on by default) also answer `Event::Copy` at the end of
  the pass and would overwrite our text while a label selection exists (a dialog's text
  dragged over). Not seen in the views (all kit-drawn); turning `selectable_labels` off in
  `design::install_style` is the fix if it bites.
- Re-opening the same file is a new epoch: automation clips copied before are skipped (the
  conservative choice). Remapping a track-relative target (`TrackGain(src)` →
  `TrackGain(dst)`) is a later improvement, as are linked clips (`cantus-clip/2` will
  carry a link id; reader 1 refuses it politely).
- A large note selection makes a large clipboard text (thousands of notes, megabytes);
  only text that starts with `{` and names the format is parsed.
